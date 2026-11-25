//! Calipers, named measurements and the model's own values to compare them with (brief B34,
//! echo simulator phase 5).
//!
//! **The teaching loop is: measure, then compare with the truth of the model.** A measurement is a
//! number the learner reads off the frozen sector, the M-mode or the Doppler trace with calipers.
//! The *reference* is what the model itself holds at the same place and instant: the boundaries
//! of the structure along the same line in the label slice, or the motion of the annulus along the
//! same beam. Nothing here is a normal range and nothing is graded; interpretation belongs to the
//! calculators (`heartsim-calc`), which cite their sources.
//!
//! # Conventions followed (Mitchell et al. 2019, JASE 32:1–64, the pages read)
//!
//! * LV dimensions and wall thicknesses in the parasternal long axis, perpendicular to the LV long
//!   axis, just below the mitral leaflet tips; **end-diastole is the first frame after mitral
//!   leaflet closure** (the R wave when the valve cannot be seen), the end-systolic dimension the
//!   smallest cavity, usually the frame before the early diastolic opening of the mitral valve;
//!   septal thickness from the interface where the RV cavity meets the compacted septum to where
//!   the septum meets the LV cavity; posterior wall from the LV cavity to the posterior
//!   wall–pericardium interface (p. 31).
//! * The left atrium **at end-systole, leading edge to leading edge**, from the posterior wall of
//!   the aortic sinus to the posterior wall of the left atrium (p. 31); the aortic root at the
//!   sinuses, **leading edge to leading edge at end-diastole** (p. 32). The pack's model has no
//!   chordae and no trabeculae, so nothing has to be avoided there.
//! * TAPSE: the M-mode cursor as parallel as possible to the movement of the lateral tricuspid
//!   annulus; the distance the **leading edge** of the annulus moves from end-diastole toward the
//!   apex at end-systole (p. 33). Tissue Doppler: the sample volume 5 to 10 mm, the angle of
//!   interrogation as parallel as possible to the beam, velocities named s′, e′ and a′, 100 mm/s
//!   (p. 44).
//!
//! The reference of a distance is read from the label line along the learner's own line: a
//! structure's extent is the run of its tissue label that contains the middle of the caliper, so
//! the measure and the reference always describe the same structure. Where the line crosses no
//! such structure, or the sample is hidden behind bone or lung, there is **no reference**, and
//! none is offered.

use crate::bmode::ImageSettings;
use crate::doppler::{Marker, WaveKind};
use crate::history::MotionHistory;
use crate::mmode::{MModeColumn, StepBuffer};
use crate::model::{AnnulusSite, MotionInput, MotionModel};
use crate::probe::ProbeFrame;
use crate::slice::LabelImage;
use glam::Vec3;
use heartsim_geometry::Label;

/// A point of the image plane, mm: `x_mm` along the index marker's side of the sector, `z_mm`
/// along the central beam (the depth direction), from the sector's apex.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Plane2 {
    pub x_mm: f32,
    pub z_mm: f32,
}

impl Plane2 {
    /// The point `(x, z)`.
    #[must_use]
    pub fn new(x_mm: f32, z_mm: f32) -> Self {
        Self { x_mm, z_mm }
    }

    /// Distance to `o`, mm.
    #[must_use]
    pub fn distance(self, o: Self) -> f32 {
        (self.x_mm - o.x_mm).hypot(self.z_mm - o.z_mm)
    }

    /// The same point in the heart frame, for the probe frame `frame`.
    #[must_use]
    pub fn to_world(self, frame: &ProbeFrame) -> Vec3 {
        frame.apex_mm + frame.marker * self.x_mm + frame.beam * self.z_mm
    }

    /// The point of the plane nearest to the heart-frame point `p` and its distance from the
    /// plane, mm.
    #[must_use]
    pub fn of_world(frame: &ProbeFrame, p: Vec3) -> (Self, f32) {
        let d = p - frame.apex_mm;
        (
            Self::new(d.dot(frame.marker), d.dot(frame.beam)),
            d.dot(frame.elevation),
        )
    }
}

/// Area enclosed by a traced outline, cm² (the shoelace formula; the outline closes by itself).
#[must_use]
pub fn polygon_area_cm2(points: &[Plane2]) -> f32 {
    if points.len() < 3 {
        return 0.0;
    }
    let mut twice = 0.0f64;
    for (i, a) in points.iter().enumerate() {
        let b = points[(i + 1) % points.len()];
        twice += f64::from(a.x_mm) * f64::from(b.z_mm) - f64::from(b.x_mm) * f64::from(a.z_mm);
    }
    (twice.abs() * 0.5 / 100.0) as f32
}

/// Slope of a trace between two points, cm/s: `ddepth_mm` of depth over `dt_ms` of time, positive
/// when the depth grows (away from the probe). `None` for no time between the points.
#[must_use]
pub fn slope_cm_s(dt_ms: f64, ddepth_mm: f32) -> Option<f32> {
    (dt_ms.abs() > 1e-9).then(|| (f64::from(ddepth_mm) / 10.0 / (dt_ms / 1000.0)) as f32)
}

/// The label of the sample of `image` nearest to the plane point `p`, and whether it is hidden
/// behind bone or lung; `None` outside the sector, beyond the depth, or without contact.
#[must_use]
pub fn label_at_plane(image: &LabelImage, p: Plane2) -> Option<(Label, bool)> {
    if !image.contact || image.depth_mm <= 0.0 || image.sector_deg <= 0.0 {
        return None;
    }
    let r = p.x_mm.hypot(p.z_mm);
    let angle = p.x_mm.atan2(p.z_mm).to_degrees();
    let f = angle / image.sector_deg + 0.5;
    if !(0.0..1.0).contains(&f) || r >= image.depth_mm || r <= 0.0 {
        return None;
    }
    let line = ((f * image.lines as f32) as usize).min(image.lines - 1);
    let sample = ((r / image.depth_mm * image.samples as f32) as usize).min(image.samples - 1);
    Some((image.label(line, sample), image.is_hidden(line, sample)))
}

/// A sample along a measuring line: its tissue label and whether it is hidden (behind bone or
/// lung, where the label is not to be trusted).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineSample {
    pub label: u8,
    pub hidden: bool,
}

/// The labels of `image` along the straight line from `a` to `b`, one sample every `step_mm`
/// (`None` where the line is outside the sector).
#[must_use]
pub fn sample_segment(image: &LabelImage, a: Plane2, b: Plane2, step_mm: f32) -> Vec<Option<LineSample>> {
    let len = a.distance(b);
    let n = ((len / step_mm.max(1e-3)).ceil() as usize).max(1);
    (0..n)
        .map(|i| {
            let f = (i as f32 + 0.5) / n as f32;
            let p = Plane2::new(a.x_mm + (b.x_mm - a.x_mm) * f, a.z_mm + (b.z_mm - a.z_mm) * f);
            label_at_plane(image, p).map(|(l, hidden)| LineSample {
                label: l as u8,
                hidden,
            })
        })
        .collect()
}

/// The labels of an M-mode column as measuring-line samples (sample `s` at depth `column.depth_of(s)`).
#[must_use]
pub fn column_samples(c: &MModeColumn) -> Vec<Option<LineSample>> {
    (0..c.labels.len())
        .map(|s| {
            Some(LineSample {
                label: c.labels[s],
                hidden: s >= usize::from(c.hidden_from),
            })
        })
        .collect()
}

/// A structure whose extent along a line the model knows from its labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Structure {
    /// The LV cavity: its blood (leaflets and papillary muscles inside it do not end it), from
    /// the septal to the posterior endocardium.
    LvCavity,
    /// The interventricular septum: the run of LV myocardium that has the LV cavity behind it.
    Septum,
    /// The LV posterior wall: the run of LV myocardium that has the LV cavity in front of it.
    PosteriorWall,
    /// The left atrium, leading edge of the wall in front of it to the leading edge of the wall
    /// behind it.
    LeftAtrium,
    /// The aortic root, leading edge of the anterior wall to the leading edge of the posterior wall.
    AorticRoot,
}

fn is(s: Option<LineSample>, f: impl Fn(Label) -> bool) -> bool {
    s.is_some_and(|s| !s.hidden && Label::from_u8(s.label).is_some_and(f))
}

/// The contiguous samples around `mid` that satisfy `f`, as `(first, end)`; `None` when `mid`
/// does not.
fn run_around(
    line: &[Option<LineSample>],
    mid: usize,
    f: impl Fn(Label) -> bool + Copy,
) -> Option<(usize, usize)> {
    if !is(line.get(mid).copied().flatten(), f) {
        return None;
    }
    let mut a = mid;
    while a > 0 && is(line[a - 1], f) {
        a -= 1;
    }
    let mut b = mid + 1;
    while b < line.len() && is(line[b], f) {
        b += 1;
    }
    Some((a, b))
}

fn cavity_like(l: Label) -> bool {
    matches!(l, Label::LvBlood | Label::ValveLeaflet | Label::PapillaryMuscle)
}

/// The extent of `what` along `line` (samples every `step_mm`) around the sample `mid`, mm; `None`
/// where the model has no such structure there (the middle sample is another tissue, or hidden,
/// or the structure's neighbour that defines it is missing).
#[must_use]
pub fn structure_extent(
    line: &[Option<LineSample>],
    step_mm: f32,
    mid: usize,
    what: Structure,
) -> Option<f32> {
    structure_span(line, mid, what).map(|(a, b)| (b - a) as f32 * step_mm)
}

/// [`structure_extent`] as the first sample and the end (exclusive) of the structure along `line`:
/// its boundaries, for the caliper that is to be put on them.
#[must_use]
pub fn structure_span(line: &[Option<LineSample>], mid: usize, what: Structure) -> Option<(usize, usize)> {
    let (a, b) = match what {
        Structure::LvCavity => {
            let (a, b) = run_around(line, mid, cavity_like)?;
            // the cavity ends at its blood: leaflets or muscle at an end belong to the wall's side
            let first = (a..b).find(|&i| is(line[i], |l| l == Label::LvBlood))?;
            let last = (a..b).rev().find(|&i| is(line[i], |l| l == Label::LvBlood))?;
            (first, last + 1)
        }
        Structure::Septum => {
            let (a, b) = run_around(line, mid, |l| l == Label::LvMyocardium)?;
            if !is(line.get(b).copied().flatten(), cavity_like) {
                return None;
            }
            (a, b)
        }
        Structure::PosteriorWall => {
            let (a, b) = run_around(line, mid, |l| l == Label::LvMyocardium)?;
            if a == 0 || !is(line[a - 1], cavity_like) {
                return None;
            }
            (a, b)
        }
        Structure::LeftAtrium => {
            let (a, b) = run_around(line, mid, |l| l == Label::LaBlood)?;
            // leading edge of the wall in front of the blood (the aortic wall at the sinuses)
            let mut a0 = a;
            while a0 > 0 && is(line[a0 - 1], |l| l == Label::AortaWall) {
                a0 -= 1;
            }
            (a0, b)
        }
        Structure::AorticRoot => {
            let (a, b) = run_around(line, mid, |l| l == Label::AortaLumen)?;
            let mut a0 = a;
            while a0 > 0 && is(line[a0 - 1], |l| l == Label::AortaWall) {
                a0 -= 1;
            }
            if !is(line.get(b).copied().flatten(), |l| l == Label::AortaWall) {
                return None;
            }
            (a0, b)
        }
    };
    Some((a, b))
}

/// Every measurement the learner can name, in the order of the short list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Named {
    /// LV internal diameter, end-diastole.
    Lvidd,
    /// LV internal diameter, end-systole.
    Lvids,
    /// Interventricular septal thickness, end-diastole.
    Ivsd,
    /// LV posterior wall thickness, end-diastole.
    Lvpwd,
    /// Left atrial anteroposterior diameter.
    LaDiameter,
    /// Aortic root at the sinuses.
    AorticRoot,
    /// Tricuspid annular plane systolic excursion.
    Tapse,
    /// Mitral annular plane systolic excursion.
    Mapse,
    /// Early diastolic annular velocity.
    EPrime,
    /// Late diastolic (atrial) annular velocity.
    APrime,
    /// Systolic annular velocity.
    SPrime,
}

/// The unit a named measurement is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unit {
    Mm,
    CmS,
}

impl Unit {
    /// The symbol.
    #[must_use]
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Mm => "mm",
            Self::CmS => "cm/s",
        }
    }
}

impl Named {
    /// The short list, in order.
    pub const ALL: [Self; 11] = [
        Self::Lvidd,
        Self::Lvids,
        Self::Ivsd,
        Self::Lvpwd,
        Self::LaDiameter,
        Self::AorticRoot,
        Self::Tapse,
        Self::Mapse,
        Self::EPrime,
        Self::APrime,
        Self::SPrime,
    ];

    /// A stable id (the page's locale keys and the tests use it).
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Lvidd => "lvidd",
            Self::Lvids => "lvids",
            Self::Ivsd => "ivsd",
            Self::Lvpwd => "lvpwd",
            Self::LaDiameter => "la",
            Self::AorticRoot => "ao-root",
            Self::Tapse => "tapse",
            Self::Mapse => "mapse",
            Self::EPrime => "e-prime",
            Self::APrime => "a-prime",
            Self::SPrime => "s-prime",
        }
    }

    /// The unit it is measured in.
    #[must_use]
    pub fn unit(self) -> Unit {
        match self {
            Self::EPrime | Self::APrime | Self::SPrime => Unit::CmS,
            _ => Unit::Mm,
        }
    }

    /// The structure whose extent along the line is its reference (distances of tissue only).
    #[must_use]
    pub fn structure(self) -> Option<Structure> {
        match self {
            Self::Lvidd | Self::Lvids => Some(Structure::LvCavity),
            Self::Ivsd => Some(Structure::Septum),
            Self::Lvpwd => Some(Structure::PosteriorWall),
            Self::LaDiameter => Some(Structure::LeftAtrium),
            Self::AorticRoot => Some(Structure::AorticRoot),
            _ => None,
        }
    }

    /// The annulus sites the excursion is read at, nearest to the cursor first.
    #[must_use]
    pub fn excursion_sites(self) -> &'static [AnnulusSite] {
        match self {
            Self::Tapse => &[AnnulusSite::TricuspidLateral],
            Self::Mapse => &[AnnulusSite::MitralLateral, AnnulusSite::MitralSeptal],
            _ => &[],
        }
    }

    /// The Doppler wave it is the peak of.
    #[must_use]
    pub fn wave(self) -> Option<WaveKind> {
        match self {
            Self::EPrime => Some(WaveKind::E),
            Self::APrime => Some(WaveKind::A),
            Self::SPrime => Some(WaveKind::S),
            _ => None,
        }
    }
}

/// A measurement and what the model holds for it: the difference shown is exactly
/// `measured − reference`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Feedback {
    pub measured: f32,
    /// The model's own value at the same place and instant; `None` where it has none.
    pub reference: Option<f32>,
    /// `measured − reference`.
    pub difference: Option<f32>,
}

/// Compare `measured` with `reference` (no reference, no difference).
#[must_use]
pub fn feedback(measured: f32, reference: Option<f32>) -> Feedback {
    Feedback {
        measured,
        reference,
        difference: reference.map(|r| measured - r),
    }
}

/// The reference of a tissue measurement along the learner's line on a frozen sector: the extent
/// of its structure along the segment `a`–`b` at the middle of the caliper.
#[must_use]
pub fn reference_on_plane(image: &LabelImage, what: Named, a: Plane2, b: Plane2) -> Option<f32> {
    let structure = what.structure()?;
    let step = (image.depth_mm / image.samples as f32).max(0.05);
    // the line from the nearer to the deeper end, so "in front of" and "behind" mean the same
    // here as on the beam
    let (a, b) = if a.z_mm <= b.z_mm { (a, b) } else { (b, a) };
    // the structure's own extent is looked for along the line through the caliper, which is
    // extended to the sector's depth so that a caliper that covers only part of the structure
    // still finds its ends
    let dir = (b.x_mm - a.x_mm, b.z_mm - a.z_mm);
    let len = a.distance(b);
    if len < step {
        return None;
    }
    let u = (dir.0 / len, dir.1 / len);
    let reach = 150.0_f32;
    let from = Plane2::new(a.x_mm - u.0 * reach, a.z_mm - u.1 * reach);
    let to = Plane2::new(b.x_mm + u.0 * reach, b.z_mm + u.1 * reach);
    let total = from.distance(to);
    let line = sample_segment(image, from, to, step);
    let step_mm = total / line.len() as f32;
    let mid_pt = Plane2::new(f32::midpoint(a.x_mm, b.x_mm), f32::midpoint(a.z_mm, b.z_mm));
    let mid = (from.distance(mid_pt) / step_mm) as usize;
    structure_extent(&line, step_mm, mid.min(line.len() - 1), structure)
}

/// The reference of a tissue measurement on an M-mode trace: the extent of its structure along
/// the cursor in the column at the time of the caliper's first end (`t_ms`), around the depth
/// `depth_mm` of its middle.
#[must_use]
pub fn reference_on_mmode(
    buffer: &StepBuffer<MModeColumn>,
    what: Named,
    t_ms: f64,
    depth_mm: f32,
) -> Option<f32> {
    let structure = what.structure()?;
    let step = buffer.step_at(t_ms + buffer.step_ms() * 0.5);
    // where no column was taken (a thinned trace), the one the trace draws there
    let col = buffer.nearest(step, 2)?;
    if !col.contact || col.labels.is_empty() {
        return None;
    }
    let line = column_samples(&col.clone());
    let step_mm = col.depth_mm / col.labels.len() as f32;
    let mid = ((depth_mm / step_mm) as usize).min(line.len() - 1);
    structure_extent(&line, step_mm, mid, structure)
}

/// The model's excursion of an annulus along the cursor between two instants, and how it was
/// found.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Excursion {
    /// The annulus site the cursor passes.
    pub site: AnnulusSite,
    /// `|(p(t₁) − p(t₂)) · beam|`, mm: the distance the annulus moved along the cursor.
    pub along_beam_mm: f32,
    /// The angle between the cursor and the annulus's own motion between the two instants,
    /// degrees (the motion is longitudinal; the cursor is not corrected for it).
    pub angle_deg: f32,
}

/// Distance from the cursor's line within which an annulus counts as crossed by it, mm.
pub const ANNULUS_CURSOR_MM: f32 = 10.0;

/// What the excursion read-out needs.
#[derive(Debug, Clone, Copy)]
pub struct ExcursionContext<'a> {
    pub history: &'a MotionHistory,
    pub frame: ProbeFrame,
    pub cursor_deg: f32,
    pub enabled: bool,
    pub exaggeration: f32,
}

/// The model's excursion of the annulus `what` is measured at, along the cursor, between `t1_ms`
/// and `t2_ms`; `None` when the cursor does not pass the annulus (within
/// [`ANNULUS_CURSOR_MM`], mid-way between the two instants) or `what` is not an excursion.
pub fn reference_excursion(
    model: &mut MotionModel,
    ctx: &ExcursionContext<'_>,
    what: Named,
    t1_ms: f64,
    t2_ms: f64,
) -> Option<Excursion> {
    let cursor = crate::mmode::cursor_frame(&ctx.frame, ctx.cursor_deg);
    let at = |model: &mut MotionModel, site: AnnulusSite, t_ms: f64| {
        model.annulus_position(
            &MotionInput {
                history: ctx.history,
                t_ms,
                resp_phase: ctx.history.respiratory_phase(t_ms),
                enabled: ctx.enabled,
                exaggeration: ctx.exaggeration,
            },
            site,
        )
    };
    let mut best: Option<(f32, Excursion)> = None;
    for &site in what.excursion_sites() {
        let (Some(p1), Some(p2)) = (at(model, site, t1_ms), at(model, site, t2_ms)) else {
            continue;
        };
        let mid = (p1 + p2) * 0.5;
        let off = mid - cursor.apex_mm;
        // distance from the cursor's ray, in the plane and out of it
        let along = off.dot(cursor.beam);
        let miss = (off - cursor.beam * along).length();
        if miss > ANNULUS_CURSOR_MM || along < 0.0 {
            continue;
        }
        let motion = p1 - p2;
        let angle = if motion.length() > 1e-3 {
            (motion.dot(cursor.beam).abs() / motion.length())
                .clamp(0.0, 1.0)
                .acos()
                .to_degrees()
        } else {
            0.0
        };
        let ex = Excursion {
            site,
            along_beam_mm: motion.dot(cursor.beam).abs(),
            angle_deg: angle,
        };
        if best.as_ref().is_none_or(|b| miss < b.0) {
            best = Some((miss, ex));
        }
    }
    best.map(|b| b.1)
}

/// The reference of a velocity measurement: the model's peak of the wave `what` is the peak of,
/// magnitude, from the markers of the beat the caliper is in (`None` where that beat has none:
/// no a′ without an atrial contraction).
#[must_use]
pub fn reference_velocity(what: Named, markers: &[Marker]) -> Option<f32> {
    let kind = what.wave()?;
    markers.iter().find(|m| m.kind == kind).map(|m| m.cm_s.abs())
}

/// The image's mm per sample along a line (for the callers that need the step of
/// [`structure_extent`]).
#[must_use]
pub fn sample_step_mm(settings: &ImageSettings) -> f32 {
    let s = settings.clamped();
    s.depth_mm / s.samples as f32
}

// ---- where the model's own diameters come from, for the tests and the page's self-check ------

/// The model's LV cavity diameter along the cursor in the column at `t_ms`, mm.
#[must_use]
pub fn cavity_diameter_of_column(c: &MModeColumn, near_depth_mm: f32) -> Option<f32> {
    let line = column_samples(c);
    let step = c.depth_mm / c.labels.len() as f32;
    structure_extent(
        &line,
        step,
        ((near_depth_mm / step) as usize).min(line.len() - 1),
        Structure::LvCavity,
    )
}

/// The cursor angle that points at the heart-frame point `p` (projected into the image plane),
/// degrees from the sector's central beam, positive toward the index marker.
#[must_use]
pub fn cursor_toward(frame: &ProbeFrame, p: Vec3) -> f32 {
    let (q, _) = Plane2::of_world(frame, p);
    q.x_mm.atan2(q.z_mm).to_degrees()
}

/// The scale of a trace on the screen: how far the learner's calipers on the screen are in time and
/// in depth. `width_mm` of screen is `speed` seconds' worth of time; `height_mm` of screen is
/// `depth_mm` of tissue.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TraceScale {
    pub speed: crate::mmode::SweepSpeed,
    /// Depth the trace's height covers, mm.
    pub depth_mm: f32,
    /// The height of the trace on the screen, mm.
    pub height_mm: f32,
}

impl TraceScale {
    /// The time between two points `dx_mm` apart on the screen, ms.
    #[must_use]
    pub fn time_ms(&self, dx_mm: f32) -> f64 {
        self.speed.time_ms(dx_mm)
    }

    /// The depth between two points `dy_mm` apart on the screen, mm.
    #[must_use]
    pub fn depth_mm(&self, dy_mm: f32) -> f32 {
        dy_mm / self.height_mm.max(1e-6) * self.depth_mm
    }

    /// The slope between two points `dx_mm` and `dy_mm` apart on the screen, cm/s (depth gained
    /// per second; positive away from the probe).
    #[must_use]
    pub fn slope_cm_s(&self, dx_mm: f32, dy_mm: f32) -> Option<f32> {
        slope_cm_s(self.time_ms(dx_mm), self.depth_mm(dy_mm))
    }
}
