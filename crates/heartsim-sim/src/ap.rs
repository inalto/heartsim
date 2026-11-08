//! Action-potential template: a normalised shape f(φ) on phase φ ∈ [0, 1] stretched to the
//! cell's APD. Tier 0 and Tier 1 both play this shape; Tier 2 integrates ODEs instead.

use heartsim_core::math::Deterministic;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApTemplate {
    /// Samples of V_norm over the AP, index 0 = upstroke. Length ≥ 16.
    pub samples: Vec<f32>,
}

impl ApTemplate {
    /// A smooth ventricular-like shape: fast upstroke, notch, dome, repolarisation by phase 1.
    #[must_use]
    pub fn ventricular(n: usize) -> Self {
        let samples = (0..n)
            .map(|i| {
                let phi = i as f32 / n as f32;
                let upstroke = 1.0 - (-phi * 80.0).exp_d();
                let plateau = 1.0 - (phi / 0.9).powi(6).min(1.0);
                let dome = 0.85 + 0.15 * (1.0 - (phi * 6.0).min(1.0));
                (upstroke * plateau * dome).clamp(0.0, 1.0)
            })
            .collect();
        Self { samples }
    }

    /// The ventricular shape with a drug / electrolyte change of its plateau and phase 3
    /// (brief B14, research §3.5): `repolarisation_slope` +1 makes phase 3 steep and late
    /// (hyperkalaemia: tall, narrow, peaked T), −1 slow and early (hypokalaemia: low, broad T);
    /// `plateau_sag` 0 … 1 lets the plateau decline progressively (digoxin: the scooped ST
    /// segment that runs down into the T wave). `(0, 0)` is exactly [`ApTemplate::ventricular`].
    #[must_use]
    pub fn ventricular_modified(n: usize, repolarisation_slope: f32, plateau_sag: f32) -> Self {
        if repolarisation_slope == 0.0 && plateau_sag == 0.0 {
            return Self::ventricular(n);
        }
        let k = 6.0 * 2f32.powf_d(repolarisation_slope.clamp(-1.5, 1.5));
        let sag = 0.4 * plateau_sag.clamp(0.0, 1.0);
        let samples = (0..n)
            .map(|i| {
                let phi = i as f32 / n as f32;
                let upstroke = 1.0 - (-phi * 80.0).exp_d();
                let plateau = 1.0 - (phi / 0.9).powf_d(k).min(1.0);
                let dome = 0.85 + 0.15 * (1.0 - (phi * 6.0).min(1.0));
                let scoop = 1.0 - sag * (phi / 0.9).min(1.0).sqrt();
                (upstroke * plateau * dome * scoop).clamp(0.0, 1.0)
            })
            .collect();
        Self { samples }
    }

    /// A triangular atrial-like shape (shorter plateau).
    #[must_use]
    pub fn atrial(n: usize) -> Self {
        let samples = (0..n)
            .map(|i| {
                let phi = i as f32 / n as f32;
                let upstroke = 1.0 - (-phi * 80.0).exp_d();
                let decay = (1.0 - phi).powf_d(1.5);
                (upstroke * decay).clamp(0.0, 1.0)
            })
            .collect();
        Self { samples }
    }

    /// His–Purkinje shape: like the ventricular one with a longer, flatter plateau (Purkinje
    /// fibres have the longest APD of the heart; the APD itself comes from the region).
    #[must_use]
    pub fn purkinje(n: usize) -> Self {
        let samples = (0..n)
            .map(|i| {
                let phi = i as f32 / n as f32;
                let upstroke = 1.0 - (-phi * 100.0).exp_d();
                let plateau = 1.0 - (phi / 0.94).powi(10).min(1.0);
                let notch = 0.9 + 0.1 * (1.0 - (phi * 10.0).min(1.0));
                (upstroke * plateau * notch).clamp(0.0, 1.0)
            })
            .collect();
        Self { samples }
    }

    /// Nodal (SA / AV node) shape: slow Ca²⁺-driven upstroke over ≈ 10 % of the cycle, no
    /// plateau, gradual repolarisation.
    #[must_use]
    pub fn nodal(n: usize) -> Self {
        let samples = (0..n)
            .map(|i| {
                let phi = i as f32 / n as f32;
                let upstroke = 1.0 - (-phi * 12.0).exp_d();
                let decay = (1.0 - phi).powf_d(1.2);
                (0.8 * upstroke * decay).clamp(0.0, 1.0)
            })
            .collect();
        Self { samples }
    }

    /// Linear interpolation at phase φ (clamped; φ ≥ 1 → 0).
    #[must_use]
    pub fn at(&self, phase: f32) -> f32 {
        if !(0.0..1.0).contains(&phase) {
            return 0.0;
        }
        let x = phase * (self.samples.len() - 1) as f32;
        let i = x.floor() as usize;
        let t = x - i as f32;
        let a = self.samples[i];
        let b = self.samples[(i + 1).min(self.samples.len() - 1)];
        a + (b - a) * t
    }

    /// Value `elapsed_ms` after activation for a given APD.
    #[must_use]
    pub fn value(&self, elapsed_ms: f32, apd_ms: f32) -> f32 {
        self.at(elapsed_ms / apd_ms)
    }

    /// APD90-style duration fraction: phase at which the template falls below 0.1.
    #[must_use]
    pub fn repolarisation_phase(&self) -> f32 {
        let n = self.samples.len();
        for i in (n / 4)..n {
            if self.samples[i] < 0.1 {
                return i as f32 / n as f32;
            }
        }
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_rises_fast_and_returns_to_rest() {
        let t = ApTemplate::ventricular(256);
        assert!(t.at(0.02) > 0.7);
        assert!(t.at(0.5) > 0.6, "plateau");
        assert!(t.at(0.999) < 0.05);
        assert_eq!(t.at(1.0), 0.0);
        assert!(t.repolarisation_phase() > 0.8);
    }

    #[test]
    fn modified_ventricular_shapes() {
        let base = ApTemplate::ventricular(256);
        assert_eq!(ApTemplate::ventricular_modified(256, 0.0, 0.0), base);
        let steep = ApTemplate::ventricular_modified(256, 1.0, 0.0);
        let slow = ApTemplate::ventricular_modified(256, -1.0, 0.0);
        // late in the plateau the steep shape is still high, the slow one already falling
        assert!(steep.at(0.7) > base.at(0.7) && base.at(0.7) > slow.at(0.7));
        assert!(steep.at(0.999) < 0.05 && slow.at(0.999) < 0.05);
        let scooped = ApTemplate::ventricular_modified(256, 0.0, 1.0);
        assert!(scooped.at(0.6) < base.at(0.6) - 0.1, "sagging plateau");
        assert!((scooped.at(0.02) - base.at(0.02)).abs() < 0.1, "upstroke kept");
    }

    #[test]
    fn nodal_upstroke_is_slow_and_purkinje_plateau_long() {
        let v = ApTemplate::ventricular(256);
        let n = ApTemplate::nodal(256);
        let p = ApTemplate::purkinje(256);
        assert!(n.at(0.02) < 0.5 * v.at(0.02), "nodal upstroke slower");
        assert!(n.at(0.15) > 0.5);
        assert!(p.repolarisation_phase() >= v.repolarisation_phase());
        assert!(p.at(0.999) < 0.05);
    }
}
