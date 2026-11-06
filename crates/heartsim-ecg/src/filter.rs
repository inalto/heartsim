//! Signal conditioning for the *display* of the ECG (requirement F-15).
//!
//! * [`LowPass`] / [`HighPass`] — cheap first-order sections.
//! * [`Biquad`] / [`BandFilter`] — the two clinical bandwidths as Butterworth cascades:
//!   **diagnostic 0.05–150 Hz** and **monitor 0.5–40 Hz** ([`FilterMode`]).
//! * [`Noise`] — deterministic white noise.
//!
//! Filters are applied to what is *shown* (see [`crate::display`]); the engine state, the analysis
//! and the default exports never see them.
//!
//! **Where the numbers come from.** Kligfield P et al., "Recommendations for the standardization
//! and interpretation of the electrocardiogram. Part I", *Circulation* 2007;115:1306–1324: the
//! diagnostic 12-lead ECG needs a low-frequency cut-off of 0.05 Hz (to leave the ST segment
//! undistorted) and a high-frequency cut-off of at least 150 Hz. Drew BJ et al., "Practice
//! standards for electrocardiographic monitoring in hospital settings", *Circulation*
//! 2004;110:2721–2746: bedside monitoring uses a narrower band (0.5–40 Hz for arrhythmia
//! monitoring) that is steadier but distorts ST segments and notches, which is why the two modes
//! exist. The sections here are 2nd-order Butterworth (−3 dB at the corner, −12 dB/octave) with a
//! 4th-order low-pass for the 40 Hz monitor corner. Being causal they delay the trace by a few
//! milliseconds (the wave labels, measured on the unfiltered signal, may sit that far off).

use core::f64::consts::{PI, TAU};
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

/// Direction of a [`Biquad`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    LowPass,
    HighPass,
}

/// A second-order IIR section (RBJ cookbook coefficients), direct form II transposed, in `f64`
/// because the 0.05 Hz high-pass puts its poles within 3·10⁻⁴ of the unit circle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    z1: f64,
    z2: f64,
}

impl Biquad {
    fn design(kind: Kind, cutoff_hz: f64, sample_rate_hz: f64, q: f64) -> Self {
        let w0 = TAU * cutoff_hz / sample_rate_hz;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / (2.0 * q);
        let a0 = 1.0 + alpha;
        let (b0, b1) = match kind {
            Kind::LowPass => ((1.0 - cos) / 2.0, 1.0 - cos),
            Kind::HighPass => (f64::midpoint(1.0, cos), -(1.0 + cos)),
        };
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b0 / a0,
            a1: -2.0 * cos / a0,
            a2: (1.0 - alpha) / a0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    /// Low-pass with corner `cutoff_hz` and quality factor `q` (1/√2 is Butterworth).
    #[must_use]
    pub fn low_pass(cutoff_hz: f64, sample_rate_hz: f64, q: f64) -> Self {
        Self::design(Kind::LowPass, cutoff_hz, sample_rate_hz, q)
    }

    /// High-pass with corner `cutoff_hz` and quality factor `q`.
    #[must_use]
    pub fn high_pass(cutoff_hz: f64, sample_rate_hz: f64, q: f64) -> Self {
        Self::design(Kind::HighPass, cutoff_hz, sample_rate_hz, q)
    }

    /// Filter one sample.
    pub fn process(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }

    /// Gain at 0 Hz: 1 for a low-pass, 0 for a high-pass.
    #[must_use]
    pub fn dc_gain(&self) -> f64 {
        (self.b0 + self.b1 + self.b2) / (1.0 + self.a1 + self.a2)
    }

    /// Put the state where a constant input `x` has already been applied for ever, so the first
    /// real sample does not start a switch-on transient.
    pub fn settle(&mut self, x: f64) {
        let y = self.dc_gain() * x;
        self.z1 = y - self.b0 * x;
        self.z2 = self.b2 * x - self.a2 * y;
    }

    /// Magnitude of the frequency response at `freq_hz` (1 = unchanged).
    #[must_use]
    pub fn magnitude(&self, freq_hz: f64, sample_rate_hz: f64) -> f64 {
        let w = TAU * freq_hz / sample_rate_hz;
        let (s1, c1) = w.sin_cos();
        let (s2, c2) = (2.0 * w).sin_cos();
        let num = (self.b0 + self.b1 * c1 + self.b2 * c2).hypot(-(self.b1 * s1 + self.b2 * s2));
        let den = (1.0 + self.a1 * c1 + self.a2 * c2).hypot(-(self.a1 * s1 + self.a2 * s2));
        num / den
    }
}

/// The Q of the `k`-th second-order section of an even-order Butterworth low/high-pass.
fn butterworth_q(order: usize, k: usize) -> f64 {
    1.0 / (2.0 * (PI * (2 * k + 1) as f64 / (2 * order) as f64).cos())
}

/// Which clinical bandwidth the displayed ECG goes through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FilterMode {
    /// The signal as the simulator produces it.
    #[default]
    Off,
    /// 0.05–150 Hz: the diagnostic 12-lead bandwidth (ST segment preserved).
    Diagnostic,
    /// 0.5–40 Hz: bedside monitoring (steady baseline, softened QRS peaks, distorted ST).
    Monitor,
}

impl FilterMode {
    /// All modes, in menu order.
    pub const ALL: [FilterMode; 3] = [Self::Off, Self::Diagnostic, Self::Monitor];

    /// `(high-pass corner, low-pass corner)` in Hz, or `None` when off.
    #[must_use]
    pub const fn band_hz(self) -> Option<(f64, f64)> {
        match self {
            Self::Off => None,
            Self::Diagnostic => Some((0.05, 150.0)),
            Self::Monitor => Some((0.5, 40.0)),
        }
    }

    /// Plain-ASCII name for file headers and captions (not translated: exports are
    /// language-neutral).
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Diagnostic => "diagnostic 0.05-150 Hz",
            Self::Monitor => "monitor 0.5-40 Hz",
        }
    }
}

/// One lead's [`FilterMode`] as a cascade of [`Biquad`]s (a high-pass, then a low-pass), with the
/// state a stream needs. The first sample primes the state to its own value, so a stream that
/// starts mid-signal does not ring.
#[derive(Debug, Clone, PartialEq)]
pub struct BandFilter {
    sections: Vec<Biquad>,
    sample_rate_hz: f64,
    primed: bool,
}

impl BandFilter {
    /// The filter for `mode` at `sample_rate_hz`. A low-pass corner at or above 0.45 of the sample
    /// rate is pulled down to it (a 250 Hz recording cannot carry 150 Hz).
    #[must_use]
    pub fn new(mode: FilterMode, sample_rate_hz: f32) -> Self {
        let fs = f64::from(sample_rate_hz).max(1.0);
        let mut sections = Vec::new();
        if let Some((hp, lp)) = mode.band_hz() {
            sections.push(Biquad::high_pass(hp, fs, butterworth_q(2, 0)));
            let lp = lp.min(0.45 * fs);
            let order = if lp <= 60.0 { 4 } else { 2 };
            for k in 0..order / 2 {
                sections.push(Biquad::low_pass(lp, fs, butterworth_q(order, k)));
            }
        }
        Self {
            sections,
            sample_rate_hz: fs,
            primed: false,
        }
    }

    /// True for [`FilterMode::Off`]: samples pass untouched.
    #[must_use]
    pub fn is_identity(&self) -> bool {
        self.sections.is_empty()
    }

    /// Filter one sample (mV).
    pub fn process(&mut self, x: f32) -> f32 {
        if self.sections.is_empty() {
            return x;
        }
        let mut v = f64::from(x);
        if !self.primed {
            self.primed = true;
            for s in &mut self.sections {
                s.settle(v);
                v *= s.dc_gain();
            }
            return v as f32;
        }
        for s in &mut self.sections {
            v = s.process(v);
        }
        v as f32
    }

    /// Gain of the whole cascade at `freq_hz` (1 = passes unchanged).
    #[must_use]
    pub fn gain_at(&self, freq_hz: f64) -> f64 {
        self.sections
            .iter()
            .map(|s| s.magnitude(freq_hz, self.sample_rate_hz))
            .product()
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

    const FS: f32 = 500.0;

    /// Steady-state amplitude of a unit sine at `f` Hz through a fresh filter, measured in the
    /// time domain over whole periods after `settle_s` seconds.
    fn measured_gain(mode: FilterMode, f: f64, settle_s: f64, measure_s: f64) -> f64 {
        let mut filt = BandFilter::new(mode, FS);
        let n_settle = (settle_s * f64::from(FS)) as usize;
        let n_measure = (measure_s * f64::from(FS)) as usize;
        let mut peak = 0.0f64;
        for i in 0..(n_settle + n_measure) {
            let t = i as f64 / f64::from(FS);
            let y = f64::from(filt.process((TAU * f * t).sin() as f32));
            if i >= n_settle {
                peak = peak.max(y.abs());
            }
        }
        peak
    }

    fn db(gain: f64) -> f64 {
        20.0 * gain.log10()
    }

    #[test]
    fn off_passes_samples_untouched() {
        let mut f = BandFilter::new(FilterMode::Off, FS);
        assert!(f.is_identity());
        for x in [0.0, 1.5, -0.25] {
            assert_eq!(f.process(x), x);
        }
        assert_eq!(f.gain_at(1000.0), 1.0);
    }

    #[test]
    fn corners_are_minus_three_db() {
        for (mode, corners) in [
            (FilterMode::Diagnostic, [0.05, 150.0]),
            (FilterMode::Monitor, [0.5, 40.0]),
        ] {
            let f = BandFilter::new(mode, FS);
            for fc in corners {
                let d = db(f.gain_at(fc));
                // the other corner is far away, so only the near one acts (the 4th-order monitor
                // low-pass is also exactly -3 dB at its corner)
                assert!((d + 3.0).abs() < 0.35, "{mode:?} at {fc} Hz: {d:.2} dB");
            }
        }
    }

    #[test]
    fn diagnostic_passes_the_clinical_band_and_stops_drift_and_hf_noise() {
        let f = BandFilter::new(FilterMode::Diagnostic, FS);
        // pass band: 1-100 Hz within 1 dB (the ST segment and the QRS survive)
        for hz in [1.0, 5.0, 20.0, 50.0, 100.0] {
            let d = db(f.gain_at(hz));
            assert!(d.abs() < 1.0, "{hz} Hz: {d:.2} dB");
        }
        // stop band: a decade below the high-pass corner is 40 dB down
        assert!(db(f.gain_at(0.005)) < -35.0);
        // above the 150 Hz corner the response falls away (the 500 Hz sample rate ends at 250 Hz)
        assert!(db(f.gain_at(200.0)) < -4.0);
    }

    #[test]
    fn monitor_passes_the_qrs_band_and_stops_wander_and_muscle() {
        let f = BandFilter::new(FilterMode::Monitor, FS);
        for hz in [2.0, 5.0, 10.0, 20.0] {
            let d = db(f.gain_at(hz));
            assert!(d.abs() < 1.0, "{hz} Hz: {d:.2} dB");
        }
        // stop band: 0.05 Hz wander is 40 dB down, 100 Hz muscle at least 30 dB down
        assert!(db(f.gain_at(0.05)) < -35.0, "{}", db(f.gain_at(0.05)));
        assert!(db(f.gain_at(100.0)) < -28.0, "{}", db(f.gain_at(100.0)));
        // 50 Hz mains is only a few dB down: the monitor band is not a notch filter
        let mains = db(f.gain_at(50.0));
        assert!((-12.0..-5.0).contains(&mains), "{mains:.1} dB");
    }

    #[test]
    fn the_time_domain_agrees_with_the_frequency_response() {
        // pass-band and stop-band amplitudes through the actual sample-by-sample filter
        let pass = measured_gain(FilterMode::Monitor, 10.0, 5.0, 2.0);
        assert!((0.9..=1.05).contains(&pass), "10 Hz through monitor: {pass}");
        let stop = measured_gain(FilterMode::Monitor, 120.0, 2.0, 1.0);
        assert!(stop < 0.03, "120 Hz through monitor: {stop}");
        let drift = measured_gain(FilterMode::Diagnostic, 0.05 / 4.0, 120.0, 40.0);
        assert!(drift < 0.1, "0.0125 Hz through diagnostic: {drift}");
        let pass = measured_gain(FilterMode::Diagnostic, 30.0, 2.0, 1.0);
        assert!((0.97..=1.03).contains(&pass), "30 Hz through diagnostic: {pass}");
    }

    #[test]
    fn a_constant_input_starts_without_a_transient() {
        // a DC offset is removed from the first sample on, not after seconds of ringing
        for mode in [FilterMode::Diagnostic, FilterMode::Monitor] {
            let mut f = BandFilter::new(mode, FS);
            for i in 0..100 {
                let y = f.process(0.8);
                assert!(y.abs() < 1e-4, "{mode:?} sample {i}: {y}");
            }
        }
    }

    #[test]
    fn filtering_is_deterministic() {
        let run = || {
            let mut f = BandFilter::new(FilterMode::Monitor, FS);
            (0..2000)
                .map(|i| f.process(((i as f32) * 0.07).sin() + 0.2))
                .collect::<Vec<f32>>()
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn a_low_sample_rate_pulls_the_low_pass_below_nyquist() {
        let f = BandFilter::new(FilterMode::Diagnostic, 250.0);
        assert!(f.gain_at(100.0).is_finite());
        assert!(f.gain_at(10.0) > 0.95);
    }

    #[test]
    fn butterworth_q_values() {
        assert!((butterworth_q(2, 0) - core::f64::consts::FRAC_1_SQRT_2).abs() < 1e-12);
        assert!((butterworth_q(4, 0) - 0.5412).abs() < 1e-3);
        assert!((butterworth_q(4, 1) - 1.3066).abs() < 1e-3);
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
