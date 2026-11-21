//! Band-limited sample-rate conversion of the phonocardiogram (brief B25, moved here in B28 so the
//! WAV export in `heartsim-io` and the app's sound output share one converter): a Kaiser-windowed
//! sinc low-pass in a polyphase table, for a stream ([`Resampler`]) or a finished block
//! ([`resample_block`]). Plain linear interpolation would leave images of every sound above 2 kHz,
//! heard as hiss. Pure: no I/O, no allocation per sample, compiled on `wasm32`.
//!
//! Every constant is marked **designed** (a choice for this program) or carries its source.

/// Passband edge of the resampling filter, Hz. Designed: heart sounds have little energy above
/// 500 Hz (research note); 1.6 kHz leaves the filter room to reach its stopband before the PCG's
/// 2 kHz Nyquist frequency, where the images begin.
pub const RESAMPLER_CUTOFF_HZ: f64 = 1600.0;
/// Taps of the resampling filter, input samples (4 ms at 4 kHz; delay half of it). Designed.
pub const RESAMPLER_TAPS: usize = 16;
/// Fractional positions tabulated in the polyphase table (linearly interpolated between). Designed.
pub const RESAMPLER_PHASES: usize = 256;
/// Kaiser window shape of the resampling filter. Designed: β = 6 gives about 60 dB of stopband
/// attenuation (Kaiser's formula, `A ≈ β / 0.1102 + 8.7`).
pub const KAISER_BETA: f64 = 6.0;

/// Zeroth-order modified Bessel function of the first kind (the Kaiser window), by its series.
fn bessel_i0(x: f64) -> f64 {
    let (mut sum, mut term, mut k) = (1.0, 1.0, 1.0);
    while term > 1e-12 * sum {
        term *= (x / (2.0 * k)).powi(2);
        sum += term;
        k += 1.0;
    }
    sum
}

/// Half the tap count: the filter's centre.
const HALF_TAPS: usize = RESAMPLER_TAPS / 2;

/// Band-limited sample-rate conversion from the PCG rate to a device rate, for a stream: one output
/// sample per [`Resampler::next`] call, pulling input samples as needed.
///
/// A Kaiser-windowed sinc low-pass at [`RESAMPLER_CUTOFF_HZ`] with [`RESAMPLER_TAPS`] taps,
/// tabulated at [`RESAMPLER_PHASES`] fractional positions (each normalised to unit DC gain) and
/// interpolated between them. The table is built once on creation; `next` allocates nothing and
/// takes no lock, so it can run on the audio thread. The output lags the newest input by
/// `HALF_TAPS + 1 − frac` input samples ([`Resampler::input_time`]).
#[derive(Debug, Clone)]
pub struct Resampler {
    table: Box<[[f32; RESAMPLER_TAPS]]>,
    history: [f32; RESAMPLER_TAPS],
    /// Position between the two centre taps, `0.0..1.0`.
    frac: f64,
    /// Input samples per output sample.
    step: f64,
    pulled: u64,
}

impl Resampler {
    /// A converter from `in_rate_hz` to `out_rate_hz`.
    #[must_use]
    pub fn new(in_rate_hz: f64, out_rate_hz: f64) -> Self {
        let fc = 2.0 * RESAMPLER_CUTOFF_HZ / in_rate_hz;
        let i0b = bessel_i0(KAISER_BETA);
        let h = HALF_TAPS as f64;
        let kernel = |d: f64| {
            let r = d / h;
            if r.abs() >= 1.0 {
                return 0.0;
            }
            let x = std::f64::consts::PI * fc * d;
            let sinc = if x.abs() < 1e-12 { 1.0 } else { x.sin() / x };
            fc * sinc * bessel_i0(KAISER_BETA * (1.0 - r * r).sqrt()) / i0b
        };
        let table: Vec<[f32; RESAMPLER_TAPS]> = (0..=RESAMPLER_PHASES)
            .map(|p| {
                let frac = p as f64 / RESAMPLER_PHASES as f64;
                let mut row = [0.0f64; RESAMPLER_TAPS];
                for (j, c) in row.iter_mut().enumerate() {
                    // distance from tap j to the output point, input samples
                    *c = kernel((RESAMPLER_TAPS - 1 - HALF_TAPS) as f64 - j as f64 + frac);
                }
                let sum: f64 = row.iter().sum();
                let mut out = [0.0f32; RESAMPLER_TAPS];
                for (o, c) in out.iter_mut().zip(row) {
                    *o = (c / sum) as f32;
                }
                out
            })
            .collect();
        Self {
            table: table.into_boxed_slice(),
            history: [0.0; RESAMPLER_TAPS],
            frac: 0.0,
            step: in_rate_hz / out_rate_hz.max(1.0),
            pulled: 0,
        }
    }

    /// Input samples per output sample (the nominal ratio times any drift correction).
    pub fn set_step(&mut self, step: f64) {
        self.step = step.max(0.0);
    }

    /// The next output sample; `pull` is called once for every input sample consumed.
    pub fn next(&mut self, mut pull: impl FnMut() -> f32) -> f32 {
        self.frac += self.step;
        while self.frac >= 1.0 {
            self.frac -= 1.0;
            self.history.copy_within(1.., 0);
            self.history[RESAMPLER_TAPS - 1] = pull();
            self.pulled += 1;
        }
        let pos = self.frac * RESAMPLER_PHASES as f64;
        let p = (pos as usize).min(RESAMPLER_PHASES - 1);
        let w = (pos - p as f64) as f32;
        let (a, b) = (&self.table[p], &self.table[p + 1]);
        let mut y = 0.0f32;
        for j in 0..RESAMPLER_TAPS {
            y += self.history[j] * (a[j] + (b[j] - a[j]) * w);
        }
        y
    }

    /// How many input samples the next [`Resampler::next`] call will pull (a caller converting a
    /// finite block stops before it runs out).
    #[must_use]
    pub fn needs(&self) -> usize {
        (self.frac + self.step).floor() as usize
    }

    /// Time of the last output sample in input samples, counting the first input pulled as 0.
    #[must_use]
    pub fn input_time(&self) -> f64 {
        self.pulled as f64 - 1.0 - HALF_TAPS as f64 + self.frac
    }

    /// Forget the input history (silence) and the position.
    pub fn reset(&mut self) {
        self.history = [0.0; RESAMPLER_TAPS];
        self.frac = 0.0;
    }
}

/// Convert a finished block `input` at `in_rate_hz` to `out_rate_hz` with [`Resampler`]: output
/// sample `j` stands for input time `j · in_rate / out_rate` to within one output sample (the
/// filter's delay is compensated, so the block starts and ends where the input does). Input beyond
/// the ends is silence.
#[must_use]
pub fn resample_block(input: &[f32], in_rate_hz: f64, out_rate_hz: f64) -> Vec<f32> {
    let mut r = Resampler::new(in_rate_hz, out_rate_hz);
    let n_out = ((input.len() as f64) * out_rate_hz / in_rate_hz).round() as usize;
    let mut k = 0usize;
    let mut out = Vec::with_capacity(n_out);
    while out.len() < n_out {
        let y = r.next(|| {
            let v = input.get(k).copied().unwrap_or(0.0);
            k += 1;
            v
        });
        // the filter looks half its length ahead: outputs before input time 0 are its start-up
        if r.input_time() >= 0.0 {
            out.push(y);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_block_keeps_its_length_and_its_timing() {
        let input: Vec<f32> = (0..4000)
            .map(|k| (std::f64::consts::TAU * 100.0 * f64::from(k) / 4000.0).sin() as f32)
            .collect();
        let out = resample_block(&input, 4000.0, 44_100.0);
        assert_eq!(out.len(), 44_100);
        // in the middle the output is the sine at the output's own times
        let worst = (2000..42_000)
            .map(|j| {
                let t = j as f64 / 44_100.0;
                (f64::from(out[j]) - (std::f64::consts::TAU * 100.0 * t).sin()).abs()
            })
            .fold(0.0, f64::max);
        assert!(worst < 0.02, "{worst}");
    }
}
