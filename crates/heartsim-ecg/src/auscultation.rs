//! Gallops, clicks, the opening snap, the pericardial rub and the classic murmurs (brief B27, part A),
//! scheduled from the same beats as S1 and S2 ([`crate::pcg`]).
//!
//! A scenario's `auscultation` section ([`heartsim_core::Auscultation`]) says what is wrong; this
//! module turns it, beat by beat, into
//!
//! * changes to the valve closures of [`crate::pcg::schedule`] (a loud S1 in mitral stenosis, a soft
//!   and late A2 in severe aortic stenosis, a soft and late P2 in pulmonary stenosis, the fixed split
//!   of an atrial septal defect),
//! * extra discrete sounds, closed-form atoms of the same family as the valve closures (S3, the
//!   ejection clicks, the mid-systolic click, the opening snap; S4 and the atrial rub component come
//!   from the atrial activation, see [`atrial_sounds`]),
//! * murmurs ([`Murmur`]): an envelope between the anchors of the beat times a **seeded sum of
//!   sinusoids** ([`Partial`]), random phases and small detunings drawn from the scenario seed and
//!   the beat time. That keeps the synthesiser random-access: a sample at time *t* depends on the
//!   events up to *t*, the seed and *t* only, with no filter state.
//!
//! **Causal.** Everything a beat schedules lies after its own event. A diastolic murmur that has no
//! planned end runs until the next ventricular event and is then faded out from that event on
//! ([`Murmur::cut`]): samples before the event are never revised.
//!
//! **Identifiable.** Every discrete sound keeps its [`crate::pcg::ComponentKind`] and every murmur
//! its [`MurmurClass`]; [`SoundGains`] scales each class at sampling time, and each murmur's
//! partials keep their frequency and weight. The listening sites of brief B27 part B apply
//! per-class gains and per-partial weights through these, without re-scheduling.
//!
//! Every number lives in [`AuscultationParams`], sourced (author, year, value) or **designed**. All
//! spectral values are designed: the published bands for murmurs conflict, and the sounds are a
//! model, not recordings.

use heartsim_core::math::Deterministic;
use heartsim_core::{Auscultation, LesionKind, MurmurTiming, Severity};
use heartsim_sim::defib::ShockRng;
use serde::{Deserialize, Serialize};

use crate::pcg::AtomParams;

/// Per-severity values `[mild, moderate, severe]`.
pub type BySeverity = [f64; 3];

fn pick(v: &BySeverity, s: Severity) -> f64 {
    match s {
        Severity::Mild => v[0],
        Severity::Moderate => v[1],
        Severity::Severe => v[2],
    }
}

/// A murmur's band: `partials` sinusoids spread log-uniformly over `lo_hz..hi_hz`, each detuned
/// at random inside its slot per beat. Designed (the sources give only qualitative pitch: "low",
/// "high-pitched", "harsh", "musical").
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Band {
    pub lo_hz: f64,
    pub hi_hz: f64,
    pub partials: u32,
}

/// Every parameter of the extra sounds and murmurs. Each field says where its value comes from. "ijcdw
/// part 1/2" are Nemani & Pechetty, *Additional heart sounds* parts 1 and 2 (Indian J Cardiovasc Dis
/// Women), read in full for this brief; "MSD" is the MSD Manual Professional page of the lesion,
/// read in full for this brief.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AuscultationParams {
    // --- extra discrete sounds -----------------------------------------------------------------------
    /// A2 to S3, ms. Felner, Clinical Methods ch. 23: 120–160 ms (via the research note); ijcdw part
    /// 1: 120–200 ms. Designed value inside both, near the lower bound so that S3 and S4 meet at
    /// fast rates (the summation gallop): 125 ms.
    pub a2_s3_ms: f64,
    /// S3: low, 25–50 Hz (ijcdw part 1); duration designed (no source).
    pub s3: AtomParams,
    pub s3_gain: f64,
    /// Organised atrial activation to S4, ms. ijcdw part 1: "occurs 70 milliseconds after the P wave".
    pub atrial_s4_ms: f64,
    /// S4: low, 20–30 Hz (ijcdw part 1); duration designed.
    pub s4: AtomParams,
    pub s4_gain: f64,
    /// QRS onset to the aortic ejection click, ms. ijcdw part 2: 120–140 ms after the Q wave.
    pub q_aortic_click_ms: f64,
    /// QRS onset to the pulmonary ejection click, ms. ijcdw part 2: 90–110 ms after the Q wave.
    pub q_pulmonary_click_ms: f64,
    /// Ejection clicks: high-pitched and brief (MSD, cardiac auscultation); spectrum designed.
    pub click: AtomParams,
    pub click_gain: f64,
    /// Position of the prolapse click between M1 and A2, fraction. Designed ("mid-systolic", MSD).
    pub mid_click_fraction: f64,
    pub mid_click: AtomParams,
    pub mid_click_gain: f64,
    /// A2 to the opening snap by severity, ms. ijcdw part 2: 40–120 ms; under 70 ms suggests severe,
    /// over 100 ms mild stenosis. Designed values inside those bounds.
    pub a2_snap_ms: BySeverity,
    /// The snap: very high-pitched and brief (MSD, cardiac auscultation); spectrum designed.
    pub snap: AtomParams,
    pub snap_gain: f64,

    // --- changes to the valve closures ----------------------------------------------------------------
    /// S1 (M1) gain factor in mitral stenosis. Designed (MSD, mitral stenosis: loud S1; no level).
    pub ms_s1_factor: BySeverity,
    /// A2 gain factor in aortic stenosis. Designed (MSD, aortic stenosis: A2 softens, S2 single when
    /// severe).
    pub as_a2_factor: BySeverity,
    /// A2 delay in aortic stenosis (prolonged left ejection), ms. Designed so that a severe stenosis
    /// gives a single S2 (MSD: "often single"; Felner ch. 23: outflow obstruction delays A2).
    pub as_a2_delay_ms: BySeverity,
    /// P2 gain factor and delay in pulmonary stenosis. Designed (MSD, pulmonic stenosis: P2 softer and
    /// the split widened, not fixed, as severity worsens).
    pub ps_p2_factor: BySeverity,
    pub ps_p2_delay_ms: BySeverity,
    /// A2–P2 split with `fixed_split`, ms. Designed: wide (MSD, ASD: "widely split, fixed S2") and
    /// constant, so the respiratory change is far below 20 ms (Felner ch. 23 via the research note).
    pub fixed_split_ms: f64,

    // --- murmurs ---------------------------------------------------------------------------------------
    /// RMS of a murmur's sum of sinusoids before its gain, so that a gain-1 murmur peaks near 1 (a
    /// normal S1). Designed.
    pub noise_rms: f64,
    /// Fade after a cut (the next ventricular event), ms. Designed: long enough not to click.
    pub cut_fade_ms: f64,
    /// Plateau murmurs' rise and fall, ms. Designed.
    pub plateau_ramp_ms: f64,
    /// Attack of the decrescendo murmurs, ms. Designed.
    pub decrescendo_attack_ms: f64,
    /// Weight of a new RR in the running mean cycle the cycle-length factor compares with. Designed.
    pub rr_mean_weight: f64,
    /// Exponent of `(RR / mean RR)` on the ejection murmurs (aortic and pulmonary stenosis, the ASD
    /// flow murmur), and its limits. Designed (Alpert, Clinical Methods ch. 26, via the research
    /// note, and Harrison's table of interventions: ejection murmurs grow after a premature beat and
    /// after long cycles in AF).
    pub ejection_cycle_exponent: f64,
    pub cycle_factor_min: f64,
    pub cycle_factor_max: f64,
    /// The same exponent on the regurgitant plateau murmurs (MR, TR, VSD). Designed small (Alpert;
    /// Harrison's table: AV-valve regurgitant murmurs do not change comparably).
    pub regurgitant_cycle_exponent: f64,
    /// Gain added at peak inspiration to right-sided murmurs (TR, PS, ASD flow), fraction. Designed
    /// (MSD, tricuspid regurgitation: louder with inspiration, Carvallo sign; MSD auscultation:
    /// inspiration augments right-sided murmurs).
    pub right_inspiration_boost: f64,

    /// Aortic stenosis: from QRS onset (ejection, at the aortic click) to A2 minus `as_end_before_a2_ms`;
    /// peak fraction later with severity (MSD, aortic stenosis); gains designed.
    pub as_start_after_q_ms: f64,
    pub as_end_before_a2_ms: f64,
    pub as_peak_fraction: BySeverity,
    pub as_gain: BySeverity,
    pub as_band: Band,
    /// Pulmonary stenosis: the same on the right, ending before P2 (the late P2 lets a severe murmur
    /// run past A2). Designed.
    pub ps_start_after_q_ms: f64,
    pub ps_end_before_p2_ms: f64,
    pub ps_peak_fraction: BySeverity,
    pub ps_gain: BySeverity,
    pub ps_band: Band,
    /// Mitral regurgitation: plateau from M1 + start to A2 + end ("through A2"). Designed (MSD:
    /// holosystolic).
    pub mr_start_after_m1_ms: f64,
    pub mr_end_after_a2_ms: f64,
    pub mr_gain: BySeverity,
    pub mr_band: Band,
    /// Tricuspid regurgitation: plateau from T1 to P2 + end. Designed (MSD: holosystolic).
    pub tr_start_after_t1_ms: f64,
    pub tr_end_after_p2_ms: f64,
    pub tr_gain: BySeverity,
    pub tr_band: Band,
    /// Ventricular septal defect: plateau from M1 to A2 − end. Designed (MSD, VSD: holosystolic,
    /// high-pitched when small).
    pub vsd_start_after_m1_ms: f64,
    pub vsd_end_before_a2_ms: f64,
    pub vsd_gain: BySeverity,
    pub vsd_band: Band,
    /// Prolapse: late systolic plateau from the click + start to A2 + end. Designed (MSD, MVP).
    pub mvp_start_after_click_ms: f64,
    pub mvp_end_after_a2_ms: f64,
    pub mvp_gain: BySeverity,
    pub mvp_band: Band,
    /// Aortic regurgitation: decrescendo from A2 + start over a fraction of the expected diastole.
    /// MSD, aortic regurgitation: high-pitched decrescendo "beginning soon after A2"; length and gain
    /// designed.
    pub ar_start_after_a2_ms: f64,
    pub ar_length_fraction: BySeverity,
    pub ar_gain: BySeverity,
    pub ar_band: Band,
    /// Mitral stenosis rumble: from the snap + start, a decrescendo over a fraction of the expected
    /// diastole that lengthens with severity; a severe rumble does not end but keeps `ms_floor` of
    /// its level until the next beat (MSD, mitral stenosis: low-pitched rumble, lengthening as the
    /// snap moves closer to S2). Designed.
    pub ms_start_after_snap_ms: f64,
    pub ms_length_fraction: BySeverity,
    pub ms_floor_severe: f64,
    pub ms_gain: BySeverity,
    pub ms_band: Band,
    /// Presystolic accentuation: a crescendo from the atrial activation + start, rising over
    /// `ms_presystolic_rise_ms`, until the next S1. MSD, mitral stenosis: presystolic crescendo from
    /// atrial contraction, lost in AF. Timing and level designed.
    pub ms_presystolic_start_ms: f64,
    pub ms_presystolic_rise_ms: f64,
    pub ms_presystolic_factor: f64,
    /// Atrial septal defect flow murmur (pulmonary ejection): MSD, ASD: soft grade 2–3/6 mid-systolic
    /// murmur. Shape and gains designed.
    pub asd_start_after_q_ms: f64,
    pub asd_end_before_p2_ms: f64,
    pub asd_peak_fraction: f64,
    pub asd_gain: BySeverity,
    pub asd_band: Band,
    /// Innocent (Still's) murmur: early-to-mid systolic, from QRS + start over a fraction of the time
    /// to A2; low and musical (UPenn, innocent murmurs: "mid systolic, low pitched, musical or
    /// vibratory"): a narrow band. Designed.
    pub innocent_start_after_q_ms: f64,
    pub innocent_length_fraction: f64,
    pub innocent_peak_fraction: f64,
    pub innocent_gain: BySeverity,
    pub innocent_band: Band,
    /// Pericardial rub: three short scratchy bursts, at atrial systole (activation + start), in
    /// ventricular systole (QRS + start) and in early diastole (A2 + start). ijcdw part 2 and MSD,
    /// pericarditis: triphasic (atrial systole, ventricular systole, rapid filling). Timing, length
    /// and band designed.
    pub rub_atrial_start_ms: f64,
    pub rub_systolic_start_ms: f64,
    pub rub_diastolic_start_ms: f64,
    pub rub_length_ms: f64,
    pub rub_gain: f64,
    pub rub_band: Band,
}

impl Default for AuscultationParams {
    fn default() -> Self {
        let low = |f0: f64, f1: f64, d: f64| AtomParams {
            f_start_hz: f0,
            f_end_hz: f1,
            duration_ms: d,
            rise_ms: 15.0,
            decay_ms: d * 0.6,
        };
        let high = |f0: f64, f1: f64, d: f64| AtomParams {
            f_start_hz: f0,
            f_end_hz: f1,
            duration_ms: d,
            rise_ms: 1.5,
            decay_ms: d * 0.4,
        };
        let band = |lo_hz: f64, hi_hz: f64, partials: u32| Band {
            lo_hz,
            hi_hz,
            partials,
        };
        Self {
            a2_s3_ms: 125.0,
            s3: low(50.0, 25.0, 80.0),
            s3_gain: 0.5,
            atrial_s4_ms: 70.0,
            s4: AtomParams {
                rise_ms: 22.0,
                ..low(32.0, 20.0, 80.0)
            },
            s4_gain: 0.45,
            q_aortic_click_ms: 130.0,
            q_pulmonary_click_ms: 100.0,
            click: high(420.0, 260.0, 16.0),
            click_gain: 0.5,
            mid_click_fraction: 0.5,
            mid_click: high(460.0, 280.0, 16.0),
            mid_click_gain: 0.55,
            a2_snap_ms: [110.0, 85.0, 60.0],
            snap: high(320.0, 160.0, 28.0),
            snap_gain: 0.55,
            ms_s1_factor: [1.2, 1.3, 1.35],
            as_a2_factor: [0.85, 0.6, 0.3],
            as_a2_delay_ms: [0.0, 10.0, 25.0],
            ps_p2_factor: [0.8, 0.55, 0.3],
            ps_p2_delay_ms: [10.0, 25.0, 45.0],
            fixed_split_ms: 50.0,
            noise_rms: 0.3,
            cut_fade_ms: 20.0,
            plateau_ramp_ms: 15.0,
            decrescendo_attack_ms: 8.0,
            rr_mean_weight: 0.25,
            ejection_cycle_exponent: 0.75,
            cycle_factor_min: 0.6,
            cycle_factor_max: 1.6,
            regurgitant_cycle_exponent: 0.1,
            right_inspiration_boost: 0.6,
            as_start_after_q_ms: 125.0,
            as_end_before_a2_ms: 20.0,
            as_peak_fraction: [0.3, 0.45, 0.62],
            as_gain: [0.25, 0.38, 0.5],
            as_band: band(70.0, 420.0, 40),
            ps_start_after_q_ms: 100.0,
            ps_end_before_p2_ms: 15.0,
            ps_peak_fraction: [0.3, 0.45, 0.62],
            ps_gain: [0.22, 0.32, 0.42],
            ps_band: band(70.0, 380.0, 40),
            mr_start_after_m1_ms: 10.0,
            mr_end_after_a2_ms: 15.0,
            mr_gain: [0.18, 0.26, 0.34],
            mr_band: band(120.0, 500.0, 40),
            tr_start_after_t1_ms: 5.0,
            tr_end_after_p2_ms: 10.0,
            tr_gain: [0.14, 0.2, 0.26],
            tr_band: band(90.0, 400.0, 40),
            vsd_start_after_m1_ms: 10.0,
            vsd_end_before_a2_ms: 5.0,
            vsd_gain: [0.22, 0.3, 0.36],
            vsd_band: band(160.0, 600.0, 40),
            mvp_start_after_click_ms: 20.0,
            mvp_end_after_a2_ms: 5.0,
            mvp_gain: [0.16, 0.24, 0.32],
            mvp_band: band(120.0, 500.0, 40),
            ar_start_after_a2_ms: 5.0,
            ar_length_fraction: [0.45, 0.65, 0.85],
            ar_gain: [0.14, 0.2, 0.26],
            ar_band: band(180.0, 650.0, 40),
            ms_start_after_snap_ms: 30.0,
            ms_length_fraction: [0.35, 0.6, 0.6],
            ms_floor_severe: 0.45,
            ms_gain: [0.2, 0.27, 0.33],
            ms_band: band(30.0, 110.0, 32),
            ms_presystolic_start_ms: 40.0,
            ms_presystolic_rise_ms: 110.0,
            ms_presystolic_factor: 1.2,
            asd_start_after_q_ms: 100.0,
            asd_end_before_p2_ms: 40.0,
            asd_peak_fraction: 0.4,
            asd_gain: [0.1, 0.14, 0.18],
            asd_band: band(60.0, 260.0, 32),
            innocent_start_after_q_ms: 90.0,
            innocent_length_fraction: 0.5,
            innocent_peak_fraction: 0.4,
            innocent_gain: [0.18, 0.22, 0.26],
            innocent_band: band(95.0, 125.0, 8),
            rub_atrial_start_ms: 40.0,
            rub_systolic_start_ms: 110.0,
            rub_diastolic_start_ms: 90.0,
            rub_length_ms: 80.0,
            rub_gain: 0.24,
            rub_band: band(220.0, 750.0, 40),
        }
    }
}

// --- murmurs ---------------------------------------------------------------------------------------

/// Which murmur (or murmur-like sound) a [`Murmur`] is. Part B's site table names these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum MurmurClass {
    AorticStenosis,
    PulmonaryStenosis,
    MitralRegurgitation,
    TricuspidRegurgitation,
    VentricularSeptalDefect,
    /// The late systolic murmur after the prolapse click.
    ProlapseLateSystolic,
    AorticRegurgitation,
    /// The mid-diastolic rumble of mitral stenosis.
    MitralRumble,
    /// Its presystolic accentuation, from the atrial contraction.
    MitralPresystolic,
    /// The pulmonary flow murmur of an atrial septal defect.
    AtrialSeptalFlow,
    InnocentMurmur,
    /// A component of the pericardial friction rub.
    FrictionRub,
}

impl MurmurClass {
    /// Every class, in the order of [`SoundGains::murmurs`].
    pub const ALL: [MurmurClass; 12] = [
        MurmurClass::AorticStenosis,
        MurmurClass::PulmonaryStenosis,
        MurmurClass::MitralRegurgitation,
        MurmurClass::TricuspidRegurgitation,
        MurmurClass::VentricularSeptalDefect,
        MurmurClass::ProlapseLateSystolic,
        MurmurClass::AorticRegurgitation,
        MurmurClass::MitralRumble,
        MurmurClass::MitralPresystolic,
        MurmurClass::AtrialSeptalFlow,
        MurmurClass::InnocentMurmur,
        MurmurClass::FrictionRub,
    ];

    /// Index in [`Self::ALL`].
    #[must_use]
    pub fn index(self) -> usize {
        self as usize
    }

    /// Where in the cycle the murmur lies; `None` for the rub, which is an extra sound
    /// (`ExtraSound::FrictionRub`), not a murmur.
    #[must_use]
    pub fn timing(self) -> Option<MurmurTiming> {
        Some(match self {
            MurmurClass::AorticStenosis | MurmurClass::PulmonaryStenosis | MurmurClass::AtrialSeptalFlow => {
                MurmurTiming::MidSystolic
            }
            MurmurClass::MitralRegurgitation
            | MurmurClass::TricuspidRegurgitation
            | MurmurClass::VentricularSeptalDefect => MurmurTiming::Holosystolic,
            MurmurClass::ProlapseLateSystolic => MurmurTiming::LateSystolic,
            MurmurClass::AorticRegurgitation => MurmurTiming::EarlyDiastolic,
            MurmurClass::MitralRumble => MurmurTiming::MidDiastolic,
            MurmurClass::MitralPresystolic => MurmurTiming::Presystolic,
            MurmurClass::InnocentMurmur => MurmurTiming::EarlySystolic,
            MurmurClass::FrictionRub => return None,
        })
    }

    /// A per-class salt for the partials' seed, so two murmurs of one beat do not share phases.
    fn salt(self) -> u64 {
        0xA076_1D64_78BD_642F_u64.wrapping_mul(self as u64 + 1)
    }
}

/// The shape of a murmur's envelope over its interval (`0..length` ms), peak 1.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Envelope {
    /// Crescendo–decrescendo (raised-sine halves) peaking at `peak_fraction` of the length.
    Diamond { peak_fraction: f64 },
    /// Rise and fall over `ramp_ms`, flat between.
    Plateau { ramp_ms: f64 },
    /// Rise over `attack_ms`, then a linear fall from 1 to `floor` over `decay_ms`, then `floor`.
    /// With a finite length and a zero floor the fall reaches 0 at the end.
    Decrescendo {
        attack_ms: f64,
        decay_ms: f64,
        floor: f64,
    },
    /// A raised-sine rise over `rise_ms`, then 1 until cut.
    Crescendo { rise_ms: f64 },
}

fn raised_sine(x: f64) -> f64 {
    let s = (std::f64::consts::FRAC_PI_2 * x.clamp(0.0, 1.0)).sin_d();
    s * s
}

impl Envelope {
    /// Value at `tau` ms into an interval of `length` ms (`f64::INFINITY` for an open one).
    #[must_use]
    pub fn at(self, tau: f64, length: f64) -> f64 {
        if tau < 0.0 || tau >= length {
            return 0.0;
        }
        match self {
            Envelope::Diamond { peak_fraction } => {
                let peak = length * peak_fraction.clamp(0.01, 0.99);
                if tau <= peak {
                    raised_sine(tau / peak)
                } else {
                    raised_sine((length - tau) / (length - peak))
                }
            }
            Envelope::Plateau { ramp_ms } => {
                let r = ramp_ms.max(1e-3).min(length * 0.5);
                raised_sine(tau / r).min(raised_sine((length - tau) / r))
            }
            Envelope::Decrescendo {
                attack_ms,
                decay_ms,
                floor,
            } => {
                let rise = if attack_ms > 0.0 {
                    (tau / attack_ms).min(1.0)
                } else {
                    1.0
                };
                let fall = if decay_ms > 0.0 {
                    (1.0 - (tau - attack_ms).max(0.0) / decay_ms).max(0.0)
                } else {
                    0.0
                };
                rise * (floor + (1.0 - floor) * fall)
            }
            Envelope::Crescendo { rise_ms } => raised_sine(tau / rise_ms.max(1e-3)),
        }
    }
}

/// One sinusoid of a murmur's sum: frequency, phase (radians at the murmur's start) and weight.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Partial {
    pub hz: f64,
    pub phase: f64,
    pub weight: f64,
}

/// A scheduled murmur: an [`Envelope`] from `start_ms` to `end_ms` (simulation time), times a seeded
/// sum of sinusoids, times `gain`. `end_ms` is infinite for a murmur that lasts until the next beat.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Murmur {
    pub class: MurmurClass,
    pub start_ms: f64,
    pub end_ms: f64,
    pub envelope: Envelope,
    pub gain: f64,
    /// Set when the next ventricular event arrives while the murmur still sounds: from here it fades
    /// out over [`Murmur::fade_ms`].
    pub cut_ms: Option<f64>,
    pub fade_ms: f64,
    pub partials: Vec<Partial>,
    /// The QRS onset (engine time) of the beat it belongs to, or the atrial activation for the
    /// presystolic accentuation and the atrial rub component.
    pub key_ms: f64,
    /// Scheduled from an atrial activation (cancelled if the ventricles fire before it starts).
    pub atrial: bool,
}

impl Murmur {
    /// Envelope times gain at simulation time `t`, including the fade after a cut.
    #[must_use]
    pub fn level_at(&self, t: f64) -> f64 {
        let base = self.envelope.at(t - self.start_ms, self.end_ms - self.start_ms);
        let fade = match self.cut_ms {
            Some(c) if t >= c => (1.0 - (t - c) / self.fade_ms.max(1e-3)).max(0.0),
            _ => 1.0,
        };
        self.gain * base * fade
    }

    /// The sum of sinusoids `tau_ms` after the murmur's start (unit RMS times the weights' scale).
    #[must_use]
    pub fn noise(&self, tau_ms: f64) -> f64 {
        self.partials
            .iter()
            .map(|p| p.weight * (std::f64::consts::TAU * p.hz * tau_ms / 1000.0 + p.phase).sin_d())
            .sum()
    }

    /// The sum of sinusoids with every partial's weight multiplied by `weight(hz)` (part B's bell and
    /// diaphragm presets apply here).
    #[must_use]
    pub fn noise_weighted(&self, tau_ms: f64, weight: impl Fn(f64) -> f64) -> f64 {
        self.partials
            .iter()
            .map(|p| {
                p.weight * weight(p.hz) * (std::f64::consts::TAU * p.hz * tau_ms / 1000.0 + p.phase).sin_d()
            })
            .sum()
    }

    /// The sum of sinusoids with every partial's frequency multiplied by `factor` (small-speaker
    /// mode, brief B28) and its weight by `weight(true hz)`; a partial that would land above
    /// `ceiling_hz` is left out.
    #[must_use]
    pub fn noise_scaled(
        &self,
        tau_ms: f64,
        factor: f64,
        ceiling_hz: f64,
        weight: impl Fn(f64) -> f64,
    ) -> f64 {
        self.partials
            .iter()
            .filter(|p| p.hz * factor <= ceiling_hz)
            .map(|p| {
                p.weight
                    * weight(p.hz)
                    * (std::f64::consts::TAU * p.hz * factor * tau_ms / 1000.0 + p.phase).sin_d()
            })
            .sum()
    }

    /// Whether the murmur is silent from `t` on.
    #[must_use]
    pub fn finished_at(&self, t: f64) -> bool {
        let stop = self
            .cut_ms
            .map_or(self.end_ms, |c| self.end_ms.min(c + self.fade_ms));
        t >= stop
    }

    /// The next ventricular event at `at_ms`: a murmur still sounding then fades out from there.
    pub fn cut(&mut self, at_ms: f64) {
        if self.cut_ms.is_none() && self.start_ms < at_ms && self.end_ms > at_ms {
            self.cut_ms = Some(at_ms);
        }
    }

    /// Where the murmur effectively stops sounding, ms (end, or cut plus fade).
    #[must_use]
    pub fn stop_ms(&self) -> f64 {
        self.cut_ms
            .map_or(self.end_ms, |c| self.end_ms.min(c + self.fade_ms))
    }
}

/// The partials of a band, from the scenario seed and the murmur's key time.
fn partials(band: Band, seed: u64, key_ms: f64, class: MurmurClass, rms: f64) -> Vec<Partial> {
    let mut rng = ShockRng::new(seed ^ key_ms.to_bits() ^ class.salt());
    let n = band.partials.max(1);
    let ratio = band.hi_hz / band.lo_hz.max(1e-3);
    let mut out: Vec<Partial> = (0..n)
        .map(|i| {
            let x = (f64::from(i) + rng.next_f64()) / f64::from(n);
            let hz = band.lo_hz * ratio.powf_d(x);
            let phase = std::f64::consts::TAU * rng.next_f64();
            // a soft window over the band so its edges do not sound cut off
            let weight = 0.35 + 0.65 * (std::f64::consts::PI * x).sin_d();
            Partial { hz, phase, weight }
        })
        .collect();
    let power: f64 = out.iter().map(|p| p.weight * p.weight * 0.5).sum();
    let scale = if power > 0.0 { rms / power.sqrt() } else { 0.0 };
    for p in &mut out {
        p.weight *= scale;
    }
    out
}

/// Per-class gains applied when sampling (brief B27 part B: the listening sites). All 1 is the
/// "overview" mix, which leaves the signal bit-identical.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoundGains {
    /// By [`crate::pcg::ComponentKind`] index.
    pub components: [f64; crate::pcg::ComponentKind::ALL.len()],
    /// By [`MurmurClass::index`].
    pub murmurs: [f64; MurmurClass::ALL.len()],
}

impl Default for SoundGains {
    fn default() -> Self {
        Self {
            components: [1.0; crate::pcg::ComponentKind::ALL.len()],
            murmurs: [1.0; MurmurClass::ALL.len()],
        }
    }
}

impl SoundGains {
    /// Only the murmurs (every discrete sound at 0): tests that measure a murmur alone.
    #[must_use]
    pub fn murmurs_only() -> Self {
        Self {
            components: [0.0; crate::pcg::ComponentKind::ALL.len()],
            ..Self::default()
        }
    }
}

// --- the per-beat model ----------------------------------------------------------------------------

/// What the murmurs of a beat need besides the beat's components.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeatContext {
    /// QRS onset as drawn (the engine event minus the lead), ms.
    pub q_ms: f64,
    /// The engine's ventricular event, ms (the key of everything the beat schedules).
    pub key_ms: f64,
    /// The beat's RR divided by the running mean RR before it (1 for the first beat).
    pub cycle_ratio: f64,
    /// Expected length of the beat's cycle, ms: its RR (the next one is not known yet).
    pub expected_cycle_ms: f64,
    /// Inspiration depth at the beat, `0..=1`.
    pub inspiration: f64,
}

fn cycle_factor(ratio: f64, exponent: f64, p: &AuscultationParams) -> f64 {
    ratio
        .max(1e-3)
        .powf_d(exponent)
        .clamp(p.cycle_factor_min, p.cycle_factor_max)
}

/// The murmurs of one beat with components `c` (after [`crate::pcg::schedule_with`]). Only an
/// ejecting beat (one with A2) has murmurs.
#[must_use]
pub fn beat_murmurs(
    c: &crate::pcg::Components,
    model: &Auscultation,
    p: &AuscultationParams,
    ctx: &BeatContext,
    seed: u64,
) -> Vec<Murmur> {
    let (Some(m1), Some(a2)) = (c.m1, c.a2) else {
        return Vec::new();
    };
    let m1 = m1.onset.0;
    let a2 = a2.onset.0;
    let t1 = c.t1.map_or(m1, |x| x.onset.0);
    let p2 = c.p2.map_or(a2, |x| x.onset.0);
    let q = ctx.q_ms;
    let mut out = Vec::new();
    let mut push = |class: MurmurClass, start: f64, end: f64, envelope: Envelope, gain: f64, band: Band| {
        if end > start && gain > 0.0 {
            out.push(Murmur {
                class,
                start_ms: start,
                end_ms: end,
                envelope,
                gain,
                cut_ms: None,
                fade_ms: p.cut_fade_ms,
                partials: partials(band, seed, ctx.key_ms, class, p.noise_rms),
                key_ms: ctx.key_ms,
                atrial: false,
            });
        }
    };
    let ejection = cycle_factor(ctx.cycle_ratio, p.ejection_cycle_exponent, p);
    let regurgitant = cycle_factor(ctx.cycle_ratio, p.regurgitant_cycle_exponent, p);
    let right = 1.0 + p.right_inspiration_boost * ctx.inspiration;
    let plateau = Envelope::Plateau {
        ramp_ms: p.plateau_ramp_ms,
    };
    // expected diastole from A2, from the beat's own RR (causal: the next RR is unknown)
    let diastole = (ctx.key_ms + ctx.expected_cycle_ms - a2).max(50.0);
    for &(kind, sev) in &model.lesions {
        match kind {
            LesionKind::AorticStenosis => push(
                MurmurClass::AorticStenosis,
                q + p.as_start_after_q_ms,
                a2 - p.as_end_before_a2_ms,
                Envelope::Diamond {
                    peak_fraction: pick(&p.as_peak_fraction, sev),
                },
                pick(&p.as_gain, sev) * ejection,
                p.as_band,
            ),
            LesionKind::PulmonaryStenosis => push(
                MurmurClass::PulmonaryStenosis,
                q + p.ps_start_after_q_ms,
                p2 - p.ps_end_before_p2_ms,
                Envelope::Diamond {
                    peak_fraction: pick(&p.ps_peak_fraction, sev),
                },
                pick(&p.ps_gain, sev) * ejection * right,
                p.ps_band,
            ),
            LesionKind::AtrialSeptalDefect => push(
                MurmurClass::AtrialSeptalFlow,
                q + p.asd_start_after_q_ms,
                p2 - p.asd_end_before_p2_ms,
                Envelope::Diamond {
                    peak_fraction: p.asd_peak_fraction,
                },
                pick(&p.asd_gain, sev) * ejection * right,
                p.asd_band,
            ),
            LesionKind::MitralRegurgitation => push(
                MurmurClass::MitralRegurgitation,
                m1 + p.mr_start_after_m1_ms,
                a2 + p.mr_end_after_a2_ms,
                plateau,
                pick(&p.mr_gain, sev) * regurgitant,
                p.mr_band,
            ),
            LesionKind::TricuspidRegurgitation => push(
                MurmurClass::TricuspidRegurgitation,
                t1 + p.tr_start_after_t1_ms,
                p2 + p.tr_end_after_p2_ms,
                plateau,
                pick(&p.tr_gain, sev) * regurgitant * right,
                p.tr_band,
            ),
            LesionKind::VentricularSeptalDefect => push(
                MurmurClass::VentricularSeptalDefect,
                m1 + p.vsd_start_after_m1_ms,
                a2 - p.vsd_end_before_a2_ms,
                plateau,
                pick(&p.vsd_gain, sev) * regurgitant,
                p.vsd_band,
            ),
            LesionKind::MitralValveProlapse => {
                if let Some(click) = c.mid_systolic_click {
                    push(
                        MurmurClass::ProlapseLateSystolic,
                        click.onset.0 + p.mvp_start_after_click_ms,
                        a2 + p.mvp_end_after_a2_ms,
                        Envelope::Plateau {
                            ramp_ms: 2.0 * p.plateau_ramp_ms,
                        },
                        pick(&p.mvp_gain, sev) * regurgitant,
                        p.mvp_band,
                    );
                }
            }
            LesionKind::AorticRegurgitation => {
                let start = a2 + p.ar_start_after_a2_ms;
                let length = pick(&p.ar_length_fraction, sev) * diastole;
                push(
                    MurmurClass::AorticRegurgitation,
                    start,
                    start + length,
                    Envelope::Decrescendo {
                        attack_ms: p.decrescendo_attack_ms,
                        decay_ms: (length - p.decrescendo_attack_ms).max(1.0),
                        floor: 0.0,
                    },
                    pick(&p.ar_gain, sev),
                    p.ar_band,
                );
            }
            LesionKind::MitralStenosis => {
                let Some(snap) = c.opening_snap else { continue };
                let start = snap.onset.0 + p.ms_start_after_snap_ms;
                let length =
                    pick(&p.ms_length_fraction, sev) * (ctx.key_ms + ctx.expected_cycle_ms - start).max(50.0);
                let (end, floor) = if sev == Severity::Severe {
                    (f64::INFINITY, p.ms_floor_severe)
                } else {
                    (start + length, 0.0)
                };
                push(
                    MurmurClass::MitralRumble,
                    start,
                    end,
                    Envelope::Decrescendo {
                        attack_ms: 2.0 * p.decrescendo_attack_ms,
                        decay_ms: (length - 2.0 * p.decrescendo_attack_ms).max(1.0),
                        floor,
                    },
                    pick(&p.ms_gain, sev),
                    p.ms_band,
                );
            }
            LesionKind::InnocentMurmur => {
                let start = q + p.innocent_start_after_q_ms;
                push(
                    MurmurClass::InnocentMurmur,
                    start,
                    start + p.innocent_length_fraction * (a2 - start),
                    Envelope::Diamond {
                        peak_fraction: p.innocent_peak_fraction,
                    },
                    pick(&p.innocent_gain, sev),
                    p.innocent_band,
                );
            }
        }
    }
    if model.rub {
        let len = p.rub_length_ms;
        let diamond = Envelope::Diamond { peak_fraction: 0.4 };
        let s = q + p.rub_systolic_start_ms;
        push(
            MurmurClass::FrictionRub,
            s,
            s + len,
            diamond,
            p.rub_gain,
            p.rub_band,
        );
        let d = a2 + p.rub_diastolic_start_ms;
        push(
            MurmurClass::FrictionRub,
            d,
            d + len,
            diamond,
            p.rub_gain,
            p.rub_band,
        );
        // one class, one beat: give the second burst its own phases
        if let Some(last) = out.last_mut().filter(|m| m.start_ms.to_bits() == d.to_bits()) {
            last.partials = partials(
                p.rub_band,
                seed,
                ctx.key_ms + 0.5,
                MurmurClass::FrictionRub,
                p.noise_rms,
            );
        }
    }
    out
}

/// The sounds of an organised atrial activation at `t_ms` (engine time): S4 (onset, gain before the
/// jitter) when the scenario has one and the activation falls in ventricular diastole; the
/// presystolic accentuation of mitral stenosis under the same condition; the atrial component of a
/// rub. Atrial fibrillation, flutter and silent atria never call this: they have no organised
/// activation.
#[must_use]
pub fn atrial_sounds(
    t_ms: f64,
    in_diastole: bool,
    model: &Auscultation,
    p: &AuscultationParams,
    seed: u64,
) -> (Option<(f64, f64)>, Vec<Murmur>) {
    let s4 = (model.s4 && in_diastole).then_some((t_ms + p.atrial_s4_ms, p.s4_gain));
    let mut murmurs = Vec::new();
    let base = |class: MurmurClass, start: f64, end: f64, envelope: Envelope, gain: f64, band: Band| Murmur {
        class,
        start_ms: start,
        end_ms: end,
        envelope,
        gain,
        cut_ms: None,
        fade_ms: p.cut_fade_ms,
        partials: partials(band, seed, t_ms, class, p.noise_rms),
        key_ms: t_ms,
        atrial: true,
    };
    if in_diastole && let Some(sev) = model.lesion(LesionKind::MitralStenosis) {
        murmurs.push(base(
            MurmurClass::MitralPresystolic,
            t_ms + p.ms_presystolic_start_ms,
            f64::INFINITY,
            Envelope::Crescendo {
                rise_ms: p.ms_presystolic_rise_ms,
            },
            pick(&p.ms_gain, sev) * p.ms_presystolic_factor,
            p.ms_band,
        ));
    }
    if model.rub {
        let s = t_ms + p.rub_atrial_start_ms;
        murmurs.push(base(
            MurmurClass::FrictionRub,
            s,
            s + p.rub_length_ms,
            Envelope::Diamond { peak_fraction: 0.4 },
            p.rub_gain,
            p.rub_band,
        ));
    }
    (s4, murmurs)
}

/// The A2 and P2 changes and the S1 factor of the section's lesions, for [`crate::pcg::schedule_with`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct ValveChanges {
    pub s1_factor: Option<f64>,
    pub a2_factor: Option<f64>,
    pub a2_delay_ms: f64,
    pub p2_factor: Option<f64>,
    pub p2_delay_ms: f64,
}

pub(crate) fn valve_changes(model: &Auscultation, p: &AuscultationParams) -> ValveChanges {
    let mut v = ValveChanges::default();
    if let Some(s) = model.lesion(LesionKind::MitralStenosis) {
        v.s1_factor = Some(pick(&p.ms_s1_factor, s));
    }
    if let Some(s) = model.lesion(LesionKind::AorticStenosis) {
        v.a2_factor = Some(pick(&p.as_a2_factor, s));
        v.a2_delay_ms = pick(&p.as_a2_delay_ms, s);
    }
    if let Some(s) = model.lesion(LesionKind::PulmonaryStenosis) {
        v.p2_factor = Some(pick(&p.ps_p2_factor, s));
        v.p2_delay_ms = pick(&p.ps_p2_delay_ms, s);
    }
    v
}

/// A2 to the opening snap for a mitral stenosis of severity `s`, ms.
#[must_use]
pub fn snap_interval_ms(s: Severity, p: &AuscultationParams) -> f64 {
    pick(&p.a2_snap_ms, s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelopes_have_their_shapes() {
        let d = Envelope::Diamond { peak_fraction: 0.3 };
        assert_eq!(d.at(-1.0, 100.0), 0.0);
        assert!((d.at(30.0, 100.0) - 1.0).abs() < 1e-12);
        assert!(d.at(10.0, 100.0) < d.at(20.0, 100.0));
        assert!(d.at(80.0, 100.0) < d.at(50.0, 100.0));
        assert_eq!(d.at(100.0, 100.0), 0.0);
        let p = Envelope::Plateau { ramp_ms: 10.0 };
        assert!((p.at(50.0, 100.0) - 1.0).abs() < 1e-12 && p.at(2.0, 100.0) < 0.2);
        let dc = Envelope::Decrescendo {
            attack_ms: 5.0,
            decay_ms: 95.0,
            floor: 0.0,
        };
        let mut prev = f64::INFINITY;
        for i in 5..100 {
            let v = dc.at(f64::from(i), 100.0);
            assert!(v <= prev);
            prev = v;
        }
        let open = Envelope::Decrescendo {
            attack_ms: 5.0,
            decay_ms: 50.0,
            floor: 0.4,
        };
        assert!((open.at(500.0, f64::INFINITY) - 0.4).abs() < 1e-12);
        let c = Envelope::Crescendo { rise_ms: 100.0 };
        assert!(c.at(50.0, f64::INFINITY) < c.at(90.0, f64::INFINITY));
        assert!((c.at(400.0, f64::INFINITY) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn partials_are_seeded_and_scaled_to_the_rms() {
        let p = AuscultationParams::default();
        let a = partials(p.as_band, 7, 1000.0, MurmurClass::AorticStenosis, 0.3);
        let b = partials(p.as_band, 7, 1000.0, MurmurClass::AorticStenosis, 0.3);
        let c = partials(p.as_band, 7, 1857.0, MurmurClass::AorticStenosis, 0.3);
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(
            a.iter()
                .all(|x| (p.as_band.lo_hz..=p.as_band.hi_hz).contains(&x.hz))
        );
        let power: f64 = a.iter().map(|x| x.weight * x.weight * 0.5).sum();
        assert!((power.sqrt() - 0.3).abs() < 1e-12);
    }

    #[test]
    fn a_cut_fades_from_the_event_and_never_before() {
        let mut m = Murmur {
            class: MurmurClass::MitralRumble,
            start_ms: 100.0,
            end_ms: f64::INFINITY,
            envelope: Envelope::Crescendo { rise_ms: 10.0 },
            gain: 1.0,
            cut_ms: None,
            fade_ms: 20.0,
            partials: Vec::new(),
            key_ms: 0.0,
            atrial: false,
        };
        let before = m.level_at(299.0);
        m.cut(300.0);
        assert!((m.level_at(299.0) - before).abs() < 1e-15);
        assert!((m.level_at(310.0) - 0.5).abs() < 1e-12);
        assert!(m.finished_at(320.0) && !m.finished_at(319.0));
        // a murmur that has not started, or has ended, is not cut
        let mut later = m.clone();
        later.cut_ms = None;
        later.start_ms = 400.0;
        later.cut(300.0);
        assert_eq!(later.cut_ms, None);
    }
}
