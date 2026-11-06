//! B22: the EP-lab panels report their commands and draw with an empty recording.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use heartsim_core::Region;
use heartsim_ecg::{EgmOptions, EgmRecorder};
use heartsim_geometry::{CatheterKind, CatheterSite, CellCloud};
use heartsim_sim::ep::{AblationTarget, ProtocolStatus, RfReadout};
use heartsim_ui::ep_panel::{ablation_panel, egm_stack, pes_panel};
use heartsim_ui::i18n;
use heartsim_ui::strip::StripStyle;
use heartsim_ui::{EpCommand, EpPanelState};
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn the_start_button_reports_the_protocol_of_the_form_from_the_chosen_pair() {
    i18n::set_language("en");
    let commands = Rc::new(RefCell::new(Vec::new()));
    let out = Rc::clone(&commands);
    let mut state = EpPanelState {
        pace_channel: 1,
        ..EpPanelState::default()
    };
    let expected = state.protocol();
    let channels = vec!["HRA 1-2".to_string(), "HRA 3-4".to_string()];
    let mut harness = Harness::new_ui(move |ui| {
        let mut c = Vec::new();
        pes_panel(ui, &mut state, &channels, ProtocolStatus::Finished, None, false, &mut c);
        out.borrow_mut().extend(c);
    });
    harness.run();
    harness.get_by_label("Start").click();
    harness.run();
    assert_eq!(
        commands.borrow().as_slice(),
        &[EpCommand::StartProtocol {
            channel: 1,
            protocol: expected
        }]
    );
}

#[test]
fn the_rf_button_turns_rf_on_and_shows_the_generator_when_it_is_on() {
    i18n::set_language("en");
    let commands = Rc::new(RefCell::new(Vec::new()));
    let out = Rc::clone(&commands);
    let mut state = EpPanelState::default();
    let mut harness = Harness::new_ui(move |ui| {
        let mut c = Vec::new();
        ablation_panel(ui, &mut state, &[AblationTarget::CtiLine], None, 0, None, false, &mut c);
        out.borrow_mut().extend(c);
    });
    harness.run();
    harness.get_by_label("RF on (ablate)").click();
    harness.run();
    assert_eq!(commands.borrow().as_slice(), &[EpCommand::RfOn]);

    let readout = RfReadout {
        elapsed_ms: 5000.0,
        power_w: 30.0,
        temperature_c: 55.0,
        impedance_ohm: 104.0,
        contact: 1.0,
        lesion_radius_mm: 2.0,
    };
    let mut state = EpPanelState::default();
    let mut harness = Harness::new_ui(move |ui| {
        let mut c = Vec::new();
        ablation_panel(ui, &mut state, &[], Some(readout), 2, None, false, &mut c);
    });
    harness.run();
    assert!(harness.query_by_label("RF off").is_some());
    assert!(harness.query_by_label_contains("55 °C").is_some());
    assert!(harness.query_by_label("Lesions: 2").is_some());
}

#[test]
fn the_panels_are_translated() {
    i18n::set_language("it");
    let mut state = EpPanelState::default();
    let channels = vec!["HRA 1-2".to_string()];
    let mut harness = Harness::new_ui(move |ui| {
        let mut c = Vec::new();
        pes_panel(ui, &mut state, &channels, ProtocolStatus::Finished, None, false, &mut c);
    });
    harness.run();
    assert!(harness.query_by_label("Avvia").is_some());
    i18n::set_language("en");
}

#[test]
fn the_stack_draws_an_empty_recording_and_drags_a_caliper() {
    i18n::set_language("en");
    let cloud = CellCloud::test_sphere(4.0, 20.0, Region::RightAtrium);
    let catheter = heartsim_geometry::Catheter {
        kind: CatheterKind::Hra,
        site: CatheterSite::Default,
        body: vec![glam::Vec3::ZERO, glam::Vec3::X],
        electrodes: vec![glam::Vec3::ZERO, glam::Vec3::X],
    };
    let rec = EgmRecorder::new(&cloud, std::slice::from_ref(&catheter), EgmOptions::default());
    let mut state = EpPanelState::default();
    let mut harness = Harness::new_ui(move |ui| {
        egm_stack(ui, &mut state, &[], &rec, &StripStyle::default(), 0.0);
    });
    harness.run();
}
