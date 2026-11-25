//! The acoustic table of the grey ultrasound image (brief B32, item 1): one row per tissue
//! [`Label`] — impedance, attenuation, scatterer density, mean and spread, backscatter level and
//! specularity — and the few global numbers of the line model, all in
//! `assets/echo/acoustic.ron` (`format_version`).
//!
//! **Every value carries its source** as a [`Sourced`] triple `(value, status, source)`:
//! [`Status::Read`] for a number read on the source the research note cites
//! (`docs/research/echo-simulator.md` §3.2), [`Status::Unverified`] where the note marks it so,
//! [`Status::Designed`] for a value chosen here. Nothing comes from the IT'IS database.

use heartsim_geometry::Label;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

/// Format version of `assets/echo/acoustic.ron`.
pub const ACOUSTIC_FORMAT_VERSION: u32 = 1;

const BUILTIN_TABLE: &str = include_str!("../../../assets/echo/acoustic.ron");

/// Where a number comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    /// Read on the cited source (the research note's §3.2 says how).
    Read,
    /// The research note marks it unverified (snippet, abstract or textbook value not read).
    Unverified,
    /// Chosen for this model; no source.
    Designed,
}

/// A number with its status and a free-text source (`"RAD225 notes (Szabo, Duck): muscle"`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sourced(pub f32, pub Status, pub String);

impl Sourced {
    /// The value.
    #[must_use]
    pub fn v(&self) -> f32 {
        self.0
    }
}

/// The acoustic properties of one tissue label.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AcousticRow {
    /// Characteristic acoustic impedance, MRayl (> 0). The reflection at a label change is
    /// Shams 2008's `((Z₂ − Z₁)/(Z₂ + Z₁))²`.
    pub impedance_mrayl: Sourced,
    /// Attenuation coefficient, dB/cm/MHz (≥ 0), applied two-way: `2·α·f·depth`.
    pub attenuation_db_cm_mhz: Sourced,
    /// Backscatter level of the tissue's speckle, dB relative to a perfect plane reflector at the
    /// same depth (finite).
    pub backscatter_db: Sourced,
    /// Fraction of the scatterer lattice's nodes that scatter, in `(0, 1]` (Mattausch 2016's
    /// threshold ρ; 1 = fully developed speckle).
    pub scatter_density: Sourced,
    /// Mean µ of a node's scatter amplitude (≥ 0): a node scatters `µ + σ·ξ` (Mattausch 2016),
    /// with ξ the magnitude of a circular complex Gaussian (its phase is the node's). µ > 0 adds
    /// a coherent part (brighter, less speckled tissue).
    pub scatter_mean: Sourced,
    /// Spread σ of a node's scatter amplitude (≥ 0); `µ = 0, σ = 1, density 1` gives a complex
    /// Gaussian field and a Rayleigh envelope.
    pub scatter_spread: Sourced,
    /// Specularity n of an interface into or out of this tissue (≥ 0): the interface echo is
    /// weighted by `max(cos 2θ, 0)ⁿ`, θ between the beam and the interface normal (Mattausch
    /// 2016's reflected-direction term); the larger n of the two tissues applies.
    pub specularity: Sourced,
}

/// `assets/echo/acoustic.ron`: one [`AcousticRow`] per label (by [`Label::name`]) and the line
/// model's global numbers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AcousticTable {
    pub format_version: u32,
    /// Speed of sound the image assumes, m/s (λ = c / f).
    pub speed_of_sound_m_s: Sourced,
    /// Attenuation the default time-gain compensation cancels, dB/cm/MHz (≥ 0).
    pub tgc_reference_db_cm_mhz: Sourced,
    /// Echo level (after compensation, dB re a perfect reflector) shown white at 0 dB gain.
    pub display_top_db: Sourced,
    /// Spacing of the finest scatterer lattice, mm (> 0).
    pub speckle_lattice_mm: Sourced,
    /// The lattice read at a sample is at least this many times the sample spacing (≥ 1), so
    /// the sampling never aliases it and the speckle rides with the tissue.
    pub speckle_oversample: Sourced,
    /// Cycles of the transmit pulse (> 0); the axial resolution (FWHM) is half the spatial pulse
    /// length, `cycles · λ / 2`.
    pub pulse_cycles: Sourced,
    /// Active aperture of the transducer, mm (> 0); lateral FWHM `λ · focus / aperture` up to
    /// the focus.
    pub aperture_mm: Sourced,
    /// Beyond the focus the lateral FWHM grows by this many times the diffraction rate (≥ 0):
    /// `λ / aperture · (focus + k · (r − focus))`.
    pub beyond_focus_widening: Sourced,
    /// Amplitude reflected back by the transducer face for each lung reverberation (in `[0, 1)`).
    pub reverberation_face_reflection: Sourced,
    /// Number of repeats of the pleural line drawn after it (≥ 0, at most 16).
    pub reverberation_repeats: Sourced,
    pub rows: BTreeMap<String, AcousticRow>,
}

/// Why an acoustic table was refused.
#[derive(Debug, Clone, PartialEq)]
pub enum AcousticError {
    Parse(String),
    FormatVersion(u32),
    /// A label without a row, or a row for no label.
    Label(String),
    /// A value outside its range (or not finite), or a source left empty.
    Value(String),
}

impl std::fmt::Display for AcousticError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "acoustic table does not parse: {e}"),
            Self::FormatVersion(v) => {
                write!(
                    f,
                    "acoustic table format_version {v}, expected {ACOUSTIC_FORMAT_VERSION}"
                )
            }
            Self::Label(e) | Self::Value(e) => write!(f, "acoustic table: {e}"),
        }
    }
}

impl std::error::Error for AcousticError {}

impl AcousticTable {
    /// Parse and validate a table.
    ///
    /// # Errors
    /// When the text does not parse or the table fails [`Self::validate`].
    pub fn from_ron(text: &str) -> Result<Self, AcousticError> {
        let t: Self = ron::from_str(text).map_err(|e| AcousticError::Parse(e.to_string()))?;
        t.validate()?;
        Ok(t)
    }

    /// The table as RON (round-trips through [`Self::from_ron`]).
    ///
    /// # Panics
    /// Never for a table of this type (every field serialises).
    #[must_use]
    pub fn to_ron(&self) -> String {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()).expect("serialisable")
    }

    /// The table compiled into the build.
    ///
    /// # Panics
    /// If the shipped file is invalid (a test guards it).
    #[must_use]
    pub fn builtin() -> Arc<Self> {
        static TABLE: OnceLock<Arc<AcousticTable>> = OnceLock::new();
        Arc::clone(TABLE.get_or_init(|| {
            Arc::new(Self::from_ron(BUILTIN_TABLE).expect("assets/echo/acoustic.ron is valid"))
        }))
    }

    /// The row of `label`.
    ///
    /// # Panics
    /// If the table lacks the label (a validated table never does).
    #[must_use]
    pub fn row(&self, label: Label) -> &AcousticRow {
        &self.rows[label.name()]
    }

    /// The format version is known, every [`Label`] has exactly one row and no row names another
    /// key, every value is finite and in range, and every source is non-empty.
    ///
    /// # Errors
    /// The first problem found.
    pub fn validate(&self) -> Result<(), AcousticError> {
        if self.format_version != ACOUSTIC_FORMAT_VERSION {
            return Err(AcousticError::FormatVersion(self.format_version));
        }
        for key in self.rows.keys() {
            if !Label::ALL.iter().any(|l| l.name() == key) {
                return Err(AcousticError::Label(format!("row {key:?} names no label")));
            }
        }
        let check = |what: &str, s: &Sourced, ok: &dyn Fn(f32) -> bool| {
            if s.2.trim().is_empty() {
                return Err(AcousticError::Value(format!("{what} has no source")));
            }
            if s.0.is_finite() && ok(s.0) {
                Ok(())
            } else {
                Err(AcousticError::Value(format!("{what} = {} out of range", s.0)))
            }
        };
        let pos = |v: f32| v > 0.0;
        let nonneg = |v: f32| v >= 0.0;
        check("speed_of_sound_m_s", &self.speed_of_sound_m_s, &|v| {
            (500.0..=5000.0).contains(&v)
        })?;
        check("tgc_reference_db_cm_mhz", &self.tgc_reference_db_cm_mhz, &nonneg)?;
        check("display_top_db", &self.display_top_db, &|v| {
            (-200.0..=50.0).contains(&v)
        })?;
        check("speckle_lattice_mm", &self.speckle_lattice_mm, &pos)?;
        check("speckle_oversample", &self.speckle_oversample, &|v| {
            (1.0..=8.0).contains(&v)
        })?;
        check("pulse_cycles", &self.pulse_cycles, &pos)?;
        check("aperture_mm", &self.aperture_mm, &pos)?;
        check("beyond_focus_widening", &self.beyond_focus_widening, &nonneg)?;
        check(
            "reverberation_face_reflection",
            &self.reverberation_face_reflection,
            &|v| (0.0..1.0).contains(&v),
        )?;
        check("reverberation_repeats", &self.reverberation_repeats, &|v| {
            (0.0..=16.0).contains(&v)
        })?;
        for l in Label::ALL {
            let r = self
                .rows
                .get(l.name())
                .ok_or_else(|| AcousticError::Label(format!("no row for {}", l.name())))?;
            let n = l.name();
            check(&format!("{n}.impedance_mrayl"), &r.impedance_mrayl, &pos)?;
            check(
                &format!("{n}.attenuation_db_cm_mhz"),
                &r.attenuation_db_cm_mhz,
                &nonneg,
            )?;
            check(&format!("{n}.backscatter_db"), &r.backscatter_db, &|v| {
                (-200.0..=50.0).contains(&v)
            })?;
            check(&format!("{n}.scatter_density"), &r.scatter_density, &|v| {
                v > 0.0 && v <= 1.0
            })?;
            check(&format!("{n}.scatter_mean"), &r.scatter_mean, &nonneg)?;
            check(&format!("{n}.scatter_spread"), &r.scatter_spread, &nonneg)?;
            check(&format!("{n}.specularity"), &r.specularity, &|v| {
                (0.0..=64.0).contains(&v)
            })?;
        }
        Ok(())
    }
}
