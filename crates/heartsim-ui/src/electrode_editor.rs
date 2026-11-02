//! Electrode editor (brief B10): the placement being edited, the lead vectors it gives the Tier-0
//! synthesiser, the standard-placement reference and extra-lead recordings, the misplacement
//! detector, and the control panel. Dragging on the 3-D torso lives in the app; it calls
//! [`ElectrodeEditor::drag_to`] with the surface point under the cursor.
//!
//! **One source of truth.** The editor never synthesises a signal: it reads the strip's ring
//! buffers, recovers the heart dipole of each sample (exact for Tier 0, [`DipoleSolver`]) and
//! projects that same dipole onto the standard lead vectors (the detector's reference) and the
//! extra leads. A placement change is installed into the synthesiser only after every sample made
//! with the old lead vectors has been read, so the recordings never mix placements.

use egui::Ui;
use glam::{DVec3, Vec3};
use heartsim_core::{Electrode, LEAD_COUNT, Scenario};
use heartsim_ecg::placement::PRECORDIAL;
use heartsim_ecg::{
    Cable, DetectorConfig, DipoleSolver, Exercise, ExtraLead, Finding, GaussianSynth, LeadRing, LeadVectors,
    Limb, Placement, Preset, Tier0Field, TorsoSurface, detect_misplacement,
};
use heartsim_geometry::{ElectrodeSet, TriMesh};
use std::collections::BTreeMap;

/// Length of the window the detector compares, s.
const DETECT_WINDOW_S: f32 = 3.0;
/// The detector waits for this much signal recorded with the current placement, s.
const DETECT_MIN_S: f32 = 1.5;
/// How often the detector runs, s of signal.
const DETECT_EVERY_S: f32 = 0.5;

/// Something the user asked the editor to do.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EditorCommand {
    /// Replace the placement by a preset (ends any exercise).
    Preset(Preset),
    /// Start an exercise from the standard placement; the answer stays hidden until revealed.
    Exercise(Exercise),
    /// Start a randomly chosen exercise.
    RandomExercise,
    /// Show the answer and the detector's explanation.
    Reveal,
    /// Exchange two cables.
    Swap(Electrode, Electrode),
    /// Put an electrode at a point (already on the skin).
    Move(Cable, Vec3),
    /// Put a limb electrode back on its own distal limb.
    PutOnLimb(Electrode),
    /// Switch an extra chest electrode on or off.
    SetExtra(ExtraLead, bool),
    /// Back to the standard 12-lead ("fit leads").
    Reset,
}

/// The editor model. See the module documentation.
#[derive(Debug)]
pub struct ElectrodeEditor {
    sites: ElectrodeSet,
    standard: Placement,
    field: Tier0Field,
    surface: Option<TorsoSurface>,
    placement: Placement,
    /// Snap dragged electrodes to intercostal spaces and chest-wall lines.
    pub snap: bool,
    /// Electrode selected in the list or by a click on the torso.
    pub selected: Option<Cable>,
    /// Cables chosen in the "swap" row of the panel.
    pub swap_pair: (Electrode, Electrode),
    exercise: Option<Exercise>,
    revealed: bool,
    findings: Option<Vec<Finding>>,
    /// Landmark the last drag snapped to.
    landmark: Option<String>,
    rng: u64,
    // --- signal side
    sample_rate_hz: f32,
    standard_leads: LeadVectors,
    solver: DipoleSolver,
    extra_vectors: Vec<(ExtraLead, DVec3)>,
    reference: Vec<LeadRing>,
    extra_rings: BTreeMap<ExtraLead, LeadRing>,
    /// Absolute index (in the strip's rings) of the next sample to read; `None` before the first.
    consumed: Option<u64>,
    /// First absolute sample recorded with the installed placement.
    installed_at: u64,
    dirty: bool,
    since_detect: u64,
}

impl ElectrodeEditor {
    /// Editor on the anatomy pack's electrode `sites` and `torso` surface (without a torso,
    /// dragging and surface moves are unavailable), reading strips sampled at `sample_rate_hz`
    /// and keeping `capacity` samples of reference and extra leads.
    #[must_use]
    pub fn new(sites: ElectrodeSet, torso: Option<&TriMesh>, sample_rate_hz: f32, capacity: usize) -> Self {
        let standard = Placement::standard(&sites);
        let field = Tier0Field::new(&sites);
        let standard_leads = field.lead_vectors(&standard);
        Self {
            surface: torso.map(TorsoSurface::new),
            placement: standard.clone(),
            standard,
            solver: DipoleSolver::new(&standard_leads),
            standard_leads,
            field,
            sites,
            snap: true,
            selected: None,
            swap_pair: (Electrode::RA, Electrode::LA),
            exercise: None,
            revealed: false,
            findings: None,
            landmark: None,
            rng: 0x9E37_79B9_7F4A_7C15,
            sample_rate_hz,
            extra_vectors: Vec::new(),
            reference: (0..LEAD_COUNT)
                .map(|_| LeadRing::new(capacity, sample_rate_hz))
                .collect(),
            extra_rings: BTreeMap::new(),
            consumed: None,
            installed_at: 0,
            dirty: true,
            since_detect: 0,
        }
    }

    // --- model --------------------------------------------------------------------------------

    #[must_use]
    pub fn placement(&self) -> &Placement {
        &self.placement
    }

    #[must_use]
    pub fn standard(&self) -> &Placement {
        &self.standard
    }

    #[must_use]
    pub fn surface(&self) -> Option<&TorsoSurface> {
        self.surface.as_ref()
    }

    /// The running exercise, if any.
    #[must_use]
    pub fn exercise(&self) -> Option<Exercise> {
        self.exercise
    }

    /// Whether the answer of the running exercise is shown (always true without one).
    #[must_use]
    pub fn revealed(&self) -> bool {
        self.exercise.is_none() || self.revealed
    }

    /// The detector's last verdict on the current placement; `None` while it is still collecting
    /// signal recorded with it.
    #[must_use]
    pub fn findings(&self) -> Option<&[Finding]> {
        self.findings.as_deref()
    }

    /// Landmark the last dragged electrode snapped to.
    #[must_use]
    pub fn landmark(&self) -> Option<&str> {
        self.landmark.as_deref()
    }

    /// Where an electrode is, in words.
    #[must_use]
    pub fn describe(&self, c: Cable) -> String {
        let Some(p) = self.placement.cable_position(c) else {
            return "off".to_string();
        };
        if let Some(l) = Limb::at(p) {
            return format!("on the {}", l.label());
        }
        let at_standard = match c {
            Cable::Standard(e) => self.standard.position(e).distance(p) < 1.0,
            Cable::Extra(x) => self.sites.get(x.label()).is_some_and(|s| s.distance(p) < 1.0),
        };
        let place = self.surface.as_ref().map_or_else(
            || format!("({:.0}, {:.0}, {:.0}) mm", p.x, p.y, p.z),
            |s| s.describe(p),
        );
        if at_standard {
            format!("{place} (standard)")
        } else {
            place
        }
    }

    /// Do what the user asked.
    pub fn apply(&mut self, cmd: EditorCommand) {
        match cmd {
            EditorCommand::Preset(p) => {
                self.placement = p.build(&self.sites);
                self.exercise = None;
            }
            EditorCommand::Exercise(x) => self.start_exercise(x),
            EditorCommand::RandomExercise => {
                let n = Exercise::ALL.len() as u64;
                let mut k = self.next_random() % n;
                if Some(Exercise::ALL[k as usize]) == self.exercise {
                    k = (k + 1) % n;
                }
                self.start_exercise(Exercise::ALL[k as usize]);
            }
            EditorCommand::Reveal => self.revealed = true,
            EditorCommand::Swap(a, b) => self.placement.swap(a, b),
            EditorCommand::Move(c, p) => self.placement.set_cable(c, p),
            EditorCommand::PutOnLimb(e) => {
                if let Some(l) = Limb::of(e) {
                    self.placement.set(e, l.anchor_mm());
                }
            }
            EditorCommand::SetExtra(x, on) => self.placement.set_extra(x, on, &self.sites),
            EditorCommand::Reset => {
                self.placement = self.standard.clone();
                self.exercise = None;
            }
        }
        self.dirty = true;
    }

    /// Move `c` to the torso point `p` (a pick on the surface), snapping to landmarks when on.
    pub fn drag_to(&mut self, c: Cable, p: Vec3) {
        let (p, landmark) = match (&self.surface, self.snap) {
            (Some(s), true) => {
                let snapped = s.snap(p);
                (snapped.position, snapped.landmark)
            }
            _ => (p, None),
        };
        self.landmark = landmark;
        if self.placement.cable_position(c) != Some(p) {
            self.apply(EditorCommand::Move(c, p));
        }
    }

    fn start_exercise(&mut self, x: Exercise) {
        self.placement = self.standard.clone();
        x.apply(&mut self.placement, &self.sites, self.surface.as_ref());
        self.exercise = Some(x);
        self.revealed = false;
        self.selected = None;
    }

    fn next_random(&mut self) -> u64 {
        // xorshift64*: deterministic per session, no extra dependency
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        self.rng.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11
    }

    /// Store the placement in a scenario (`electrodes_mm`, `extra_electrodes_mm`).
    pub fn save_scenario(&self, scenario: &mut Scenario) {
        self.placement.write_scenario(scenario, &self.standard);
    }

    /// Take the placement of a scenario (ends any exercise).
    pub fn load_scenario(&mut self, scenario: &Scenario) {
        self.placement = Placement::from_scenario(scenario, &self.sites);
        self.exercise = None;
        self.dirty = true;
    }

    // --- signal -------------------------------------------------------------------------------

    /// Twelve-lead lead vectors of the current placement.
    #[must_use]
    pub fn lead_vectors(&self) -> LeadVectors {
        self.field.lead_vectors(&self.placement)
    }

    /// The same heart recorded with the standard placement, in step with the strip.
    #[must_use]
    pub fn reference_rings(&self) -> &[LeadRing] {
        &self.reference
    }

    /// Recordings of the extra leads that are on.
    pub fn extra_rings(&self) -> impl Iterator<Item = (ExtraLead, &LeadRing)> {
        self.extra_rings.iter().map(|(x, r)| (*x, r))
    }

    /// Read the strip's new samples (made with the installed lead vectors) into the reference and
    /// extra recordings, then install a changed placement into `synth` and run the detector when
    /// due. Call once per frame after the simulation step.
    pub fn update(&mut self, rings: &[LeadRing], synth: &mut GaussianSynth) {
        let Some(first) = rings.first() else { return };
        let end = first.pushed();
        let start = self.consumed.unwrap_or(end).max(first.oldest_index());
        for abs in start..end {
            let v: [f64; LEAD_COUNT] =
                std::array::from_fn(|l| f64::from(rings.get(l).and_then(|r| r.at(abs)).unwrap_or(0.0)));
            let d = self.solver.dipole(&v);
            for (ring, x) in self.reference.iter_mut().zip(self.standard_leads.project(d)) {
                ring.push(x as f32);
            }
            for (x, vec) in &self.extra_vectors {
                if let Some(r) = self.extra_rings.get_mut(x) {
                    r.push(vec.dot(d) as f32);
                }
            }
        }
        self.consumed = Some(end);
        self.since_detect += end - start;
        if self.dirty {
            self.install(synth, end);
        }
        if self.since_detect as f32 >= DETECT_EVERY_S * self.sample_rate_hz {
            self.detect(rings);
        }
    }

    fn install(&mut self, synth: &mut GaussianSynth, at: u64) {
        let lv = self.lead_vectors();
        self.solver = DipoleSolver::new(&lv);
        synth.leads = lv;
        self.extra_vectors = self.field.extra_vectors(&self.placement);
        let (cap, rate) = (self.reference[0].capacity(), self.sample_rate_hz);
        self.extra_rings
            .retain(|x, _| self.placement.extras.contains_key(x));
        for x in self.placement.extras.keys() {
            self.extra_rings
                .entry(*x)
                .or_insert_with(|| LeadRing::new(cap, rate));
        }
        self.installed_at = at;
        self.findings = None;
        self.dirty = false;
    }

    /// Compare the strip with the reference over the signal recorded since the last change.
    fn detect(&mut self, rings: &[LeadRing]) {
        self.since_detect = 0;
        let Some(end) = rings.first().map(LeadRing::pushed) else {
            return;
        };
        // the reference rings were filled in step with the strip up to `end`, so the strip's
        // last `n` samples and theirs were taken at the same instants
        let n = end
            .saturating_sub(self.installed_at)
            .min((DETECT_WINDOW_S * self.sample_rate_hz) as u64)
            .min(rings[0].len() as u64)
            .min(self.reference[0].len() as u64);
        if (n as f32) < DETECT_MIN_S * self.sample_rate_hz {
            return;
        }
        let from = end - n;
        let n = n as usize;
        let mut cur = vec![Vec::new(); LEAD_COUNT];
        let mut reference = vec![Vec::new(); LEAD_COUNT];
        for l in 0..LEAD_COUNT {
            rings[l].range(from, end, &mut cur[l]);
            self.reference[l].latest(n, &mut reference[l]);
        }
        let c: Vec<&[f32]> = cur.iter().map(Vec::as_slice).collect();
        let r: Vec<&[f32]> = reference.iter().map(Vec::as_slice).collect();
        self.findings = Some(detect_misplacement(&c, &r, &DetectorConfig::default()));
    }
}

// --- panel ---------------------------------------------------------------------------------------

/// All cables plus the extras that are on, in panel order.
fn cables(p: &Placement) -> Vec<Cable> {
    let mut v: Vec<Cable> = Electrode::ALL.into_iter().map(Cable::Standard).collect();
    v.extend(p.extras.keys().map(|x| Cable::Extra(*x)));
    v
}

/// The editor's control panel: presets, extra leads, the electrode list, cable swaps, exercises
/// and the detector's verdict.
pub fn electrode_editor_panel(ui: &mut Ui, ed: &mut ElectrodeEditor) {
    let mut cmd = None;
    ui.strong("Presets");
    ui.horizontal_wrapped(|ui| {
        for p in Preset::ALL {
            if ui.button(p.label()).on_hover_text(p.description()).clicked() {
                cmd = Some(EditorCommand::Preset(p));
            }
        }
    });
    if ui
        .button("Fit leads")
        .on_hover_text("Put every electrode back on its standard site.")
        .clicked()
    {
        cmd = Some(EditorCommand::Reset);
    }
    ui.separator();
    ui.strong("Extra leads");
    ui.horizontal_wrapped(|ui| {
        for x in ExtraLead::ALL {
            let mut on = ed.placement.extras.contains_key(&x);
            if ui.checkbox(&mut on, x.label()).changed() {
                cmd = Some(EditorCommand::SetExtra(x, on));
            }
        }
    });
    ui.separator();

    ui.strong("Exercises");
    ui.horizontal_wrapped(|ui| {
        if ui.button("New exercise").clicked() {
            cmd = Some(EditorCommand::RandomExercise);
        }
        egui::ComboBox::new("exercise", "Choose")
            .selected_text("…")
            .show_ui(ui, |ui| {
                for x in Exercise::ALL {
                    if ui.selectable_label(false, x.label()).clicked() {
                        cmd = Some(EditorCommand::Exercise(x));
                    }
                }
            });
        if ui
            .add_enabled(!ed.revealed(), egui::Button::new("Explain"))
            .clicked()
        {
            cmd = Some(EditorCommand::Reveal);
        }
    });
    match (ed.exercise, ed.revealed()) {
        (Some(_), false) => {
            ui.label(
                "Something is wrong with the electrodes. Read the 12-lead, decide what, then press Explain.",
            );
        }
        (Some(x), true) => {
            ui.label(format!("Exercise: {}.", x.label()));
        }
        (None, _) => {}
    }
    if ed.revealed() {
        ui.separator();
        ui.strong("Detector");
        match ed.findings() {
            None => {
                ui.weak("Recording with the new placement…");
            }
            Some([]) => {
                ui.label("The leads match the standard placement.");
            }
            Some(f) => {
                for finding in f {
                    ui.colored_label(egui::Color32::from_rgb(230, 160, 60), finding.title());
                    ui.label(finding.explanation());
                }
            }
        }
        ui.separator();
        electrode_list(ui, ed, &mut cmd);
    }
    if let Some(c) = cmd {
        ed.apply(c);
    }
}

/// Electrode table, selected-electrode actions and the cable-swap row (hidden during an
/// unrevealed exercise, where it would give the answer away).
fn electrode_list(ui: &mut Ui, ed: &mut ElectrodeEditor, cmd: &mut Option<EditorCommand>) {
    ui.strong("Electrodes");
    ui.checkbox(&mut ed.snap, "Snap to intercostal spaces and chest lines");
    egui::Grid::new("electrodes")
        .num_columns(2)
        .striped(true)
        .show(ui, |ui| {
            for c in cables(&ed.placement) {
                let selected = ed.selected == Some(c);
                if ui.selectable_label(selected, c.label()).clicked() {
                    ed.selected = (!selected).then_some(c);
                }
                ui.label(ed.describe(c));
                ui.end_row();
            }
        });
    if let Some(l) = ed.landmark() {
        ui.weak(format!("Last snap: {l}"));
    }
    if let Some(Cable::Standard(e)) = ed.selected {
        ui.horizontal_wrapped(|ui| {
            if let Some(l) = Limb::of(e)
                && ed.placement.limb_of(e) != Some(l)
                && ui.button(format!("Back on the {}", l.label())).clicked()
            {
                *cmd = Some(EditorCommand::PutOnLimb(e));
            }
            if PRECORDIAL.contains(&e) && ui.button("Back to the standard site").clicked() {
                *cmd = Some(EditorCommand::Move(Cable::Standard(e), ed.standard.position(e)));
            }
        });
        ui.weak("Drag it on the torso to move it.");
    }
    ui.horizontal_wrapped(|ui| {
        for (id, slot, label) in [("swap_a", 0, "and"), ("swap_b", 1, "cables")] {
            let current = if slot == 0 { ed.swap_pair.0 } else { ed.swap_pair.1 };
            egui::ComboBox::new(id, label)
                .selected_text(current.label())
                .width(48.0)
                .show_ui(ui, |ui| {
                    for e in Electrode::ALL {
                        let target = if slot == 0 {
                            &mut ed.swap_pair.0
                        } else {
                            &mut ed.swap_pair.1
                        };
                        ui.selectable_value(target, e, e.label());
                    }
                });
        }
        if ui
            .add_enabled(ed.swap_pair.0 != ed.swap_pair.1, egui::Button::new("Swap"))
            .clicked()
        {
            *cmd = Some(EditorCommand::Swap(ed.swap_pair.0, ed.swap_pair.1));
        }
    });
}
