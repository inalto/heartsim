//! The standard 12-lead system: 10 electrodes → 12 leads.
//!
//! Electrode potentials come from the forward model (`heartsim-sim`/`heartsim-ecg`); this module
//! only does the linear algebra of lead derivation, which is the same for every tier.
//!
//! * Einthoven: I = LA − RA, II = LL − RA, III = LL − LA
//! * Wilson central terminal: WCT = (RA + LA + LL) / 3
//! * Goldberger augmented: aVR = RA − (LA + LL)/2, aVL = LA − (RA + LL)/2, aVF = LL − (RA + LA)/2
//! * Precordial: Vi = φ(Vi) − WCT
//!
//! The right-leg electrode (N) is the reference / driven ground and carries no signal.

use serde::{Deserialize, Serialize};

pub const ELECTRODE_COUNT: usize = 10;
pub const LEAD_COUNT: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Electrode {
    RA = 0,
    LA = 1,
    LL = 2,
    /// Right leg: reference only.
    RL = 3,
    V1 = 4,
    V2 = 5,
    V3 = 6,
    V4 = 7,
    V5 = 8,
    V6 = 9,
}

impl Electrode {
    pub const ALL: [Electrode; ELECTRODE_COUNT] = [
        Electrode::RA,
        Electrode::LA,
        Electrode::LL,
        Electrode::RL,
        Electrode::V1,
        Electrode::V2,
        Electrode::V3,
        Electrode::V4,
        Electrode::V5,
        Electrode::V6,
    ];
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Electrode::RA => "RA",
            Electrode::LA => "LA",
            Electrode::LL => "LL",
            Electrode::RL => "RL",
            Electrode::V1 => "V1",
            Electrode::V2 => "V2",
            Electrode::V3 => "V3",
            Electrode::V4 => "V4",
            Electrode::V5 => "V5",
            Electrode::V6 => "V6",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Lead {
    I = 0,
    II = 1,
    III = 2,
    AVR = 3,
    AVL = 4,
    AVF = 5,
    V1 = 6,
    V2 = 7,
    V3 = 8,
    V4 = 9,
    V5 = 10,
    V6 = 11,
}

impl Lead {
    pub const ALL: [Lead; LEAD_COUNT] = [
        Lead::I,
        Lead::II,
        Lead::III,
        Lead::AVR,
        Lead::AVL,
        Lead::AVF,
        Lead::V1,
        Lead::V2,
        Lead::V3,
        Lead::V4,
        Lead::V5,
        Lead::V6,
    ];
    /// Limb leads in Cabrera order (aVL, I, −aVR, II, aVF, III) are a UI concern; this is the
    /// conventional printing order.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Lead::I => "I",
            Lead::II => "II",
            Lead::III => "III",
            Lead::AVR => "aVR",
            Lead::AVL => "aVL",
            Lead::AVF => "aVF",
            Lead::V1 => "V1",
            Lead::V2 => "V2",
            Lead::V3 => "V3",
            Lead::V4 => "V4",
            Lead::V5 => "V5",
            Lead::V6 => "V6",
        }
    }
    /// Frontal-plane angle of the limb leads in degrees (hexaxial reference system, clockwise
    /// positive as on the Cabrera circle). `None` for precordial leads.
    #[must_use]
    pub fn frontal_angle_deg(self) -> Option<f64> {
        match self {
            Lead::I => Some(0.0),
            Lead::II => Some(60.0),
            Lead::III => Some(120.0),
            Lead::AVR => Some(-150.0),
            Lead::AVL => Some(-30.0),
            Lead::AVF => Some(90.0),
            _ => None,
        }
    }
    #[must_use]
    pub fn is_limb(self) -> bool {
        (self as u8) < 6
    }
}

/// Potentials (mV) at the ten electrodes, indexed by `Electrode as usize`.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct ElectrodePotentials(pub [f64; ELECTRODE_COUNT]);

impl ElectrodePotentials {
    #[must_use]
    pub fn get(&self, e: Electrode) -> f64 {
        self.0[e as usize]
    }
    pub fn set(&mut self, e: Electrode, v: f64) {
        self.0[e as usize] = v;
    }
    /// Wilson central terminal.
    #[must_use]
    pub fn wct(&self) -> f64 {
        (self.get(Electrode::RA) + self.get(Electrode::LA) + self.get(Electrode::LL)) / 3.0
    }
    /// Derive the 12 leads.
    #[must_use]
    pub fn to_leads(&self) -> TwelveLead {
        let ra = self.get(Electrode::RA);
        let la = self.get(Electrode::LA);
        let ll = self.get(Electrode::LL);
        let wct = self.wct();
        let mut out = [0.0; LEAD_COUNT];
        out[Lead::I as usize] = la - ra;
        out[Lead::II as usize] = ll - ra;
        out[Lead::III as usize] = ll - la;
        out[Lead::AVR as usize] = ra - f64::midpoint(la, ll);
        out[Lead::AVL as usize] = la - f64::midpoint(ra, ll);
        out[Lead::AVF as usize] = ll - f64::midpoint(ra, la);
        for (i, e) in [
            Electrode::V1,
            Electrode::V2,
            Electrode::V3,
            Electrode::V4,
            Electrode::V5,
            Electrode::V6,
        ]
        .into_iter()
        .enumerate()
        {
            out[Lead::V1 as usize + i] = self.get(e) - wct;
        }
        TwelveLead(out)
    }
}

/// One sample of the twelve leads (mV), indexed by `Lead as usize`.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct TwelveLead(pub [f64; LEAD_COUNT]);

impl TwelveLead {
    #[must_use]
    pub fn get(&self, l: Lead) -> f64 {
        self.0[l as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;
    use proptest::prelude::*;

    proptest! {
        /// Einthoven's law: II = I + III, and the augmented leads sum to zero.
        #[test]
        fn einthoven_and_goldberger_identities(ra in -5.0f64..5.0, la in -5.0f64..5.0, ll in -5.0f64..5.0) {
            let mut p = ElectrodePotentials::default();
            p.set(Electrode::RA, ra); p.set(Electrode::LA, la); p.set(Electrode::LL, ll);
            let l = p.to_leads();
            prop_assert!((l.get(Lead::II) - (l.get(Lead::I) + l.get(Lead::III))).abs() < 1e-9);
            prop_assert!((l.get(Lead::AVR) + l.get(Lead::AVL) + l.get(Lead::AVF)).abs() < 1e-9);
            // aVF = (II + III)/2
            prop_assert!((l.get(Lead::AVF) - f64::midpoint(l.get(Lead::II), l.get(Lead::III))).abs() < 1e-9);
        }
    }

    #[test]
    fn precordial_is_relative_to_wct() {
        let mut p = ElectrodePotentials::default();
        p.set(Electrode::RA, 1.0);
        p.set(Electrode::LA, 1.0);
        p.set(Electrode::LL, 1.0);
        p.set(Electrode::V3, 2.5);
        assert_abs_diff_eq!(p.to_leads().get(Lead::V3), 1.5);
    }
}
