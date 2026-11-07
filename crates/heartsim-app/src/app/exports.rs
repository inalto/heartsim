//! The Export / Import window of [`HeartSimApp`] (brief B20). A child module of `app` so it can
//! reach the model's fields while `app.rs` keeps only one-line hooks.
//!
//! **Review mode.** Importing a recording swaps it into the strip rings and pauses; the live rings
//! are kept aside. Resuming (the window's "Back to live" or the rhythm bar's Run) swaps them back,
//! so the simulation continues exactly where it was paused. Nothing is exported while reviewing:
//! the headers would name the running scenario and call a foreign recording "simulated".

use super::HeartSimApp;
use crate::export_panel::{
    ExportPanel, ExportRequest, Review, add_to_library, custom_scenario, export_from_rings, newest_sample_ms,
};
use eframe::egui;
use heartsim_ecg::Tier0Scenario;
use heartsim_io::{Paper, write_files};
use heartsim_sim::Engine as _;
use heartsim_ui::t;
use std::path::{Path, PathBuf};

impl HeartSimApp {
    /// Once per frame, before the simulation steps: leave review mode when playback resumed, take
    /// files dropped on the window, collect a finished screenshot, and draw the window.
    pub(super) fn exports_frame(&mut self, ctx: &egui::Context) {
        if self.exports.review.is_some() && !self.ecg.is_paused() {
            self.end_review();
        }
        let dropped: Option<PathBuf> =
            ctx.input(|i| i.raw.dropped_files.first().map(|f| f.path().to_path_buf()));
        if let Some(p) = dropped {
            self.exports.open = true;
            self.import(&p);
        }
        if let Some(msg) = self.exports.screenshot.lock().ok().and_then(|mut s| s.take()) {
            self.exports.status = Some(msg);
        }
        let mut open = self.exports.open;
        egui::Window::new(t!("export-title"))
            .open(&mut open)
            .resizable(false)
            .default_width(380.0)
            .show(ctx, |ui| self.exports_ui(ui));
        self.exports.open = open;
    }

    /// The warning in the menu bar while an exported file is open for review (the window itself is
    /// opened from the Menu).
    pub(super) fn export_review_label(&self, ui: &mut egui::Ui) {
        if let Some(r) = &self.exports.review {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.colored_label(
                    ui.visuals().warn_fg_color,
                    t!("export-reviewing", label = r.label.clone()),
                );
            });
        }
    }

    fn exports_ui(&mut self, ui: &mut egui::Ui) {
        ui.strong(t!("export-heading"));
        egui::Grid::new("export_settings").num_columns(2).show(ui, |ui| {
            ui.label(t!("export-length"));
            ui.add(
                egui::DragValue::new(&mut self.exports.window_s)
                    .range(1.0..=60.0)
                    .speed(0.5)
                    .suffix(format!(" {}", t!("export-seconds"))),
            );
            ui.end_row();
            ui.label(t!("export-pdf-paper"));
            ui.horizontal(|ui| {
                for p in [Paper::A4, Paper::Letter] {
                    ui.selectable_value(&mut self.exports.paper, p, p.label());
                }
            });
            ui.end_row();
            ui.label(t!("export-formats"));
            ui.horizontal_wrapped(|ui| {
                let f = &mut self.exports.formats;
                ui.checkbox(&mut f.pdf, "PDF");
                ui.checkbox(&mut f.png, "PNG");
                ui.checkbox(&mut f.csv, "CSV");
                ui.checkbox(&mut f.wfdb, "WFDB");
                ui.checkbox(&mut f.ron, t!("export-scenario"));
                ui.checkbox(&mut f.bundle, t!("export-bundle"));
            });
            ui.end_row();
            ui.label(t!("export-trace"));
            ui.vertical(|ui| {
                ui.checkbox(&mut self.exports.as_displayed, t!("export-as-displayed"))
                    .on_hover_text(t!("export-as-displayed-hint"));
                if self.exports.as_displayed && !self.ecg.display_config().is_identity() {
                    ui.weak(t!("export-as-displayed-active"));
                }
            });
            ui.end_row();
            ui.label(t!("export-folder"));
            ui.text_edit_singleline(&mut self.exports.dir);
            ui.end_row();
        });
        ui.weak(t!(
            "export-settings-summary",
            speed = self.ecg.style.mm_per_s.to_string(),
            gain = self.ecg.style.mm_per_mv.to_string(),
            order = self.ecg.order.label()
        ));
        ui.horizontal(|ui| {
            let reviewing = self.exports.review.is_some();
            if ui
                .add_enabled(!reviewing, egui::Button::new(t!("export-button")))
                .on_disabled_hover_text(t!("export-live-hint"))
                .clicked()
            {
                self.export_now();
            }
            if ui
                .button(t!("export-screenshot"))
                .on_hover_text(t!("export-screenshot-hint"))
                .clicked()
            {
                self.request_window_screenshot(ui.ctx());
            }
        });
        ui.separator();
        ui.strong(t!("import-heading"));
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.exports.import_path)
                    .hint_text(t!("import-hint"))
                    .desired_width(260.0),
            );
            if ui.button(t!("import-button")).clicked() {
                let p = PathBuf::from(self.exports.import_path.trim());
                self.import(&p);
            }
        });
        ui.weak(t!("import-drop-hint"));
        if let Some(r) = &self.exports.review {
            let text = t!(
                "export-review-length",
                label = r.label.clone(),
                seconds = heartsim_ui::i18n::number(r.duration_s, 1)
            );
            let back = ui
                .horizontal(|ui| {
                    ui.label(text);
                    ui.button(t!("export-back-live")).clicked()
                })
                .inner;
            if back {
                self.end_review();
            }
        }
        if let Some(s) = &self.exports.status {
            ui.separator();
            ui.label(s);
        }
    }

    fn export_now(&mut self) {
        let custom;
        let scenario = if let Some(s) = self.scenario.and_then(|i| self.library.entries.get(i)) {
            s
        } else {
            custom = custom_scenario(&self.engine.params, &self.synth);
            &custom
        };
        // by default the clean engine signal; "as displayed" exports the filtered trace and says so
        let trace_note = if self.exports.as_displayed {
            self.ecg.display_config().describe()
        } else {
            None
        };
        let req = ExportRequest {
            scenario,
            style: &self.ecg.style,
            order: self.ecg.order,
            strip_lead: self.selected_lead,
            window_s: self.exports.window_s,
            paper: self.exports.paper,
            formats: self.exports.formats,
            date: heartsim_io::today(),
            trace_note: trace_note.clone(),
        };
        let head = self.ecg.head(&self.rings);
        let fs = self.rings.first().map_or(500.0, |r| f64::from(r.sample_rate_hz));
        let pushed = self.rings.first().map_or(0, heartsim_ecg::LeadRing::pushed);
        let newest = newest_sample_ms(self.engine.now().0, pushed, head, fs);
        let rings = if trace_note.is_some() {
            self.ecg.display_rings(&self.rings)
        } else {
            &self.rings
        };
        let files = export_from_rings(rings, head, newest, &req);
        let dir = PathBuf::from(self.exports.dir.trim());
        self.exports.status = Some(
            match files
                .map_err(|e| e.to_string())
                .and_then(|f| write_files(&dir, &f).map_err(|e| e.to_string()))
            {
                Ok(written) => ExportPanel::describe(&written),
                Err(e) => t!("export-failed", error = e.as_str()),
            },
        );
    }

    fn request_window_screenshot(&mut self, ctx: &egui::Context) {
        let dir = PathBuf::from(self.exports.dir.trim());
        let path = dir.join(format!("heartsim-window-{:.0}.png", self.engine.now().0));
        let slot = std::sync::Arc::clone(&self.exports.screenshot);
        ctx.request_screenshot(move |img| {
            let msg = match save_png(&img, &dir, &path) {
                Ok(()) => t!("export-saved", path = path.display().to_string()),
                Err(e) => t!("export-screenshot-failed", error = e.to_string()),
            };
            if let Ok(mut s) = slot.lock() {
                *s = Some(msg);
            }
        });
    }

    fn import(&mut self, path: &Path) {
        self.exports.import_path = path.display().to_string();
        if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("ron")) {
            self.open_scenario(path);
            return;
        }
        match heartsim_io::import_path(path) {
            Ok(imp) => {
                let previous = self.exports.review.take();
                let review = Review::begin(&mut self.rings, &mut self.pacing.marks, previous, &imp);
                self.exports.status = Some(t!(
                    "import-reviewing",
                    label = imp.label.clone(),
                    seconds = heartsim_ui::i18n::number(review.duration_s, 1),
                    rate = format!("{:.0}", imp.record.sample_rate_hz)
                ));
                self.exports.review = Some(review);
                self.ecg.playback.pause();
                self.ecg.playback.set_scrub_ms(0.0, 0.0);
                self.ecg.signal_replaced();
            }
            Err(e) => self.exports.status = Some(t!("import-failed", error = e.to_string())),
        }
    }

    /// Run a scenario file: it joins the library for this session and loads like a picked rhythm.
    fn open_scenario(&mut self, path: &Path) {
        let parsed = std::fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|text| Tier0Scenario::from_ron(&text).map_err(|e| e.to_string()));
        match parsed {
            Ok(s) => {
                let title = s.scenario.title.clone();
                let index = add_to_library(&mut self.library, s);
                self.load_scenario(index);
                self.exports.status = Some(t!("export-running", title = title.as_str()));
            }
            Err(e) => {
                self.exports.status = Some(t!(
                    "import-failed-path",
                    path = path.display().to_string(),
                    error = e.as_str()
                ));
            }
        }
    }

    /// Put the live rings back and resume.
    fn end_review(&mut self) {
        if let Some(r) = self.exports.review.take() {
            self.exports.status = Some(t!("export-closed", label = r.label.clone()));
            r.end(&mut self.rings, &mut self.pacing.marks);
            self.ecg.signal_replaced();
        }
        if self.ecg.is_paused() {
            self.ecg.toggle_pause();
        }
    }
}

fn save_png(img: &egui::ColorImage, dir: &Path, path: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    let [w, h] = img.size;
    let bytes: Vec<u8> = img
        .pixels
        .iter()
        .flat_map(egui::Color32::to_srgba_unmultiplied)
        .collect();
    image::RgbaImage::from_raw(w as u32, h as u32, bytes)
        .ok_or_else(|| anyhow::anyhow!("screenshot buffer size"))?
        .save(path)?;
    Ok(())
}
