//! Heart vector: the 3-D equivalent dipole recovered from the 12 leads.
//!
//! The leads are linear in the dipole, `v = L·d` (see [`crate::leadvec`]); the least-squares
//! inverse `d = (LᵀL)⁻¹Lᵀ v` uses all twelve leads (the redundant limb leads average out noise)
//! and returns the dipole exactly for noise-free data generated with the same lead vectors.
//! With measured leads it is a Frank-like inverse in the spirit of the Dower/Kors transforms.

use crate::leadvec::LeadVectors;
use glam::{DMat3, DVec3};
use heartsim_core::LEAD_COUNT;

/// Precomputed least-squares inverse of a set of lead vectors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeartVectorSolver {
    /// Rows of `(LᵀL)⁻¹Lᵀ`: dipole component = `rows[c] · v`.
    rows: [[f64; LEAD_COUNT]; 3],
}

impl HeartVectorSolver {
    /// Build the inverse for `leads`. Returns `None` when the lead vectors do not span 3-D
    /// (e.g. only frontal-plane leads), in which case no unique dipole exists.
    #[must_use]
    pub fn new(leads: &LeadVectors) -> Option<Self> {
        let mut ltl = DMat3::ZERO;
        for l in &leads.vectors {
            ltl += DMat3::from_cols(*l * l.x, *l * l.y, *l * l.z);
        }
        if ltl.determinant().abs() < 1e-9 {
            return None;
        }
        let inv = ltl.inverse();
        let mut rows = [[0.0; LEAD_COUNT]; 3];
        for (k, l) in leads.vectors.iter().enumerate() {
            let col = inv * *l;
            rows[0][k] = col.x;
            rows[1][k] = col.y;
            rows[2][k] = col.z;
        }
        Some(Self { rows })
    }

    /// RMS gain of independent per-lead noise onto one dipole component: white noise of standard
    /// deviation σ on every lead produces σ·`noise_gain()` per component.
    #[must_use]
    pub fn noise_gain(&self) -> f64 {
        (self.rows.iter().flatten().map(|r| r * r).sum::<f64>() / 3.0).sqrt()
    }

    /// Dipole (mV in lead units; +x left, +y cranial, +z anterior) for one 12-lead sample.
    #[must_use]
    pub fn solve(&self, v: &[f64; LEAD_COUNT]) -> DVec3 {
        let dot = |r: &[f64; LEAD_COUNT]| r.iter().zip(v).map(|(a, b)| a * b).sum::<f64>();
        DVec3::new(dot(&self.rows[0]), dot(&self.rows[1]), dot(&self.rows[2]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovers_a_dipole_exactly() {
        let lv = LeadVectors::standard();
        let s = HeartVectorSolver::new(&lv).expect("standard leads span 3-D");
        for d in [
            DVec3::new(1.0, -0.5, 0.3),
            DVec3::new(-0.2, 0.9, -0.7),
            DVec3::ZERO,
        ] {
            let got = s.solve(&lv.project(d));
            assert!((got - d).length() < 1e-9, "{got} vs {d}");
        }
    }

    #[test]
    fn frontal_only_leads_are_rejected() {
        let mut lv = LeadVectors::standard();
        for v in &mut lv.vectors[6..] {
            *v = LeadVectors::frontal(30.0);
        }
        assert!(HeartVectorSolver::new(&lv).is_none());
    }
}
