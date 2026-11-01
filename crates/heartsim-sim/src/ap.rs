//! Action-potential template: a normalised shape f(φ) on phase φ ∈ [0, 1] stretched to the
//! cell's APD. Tier 0 and Tier 1 both play this shape; Tier 2 integrates ODEs instead.

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
                let upstroke = 1.0 - (-phi * 80.0).exp();
                let plateau = 1.0 - (phi / 0.9).powi(6).min(1.0);
                let dome = 0.85 + 0.15 * (1.0 - (phi * 6.0).min(1.0));
                (upstroke * plateau * dome).clamp(0.0, 1.0)
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
                let upstroke = 1.0 - (-phi * 80.0).exp();
                let decay = (1.0 - phi).powf(1.5);
                (upstroke * decay).clamp(0.0, 1.0)
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
}
