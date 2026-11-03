//! The pacemaker window of the Simulator tab (brief B13): the temporary-pacemaker panel, the
//! strip's pacing-spike marks and the lead markers on the heart. Everything is driven by the
//! timing engine's marker channel, so spikes, LEDs and lead flashes are the same paces.

use super::HeartSimApp;
use eframe::egui;
use heartsim_sim::Engine;
use heartsim_sim::tier1::LeadProbe;
use heartsim_sim::timing::{PacerChannel, PacerConfig};
use heartsim_ui::{PaceMarks, PacerPanelState, pacer_panel};

/// Pacemaker UI state owned by the app.
#[derive(Debug, Default)]
pub(super) struct Pacing {
    /// The pacemaker window is shown.
    pub open: bool,
    pub panel: PacerPanelState,
    /// Pacing spikes in strip-sample coordinates.
    pub marks: PaceMarks,
    /// Electrogram probes at the lead tips on the 3-D heart (sensing amplitude from the mesh).
    probes: [Option<LeadProbe>; 3],
}

impl HeartSimApp {
    /// After one engine step (events already fed to the synth and the heart): take the marker
    /// channel, give non-captured paces their spike, record the strip marks, flash the LEDs, and
    /// measure the leads' electrograms on the heart for the next sensing decision.
    pub(super) fn pacing_step(&mut self) {
        let markers = self.engine.drain_pacer_markers();
        self.synth.on_pacer_markers(&markers);
        self.pacing.marks.record(&markers, self.rings[0].pushed());
        self.pacing.panel.on_markers(&markers, self.engine.now().0);
        let Some(h) = &self.heart else { return };
        if !h.tier1 {
            return;
        }
        let vm = &h.engine().cells().vm;
        let cfg = self.engine.params.pacer.clone();
        for ch in PacerChannel::ALL {
            if let Some(probe) = &mut self.pacing.probes[ch as usize]
                && let Some(f) = probe.update(vm, self.clock.dt.0)
                && let Some(lead) = cfg.as_ref().and_then(|c| c.lead(ch))
            {
                self.engine
                    .set_measured_amplitude_mv(ch, Some(f * lead.intrinsic()));
            }
        }
    }

    /// Put the leads of `cfg` on the heart and rebuild their probes.
    fn place_leads(&mut self, cfg: Option<&PacerConfig>) {
        for ch in PacerChannel::ALL {
            self.engine.set_measured_amplitude_mv(ch, None);
        }
        self.pacing.probes = [None, None, None];
        let (Some(h), Some(cfg)) = (&mut self.heart, cfg) else {
            return;
        };
        h.set_pacer_leads(cfg);
        let tips = h.lead_tip_cells();
        let cloud = h.engine().cloud();
        self.pacing.probes = PacerChannel::ALL.map(|ch| {
            let lead = cfg.lead(ch)?;
            (lead.site != heartsim_sim::timing::LeadSite::HisBundle)
                .then(|| LeadProbe::new(cloud, &tips[ch as usize]))
        });
    }

    /// The floating pacemaker window, and the lead markers it drives on the heart.
    pub(super) fn pacing_window(&mut self, ctx: &egui::Context) {
        // A scenario may have replaced the device: show what the engine runs.
        if self.engine.params.pacer != self.pacing.panel.device() {
            let device = self.engine.params.pacer.clone();
            self.pacing.panel = PacerPanelState::new(device.as_ref());
            self.place_leads(device.as_ref());
        }
        let mut open = self.pacing.open;
        let mut response = None;
        egui::Window::new("Pacemaker")
            .open(&mut open)
            .resizable(false)
            .show(ctx, |ui| response = Some(pacer_panel(ui, &mut self.pacing.panel)));
        self.pacing.open = open;
        if let Some(r) = response
            && r.changed
        {
            let device = self.pacing.panel.device();
            let reconnect = device.is_some() != self.engine.params.pacer.is_some();
            self.engine.params.pacer.clone_from(&device);
            if r.leads_moved || reconnect {
                self.place_leads(device.as_ref());
            }
        }
        if let Some(h) = &mut self.heart {
            let glyphs = if self.pacing.panel.connected {
                heartsim_ui::lead_glyphs(h.lead_tips(), &self.pacing.panel)
            } else {
                Vec::new()
            };
            h.set_lead_glyphs(glyphs);
        }
    }
}
