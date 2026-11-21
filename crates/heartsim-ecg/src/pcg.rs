//! Silent phonocardiogram (brief B24): the first and second heart sounds synthesised from the same
//! timing-engine events as the ECG, so the trace under the strip, and later the sound, can never
//! disagree with the rhythm on it.
//!
//! Three stages, each a pure function of its input:
//!
//! 1. [`BeatAssembler`] turns the event stream into one [`Beat`] per ventricular activation: QRS
//!    onset, the last atrial activation and the atrium-to-ventricle interval, the preceding RR, the
//!    origin, and which ventricle is activated late and by how much. The bundle-branch decision is the
//!    one the ECG synthesiser uses ([`crate::activation`]).
//! 2. [`schedule`] places the valve closures of the beat ([`Components`]): M1 and T1 (S1), A2 and P2
//!    (S2), with S1 loudness from the atrium-to-ventricle interval and the S2 split from the respiratory
//!    phase and the interventricular delay. The rules and their sources are in
//!    `docs/research/heart-sounds.md`.
//! 3. [`PcgSynth`] renders each closure as a closed-form falling-frequency chirp at
//!    [`PCG_RATE_HZ`], with a small amplitude jitter that is a pure function of the scenario seed and
//!    the beat's time.
//! 4. [`WallTimeSynth`] (brief B25) renders the same components in wall time for listening in slow
//!    motion: the intervals between the sounds stretch with the speed, each sound keeps its pitch.
//!
//! Every number lives in [`PcgParams`], whose field documentation names the source or says
//! **designed**. "Unverified" marks values the research could only read in search extracts of the
//! paper (see the brief's list); they are to be re-read before being called sourced.
//!
//! No I/O, no audio, no wall clock: the module compiles on `wasm32` and the same events give the
//! same samples on every platform (transcendentals through [`Deterministic`]).

use crate::activation::{BranchBlocks, Ventricle};
use crate::auscultation::{
    AuscultationParams, BeatContext, Murmur, SoundGains, ValveChanges, atrial_sounds, beat_murmurs,
    snap_interval_ms, valve_changes,
};
use crate::listening::{ChestWeight, Listening};
use heartsim_core::Millis;
use heartsim_core::math::Deterministic;
use heartsim_core::{Auscultation, LesionKind};
use heartsim_sim::defib::ShockRng;
use heartsim_sim::timing::{RespiratoryClock, inspiration_depth};
use heartsim_sim::{Circuit, Event, EventKind, Origin};
use serde::{Deserialize, Serialize};

/// Internal sampling rate of the phonocardiogram, Hz: 2 kHz of bandwidth (more than the heart
/// sounds need), four samples per 1 ms engine step and an integer ratio to 48 kHz audio
/// (research note, "A fixed internal rate of 4 kHz").
pub const PCG_RATE_HZ: f32 = 4000.0;
/// Sample period of the phonocardiogram, ms.
pub const PCG_PERIOD_MS: f64 = 1000.0 / PCG_RATE_HZ as f64;

/// Small-speaker mode (brief B28): every component's frequency is multiplied by this factor in the
/// sound that is played, so S3, S4 and the mitral rumble (20–110 Hz in this model) move to 60–330 Hz,
/// where laptop and phone speakers still reproduce something. **Designed**: one and a half octaves is
/// enough to lift the lowest model sounds above the roughly 150–200 Hz at which small speakers fall
/// away, and keeps the highest discrete sound (the mid-systolic click, 460 Hz) below
/// [`SCALED_CEILING_HZ`]. No study shows a teaching effect (research note, "Repetition, a synchronised
/// trace…"); the interface labels the sound as altered.
pub const SMALL_SPEAKER_FACTOR: f64 = 3.0;
/// Highest frequency a murmur partial may reach after scaling, Hz: partials that would land above it
/// are left out, so nothing folds back from the 2 kHz Nyquist limit or falls into the resampler's
/// transition band (above 1.6 kHz). Designed.
pub const SCALED_CEILING_HZ: f64 = 1500.0;

/// Every parameter of the heart-sound model. Each field says where its value comes from: a source
/// (author, year, the value), or **designed** (a choice made for the model, within what the sources
/// describe qualitatively). The research note is `docs/research/heart-sounds.md`; the notes under
/// `docs/research/heart-sounds-notes/` hold the extracts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PcgParams {
    // --- beat assembly ---------------------------------------------------------------------------
    /// An atrial activation more than this long before the QRS does not belong to the beat, ms.
    /// Designed (brief B24: 400 ms, longer than any conducted PR in the scenario library).
    pub av_window_ms: f64,
    /// Ventricular activations closer than this are fronts of one beat, ms. Designed (the ECG
    /// synthesiser's `SAME_BEAT_MS`).
    pub same_beat_ms: f64,
    /// An atrial flutter or fibrillation event this recent means the atria do not contract, ms.
    /// Designed (longer than the slowest flutter cycle in the library).
    pub atrial_circuit_window_ms: f64,
    /// Organised atria that last fired within this time but outside [`Self::av_window_ms`] are
    /// beating but dissociated (complete block): the leaflets have drifted shut, ms. Designed.
    pub atrial_memory_ms: f64,

    // --- interventricular delays (order of activation) -------------------------------------------
    /// Right-ventricular delay of right bundle-branch block, ms. Designed: the sources give no
    /// magnitude; chosen so that T1 reaches its 60 ms maximum (Felner, Clinical Methods ch. 22) and
    /// the S2 split stays wide in both phases (Felner ch. 23).
    pub rbbb_rv_delay_ms: f64,
    /// Left-ventricular delay of left bundle-branch block, ms. Designed: large enough that P2 precedes
    /// A2 in expiration and the paradoxical split narrows on inspiration (Felner ch. 23; MSD Manual).
    pub lbbb_lv_delay_ms: f64,
    /// Left-ventricular delay of right-ventricular pacing, ms. Designed to reproduce the one sourced
    /// paced case, a paradoxical split of 100 ms in expiration and 70 ms in inspiration
    /// (Eur Heart J Case Rep 2023, 7(5) ytad233), with the base split below.
    pub rv_pacing_lv_delay_ms: f64,
    /// Right-ventricular delay of left-ventricular pacing, ms. Designed (Felner ch. 23: LV-paced
    /// beats split S2 like RBBB; no magnitude given).
    pub lv_pacing_rv_delay_ms: f64,
    /// Delay of the far ventricle in a ventricular ectopic, escape or VT beat, ms. Designed (Felner
    /// ch. 22–23: ventricular ectopy splits S1 and S2 like the matching bundle-branch block).
    pub ectopic_delay_ms: f64,
    /// Delay of the far ventricle in a pre-excited beat (fusion over the accessory pathway and the
    /// His–Purkinje system), ms. Designed.
    pub preexcited_delay_ms: f64,
    /// Interventricular delay of a biventricular paced beat, ms. Designed (resynchronisation: both
    /// ventricles start together).
    pub biventricular_delay_ms: f64,

    // --- timing of the components ----------------------------------------------------------------
    /// How far the drawn QRS onset precedes the engine's ventricular event, ms: the sources time M1
    /// and A2 from the first deflection of the QRS, and the Tier-0 Q wave (centre 8 ms, σ 7 ms) leaves
    /// the baseline about 5 ms before the event (measured by the B4 analyser at 50–100 bpm). Designed.
    pub qrs_onset_lead_ms: f64,
    /// QRS onset to M1, ms. Leech et al. 1980 (Br Heart J), about 40 ms at a PR of 180–190 ms
    /// (unverified: seen as an extract); a 2025 synchronised ECG/PCG study measured 40.45 ms
    /// (PubMed 40648475, extract).
    pub q_m1_ms: f64,
    /// Fraction of a late left-ventricular delay by which M1 moves. Designed (Felner ch. 22: LBBB
    /// does not split S1, so M1 follows the LV by less than the full electrical delay).
    pub m1_lv_delay_fraction: f64,
    /// M1 to T1 with normal activation, ms. Felner ch. 22: 20–30 ms.
    pub m1_t1_ms: f64,
    /// Widest M1–T1 interval (late right ventricle), ms. Felner ch. 22: up to 60 ms in RBBB,
    /// ventricular ectopy and LV pacing.
    pub m1_t1_max_ms: f64,
    /// QS2 regression intercept, ms (`QS2 = intercept − slope · HR`). Weissler 1968 as tabulated by
    /// Boudoulas et al. 1979 (Circulation 60:164), men: 546 ms (unverified: seen as an extract).
    pub qs2_intercept_ms: f64,
    /// QS2 regression slope, ms per bpm. Weissler / Boudoulas 1979, men: 2.1; the slopes are confirmed
    /// in Orter et al. 2022 (Exp Physiol 107:213), read in full.
    pub qs2_slope_ms_per_bpm: f64,
    /// Lowest and highest heart rate the QS2 regression is applied at, bpm. Designed (brief B24:
    /// 40–180 bpm, the range of the regression's use in the sources).
    pub qs2_min_bpm: f64,
    pub qs2_max_bpm: f64,
    /// Heart rate assumed for the first beat, which has no preceding RR, bpm. Designed.
    pub first_beat_bpm: f64,
    /// A2–P2 split in expiration, ms. Felner ch. 23: under 30 ms in expiration; Djebbari &
    /// Bereksi-Reguig 2013 (citing): under 15 ms. Designed value within both: 10 ms.
    pub split_expiration_ms: f64,
    /// A2–P2 split at peak inspiration, ms. Felner ch. 23: 20–80 ms in inspiration; Djebbari 2013:
    /// 30–80 ms. Designed value within both: 40 ms.
    pub split_inspiration_ms: f64,
    /// A ventricular beat coupled closer than this ejects nothing: S1 only, no S2, ms. Designed (the
    /// research found no source; standard clinical reasoning).
    pub min_ejecting_rr_ms: f64,

    // --- loudness ----------------------------------------------------------------------------------
    /// S1 is loudest when the atrium-to-ventricle interval is at most this, ms. Felner ch. 22:
    /// loudest at a PR of 80–120 ms.
    pub s1_loud_av_ms: f64,
    /// S1 is soft from this atrium-to-ventricle interval, ms. Felner ch. 22: soft or absent beyond
    /// about 200 ms.
    pub s1_soft_av_ms: f64,
    /// Beyond this interval S1 keeps its floor gain, ms. Designed.
    pub s1_floor_av_ms: f64,
    /// S1 gain at and below [`Self::s1_loud_av_ms`]. Designed (no decibel relation was sourced),
    /// so that the library's normal PR of about 160 ms gives a gain near 1.
    pub s1_gain_loud: f64,
    /// S1 gain at [`Self::s1_soft_av_ms`]. Designed.
    pub s1_gain_soft: f64,
    /// S1 gain at and beyond [`Self::s1_floor_av_ms`], and of a beat whose organised atria fired
    /// too long ago (dissociation). Designed.
    pub s1_gain_floor: f64,
    /// S1 gain with no atrial contraction (fibrillation, flutter, silent atria) at the reference
    /// cycle. Designed (Felner ch. 22: S1 varies in AF; no level given).
    pub s1_gain_no_atrium: f64,
    /// Reference RR of the no-atrium gain, ms, and the exponent of `(reference / RR)`: a short
    /// cycle leaves the leaflets wide open and S1 louder. Designed.
    pub s1_rr_reference_ms: f64,
    pub s1_rr_exponent: f64,
    /// Limits of that cycle-length factor. Designed.
    pub s1_rr_factor_min: f64,
    pub s1_rr_factor_max: f64,
    /// T1 amplitude relative to M1. Designed (Felner ch. 22: T1 is softer than M1).
    pub t1_gain: f64,
    /// A2 and P2 amplitudes relative to a gain-1 M1. Designed (the trace stands for a mid-precordial
    /// site; P2 is softer than A2, Felner ch. 23).
    pub a2_gain: f64,
    pub p2_gain: f64,
    /// Beat-to-beat amplitude jitter of every component, as a fraction (uniform ±). Designed (a few
    /// per cent, so a regular rhythm does not sound machine-made yet S1 stays steady in sinus).
    pub amplitude_jitter: f64,

    // --- atoms ---------------------------------------------------------------------------------------
    /// Mitral and tricuspid closure chirps. Designed in the A2/P2 family, lower and longer, so that
    /// S1 lasts about 120 ms overall (Springer et al. 2016: S1 122 ± 22 ms, energy mostly below
    /// 150 Hz).
    pub m1: AtomParams,
    pub t1: AtomParams,
    /// Aortic and pulmonic closure chirps: A2 250 → 53 Hz and P2 200 → 50 Hz over 60 ms, the
    /// trajectory of Xu, Durand & Pibarot 2001 (IEEE TBME 48:277) as restated by Djebbari &
    /// Bereksi-Reguig 2013 (unverified: the constants were not re-read in the original). The
    /// envelope time constants are designed.
    pub a2: AtomParams,
    pub p2: AtomParams,

    // --- extra sounds and murmurs (brief B27) --------------------------------------------------------
    /// S3, S4, clicks, the snap, the murmurs and the rub; used only by scenarios with an
    /// `auscultation` section.
    pub auscultation: AuscultationParams,
}

/// One valve-closure atom: a chirp whose instantaneous frequency falls as `f(τ) = a + b / √τ` from
/// `f_start_hz` at 1 ms to `f_end_hz` at `duration_ms` (the square-root law of the Xu model), under the
/// envelope `(1 − e^(−τ/rise)) · e^(−τ/decay) · sin(π τ / duration)`, normalised to a peak of 1.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AtomParams {
    pub f_start_hz: f64,
    pub f_end_hz: f64,
    pub duration_ms: f64,
    pub rise_ms: f64,
    pub decay_ms: f64,
}

impl Default for PcgParams {
    fn default() -> Self {
        Self {
            av_window_ms: 400.0,
            same_beat_ms: 150.0,
            atrial_circuit_window_ms: 600.0,
            atrial_memory_ms: 2500.0,
            rbbb_rv_delay_ms: 40.0,
            lbbb_lv_delay_ms: 50.0,
            rv_pacing_lv_delay_ms: 110.0,
            lv_pacing_rv_delay_ms: 50.0,
            ectopic_delay_ms: 50.0,
            preexcited_delay_ms: 20.0,
            biventricular_delay_ms: 0.0,
            qrs_onset_lead_ms: 5.0,
            q_m1_ms: 40.0,
            m1_lv_delay_fraction: 0.5,
            m1_t1_ms: 25.0,
            m1_t1_max_ms: 60.0,
            qs2_intercept_ms: 546.0,
            qs2_slope_ms_per_bpm: 2.1,
            qs2_min_bpm: 40.0,
            qs2_max_bpm: 180.0,
            first_beat_bpm: 70.0,
            split_expiration_ms: 10.0,
            split_inspiration_ms: 40.0,
            min_ejecting_rr_ms: 300.0,
            s1_loud_av_ms: 120.0,
            s1_soft_av_ms: 200.0,
            s1_floor_av_ms: 300.0,
            s1_gain_loud: 1.4,
            s1_gain_soft: 0.6,
            s1_gain_floor: 0.35,
            s1_gain_no_atrium: 0.85,
            s1_rr_reference_ms: 800.0,
            s1_rr_exponent: 0.5,
            s1_rr_factor_min: 0.6,
            s1_rr_factor_max: 1.4,
            t1_gain: 0.6,
            a2_gain: 0.8,
            p2_gain: 0.45,
            amplitude_jitter: 0.03,
            m1: AtomParams {
                f_start_hz: 140.0,
                f_end_hz: 30.0,
                duration_ms: 95.0,
                rise_ms: 8.0,
                decay_ms: 30.0,
            },
            t1: AtomParams {
                f_start_hz: 120.0,
                f_end_hz: 30.0,
                duration_ms: 95.0,
                rise_ms: 8.0,
                decay_ms: 30.0,
            },
            a2: AtomParams {
                f_start_hz: 250.0,
                f_end_hz: 53.0,
                duration_ms: 60.0,
                rise_ms: 5.0,
                decay_ms: 18.0,
            },
            p2: AtomParams {
                f_start_hz: 200.0,
                f_end_hz: 50.0,
                duration_ms: 60.0,
                rise_ms: 5.0,
                decay_ms: 18.0,
            },
            auscultation: AuscultationParams::default(),
        }
    }
}

// --- beats -----------------------------------------------------------------------------------------

/// Where a beat started, from `Event::origin` and the kind of its first ventricular event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BeatOrigin {
    Sinus,
    /// An atrial focus, re-entry or accessory-pathway beat conducted from the atria.
    Atrial,
    /// Junctional escape, junctional focus, AV-nodal re-entry or a His stimulus.
    Junctional,
    /// A ventricular focus, escape, tachycardia or fibrillation event.
    Ventricular,
    /// An artificial pacemaker or a programmed stimulus.
    Paced,
}

/// What the atria were doing when the ventricles were activated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AtrialActivity {
    /// An organised atrial activation within [`PcgParams::av_window_ms`] before the QRS.
    Contracted,
    /// Organised atria, but their last activation is older than the window (AV dissociation).
    Dissociated,
    /// Atrial fibrillation: no atrial contraction.
    Fibrillating,
    /// Atrial flutter: no effective contraction to time S1 from.
    Fluttering,
    /// No atrial activity (sinus arrest, junctional rhythm without retrograde P).
    Silent,
}

/// One ventricular activation, as the heart-sound model needs it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Beat {
    /// Time of the first ventricular event of the beat (QRS onset).
    pub qrs_onset: Millis,
    /// The last organised atrial activation within the window before the QRS (conducted or not:
    /// in complete block the P waves still time the leaflets).
    pub atrial_activation: Option<Millis>,
    /// QRS onset minus that activation, ms; `None` when the atria fibrillate, flutter or are silent,
    /// or fired too long ago.
    pub av_interval_ms: Option<f64>,
    pub atria: AtrialActivity,
    /// Interval from the previous ventricular activation, ms.
    pub rr_ms: Option<f64>,
    pub origin: BeatOrigin,
    /// The ventricle activated late, if one is (`None`: simultaneous).
    pub late_ventricle: Option<Ventricle>,
    /// How much later the late ventricle is activated, ms (0 when simultaneous).
    pub interventricular_delay_ms: f64,
    /// `false` for a wavelet of ventricular fibrillation: no organised contraction, no sound.
    pub organised: bool,
    /// This beat revises the previous one (the second lead of a biventricular pace has fused with
    /// the first): its components replace those scheduled for the same QRS onset.
    pub replaces_previous: bool,
}

impl Beat {
    /// The ventricle activated first, if one leads.
    #[must_use]
    pub fn first_ventricle(&self) -> Option<Ventricle> {
        self.late_ventricle.map(Ventricle::other)
    }

    /// `(left, right)` activation delays after the QRS onset, ms.
    #[must_use]
    pub fn ventricle_delays_ms(&self) -> (f64, f64) {
        match self.late_ventricle {
            Some(Ventricle::Left) => (self.interventricular_delay_ms, 0.0),
            Some(Ventricle::Right) => (0.0, self.interventricular_delay_ms),
            None => (0.0, 0.0),
        }
    }
}

/// Turns timing-engine events (in engine order) into [`Beat`]s.
#[derive(Debug, Clone)]
pub struct BeatAssembler {
    params: PcgParams,
    blocks: BranchBlocks,
    /// Last organised atrial activation: time and origin.
    last_atrial: Option<(f64, Origin)>,
    /// Last atrial circuit event: time and circuit.
    last_atrial_circuit: Option<(f64, Circuit)>,
    /// Start of the current ventricular beat, ms.
    last_beat_ms: Option<f64>,
    /// The beat in progress was started by a pacer channel at that time.
    paced: Option<(f64, u8)>,
    /// The last beat emitted (for biventricular revision).
    last: Option<Beat>,
}

impl BeatAssembler {
    #[must_use]
    pub fn new(params: &PcgParams) -> Self {
        Self {
            params: params.clone(),
            blocks: BranchBlocks::default(),
            last_atrial: None,
            last_atrial_circuit: None,
            last_beat_ms: None,
            paced: None,
            last: None,
        }
    }

    /// Every beat of an event list (a recording).
    #[must_use]
    pub fn assemble(events: &[Event], params: &PcgParams) -> Vec<Beat> {
        let mut a = Self::new(params);
        events.iter().filter_map(|e| a.push(e)).collect()
    }

    /// Feed one event; returns the beat it starts (or revises), if any.
    pub fn push(&mut self, ev: &Event) -> Option<Beat> {
        let t = ev.at.0;
        if ev.kind == EventKind::Blocked {
            self.blocks.record_block(ev.region);
            return None;
        }
        if ev.region == heartsim_core::Region::HisBundle {
            self.blocks.clear();
            return None;
        }
        if ev.region.is_atrial() {
            match ev.origin {
                Origin::Circuit(c) if !c.is_ventricular() => self.last_atrial_circuit = Some((t, c)),
                o => self.last_atrial = Some((t, o)),
            }
            return None;
        }
        if !ev.region.is_ventricular() {
            return None;
        }
        if let Origin::Circuit(c) = ev.origin
            && c.is_ventricular()
        {
            self.blocks.clear();
            let beat = self.new_beat(
                t,
                BeatOrigin::Ventricular,
                Some(Ventricle::of_site(ev.region).other()),
                self.params.ectopic_delay_ms,
            );
            return Some(Beat {
                organised: c != Circuit::VentricularFibrillation,
                ..beat
            });
        }
        if let Some(start) = self.last_beat_ms
            && t - start < self.params.same_beat_ms
        {
            return self.later_front(t, ev);
        }
        let p = self.params.clone();
        let beat = match (ev.kind, ev.origin) {
            (EventKind::Conducted, Origin::AccessoryPathway) => {
                self.blocks.clear();
                let origin = self.supraventricular_origin(t);
                self.new_beat(
                    t,
                    origin,
                    Some(Ventricle::of_site(ev.region).other()),
                    p.preexcited_delay_ms,
                )
            }
            (EventKind::Conducted, o) => {
                // the same pattern decides the QRS template of the strip (`GaussianSynth`)
                let pattern = self.blocks.take();
                let late = pattern.late_ventricle();
                let delay = match late {
                    Some(Ventricle::Left) => p.lbbb_lv_delay_ms,
                    Some(Ventricle::Right) => p.rbbb_rv_delay_ms,
                    None => 0.0,
                };
                let origin = match o {
                    Origin::Sinus => BeatOrigin::Sinus,
                    Origin::Escape => BeatOrigin::Junctional,
                    Origin::Pacer(_) | Origin::External => BeatOrigin::Paced,
                    _ => self.supraventricular_origin(t),
                };
                self.new_beat(t, origin, late, delay)
            }
            (EventKind::Stimulus, Origin::Pacer(ch)) => {
                self.blocks.clear();
                self.paced = Some((t, ch));
                // channel 2 is the LV lead of a biventricular device; 1 (and any other) the RV lead
                let (late, delay) = if ch == 2 {
                    (Ventricle::Right, p.lv_pacing_rv_delay_ms)
                } else {
                    (Ventricle::Left, p.rv_pacing_lv_delay_ms)
                };
                self.new_beat(t, BeatOrigin::Paced, Some(late), delay)
            }
            (EventKind::Stimulus, _) => {
                self.blocks.clear();
                self.new_beat(
                    t,
                    BeatOrigin::Paced,
                    Some(Ventricle::of_site(ev.region).other()),
                    p.ectopic_delay_ms,
                )
            }
            _ => {
                self.blocks.clear();
                self.new_beat(
                    t,
                    BeatOrigin::Ventricular,
                    Some(Ventricle::of_site(ev.region).other()),
                    p.ectopic_delay_ms,
                )
            }
        };
        Some(beat)
    }

    /// A ventricular event inside the current beat: only the second lead of a biventricular pace
    /// changes it.
    fn later_front(&mut self, t: f64, ev: &Event) -> Option<Beat> {
        if let (EventKind::Stimulus, Origin::Pacer(ch)) = (ev.kind, ev.origin)
            && let Some((t0, first)) = self.paced
            && first != ch
            && t - t0 < self.params.same_beat_ms
            && let Some(prev) = self.last
        {
            self.paced = None;
            let revised = Beat {
                late_ventricle: None,
                interventricular_delay_ms: self.params.biventricular_delay_ms,
                replaces_previous: true,
                ..prev
            };
            self.last = Some(revised);
            return Some(revised);
        }
        None
    }

    /// Atrial during flutter or fibrillation, sinus when the beat follows a sinus P, atrial when
    /// another atrial activation precedes it in the window, junctional otherwise.
    fn supraventricular_origin(&self, t: f64) -> BeatOrigin {
        if self
            .last_atrial_circuit
            .is_some_and(|(a, _)| t - a <= self.params.atrial_circuit_window_ms)
        {
            return BeatOrigin::Atrial;
        }
        match self.last_atrial {
            Some((a, Origin::Sinus)) if t - a <= self.params.av_window_ms => BeatOrigin::Sinus,
            Some((a, _)) if t - a <= self.params.av_window_ms => BeatOrigin::Atrial,
            _ => BeatOrigin::Junctional,
        }
    }

    fn new_beat(&mut self, t: f64, origin: BeatOrigin, late: Option<Ventricle>, delay_ms: f64) -> Beat {
        let p = &self.params;
        let rr_ms = self.last_beat_ms.map(|l| t - l);
        self.last_beat_ms = Some(t);
        if !matches!(origin, BeatOrigin::Paced) {
            self.paced = None;
        }
        let circuit = self
            .last_atrial_circuit
            .filter(|(at, _)| t - at <= p.atrial_circuit_window_ms)
            .map(|(_, c)| c);
        let (atria, atrial_activation) = match (circuit, self.last_atrial) {
            (Some(Circuit::AtrialFibrillation), _) => (AtrialActivity::Fibrillating, None),
            (Some(_), _) => (AtrialActivity::Fluttering, None),
            (None, Some((a, _))) if t - a <= p.av_window_ms => (AtrialActivity::Contracted, Some(Millis(a))),
            (None, Some((a, _))) if t - a <= p.atrial_memory_ms => (AtrialActivity::Dissociated, None),
            _ => (AtrialActivity::Silent, None),
        };
        let beat = Beat {
            qrs_onset: Millis(t),
            atrial_activation,
            av_interval_ms: atrial_activation.map(|a| t - a.0),
            atria,
            rr_ms,
            origin,
            late_ventricle: late.filter(|_| delay_ms > 0.0),
            interventricular_delay_ms: if late.is_some() { delay_ms.max(0.0) } else { 0.0 },
            organised: true,
            replaces_previous: false,
        };
        self.last = Some(beat);
        beat
    }
}

// --- the electromechanical model -------------------------------------------------------------------

/// One scheduled valve closure: onset time and amplitude (before the seeded jitter).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Component {
    pub onset: Millis,
    pub gain: f64,
}

/// The heart sounds of one beat. Missing components are silent (no organised beat, or no
/// ejection). The valve closures are always scheduled; the extra sounds (brief B27) only when the
/// scenario's `auscultation` section asks for them. S4 is not here: it follows the atrial
/// activation, not the beat.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Components {
    pub m1: Option<Component>,
    pub t1: Option<Component>,
    pub a2: Option<Component>,
    pub p2: Option<Component>,
    /// Third heart sound, A2 + [`AuscultationParams::a2_s3_ms`].
    pub s3: Option<Component>,
    /// Opening snap of mitral stenosis.
    pub opening_snap: Option<Component>,
    /// Aortic ejection click.
    pub aortic_click: Option<Component>,
    /// Pulmonary ejection click.
    pub pulmonary_click: Option<Component>,
    /// Mid-systolic click of mitral valve prolapse.
    pub mid_systolic_click: Option<Component>,
}

impl Components {
    /// P2 onset minus A2 onset, ms: positive is the normal order, negative a paradoxical split.
    #[must_use]
    pub fn split_ms(&self) -> Option<f64> {
        Some(self.p2?.onset.0 - self.a2?.onset.0)
    }

    /// Onset of S1 (M1), if any.
    #[must_use]
    pub fn s1_onset(&self) -> Option<Millis> {
        self.m1.map(|c| c.onset)
    }

    /// Onset of S2 (whichever of A2 and P2 comes first), if any.
    #[must_use]
    pub fn s2_onset(&self) -> Option<Millis> {
        match (self.a2, self.p2) {
            (Some(a), Some(p)) => Some(Millis(a.onset.0.min(p.onset.0))),
            (Some(c), None) | (None, Some(c)) => Some(c.onset),
            (None, None) => None,
        }
    }
}

/// QS2 (QRS onset to A2) for heart rate `hr_bpm`, ms: `intercept − slope · HR`, HR clamped to the
/// regression's range.
#[must_use]
pub fn qs2_ms(hr_bpm: f64, p: &PcgParams) -> f64 {
    p.qs2_intercept_ms - p.qs2_slope_ms_per_bpm * hr_bpm.clamp(p.qs2_min_bpm, p.qs2_max_bpm)
}

/// S1 gain from the atrium-to-ventricle interval: [`PcgParams::s1_gain_loud`] up to
/// [`PcgParams::s1_loud_av_ms`], falling linearly to the soft gain at [`PcgParams::s1_soft_av_ms`]
/// and to the floor at [`PcgParams::s1_floor_av_ms`], constant beyond. Non-increasing in the
/// interval for any parameters with loud ≥ soft ≥ floor.
#[must_use]
pub fn s1_gain_for_av(av_ms: f64, p: &PcgParams) -> f64 {
    let lerp = |x: f64, x0: f64, x1: f64, y0: f64, y1: f64| {
        if x1 <= x0 {
            y1
        } else {
            y0 + (y1 - y0) * ((x - x0) / (x1 - x0)).clamp(0.0, 1.0)
        }
    };
    if av_ms <= p.s1_loud_av_ms {
        p.s1_gain_loud
    } else if av_ms <= p.s1_soft_av_ms {
        lerp(
            av_ms,
            p.s1_loud_av_ms,
            p.s1_soft_av_ms,
            p.s1_gain_loud,
            p.s1_gain_soft,
        )
    } else {
        lerp(
            av_ms,
            p.s1_soft_av_ms,
            p.s1_floor_av_ms,
            p.s1_gain_soft,
            p.s1_gain_floor,
        )
    }
}

/// S1 gain of a beat: from its atrium-to-ventricle interval; the floor for dissociated atria; the
/// no-atrium gain scaled by the preceding cycle (shorter is louder) without atrial contraction.
#[must_use]
pub fn s1_gain(beat: &Beat, p: &PcgParams) -> f64 {
    match (beat.av_interval_ms, beat.atria) {
        (Some(av), _) => s1_gain_for_av(av, p),
        (None, AtrialActivity::Dissociated) => p.s1_gain_floor,
        (None, _) => {
            let factor = beat.rr_ms.filter(|rr| *rr > 0.0).map_or(1.0, |rr| {
                (p.s1_rr_reference_ms / rr)
                    .powf_d(p.s1_rr_exponent)
                    .clamp(p.s1_rr_factor_min, p.s1_rr_factor_max)
            });
            p.s1_gain_no_atrium * factor
        }
    }
}

/// Place the valve closures of `beat` at respiratory phase `resp_phase` (`0.0..1.0`, see
/// `heartsim_sim::timing::respiratory_phase`):
///
/// * M1 at QRS + [`PcgParams::q_m1_ms`], later by a fraction of a left-ventricular delay (the QRS
///   onset being the drawn one, [`PcgParams::qrs_onset_lead_ms`] before the engine's event);
/// * T1 after M1 by [`PcgParams::m1_t1_ms`] plus the right-ventricular delay, at most
///   [`PcgParams::m1_t1_max_ms`];
/// * A2 at QRS + QS2 (HR of the preceding RR), later by the left-ventricular delay;
/// * P2 at A2 + the respiratory base split + (right − left ventricular delay): negative is a
///   paradoxical split, P2 before A2;
/// * nothing for a fibrillation wavelet; S1 only for a ventricular beat too early to eject.
#[must_use]
pub fn schedule(beat: &Beat, p: &PcgParams, resp_phase: f64) -> Components {
    if !beat.organised {
        return Components::default();
    }
    // the QRS onset on the strip, which the sources measure from
    let q = beat.qrs_onset.0 - p.qrs_onset_lead_ms;
    let (lv, rv) = beat.ventricle_delays_ms();
    let s1 = s1_gain(beat, p);
    let m1 = q + p.q_m1_ms + p.m1_lv_delay_fraction * lv;
    let t1_gap = (p.m1_t1_ms + rv - p.m1_lv_delay_fraction * lv).clamp(0.0, p.m1_t1_max_ms);
    let mut c = Components {
        m1: Some(Component {
            onset: Millis(m1),
            gain: s1,
        }),
        t1: Some(Component {
            onset: Millis(m1 + t1_gap),
            gain: s1 * p.t1_gain,
        }),
        a2: None,
        p2: None,
        ..Components::default()
    };
    let too_early =
        beat.origin == BeatOrigin::Ventricular && beat.rr_ms.is_some_and(|rr| rr < p.min_ejecting_rr_ms);
    if too_early {
        return c;
    }
    let hr = beat
        .rr_ms
        .filter(|rr| *rr > 0.0)
        .map_or(p.first_beat_bpm, |rr| 60_000.0 / rr);
    let a2 = q + qs2_ms(hr, p) + lv;
    let base = p.split_expiration_ms
        + (p.split_inspiration_ms - p.split_expiration_ms) * inspiration_depth(resp_phase);
    let p2 = a2 + base + rv - lv;
    c.a2 = Some(Component {
        onset: Millis(a2),
        gain: p.a2_gain,
    });
    c.p2 = Some(Component {
        onset: Millis(p2),
        gain: p.p2_gain,
    });
    c
}

/// [`schedule`] with a scenario's `auscultation` section (brief B27): an empty section returns
/// exactly [`schedule`]'s components. Otherwise, on top of them:
///
/// * mitral stenosis makes M1 louder; aortic stenosis makes A2 softer and later (single S2 when
///   severe); pulmonary stenosis makes P2 softer and later; `fixed_split` replaces the respiratory
///   split by [`AuscultationParams::fixed_split_ms`];
/// * S3 after A2 of an ejecting beat, the ejection clicks of mild and moderate aortic and pulmonary
///   stenosis (a severe, rigid valve has none: designed), the prolapse click in mid-systole and the
///   snap of mitral stenosis after A2.
#[must_use]
pub fn schedule_with(beat: &Beat, p: &PcgParams, resp_phase: f64, model: &Auscultation) -> Components {
    let c = schedule(beat, p, resp_phase);
    if model.is_empty() {
        return c;
    }
    let v = valve_changes(model, &p.auscultation);
    apply_auscultation(c, beat, p, model, &v)
}

fn apply_auscultation(
    mut c: Components,
    beat: &Beat,
    p: &PcgParams,
    model: &Auscultation,
    v: &ValveChanges,
) -> Components {
    let ap = &p.auscultation;
    let q = beat.qrs_onset.0 - p.qrs_onset_lead_ms;
    if let (Some(f), Some(m1)) = (v.s1_factor, c.m1.as_mut()) {
        m1.gain *= f;
    }
    if model.fixed_split
        && let (Some(a2), Some(p2)) = (c.a2, c.p2.as_mut())
    {
        let (lv, rv) = beat.ventricle_delays_ms();
        p2.onset = Millis(a2.onset.0 + ap.fixed_split_ms + rv - lv);
    }
    if let Some(p2) = c.p2.as_mut() {
        if let Some(f) = v.p2_factor {
            p2.gain *= f;
        }
        p2.onset = Millis(p2.onset.0 + v.p2_delay_ms);
    }
    if let Some(a2) = c.a2.as_mut() {
        if let Some(f) = v.a2_factor {
            a2.gain *= f;
        }
        a2.onset = Millis(a2.onset.0 + v.a2_delay_ms);
    }
    let (Some(m1), Some(a2)) = (c.m1, c.a2) else {
        return c;
    };
    let at = |t: f64, gain: f64| {
        Some(Component {
            onset: Millis(t),
            gain,
        })
    };
    if model.s3 {
        c.s3 = at(a2.onset.0 + ap.a2_s3_ms, ap.s3_gain);
    }
    let mobile = |s: heartsim_core::Severity| s != heartsim_core::Severity::Severe;
    if model.lesion(LesionKind::AorticStenosis).is_some_and(mobile) {
        c.aortic_click = at(q + ap.q_aortic_click_ms, ap.click_gain);
    }
    if model.lesion(LesionKind::PulmonaryStenosis).is_some_and(mobile) {
        c.pulmonary_click = at(q + ap.q_pulmonary_click_ms, ap.click_gain);
    }
    if model.lesion(LesionKind::MitralValveProlapse).is_some() {
        let t = m1.onset.0 + ap.mid_click_fraction * (a2.onset.0 - m1.onset.0);
        c.mid_systolic_click = at(t, ap.mid_click_gain);
    }
    if let Some(s) = model.lesion(LesionKind::MitralStenosis) {
        c.opening_snap = at(a2.onset.0 + snap_interval_ms(s, ap), ap.snap_gain);
    }
    c
}

// --- synthesis ---------------------------------------------------------------------------------------

/// The label drawn over a heart sound on the trace. Discrete sounds only: a murmur is seen as its
/// envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SoundLabel {
    S1,
    S2,
    A2,
    P2,
    S3,
    S4,
    /// Opening snap.
    OpeningSnap,
    /// Aortic or pulmonary ejection click.
    EjectionClick,
    /// Mid-systolic click of mitral valve prolapse.
    Click,
}

impl SoundLabel {
    /// The text on the trace.
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            SoundLabel::S1 => "S1",
            SoundLabel::S2 => "S2",
            SoundLabel::A2 => "A2",
            SoundLabel::P2 => "P2",
            SoundLabel::S3 => "S3",
            SoundLabel::S4 => "S4",
            SoundLabel::OpeningSnap => "OS",
            SoundLabel::EjectionClick => "EC",
            SoundLabel::Click => "C",
        }
    }

    /// The extra sounds of brief B27 (drawn on the lane's lower row with A2 and P2).
    pub const EXTRA: [SoundLabel; 5] = [
        SoundLabel::S3,
        SoundLabel::S4,
        SoundLabel::OpeningSnap,
        SoundLabel::EjectionClick,
        SoundLabel::Click,
    ];
}

/// A label at the onset of a scheduled component (from the schedule, not from peak picking).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SoundMark {
    pub at: Millis,
    pub label: SoundLabel,
}

/// A2 and P2 get their own labels when they are this far apart, ms (the ear resolves a split from
/// about 20 ms: Felner ch. 23; Luo et al. 2021).
pub const LABEL_SPLIT_MS: f64 = 20.0;

/// The labels of one beat's components: S1 at M1, S2 at the first of A2/P2, and A2, P2 at their
/// own onsets when the split exceeds [`LABEL_SPLIT_MS`]; then S3, the clicks and the snap when
/// scheduled.
#[must_use]
pub fn labels(c: &Components) -> Vec<SoundMark> {
    let mut out = Vec::with_capacity(4);
    if let Some(at) = c.s1_onset() {
        out.push(SoundMark {
            at,
            label: SoundLabel::S1,
        });
    }
    if let Some(at) = c.s2_onset() {
        out.push(SoundMark {
            at,
            label: SoundLabel::S2,
        });
    }
    if let (Some(split), Some(a2), Some(p2)) = (c.split_ms(), c.a2, c.p2)
        && split.abs() > LABEL_SPLIT_MS
    {
        out.push(SoundMark {
            at: a2.onset,
            label: SoundLabel::A2,
        });
        out.push(SoundMark {
            at: p2.onset,
            label: SoundLabel::P2,
        });
    }
    for (comp, label) in [
        (c.aortic_click, SoundLabel::EjectionClick),
        (c.pulmonary_click, SoundLabel::EjectionClick),
        (c.mid_systolic_click, SoundLabel::Click),
        (c.opening_snap, SoundLabel::OpeningSnap),
        (c.s3, SoundLabel::S3),
    ] {
        if let Some(comp) = comp {
            out.push(SoundMark {
                at: comp.onset,
                label,
            });
        }
    }
    out
}

/// A precomputed atom: chirp coefficients and envelope normalisation.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Atom {
    p: AtomParams,
    /// `f(τ) = a + b / √τ`, Hz, τ in ms.
    a: f64,
    b: f64,
    /// 1 / peak of the raw envelope.
    env_norm: f64,
}

impl Atom {
    fn new(p: AtomParams) -> Self {
        let d = p.duration_ms.max(1.0 + 1e-6);
        // f(1 ms) = f_start, f(d) = f_end
        let b = (p.f_start_hz - p.f_end_hz) / (1.0 - 1.0 / d.sqrt());
        let a = p.f_start_hz - b;
        let mut atom = Self {
            p: AtomParams { duration_ms: d, ..p },
            a,
            b,
            env_norm: 1.0,
        };
        let steps = 2000;
        let peak = (0..=steps)
            .map(|i| atom.envelope(d * f64::from(i) / f64::from(steps)))
            .fold(0.0, f64::max);
        atom.env_norm = if peak > 0.0 { 1.0 / peak } else { 0.0 };
        atom
    }

    fn envelope(&self, tau: f64) -> f64 {
        let p = &self.p;
        let rise = if p.rise_ms > 0.0 {
            1.0 - (-tau / p.rise_ms).exp_d()
        } else {
            1.0
        };
        let decay = if p.decay_ms > 0.0 {
            (-tau / p.decay_ms).exp_d()
        } else {
            1.0
        };
        rise * decay * (std::f64::consts::PI * tau / p.duration_ms).sin_d()
    }

    /// Value at `tau` ms after onset (0 outside the atom).
    fn value(&self, tau: f64) -> f64 {
        if !(0.0..self.p.duration_ms).contains(&tau) {
            return 0.0;
        }
        let phase = std::f64::consts::TAU * (self.a * tau + 2.0 * self.b * tau.sqrt()) / 1000.0;
        self.envelope(tau) * self.env_norm * phase.sin_d()
    }

    /// Instantaneous frequency `a + b / √τ` at `tau` ms, Hz; `f_start_hz` before 1 ms, where the
    /// square-root law starts (it is unbounded at 0).
    fn frequency_hz(&self, tau: f64) -> f64 {
        if tau < 1.0 {
            self.p.f_start_hz
        } else {
            self.a + self.b / tau.sqrt()
        }
    }

    /// [`Atom::value`] weighted by a chest piece at the instantaneous frequency (brief B27 part B):
    /// still closed-form, no filter state.
    fn value_weighted(&self, tau: f64, chest: &ChestWeight) -> f64 {
        let v = self.value(tau);
        if v == 0.0 {
            return 0.0;
        }
        v * chest.weight(self.frequency_hz(tau))
    }

    /// The atom's value at `tau` with or without a chest piece (`None`: exactly [`Atom::value`]).
    fn value_with(&self, tau: f64, chest: Option<&ChestWeight>) -> f64 {
        match chest {
            None => self.value(tau),
            Some(w) => self.value_weighted(tau, w),
        }
    }

    /// [`Atom::value_with`] with every frequency multiplied by `factor` (small-speaker mode, B28):
    /// the same envelope, the phase run `factor` times faster. The chest piece weighs the true
    /// frequency, as the stethoscope hears it before the sound is shifted. `factor == 1` is
    /// [`Atom::value_with`] itself.
    fn value_scaled(&self, tau: f64, chest: Option<&ChestWeight>, factor: f64) -> f64 {
        if factor.to_bits() == 1.0f64.to_bits() {
            return self.value_with(tau, chest);
        }
        if !(0.0..self.p.duration_ms).contains(&tau) {
            return 0.0;
        }
        let phase = std::f64::consts::TAU * factor * (self.a * tau + 2.0 * self.b * tau.sqrt()) / 1000.0;
        let v = self.envelope(tau) * self.env_norm * phase.sin_d();
        match chest {
            None => v,
            Some(w) => v * w.weight(self.frequency_hz(tau)),
        }
    }
}

/// A murmur's sum of sinusoids `tau_ms` after its start, with or without a chest piece (`None`:
/// exactly [`Murmur::noise`]), every partial's frequency multiplied by `factor` (1: unchanged; see
/// [`SMALL_SPEAKER_FACTOR`]).
fn murmur_noise(m: &Murmur, tau_ms: f64, chest: Option<&ChestWeight>, factor: f64) -> f64 {
    if factor.to_bits() != 1.0f64.to_bits() {
        return m.noise_scaled(tau_ms, factor, SCALED_CEILING_HZ, |hz| {
            chest.map_or(1.0, |w| w.weight(hz))
        });
    }
    match chest {
        None => m.noise(tau_ms),
        Some(w) => m.noise_weighted(tau_ms, |hz| w.weight(hz)),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Active {
    atom: usize,
    onset_ms: f64,
    gain: f64,
    /// QRS onset of the beat it belongs to (for a biventricular revision), or the atrial activation
    /// of an S4.
    beat_ms: f64,
    /// Scheduled from an atrial activation (S4).
    atrial: bool,
}

/// A discrete sound as scheduled (kept with [`PcgSynth::keep_beats`] for tests and recordings).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ScheduledSound {
    pub kind: ComponentKind,
    pub onset_ms: f64,
    /// Gain with the seeded jitter.
    pub gain: f64,
    /// QRS onset (engine time) of its beat, or the atrial activation of an S4.
    pub key_ms: f64,
}

/// One discrete sound handed to a synthesiser by the [`Scheduler`].
#[derive(Debug, Clone, Copy, PartialEq)]
struct AtomStart {
    atom: usize,
    onset_ms: f64,
    gain: f64,
    key_ms: f64,
    atrial: bool,
}

/// What one event changes for a synthesiser.
#[derive(Debug, Clone, Default)]
struct Update {
    /// The beat (and its components) for a ventricular activation.
    beat: Option<(Beat, Components)>,
    atoms: Vec<AtomStart>,
    murmurs: Vec<Murmur>,
    /// Labels with their key time (the beat's QRS, or the atrial activation of an S4).
    marks: Vec<(f64, SoundMark)>,
    /// A new ventricular event at this time: murmurs still sounding fade out from the cut time
    /// (`.1`), and atrial sounds that would start after `.0` are cancelled.
    cut: Option<(f64, f64)>,
}

/// The state both synthesisers share: the beat assembler, the scenario's auscultation section and
/// what the extra sounds need to remember (the running mean RR, the last beat's A2). Fed the same
/// events, two schedulers give the same updates.
#[derive(Debug, Clone)]
struct Scheduler {
    params: PcgParams,
    seed: u64,
    respiration_rate_bpm: f64,
    /// The engine's respiratory clock (B28 breath-holds); the plain function of time until set.
    respiration: RespiratoryClock,
    assembler: BeatAssembler,
    model: Auscultation,
    valve: ValveChanges,
    /// Running mean RR before the current beat, ms.
    rr_mean_ms: Option<f64>,
    /// The last beat: its engine time, its A2 onset if it ejected, whether it was organised.
    last_beat: Option<(f64, Option<f64>, bool)>,
}

/// Salt of the jitter draws of the atrial sounds, so they never share a stream with a beat's.
const ATRIAL_JITTER_SALT: u64 = 0x5851_F42D_4C95_7F2D;

impl Scheduler {
    fn new(params: PcgParams, seed: u64, respiration_rate_bpm: f64) -> Self {
        Self {
            assembler: BeatAssembler::new(&params),
            valve: ValveChanges::default(),
            params,
            seed,
            respiration_rate_bpm,
            respiration: RespiratoryClock::default(),
            model: Auscultation::default(),
            rr_mean_ms: None,
            last_beat: None,
        }
    }

    fn set_model(&mut self, model: Auscultation) {
        self.valve = valve_changes(&model, &self.params.auscultation);
        self.model = model;
    }

    fn push(&mut self, ev: &Event) -> Option<Update> {
        let atrial = if self.model.is_empty() {
            None
        } else {
            self.atrial_update(ev)
        };
        let beat = self.assembler.push(ev);
        match beat {
            Some(beat) => Some(self.beat_update(&beat)),
            None => atrial,
        }
    }

    /// The sounds of an organised atrial activation (S4, presystolic accentuation, atrial rub).
    fn atrial_update(&self, ev: &Event) -> Option<Update> {
        let organised = ev.region.is_atrial()
            && ev.kind != EventKind::Blocked
            && !matches!(ev.origin, Origin::Circuit(c) if !c.is_ventricular());
        if !organised {
            return None;
        }
        let t = ev.at.0;
        let in_diastole = match self.last_beat {
            None => true,
            Some((key, a2, organised)) => {
                organised && t >= a2.unwrap_or(key + self.params.min_ejecting_rr_ms)
            }
        };
        let (s4, murmurs) = atrial_sounds(t, in_diastole, &self.model, &self.params.auscultation, self.seed);
        let mut u = Update {
            murmurs,
            ..Update::default()
        };
        if let Some((onset, gain)) = s4 {
            let mut rng = ShockRng::new(self.seed ^ t.to_bits() ^ ATRIAL_JITTER_SALT);
            let jitter = 1.0 + self.params.amplitude_jitter * (2.0 * rng.next_f64() - 1.0);
            u.atoms.push(AtomStart {
                atom: ComponentKind::S4 as usize,
                onset_ms: onset,
                gain: gain * jitter,
                key_ms: t,
                atrial: true,
            });
            u.marks.push((
                t,
                SoundMark {
                    at: Millis(onset),
                    label: SoundLabel::S4,
                },
            ));
        }
        (!u.atoms.is_empty() || !u.murmurs.is_empty()).then_some(u)
    }

    /// The schedule of a beat and, per atom, its onset (simulation ms) and its gain with the seeded
    /// jitter. One jitter draw per valve-closure slot, used or not, then one per extra slot, so that
    /// every synthesiser built on it gives the same amplitudes for the same scenario seed and beat
    /// time, and a scenario without an `auscultation` section the amplitudes of brief B24.
    fn beat_update(&mut self, beat: &Beat) -> Update {
        let p = &self.params;
        let q = beat.qrs_onset.0;
        let phase = self.respiration.phase(self.respiration_rate_bpm, q);
        let empty = self.model.is_empty();
        let c = if empty {
            schedule(beat, p, phase)
        } else {
            apply_auscultation(schedule(beat, p, phase), beat, p, &self.model, &self.valve)
        };
        let mut rng = ShockRng::new(self.seed ^ q.to_bits());
        let j = p.amplitude_jitter;
        let mut atoms = Vec::with_capacity(4);
        let mut slots = |rng: &mut ShockRng, list: &[(ComponentKind, Option<Component>)]| {
            for (kind, comp) in list {
                let jitter = 1.0 + j * (2.0 * rng.next_f64() - 1.0);
                if let Some(comp) = comp {
                    atoms.push(AtomStart {
                        atom: *kind as usize,
                        onset_ms: comp.onset.0,
                        gain: comp.gain * jitter,
                        key_ms: q,
                        atrial: false,
                    });
                }
            }
        };
        slots(
            &mut rng,
            &[
                (ComponentKind::M1, c.m1),
                (ComponentKind::T1, c.t1),
                (ComponentKind::A2, c.a2),
                (ComponentKind::P2, c.p2),
            ],
        );
        let mut u = Update::default();
        if !empty {
            slots(
                &mut rng,
                &[
                    (ComponentKind::S3, c.s3),
                    (ComponentKind::OpeningSnap, c.opening_snap),
                    (ComponentKind::AorticClick, c.aortic_click),
                    (ComponentKind::PulmonaryClick, c.pulmonary_click),
                    (ComponentKind::MidSystolicClick, c.mid_systolic_click),
                ],
            );
            if !beat.replaces_previous {
                u.cut = Some((q, c.m1.map_or(q, |m| m.onset.0.max(q))));
            }
            let rr = beat.rr_ms.filter(|rr| *rr > 0.0);
            let cycle_ratio = match (rr, self.rr_mean_ms) {
                (Some(rr), Some(mean)) if mean > 0.0 => rr / mean,
                _ => 1.0,
            };
            let ctx = BeatContext {
                q_ms: q - p.qrs_onset_lead_ms,
                key_ms: q,
                cycle_ratio,
                expected_cycle_ms: rr.unwrap_or(60_000.0 / p.first_beat_bpm),
                inspiration: inspiration_depth(phase),
            };
            u.murmurs = beat_murmurs(&c, &self.model, &p.auscultation, &ctx, self.seed);
            if !beat.replaces_previous
                && let Some(rr) = rr
            {
                let w = p.auscultation.rr_mean_weight;
                self.rr_mean_ms = Some(self.rr_mean_ms.map_or(rr, |m| m + w * (rr - m)));
            }
            self.last_beat = Some((q, c.a2.map(|a| a.onset.0), beat.organised));
        }
        u.atoms = atoms;
        u.marks = labels(&c).into_iter().map(|m| (q, m)).collect();
        u.beat = Some((*beat, c));
        u
    }
}

/// Event-driven phonocardiogram synthesiser, the counterpart of `GaussianSynth`: feed it the
/// engine's events with [`PcgSynth::on_events`], then read [`PcgSynth::sample`] at increasing times.
/// The output is in arbitrary units with a normal S1 (gain 1) peaking at 1.0.
#[derive(Debug, Clone)]
pub struct PcgSynth {
    sched: Scheduler,
    /// M1, T1, A2, P2, then the extra sounds, in [`ComponentKind::ALL`] order.
    atoms: [Atom; ComponentKind::ALL.len()],
    /// Output scale: a gain-1 S1 (M1 + T1 at the normal gap) peaks at 1.
    scale: f64,
    active: Vec<Active>,
    murmurs: Vec<Murmur>,
    gains: SoundGains,
    /// The chest piece of the listening site (brief B27 part B); `None` at the overview.
    chest: Option<ChestWeight>,
    /// Labels with the QRS onset of their beat, until the beat can no longer be revised.
    marks: Vec<(f64, SoundMark)>,
    beats: Vec<(Beat, Components)>,
    sound_log: Vec<ScheduledSound>,
    murmur_log: Vec<Murmur>,
    keep_beats: bool,
    /// Frequency factor of small-speaker mode (B28); 1 for the trace.
    frequency_factor: f64,
}

impl PcgSynth {
    /// A synthesiser with `params`, the scenario `seed` (the amplitude jitter) and the breathing rate
    /// of the timing engine (`TimingParams::sinus.respiration_rate_bpm`), so the S2 split follows the
    /// same respiratory clock as the sinus arrhythmia.
    #[must_use]
    pub fn new(params: PcgParams, seed: u64, respiration_rate_bpm: f64) -> Self {
        let atoms = atoms_of(&params);
        let scale = reference_scale(&params, &atoms);
        Self {
            sched: Scheduler::new(params, seed, respiration_rate_bpm),
            atoms,
            scale,
            active: Vec::new(),
            murmurs: Vec::new(),
            gains: SoundGains::default(),
            chest: None,
            marks: Vec::new(),
            beats: Vec::new(),
            sound_log: Vec::new(),
            murmur_log: Vec::new(),
            keep_beats: false,
            frequency_factor: 1.0,
        }
    }

    /// Follow the engine's respiratory clock (`heartsim_sim::TimingEngine::respiration`, brief B28):
    /// beats scheduled from now on take their S2 split from it, so a breath-hold freezes the split
    /// with the sinus arrhythmia. A clock that was never held changes nothing.
    pub fn set_respiration(&mut self, clock: RespiratoryClock) {
        self.sched.respiration = clock;
    }

    /// Multiply every frequency by `factor` from the next sample on (small-speaker mode, brief B28,
    /// [`SMALL_SPEAKER_FACTOR`]). Only for a synthesiser whose output is played, never for the one
    /// the lane draws: the trace keeps the true signal. 1 is the unaltered output, bit for bit.
    pub fn set_frequency_factor(&mut self, factor: f64) {
        self.frequency_factor = if factor.is_finite() && factor > 0.0 {
            factor
        } else {
            1.0
        };
    }

    /// The same synthesiser with a scenario's `auscultation` section (brief B27). An empty section
    /// changes nothing.
    #[must_use]
    pub fn with_auscultation(mut self, model: &Auscultation) -> Self {
        self.sched.set_model(model.clone());
        self
    }

    /// Per-class gains applied when sampling (all 1: the overview mix, bit-identical output).
    pub fn set_gains(&mut self, gains: SoundGains) {
        self.gains = gains;
    }

    /// A listening site and chest piece (brief B27 part B), from the next sample on: its gains and
    /// its weight curve. [`Listening::overview`] gives the output of brief B24 and part A bit for
    /// bit. Nothing is re-scheduled.
    pub fn set_listening(&mut self, listening: &Listening) {
        self.gains.clone_from(&listening.gains);
        self.chest = listening.chest;
    }

    /// Drop the beats, sounds and murmurs kept with [`PcgSynth::keep_beats`] that are over before
    /// `t_ms` (a live run keeps only what the 3-D view can still draw). A murmur still sounding, or
    /// not yet cut, stays.
    pub fn forget_before(&mut self, t_ms: f64) {
        self.beats
            .retain(|(b, _)| b.qrs_onset.0 + 2.0 * self.sched.params.same_beat_ms >= t_ms);
        self.sound_log
            .retain(|s| s.onset_ms + crate::listening::VALVE_FLASH_MS >= t_ms);
        self.murmur_log.retain(|m| m.stop_ms() >= t_ms);
    }

    /// The model's parameters.
    #[must_use]
    pub fn params(&self) -> &PcgParams {
        &self.sched.params
    }

    /// Keep every beat and its schedule for [`PcgSynth::beats`] (tests and offline recordings).
    pub fn keep_beats(&mut self, keep: bool) {
        self.keep_beats = keep;
    }

    /// The beats seen so far with their components (only with [`PcgSynth::keep_beats`]).
    #[must_use]
    pub fn beats(&self) -> &[(Beat, Components)] {
        &self.beats
    }

    /// Every discrete sound scheduled so far, S4 included (only with [`PcgSynth::keep_beats`]).
    #[must_use]
    pub fn scheduled_sounds(&self) -> &[ScheduledSound] {
        &self.sound_log
    }

    /// Every murmur scheduled so far, with its cut once known (only with [`PcgSynth::keep_beats`]).
    #[must_use]
    pub fn scheduled_murmurs(&self) -> &[Murmur] {
        &self.murmur_log
    }

    /// Feed timing-engine events (in engine order).
    pub fn on_events(&mut self, events: &[Event]) {
        for ev in events {
            if let Some(u) = self.sched.push(ev) {
                self.apply(u);
            }
        }
    }

    fn apply(&mut self, u: Update) {
        if let Some((beat, _)) = &u.beat
            && beat.replaces_previous
        {
            let q = beat.qrs_onset.0;
            let same = |k: f64| k.to_bits() == q.to_bits();
            self.active.retain(|a| a.atrial || !same(a.beat_ms));
            self.murmurs.retain(|m| m.atrial || !same(m.key_ms));
            self.marks.retain(|(b, _)| !same(*b));
            if self.keep_beats {
                if let Some(last) = self.beats.last()
                    && same(last.0.qrs_onset.0)
                {
                    self.beats.pop();
                }
                self.sound_log
                    .retain(|s| !same(s.key_ms) || s.kind == ComponentKind::S4);
                self.murmur_log.retain(|m| m.atrial || !same(m.key_ms));
            }
        }
        if let Some((event_ms, cut_ms)) = u.cut {
            let late = |onset: f64, atrial: bool| atrial && onset > event_ms;
            self.active.retain(|a| !late(a.onset_ms, a.atrial));
            self.murmurs.retain(|m| !late(m.start_ms, m.atrial));
            self.marks
                .retain(|(_, m)| !(m.label == SoundLabel::S4 && m.at.0 > event_ms));
            for m in &mut self.murmurs {
                m.cut(cut_ms);
            }
            if self.keep_beats {
                self.sound_log
                    .retain(|s| !(s.kind == ComponentKind::S4 && s.onset_ms > event_ms));
                self.murmur_log.retain(|m| !late(m.start_ms, m.atrial));
                for m in &mut self.murmur_log {
                    m.cut(cut_ms);
                }
            }
        }
        for a in &u.atoms {
            self.active.push(Active {
                atom: a.atom,
                onset_ms: a.onset_ms,
                gain: a.gain,
                beat_ms: a.key_ms,
                atrial: a.atrial,
            });
        }
        self.marks.extend(u.marks);
        if self.keep_beats {
            self.sound_log.extend(u.atoms.iter().map(|a| ScheduledSound {
                kind: ComponentKind::ALL[a.atom],
                onset_ms: a.onset_ms,
                gain: a.gain,
                key_ms: a.key_ms,
            }));
            self.murmur_log.extend(u.murmurs.iter().cloned());
            if let Some(b) = u.beat {
                self.beats.push(b);
            }
        }
        self.murmurs.extend(u.murmurs);
    }

    /// The trace at time `t`. Components that have ended are dropped, so read at increasing times.
    pub fn sample(&mut self, t: Millis) -> f32 {
        let atoms = &self.atoms;
        let gains = &self.gains;
        let chest = self.chest.as_ref();
        let k = self.frequency_factor;
        let mut v = 0.0;
        self.active.retain(|a| {
            let tau = t.0 - a.onset_ms;
            let atom = &atoms[a.atom];
            if tau >= atom.p.duration_ms {
                return false;
            }
            v += a.gain * gains.components[a.atom] * atom.value_scaled(tau, chest, k);
            true
        });
        let mut out = v * self.scale;
        if !self.murmurs.is_empty() {
            let mut m_sum = 0.0;
            self.murmurs.retain(|m| {
                if m.finished_at(t.0) {
                    return false;
                }
                let level = m.level_at(t.0);
                if level != 0.0 {
                    m_sum +=
                        gains.murmurs[m.class.index()] * level * murmur_noise(m, t.0 - m.start_ms, chest, k);
                }
                true
            });
            out += m_sum;
        }
        out as f32
    }

    /// Labels of the beats that started at least [`PcgParams::same_beat_ms`] before `now` (a later
    /// front can no longer change them), in schedule order; they are handed out once. Labels lie in
    /// the future of their beat's QRS, so a caller drawing at `now` misses none.
    pub fn take_settled_marks(&mut self, now: Millis) -> Vec<SoundMark> {
        let settle = self.sched.params.same_beat_ms;
        let n = self.marks.iter().take_while(|(b, _)| b + settle <= now.0).count();
        self.marks.drain(..n).map(|(_, m)| m).collect()
    }

    /// Every label not handed out yet, settled or not.
    pub fn take_all_marks(&mut self) -> Vec<SoundMark> {
        self.marks.drain(..).map(|(_, m)| m).collect()
    }

    /// Components and murmurs scheduled and not yet finished.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.active.len() + self.murmurs.len()
    }
}

/// The atoms of a parameter set, in [`ComponentKind::ALL`] order.
fn atoms_of(params: &PcgParams) -> [Atom; ComponentKind::ALL.len()] {
    ComponentKind::ALL.map(|k| Atom::new(k.atom(params)))
}

/// Output scale such that a gain-1 S1 (M1 and T1 at the normal gap, sampled on the output grid)
/// peaks at 1.
fn reference_scale(params: &PcgParams, atoms: &[Atom]) -> f64 {
    let gap = params.m1_t1_ms;
    let t1 = params.t1_gain;
    let len = params.m1.duration_ms.max(gap + params.t1.duration_ms);
    let n = (len / PCG_PERIOD_MS).ceil() as usize;
    let peak = (0..n)
        .map(|k| {
            let tau = k as f64 * PCG_PERIOD_MS;
            (atoms[0].value(tau) + t1 * atoms[1].value(tau - gap)).abs()
        })
        .fold(0.0, f64::max);
    if peak > 0.0 { 1.0 / peak } else { 0.0 }
}

// --- slow-motion listening (brief B25) -------------------------------------------------------------

/// Which discrete heart sound a component is: the valve closures of brief B24, then the extra
/// sounds of brief B27.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ComponentKind {
    M1,
    T1,
    A2,
    P2,
    S3,
    S4,
    OpeningSnap,
    AorticClick,
    PulmonaryClick,
    MidSystolicClick,
}

impl ComponentKind {
    /// In the order of [`Components`] (S4 inserted after S3) and of the synthesisers' atoms.
    pub const ALL: [ComponentKind; 10] = [
        ComponentKind::M1,
        ComponentKind::T1,
        ComponentKind::A2,
        ComponentKind::P2,
        ComponentKind::S3,
        ComponentKind::S4,
        ComponentKind::OpeningSnap,
        ComponentKind::AorticClick,
        ComponentKind::PulmonaryClick,
        ComponentKind::MidSystolicClick,
    ];

    /// The atom parameters of this component in `params`.
    #[must_use]
    pub fn atom(self, params: &PcgParams) -> AtomParams {
        let a = &params.auscultation;
        match self {
            ComponentKind::M1 => params.m1,
            ComponentKind::T1 => params.t1,
            ComponentKind::A2 => params.a2,
            ComponentKind::P2 => params.p2,
            ComponentKind::S3 => a.s3,
            ComponentKind::S4 => a.s4,
            ComponentKind::OpeningSnap => a.snap,
            ComponentKind::AorticClick | ComponentKind::PulmonaryClick => a.click,
            ComponentKind::MidSystolicClick => a.mid_click,
        }
    }
}

/// A component as the wall-time synthesiser started it: its simulation onset and where that
/// landed in wall time.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WallStart {
    pub kind: ComponentKind,
    /// Onset in simulation time, ms (from [`schedule`]).
    pub sim_ms: f64,
    /// Onset in the synthesiser's wall time, ms.
    pub wall_ms: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct WallComponent {
    atom: usize,
    onset_sim_ms: f64,
    gain: f64,
    beat_ms: f64,
    atrial: bool,
    /// Fixed when the component starts: later speed changes no longer move it.
    wall_onset_ms: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
struct WallMurmur {
    m: Murmur,
    /// Wall time of the murmur's start, fixed when it starts: its partials run from there at their
    /// own frequencies while its envelope follows simulation time.
    wall_onset_ms: Option<f64>,
}

/// The phonocardiogram re-timed for listening in slow motion (brief B25).
///
/// Fed the same events as [`PcgSynth`], it schedules the same components ([`schedule`], the same
/// seeded jitter, the same handling of [`Beat::replaces_previous`]), but renders them in *wall*
/// time: a component whose onset is at simulation time `s` starts at wall time
/// `anchor_wall + (s − anchor_sim) / speed`, and then plays its atom at its natural frequency and
/// duration. Slowing the simulation down therefore stretches the intervals between the sounds (an
/// A2–P2 split of 40 ms at 0.25× is heard as 160 ms) while each sound keeps its pitch: no
/// resampling, no vocoder. A speed change moves the anchor to the sample being rendered, so it
/// takes effect for the components that have not started yet.
///
/// A murmur (brief B27) keeps its band too: its partials run in wall time from its wall start,
/// while its envelope is read at the simulation time the wall sample stands for, so in slow motion
/// it stretches with the interval it fills.
///
/// At speed 1 with the initial anchor (0, 0) the output equals [`PcgSynth::sample`] on the same
/// grid sample for sample: the mapping is the identity and the arithmetic is the same. The drawn
/// trace keeps showing the true signal; only the sound is re-timed.
///
/// Wall sample `k` is at `k · PCG_PERIOD_MS`; output is in the units of [`PcgSynth`]. Pure: no
/// clock is read, the caller says how far simulation time has advanced.
#[derive(Debug, Clone)]
pub struct WallTimeSynth {
    sched: Scheduler,
    atoms: [Atom; ComponentKind::ALL.len()],
    scale: f64,
    components: Vec<WallComponent>,
    murmurs: Vec<WallMurmur>,
    gains: SoundGains,
    chest: Option<ChestWeight>,
    anchor_sim_ms: f64,
    anchor_wall_ms: f64,
    speed: f64,
    /// Wall samples rendered (or skipped) so far.
    rendered: u64,
    starts: Option<Vec<WallStart>>,
    /// Frequency factor of small-speaker mode (B28).
    frequency_factor: f64,
}

impl WallTimeSynth {
    /// A synthesiser like [`PcgSynth::new`], at speed 1 with simulation and wall time both at 0.
    #[must_use]
    pub fn new(params: PcgParams, seed: u64, respiration_rate_bpm: f64) -> Self {
        let atoms = atoms_of(&params);
        let scale = reference_scale(&params, &atoms);
        Self {
            sched: Scheduler::new(params, seed, respiration_rate_bpm),
            atoms,
            scale,
            components: Vec::new(),
            murmurs: Vec::new(),
            gains: SoundGains::default(),
            chest: None,
            anchor_sim_ms: 0.0,
            anchor_wall_ms: 0.0,
            speed: 1.0,
            rendered: 0,
            starts: None,
            frequency_factor: 1.0,
        }
    }

    /// Follow the engine's respiratory clock, as [`PcgSynth::set_respiration`].
    pub fn set_respiration(&mut self, clock: RespiratoryClock) {
        self.sched.respiration = clock;
    }

    /// Multiply every frequency by `factor` (small-speaker mode), as
    /// [`PcgSynth::set_frequency_factor`].
    pub fn set_frequency_factor(&mut self, factor: f64) {
        self.frequency_factor = if factor.is_finite() && factor > 0.0 {
            factor
        } else {
            1.0
        };
    }

    /// The same synthesiser with a scenario's `auscultation` section, as
    /// [`PcgSynth::with_auscultation`].
    #[must_use]
    pub fn with_auscultation(mut self, model: &Auscultation) -> Self {
        self.sched.set_model(model.clone());
        self
    }

    /// Per-class gains, as [`PcgSynth::set_gains`].
    pub fn set_gains(&mut self, gains: SoundGains) {
        self.gains = gains;
    }

    /// A listening site and chest piece, as [`PcgSynth::set_listening`].
    pub fn set_listening(&mut self, listening: &Listening) {
        self.gains.clone_from(&listening.gains);
        self.chest = listening.chest;
    }

    /// Feed timing-engine events (in engine order), as [`PcgSynth::on_events`].
    pub fn on_events(&mut self, events: &[Event]) {
        for ev in events {
            if let Some(u) = self.sched.push(ev) {
                self.apply(u);
            }
        }
    }

    fn apply(&mut self, u: Update) {
        if let Some((beat, _)) = &u.beat
            && beat.replaces_previous
        {
            let q = beat.qrs_onset.0;
            let same = |k: f64| k.to_bits() == q.to_bits();
            self.components.retain(|c| c.atrial || !same(c.beat_ms));
            self.murmurs.retain(|w| w.m.atrial || !same(w.m.key_ms));
        }
        if let Some((event_ms, cut_ms)) = u.cut {
            // only what has not started can be cancelled: its onset lies after the event
            self.components
                .retain(|c| !(c.atrial && c.wall_onset_ms.is_none() && c.onset_sim_ms > event_ms));
            self.murmurs
                .retain(|w| !(w.m.atrial && w.wall_onset_ms.is_none() && w.m.start_ms > event_ms));
            for w in &mut self.murmurs {
                w.m.cut(cut_ms);
            }
        }
        for a in u.atoms {
            self.components.push(WallComponent {
                atom: a.atom,
                onset_sim_ms: a.onset_ms,
                gain: a.gain,
                beat_ms: a.key_ms,
                atrial: a.atrial,
                wall_onset_ms: None,
            });
        }
        self.murmurs.extend(u.murmurs.into_iter().map(|m| WallMurmur {
            m,
            wall_onset_ms: None,
        }));
    }

    /// Simulation speed the mapping uses (wall ms per simulation ms is `1 / speed`).
    #[must_use]
    pub fn speed(&self) -> f64 {
        self.speed
    }

    /// Change the speed (clamped to at least 0.001) from the sample being rendered on: the anchor
    /// moves there, so the components already started keep their wall onsets and the others are
    /// placed with the new speed. Setting the same speed changes nothing.
    pub fn set_speed(&mut self, speed: f64) {
        let speed = speed.max(1e-3);
        if speed.to_bits() == self.speed.to_bits() {
            return;
        }
        let w = self.wall_now_ms();
        self.anchor_sim_ms = self.sim_at_wall(w);
        self.anchor_wall_ms = w;
        self.speed = speed;
    }

    /// Wall time of the last sample rendered, ms.
    #[must_use]
    pub fn wall_now_ms(&self) -> f64 {
        self.rendered as f64 * PCG_PERIOD_MS
    }

    /// Simulation time that wall time `wall_ms` stands for under the current mapping, ms.
    #[must_use]
    pub fn sim_at_wall(&self, wall_ms: f64) -> f64 {
        self.anchor_sim_ms + (wall_ms - self.anchor_wall_ms) * self.speed
    }

    /// Wall time at which simulation time `sim_ms` is rendered under the current mapping, ms.
    #[must_use]
    pub fn wall_at_sim(&self, sim_ms: f64) -> f64 {
        self.anchor_wall_ms + (sim_ms - self.anchor_sim_ms) / self.speed
    }

    /// Append the wall samples up to the one that stands for simulation time `sim_ms` (the engine's
    /// clock after this frame's steps) to `out`. Every component it can contain is known by then: a
    /// sound never starts before the event that schedules it.
    pub fn render_until_sim(&mut self, sim_ms: f64, out: &mut Vec<f32>) {
        let end = self.wall_at_sim(sim_ms);
        while (self.rendered + 1) as f64 * PCG_PERIOD_MS <= end {
            self.rendered += 1;
            let w = self.rendered as f64 * PCG_PERIOD_MS;
            out.push(self.sample_at(w));
        }
    }

    fn sample_at(&mut self, w: f64) -> f32 {
        let (anchor_wall, anchor_sim, speed) = (self.anchor_wall_ms, self.anchor_sim_ms, self.speed);
        let atoms = &self.atoms;
        let gains = &self.gains;
        let chest = self.chest.as_ref();
        let starts = &mut self.starts;
        let k = self.frequency_factor;
        let mut v = 0.0;
        self.components.retain_mut(|c| {
            let onset = c
                .wall_onset_ms
                .unwrap_or_else(|| anchor_wall + (c.onset_sim_ms - anchor_sim) / speed);
            let tau = w - onset;
            let atom = &atoms[c.atom];
            if tau >= atom.p.duration_ms {
                return false;
            }
            if tau >= 0.0 && c.wall_onset_ms.is_none() {
                c.wall_onset_ms = Some(onset);
                if let Some(log) = starts {
                    log.push(WallStart {
                        kind: ComponentKind::ALL[c.atom],
                        sim_ms: c.onset_sim_ms,
                        wall_ms: onset,
                    });
                }
            }
            v += c.gain * gains.components[c.atom] * atom.value_scaled(tau, chest, k);
            true
        });
        let mut out = v * self.scale;
        if !self.murmurs.is_empty() {
            let s = anchor_sim + (w - anchor_wall) * speed;
            let mut m_sum = 0.0;
            self.murmurs.retain_mut(|wm| {
                if wm.m.finished_at(s) {
                    return false;
                }
                if wm.wall_onset_ms.is_none() && s >= wm.m.start_ms {
                    wm.wall_onset_ms = Some(anchor_wall + (wm.m.start_ms - anchor_sim) / speed);
                }
                let level = wm.m.level_at(s);
                if level != 0.0
                    && let Some(onset) = wm.wall_onset_ms
                {
                    m_sum +=
                        gains.murmurs[wm.m.class.index()] * level * murmur_noise(&wm.m, w - onset, chest, k);
                }
                true
            });
            out += m_sum;
        }
        out as f32
    }

    /// Keep up with the simulation without producing sound (live 1× playback from the ring, or
    /// heart sounds muted above 1×): the mapping becomes speed 1 from the current sample and wall
    /// time advances to `sim_ms`, starting and retiring components as rendering would. A later
    /// switch to slow motion then continues the sounds in progress at their natural rate.
    pub fn follow(&mut self, sim_ms: f64) {
        self.set_speed(1.0);
        let end = self.wall_at_sim(sim_ms);
        if (self.rendered + 1) as f64 * PCG_PERIOD_MS > end {
            return;
        }
        self.rendered = (end / PCG_PERIOD_MS).floor().max(0.0) as u64;
        let w = self.wall_now_ms();
        let (anchor_wall, anchor_sim, speed) = (self.anchor_wall_ms, self.anchor_sim_ms, self.speed);
        let atoms = &self.atoms;
        let starts = &mut self.starts;
        self.components.retain_mut(|c| {
            let onset = c
                .wall_onset_ms
                .unwrap_or_else(|| anchor_wall + (c.onset_sim_ms - anchor_sim) / speed);
            if w - onset >= atoms[c.atom].p.duration_ms {
                return false;
            }
            if w >= onset && c.wall_onset_ms.is_none() {
                c.wall_onset_ms = Some(onset);
                if let Some(log) = starts {
                    log.push(WallStart {
                        kind: ComponentKind::ALL[c.atom],
                        sim_ms: c.onset_sim_ms,
                        wall_ms: onset,
                    });
                }
            }
            true
        });
        let s = anchor_sim + (w - anchor_wall) * speed;
        self.murmurs.retain_mut(|wm| {
            if wm.m.finished_at(s) {
                return false;
            }
            if wm.wall_onset_ms.is_none() && s >= wm.m.start_ms {
                wm.wall_onset_ms = Some(anchor_wall + (wm.m.start_ms - anchor_sim) / speed);
            }
            true
        });
    }

    /// Record every component start for [`WallTimeSynth::take_starts`] (tests).
    pub fn record_starts(&mut self, on: bool) {
        self.starts = on.then(Vec::new);
    }

    /// The starts recorded since the last call.
    pub fn take_starts(&mut self) -> Vec<WallStart> {
        self.starts.as_mut().map(std::mem::take).unwrap_or_default()
    }

    /// Components and murmurs scheduled and not yet finished.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.components.len() + self.murmurs.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atoms_follow_their_frequency_trajectory_and_peak_at_one() {
        let p = PcgParams::default();
        let a2 = Atom::new(p.a2);
        assert!((a2.a + a2.b - 250.0).abs() < 1e-9);
        assert!((a2.a + a2.b / 60f64.sqrt() - 53.0).abs() < 1e-9);
        let peak = (0..2400)
            .map(|i| a2.envelope(f64::from(i) * 0.025) * a2.env_norm)
            .fold(0.0, f64::max);
        assert!((peak - 1.0).abs() < 1e-3, "{peak}");
        assert_eq!(a2.value(-0.1), 0.0);
        assert_eq!(a2.value(60.0), 0.0);
    }

    #[test]
    fn a_normal_s1_peaks_at_one() {
        let mut s = PcgSynth::new(PcgParams::default(), 0, 12.0);
        s.active.push(Active {
            atom: 0,
            onset_ms: 0.0,
            gain: 1.0,
            beat_ms: 0.0,
            atrial: false,
        });
        s.active.push(Active {
            atom: 1,
            onset_ms: 25.0,
            gain: 0.6,
            beat_ms: 0.0,
            atrial: false,
        });
        let peak = (0..600)
            .map(|k| s.sample(Millis(f64::from(k) * PCG_PERIOD_MS)).abs())
            .fold(0.0, f32::max);
        assert!((peak - 1.0).abs() < 1e-6, "{peak}");
        assert_eq!(s.active_count(), 0, "finished components are dropped");
    }

    #[test]
    fn qs2_follows_weissler() {
        let p = PcgParams::default();
        assert!((qs2_ms(60.0, &p) - 420.0).abs() < 1e-9);
        assert!((qs2_ms(100.0, &p) - 336.0).abs() < 1e-9);
        assert!((qs2_ms(250.0, &p) - qs2_ms(180.0, &p)).abs() < 1e-9, "clamped");
    }
}
