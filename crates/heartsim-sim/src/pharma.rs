//! Pharmacology (brief B14): drugs and electrolyte disturbances as *parameter-modifier sets*
//! with onset / offset kinetics.
//!
//! A [`Drug`] of the [`DrugTable`] (`assets/drugs.ron`) states what it does at its standard dose
//! as a [`DrugEffects`] vector — multipliers of rates, conduction velocities and APDs, additions
//! to refractory periods, AP-shape changes — and how the effect develops after administration
//! ([`Kinetics`]). [`Pharmacology`] keeps every administration and resolves the combined effect
//! at a time `t`; the engines apply that effect at their use sites (the timing engine to the
//! SA rate, AV node, His–Purkinje delays and ERPs, Tier 1 to the per-cell CV/APD and the AP
//! template), so the strip and the 3-D heart show the same drug.
//!
//! Effect fraction of one administration of `dose` (1 = the table's standard dose) at
//! `elapsed` ms after it: `dose · rise(elapsed) · decay(elapsed)` with
//! `rise = 1 − exp(−3 · elapsed / onset)` (95 % at `onset_ms`) and, once `onset + hold` has
//! passed, `decay = 0.5^((elapsed − onset − hold) / half_life)` (no decay for `half_life = 0`,
//! e.g. an electrolyte disturbance that persists until corrected). Scales interpolate from 1:
//! `1 + (s − 1) · fraction`; additions scale linearly. Several drugs multiply their scales and
//! add their additions.
//!
//! Values are re-derived from the pharmacology literature (Opie & Gersh, *Drugs for the Heart*,
//! 9th ed.; Vaughan Williams classes; research doc §2.5.1 and §3.5 for the electrolyte / digoxin
//! AP changes) and simplified for teaching; they are not dosing guidance.

use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock};

/// Format version of `assets/drugs.ron`.
pub const DRUG_FORMAT_VERSION: u32 = 1;

/// The built-in drug table, compiled in so the engines need no I/O.
const BUILTIN_DRUGS: &str = include_str!("../../../assets/drugs.ron");

/// Lower bound of every resolved scale (a drug never stops conduction outright).
const MIN_SCALE: f64 = 0.05;
/// An administration whose effect has fallen below this fraction (after its hold) is dropped.
const SPENT_FRACTION: f64 = 1e-3;

/// What a drug does at its standard dose and full effect. Every field is neutral by default
/// (scales 1, additions 0), so a table entry states only what the drug changes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DrugEffects {
    /// SA-node rate multiplier (atropine, adrenaline > 1; adenosine, digoxin, verapamil < 1).
    pub sa_rate_scale: f64,
    /// Rate multiplier of automatic ectopic foci and escape pacemakers (adrenaline and digoxin
    /// toxicity ↑, lidocaine ↓ Purkinje automaticity).
    pub automaticity_scale: f64,
    /// Added to the recovery-time threshold (ERP) of both AV-nodal pathways, ms. Adenosine
    /// +300 (transient AV block), calcium-channel blockers +100…150, atropine −60.
    pub av_erp_add_ms: f64,
    /// AV-nodal conduction-velocity multiplier: nodal conduction times are divided by it
    /// (calcium-channel blockers ≈ 0.6 → PR ↑, slower ventricular response in AF).
    pub av_conduction_scale: f64,
    /// His–Purkinje CV multiplier: bundle delays are divided by it (class Ic, hyperkalaemia).
    pub his_purkinje_cv_scale: f64,
    /// Atrial myocardium CV multiplier (P widening; atrial conduction time is divided by it).
    pub atrial_cv_scale: f64,
    /// Ventricular myocardium CV multiplier (QRS widening: flecainide, hyperkalaemia).
    pub ventricular_cv_scale: f64,
    /// Atrial APD (and ERP) multiplier.
    pub atrial_apd_scale: f64,
    /// Ventricular APD (and ERP) multiplier: amiodarone, hypokalaemia > 1 (QT ↑); digoxin,
    /// hyperkalaemia < 1.
    pub ventricular_apd_scale: f64,
    /// Ventricular ERP multiplier on top of the APD change: post-repolarisation refractoriness
    /// of the sodium-channel blockers (lidocaine).
    pub ventricular_erp_scale: f64,
    /// Added to the accessory-pathway ERP, ms (class Ic and III drugs slow pre-excited AF).
    pub accessory_erp_add_ms: f64,
    /// Phase-3 slope of the ventricular AP: +1 = steep, early repolarisation (hyperkalaemia:
    /// tall, narrow, peaked T); −1 = slow, late repolarisation (hypokalaemia: low flat T).
    pub repolarisation_slope: f64,
    /// Sag of the ventricular plateau, 0 … 1 (digoxin: the "scooped", reverse-tick ST segment).
    pub plateau_sag: f64,
    /// Resting-potential offset of all myocardium, normalised V_m (0.1 ≈ +10 mV; hyperkalaemia
    /// depolarises, hypokalaemia hyperpolarises). Uniform, so it shows on the heart's voltage
    /// map but adds no injury current to the ECG.
    pub rest_offset: f64,
    /// U-wave amplitude, 0 … 1 of the textbook prominent U (hypokalaemia). Tier-0 morphology
    /// only: the Tier-1 mesh has no separate late-repolarising population.
    pub u_wave: f64,
}

impl Default for DrugEffects {
    fn default() -> Self {
        Self::NONE
    }
}

impl DrugEffects {
    /// No drug: every scale 1, every addition 0.
    pub const NONE: Self = Self {
        sa_rate_scale: 1.0,
        automaticity_scale: 1.0,
        av_erp_add_ms: 0.0,
        av_conduction_scale: 1.0,
        his_purkinje_cv_scale: 1.0,
        atrial_cv_scale: 1.0,
        ventricular_cv_scale: 1.0,
        atrial_apd_scale: 1.0,
        ventricular_apd_scale: 1.0,
        ventricular_erp_scale: 1.0,
        accessory_erp_add_ms: 0.0,
        repolarisation_slope: 0.0,
        plateau_sag: 0.0,
        rest_offset: 0.0,
        u_wave: 0.0,
    };

    /// These effects at `fraction` of their full size (dose × kinetics).
    #[must_use]
    pub fn scaled(&self, fraction: f64) -> Self {
        let s = |v: f64| (1.0 + (v - 1.0) * fraction).max(MIN_SCALE);
        let a = |v: f64| v * fraction;
        Self {
            sa_rate_scale: s(self.sa_rate_scale),
            automaticity_scale: s(self.automaticity_scale),
            av_erp_add_ms: a(self.av_erp_add_ms),
            av_conduction_scale: s(self.av_conduction_scale),
            his_purkinje_cv_scale: s(self.his_purkinje_cv_scale),
            atrial_cv_scale: s(self.atrial_cv_scale),
            ventricular_cv_scale: s(self.ventricular_cv_scale),
            atrial_apd_scale: s(self.atrial_apd_scale),
            ventricular_apd_scale: s(self.ventricular_apd_scale),
            ventricular_erp_scale: s(self.ventricular_erp_scale),
            accessory_erp_add_ms: a(self.accessory_erp_add_ms),
            repolarisation_slope: a(self.repolarisation_slope),
            plateau_sag: a(self.plateau_sag),
            rest_offset: a(self.rest_offset),
            u_wave: a(self.u_wave),
        }
    }

    /// Two drugs at once: scales multiply, additions add.
    #[must_use]
    pub fn combine(&self, o: &Self) -> Self {
        Self {
            sa_rate_scale: self.sa_rate_scale * o.sa_rate_scale,
            automaticity_scale: self.automaticity_scale * o.automaticity_scale,
            av_erp_add_ms: self.av_erp_add_ms + o.av_erp_add_ms,
            av_conduction_scale: self.av_conduction_scale * o.av_conduction_scale,
            his_purkinje_cv_scale: self.his_purkinje_cv_scale * o.his_purkinje_cv_scale,
            atrial_cv_scale: self.atrial_cv_scale * o.atrial_cv_scale,
            ventricular_cv_scale: self.ventricular_cv_scale * o.ventricular_cv_scale,
            atrial_apd_scale: self.atrial_apd_scale * o.atrial_apd_scale,
            ventricular_apd_scale: self.ventricular_apd_scale * o.ventricular_apd_scale,
            ventricular_erp_scale: self.ventricular_erp_scale * o.ventricular_erp_scale,
            accessory_erp_add_ms: self.accessory_erp_add_ms + o.accessory_erp_add_ms,
            repolarisation_slope: self.repolarisation_slope + o.repolarisation_slope,
            plateau_sag: self.plateau_sag + o.plateau_sag,
            rest_offset: self.rest_offset + o.rest_offset,
            u_wave: self.u_wave + o.u_wave,
        }
    }

    /// Largest relative difference between two effect sets (scales relative, additions in
    /// their own units / 100 ms or / 1), used to skip re-baking unchanged modifiers.
    #[must_use]
    pub fn distance(&self, o: &Self) -> f64 {
        let r = |a: f64, b: f64| (a / b.max(MIN_SCALE) - 1.0).abs();
        [
            r(self.sa_rate_scale, o.sa_rate_scale),
            r(self.automaticity_scale, o.automaticity_scale),
            (self.av_erp_add_ms - o.av_erp_add_ms).abs() / 100.0,
            r(self.av_conduction_scale, o.av_conduction_scale),
            r(self.his_purkinje_cv_scale, o.his_purkinje_cv_scale),
            r(self.atrial_cv_scale, o.atrial_cv_scale),
            r(self.ventricular_cv_scale, o.ventricular_cv_scale),
            r(self.atrial_apd_scale, o.atrial_apd_scale),
            r(self.ventricular_apd_scale, o.ventricular_apd_scale),
            r(self.ventricular_erp_scale, o.ventricular_erp_scale),
            (self.accessory_erp_add_ms - o.accessory_erp_add_ms).abs() / 100.0,
            (self.repolarisation_slope - o.repolarisation_slope).abs(),
            (self.plateau_sag - o.plateau_sag).abs(),
            (self.rest_offset - o.rest_offset).abs() * 10.0,
            (self.u_wave - o.u_wave).abs(),
        ]
        .into_iter()
        .fold(0.0, f64::max)
    }
}

/// Time course of a drug effect after one administration (see the module docs).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Kinetics {
    /// Time to 95 % of the full effect, ms (0 = immediate).
    pub onset_ms: f64,
    /// Full effect is kept this long after the onset, ms.
    pub hold_ms: f64,
    /// Elimination half-life of the effect after the hold, ms (0 = no decay).
    pub half_life_ms: f64,
}

impl Kinetics {
    /// Effect fraction (0 … 1) `elapsed_ms` after an administration of the standard dose.
    #[must_use]
    pub fn fraction(&self, elapsed_ms: f64) -> f64 {
        if elapsed_ms < 0.0 {
            return 0.0;
        }
        let rise = if self.onset_ms > 0.0 {
            1.0 - (-3.0 * elapsed_ms / self.onset_ms).exp()
        } else {
            1.0
        };
        let after = elapsed_ms - self.onset_ms - self.hold_ms;
        let decay = if self.half_life_ms > 0.0 && after > 0.0 {
            0.5f64.powf(after / self.half_life_ms)
        } else {
            1.0
        };
        rise * decay
    }

    /// The effect has worn off (it decays and is below the spent threshold).
    #[must_use]
    pub fn is_spent(&self, elapsed_ms: f64) -> bool {
        self.half_life_ms > 0.0
            && elapsed_ms > self.onset_ms + self.hold_ms
            && self.fraction(elapsed_ms) < SPENT_FRACTION
    }
}

/// One entry of the drug table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Drug {
    /// Lower-case id used by `Action::Drug { name }`, e.g. `"verapamil"`.
    pub name: String,
    /// Display name.
    pub title: String,
    /// Pharmacological class (Vaughan Williams class, receptor).
    pub class: String,
    /// What dose 1.0 stands for, e.g. `"6 mg IV rapid bolus"`.
    pub standard_dose: String,
    /// Teaching text: mechanism and ECG effect.
    #[serde(default)]
    pub notes: String,
    /// Dose multipliers the drug tray offers.
    #[serde(default = "default_doses")]
    pub doses: Vec<f64>,
    pub kinetics: Kinetics,
    pub effects: DrugEffects,
}

fn default_doses() -> Vec<f64> {
    vec![0.5, 1.0, 2.0]
}

/// The drug table (`assets/drugs.ron`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrugTable {
    pub format_version: u32,
    pub drugs: Vec<Drug>,
}

/// Errors reading a drug table.
#[derive(Debug)]
pub enum DrugTableError {
    Parse(ron::error::SpannedError),
    /// The file is newer than this build understands.
    Version(u32),
}

impl std::fmt::Display for DrugTableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DrugTableError::Parse(e) => write!(f, "drug table parse error: {e}"),
            DrugTableError::Version(v) => write!(
                f,
                "drug table format version {v} is newer than supported ({DRUG_FORMAT_VERSION})"
            ),
        }
    }
}

impl std::error::Error for DrugTableError {}

impl DrugTable {
    /// Parse a drug table from RON text.
    pub fn from_ron(text: &str) -> Result<Self, DrugTableError> {
        let t: Self = ron::from_str(text).map_err(DrugTableError::Parse)?;
        if t.format_version > DRUG_FORMAT_VERSION {
            return Err(DrugTableError::Version(t.format_version));
        }
        Ok(t)
    }

    /// The table shipped in `assets/drugs.ron` (compiled in; parsed once).
    ///
    /// # Panics
    /// If the shipped table does not parse (caught by the unit tests).
    #[must_use]
    pub fn builtin() -> Arc<Self> {
        static TABLE: OnceLock<Arc<DrugTable>> = OnceLock::new();
        Arc::clone(
            TABLE.get_or_init(|| Arc::new(Self::from_ron(BUILTIN_DRUGS).expect("assets/drugs.ron parses"))),
        )
    }

    /// Index of the drug called `name` (case-insensitive).
    #[must_use]
    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.drugs.iter().position(|d| d.name.eq_ignore_ascii_case(name))
    }

    /// The drug called `name` (case-insensitive).
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Drug> {
        self.index_of(name).map(|i| &self.drugs[i])
    }
}

/// One administration still acting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Administration {
    /// Index into the table.
    pub drug: usize,
    pub dose: f64,
    pub at_ms: f64,
}

/// What a drug is doing now (drug-tray timers).
#[derive(Debug, Clone, PartialEq)]
pub struct ActiveDrug {
    pub name: String,
    pub title: String,
    /// Sum of the doses still acting.
    pub dose: f64,
    /// Time since the latest administration, ms.
    pub since_ms: f64,
    /// Current effect as a fraction of the standard dose's full effect (dose × kinetics).
    pub fraction: f64,
}

/// Every administration so far and their combined effect (see the module docs). Deterministic:
/// the effect at `t` depends only on the administrations and `t`.
#[derive(Debug, Clone)]
pub struct Pharmacology {
    table: Arc<DrugTable>,
    doses: Vec<Administration>,
    /// Kinetics run this many times faster than real time (teaching mode; 1 = real time).
    speedup: f64,
}

impl Default for Pharmacology {
    fn default() -> Self {
        Self::new(DrugTable::builtin())
    }
}

impl Pharmacology {
    #[must_use]
    pub fn new(table: Arc<DrugTable>) -> Self {
        Self {
            table,
            doses: Vec::with_capacity(8),
            speedup: 1.0,
        }
    }

    /// The table drugs are looked up in.
    #[must_use]
    pub fn table(&self) -> &Arc<DrugTable> {
        &self.table
    }

    /// Replace the table; administrations of drugs that no longer exist are dropped.
    pub fn set_table(&mut self, table: Arc<DrugTable>) {
        let old = std::mem::replace(&mut self.table, table);
        let table = &self.table;
        self.doses
            .retain_mut(|a| match table.index_of(&old.drugs[a.drug].name) {
                Some(i) => {
                    a.drug = i;
                    true
                }
                None => false,
            });
    }

    /// Run the kinetics `speedup` times faster than real time (≥ 1e-3; 1 = real time).
    pub fn set_speedup(&mut self, speedup: f64) {
        self.speedup = speedup.max(1e-3);
    }

    #[must_use]
    pub fn speedup(&self) -> f64 {
        self.speedup
    }

    /// Give `dose` (multiples of the standard dose) of `name` at `at_ms`. A dose ≤ 0 stops the
    /// drug (every administration of it is removed: infusion stopped, electrolyte corrected).
    /// Returns `false` for an unknown drug.
    pub fn administer(&mut self, name: &str, dose: f64, at_ms: f64) -> bool {
        let Some(drug) = self.table.index_of(name) else {
            return false;
        };
        if dose <= 0.0 {
            self.doses.retain(|a| a.drug != drug);
        } else {
            self.doses.push(Administration { drug, dose, at_ms });
        }
        true
    }

    fn fraction(&self, a: &Administration, t: f64) -> f64 {
        let k = self.table.drugs[a.drug].kinetics;
        a.dose * k.fraction((t - a.at_ms) * self.speedup)
    }

    /// Combined effect at `t_ms`; drops administrations that have worn off.
    pub fn effects_at(&mut self, t_ms: f64) -> DrugEffects {
        let (table, speedup) = (&self.table, self.speedup);
        self.doses
            .retain(|a| !table.drugs[a.drug].kinetics.is_spent((t_ms - a.at_ms) * speedup));
        self.doses.iter().fold(DrugEffects::NONE, |acc, a| {
            acc.combine(&self.table.drugs[a.drug].effects.scaled(self.fraction(a, t_ms)))
        })
    }

    /// The drugs acting at `t_ms`, one entry per drug, in table order.
    #[must_use]
    pub fn active(&self, t_ms: f64) -> Vec<ActiveDrug> {
        let mut out: Vec<ActiveDrug> = Vec::new();
        for (i, d) in self.table.drugs.iter().enumerate() {
            let mine: Vec<&Administration> = self.doses.iter().filter(|a| a.drug == i).collect();
            let Some(latest) = mine.iter().map(|a| a.at_ms).reduce(f64::max) else {
                continue;
            };
            out.push(ActiveDrug {
                name: d.name.clone(),
                title: d.title.clone(),
                dose: mine.iter().map(|a| a.dose).sum(),
                since_ms: t_ms - latest,
                fraction: mine.iter().map(|a| self.fraction(a, t_ms)).sum(),
            });
        }
        out
    }

    /// Forget every administration.
    pub fn clear(&mut self) {
        self.doses.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_table_parses_and_covers_the_brief() {
        let t = DrugTable::builtin();
        for name in [
            "adenosine",
            "verapamil",
            "diltiazem",
            "digoxin",
            "flecainide",
            "amiodarone",
            "atropine",
            "adrenaline",
            "lidocaine",
            "hyperkalaemia",
            "hypokalaemia",
        ] {
            let d = t.get(name).unwrap_or_else(|| panic!("{name} missing"));
            assert!(!d.title.is_empty() && !d.standard_dose.is_empty(), "{name}");
            assert!(d.effects != DrugEffects::NONE, "{name} does nothing");
        }
        assert!(t.get("Adenosine").is_some(), "lookup is case-insensitive");
    }

    #[test]
    fn kinetics_rise_hold_and_decay() {
        let k = Kinetics {
            onset_ms: 1000.0,
            hold_ms: 2000.0,
            half_life_ms: 500.0,
        };
        assert_eq!(k.fraction(-1.0), 0.0);
        assert!((k.fraction(1000.0) - 0.95).abs() < 0.01);
        assert!(k.fraction(2900.0) > 0.99);
        assert!((k.fraction(3500.0) / k.fraction(3000.0) - 0.5).abs() < 0.01);
        assert!(!k.is_spent(3500.0));
        assert!(k.is_spent(3000.0 + 500.0 * 11.0));
        let persistent = Kinetics {
            half_life_ms: 0.0,
            ..k
        };
        assert!(persistent.fraction(1e9) > 0.99 && !persistent.is_spent(1e9));
    }

    #[test]
    fn scaling_and_combining() {
        let e = DrugEffects {
            av_conduction_scale: 0.6,
            av_erp_add_ms: 100.0,
            ..DrugEffects::NONE
        };
        let half = e.scaled(0.5);
        assert!((half.av_conduction_scale - 0.8).abs() < 1e-12);
        assert!((half.av_erp_add_ms - 50.0).abs() < 1e-12);
        assert_eq!(e.scaled(0.0), DrugEffects::NONE);
        let both = half.combine(&half);
        assert!((both.av_conduction_scale - 0.64).abs() < 1e-12);
        assert!((both.av_erp_add_ms - 100.0).abs() < 1e-12);
        assert!(e.scaled(10.0).av_conduction_scale >= MIN_SCALE);
    }

    #[test]
    fn administrations_wear_off_and_can_be_stopped() {
        let mut p = Pharmacology::default();
        assert!(!p.administer("snake oil", 1.0, 0.0));
        assert!(p.administer("adenosine", 1.0, 0.0));
        let peak = p.effects_at(3000.0).av_erp_add_ms;
        assert!(peak > 250.0, "adenosine peak ERP add {peak}");
        assert_eq!(p.active(3000.0).len(), 1);
        assert_eq!(p.effects_at(120_000.0), DrugEffects::NONE);
        assert_eq!(p.active(120_000.0).len(), 0, "spent dose dropped");
        p.administer("hyperkalaemia", 1.0, 0.0);
        assert!(p.effects_at(1e7).ventricular_apd_scale < 1.0);
        p.administer("hyperkalaemia", 0.0, 1e7);
        assert_eq!(p.effects_at(1e7), DrugEffects::NONE);
    }

    #[test]
    fn speedup_compresses_time() {
        let mut p = Pharmacology::default();
        p.administer("verapamil", 1.0, 0.0);
        let slow = p.effects_at(10_000.0).av_conduction_scale;
        p.set_speedup(10.0);
        let fast = p.effects_at(10_000.0).av_conduction_scale;
        assert!(fast < slow, "{fast} vs {slow}");
    }
}
