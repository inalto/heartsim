//! The grey ultrasound image (brief B32, items 2–5): a line model that turns the label slice of
//! the same frame into a simulated B-mode image, as a `u8` polar image of the [`LabelImage`]'s
//! `lines × samples` layout. It is a **simulated appearance from a simple line model**, written
//! from published formulas (below); no simulator code, no echo recording and no Monte-Carlo or
//! path tracing are used.
//!
//! # The model, per scan line (stages, in this order)
//!
//! 1. **Line model** ([`EchoImager::stage_line_model`]). March the samples; each label's row of
//!    the [`AcousticTable`] gives its impedance Z, attenuation α, backscatter and specularity.
//!    * Two-way attenuation `2·α·f·Δr` accumulates in dB (Beer–Lambert, Mattausch & Goksel 2016).
//!    * At a label change, the reflection `R = ((Z₂ − Z₁)/(Z₂ + Z₁))²` (Shams, Hartley & Navab
//!      2008) gives an interface echo of amplitude `√R · √w` times the gain reached there, and the
//!      transmitted energy `(1 − R)²` (there and back) is carried down the line. `w =
//!      max(cos 2θ, 0)ⁿ` is the incidence weighting (Mattausch 2016's reflected-direction term;
//!      refraction is out of scope), θ the angle between the beam and the interface normal and n
//!      the larger specularity of the two tissues. The normal is the gradient of the deeper
//!      label's indicator smoothed over a ±1 mm box: in the image plane from the label image,
//!      across it from labels read at rest points ±1 mm along the elevation axis.
//!    * Bone (Z 5.69) reflects 30 % and attenuates 10 dB/cm/MHz: a shadow. Lung (treated as air)
//!      reflects 99.9 %; the first pleural echo of a line is repeated at 2, 3 … times its depth
//!      (**designed** reverberation: each repeat loses the transducer face's reflection and one
//!      more round trip of the wall's attenuation), and the lung behind is dark. Blood attenuates
//!      little, so what lies behind it is brighter (enhancement) with no special case.
//! 2. **Speckle** ([`EchoImager::stage_speckle`]). Each sample's complex scatter amplitude is a
//!    hash of its **rest** position ([`LabelImage::rest_mm`]), so the speckle rides with the
//!    tissue and decorrelates when the plane moves through it: nodes of a lattice of
//!    [`AcousticTable::speckle_lattice_mm`] carry `(µ + σ·ξ)·e^{iφ}` where `ξ e^{iφ}` is a circular
//!    complex Gaussian, zero where `ξ` is below the label's density threshold (Mattausch 2016's
//!    thresholded Gaussian scatterers; density `d` keeps a node when `ξ > √(−ln d)`), read by
//!    trilinear interpolation (normalised to unit variance). The lattice read at a sample is at
//!    least [`AcousticTable::speckle_oversample`] times its spacing (axial and lateral), taken as
//!    two blended power-of-two octaves of the finest lattice: a finer lattice would be aliased by
//!    the sampling and the speckle would re-draw with any sub-sample motion.
//! 3. **Image formation** ([`EchoImager::stage_psf`]). A separable point-spread function in polar
//!    space (COLE's insight, Gao 2009): axially a Gaussian of FWHM `cycles · λ / 2` (half the
//!    spatial pulse length), laterally a Gaussian over the lines of FWHM `λ · focus / aperture`,
//!    widening beyond the focus. The scatter is convolved at baseband (complex) and the envelope
//!    taken as its magnitude; the kernels are scaled so that the mean speckle power does not
//!    depend on depth or frequency. The interface echoes are blurred by the same kernels (peak 1)
//!    and added to the envelope (Shams 2008's `(I_r + α)·I_s` in spirit).
//! 4. **Compensation and display** ([`EchoImager::stage_compress`]). Time-gain compensation
//!    (by default `2 · α_ref · f · depth` with α_ref = [`AcousticTable::tgc_reference_db_cm_mhz`],
//!    plus [`TGC_BANDS`] per-depth sliders), overall gain, log compression over the dynamic range
//!    (white at [`AcousticTable::display_top_db`]), quantised to `u8`.
//!
//! # Using it (the page)
//!
//! ```text
//! // once:
//! let table = AcousticTable::builtin();             // assets/echo/acoustic.ron, validated
//! let mut imager = EchoImager::new(table);
//! let mut settings = ImageSettings::default();      // ranges in ImageSettings::*_RANGE_*
//! let mut labels = settings.label_image();          // LabelImage::new(lines, samples)
//! let mut grey = GreyImage::new(settings.lines, settings.samples);
//!
//! // every drawn frame, after the slice state took this frame (B31):
//! let pose = settings.apply_to(&pose);              // depth and sector from the settings
//! state.slice(&anatomy, &pose, &mut labels);
//! imager.image(&anatomy, &state, &pose, &labels, &settings, &mut grey);
//! // grey.pixels: lines × samples bytes (line-major, the LabelImage layout); grey.contact
//! ```
//!
//! **Pure and deterministic**: the image is a function of (label image, probe frame, the slice
//! state's rest labels, table, settings, seed) only, computed on the CPU in `f32` with the `libm`
//! crate's portable functions, so the same inputs give the same bytes. The motion is not
//! evaluated again: the label slice of the same frame carries the rest positions.

use crate::acoustic::AcousticTable;
use crate::anatomy::EchoAnatomy;
use crate::probe::{
    DEFAULT_DEPTH_MM, DEFAULT_SECTOR_DEG, DEPTH_RANGE_MM, ProbeFrame, ProbePose, SECTOR_RANGE_DEG,
};
use crate::slice::{DEFAULT_LINES, DEFAULT_SAMPLES, LabelImage, SliceState};
use glam::Vec3;
use heartsim_geometry::Label;
use std::sync::Arc;

/// Number of time-gain-compensation sliders, spread evenly over the image depth.
pub const TGC_BANDS: usize = 8;
/// Half-size of the box the interface normal is smoothed over, mm. **Designed**.
const NORMAL_BOX_MM: f32 = 1.0;
/// Most lines the lateral kernel reaches on each side (near the apex the lines crowd).
const MAX_LATERAL_RADIUS: usize = 8;
/// Below this gain (amplitude, ≈ −160 dB) a sample's scatter is not evaluated.
const NEGLIGIBLE_GAIN: f32 = 1e-8;
/// Size of the table of complex Gaussian values the lattice hash indexes (power of two).
const GAUSS_TABLE: usize = 4096;
/// FWHM of a Gaussian over its σ, `2·√(2 ln 2)`.
const FWHM_OVER_SIGMA: f32 = 2.354_82;

/// The settings of the grey image: what the page's controls change. Every consumer takes
/// [`Self::clamped`], so out-of-range or non-finite values never reach the model.
#[derive(Debug, Clone, PartialEq)]
pub struct ImageSettings {
    /// Transmit frequency, MHz ([`Self::FREQUENCY_RANGE_MHZ`]; "typical range 2.0 to 5.0 MHz",
    /// Mitchell 2019). Higher: finer speckle, less penetration.
    pub frequency_mhz: f32,
    /// Imaging depth, mm (the probe's [`DEPTH_RANGE_MM`]); applied to the pose by
    /// [`Self::apply_to`].
    pub depth_mm: f32,
    /// Sector width, degrees (the probe's [`SECTOR_RANGE_DEG`]).
    pub sector_deg: f32,
    /// Transmit focus depth, mm ([`Self::FOCUS_RANGE_MM`], at most the depth).
    pub focus_mm: f32,
    /// Overall gain, dB ([`Self::GAIN_RANGE_DB`]).
    pub gain_db: f32,
    /// Whether the default compensation (`2 · α_ref · f · depth`) is applied under the sliders.
    pub tgc_auto: bool,
    /// Per-depth gain sliders, dB ([`Self::TGC_RANGE_DB`]); slider `i` sits at depth
    /// `(i + ½)/TGC_BANDS` of the image, linear in between, flat beyond the ends.
    pub tgc_db: [f32; TGC_BANDS],
    /// Displayed dynamic range, dB ([`Self::DYNAMIC_RANGE_DB`]; 60 dB default, research note
    /// §3.2). Smaller: more contrast.
    pub dynamic_range_db: f32,
    /// Image lines ([`Self::LINES_RANGE`]).
    pub lines: usize,
    /// Samples per line ([`Self::SAMPLES_RANGE`]).
    pub samples: usize,
    /// Seed of the scatterer hash (the scenario's seed).
    pub seed: u64,
}

impl Default for ImageSettings {
    fn default() -> Self {
        Self {
            frequency_mhz: 2.5,
            depth_mm: DEFAULT_DEPTH_MM,
            sector_deg: DEFAULT_SECTOR_DEG,
            focus_mm: 80.0,
            gain_db: 0.0,
            tgc_auto: true,
            tgc_db: [0.0; TGC_BANDS],
            dynamic_range_db: 60.0,
            lines: DEFAULT_LINES,
            samples: DEFAULT_SAMPLES,
            seed: 0,
        }
    }
}

fn finite_or(v: f32, d: f32) -> f32 {
    if v.is_finite() { v } else { d }
}

impl ImageSettings {
    /// Transmit frequency range, MHz.
    pub const FREQUENCY_RANGE_MHZ: (f32, f32) = (2.0, 5.0);
    /// Focus range, mm (the upper end is also capped at the depth).
    pub const FOCUS_RANGE_MM: (f32, f32) = (20.0, 300.0);
    /// Gain range, dB.
    pub const GAIN_RANGE_DB: (f32, f32) = (-30.0, 30.0);
    /// Range of each TGC slider, dB.
    pub const TGC_RANGE_DB: (f32, f32) = (-20.0, 20.0);
    /// Dynamic range, dB.
    pub const DYNAMIC_RANGE_DB: (f32, f32) = (30.0, 90.0);
    /// Lines per image.
    pub const LINES_RANGE: (usize, usize) = (32, 256);
    /// Samples per line.
    pub const SAMPLES_RANGE: (usize, usize) = (128, 1024);

    /// Every field finite and in range (non-finite values take the default).
    #[must_use]
    pub fn clamped(&self) -> Self {
        let d = Self::default();
        let c = |v: f32, dv: f32, r: (f32, f32)| finite_or(v, dv).clamp(r.0, r.1);
        let depth_mm = c(self.depth_mm, d.depth_mm, DEPTH_RANGE_MM);
        let mut tgc_db = self.tgc_db;
        for t in &mut tgc_db {
            *t = c(*t, 0.0, Self::TGC_RANGE_DB);
        }
        Self {
            frequency_mhz: c(self.frequency_mhz, d.frequency_mhz, Self::FREQUENCY_RANGE_MHZ),
            depth_mm,
            sector_deg: c(self.sector_deg, d.sector_deg, SECTOR_RANGE_DEG),
            focus_mm: c(self.focus_mm, d.focus_mm, Self::FOCUS_RANGE_MM).min(depth_mm),
            gain_db: c(self.gain_db, d.gain_db, Self::GAIN_RANGE_DB),
            tgc_auto: self.tgc_auto,
            tgc_db,
            dynamic_range_db: c(self.dynamic_range_db, d.dynamic_range_db, Self::DYNAMIC_RANGE_DB),
            lines: self.lines.clamp(Self::LINES_RANGE.0, Self::LINES_RANGE.1),
            samples: self.samples.clamp(Self::SAMPLES_RANGE.0, Self::SAMPLES_RANGE.1),
            seed: self.seed,
        }
    }

    /// `pose` with this depth and sector width (the slice reads them from the pose).
    #[must_use]
    pub fn apply_to(&self, pose: &ProbePose) -> ProbePose {
        let s = self.clamped();
        ProbePose {
            depth_mm: s.depth_mm,
            sector_deg: s.sector_deg,
            ..*pose
        }
    }

    /// An empty label image of this size, for [`SliceState::slice`].
    #[must_use]
    pub fn label_image(&self) -> LabelImage {
        let s = self.clamped();
        LabelImage::new(s.lines, s.samples)
    }

    /// The compensation the sliders add at `depth_mm` of an image `image_depth_mm` deep, dB.
    #[must_use]
    pub fn slider_tgc_db(&self, depth_mm: f32, image_depth_mm: f32) -> f32 {
        let u = (depth_mm / image_depth_mm.max(1e-3)) * TGC_BANDS as f32 - 0.5;
        if u <= 0.0 {
            return self.tgc_db[0];
        }
        let i = u.floor() as usize;
        if i >= TGC_BANDS - 1 {
            return self.tgc_db[TGC_BANDS - 1];
        }
        let t = u - i as f32;
        self.tgc_db[i] * (1.0 - t) + self.tgc_db[i + 1] * t
    }
}

/// The grey polar image: `lines × samples` bytes, line-major — the [`LabelImage`] layout (line 0
/// away from the marker, sample 0 at the skin). An `R8` texture for the scan conversion.
#[derive(Debug, Clone, PartialEq)]
pub struct GreyImage {
    pub lines: usize,
    pub samples: usize,
    pub pixels: Vec<u8>,
    /// Whether the probe had contact (without it the image is black).
    pub contact: bool,
    /// Depth, sector and focus of the image, mm and degrees.
    pub depth_mm: f32,
    pub sector_deg: f32,
    pub focus_mm: f32,
}

impl GreyImage {
    /// A black image of `lines × samples` (each at least 1).
    #[must_use]
    pub fn new(lines: usize, samples: usize) -> Self {
        let (lines, samples) = (lines.max(1), samples.max(1));
        Self {
            lines,
            samples,
            pixels: vec![0; lines * samples],
            contact: false,
            depth_mm: 0.0,
            sector_deg: 0.0,
            focus_mm: 0.0,
        }
    }

    /// The grey level of a sample.
    #[must_use]
    pub fn pixel(&self, line: usize, sample: usize) -> u8 {
        self.pixels[line * self.samples + sample]
    }

    /// Depth of sample `s`, mm (its centre; as [`LabelImage::depth_of`]).
    #[must_use]
    pub fn depth_of(&self, sample: usize) -> f32 {
        (sample as f32 + 0.5) / self.samples as f32 * self.depth_mm
    }
}

/// A label's row compiled for the inner loops.
#[derive(Debug, Clone, Copy, Default)]
struct Row {
    z: f32,
    alpha: f32,
    /// Backscatter amplitude (linear).
    amp: f32,
    mean: f32,
    spread: f32,
    /// Density threshold on ξ.
    thr: f32,
    spec: f32,
}

/// Per sample row (depth) of the current geometry: the speckle octave and the kernels.
#[derive(Debug, Clone, Default)]
struct Depth {
    /// Octave `k` (lattice `L₀·2ᵏ`), blend weight `t` toward `k + 1`, and the blend's norm.
    octave: u32,
    t: f32,
    norm: f32,
    inv_lk: [f32; 2],
    /// Lateral kernel: speckle weights (power-normalised) and interface weights (peak 1).
    lat_w: Vec<f32>,
    lat_wi: Vec<f32>,
    /// The kernel's radius in lines.
    lat_rad: usize,
    /// Compensation + gain at this depth, dB.
    comp_db: f32,
}

/// The grey-image engine: the table compiled once and the scratch buffers of the last frame.
/// See the module docs.
#[derive(Debug, Clone)]
pub struct EchoImager {
    table: Arc<AcousticTable>,
    rows: Box<[Row; 256]>,
    /// Complex Gaussian values `(re, im, |·|)`, `E|ζ|² = 1`.
    gauss: Vec<[f32; 3]>,
    lines: usize,
    samples: usize,
    /// Per sample: amplitude gain (√transmission × attenuation) at its centre.
    gain: Vec<f32>,
    /// Per sample: interface echo amplitude (before the PSF).
    iface: Vec<f32>,
    /// Per sample: complex scatter (before, then after, the PSF).
    scatter: Vec<[f32; 2]>,
    tmp: Vec<[f32; 2]>,
    tmp_i: Vec<f32>,
    /// Per sample: the envelope after compensation and gain (linear amplitude).
    env: Vec<f32>,
    depths: Vec<Depth>,
    /// Axial kernels (speckle power-normalised per depth row, interface peak 1) and radius.
    ax_w: Vec<Vec<f32>>,
    ax_wi: Vec<f32>,
    contact: bool,
}

/// Murmur3's 32-bit finaliser.
fn fmix32(mut h: u32) -> u32 {
    h ^= h >> 16;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^= h >> 16;
    h
}

/// Hash of one lattice coordinate (a node's hash is `fmix32` of the three combined with the
/// seed and the octave).
#[inline]
fn axis_hash(v: i32, mul: u32) -> u32 {
    fmix32((v as u32).wrapping_mul(mul).wrapping_add(0x1b87_3593))
}

/// SplitMix64.
fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// The table of complex Gaussians (Box–Muller on a fixed SplitMix64 stream, `libm`).
fn gauss_table() -> Vec<[f32; 3]> {
    let mut st = 0x5eed_ec40_0000_b32a_u64;
    (0..GAUSS_TABLE)
        .map(|_| {
            let u1 = ((splitmix(&mut st) >> 11) as f64 + 1.0) / (1u64 << 53) as f64; // (0, 1]
            let u2 = (splitmix(&mut st) >> 11) as f64 / (1u64 << 53) as f64;
            // E|ζ|² = 1: each component has variance ½
            let r = libm::sqrt(-libm::log(u1));
            let (s, c) = libm::sincos(2.0 * std::f64::consts::PI * u2);
            [(r * c) as f32, (r * s) as f32, r as f32]
        })
        .collect()
}

/// Correlation of trilinearly interpolated white noise at a shift of `d` lattice spacings along
/// one axis (the cubic B-spline, normalised to 1 at 0).
fn bspline_corr(d: f32) -> f32 {
    let a = d.abs();
    let v = if a < 1.0 {
        2.0 / 3.0 - a * a + 0.5 * a * a * a
    } else if a < 2.0 {
        (2.0 - a).powi(3) / 6.0
    } else {
        0.0
    };
    v * 1.5
}

/// Gaussian weights of standard deviation `sigma` (in samples) out to `radius`.
fn gaussian(sigma: f32, radius: usize) -> Vec<f32> {
    let s2 = 2.0 * sigma.max(1e-3) * sigma.max(1e-3);
    (0..=2 * radius)
        .map(|i| {
            let d = i as f32 - radius as f32;
            libm::expf(-d * d / s2)
        })
        .collect()
}

/// `w` scaled so that `Σᵢⱼ wᵢ wⱼ ρ(i − j) = 1` for an input correlated as `rho`.
fn power_normalised(w: &[f32], rho: impl Fn(f32) -> f32) -> Vec<f32> {
    let mut g = 0.0f32;
    for (i, a) in w.iter().enumerate() {
        for (j, b) in w.iter().enumerate() {
            g += a * b * rho(i as f32 - j as f32);
        }
    }
    let k = 1.0 / libm::sqrtf(g.max(1e-12));
    w.iter().map(|v| v * k).collect()
}

impl EchoImager {
    /// An imager for `table` (already validated: [`AcousticTable::from_ron`] or
    /// [`AcousticTable::builtin`]).
    #[must_use]
    pub fn new(table: Arc<AcousticTable>) -> Self {
        let mut rows = Box::new([Row::default(); 256]);
        let outside = compile_row(&table, Label::Outside);
        rows.fill(outside);
        for l in Label::ALL {
            rows[l as usize] = compile_row(&table, l);
        }
        Self {
            table,
            rows,
            gauss: gauss_table(),
            lines: 0,
            samples: 0,
            gain: Vec::new(),
            iface: Vec::new(),
            scatter: Vec::new(),
            tmp: Vec::new(),
            tmp_i: Vec::new(),
            env: Vec::new(),
            depths: Vec::new(),
            ax_w: Vec::new(),
            ax_wi: Vec::new(),
            contact: false,
        }
    }

    /// The table in use.
    #[must_use]
    pub fn table(&self) -> &AcousticTable {
        &self.table
    }

    /// The envelope of the last image after compensation and gain, before log compression
    /// (linear amplitude, `lines × samples`, the image layout). For tests and diagnostics.
    #[must_use]
    pub fn envelope(&self) -> &[f32] {
        &self.env
    }

    /// The grey image of this frame: `labels` must be what `state.slice(anatomy, pose, labels)`
    /// produced for this frame (the same `pose`, the same state), `settings` the image settings
    /// (depth and sector are taken from `labels`, which took them from the pose — see
    /// [`ImageSettings::apply_to`]). Fills `out` (resized to the label image).
    pub fn image(
        &mut self,
        anatomy: &EchoAnatomy,
        state: &SliceState,
        pose: &ProbePose,
        labels: &LabelImage,
        settings: &ImageSettings,
        out: &mut GreyImage,
    ) {
        let frame = pose.frame(anatomy.designed.contact_limit_deg);
        self.image_with(
            labels,
            &frame,
            settings,
            |rest, heart| state.rest_label(anatomy, rest, heart) as u8,
            out,
        );
    }

    /// [`Self::image`] from any label image: `frame` is the probe frame the image was cut with,
    /// `rest_label(rest_mm, from_heart)` the label at a rest point (for the interface normals
    /// across the plane). Synthetic tests use it with hand-made label images.
    pub fn image_with(
        &mut self,
        labels: &LabelImage,
        frame: &ProbeFrame,
        settings: &ImageSettings,
        rest_label: impl Fn(Vec3, bool) -> u8,
        out: &mut GreyImage,
    ) {
        let s = settings.clamped();
        self.stage_line_model(labels, frame, &s, rest_label);
        self.stage_speckle(labels, &s);
        self.stage_psf();
        self.stage_compress(labels, &s, out);
    }

    /// Stage 1: per sample the gain reached (attenuation and transmission) and the interface
    /// echoes (with the lung reverberation); also prepares the depth-dependent kernels.
    #[doc(hidden)]
    pub fn stage_line_model(
        &mut self,
        labels: &LabelImage,
        frame: &ProbeFrame,
        s: &ImageSettings,
        rest_label: impl Fn(Vec3, bool) -> u8,
    ) {
        let (lines, samples) = (labels.lines, labels.samples);
        let n = lines * samples;
        self.lines = lines;
        self.samples = samples;
        self.contact = labels.contact && labels.labels.len() == n;
        for v in [&mut self.gain, &mut self.iface, &mut self.env, &mut self.tmp_i] {
            v.clear();
            v.resize(n, 0.0);
        }
        for v in [&mut self.scatter, &mut self.tmp] {
            v.clear();
            v.resize(n, [0.0; 2]);
        }
        self.prepare_depths(labels, s);
        if !self.contact {
            return;
        }
        let t = &*self.table;
        let f = s.frequency_mhz;
        let dr = labels.depth_mm / samples as f32;
        let dr_cm = dr * 0.1;
        let dtheta = labels.sector_deg.to_radians() / lines as f32;
        let face = t.reverberation_face_reflection.v();
        let repeats = t.reverberation_repeats.v() as usize;
        let h_ax = ((NORMAL_BOX_MM / dr).round() as usize).max(1);
        let lab = &labels.labels;
        // per label, the amplitude kept over half a sample, there and back
        let mut half_k = [1.0f32; 256];
        for (k, r) in half_k.iter_mut().zip(self.rows.iter()) {
            *k = libm::exp10f(-r.alpha * f * dr_cm / 20.0);
        }
        for line in 0..lines {
            let base = line * samples;
            let mut att_db = 0.0f32; // two-way, to the top of the current sample
            let mut att = 1.0f32; // the same as an amplitude factor
            let mut trans_amp = 1.0f32; // √ of the two-way energy transmission
            let mut prev = lab[base];
            let mut pleura: Option<(usize, f32, f32, f32)> = None;
            for smp in 0..samples {
                let l = lab[base + smp];
                let row = self.rows[l as usize];
                if smp > 0 && l != prev {
                    let pr = self.rows[prev as usize];
                    let r_amp = (row.z - pr.z).abs() / (row.z + pr.z);
                    let r_energy = r_amp * r_amp;
                    if r_amp > 1e-4 {
                        let g_before = trans_amp * att;
                        let spec = row.spec.max(pr.spec);
                        let w = if spec > 0.0 {
                            let r_mm = smp as f32 * dr;
                            let c = incidence_cos(
                                labels,
                                frame,
                                line,
                                smp,
                                l,
                                h_ax,
                                dr,
                                r_mm * dtheta,
                                &rest_label,
                            );
                            let c2 = (2.0 * c * c - 1.0).max(0.0);
                            libm::powf(c2, spec)
                        } else {
                            1.0
                        };
                        let a = r_amp * libm::sqrtf(w) * g_before;
                        self.iface[base + smp] += a;
                        if pleura.is_none() && l == Label::Lung as u8 && prev != Label::Lung as u8 {
                            pleura = Some((smp, a, att_db, r_amp));
                        }
                    }
                    trans_amp *= 1.0 - r_energy;
                    prev = l;
                }
                let hk = half_k[l as usize];
                self.gain[base + smp] = trans_amp * att * hk;
                att *= hk * hk;
                att_db += 2.0 * row.alpha * f * dr_cm;
            }
            // designed reverberation: the pleural echo again at k times its depth
            if let Some((sp, a, att_p, r_p)) = pleura {
                // each repeat: back from the face, through the wall twice more, off the pleura
                let q = face * r_p * libm::exp10f(-att_p / 20.0);
                let mut amp = a;
                for k in 2..=repeats + 1 {
                    let sk = k * sp;
                    if sk >= samples {
                        break;
                    }
                    amp *= q;
                    self.iface[base + sk] += amp;
                }
            }
        }
    }

    /// Per sample row: speckle octave, lateral kernels, compensation; and the axial kernels.
    fn prepare_depths(&mut self, labels: &LabelImage, s: &ImageSettings) {
        let t = &*self.table;
        let samples = labels.samples;
        let lines = labels.lines;
        let depth = labels.depth_mm.max(1e-3);
        let dr = depth / samples as f32;
        let dtheta = labels.sector_deg.to_radians() / lines as f32;
        let lambda = t.speed_of_sound_m_s.v() / (s.frequency_mhz * 1000.0);
        let l0 = t.speckle_lattice_mm.v();
        let over = t.speckle_oversample.v();
        let ap = t.aperture_mm.v();
        let widen = t.beyond_focus_widening.v();
        let focus = s.focus_mm.min(depth);
        let auto = if s.tgc_auto {
            2.0 * t.tgc_reference_db_cm_mhz.v() * s.frequency_mhz * 0.1
        } else {
            0.0
        };
        // axial kernel
        let sig_ax = t.pulse_cycles.v() * lambda * 0.5 / FWHM_OVER_SIGMA / dr;
        let rad_ax = ((3.0 * sig_ax).ceil() as usize).clamp(1, 16);
        let g_ax = gaussian(sig_ax, rad_ax);
        self.ax_wi.clone_from(&g_ax);
        self.depths.clear();
        self.ax_w.clear();
        for smp in 0..samples {
            let r = (smp as f32 + 0.5) * dr;
            let spacing = over * l0.max(dr).max(r * dtheta);
            let level = libm::log2f(spacing / l0).max(0.0);
            let octave = level.floor() as u32;
            let tb = level - octave as f32;
            let lk = l0 * libm::exp2f(octave as f32);
            let w0 = 1.0 - tb;
            let norm = 1.0 / libm::sqrtf(w0 * w0 + tb * tb);
            let rho = |d_mm: f32| {
                (w0 * w0 * bspline_corr(d_mm / lk) + tb * tb * bspline_corr(d_mm / (2.0 * lk))) * norm * norm
            };
            self.ax_w.push(power_normalised(&g_ax, |d| rho(d * dr)));
            let fwhm = lambda / ap * (focus + widen * (r - focus).max(0.0));
            let lat_sp = (r * dtheta).max(1e-4);
            let sig_l = fwhm / FWHM_OVER_SIGMA / lat_sp;
            let rad_l = ((3.0 * sig_l).ceil() as usize).clamp(1, MAX_LATERAL_RADIUS);
            let g_l = gaussian(sig_l, rad_l);
            let lat_w = power_normalised(&g_l, |d| rho(d * lat_sp));
            let comp_db = auto * r + s.slider_tgc_db(r, depth) + s.gain_db;
            self.depths.push(Depth {
                octave,
                t: tb,
                norm,
                inv_lk: [1.0 / lk, 0.5 / lk],
                lat_w,
                lat_wi: g_l,
                lat_rad: rad_l,
                comp_db,
            });
        }
    }

    /// Stage 2: the complex scatter of every sample, from its rest position, scaled by the
    /// gain the line model reached there.
    #[doc(hidden)]
    pub fn stage_speckle(&mut self, labels: &LabelImage, s: &ImageSettings) {
        if !self.contact {
            return;
        }
        let seed = (s.seed ^ (s.seed >> 32)) as u32;
        let samples = self.samples;
        for line in 0..self.lines {
            let base = line * samples;
            for smp in 0..samples {
                let idx = base + smp;
                let row = self.rows[labels.labels[idx] as usize];
                let g = self.gain[idx] * row.amp;
                if g < NEGLIGIBLE_GAIN || (row.spread <= 0.0 && row.mean <= 0.0) {
                    continue;
                }
                let d = &self.depths[smp];
                let x = labels.rest_mm[idx];
                let mut acc = [0.0f32; 2];
                for (o, w) in [(0u32, 1.0 - d.t), (1, d.t)] {
                    if w <= 0.0 {
                        continue;
                    }
                    let v = self.lattice(x * d.inv_lk[o as usize], d.octave + o, seed, &row);
                    acc[0] += w * v[0];
                    acc[1] += w * v[1];
                }
                let k = g * d.norm;
                self.scatter[idx] = [acc[0] * k, acc[1] * k];
            }
        }
    }

    /// The unit-variance interpolated lattice value at lattice coordinates `q`.
    #[inline]
    fn lattice(&self, q: Vec3, octave: u32, seed: u32, row: &Row) -> [f32; 2] {
        let fl = q.floor();
        let fr = q - fl;
        let (i, j, k) = (fl.x as i32, fl.y as i32, fl.z as i32);
        let wx = [1.0 - fr.x, fr.x];
        let wy = [1.0 - fr.y, fr.y];
        let wz = [1.0 - fr.z, fr.z];
        let plain = row.mean <= 0.0 && row.thr <= 0.0;
        let mut acc = [0.0f32; 2];
        let base = seed ^ octave.wrapping_mul(0x9e37_79b9);
        let hx = [axis_hash(i, 0x85eb_ca6b), axis_hash(i + 1, 0x85eb_ca6b)];
        let hy = [axis_hash(j, 0xc2b2_ae35), axis_hash(j + 1, 0xc2b2_ae35)];
        let hz = [axis_hash(k, 0x27d4_eb2f), axis_hash(k + 1, 0x27d4_eb2f)];
        for (dz, wzv) in wz.iter().enumerate() {
            for (dy, wyv) in wy.iter().enumerate() {
                let wyz = wyv * wzv;
                let hyz = base ^ hy[dy] ^ hz[dz].rotate_left(16);
                for (dx, wxv) in wx.iter().enumerate() {
                    let h = fmix32(hyz ^ hx[dx].rotate_left(8));
                    let g = self.gauss[(h >> 20) as usize];
                    let w = wxv * wyz;
                    if plain {
                        acc[0] += w * g[0];
                        acc[1] += w * g[1];
                    } else if g[2] > row.thr {
                        let a = (row.mean + row.spread * g[2]) / g[2].max(1e-12);
                        acc[0] += w * a * g[0];
                        acc[1] += w * a * g[1];
                    }
                }
            }
        }
        let s = if plain { row.spread } else { 1.0 };
        let var = (wx[0] * wx[0] + wx[1] * wx[1])
            * (wy[0] * wy[0] + wy[1] * wy[1])
            * (wz[0] * wz[0] + wz[1] * wz[1]);
        let k = s / libm::sqrtf(var);
        [acc[0] * k, acc[1] * k]
    }

    /// Stage 3: the separable polar PSF — axial along each line, then lateral across the lines
    /// (the kernel of each depth row) — and the envelope (scatter magnitude plus the blurred
    /// interface echoes) into the envelope buffer.
    #[doc(hidden)]
    pub fn stage_psf(&mut self) {
        let (lines, samples) = (self.lines, self.samples);
        if !self.contact {
            return;
        }
        // axial
        let rad = (self.ax_wi.len() - 1) / 2;
        for line in 0..lines {
            let base = line * samples;
            for smp in 0..samples {
                let w = &self.ax_w[smp];
                let lo = smp.saturating_sub(rad);
                let hi = (smp + rad).min(samples - 1);
                let mut acc = [0.0f32; 2];
                let mut acc_i = 0.0f32;
                for k in lo..=hi {
                    let wi = k + rad - smp;
                    let v = self.scatter[base + k];
                    acc[0] += w[wi] * v[0];
                    acc[1] += w[wi] * v[1];
                    acc_i += self.ax_wi[wi] * self.iface[base + k];
                }
                self.tmp[base + smp] = acc;
                self.tmp_i[base + smp] = acc_i;
            }
        }
        // lateral (offset by offset, the samples of a line contiguous), then the envelope
        let width = 2 * MAX_LATERAL_RADIUS + 1;
        let mut w = vec![0.0f32; width * samples];
        let mut wi = vec![0.0f32; width * samples];
        for (smp, d) in self.depths.iter().enumerate() {
            for o in 0..=2 * d.lat_rad {
                let col = o + MAX_LATERAL_RADIUS - d.lat_rad;
                w[col * samples + smp] = d.lat_w[o];
                wi[col * samples + smp] = d.lat_wi[o];
            }
        }
        for line in 0..lines {
            let out = line * samples;
            let acc = &mut self.scatter[out..out + samples];
            acc.fill([0.0; 2]);
            let acc_i = &mut self.env[out..out + samples];
            acc_i.fill(0.0);
            for col in 0..width {
                // the source line `line + col − MAX_LATERAL_RADIUS`, when it exists
                let Some(k) = (line + col)
                    .checked_sub(MAX_LATERAL_RADIUS)
                    .filter(|k| *k < lines)
                else {
                    continue;
                };
                let src = &self.tmp[k * samples..(k + 1) * samples];
                let src_i = &self.tmp_i[k * samples..(k + 1) * samples];
                let wc = &w[col * samples..(col + 1) * samples];
                let wic = &wi[col * samples..(col + 1) * samples];
                for smp in 0..samples {
                    acc[smp][0] += wc[smp] * src[smp][0];
                    acc[smp][1] += wc[smp] * src[smp][1];
                    acc_i[smp] += wic[smp] * src_i[smp];
                }
            }
            for smp in 0..samples {
                let v = acc[smp];
                acc_i[smp] += libm::sqrtf(v[0] * v[0] + v[1] * v[1]);
            }
        }
    }

    /// Stage 4: compensation and gain (into the envelope buffer), log compression over the
    /// dynamic range, `u8`.
    #[doc(hidden)]
    pub fn stage_compress(&mut self, labels: &LabelImage, s: &ImageSettings, out: &mut GreyImage) {
        let (lines, samples) = (self.lines, self.samples);
        if out.lines != lines || out.samples != samples || out.pixels.len() != lines * samples {
            *out = GreyImage::new(lines, samples);
        }
        out.contact = self.contact;
        out.depth_mm = labels.depth_mm;
        out.sector_deg = labels.sector_deg;
        out.focus_mm = s.focus_mm.min(labels.depth_mm);
        if !self.contact {
            out.pixels.fill(0);
            return;
        }
        let dr_db = s.dynamic_range_db;
        let floor_db = self.table.display_top_db.v() - dr_db;
        let scale = 255.0 / dr_db;
        for smp in 0..samples {
            let k = libm::exp10f(self.depths[smp].comp_db / 20.0);
            for line in 0..lines {
                let idx = line * samples + smp;
                let e = self.env[idx] * k;
                self.env[idx] = e;
                let y = if e > 0.0 {
                    20.0 * libm::log10f(e) - floor_db
                } else {
                    0.0
                };
                out.pixels[idx] = (y * scale).clamp(0.0, 255.0).round() as u8;
            }
        }
    }
}

fn compile_row(t: &AcousticTable, l: Label) -> Row {
    let r = t.row(l);
    let d = r.scatter_density.v().clamp(1e-6, 1.0);
    Row {
        z: r.impedance_mrayl.v(),
        alpha: r.attenuation_db_cm_mhz.v(),
        amp: libm::exp10f(r.backscatter_db.v() / 20.0),
        mean: r.scatter_mean.v(),
        spread: r.scatter_spread.v(),
        thr: libm::sqrtf(-libm::logf(d)),
        spec: r.specularity.v(),
    }
}

/// |cos θ| between the beam (along the line) and the normal of the interface entering `label`
/// at `(line, smp)`: the regression slope of the label's indicator over a ±1 mm box — in plane
/// from the label image (`h_ax` samples of `dr` mm, lines `lat_mm` apart at this depth), across
/// it from `rest_label` at rest points ±⅓, ⅔, 1 mm along the elevation axis.
#[allow(clippy::too_many_arguments)]
fn incidence_cos(
    labels: &LabelImage,
    frame: &ProbeFrame,
    line: usize,
    smp: usize,
    label: u8,
    h_ax: usize,
    dr: f32,
    lat_mm: f32,
    rest_label: &impl Fn(Vec3, bool) -> u8,
) -> f32 {
    let (lines, samples) = (labels.lines, labels.samples);
    let k_lat = ((NORMAL_BOX_MM / lat_mm.max(1e-4)).round() as usize).clamp(1, 6);
    let (mut sa, mut saa, mut sl, mut sll) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    let (mut sx, mut sy, mut mean_i, mut count) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    let lo_s = smp.saturating_sub(h_ax);
    let hi_s = (smp + h_ax).min(samples);
    let lo_l = line.saturating_sub(k_lat);
    let hi_l = (line + k_lat).min(lines - 1);
    // regression of the indicator on (axial, lateral) offsets, centred on the boundary
    for l2 in lo_l..=hi_l {
        let y = (l2 as f32 - line as f32) * lat_mm;
        for s2 in lo_s..hi_s {
            let x = (s2 as f32 + 0.5 - smp as f32) * dr;
            let ind = f32::from(u8::from(labels.labels[l2 * samples + s2] == label));
            sa += ind * x;
            saa += x * x;
            sl += ind * y;
            sll += y * y;
            sx += x;
            sy += y;
            mean_i += ind;
            count += 1.0;
        }
    }
    // the indicator's mean removed (the box is not symmetric at the image edges)
    let m = mean_i / count.max(1.0);
    let g_ax = if saa > 0.0 { (sa - m * sx) / saa } else { 0.0 };
    let g_lat = if sll > 0.0 { (sl - m * sy) / sll } else { 0.0 };
    // across the plane, at rest
    let idx = line * samples + smp;
    let rest = labels.rest_mm[idx];
    let heart = labels.from_heart[idx];
    let (mut se, mut see) = (0.0f32, 0.0f32);
    let mut me = 0.0f32;
    let offs = [-3.0f32, -2.0, -1.0, 1.0, 2.0, 3.0];
    let mut ind_e = [0.0f32; 6];
    for (k, o) in offs.iter().enumerate() {
        let z = o * NORMAL_BOX_MM / 3.0;
        ind_e[k] = f32::from(u8::from(rest_label(rest + frame.elevation * z, heart) == label));
        me += ind_e[k];
    }
    me /= offs.len() as f32;
    for (k, o) in offs.iter().enumerate() {
        let z = o * NORMAL_BOX_MM / 3.0;
        se += (ind_e[k] - me) * z;
        see += z * z;
    }
    let g_el = se / see;
    let n2 = g_ax * g_ax + g_lat * g_lat + g_el * g_el;
    if n2 <= 1e-12 {
        1.0
    } else {
        g_ax.abs() / libm::sqrtf(n2)
    }
}
