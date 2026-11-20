//! B10 acceptance: the electrode editor drives the Tier-0 strip pipeline (timing engine →
//! synthesiser → ring buffers) the way the app does, and its panel edits the placement.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glam::Vec3;
use heartsim_core::{Electrode, LEAD_COUNT, Lead, Millis, Scenario};
use heartsim_ecg::placement::LIMB_CAPTURE_MM;
use heartsim_ecg::{Cable, Finding, GaussianSynth, LeadRing, Limb};
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

fn editor() -> ElectrodeEditor {
    ElectrodeEditor::new(pack().electrodes.clone(), Some(&pack().torso), RATE_HZ, 1000)
}

/// A drop on a limb stores the electrode at the limb's anchor (the ECG sees a limb electrode) and
/// remembers the spot for drawing; the leads equal those of "back on limb".
#[test]
fn a_limb_drop_records_the_spot_and_keeps_the_anchor() {
    let mut ed = editor();
    let ra = Cable::Standard(Electrode::RA);
    ed.apply(EditorCommand::Move(ra, pack().electrodes.get("RA").unwrap()));
    let spot = Vec3::new(-300.0, -100.0, 20.0);
    ed.place_on_limb(ra, Limb::RightArm, spot);
    assert_eq!(ed.placement().position(Electrode::RA), Limb::RightArm.anchor_mm());
    assert_eq!(ed.limb_spot(ra), Some(spot));
    let dropped = ed.lead_vectors();

    let mut other = editor();
    other.apply(EditorCommand::Move(ra, pack().electrodes.get("RA").unwrap()));
    other.apply(EditorCommand::PutOnLimb(Electrode::RA));
    assert_eq!(other.limb_spot(ra), None, "PutOnLimb has no spot");
    assert_eq!(other.lead_vectors(), dropped);

    // a second drop elsewhere on the arm only moves the spot
    let spot2 = Vec3::new(-250.0, 50.0, 0.0);
    ed.place_on_limb(ra, Limb::RightArm, spot2);
    assert_eq!(ed.limb_spot(ra), Some(spot2));
    ed.apply(EditorCommand::PutOnLimb(Electrode::RA));
    assert_eq!(ed.limb_spot(ra), None);
}

#[test]
fn reset_preset_swap_and_scenarios_clear_limb_spots() {
    let ll = Cable::Standard(Electrode::LL);
    let drop = |ed: &mut ElectrodeEditor| {
        ed.place_on_limb(ll, Limb::LeftLeg, Vec3::new(80.0, -700.0, 0.0));
        assert!(ed.limb_spot(ll).is_some());
    };
    for cmd in [
        EditorCommand::Reset,
        EditorCommand::Preset(heartsim_ecg::Preset::Posterior),
        EditorCommand::Swap(Electrode::LL, Electrode::RL),
        EditorCommand::Exercise(heartsim_ecg::Exercise::V1V2Reversed),
        EditorCommand::RandomExercise,
    ] {
        let mut ed = editor();
        drop(&mut ed);
        ed.apply(cmd);
        assert_eq!(ed.limb_spot(ll), None, "{cmd:?}");
    }
    let mut ed = editor();
    drop(&mut ed);
    ed.load_scenario(&Scenario::sinus_normal());
    assert_eq!(ed.limb_spot(ll), None, "load_scenario");
}

#[test]
fn dragging_back_to_the_trunk_clears_the_spot_and_never_reads_as_a_limb() {
    let mut ed = editor();
    let la = Cable::Standard(Electrode::LA);
    ed.place_on_limb(la, Limb::LeftArm, Vec3::new(240.0, -300.0, 20.0));
    assert!(ed.limb_spot(la).is_some());
    ed.drag_to(la, Vec3::new(60.0, 40.0, 100.0));
    assert_eq!(ed.limb_spot(la), None);
    assert_eq!(ed.placement().limb_of(Electrode::LA), None);
    // even a pick right at an anchor stays a torso point
    for l in Limb::ALL {
        for off in [Vec3::ZERO, Vec3::new(5.0, 0.0, 0.0)] {
            ed.drag_to(la, l.anchor_mm() + off);
            let p = ed.placement().position(Electrode::LA);
            assert_eq!(Limb::at(p), None, "{l:?}: {p}");
            assert!(p.distance(l.anchor_mm()) <= LIMB_CAPTURE_MM + 2.0);
        }
    }
}

/// Exercises move electrodes along the pack torso the sites are defined on, not along the figure
/// they are dragged on: the placement, and with it the ECG, is the same with and without a figure.
#[test]
fn exercises_do_not_depend_on_the_drag_surface() {
    use heartsim_ecg::{Exercise, TorsoSurface};
    let body = pack().body.as_ref().expect("default pack ships human.glb");
    let torso = TorsoSurface::new(&pack().torso);
    let figure = TorsoSurface::with_axis(body, torso.axis());
    let mut on_figure = ElectrodeEditor::on_surface(pack().electrodes.clone(), Some(figure), RATE_HZ, 1000)
        .with_exercise_surface(Some(torso));
    let mut on_torso = editor();
    for x in Exercise::ALL {
        on_figure.apply(EditorCommand::Exercise(x));
        on_torso.apply(EditorCommand::Exercise(x));
        assert_eq!(on_figure.placement(), on_torso.placement(), "{}", x.label());
    }
}
