//! Where and how the learner listens (brief B27, part B): the five classic auscultation sites, the
//! bell and the diaphragm, and the valve highlights of the 3-D heart.
//!
//! * A **site** scales each discrete sound ([`ComponentKind`]) and each murmur class
//!   ([`MurmurClass`]) by a gain from `assets/auscultation_sites.ron` ([`SiteTable`]), applied when
//!   sampling through [`SoundGains`]: nothing is re-scheduled, so a site change is heard from the
//!   next sample and the trace stays random-access.
//! * A **chest piece** weights every murmur partial by its frequency and every discrete sound by its
//!   instantaneous frequency ([`ChestWeight`]): a closed-form per-partial weight, not a time-domain
//!   filter, so the synthesiser keeps no filter state.
//! * The **overview** ([`ListeningSite::Overview`], the default) is every gain 1 and no chest piece:
//!   the signal of brief B24 and part A bit for bit, whatever chest piece is selected.
//!
//! Every gain and both chest-piece curves are **designed**: they put into numbers the qualitative
//! statements listed at the top of the RON file (S1 loudest at the apex, A2 dominant at the aortic
//! area, each murmur loudest over its valve, aortic regurgitation at Erb's point), nothing more.
//!
//! [`valve_glow_at`] turns the schedule the sound is made from into per-valve highlight levels, so
//! the ring that flashes on the 3-D heart and the sound heard can never disagree.

use crate::auscultation::{Murmur, MurmurClass, SoundGains};
use crate::pcg::{ComponentKind, ScheduledSound};
use crate::placement::{INTERCOSTAL_SPACES, VERTICAL_LINES};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

/// Format version of `assets/auscultation_sites.ron`.
pub const SITE_TABLE_FORMAT_VERSION: u32 = 1;

/// The built-in site table, compiled in (desktop and web alike) so the synthesiser needs no I/O.
const BUILTIN_SITES: &str = include_str!("../../../assets/auscultation_sites.ron");

/// Largest gain a site may give a sound. Designed: enough to make a sound dominant at its site,
/// small enough that a site never drives the B25 limiter far past its knee.
pub const MAX_SITE_GAIN: f64 = 2.0;

// --- sites ---------------------------------------------------------------------------------------

/// Where the stethoscope is. `Overview` is not an anatomical site: it is the unweighted mix the
/// trace has always shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub enum ListeningSite {
    /// Every sound at its modelled level, no chest piece (brief B24 and part A, unchanged).
    #[default]
    Overview,
    /// Aortic area: 2nd intercostal space, right sternal border.
    Aortic,
    /// Pulmonic area: 2nd intercostal space, left sternal border.
    Pulmonic,
    /// Erb's point: 3rd intercostal space, left sternal border.
    Erb,
    /// Tricuspid area: lower left sternal border (4th intercostal space).
    Tricuspid,
    /// Mitral area: the apex, 5th intercostal space, left mid-clavicular line.
    Mitral,
}

impl ListeningSite {
    /// Every choice, in menu order (overview first, then down the chest as auscultated).
    pub const ALL: [ListeningSite; 6] = [
        ListeningSite::Overview,
        ListeningSite::Aortic,
        ListeningSite::Pulmonic,
        ListeningSite::Erb,
        ListeningSite::Tricuspid,
        ListeningSite::Mitral,
    ];

    /// The five anatomical sites (every choice but the overview).
    pub const SITES: [ListeningSite; 5] = [
        ListeningSite::Aortic,
        ListeningSite::Pulmonic,
        ListeningSite::Erb,
        ListeningSite::Tricuspid,
        ListeningSite::Mitral,
    ];

    /// The chest-wall landmark of the site: an intercostal space and a vertical line, named as in
    /// [`INTERCOSTAL_SPACES`] and [`VERTICAL_LINES`] (the electrode editor's landmarks). `None` for
    /// the overview.
    #[must_use]
    pub fn landmark(self) -> Option<(&'static str, &'static str)> {
        Some(match self {
            ListeningSite::Overview => return None,
            ListeningSite::Aortic => ("2nd intercostal space", "right sternal border"),
            ListeningSite::Pulmonic => ("2nd intercostal space", "left sternal border"),
            ListeningSite::Erb => ("3rd intercostal space", "left sternal border"),
            ListeningSite::Tricuspid => ("4th intercostal space", "left sternal border"),
            ListeningSite::Mitral => ("5th intercostal space", "left mid-clavicular line"),
        })
    }

    /// The landmark as `(height in the heart frame, mm; azimuth around the torso axis, degrees,
    /// positive towards the patient's left)`, looked up in the electrode editor's tables.
    #[must_use]
    pub fn landmark_mm(self) -> Option<(f32, f32)> {
        let (space, line) = self.landmark()?;
        let y = INTERCOSTAL_SPACES.iter().find(|(n, _)| *n == space)?.1;
        let az = VERTICAL_LINES.iter().find(|(n, _)| *n == line)?.1;
        Some((y, az))
    }

    /// Stable identifier (settings, the RON table, message keys): `overview`, `aortic`, ….
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            ListeningSite::Overview => "overview",
            ListeningSite::Aortic => "aortic",
            ListeningSite::Pulmonic => "pulmonic",
            ListeningSite::Erb => "erb",
            ListeningSite::Tricuspid => "tricuspid",
            ListeningSite::Mitral => "mitral",
        }
    }
}

/// The site where a murmur class is loudest (the site table must agree; the tests check it).
/// From the qualitative statements at the top of `assets/auscultation_sites.ron`.
#[must_use]
pub fn loudest_site(class: MurmurClass) -> ListeningSite {
    match class {
        MurmurClass::AorticStenosis => ListeningSite::Aortic,
        MurmurClass::PulmonaryStenosis | MurmurClass::AtrialSeptalFlow => ListeningSite::Pulmonic,
        MurmurClass::MitralRegurgitation
        | MurmurClass::ProlapseLateSystolic
        | MurmurClass::MitralRumble
        | MurmurClass::MitralPresystolic => ListeningSite::Mitral,
        MurmurClass::TricuspidRegurgitation
        | MurmurClass::VentricularSeptalDefect
        | MurmurClass::InnocentMurmur => ListeningSite::Tricuspid,
        MurmurClass::AorticRegurgitation | MurmurClass::FrictionRub => ListeningSite::Erb,
    }
}

// --- chest pieces --------------------------------------------------------------------------------

/// Bell or diaphragm. Ignored at the overview.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum ChestPiece {
    /// The diaphragm, pressed firmly: high sounds (S2, clicks, regurgitant murmurs) stand out.
    #[default]
    Diaphragm,
    /// The bell, held lightly: low sounds (S3, S4, the mitral rumble) stand out.
    Bell,
}

impl ChestPiece {
    /// Both, in menu order.
    pub const ALL: [ChestPiece; 2] = [ChestPiece::Diaphragm, ChestPiece::Bell];

    /// Stable identifier: `diaphragm`, `bell`.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            ChestPiece::Diaphragm => "diaphragm",
            ChestPiece::Bell => "bell",
        }
    }
}

/// A chest piece's weight curve: a corner frequency and a gain. Designed (see the RON file).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ChestCurve {
    pub corner_hz: f64,
    pub gain: f64,
}

/// A chest piece as the synthesisers apply it: a weight by frequency.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ChestWeight {
    pub piece: ChestPiece,
    pub curve: ChestCurve,
}

impl ChestWeight {
    /// The weight at `hz`, with `x = hz / corner`: for the bell `gain / (1 + x²)` (low emphasis,
    /// non-increasing in frequency), for the diaphragm `gain · x² / (1 + x²)` (high emphasis,
    /// non-decreasing). The magnitudes of a second-order low- and high-pass, applied as weights:
    /// designed. Finite and non-negative for any `hz`; arithmetic only, so the same on every
    /// platform.
    #[must_use]
    pub fn weight(&self, hz: f64) -> f64 {
        let x = hz.max(0.0) / self.curve.corner_hz.max(1e-3);
        let x2 = x * x;
        if !x2.is_finite() {
            return match self.piece {
                ChestPiece::Bell => 0.0,
                ChestPiece::Diaphragm => self.curve.gain,
            };
        }
        let inv = 1.0 / (1.0 + x2);
        match self.piece {
            ChestPiece::Bell => self.curve.gain * inv,
            ChestPiece::Diaphragm => self.curve.gain * x2 * inv,
        }
    }
}

/// What the learner chose: a site and a chest piece (the chest piece is ignored at the overview).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct ListeningChoice {
    pub site: ListeningSite,
    pub chest: ChestPiece,
}

impl ListeningChoice {
    /// The overview: today's signal.
    #[must_use]
    pub fn is_overview(self) -> bool {
        self.site == ListeningSite::Overview
    }
}

/// What a synthesiser applies when sampling: per-class gains and an optional chest piece. The
/// default (all gains 1, no chest piece) leaves the output bit-identical.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Listening {
    pub gains: SoundGains,
    pub chest: Option<ChestWeight>,
}

impl Listening {
    /// The overview mix.
    #[must_use]
    pub fn overview() -> Self {
        Self::default()
    }
}

// --- the table -----------------------------------------------------------------------------------

/// The gains of one site, by name. Every key must be present (see [`SiteTable::validate`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SiteGains {
    pub components: BTreeMap<ComponentKind, f64>,
    pub murmurs: BTreeMap<MurmurClass, f64>,
}

/// `assets/auscultation_sites.ron`: the gains of the five sites and the two chest-piece curves.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SiteTable {
    pub format_version: u32,
    pub sites: BTreeMap<ListeningSite, SiteGains>,
    pub bell: ChestCurve,
    pub diaphragm: ChestCurve,
}

/// Why a site table was refused.
#[derive(Debug, Clone, PartialEq)]
pub enum SiteTableError {
    Parse(String),
    FormatVersion(u32),
    /// A site, a component or a murmur class without a gain (or an overview entry, which is fixed).
    Missing(String),
    /// A gain or a curve value outside its range.
    OutOfRange(String),
}

impl std::fmt::Display for SiteTableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SiteTableError::Parse(e) => write!(f, "site table does not parse: {e}"),
            SiteTableError::FormatVersion(v) => write!(
                f,
                "site table format_version {v}, expected {SITE_TABLE_FORMAT_VERSION}"
            ),
            SiteTableError::Missing(what) => write!(f, "site table: {what}"),
            SiteTableError::OutOfRange(what) => write!(f, "site table: {what} out of range"),
        }
    }
}

impl std::error::Error for SiteTableError {}

impl SiteTable {
    /// Parse and validate a table.
    ///
    /// # Errors
    /// When the text does not parse or the table fails [`SiteTable::validate`].
    pub fn from_ron(text: &str) -> Result<Self, SiteTableError> {
        let t: Self = ron::from_str(text).map_err(|e| SiteTableError::Parse(e.to_string()))?;
        t.validate()?;
        Ok(t)
    }

    /// The table compiled into the build.
    ///
    /// # Panics
    /// If the shipped file is invalid (a test guards it).
    #[must_use]
    pub fn builtin() -> Arc<Self> {
        static TABLE: OnceLock<Arc<SiteTable>> = OnceLock::new();
        Arc::clone(TABLE.get_or_init(|| {
            Arc::new(Self::from_ron(BUILTIN_SITES).expect("assets/auscultation_sites.ron is valid"))
        }))
    }

    /// The format version is known, every anatomical site lists a gain for every component and
    /// murmur class, there is no overview entry, every gain is in `0..=MAX_SITE_GAIN` and both
    /// curves have a positive corner and gain.
    ///
    /// # Errors
    /// The first problem found.
    pub fn validate(&self) -> Result<(), SiteTableError> {
        if self.format_version != SITE_TABLE_FORMAT_VERSION {
            return Err(SiteTableError::FormatVersion(self.format_version));
        }
        if self.sites.contains_key(&ListeningSite::Overview) {
            return Err(SiteTableError::Missing(
                "the overview is fixed (all gains 1) and must not be listed".into(),
            ));
        }
        let ok = |g: f64| g.is_finite() && (0.0..=MAX_SITE_GAIN).contains(&g);
        for site in ListeningSite::SITES {
            let s = self
                .sites
                .get(&site)
                .ok_or_else(|| SiteTableError::Missing(format!("no entry for {site:?}")))?;
            for k in ComponentKind::ALL {
                let g = *s
                    .components
                    .get(&k)
                    .ok_or_else(|| SiteTableError::Missing(format!("{site:?} has no gain for {k:?}")))?;
                if !ok(g) {
                    return Err(SiteTableError::OutOfRange(format!("{site:?} {k:?} = {g}")));
                }
            }
            for c in MurmurClass::ALL {
                let g = *s
                    .murmurs
                    .get(&c)
                    .ok_or_else(|| SiteTableError::Missing(format!("{site:?} has no gain for {c:?}")))?;
                if !ok(g) {
                    return Err(SiteTableError::OutOfRange(format!("{site:?} {c:?} = {g}")));
                }
            }
        }
        for (name, c) in [("bell", self.bell), ("diaphragm", self.diaphragm)] {
            let pos = |v: f64| v.is_finite() && v > 0.0;
            if !pos(c.corner_hz) || !pos(c.gain) || c.gain > MAX_SITE_GAIN {
                return Err(SiteTableError::OutOfRange(format!("{name} curve {c:?}")));
            }
        }
        Ok(())
    }

    /// The gains of `site` as the synthesisers take them (all 1 at the overview).
    #[must_use]
    pub fn gains(&self, site: ListeningSite) -> SoundGains {
        let mut g = SoundGains::default();
        if let Some(s) = self.sites.get(&site) {
            for (i, k) in ComponentKind::ALL.iter().enumerate() {
                g.components[i] = s.components.get(k).copied().unwrap_or(1.0);
            }
            for c in MurmurClass::ALL {
                g.murmurs[c.index()] = s.murmurs.get(&c).copied().unwrap_or(1.0);
            }
        }
        g
    }

    /// The weight curve of `piece`.
    #[must_use]
    pub fn chest(&self, piece: ChestPiece) -> ChestWeight {
        ChestWeight {
            piece,
            curve: match piece {
                ChestPiece::Bell => self.bell,
                ChestPiece::Diaphragm => self.diaphragm,
            },
        }
    }

    /// What the synthesisers apply for `choice`: [`Listening::overview`] at the overview (whatever
    /// the chest piece), else the site's gains and the chest piece's curve.
    #[must_use]
    pub fn listening(&self, choice: ListeningChoice) -> Listening {
        if choice.is_overview() {
            return Listening::overview();
        }
        Listening {
            gains: self.gains(choice.site),
            chest: Some(self.chest(choice.chest)),
        }
    }
}

// --- valve highlights ----------------------------------------------------------------------------

/// A cardiac valve, as the 3-D heart draws its ring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Valve {
    Mitral,
    Tricuspid,
    Aortic,
    Pulmonic,
}

impl Valve {
    /// Every valve, in the order of [`valve_glow_at`]'s result.
    pub const ALL: [Valve; 4] = [Valve::Mitral, Valve::Tricuspid, Valve::Aortic, Valve::Pulmonic];

    /// Index in [`Self::ALL`].
    #[must_use]
    pub fn index(self) -> usize {
        self as usize
    }

    /// The valve whose closure a discrete sound is: M1 mitral, T1 tricuspid, A2 aortic, P2
    /// pulmonic. The extra sounds (S3, S4, clicks, snap) do not flash a ring.
    #[must_use]
    pub fn of_component(kind: ComponentKind) -> Option<Valve> {
        match kind {
            ComponentKind::M1 => Some(Valve::Mitral),
            ComponentKind::T1 => Some(Valve::Tricuspid),
            ComponentKind::A2 => Some(Valve::Aortic),
            ComponentKind::P2 => Some(Valve::Pulmonic),
            _ => None,
        }
    }

    /// The stenotic or regurgitant valve a murmur comes from. `None` for the murmurs that are not a
    /// valve lesion: the septal defects, the innocent murmur, the rub.
    #[must_use]
    pub fn of_murmur(class: MurmurClass) -> Option<Valve> {
        match class {
            MurmurClass::AorticStenosis | MurmurClass::AorticRegurgitation => Some(Valve::Aortic),
            MurmurClass::PulmonaryStenosis => Some(Valve::Pulmonic),
            MurmurClass::MitralRegurgitation
            | MurmurClass::ProlapseLateSystolic
            | MurmurClass::MitralRumble
            | MurmurClass::MitralPresystolic => Some(Valve::Mitral),
            MurmurClass::TricuspidRegurgitation => Some(Valve::Tricuspid),
            MurmurClass::VentricularSeptalDefect
            | MurmurClass::AtrialSeptalFlow
            | MurmurClass::InnocentMurmur
            | MurmurClass::FrictionRub => None,
        }
    }
}

/// How long a closure flash lasts, simulation ms: full at the sound's onset, fading linearly to
/// nothing. Designed: longer than the atoms (60–95 ms) so a 60 Hz display shows it for several
/// frames, shorter than the A2–M1 gap at 180 bpm.
pub const VALVE_FLASH_MS: f64 = 150.0;
/// Level of the steady glow of a valve while its murmur sounds (a flash is 1). Designed.
pub const MURMUR_GLOW_LEVEL: f64 = 0.6;

/// One highlight of a valve: `start_ms..end_ms` in simulation time; a flash fades from 1, a murmur
/// glow stays at [`MURMUR_GLOW_LEVEL`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ValveHighlight {
    pub valve: Valve,
    pub start_ms: f64,
    pub end_ms: f64,
    /// `true` for a closure flash, `false` for a murmur's glow.
    pub flash: bool,
}

impl ValveHighlight {
    /// Level at simulation time `t_ms`, `0..=1`.
    #[must_use]
    pub fn level_at(&self, t_ms: f64) -> f64 {
        if t_ms < self.start_ms || t_ms >= self.end_ms {
            return 0.0;
        }
        if self.flash {
            1.0 - (t_ms - self.start_ms) / (self.end_ms - self.start_ms).max(1e-6)
        } else {
            MURMUR_GLOW_LEVEL
        }
    }
}

/// The highlights of a schedule: a flash at every valve closure ([`Valve::of_component`]) and a
/// glow over each valve murmur's sounding time ([`Valve::of_murmur`], until its end or its cut
/// plus fade). Sounds and murmurs are those of [`crate::PcgSynth::scheduled_sounds`] and
/// [`crate::PcgSynth::scheduled_murmurs`]: the schedule the sound is synthesised from.
#[must_use]
pub fn valve_highlights(sounds: &[ScheduledSound], murmurs: &[Murmur]) -> Vec<ValveHighlight> {
    let flashes = sounds.iter().filter_map(|s| {
        Valve::of_component(s.kind).map(|valve| ValveHighlight {
            valve,
            start_ms: s.onset_ms,
            end_ms: s.onset_ms + VALVE_FLASH_MS,
            flash: true,
        })
    });
    let glows = murmurs.iter().filter_map(|m| {
        Valve::of_murmur(m.class).map(|valve| ValveHighlight {
            valve,
            start_ms: m.start_ms,
            end_ms: m.stop_ms(),
            flash: false,
        })
    });
    flashes.chain(glows).collect()
}

/// Highlight level of every valve ([`Valve::ALL`] order) at simulation time `t_ms`: the brightest
/// of its flashes and glows.
#[must_use]
pub fn valve_glow_at(sounds: &[ScheduledSound], murmurs: &[Murmur], t_ms: f64) -> [f32; 4] {
    let mut out = [0.0f32; 4];
    for h in valve_highlights(sounds, murmurs) {
        let v = h.level_at(t_ms) as f32;
        let slot = &mut out[h.valve.index()];
        *slot = slot.max(v);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_builtin_table_is_valid_and_every_site_has_a_landmark() {
        let t = SiteTable::builtin();
        assert_eq!(t.format_version, SITE_TABLE_FORMAT_VERSION);
        for site in ListeningSite::SITES {
            assert!(site.landmark_mm().is_some(), "{site:?}");
        }
        assert_eq!(ListeningSite::Overview.landmark_mm(), None);
    }

    #[test]
    fn a_table_missing_a_key_is_refused() {
        let text = BUILTIN_SITES.replace("FrictionRub: 0.8,", "");
        assert!(matches!(
            SiteTable::from_ron(&text),
            Err(SiteTableError::Missing(_))
        ));
        let text = BUILTIN_SITES.replace("format_version: 1", "format_version: 2");
        assert!(matches!(
            SiteTable::from_ron(&text),
            Err(SiteTableError::FormatVersion(2))
        ));
    }

    #[test]
    fn the_curves_have_their_emphasis() {
        let t = SiteTable::builtin();
        let bell = t.chest(ChestPiece::Bell);
        let dia = t.chest(ChestPiece::Diaphragm);
        let mut prev = (f64::INFINITY, 0.0);
        for hz in [0.0, 10.0, 30.0, 60.0, 120.0, 250.0, 500.0, 2000.0] {
            let (b, d) = (bell.weight(hz), dia.weight(hz));
            assert!(b <= prev.0 && d >= prev.1, "{hz} Hz");
            prev = (b, d);
        }
        assert!(bell.weight(30.0) / bell.weight(250.0) > dia.weight(30.0) / dia.weight(250.0));
    }

    #[test]
    fn a_closure_flash_fades_and_a_murmur_glows_until_its_stop() {
        let h = ValveHighlight {
            valve: Valve::Aortic,
            start_ms: 100.0,
            end_ms: 100.0 + VALVE_FLASH_MS,
            flash: true,
        };
        assert_eq!(h.level_at(99.0), 0.0);
        assert!((h.level_at(100.0) - 1.0).abs() < 1e-12);
        assert!(h.level_at(150.0) < h.level_at(120.0));
        assert_eq!(h.level_at(100.0 + VALVE_FLASH_MS), 0.0);
    }
}
