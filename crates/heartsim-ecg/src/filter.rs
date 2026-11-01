//! Minimal signal conditioning. Real monitors apply a 0.05–150 Hz diagnostic band or a
//! 0.5–40 Hz monitor band; we expose first-order sections that are cheap and stateful per lead.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LowPass {
    alpha: f32,
    y: f32,
}

impl LowPass {
    #[must_use]
    pub fn new(cutoff_hz: f32, sample_rate_hz: f32) -> Self {
        let rc = 1.0 / (core::f32::consts::TAU * cutoff_hz);
        let dt = 1.0 / sample_rate_hz;
        Self {
            alpha: dt / (rc + dt),
            y: 0.0,
        }
    }
    pub fn process(&mut self, x: f32) -> f32 {
        self.y += self.alpha * (x - self.y);
        self.y
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct HighPass {
    alpha: f32,
    prev_x: f32,
    y: f32,
}

impl HighPass {
    #[must_use]
    pub fn new(cutoff_hz: f32, sample_rate_hz: f32) -> Self {
        let rc = 1.0 / (core::f32::consts::TAU * cutoff_hz);
        let dt = 1.0 / sample_rate_hz;
        Self {
            alpha: rc / (rc + dt),
            prev_x: 0.0,
            y: 0.0,
        }
    }
    pub fn process(&mut self, x: f32) -> f32 {
        self.y = self.alpha * (self.y + x - self.prev_x);
        self.prev_x = x;
        self.y
    }
}

/// Deterministic white noise (xorshift), mV amplitude.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Noise {
    state: u64,
    pub amplitude: f32,
}

impl Noise {
    #[must_use]
    pub fn new(seed: u64, amplitude: f32) -> Self {
        Self {
            state: seed.max(1),
            amplitude,
        }
    }
    pub fn sample(&mut self) -> f32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        let u = (x >> 11) as f32 / (1u64 << 53) as f32; // [0,1)
        (u * 2.0 - 1.0) * self.amplitude
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowpass_attenuates_high_frequency() {
        let fs = 1000.0;
        let mut lp = LowPass::new(40.0, fs);
        let mut amp_lo: f32 = 0.0;
        let mut amp_hi: f32 = 0.0;
        for i in 0..4000 {
            let t = i as f32 / fs;
            let y = lp.process((core::f32::consts::TAU * 2.0 * t).sin());
            if i > 2000 {
                amp_lo = amp_lo.max(y.abs());
            }
        }
        let mut lp = LowPass::new(40.0, fs);
        for i in 0..4000 {
            let t = i as f32 / fs;
            let y = lp.process((core::f32::consts::TAU * 200.0 * t).sin());
            if i > 2000 {
                amp_hi = amp_hi.max(y.abs());
            }
        }
        assert!(amp_lo > 0.9 && amp_hi < 0.3, "lo {amp_lo} hi {amp_hi}");
    }

    #[test]
    fn noise_is_bounded_and_deterministic() {
        let mut a = Noise::new(7, 0.05);
        let mut b = Noise::new(7, 0.05);
        for _ in 0..1000 {
            let x = a.sample();
            assert_eq!(x, b.sample());
            assert!(x.abs() <= 0.05);
        }
    }
}
