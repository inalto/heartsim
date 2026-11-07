//! The `kira` audio output of the monitor beep (feature `audio`, desktop only).
//!
//! The only place in the app that talks to an audio device. The tone itself comes from
//! [`crate::monitor_beep::synth_beep`]; nothing is loaded from disk.

use kira::sound::static_sound::StaticSoundData;
use kira::{AudioManager, AudioManagerSettings, Decibels, DefaultBackend, Frame};

use crate::monitor_beep::{BeepSink, synth_beep};

/// Sample rate of the synthesised beep, Hz. `kira` does not report the device's rate; a static sound is
/// resampled to the output rate on playback, so the tone keeps its pitch.
const BEEP_SAMPLE_RATE_HZ: u32 = 44_100;

/// Plays the beep through the default output device. Owns the device stream: dropping it stops the audio.
pub struct KiraBeepSink {
    manager: AudioManager<DefaultBackend>,
    sound: StaticSoundData,
}

impl std::fmt::Debug for KiraBeepSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KiraBeepSink").finish_non_exhaustive()
    }
}

impl KiraBeepSink {
    /// Opens the default output device and prepares the beep.
    ///
    /// # Errors
    /// The device error as text when there is no output device or the stream cannot be started (headless
    /// machines, CI, a sound server that is not running).
    pub fn new() -> Result<Self, String> {
        let frames: Vec<Frame> = synth_beep(BEEP_SAMPLE_RATE_HZ)
            .into_iter()
            .map(Frame::from_mono)
            .collect();
        let sound = StaticSoundData {
            sample_rate: BEEP_SAMPLE_RATE_HZ,
            frames: frames.into(),
            settings: kira::sound::static_sound::StaticSoundSettings::default(),
            slice: None,
        };
        let manager = AudioManager::<DefaultBackend>::new(AudioManagerSettings::default())
            .map_err(|e| e.to_string())?;
        Ok(Self { manager, sound })
    }
}

/// Linear gain `0.0..=1.0` as `kira` decibels; zero (or less) is `Decibels::SILENCE`, which kira renders as
/// silence (at or below -60 dB the amplitude is exactly 0).
fn volume_to_decibels(volume: f32) -> Decibels {
    if volume <= 0.0 {
        Decibels::SILENCE
    } else {
        Decibels(20.0 * volume.min(1.0).log10())
    }
}

impl BeepSink for KiraBeepSink {
    fn play(&mut self, volume: f32) {
        if volume <= 0.0 {
            return;
        }
        let sound = self.sound.volume(volume_to_decibels(volume));
        if let Err(e) = self.manager.play(sound) {
            tracing::debug!("monitor beep: could not start the sound: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_volume_is_unity_gain() {
        assert!((volume_to_decibels(1.0).as_amplitude() - 1.0).abs() < 1e-6);
        assert!((volume_to_decibels(7.0).as_amplitude() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn half_volume_is_about_minus_six_decibels() {
        let db = volume_to_decibels(0.5);
        assert!((db.0 + 6.02).abs() < 0.01, "{db:?}");
        assert!((db.as_amplitude() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn zero_volume_is_silent() {
        assert!(volume_to_decibels(0.0).as_amplitude() <= 0.0);
        assert!(volume_to_decibels(-1.0).as_amplitude() <= 0.0);
    }
}
