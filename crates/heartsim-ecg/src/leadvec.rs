//! Lead vectors: unit directions in heart coordinates such that a dipole **d** (mV·mm) produces
//! `v_lead = L_lead · d` (far-field, homogeneous torso approximation).
//!
//! Coordinate frame (same as the anatomy packs): +x = patient's left, +y = cranial (up),
//! +z = anterior. The frontal plane is x–y; the hexaxial angle θ is measured from +x towards −y
//! (i.e. towards the feet) so that +90° = aVF (inferior), matching clinical convention.
//!
//! Precordial vectors are approximate Frank/Dower-style directions tilted towards the chest wall;
//! Tier 1 replaces them with the exact 1/r² (or lead-field) weights from electrode positions.

use glam::DVec3;
use heartsim_core::{LEAD_COUNT, Lead};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LeadVectors {
    pub vectors: [DVec3; LEAD_COUNT],
}

impl LeadVectors {
    /// Unit vector in the frontal plane at hexaxial angle `deg`.
    #[must_use]
    pub fn frontal(deg: f64) -> DVec3 {
        let r = deg.to_radians();
        DVec3::new(r.cos(), -r.sin(), 0.0)
    }

    #[must_use]
    pub fn standard() -> Self {
        let mut v = [DVec3::ZERO; LEAD_COUNT];
        for l in Lead::ALL {
            if let Some(a) = l.frontal_angle_deg() {
                v[l as usize] = Self::frontal(a);
            }
        }
        // Precordial leads sweep from right-anterior (V1) to left-lateral (V6).
        let prec = [
            (Lead::V1, DVec3::new(-0.6, -0.1, 0.8)),
            (Lead::V2, DVec3::new(-0.2, -0.15, 0.95)),
            (Lead::V3, DVec3::new(0.3, -0.25, 0.9)),
            (Lead::V4, DVec3::new(0.65, -0.35, 0.65)),
            (Lead::V5, DVec3::new(0.85, -0.3, 0.4)),
            (Lead::V6, DVec3::new(0.95, -0.25, 0.1)),
        ];
        for (l, d) in prec {
            v[l as usize] = d.normalize();
        }
        Self { vectors: v }
    }

    #[must_use]
    pub fn project(&self, dipole: DVec3) -> [f64; LEAD_COUNT] {
        let mut out = [0.0; LEAD_COUNT];
        for (o, l) in out.iter_mut().zip(self.vectors.iter()) {
            *o = l.dot(dipole);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;
    use heartsim_core::{FrontalAxis, TwelveLead};

    #[test]
    fn frontal_dipole_recovers_its_axis() {
        let lv = LeadVectors::standard();
        for deg in [-60.0, 0.0, 45.0, 90.0, 150.0] {
            let leads = TwelveLead(lv.project(LeadVectors::frontal(deg) * 1.5));
            assert_abs_diff_eq!(FrontalAxis::from_limb_leads(&leads).degrees, deg, epsilon = 1e-9);
            assert_abs_diff_eq!(
                leads.get(Lead::II),
                leads.get(Lead::I) + leads.get(Lead::III),
                epsilon = 1e-12
            );
        }
    }
}
