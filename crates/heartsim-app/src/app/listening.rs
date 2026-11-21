//! Heart-sound audio in the app frame (B25): the Display popup's and the Settings window's switch
//! kept as one setting, the player run once per frame after the simulation stepped, and its
//! display delay, status and playhead handed to the strip. A child module of `app` so that
//! `app.rs` keeps one-line hooks.

use super::HeartSimApp;
use crate::heart_sound_player::{AUDIO_BUILD, PlayerInput};
use heartsim_sim::Engine as _;
use heartsim_ui::LaneRequest;

impl HeartSimApp {
    /// Before the Settings window: a change made in the Display popup last frame becomes the
    /// setting.
    pub(super) fn heart_sound_popup_to_settings(&mut self) {
        if let Some(on) = self.ecg.play_heart_sounds {
            self.settings.heart_sound_audio = on;
        }
    }

    /// After the Settings window: the setting is what the Display popup shows (absent without
    /// audio output). Switching the sound on in Settings also shows the lane.
    pub(super) fn heart_sound_settings_to_popup(&mut self, settings_changed: bool) {
        if settings_changed && self.settings.heart_sounds {
            self.ecg.show_heart_sounds = true;
        }
        self.ecg.play_heart_sounds = AUDIO_BUILD.then_some(self.settings.heart_sound_audio);
    }

    /// Run the heart-sound player for this frame (after the simulation stepped) and hand its
    /// display delay, status and playhead to the strip.
    pub(super) fn heart_sound_frame(&mut self, dt_s: f64) {
        match self.ecg.take_lane_request() {
            Some(LaneRequest::Play { start, end }) => self.sound_player.play_slice(start, end),
            Some(LaneRequest::Stop) => self.sound_player.stop_slice(),
            None => {}
        }
        let input = PlayerInput {
            enabled: AUDIO_BUILD && self.settings.heart_sound_audio,
            volume: self.settings.heart_sound_volume,
            offset_ms: self.settings.audio_offset_ms,
            paused: self.ecg.is_paused(),
            speed: self.clock.speed,
            sim_now_ms: self.engine.now().0,
            dt_s,
            // an imported recording's beats are not the engine's
            muted: !self.heart_sounds_live(),
        };
        self.sound_player.frame(&mut self.sounds, &input);
        self.ecg
            .set_display_delay_ms(self.sound_player.display_delay_ms());
        self.ecg.heart_sound_status = self.sound_player.status();
        self.ecg.playhead = self.sound_player.playhead();
        self.ecg.slice_playing = self.sound_player.slice_playing();
    }

    /// How far behind the engine the 3-D view is drawn, ms: the scrub, plus the heart-sound display
    /// delay where the colour-map history is recorded (the Tier-0 simulator; the EP lab and the
    /// re-entry presets keep their 3-D heart live).
    pub(super) fn heart_view_offset_ms(&self) -> f64 {
        if self.ep_active() || self.reentry_active() {
            self.ecg.scrub_offset_ms()
        } else {
            self.ecg.view_offset_ms()
        }
    }
}
