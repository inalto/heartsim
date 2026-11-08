//! RF ablation: a generator stub and the book of lesions made.
//!
//! The lesion is a sphere around the tip whose radius approaches its final size exponentially with
//! the tissue temperature, `r(t) = r_max·(1 − exp(−t/τ))`; everything inside stops conducting. The
//! read-outs are a *stub* for teaching what the operator watches, not a thermal model:
//!
//! * tissue temperature `37 + ΔT·contact·(1 − exp(−t/τ))`, with `ΔT` from the power. Hyperthermic
//!   injury becomes irreversible above about 50 °C, so a catheter floating in blood makes no lesion;
//! * impedance `Z_blood + Z_contact·contact`, falling by up to [`RfParams::impedance_fall_ohm`] as the
//!   tissue heats, the drop operators read as a sign of effective heating;
//! * `τ` is on the order of the temperature half-time of ≈ 8 s reported for a 4 mm tip (Haines &
//!   Watson 1989; a value recalled from the paper, not checked against it). A lesion is about
//!   5–6 mm deep and 7–8 mm wide, hence the default `r_max` of 4 mm.

use glam::Vec3;
use heartsim_core::math::Deterministic;
use serde::{Deserialize, Serialize};

/// Generator settings and the tissue response to them.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RfParams {
    /// Delivered power, W.
    pub power_w: f64,
    /// Time constant of tissue heating and lesion growth, ms.
    pub tau_ms: f64,
    /// Final lesion radius at full contact, mm.
    pub max_radius_mm: f32,
    /// Temperature rise per watt at full contact and steady state, °C/W.
    pub heating_c_per_w: f64,
    /// Tissue must be hotter than this for the lesion to grow, °C.
    pub lesion_threshold_c: f64,
    /// Beyond this distance from the nearest myocardium the tip has no contact, mm.
    pub contact_range_mm: f32,
    /// Impedance of a tip in blood, Ω.
    pub blood_impedance_ohm: f64,
    /// Added impedance at full contact, Ω.
    pub contact_impedance_ohm: f64,
    /// Impedance fall when the tissue has reached steady temperature, Ω.
    pub impedance_fall_ohm: f64,
}

impl Default for RfParams {
    fn default() -> Self {
        Self {
            power_w: 30.0,
            tau_ms: 8000.0,
            max_radius_mm: 4.0,
            heating_c_per_w: 0.9,
            lesion_threshold_c: 50.0,
            contact_range_mm: 3.0,
            blood_impedance_ohm: 90.0,
            contact_impedance_ohm: 20.0,
            impedance_fall_ohm: 10.0,
        }
    }
}

/// What the generator shows.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RfReadout {
    /// Time since RF on, ms.
    pub elapsed_ms: f64,
    pub power_w: f64,
    pub temperature_c: f64,
    pub impedance_ohm: f64,
    /// Electrode–tissue contact, 0 (blood) … 1.
    pub contact: f64,
    /// Current radius of the lesion being made, mm.
    pub lesion_radius_mm: f32,
}

/// One RF application in progress.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RfApplication {
    pub tip_mm: Vec3,
    pub start_ms: f64,
    pub contact: f64,
    pub params: RfParams,
}

impl RfApplication {
    /// RF on at `now_ms`; `distance_mm` is from the tip to the nearest conducting myocardium.
    #[must_use]
    pub fn new(tip_mm: Vec3, now_ms: f64, distance_mm: f32, params: RfParams) -> Self {
        let contact = f64::from(1.0 - (distance_mm / params.contact_range_mm.max(1e-3)).clamp(0.0, 1.0));
        Self {
            tip_mm,
            start_ms: now_ms,
            contact,
            params,
        }
    }

    /// Generator read-out at `now_ms`.
    #[must_use]
    pub fn readout(&self, now_ms: f64) -> RfReadout {
        let p = &self.params;
        let elapsed_ms = (now_ms - self.start_ms).max(0.0);
        let growth = 1.0 - (-elapsed_ms / p.tau_ms.max(1.0)).exp_d();
        let temperature_c = 37.0 + p.heating_c_per_w * p.power_w * self.contact * growth;
        let impedance_ohm = p.blood_impedance_ohm + p.contact_impedance_ohm * self.contact
            - p.impedance_fall_ohm * self.contact * growth;
        let lesion_radius_mm = if temperature_c >= p.lesion_threshold_c {
            p.max_radius_mm * (self.contact as f32).sqrt() * growth as f32
        } else {
            0.0
        };
        RfReadout {
            elapsed_ms,
            power_w: p.power_w,
            temperature_c,
            impedance_ohm,
            contact: self.contact,
            lesion_radius_mm,
        }
    }
}

/// A finished or growing lesion.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Lesion {
    pub centre_mm: [f32; 3],
    pub radius_mm: f32,
    /// When it was made, ms of the simulation clock.
    pub at_ms: f64,
}

impl Lesion {
    /// Centre as a vector.
    #[must_use]
    pub fn centre(&self) -> Vec3 {
        Vec3::from(self.centre_mm)
    }

    /// `p` is inside.
    #[must_use]
    pub fn contains(&self, p: Vec3) -> bool {
        self.centre().distance_squared(p) <= self.radius_mm * self.radius_mm
    }
}

/// All lesions of a procedure. Scar does not heal, so the book survives an engine reset and the
/// lab re-applies it afterwards (the engines keep the cells; the book keeps the record).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LesionBook {
    lesions: Vec<Lesion>,
}

impl LesionBook {
    /// The lesions, oldest first.
    #[must_use]
    pub fn lesions(&self) -> &[Lesion] {
        &self.lesions
    }

    /// Record a lesion.
    pub fn add(&mut self, lesion: Lesion) {
        self.lesions.push(lesion);
    }

    /// Grow the most recent lesion in place (an RF application in progress).
    pub fn grow_last(&mut self, radius_mm: f32) {
        if let Some(l) = self.lesions.last_mut() {
            l.radius_mm = l.radius_mm.max(radius_mm);
        }
    }

    /// Forget the most recent lesion if it never grew.
    pub fn drop_last_if_empty(&mut self) {
        if self.lesions.last().is_some_and(|l| l.radius_mm <= 0.0) {
            self.lesions.pop();
        }
    }

    /// `p` lies in some lesion.
    #[must_use]
    pub fn contains(&self, p: Vec3) -> bool {
        self.lesions.iter().any(|l| l.contains(p))
    }

    /// Number of lesions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lesions.len()
    }

    /// No lesion yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lesions.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_radius_follows_the_time_constant() {
        let p = RfParams::default();
        let rf = RfApplication::new(Vec3::ZERO, 1000.0, 0.0, p);
        let at_tau = rf.readout(1000.0 + p.tau_ms);
        let expected = p.max_radius_mm * (1.0 - (-1.0f32).exp_d());
        assert!((at_tau.lesion_radius_mm - expected).abs() < 1e-3, "{at_tau:?}");
        let late = rf.readout(1000.0 + 10.0 * p.tau_ms);
        assert!(late.lesion_radius_mm > at_tau.lesion_radius_mm);
        assert!(
            late.impedance_ohm < rf.readout(1000.0).impedance_ohm,
            "impedance falls with heating"
        );
        assert!(late.temperature_c > 55.0 && late.temperature_c < 70.0, "{late:?}");
    }

    #[test]
    fn no_lesion_below_the_injury_temperature_or_without_contact() {
        let p = RfParams::default();
        let floating = RfApplication::new(Vec3::ZERO, 0.0, 10.0, p).readout(60_000.0);
        assert_eq!(floating.contact, 0.0);
        assert_eq!(floating.lesion_radius_mm, 0.0);
        assert_eq!(floating.temperature_c, 37.0);
        let touching = RfApplication::new(Vec3::ZERO, 0.0, 0.0, p);
        assert_eq!(
            touching.readout(100.0).lesion_radius_mm,
            0.0,
            "tissue not hot yet"
        );
    }

    #[test]
    fn the_book_remembers() {
        let mut b = LesionBook::default();
        assert!(b.is_empty());
        b.add(Lesion {
            centre_mm: [0.0; 3],
            radius_mm: 1.0,
            at_ms: 0.0,
        });
        b.grow_last(3.0);
        b.grow_last(2.0);
        assert_eq!(b.lesions()[0].radius_mm, 3.0);
        assert!(b.contains(Vec3::new(2.5, 0.0, 0.0)));
        assert!(!b.contains(Vec3::new(3.5, 0.0, 0.0)));
    }
}
