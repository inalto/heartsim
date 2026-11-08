//! Defibrillation and synchronised cardioversion (brief B14).
//!
//! `Action::Shock { joules, synchronised }` reaches the timing engine, which owns the shock:
//!
//! * **Unsynchronised** (defibrillation): delivered at the action's time.
//! * **Synchronised** (cardioversion): armed, then delivered [`SYNC_DELAY_MS`] after the next
//!   ventricular activation — the R wave, which the timing engine knows exactly (the same beats
//!   the B4 QRS detector finds on the strip). With no R wave (asystole, VF) it never fires, as a
//!   real defibrillator in sync mode refuses to. Polymorphic VT and torsades are treated the same
//!   way: their QRS changes from beat to beat, so a synchroniser has no reliable R wave and these
//!   rhythms are shocked unsynchronised.
//!
//! The outcome depends on the rhythm class at delivery and the energy:
//! `p(E) = p_max · (1 − exp(−E / E_c))` ([`success_probability`]), drawn from the engine's
//! seeded generator so a scenario replays identically. Biphasic first-shock efficacy: VF
//! 85–94 % at 150–200 J (van Alem et al. 2003; Morrison et al. 2005), monomorphic VT ≥ 90 % at
//! 100 J, AF 80–90 % at 120–200 J (Mittal et al. 2000), flutter and SVT ≥ 90 % at 50–100 J
//! (ERC 2021 / AHA 2020 cardioversion energies). A successful shock depolarises every excitable
//! cell at once: wavefronts die, re-entry and (Tier-1 stub) ectopic tachycardia stop, and the
//! sinus node takes over. An organised rhythm is reset the same way; asystole is not shockable.
//!
//! An unsynchronised shock that lands in the ventricular vulnerable period (the T wave) is
//! flagged ([`ShockRecord::vulnerable`]); with Tier 2 active (B16/B17) that is what induces VF.

use heartsim_core::Millis;
use heartsim_core::math::Deterministic;
use serde::{Deserialize, Serialize};

/// Delay from the R wave to a synchronised shock, ms (devices deliver within ≈ 60 ms of the
/// R peak, i.e. inside the QRS and far from the T wave).
pub const SYNC_DELAY_MS: f64 = 20.0;
/// Vulnerable period relative to the last ventricular activation, as fractions of the
/// ventricular ERP: the T wave, ≈ 200–360 ms after the R wave at the default ERP of 280 ms.
pub const VULNERABLE_WINDOW: (f64, f64) = (0.7, 1.3);
/// Charge time per joule, ms (≈ 5 s to 200 J; IEC 60601-2-4 requires ≤ 15 s to maximum).
pub const CHARGE_MS_PER_J: f64 = 25.0;
/// Energy steps a biphasic defibrillator offers, J.
pub const ENERGY_STEPS_J: [f64; 9] = [5.0, 10.0, 20.0, 50.0, 70.0, 100.0, 120.0, 150.0, 200.0];

/// What the ventricles (and atria) are doing when the shock arrives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RhythmClass {
    /// Ventricular fibrillation (Tier 2 only).
    VentricularFibrillation,
    /// Monomorphic ventricular tachycardia (ventricular origin, rate > 100 bpm).
    VentricularTachycardia,
    /// Polymorphic ventricular tachycardia or torsades de pointes: treated like VF.
    PolymorphicVentricularTachycardia,
    /// Re-entrant or ectopic supraventricular tachycardia (rate > 100 bpm).
    SupraventricularTachycardia,
    /// Atrial fibrillation or flutter (atrial rate > 250 bpm) when the two are not told apart.
    AtrialFibrillationOrFlutter,
    /// Atrial flutter from the timing engine's own circuit: an organised macro-re-entry that
    /// needs less energy than AF.
    AtrialFlutter,
    /// Sinus, bradycardia, paced or escape rhythm: nothing to terminate.
    Organised,
    /// No ventricular activity: not shockable.
    Asystole,
}

impl RhythmClass {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            RhythmClass::VentricularFibrillation => "VF",
            RhythmClass::VentricularTachycardia => "VT",
            RhythmClass::PolymorphicVentricularTachycardia => "polymorphic VT",
            RhythmClass::SupraventricularTachycardia => "SVT",
            RhythmClass::AtrialFibrillationOrFlutter => "AF / flutter",
            RhythmClass::AtrialFlutter => "flutter",
            RhythmClass::Organised => "organised rhythm",
            RhythmClass::Asystole => "asystole",
        }
    }

    /// `(p_max, E_c J)` of [`success_probability`].
    fn efficacy(self) -> (f64, f64) {
        match self {
            RhythmClass::VentricularFibrillation | RhythmClass::PolymorphicVentricularTachycardia => {
                (0.95, 60.0)
            }
            RhythmClass::VentricularTachycardia => (0.97, 30.0),
            RhythmClass::SupraventricularTachycardia | RhythmClass::AtrialFlutter => (0.97, 25.0),
            RhythmClass::AtrialFibrillationOrFlutter => (0.92, 60.0),
            RhythmClass::Organised => (1.0, 0.0),
            RhythmClass::Asystole => (0.0, 1.0),
        }
    }
}

/// Probability that a shock of `joules` terminates `rhythm` (see the module docs). An organised
/// rhythm is always "reset" (probability 1); asystole never responds (0).
#[must_use]
pub fn success_probability(rhythm: RhythmClass, joules: f64) -> f64 {
    let (p_max, e_c) = rhythm.efficacy();
    if e_c <= 0.0 {
        return p_max;
    }
    p_max * (1.0 - (-joules.max(0.0) / e_c).exp_d())
}

/// One delivered shock: the animation / sound hook and the audit trail.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ShockRecord {
    pub at: Millis,
    pub joules: f64,
    pub synchronised: bool,
    /// Rhythm class at delivery.
    pub rhythm: RhythmClass,
    /// The shock depolarised the heart and terminated the arrhythmia.
    pub success: bool,
    /// Delivered in the vulnerable period (T wave) of the last ventricular beat.
    pub vulnerable: bool,
}

/// Deterministic generator for shock outcomes (`SplitMix64`, Steele et al. 2014).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShockRng(u64);

impl ShockRng {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// Uniform in `[0, 1)`.
    pub fn next_f64(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Front-panel state of the defibrillator.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum DefibState {
    Idle,
    /// Charging; ready at `until_ms`.
    Charging {
        until_ms: f64,
    },
    /// Charged to the selected energy.
    Ready,
}

/// The defibrillator a learner operates (charge → shock). It only produces the
/// `Action::Shock`; synchronisation and outcome happen in the timing engine, so a timeline or
/// tutorial that issues the action directly behaves the same. Time comes from the caller
/// (simulation time), never a clock.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Defibrillator {
    /// Selected energy, J.
    pub energy_j: f64,
    /// Sync mode (cardioversion).
    pub synchronised: bool,
    pub state: DefibState,
    /// Energy the capacitor was charged to (charging again after changing energy is needed).
    charged_j: f64,
}

impl Default for Defibrillator {
    fn default() -> Self {
        Self {
            energy_j: 150.0,
            synchronised: false,
            state: DefibState::Idle,
            charged_j: 0.0,
        }
    }
}

impl Defibrillator {
    /// Charge time to `joules`, ms.
    #[must_use]
    pub fn charge_time_ms(joules: f64) -> f64 {
        CHARGE_MS_PER_J * joules.max(0.0)
    }

    /// Start charging to the selected energy at `now_ms`.
    pub fn charge(&mut self, now_ms: f64) {
        self.charged_j = self.energy_j;
        self.state = DefibState::Charging {
            until_ms: now_ms + Self::charge_time_ms(self.energy_j),
        };
    }

    /// Advance to `now_ms`; returns `true` when charging just completed (the "ready" tone).
    pub fn update(&mut self, now_ms: f64) -> bool {
        if let DefibState::Charging { until_ms } = self.state
            && now_ms >= until_ms
        {
            self.state = DefibState::Ready;
            return true;
        }
        false
    }

    /// Fraction of the charge reached at `now_ms` (0 … 1).
    #[must_use]
    pub fn charge_fraction(&self, now_ms: f64) -> f64 {
        match self.state {
            DefibState::Idle => 0.0,
            DefibState::Ready => 1.0,
            DefibState::Charging { until_ms } => {
                let total = Self::charge_time_ms(self.charged_j).max(1.0);
                (1.0 - (until_ms - now_ms) / total).clamp(0.0, 1.0)
            }
        }
    }

    /// Discharge into the patient: the shock action (and back to idle), or `None` if not ready
    /// or the energy was changed after charging.
    pub fn shock(&mut self) -> Option<heartsim_core::Action> {
        if self.state != DefibState::Ready || (self.charged_j - self.energy_j).abs() > 1e-9 {
            return None;
        }
        self.state = DefibState::Idle;
        Some(heartsim_core::Action::Shock {
            joules: self.charged_j,
            synchronised: self.synchronised,
        })
    }

    /// Dump the charge internally without a shock.
    pub fn disarm(&mut self) {
        self.state = DefibState::Idle;
        self.charged_j = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn efficacy_rises_with_energy_and_matches_the_literature() {
        let vf = |e| success_probability(RhythmClass::VentricularFibrillation, e);
        assert!(vf(50.0) < vf(150.0) && vf(150.0) < vf(200.0));
        assert!((0.85..=0.94).contains(&vf(150.0)), "VF @150 J {}", vf(150.0));
        assert!(success_probability(RhythmClass::VentricularTachycardia, 100.0) >= 0.9);
        let af = success_probability(RhythmClass::AtrialFibrillationOrFlutter, 200.0);
        assert!((0.8..=0.92).contains(&af), "AF @200 J {af}");
        assert!(success_probability(RhythmClass::SupraventricularTachycardia, 100.0) >= 0.9);
        assert!(success_probability(RhythmClass::AtrialFlutter, 100.0) >= 0.9);
        assert_eq!(
            success_probability(RhythmClass::PolymorphicVentricularTachycardia, 150.0),
            success_probability(RhythmClass::VentricularFibrillation, 150.0)
        );
        assert_eq!(success_probability(RhythmClass::Asystole, 360.0), 0.0);
        assert_eq!(success_probability(RhythmClass::Organised, 5.0), 1.0);
    }

    #[test]
    fn rng_is_uniform_and_deterministic() {
        let mut a = ShockRng::new(7);
        let mut b = ShockRng::new(7);
        let xs: Vec<f64> = (0..10_000).map(|_| a.next_f64()).collect();
        assert!(xs.iter().all(|x| (0.0..1.0).contains(x)));
        let mean = xs.iter().sum::<f64>() / xs.len() as f64;
        assert!((mean - 0.5).abs() < 0.02);
        assert_eq!(xs[0], b.next_f64());
    }

    #[test]
    fn charge_then_shock() {
        let mut d = Defibrillator {
            energy_j: 200.0,
            synchronised: true,
            ..Defibrillator::default()
        };
        assert!(d.shock().is_none(), "not charged");
        d.charge(1000.0);
        assert!(!d.update(5999.0));
        assert!((d.charge_fraction(3500.0) - 0.5).abs() < 1e-9);
        assert!(d.update(6000.0), "200 J in 5 s");
        assert_eq!(
            d.shock(),
            Some(heartsim_core::Action::Shock {
                joules: 200.0,
                synchronised: true
            })
        );
        assert_eq!(d.state, DefibState::Idle);
        d.charge(0.0);
        d.update(1e6);
        d.energy_j = 100.0;
        assert!(d.shock().is_none(), "energy changed after charging");
    }
}
