//! The heart-sound teaching pages in the app frame (brief B28): the Sound Trainer and the
//! auscultation quiz drawn with the shared player, the tutorial's sound check played through it, and
//! the clips' loops counted after the player has run. A child module of `app` so that `app.rs`
//! keeps one-line hooks.

use super::{HeartSimApp, Tab};
use crate::sound_clips::sound_available;
use eframe::egui;

impl HeartSimApp {
    /// Whether the open page plays its own clips: the live heart sounds stay quiet there.
    pub(super) fn teaching_page_open(&self) -> bool {
        matches!(self.tab, Tab::SoundTrainer | Tab::AuscultationQuiz)
    }

    /// Before the player runs: the small-speaker setting reaches the clips, the sound check knows
    /// whether there is sound, and a tone it asked for starts. A page that is left stops its clip.
    pub(super) fn teaching_before_player(&mut self) {
        let small = self.settings.small_speaker;
        self.sound_trainer.small_speaker = small;
        self.auscultation_quiz.small_speaker = small;
        self.tutorials.sound_available = sound_available(&self.sound_player);
        if let Some(tone) = self.tutorials.take_tone_request() {
            self.sound_trainer.stop(&mut self.sound_player);
            self.auscultation_quiz.stop(&mut self.sound_player);
            let samples: std::rc::Rc<[f32]> = tone.samples().into();
            self.sound_player.play_clip(samples, 0, false);
        }
        if self.tab != Tab::SoundTrainer && self.sound_trainer.playing {
            self.sound_trainer.stop(&mut self.sound_player);
        }
        if self.tab != Tab::AuscultationQuiz && self.auscultation_quiz.playing {
            self.auscultation_quiz.stop(&mut self.sound_player);
        }
    }

    /// After the player ran: count the loops heard.
    pub(super) fn teaching_after_player(&mut self) {
        self.sound_trainer.update(&self.sound_player);
        self.auscultation_quiz.update(&self.sound_player);
    }

    pub(super) fn sound_trainer_tab(&mut self, ui: &mut egui::Ui) {
        let request = self
            .sound_trainer
            .ui(ui, &self.library, &mut self.sound_player, &self.ecg.style);
        match request {
            #[cfg(feature = "export")]
            Some(crate::sound_trainer::TrainerRequest::SaveWav(entry)) => self.save_clip_wav(&entry),
            #[cfg(not(feature = "export"))]
            Some(_) => {}
            None => {}
        }
    }

    pub(super) fn auscultation_quiz_tab(&mut self, ui: &mut egui::Ui) {
        if let Some(open) =
            self.auscultation_quiz
                .ui(ui, &self.library, &mut self.sound_player, &self.ecg.style)
        {
            self.auscultation_quiz.stop(&mut self.sound_player);
            self.load_scenario(open.scenario);
            self.ecg.listening = open.listening;
            self.ecg.show_heart_sounds = true;
            self.tab = Tab::Simulator;
        }
    }
}
