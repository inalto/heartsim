//! The Cases page of [`HeartSimApp`] (brief B41) and the case files dropped on the window. A child
//! module of `app` so it reaches the model's fields while `app.rs` keeps one-line hooks; the page
//! itself is [`crate::case_ui::Cases::library_ui`], the platform part [`crate::case_io`].

use super::{HeartSimApp, Tab};
use eframe::egui;

impl HeartSimApp {
    /// Once per frame, on every page: a `.hs` dropped on the window opens on the Cases page; a
    /// finished open or save dialog is handled; My cases are written to eframe storage when they
    /// changed (browser).
    pub(super) fn cases_frame(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.case_drops.take(ctx);
        while let Some((name, bytes)) = self.case_drops.poll() {
            self.tab = Tab::Cases;
            match bytes {
                Ok(b) => {
                    let _ = self.cases.open_bytes(&b, &name, &mut self.library);
                }
                Err(e) => self.cases.report(&name, &e),
            }
        }
        if self.cases.poll_io(&mut self.library) {
            self.tab = Tab::Cases;
        }
        if let Some(text) = self.cases.take_storage_text() {
            self.case_storage_text = Some(text);
        }
        if let Some(text) = self.case_storage_text.take() {
            if let Some(storage) = frame.storage_mut() {
                storage.set_string(crate::case_io::STORAGE_KEY, text);
                storage.flush();
            } else {
                // written by `save` instead
                self.case_storage_text = Some(text);
            }
        }
    }

    /// The Cases page: the built-in cases, My cases and the online slot (the reader opens on the
    /// right, as for tutorials).
    pub(super) fn cases_tab(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let outcome = self.cases.library_ui(ui, true);
                if outcome.started {
                    self.tutorials.close();
                }
            });
        });
    }
}
