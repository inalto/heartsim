//! The Echo tab (brief B31): the probe on the figure, the sector in the 3-D heart and the anatomical
//! cross-section ([`crate::echo_page::EchoPage`]), above the strip the beat can be related to. The
//! page shares the rhythm bar with the Simulator; the slice is drawn at the strip's delayed time
//! (the wall motion's time), so pausing and scrubbing the strip pause and scrub the heart.

use super::HeartSimApp;
use eframe::egui;
use heartsim_render::LutStyle;
use heartsim_sim::Engine as _;

impl HeartSimApp {
    pub(super) fn echo_tab(&mut self, ui: &mut egui::Ui, rs: Option<&egui_wgpu::RenderState>) {
        egui::Panel::bottom("echo_strip")
            .resizable(true)
            .default_size(150.0)
            .min_size(crate::responsive::strip_min_height(ui.ctx()))
            .show(ui, |ui| {
                self.ecg.controls(ui, &self.rings, Some(&mut self.selected_lead));
                let rect = egui::Rect::from_min_size(ui.cursor().min, ui.available_size());
                self.ecg.strip(ui, rect, &self.rings, self.selected_lead);
                ui.allocate_rect(rect, egui::Sense::hover());
            });
        let colour_blind = self
            .heart
            .as_ref()
            .is_some_and(|h| h.lut_style == LutStyle::ColourBlindSafe);
        // the ECG lead of the strip under the trace, in step with it (brief B34)
        let ecg = self.echo_ecg_trace();
        self.echo.set_ecg(ecg);
        self.echo.set_fast_mmode(self.settings.echo_fast_mmode);
        self.echo.ui(ui, self.heart.as_mut(), rs, colour_blind);
        if let Some(req) = self.echo.take_send() {
            self.send_to_calculators(&req);
        }
    }

    /// The selected lead of the strip for the last seconds, on the simulation clock.
    fn echo_ecg_trace(&self) -> Option<crate::echo_page::EcgTrace> {
        let ring = self.rings.get(self.selected_lead as usize)?;
        let fs = f64::from(ring.sample_rate_hz);
        if fs <= 0.0 {
            return None;
        }
        let end = ring.pushed();
        let start = end.saturating_sub((fs * 14.0) as u64);
        let mut samples = Vec::new();
        let first = ring.range(start, end, &mut samples);
        Some(crate::echo_page::EcgTrace {
            samples,
            // the newest sample is the engine's now (the rings are not reset with the engine's clock,
            // so the time base is anchored here, not at sample 0)
            first_ms: self.engine.now().0 - (end - first) as f64 * 1000.0 / fs,
            dt_ms: 1000.0 / fs,
            lead: format!("{:?}", self.selected_lead),
        })
    }

    /// A measurement of the Echo page goes to the calculators that take it: the fields are
    /// filled and marked "from the echo simulator", the first of them is opened.
    fn send_to_calculators(&mut self, req: &crate::echo_page::SendRequest) {
        let targets = crate::echo_send::targets(req.name, req.site);
        for t in &targets {
            let text = crate::echo_send::input_text(*t, req.value);
            self.echo_calc.receive_from_echo(t.calculator, t.input, &text);
        }
        for (t, choice) in crate::echo_send::companion_inputs(req.name) {
            self.echo_calc.receive_from_echo(t.calculator, t.input, choice);
        }
        if let Some(first) = targets.first() {
            self.echo_calc.select(first.calculator);
            self.tab = super::Tab::EchoCalculators;
        }
    }
}
