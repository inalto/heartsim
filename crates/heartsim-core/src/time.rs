//! Simulation time. The engine runs on a fixed step; rendering interpolates. Keep the two clocks apart.

use serde::{Deserialize, Serialize};

/// Milliseconds as `f64`. Simulation time is always expressed in milliseconds so that
/// APD (300 ms), conduction delays (≈1 ms) and RR intervals (1000 ms) share one unit.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Millis(pub f64);

impl Millis {
    pub const ZERO: Self = Self(0.0);
    #[must_use]
    pub fn from_seconds(s: f64) -> Self {
        Self(s * 1000.0)
    }
    #[must_use]
    pub fn seconds(self) -> f64 {
        self.0 / 1000.0
    }
    /// Beats per minute → cycle length.
    #[must_use]
    pub fn from_bpm(bpm: f64) -> Self {
        Self(60_000.0 / bpm)
    }
    #[must_use]
    pub fn bpm(self) -> f64 {
        60_000.0 / self.0
    }
}

impl core::ops::Add for Millis {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}
impl core::ops::Sub for Millis {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }
}
impl core::ops::AddAssign for Millis {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}
impl core::ops::Mul<f64> for Millis {
    type Output = Self;
    fn mul(self, rhs: f64) -> Self {
        Self(self.0 * rhs)
    }
}

/// Fixed-step accumulator: converts wall-clock frame time into an integer number of simulation
/// steps, with a cap so a stalled frame never produces a "spiral of death".
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SimClock {
    /// Step length, ms (engine Tier 1 default 1 ms; Tier 2 reaction–diffusion 0.1–0.5 ms).
    pub dt: Millis,
    /// Current simulation time.
    pub now: Millis,
    /// Simulation speed multiplier (1 = real time, 0.1 = slow motion for teaching).
    pub speed: f64,
    /// Accumulated un-stepped time.
    accumulator: f64,
    /// Upper bound on steps per `advance` call.
    pub max_steps_per_frame: u32,
}

impl SimClock {
    #[must_use]
    pub fn new(dt: Millis) -> Self {
        Self {
            dt,
            now: Millis::ZERO,
            speed: 1.0,
            accumulator: 0.0,
            max_steps_per_frame: 200,
        }
    }

    /// Feed a frame of wall-clock time; returns how many fixed steps the engine must execute.
    pub fn advance(&mut self, frame: Millis) -> u32 {
        self.accumulator += frame.0 * self.speed;
        let mut steps = 0;
        while self.accumulator >= self.dt.0 && steps < self.max_steps_per_frame {
            self.accumulator -= self.dt.0;
            self.now += self.dt;
            steps += 1;
        }
        if steps == self.max_steps_per_frame {
            self.accumulator = 0.0; // drop the backlog rather than chase it
        }
        steps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bpm_roundtrip() {
        assert!((Millis::from_bpm(60.0).0 - 1000.0).abs() < 1e-9);
        assert!((Millis(500.0).bpm() - 120.0).abs() < 1e-9);
    }

    #[test]
    fn clock_steps_and_caps() {
        let mut c = SimClock::new(Millis(1.0));
        assert_eq!(c.advance(Millis(16.5)), 16);
        assert!((c.now.0 - 16.0).abs() < 1e-9);
        assert_eq!(c.advance(Millis(0.5)), 1); // leftover 0.5 + 0.5
        c.max_steps_per_frame = 10;
        assert_eq!(c.advance(Millis(1000.0)), 10);
        assert_eq!(c.advance(Millis(0.0)), 0); // backlog dropped
    }
}
