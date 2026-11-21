//! The sound check of the heart-sound tutorial (brief B28): a low tone and a higher one, played
//! through the heart-sound sink, and what to do when the low one cannot be heard.
//!
//! The low tone is 200 Hz, the check the Watsjold et al. (2021) module gated entry with: heart
//! sounds and murmurs sit mostly below it, and a laptop or phone speaker that cannot reproduce it
//! cannot reproduce them. The higher tone shows that the output works at all.

use heartsim_ecg::PCG_RATE_HZ;

/// Frequency of the low tone, Hz (Watsjold et al. 2021).
pub const LOW_TONE_HZ: f64 = 200.0;
/// Frequency of the higher tone, Hz. Designed: well inside what any speaker reproduces and below
/// the sound output's 1.6 kHz passband edge.
pub const HIGH_TONE_HZ: f64 = 800.0;
/// Length of each tone, s. Designed.
pub const TONE_S: f64 = 1.5;
/// Fade at each end of a tone, ms. Designed: no click at the start or the end.
pub const TONE_FADE_MS: f64 = 30.0;
/// Peak of a tone in PCG units (a normal S1 peaks at 1): as loud as a normal S1 at the same volume
/// setting. Designed.
pub const TONE_PEAK: f32 = 1.0;

/// Which tone the learner asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Low,
    High,
}

impl Tone {
    /// The tone's frequency, Hz.
    #[must_use]
    pub fn hz(self) -> f64 {
        match self {
            Tone::Low => LOW_TONE_HZ,
            Tone::High => HIGH_TONE_HZ,
        }
    }

    /// The tone at the PCG rate, ready for `HeartSoundPlayer::play_clip`.
    #[must_use]
    pub fn samples(self) -> Vec<f32> {
        tone(self.hz())
    }
}

/// A sine of `hz` for [`TONE_S`] with a raised-cosine fade of [`TONE_FADE_MS`] at each end.
#[must_use]
pub fn tone(hz: f64) -> Vec<f32> {
    let rate = f64::from(PCG_RATE_HZ);
    let n = (TONE_S * rate) as usize;
    let fade = (TONE_FADE_MS / 1000.0 * rate) as usize;
    (0..n)
        .map(|i| {
            let edge = i.min(n - 1 - i);
            let w = if edge < fade {
                0.5 - 0.5 * (std::f64::consts::PI * edge as f64 / fade as f64).cos()
            } else {
                1.0
            };
            let s = (std::f64::consts::TAU * hz * i as f64 / rate).sin();
            (f64::from(TONE_PEAK) * w * s) as f32
        })
        .collect()
}

/// What the learner answered about the low tone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SoundCheck {
    /// `Some(true)`: heard; `Some(false)`: not heard.
    pub heard_low: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tones_have_their_pitch_and_fade_in_and_out() {
        for t in [Tone::Low, Tone::High] {
            let x = t.samples();
            assert_eq!(x.len(), (TONE_S * f64::from(PCG_RATE_HZ)) as usize);
            assert!(x[0].abs() < 1e-6 && x[x.len() - 1].abs() < 0.05);
            // zero crossings in the middle second give the frequency
            let mid = &x[2000..6000];
            let crossings = mid.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count();
            assert!((crossings as f64 - t.hz()).abs() <= 1.0, "{t:?}: {crossings}");
        }
    }
}
