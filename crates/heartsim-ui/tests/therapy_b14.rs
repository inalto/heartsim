//! B14 acceptance, UI side: the drug tray, the ischaemia panel and the defibrillator only
//! produce scenario `Action`s (so timelines and tutorials can do the same), and what they show
//! comes back from the engine.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use heartsim_core::{Action, Millis};
use heartsim_sim::defib::DefibState;
use heartsim_sim::ischaemia::{IschaemiaPreset, IschaemiaState};
use heartsim_sim::pharma::{ActiveDrug, DrugTable};
use heartsim_sim::timing::TimingParams;
use heartsim_sim::{Engine, TimingEngine};
use heartsim_ui::{
    DefibPanelState, DefibSound, IschaemiaPanelState, defib_panel, drug_tray, ischaemia_panel,
};

#[test]
fn drug_tray_doses_go_through_the_engine() {
    struct Model {
        engine: TimingEngine,
        active: Vec<ActiveDrug>,
        sent: Vec<Action>,
    }
    let table = DrugTable::builtin();
    let mut h = Harness::new_ui_state(
        move |ui, m: &mut Model| {
            let actions = drug_tray(ui, &table, &m.active);
            for a in &actions {
                m.engine.apply(a, m.engine.now()).unwrap();
            }
            m.sent.extend(actions);
        },
        Model {
            engine: TimingEngine::new(TimingParams::default()),
            active: Vec::new(),
            sent: Vec::new(),
        },
    );
    // adenosine offers ×1 and ×2: the first "×1" button belongs to it (table order)
    h.get_all_by_label("×1").next().unwrap().click();
    h.run();
    assert_eq!(
        h.state().sent,
        vec![Action::Drug {
            name: "adenosine".into(),
            dose: 1.0
        }]
    );
    let m = h.state_mut();
    for _ in 0..2000 {
        m.engine.step(Millis(1.0));
    }
    m.active = m.engine.pharmacology().active(m.engine.now().0);
    assert_eq!(m.active.len(), 1);
    h.run();
    h.get_by_label("Stop").click();
    h.run();
    let m = h.state_mut();
    m.engine.step(Millis(1.0));
    assert_eq!(
        m.engine.pharmacology().active(m.engine.now().0).len(),
        0,
        "stopped"
    );
}

#[test]
fn ischaemia_presets_and_apply_issue_stenosis_actions() {
    struct Model {
        panel: IschaemiaPanelState,
        lesions: IschaemiaState,
        highlight: Option<heartsim_sim::ischaemia::CoronaryBranch>,
    }
    let mut h = Harness::new_ui_state(
        |ui, m: &mut Model| {
            let r = ischaemia_panel(ui, &mut m.panel, m.lesions.lesions());
            for a in r.actions {
                let Action::Stenosis { branch, severity } = a else {
                    panic!("only stenosis actions")
                };
                m.lesions.apply(&branch, severity).unwrap();
            }
            m.highlight = r.highlight;
        },
        Model {
            panel: IschaemiaPanelState::default(),
            lesions: IschaemiaState::default(),
            highlight: None,
        },
    );
    h.run();
    assert_eq!(
        h.state().highlight,
        Some(heartsim_sim::ischaemia::CoronaryBranch::Lad)
    );
    h.get_by_label(IschaemiaPreset::InferiorStemi.label()).click();
    h.run();
    let l = h.state().lesions.lesions();
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].branch.id(), "RCA");
    h.get_by_label("Apply").click();
    h.run();
    assert_eq!(h.state().lesions.lesions().len(), 2, "LAD 0.9 added");
    h.get_by_label("Clear all").click();
    h.run();
    assert!(h.state().lesions.is_empty());
}

#[test]
fn defibrillator_charges_then_shocks_on_the_r_wave() {
    struct Model {
        panel: DefibPanelState,
        engine: TimingEngine,
        sounds: Vec<DefibSound>,
    }
    let mut h = Harness::new_ui_state(
        |ui, m: &mut Model| {
            let now = m.engine.now().0;
            let armed = m.engine.shock_armed();
            let r = defib_panel(ui, &mut m.panel, now, armed);
            if let Some(a) = r.action {
                m.engine.apply(&a, m.engine.now()).unwrap();
            }
            m.sounds.extend(r.sounds);
        },
        Model {
            panel: DefibPanelState::default(),
            engine: TimingEngine::new(TimingParams::default()),
            sounds: Vec::new(),
        },
    );
    h.get_by_label("SYNC").click();
    h.get_by_label("CHARGE").click();
    h.run();
    assert!(matches!(h.state().panel.defib.state, DefibState::Charging { .. }));
    // 150 J charge in 3.75 s of simulated time
    for _ in 0..4000 {
        h.state_mut().engine.step(Millis(1.0));
    }
    h.run();
    assert_eq!(h.state().panel.defib.state, DefibState::Ready);
    assert!(h.state().sounds.contains(&DefibSound::Charged));
    h.get_by_label("SHOCK").click();
    h.run();
    assert!(h.state().engine.shock_armed(), "waits for the R wave");
    let m = h.state_mut();
    let mut shocks = Vec::new();
    for _ in 0..2000 {
        m.engine.step(Millis(1.0));
        shocks.extend(m.engine.drain_shocks());
    }
    assert_eq!(shocks.len(), 1);
    assert!(shocks[0].synchronised);
    assert_eq!(m.panel.on_shocks(&shocks), vec![DefibSound::Shock]);
}
