//! Heart-sound audio in the app frame (B25): the Display popup's and the Settings window's switch
//! kept as one setting, the player run once per frame after the simulation stepped, and its
//! display delay, status and playhead handed to the strip. A child module of `app` so that
//! `app.rs` keeps one-line hooks.

use super::HeartSimApp;
use crate::heart_sound_player::{PcmSinkFactory, PlayerInput};
use heartsim_sim::Engine as _;
use heartsim_ui::{HeartSoundStatus, LaneRequest};

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
        self.ecg.play_heart_sounds = self
            .sound_player
            .has_output()
            .then_some(self.settings.heart_sound_audio);
    }

    /// Register the heart-sound output of the embedding crate (B26: the web build's Web Audio
    /// sink), replacing the one compiled in. `unavailable` is what the interface says when the
    /// factory fails. The factory is called the first time the sound is switched on, never before.
    pub fn set_heart_sound_output(&mut self, factory: PcmSinkFactory, unavailable: HeartSoundStatus) {
        self.sound_player.set_factory(factory, unavailable);
        self.ecg.play_heart_sounds = Some(self.settings.heart_sound_audio);
    }

    /// The "Play heart sounds" setting.
    #[must_use]
    pub fn heart_sound_audio(&self) -> bool {
        self.settings.heart_sound_audio
    }

    /// Switch the heart sounds on or off as the Settings window does (on also shows the lane); the
    /// web build's `?sound=1` uses it.
    pub fn set_heart_sound_audio(&mut self, on: bool) {
        self.settings.heart_sound_audio = on;
        if on {
            self.settings.heart_sounds = true;
            self.ecg.show_heart_sounds = true;
        }
        self.ecg.play_heart_sounds = self.sound_player.has_output().then_some(on);
    }

    /// What the heart-sound player reports (for hosts: `window.heartsimStatus` on the web).
    #[must_use]
    pub fn heart_sound_status(&self) -> HeartSoundStatus {
        self.sound_player.status()
    }

    /// Run the heart-sound player for this frame (after the simulation stepped) and hand its
    /// display delay, status and playhead to the strip.
    pub(super) fn heart_sound_frame(&mut self, dt_s: f64) {
        self.teaching_before_player();
        self.sounds.set_small_speaker(self.settings.small_speaker);
        match self.ecg.take_lane_request() {
            Some(LaneRequest::Play { start, end }) => self.sound_player.play_slice(start, end),
            Some(LaneRequest::Stop) => self.sound_player.stop_slice(),
            None => {}
        }
        let input = PlayerInput {
            enabled: self.sound_player.has_output() && self.settings.heart_sound_audio,
            volume: self.settings.heart_sound_volume,
            offset_ms: self.settings.audio_offset_ms,
            paused: self.ecg.is_paused(),
            speed: self.clock.speed,
            sim_now_ms: self.engine.now().0,
            dt_s,
            // an imported recording's beats are not the engine's; the teaching pages play their own
            // clips
            muted: !self.heart_sounds_live() || self.teaching_page_open(),
        };
        self.sound_player.frame(&mut self.sounds, &input);
        self.teaching_after_player();
        // the lane says when what is heard is not what it draws (B28)
        self.ecg.sound_tag =
            crate::heart_sounds::altered_sound_tag(self.sound_player.status(), self.sounds.small_speaker());
        self.ecg
            .set_display_delay_ms(self.sound_player.display_delay_ms());
        self.ecg.heart_sound_status = self.sound_player.status();
        self.ecg.playhead = self.sound_player.playhead();
        self.ecg.slice_playing = self.sound_player.slice_playing();
    }

    /// The listening site and chest piece (B27 part B): the Display popup's choice becomes the
    /// setting and what both synthesisers hear, so the lane, its title and the sound agree. Then the
    /// valve highlights of the 3-D heart, from the same schedule at the time the strip and the sound
    /// are at (the engine's minus the display delay and the scrub); none with reduced motion.
    pub(super) fn listening_frame(&mut self) {
        // the breathing choice (B28): a change becomes a breath-hold action for the engine, so the
        // sinus arrhythmia and the split freeze together; the popup then shows the engine's state
        if let Some(phase) = self.ecg.take_breath_request() {
            self.apply_therapy(&heartsim_core::Action::BreathHold { phase });
        }
        self.ecg.breath_hold_available = !self.ep_active() && !self.reentry_active();
        self.ecg.breath_hold = self.engine.respiration().held();
        let choice = self.ecg.listening;
        self.settings.listening_site = choice.site;
        self.settings.chest_piece = choice.chest;
        self.ecg.chest_image_opacity = self.settings.torso_image_opacity;
        self.sounds.set_listening(choice);
        let glow = match self.sounds.newest_ms() {
            Some(now) if !self.settings.reduced_motion && self.heart_sounds_live() => self
                .sounds
                .valve_glow_at(crate::heart_sounds::valve_display_time_ms(
                    now,
                    self.ecg.view_offset_ms(),
                )),
            _ => [0.0; 4],
        };
        if let Some(h) = &mut self.heart {
            h.set_valve_glow(glow);
        }
    }

    /// The listening site and chest piece now in effect (tests, the web build).
    #[must_use]
    pub fn listening(&self) -> heartsim_ecg::ListeningChoice {
        self.sounds.listening()
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
