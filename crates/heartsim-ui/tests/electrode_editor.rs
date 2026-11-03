//! B10 acceptance: the electrode editor drives the Tier-0 strip pipeline (timing engine →
//! synthesiser → ring buffers) the way the app does, and its panel edits the placement.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use heartsim_core::{Electrode, LEAD_COUNT, Lead, Millis, Scenario};
use heartsim_ecg::{Finding, GaussianSynth, LeadRing, Limb};
use heartsim_geometry::AnatomyPack;
use heartsim_sim::{Engine, TimingEngine, TimingParams};
use heartsim_ui::{EditorCommand, ElectrodeEditor, electrode_editor_panel};
use std::path::Path;
use std::sync::OnceLock;

const RATE_HZ: f32 = 500.0;

fn pack() -> &'static AnatomyPack {
    static PACK: OnceLock<AnatomyPack> = OnceLock::new();
    PACK.get_or_init(|| {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/anatomy/default");
        AnatomyPack::load(&dir).expect("default pack loads")
    })
}

/// The app's Tier-0 loop: 1 ms steps, a 500 Hz strip, the editor updated once per 16 ms frame.
struct Sim {
    engine: TimingEngine,
    synth: GaussianSynth,
    rings: Vec<LeadRing>,
    editor: ElectrodeEditor,
}

impl Sim {
    fn new() -> Self {
        Self {
            engine: TimingEngine::new(TimingParams::default()),
            synth: GaussianSynth::textbook(),
            rings: (0..LEAD_COUNT)
                .map(|_| LeadRing::new((RATE_HZ * 30.0) as usize, RATE_HZ))
                .collect(),
            editor: ElectrodeEditor::new(
                pack().electrodes.clone(),
                Some(&pack().torso),
                RATE_HZ,
                (RATE_HZ * 30.0) as usize,
            ),
        }
    }

    fn run_ms(&mut self, ms: u32) {
        for step in 1..=ms {
            self.engine.step(Millis(1.0));
            let ev = self.engine.drain_events();
            self.synth.on_events(&ev);
            if step % 2 == 0 {
                let s = self.synth.sample(self.engine.now());
                for (r, v) in self.rings.iter_mut().zip(s.0) {
                    r.push(v as f32);
                }
            }
            if step % 16 == 0 {
                self.editor.update(&self.rings, &mut self.synth);
            }
        }
        self.editor.update(&self.rings, &mut self.synth);
    }

    fn last(&self, lead: Lead, seconds: f32) -> Vec<f32> {
        let mut v = Vec::new();
        self.rings[lead as usize].latest((seconds * RATE_HZ) as usize, &mut v);
        v
    }

    fn reference(&self, lead: Lead, seconds: f32) -> Vec<f32> {
        let mut v = Vec::new();
        self.editor.reference_rings()[lead as usize].latest((seconds * RATE_HZ) as usize, &mut v);
        v
    }
}

fn peak(x: &[f32]) -> f32 {
    x.iter()
        .copied()
        .fold(0.0, |m, v| if v.abs() > m.abs() { v } else { m })
}

#[test]
fn swapping_ra_la_in_the_editor_inverts_lead_i_in_the_ring_buffers() {
    let mut sim = Sim::new();
    sim.run_ms(3000);
    let before = sim.last(Lead::I, 2.0);
    assert_eq!(
        sim.editor.findings(),
        Some(&[][..]),
        "standard placement: nothing to report"
    );
    let r_before = peak(&before);
    assert!(r_before > 0.3, "upright QRS in lead I ({r_before} mV)");

    sim.editor
        .apply(EditorCommand::Swap(Electrode::RA, Electrode::LA));
    sim.run_ms(4000);
    let after = sim.last(Lead::I, 2.0);
    let reference = sim.reference(Lead::I, 2.0);
    assert_eq!(after.len(), reference.len());
    for (a, r) in after.iter().zip(&reference) {
        assert!((a + r).abs() < 1e-4, "lead I = −(standard lead I): {a} vs {r}");
    }
    assert!(peak(&after) < -0.3, "QRS in lead I now negative");
    // aVF and the chest leads are unchanged
    for lead in [Lead::AVF, Lead::V2, Lead::V5] {
        for (a, r) in sim.last(lead, 2.0).iter().zip(&sim.reference(lead, 2.0)) {
            assert!((a - r).abs() < 1e-4, "{}", lead.label());
        }
    }
    assert_eq!(
        sim.editor.findings(),
        Some(&[Finding::CableSwap(Electrode::RA, Electrode::LA)][..])
    );

    // "fit leads" restores the standard strip
    sim.editor.apply(EditorCommand::Reset);
    sim.run_ms(3000);
    for (a, r) in sim.last(Lead::I, 1.0).iter().zip(&sim.reference(Lead::I, 1.0)) {
        assert!((a - r).abs() < 1e-4);
    }
}

#[test]
fn extra_leads_are_recorded_and_placement_reloads_from_the_scenario() {
    let mut sim = Sim::new();
    sim.editor
        .apply(EditorCommand::Preset(heartsim_ecg::Preset::Posterior));
    sim.run_ms(3000);
    let extras: Vec<_> = sim.editor.extra_rings().map(|(x, r)| (x, r.len())).collect();
    assert_eq!(extras.len(), 3);
    for (x, n) in &extras {
        assert!(*n > 1000, "{} recorded {n} samples", x.label());
    }

    let mut s = Scenario::sinus_normal();
    sim.editor
        .apply(EditorCommand::Swap(Electrode::LA, Electrode::LL));
    sim.editor.save_scenario(&mut s);
    let text = s.to_ron().unwrap();
    let mut other = Sim::new();
    other.editor.load_scenario(&Scenario::from_ron(&text).unwrap());
    assert_eq!(other.editor.placement(), sim.editor.placement());
}

#[test]
fn panel_presets_and_exercises_edit_the_placement() {
    let editor = ElectrodeEditor::new(pack().electrodes.clone(), Some(&pack().torso), RATE_HZ, 1000);
    let mut h = Harness::new_ui_state(
        |ui, ed: &mut ElectrodeEditor| electrode_editor_panel(ui, ed),
        editor,
    );
    h.get_by_label("Mason–Likar").click();
    h.run();
    assert_eq!(
        h.state().placement().limb_of(Electrode::RA),
        None,
        "RA on the torso"
    );
    h.get_by_label("Fit leads").click();
    h.run();
    assert_eq!(h.state().placement().limb_of(Electrode::RA), Some(Limb::RightArm));
    h.get_by_label("V4R").click();
    h.run();
    assert_eq!(h.state().placement().extras.len(), 1);
    h.get_by_label("New exercise").click();
    h.run();
    assert!(h.state().exercise().is_some());
    assert!(!h.state().revealed(), "the answer is hidden");
    h.get_by_label("Explain").click();
    h.run();
    assert!(h.state().revealed());
}
