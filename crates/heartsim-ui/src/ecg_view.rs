//! The ECG display state the app holds once: paper style, pause/scrub, calipers, wave labels and
//! the interval read-out, shared by the single-lead strip and the 12-lead page.

use crate::annotations::{EcgAnnotations, interval_readout};
use crate::calipers::{CaliperSet, Playback, draw_calipers, strip_interact};
use crate::strip::{GAINS_MM_MV, SPEEDS_MM_S, StripStyle, StripTrace, StripWindow, SweepMode, draw_strip};
use crate::twelve_lead::{LeadOrder, TwelveLeadLayout, page_cells};
use egui::{Rect, Ui};
use heartsim_core::Lead;
use heartsim_ecg::{Analysis, Analyzer, LeadRing};

/// How often (in signal time) the wave labels and intervals are re-measured, ms.
pub const ANALYSIS_PERIOD_MS: f64 = 250.0;

/// ECG display state. Rings are passed in on every call (they belong to the app's model).
#[derive(Debug)]
pub struct EcgView {
    pub style: StripStyle,
    pub playback: Playback,
    pub order: LeadOrder,
    /// Draw P/R/T letters.
    pub show_waves: bool,
    /// Draw PR and RR brackets.
    pub show_intervals: bool,
    analyzer: Analyzer,
    annotations: Option<EcgAnnotations>,
    strip_calipers: CaliperSet,
    page_calipers: Vec<CaliperSet>,
    page_layout: Option<TwelveLeadLayout>,
    /// Where the 12-lead page was last drawn, screen points (snapshot size).
    page_rect: Option<Rect>,
    /// Result of the last snapshot export, for the status line.
    pub last_export: Option<String>,
}

impl Default for EcgView {
    fn default() -> Self {
        Self {
            style: StripStyle::default(),
            playback: Playback::default(),
            order: LeadOrder::Standard,
            show_waves: true,
            show_intervals: true,
            analyzer: Analyzer::default(),
            annotations: None,
            strip_calipers: CaliperSet::default(),
            page_calipers: Vec::new(),
            page_layout: None,
            page_rect: None,
            last_export: None,
        }
    }
}

fn fs_of(rings: &[LeadRing]) -> f32 {
    rings.first().map_or(500.0, |r| r.sample_rate_hz)
}

fn pushed_of(rings: &[LeadRing]) -> u64 {
    rings.first().map_or(0, LeadRing::pushed)
}

impl EcgView {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the display (and so the simulation) is paused.
    #[must_use]
    pub fn is_paused(&self) -> bool {
        self.playback.is_paused()
    }

    /// Pause, or resume from "now".
    pub fn toggle_pause(&mut self) {
        self.playback.toggle();
    }

    /// How far the scrub cursor is behind the newest sample, ms (0 when live). The 3-D view
    /// shows the state at `now − scrub_offset_ms()`.
    #[must_use]
    pub fn scrub_offset_ms(&self) -> f64 {
        self.playback.scrub_ms()
    }

    /// Time of the newest sample on screen, ms since the first sample.
    #[must_use]
    pub fn cursor_time_ms(&self, rings: &[LeadRing]) -> f64 {
        self.playback.cursor_time_ms(pushed_of(rings), fs_of(rings))
    }

    /// Absolute index one past the newest sample on screen.
    #[must_use]
    pub fn head(&self, rings: &[LeadRing]) -> u64 {
        self.playback.head(pushed_of(rings), fs_of(rings))
    }

    /// Latest measurements, if the analyser has run.
    #[must_use]
    pub fn analysis(&self) -> Option<&Analysis> {
        self.annotations.as_ref().map(|a| &a.analysis)
    }

    /// Re-measure when the view head has moved by [`ANALYSIS_PERIOD_MS`] (or jumped backwards).
    /// Call once per frame before drawing.
    pub fn update(&mut self, rings: &[LeadRing]) {
        if rings.len() < heartsim_core::LEAD_COUNT {
            return;
        }
        let head = self.head(rings);
        let period = (ANALYSIS_PERIOD_MS * f64::from(fs_of(rings)) / 1000.0) as u64;
        let due = self
            .annotations
            .as_ref()
            .is_none_or(|a| head < a.head || head - a.head >= period.max(1));
        if due {
            self.annotations = Some(EcgAnnotations::compute(&self.analyzer, rings, head));
        }
    }

    /// The rings now hold a different signal (a recording swapped in or out): forget what was
    /// measured on the old one. The analysis is otherwise only redone when the head moves, so a
    /// paused view would keep showing the old read-out.
    pub fn signal_replaced(&mut self) {
        self.annotations = None;
        self.strip_calipers.reset();
        self.page_calipers.iter_mut().for_each(CaliperSet::reset);
    }

    /// Paper speed, gain, mode, colours, grid, calibration, labels, caliper reset and (while
    /// paused) the scrub slider.
    pub fn controls(&mut self, ui: &mut Ui, rings: &[LeadRing]) {
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt("ecg_speed")
                .selected_text(format!("{} mm/s", self.style.mm_per_s))
                .show_ui(ui, |ui| {
                    for s in SPEEDS_MM_S {
                        ui.selectable_value(&mut self.style.mm_per_s, s, format!("{s} mm/s"));
                    }
                });
            egui::ComboBox::from_id_salt("ecg_gain")
                .selected_text(format!("{} mm/mV", self.style.mm_per_mv))
                .show_ui(ui, |ui| {
                    for g in GAINS_MM_MV {
                        ui.selectable_value(&mut self.style.mm_per_mv, g, format!("{g} mm/mV"));
                    }
                });
            for m in [SweepMode::Scroll, SweepMode::Sweep] {
                ui.selectable_value(&mut self.style.mode, m, m.label());
            }
            let mut dark = self.style.is_dark();
            if ui.checkbox(&mut dark, "Monitor colours").changed() {
                self.style = if dark {
                    self.style.monitor()
                } else {
                    self.style.with_paper_colours()
                };
            }
            ui.checkbox(&mut self.style.show_grid, "Grid");
            ui.checkbox(&mut self.show_waves, "Waves");
            ui.checkbox(&mut self.show_intervals, "PR/RR");
            ui.add(
                egui::Slider::new(&mut self.style.px_per_mm, 1.5..=8.0)
                    .text("pt/mm")
                    .max_decimals(2),
            )
            .on_hover_text("Screen calibration: hold a ruler to the 5 mm grid squares and adjust until they measure 5 mm.");
            if ui
                .button("Calipers ✕")
                .on_hover_text("Shift-click a strip to add a caliper pair; drag lines; ←/→ nudge; Esc clears.")
                .clicked()
            {
                self.strip_calipers.reset();
                self.page_calipers.iter_mut().for_each(CaliperSet::reset);
            }
            if self.playback.is_paused() {
                ui.separator();
                let fs = fs_of(rings);
                let visible = rings.first().map_or(0, |r| r.len() as u64).min((10.0 * fs) as u64);
                let max = rings.first().map_or(0.0, |r| Playback::max_scrub_ms(r, visible));
                let mut back_s = self.playback.scrub_ms() / 1000.0;
                if ui
                    .add(egui::Slider::new(&mut back_s, 0.0..=(max / 1000.0).max(0.0)).text("s back"))
                    .changed()
                {
                    self.playback.set_scrub_ms(back_s * 1000.0, max);
                }
            }
        });
    }

    /// Interval read-out panel (HR, PR, QRS, QT, QTc, axis).
    pub fn readout(&self, ui: &mut Ui) {
        interval_readout(ui, self.analysis());
    }

    fn trace<'a>(&'a self, lead: Lead, label: &'a str, invert: bool, head: u64) -> StripTrace<'a> {
        StripTrace {
            label,
            invert,
            head,
            marks: self.annotations.as_ref().map(|a| a.lead(lead)),
            show_waves: self.show_waves,
            show_intervals: self.show_intervals,
        }
    }

    /// One interactive strip of `lead` filling `rect`. Returns its window.
    pub fn strip(&mut self, ui: &mut Ui, rect: Rect, rings: &[LeadRing], lead: Lead) -> StripWindow {
        let id = ui.scope_id().with("ecg_view_strip");
        let ring = &rings[lead as usize];
        let head = self.head(rings);
        let win = draw_strip(
            ui,
            id,
            rect,
            ring,
            &self.style,
            &self.trace(lead, lead.label(), false, head),
        );
        strip_interact(ui, id, &win, ring, &mut self.strip_calipers, &mut self.playback);
        draw_calipers(ui, &win, ring, false, &self.strip_calipers, &self.style);
        win
    }

    /// The 12-lead page in `rect`, each cell with its own calipers. Returns the cell windows.
    pub fn page(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        rings: &[LeadRing],
        layout: TwelveLeadLayout,
    ) -> Vec<StripWindow> {
        let cells = page_cells(rect, layout, self.order);
        if self.page_layout != Some(layout) || self.page_calipers.len() != cells.len() {
            self.page_calipers = vec![CaliperSet::default(); cells.len()];
            self.page_layout = Some(layout);
        }
        self.page_rect = Some(rect);
        let head = self.head(rings);
        let base = ui.scope_id().with("ecg_view_page");
        let mut wins = Vec::with_capacity(cells.len());
        for (i, cell) in cells.iter().enumerate() {
            let ring = &rings[cell.lead as usize];
            let id = base.with(i);
            let probe = StripWindow::new(cell.rect, &self.style, ring.sample_rate_hz, head);
            let cell_head = head.saturating_sub(cell.columns_before_head as u64 * probe.visible_samples);
            let trace = self.trace(cell.lead, cell.lead.label(), cell.invert, cell_head);
            let win = draw_strip(ui, id, cell.rect, ring, &self.style, &trace);
            strip_interact(ui, id, &win, ring, &mut self.page_calipers[i], &mut self.playback);
            draw_calipers(ui, &win, ring, cell.invert, &self.page_calipers[i], &self.style);
            wins.push(win);
        }
        wins
    }

    /// PNG of the current 12-lead page (what is on screen at the scrub cursor), `size_px` pixels.
    pub fn page_png(
        &self,
        rings: &[LeadRing],
        layout: TwelveLeadLayout,
        size_px: [u32; 2],
        px_per_point: f32,
    ) -> Result<Vec<u8>, image::ImageError> {
        let img = crate::snapshot::render_page(
            rings,
            &self.style,
            layout,
            self.order,
            self.head(rings),
            size_px,
            px_per_point,
        );
        crate::snapshot::encode_png(&img)
    }

    /// Write [`EcgView::page_png`] at the on-screen page size to `path` and record the outcome in
    /// `last_export`.
    pub fn export_page_png(
        &mut self,
        rings: &[LeadRing],
        layout: TwelveLeadLayout,
        px_per_point: f32,
        path: &std::path::Path,
    ) {
        let size = self
            .page_rect
            .map_or(egui::Vec2::new(1600.0, 1000.0), |r| r.size() * px_per_point);
        let size_px = [size.x.max(1.0) as u32, size.y.max(1.0) as u32];
        let res = self
            .page_png(rings, layout, size_px, px_per_point)
            .map_err(|e| e.to_string())
            .and_then(|png| std::fs::write(path, png).map_err(|e| e.to_string()));
        self.last_export = Some(match res {
            Ok(()) => format!("Saved {}", path.display()),
            Err(e) => format!("Snapshot failed: {e}"),
        });
    }

    /// Calipers of the single-lead strip.
    #[must_use]
    pub fn strip_calipers(&self) -> &CaliperSet {
        &self.strip_calipers
    }

    /// Calipers of the single-lead strip, for programmatic placement.
    pub fn strip_calipers_mut(&mut self) -> &mut CaliperSet {
        &mut self.strip_calipers
    }
}
