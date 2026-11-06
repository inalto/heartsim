//! B22 acceptance tests on the engine side (Tier 1): lesions and their persistence, RF, programmed
//! stimulation, ERP search, induction and ablation of AVNRT and AVRT, entrainment. The electrogram
//! checks are in `heartsim-ecg/tests/ep_lab.rs`, flutter (Tier 2) in `tests/ep_flutter.rs`.
//!
//! Reference ranges: see the module documentation of `heartsim_sim::ep`.

use glam::Vec3;
use heartsim_core::{Millis, Region};
use heartsim_geometry::AnatomyPack;
use heartsim_sim::ep::{
    AblationTarget, EpLab, EpPreset, ErpTarget, Protocol, ProtocolOutcome, ProtocolStatus, RfParams,
};
use heartsim_sim::{Engine, Event, EventKind, Origin};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

fn assets() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

fn pack() -> &'static AnatomyPack {
    static PACK: OnceLock<AnatomyPack> = OnceLock::new();
    PACK.get_or_init(|| AnatomyPack::load(&assets().join("anatomy/default")).expect("default pack loads"))
}

fn lab(id: &str) -> EpLab {
    let path = assets().join(format!("scenarios/ep/{id}.ron"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let preset = EpPreset::from_ron(&text)
        .unwrap_or_else(|e| panic!("{id}: {e}"))
        .unwrap_or_else(|| panic!("{id}: no ep section"));
    preset.build(
        Arc::new(pack().cells.clone()),
        &pack().landmarks.sets,
        &pack().aha_segments,
    )
}

fn cell(name: &str) -> u32 {
    pack().cells.landmarks[name]
}

fn position(c: u32) -> Vec3 {
    pack().cells.positions[c as usize]
}

/// Atrial cell the HRA pair would pace.
fn hra_cell(l: &EpLab) -> u32 {
    l.capture_cell(position(cell("sa_node")))
        .expect("tissue at the HRA")
}

fn run(l: &mut EpLab, ms: f64) -> Vec<Event> {
    let end = l.now().0 + ms;
    let mut all = Vec::new();
    while l.now().0 + 0.5 < end {
        l.step(Millis(1.0));
        all.extend(l.drain_events());
    }
    all
}

fn run_until_done(l: &mut EpLab, limit_ms: f64) -> Vec<Event> {
    let end = l.now().0 + limit_ms;
    let mut all = Vec::new();
    while l.protocol_status() != ProtocolStatus::Finished && l.now().0 < end {
        l.step(Millis(1.0));
        all.extend(l.drain_events());
    }
    assert_eq!(
        l.protocol_status(),
        ProtocolStatus::Finished,
        "protocol still running"
    );
    all
}

fn count(evs: &[Event], region: Region, kind: EventKind) -> usize {
    evs.iter()
        .filter(|e| e.region == region && e.kind == kind)
        .count()
}

fn his_rate_bpm(evs: &[Event], ms: f64) -> f64 {
    count(evs, Region::HisBundle, EventKind::Conducted) as f64 * 60_000.0 / ms
}

// --- lesions -----------------------------------------------------------------------------------

#[test]
fn a_lesion_persists_across_an_engine_reset() {
    let mut l = lab("ep_baseline");
    let c = cell("ra_lateral");
    let far = cell("la_roof");
    l.ablate_now(position(c), 8.0);
    assert!(l.engine().tier1().eikonal().ablated()[c as usize]);
    l.reset();
    assert_eq!(l.now(), Millis::ZERO);
    assert_eq!(l.lesions().len(), 1, "the book survives");
    assert!(
        l.engine().tier1().eikonal().ablated()[c as usize],
        "the cells survive"
    );
    run(&mut l, 1500.0);
    let t = &l.cells().activation_ms;
    assert!(
        t[c as usize].is_nan(),
        "lesion cell activated at {}",
        t[c as usize]
    );
    assert!(
        t[far as usize].is_finite(),
        "the rest of the atria still activates"
    );
}

#[test]
fn an_av_node_lesion_stays_after_a_reset_and_leaves_an_escape_rhythm() {
    let mut l = lab("ep_baseline");
    assert_eq!(l.ablate_target(AblationTarget::AvNode), 1);
    let check = |l: &mut EpLab| {
        let evs = run(l, 8000.0);
        let conducted_by_node = evs
            .iter()
            .filter(|e| e.region == Region::HisBundle && e.origin == Origin::Sinus)
            .count();
        assert_eq!(conducted_by_node, 0, "a sinus beat crossed the ablated node");
        let rate = his_rate_bpm(&evs, 8000.0);
        assert!((30.0..60.0).contains(&rate), "junctional escape at {rate} bpm");
        assert!(count(&evs, Region::AvNodeCompact, EventKind::Blocked) > 5);
    };
    check(&mut l);
    l.reset();
    check(&mut l);
}

#[test]
fn rf_grows_a_lesion_with_its_time_constant_and_keeps_it() {
    let mut l = lab("ep_baseline");
    let tau_ms = 2000.0;
    *l.rf_params_mut() = RfParams {
        tau_ms,
        ..RfParams::default()
    };
    let c = cell("ra_lateral");
    let tip = position(c);
    l.rf_on(tip);
    run(&mut l, tau_ms);
    let r = l.rf_readout().expect("RF on");
    let expected = RfParams::default().max_radius_mm * (1.0 - (-1.0f32).exp());
    assert!((r.lesion_radius_mm - expected).abs() < 0.1, "{r:?}");
    assert!(r.temperature_c > 50.0, "{r:?}");
    let radius = r.lesion_radius_mm;
    run(&mut l, 2.0 * tau_ms);
    l.rf_off();
    assert!(l.rf_readout().is_none());
    let final_r = l.lesions().lesions()[0].radius_mm;
    assert!(final_r > radius, "{final_r} after {radius}");
    let dead = l.engine().tier1().eikonal().ablated();
    let cloud = l.cloud();
    let mid = (0..cloud.len()).filter(|&i| cloud.positions[i].distance(tip) < final_r - 0.3);
    for i in mid {
        assert!(dead[i], "cell {i} inside the lesion still conducts");
    }
    assert!(!dead[cell("la_roof") as usize]);
}

#[test]
fn rf_off_contact_or_stopped_too_soon_makes_no_lesion() {
    let mut l = lab("ep_baseline");
    // the middle of the right atrium: nothing within reach
    let centre = heartsim_geometry::CatheterAnatomy::from_cloud(&pack().cells)
        .cavity(heartsim_geometry::CardiacChamber::RightAtrium)
        .centre();
    l.rf_on(centre);
    run(&mut l, 2000.0);
    assert_eq!(l.rf_readout().unwrap().contact, 0.0);
    l.rf_off();
    assert!(l.lesions().is_empty());
    // full contact, 100 ms: the tissue never reached the injury temperature
    l.rf_on(position(cell("ra_lateral")));
    run(&mut l, 100.0);
    l.rf_off();
    assert!(l.lesions().is_empty());
}

// --- stimulation --------------------------------------------------------------------------------

#[test]
fn a_catheter_stimulus_is_decided_by_the_timing_engine_and_starts_the_front_at_its_own_cell() {
    let mut l = lab("ep_baseline");
    let c = cell("ra_lateral");
    run(&mut l, 1000.0);
    l.stimulate(c).unwrap();
    let t0 = l.now().0;
    let evs = run(&mut l, 30.0);
    assert!(
        evs.iter()
            .any(|e| e.kind == EventKind::Stimulus && e.origin == Origin::External),
        "not captured"
    );
    let t = f64::from(l.cells().activation_ms[c as usize]);
    assert!((t - t0).abs() <= 1.0, "site activated at {t}, stimulus at {t0}");
    // a stimulus inside the atrial ERP is refused, and then no new front starts at the site
    l.stimulate(c).unwrap();
    let evs = run(&mut l, 30.0);
    assert!(
        evs.iter()
            .any(|e| e.kind == EventKind::Blocked && e.origin == Origin::External),
        "captured inside the ERP"
    );
    assert_eq!(
        f64::from(l.cells().activation_ms[c as usize]),
        t,
        "front started anyway"
    );
}

#[test]
fn a_pair_in_the_blood_pool_captures_nothing() {
    let l = lab("ep_baseline");
    let centre = heartsim_geometry::CatheterAnatomy::from_cloud(&pack().cells)
        .cavity(heartsim_geometry::CardiacChamber::RightAtrium)
        .centre();
    assert_eq!(l.capture_cell(centre), None);
    assert!(l.capture_cell(position(cell("rv_apex"))).is_some());
}

#[test]
fn the_his_catheter_captures_the_bundle_and_the_rva_the_ventricle() {
    let mut l = lab("ep_baseline");
    run(&mut l, 1000.0);
    let his = l.capture_cell(position(cell("his"))).unwrap();
    assert_eq!(pack().cells.regions[his as usize], Region::HisBundle);
    l.stimulate(his).unwrap();
    let evs = run(&mut l, 100.0);
    assert!(
        evs.iter()
            .any(|e| e.region == Region::HisBundle && e.origin == Origin::External)
    );
    run(&mut l, 900.0);
    let rv = l.capture_cell(position(cell("rv_apex"))).unwrap();
    l.stimulate(rv).unwrap();
    let evs = run(&mut l, 50.0);
    assert!(
        evs.iter()
            .any(|e| e.chamber == heartsim_sim::Chamber::Ventricle && e.kind == EventKind::Stimulus)
    );
}

#[test]
fn a_train_delivers_its_stimuli_and_captures_at_a_safe_rate() {
    let mut l = lab("ep_baseline");
    let c = hra_cell(&l);
    run(&mut l, 1000.0);
    l.start_protocol(
        c,
        Protocol::Train {
            s1_cycle_ms: 600.0,
            s1_count: 8,
            s2_ms: None,
            s3_ms: None,
        },
    );
    let evs = run_until_done(&mut l, 10_000.0);
    assert_eq!(
        l.protocol_outcome(),
        Some(&ProtocolOutcome::Delivered { stimuli: 8 })
    );
    let captured = evs
        .iter()
        .filter(|e| e.kind == EventKind::Stimulus && e.origin == Origin::External)
        .count();
    assert!(captured >= 7, "{captured} of 8");
    assert_eq!(l.take_stimuli().len(), 8);
}

#[test]
fn decremental_atrial_pacing_reaches_av_nodal_block() {
    let mut l = lab("ep_baseline");
    let c = hra_cell(&l);
    run(&mut l, 1000.0);
    l.start_protocol(
        c,
        Protocol::Decremental {
            start_cycle_ms: 600.0,
            end_cycle_ms: 280.0,
            step_ms: 40.0,
            beats: 6,
        },
    );
    let evs = run_until_done(&mut l, 30_000.0);
    assert!(
        count(&evs, Region::AvNodeCompact, EventKind::Blocked) > 0,
        "no AV-nodal block down to 280 ms"
    );
}

#[test]
fn continuous_pacing_runs_until_stopped() {
    let mut l = lab("ep_baseline");
    let c = cell("rv_apex");
    l.start_protocol(c, Protocol::Continuous { cycle_ms: 500.0 });
    run(&mut l, 3100.0);
    assert!(matches!(l.protocol_status(), ProtocolStatus::Pacing { .. }));
    l.stop_protocol();
    assert_eq!(l.protocol_status(), ProtocolStatus::Finished);
    assert!(
        matches!(l.protocol_outcome(), Some(ProtocolOutcome::Delivered { stimuli }) if *stimuli >= 6),
        "{:?}",
        l.protocol_outcome()
    );
}

// --- ERP search --------------------------------------------------------------------------------

fn erp(l: &mut EpLab, cell: u32, target: ErpTarget) -> heartsim_sim::ep::ErpResult {
    run(l, 1000.0);
    l.start_protocol(cell, Protocol::erp_search(target));
    run_until_done(l, 400_000.0);
    match l.protocol_outcome() {
        Some(ProtocolOutcome::Erp(e)) => e.clone(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn atrial_av_nodal_and_ventricular_erps_are_found_in_the_normal_ranges() {
    let mut l = lab("ep_baseline");
    let hra = hra_cell(&l);
    let atrium = erp(&mut l, hra, ErpTarget::Atrium);
    let a = atrium.erp_ms.expect("atrial ERP found");
    assert!((170.0..=300.0).contains(&a), "atrial ERP {a}");
    assert!(atrium.curve.len() > 3, "{:?}", atrium.curve);

    let mut l = lab("ep_baseline");
    let node = erp(&mut l, hra, ErpTarget::AvNode);
    let n = node
        .erp_ms
        .unwrap_or_else(|| panic!("AV-nodal ERP not found: {node:?}"));
    assert!((230.0..=425.0).contains(&n), "AV-nodal ERP {n}");

    let mut l = lab("ep_baseline");
    let rv = l.capture_cell(position(cell("rv_apex"))).unwrap();
    let ventricle = erp(&mut l, rv, ErpTarget::Ventricle);
    let v = ventricle.erp_ms.expect("ventricular ERP found");
    assert!((170.0..=290.0).contains(&v), "ventricular ERP {v}");
}

#[test]
fn the_erp_found_is_the_engine_parameter_it_should_measure() {
    let mut l = lab("ep_baseline");
    let hra = hra_cell(&l);
    let atrium = erp(&mut l, hra, ErpTarget::Atrium);
    let parameter = l.timing().params.atrial_erp_ms;
    let a = atrium.erp_ms.unwrap();
    assert!(
        a < parameter && parameter <= a + 10.0,
        "ERP {a} for parameter {parameter}"
    );
}

// --- AVNRT --------------------------------------------------------------------------------------

/// S1×8 at 600 ms, S2 at `s2`, from the HRA; returns the His rate in the 3 s after S2.
fn induce(l: &mut EpLab, s2: f64) -> f64 {
    let c = hra_cell(l);
    run(l, 1000.0);
    l.start_protocol(
        c,
        Protocol::Train {
            s1_cycle_ms: 600.0,
            s1_count: 8,
            s2_ms: Some(s2),
            s3_ms: None,
        },
    );
    run_until_done(l, 20_000.0);
    run(l, 500.0);
    let evs = run(l, 3000.0);
    his_rate_bpm(&evs, 3000.0)
}

#[test]
fn dual_pathways_show_a_jump_and_s2_induces_avnrt() {
    let mut l = lab("ep_avnrt");
    let hra = hra_cell(&l);
    let e = erp(&mut l, hra, ErpTarget::FastPathway);
    let (coupling, jump) = e
        .ah_jump
        .unwrap_or_else(|| panic!("no A2-H2 jump: {:?}", e.curve));
    assert!(jump >= 50.0, "jump {jump} at {coupling}");
    let fast_erp = e.erp_ms.expect("fast-pathway ERP");
    assert_eq!(fast_erp, coupling, "the jump is where the fast pathway fails");

    let mut l = lab("ep_avnrt");
    let rate = induce(&mut l, fast_erp);
    assert!((120.0..=220.0).contains(&rate), "AVNRT at {rate} bpm");

    let mut l = lab("ep_avnrt");
    let rate = induce(&mut l, fast_erp + 60.0);
    assert!(rate < 110.0, "a late S2 induced {rate} bpm");
}

#[test]
fn slow_pathway_ablation_stops_induction_and_spares_av_conduction() {
    let mut l = lab("ep_avnrt");
    let hra = hra_cell(&l);
    let e = erp(&mut l, hra, ErpTarget::FastPathway);
    let coupling = e.erp_ms.unwrap();

    let mut l = lab("ep_avnrt");
    assert_eq!(l.ablate_target(AblationTarget::SlowPathway), 1);
    assert!(l.timing().params.av.slow.blocked, "slow pathway not hit");
    assert!(!l.timing().params.av.complete_block, "compact node damaged");
    assert!(!l.timing().params.av.fast.blocked, "fast pathway damaged");
    assert!(l.goal_reached());
    let rate = induce(&mut l, coupling);
    assert!(rate < 110.0, "still inducible: {rate} bpm");
    // and sinus beats still reach the His bundle
    let evs = run(&mut l, 4000.0);
    assert!(his_rate_bpm(&evs, 4000.0) > 50.0);
}

// --- accessory pathway --------------------------------------------------------------------------

#[test]
fn a_premature_stimulus_induces_avrt_and_ablating_the_pathway_ends_it_for_good() {
    let mut l = lab("ep_ap_left_lateral");
    let rate = induce(&mut l, 280.0);
    assert!((130.0..=260.0).contains(&rate), "AVRT at {rate} bpm");
    assert!(!l.goal_reached());

    let n = l.ablate_target(AblationTarget::AccessoryPathway);
    assert_eq!(n, 1);
    assert!(l.timing().params.accessory.is_none());
    assert!(l.goal_reached());
    run(&mut l, 1000.0);
    let evs = run(&mut l, 4000.0);
    let rate = his_rate_bpm(&evs, 4000.0);
    assert!(rate < 110.0, "tachycardia went on at {rate} bpm");

    let mut l2 = l.clone();
    l2.reset();
    assert!(
        l2.timing().params.accessory.is_none(),
        "the pathway came back after a reset"
    );
    let rate = induce(&mut l2, 280.0);
    assert!(rate < 110.0, "still inducible after a reset: {rate} bpm");
}

#[test]
fn entrainment_from_the_ventricle_reads_tcl_and_ppi_during_avrt() {
    let mut l = lab("ep_ap_left_lateral");
    let tcl_rate = induce(&mut l, 280.0);
    assert!(tcl_rate > 130.0, "{tcl_rate}");
    let rv = l.capture_cell(position(cell("rv_apex"))).unwrap();
    l.start_protocol(
        rv,
        Protocol::Entrainment {
            offset_ms: 20.0,
            count: 8,
        },
    );
    run_until_done(&mut l, 30_000.0);
    let Some(ProtocolOutcome::Entrainment(e)) = l.protocol_outcome() else {
        panic!("{:?}", l.protocol_outcome());
    };
    assert_eq!(
        e.captured, e.delivered,
        "every stimulus must capture to entrain: {e:?}"
    );
    let tcl_expected = 60_000.0 / tcl_rate;
    assert!((e.tcl_ms - tcl_expected).abs() < 40.0, "{e:?} for {tcl_expected}");
    assert!(e.ppi_ms >= e.tcl_ms - 20.0, "{e:?}");
    assert!(
        e.ppi_ms - e.tcl_ms <= 115.0,
        "AVRT should stay under Michaud's 115 ms: {e:?}"
    );
}

#[test]
fn entrainment_without_a_rhythm_to_entrain_reports_no_return() {
    let mut l = lab("ep_baseline");
    // a site that is never activated: inside a lesion
    let c = cell("ra_lateral");
    l.ablate_now(position(c), 6.0);
    l.start_protocol(
        c,
        Protocol::Entrainment {
            offset_ms: 20.0,
            count: 3,
        },
    );
    run_until_done(&mut l, 20_000.0);
    assert_eq!(l.protocol_outcome(), Some(&ProtocolOutcome::NoReturn));
}

// --- ablation lines on Tier 1 --------------------------------------------------------------------

#[test]
fn the_cti_line_kills_the_isthmus_on_tier_1_and_sinus_rhythm_still_conducts() {
    let mut l = lab("ep_baseline");
    let n = l.ablate_target(AblationTarget::CtiLine);
    assert!(n >= 3, "{n} lesions");
    let cloud = Arc::clone(l.cloud());
    let isthmus: Vec<usize> = (0..cloud.len())
        .filter(|&i| cloud.regions[i] == Region::CavotricuspidIsthmus)
        .collect();
    assert_ne!(isthmus.len(), 0);
    let dead = l.engine().tier1().eikonal().ablated();
    assert!(isthmus.iter().all(|&i| dead[i]), "a gap in the line");
    let evs = run(&mut l, 3000.0);
    assert!(count(&evs, Region::HisBundle, EventKind::Conducted) >= 2);
    let t = &l.cells().activation_ms;
    assert!(isthmus.iter().all(|&i| t[i].is_nan()));
    assert!(t[cell("la_roof") as usize].is_finite());
}

#[test]
fn pulmonary_vein_isolation_is_recognised_by_coverage_of_the_veins() {
    let mut l = lab("ep_af_pvi");
    assert!(!l.goal_reached());
    let sites = l.target_sites(AblationTarget::PulmonaryVeins);
    assert!(sites.len() >= 4, "{} stops", sites.len());
    // part of the ring is not enough
    l.ablate_now(sites[0], 4.0);
    assert!(!l.goal_reached());
    l.ablate_target(AblationTarget::PulmonaryVeins);
    assert!(l.goal_reached());
}

// --- determinism ---------------------------------------------------------------------------------

#[test]
fn a_procedure_is_deterministic() {
    let once = || {
        let mut l = lab("ep_avnrt");
        let c = hra_cell(&l);
        run(&mut l, 500.0);
        l.start_protocol(
            c,
            Protocol::Train {
                s1_cycle_ms: 500.0,
                s1_count: 4,
                s2_ms: Some(300.0),
                s3_ms: None,
            },
        );
        let evs = run(&mut l, 3000.0);
        l.ablate_target(AblationTarget::SlowPathway);
        let more = run(&mut l, 1000.0);
        (evs, more, l.cells().vm.clone())
    };
    assert_eq!(once(), once());
}
