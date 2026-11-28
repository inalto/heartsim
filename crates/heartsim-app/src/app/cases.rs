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
        // the online library (B43): an answer that arrived, and a repaint while one is awaited
        self.cases.poll_online(&mut self.library);
        if self.cases.online().busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        if let Some(address) = self.cases.online_mut().take_address_change() {
            self.settings.case_library_address = address;
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

    /// Use `fetch` for the Cases page's online library (brief B43), with `default_base` as the
    /// library of an empty address field: the browser build's `fetch` transport and `../cases/`
    /// (the desktop has its own `ureq` one with the `online-cases` feature). Nothing is requested.
    pub fn set_case_fetch(&mut self, fetch: Box<dyn crate::case_fetch::CaseFetch>, default_base: &str) {
        if self.tour.is_none() {
            self.cases.set_online(fetch, Some(default_base));
        }
    }

    /// The Cases page: the built-in cases, My cases and the online library (the reader opens on the
    /// right, as for tutorials), or the case editor (B42), which scrolls its form under a header
    /// that stays in view and so needs no scroll area around it.
    pub(super) fn cases_tab(&mut self, ui: &mut egui::Ui) {
        if self.cases.is_editing() {
            let outcome = self.cases.page_ui(ui, true, &self.library);
            if outcome.started {
                self.tutorials.close();
            }
            return;
        }
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let outcome = self.cases.page_ui(ui, true, &self.library);
                if outcome.started {
                    self.tutorials.close();
                }
            });
        });
    }
}
