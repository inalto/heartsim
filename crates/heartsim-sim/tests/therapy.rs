//! Acceptance tests for brief B14 on the engines: drugs on rhythms (adenosine on AVNRT,
//! verapamil on the ventricular response to fibrillating atria), the drug table's actions on the
//! timing engine, and synchronised / unsynchronised shocks (VT on the Tier-1 heart → sinus).

use heartsim_core::scenario::{Action, StimSite};
use heartsim_core::{Millis, Parameters};
use heartsim_geometry::AnatomyPack;
use heartsim_sim::defib::{RhythmClass, ShockRecord};
use heartsim_sim::tier1::{CellKind, EikonalEngine, EikonalParams, Tier1Engine};
use heartsim_sim::timing::{Focus, FocusMode, FocusRegion, TimingParams};
use heartsim_sim::{Chamber, Engine, Event, EventKind, Origin, TimingEngine};
use std::path::Path;
use std::sync::Arc;

/// Run `engine` to `seconds`, applying each `(at_ms, action)` when the clock reaches it.
fn run_engine(
    e: &mut TimingEngine,
    seconds: f64,
    actions: &[(f64, Action)],
) -> (Vec<Event>, Vec<ShockRecord>) {
    let (mut evs, mut shocks) = (Vec::new(), Vec::new());
    let mut next = 0;
    while e.now().0 < seconds * 1000.0 {
        while let Some((at, action)) = actions.get(next)
            && *at <= e.now().0
        {
            e.apply(action, Millis(*at)).expect("action accepted");
            next += 1;
        }
        e.step(Millis(1.0));
        evs.extend(e.drain_events());
        shocks.extend(e.drain_shocks());
    }
    (evs, shocks)
}

fn run_with(params: TimingParams, seconds: f64, actions: &[(f64, Action)]) -> Vec<Event> {
    run_engine(&mut TimingEngine::new(params), seconds, actions).0
}

fn active(e: &Event) -> bool {
    e.kind != EventKind::Blocked
}

/// Ventricular activations (QRS onsets), fusion duplicates removed.
fn v_times(evs: &[Event]) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    for e in evs
        .iter()
        .filter(|e| e.chamber == Chamber::Ventricle && active(e))
    {
        if out.last().is_none_or(|l| e.at.0 - l > 150.0) {
            out.push(e.at.0);
        }
    }
    out
}

fn rate_bpm(v: &[f64], from: f64, to: f64) -> f64 {
    let n = v.iter().filter(|t| (from..to).contains(*t)).count();
    n as f64 * 60_000.0 / (to - from)
}

fn drug(name: &str, dose: f64) -> Action {
    Action::Drug {
        name: name.into(),
        dose,
    }
}

fn scenario_timing(id: &str) -> TimingParams {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../assets/scenarios/{id}.ron"));
    TimingParams::from_scenario_ron(&std::fs::read_to_string(path).unwrap()).unwrap()
}

// --- drugs -----------------------------------------------------------------------------------

#[test]
fn adenosine_terminates_avnrt_within_10_s() {
    // the B11 AVNRT preset: PAC at 3 s starts the loop
    let pac = (
        3000.0,
        Action::Stimulate {
            site: StimSite::Landmark("ra_appendage".into()),
            strength: 1.0,
            duration_ms: 2.0,
        },
    );
    let given = 15_000.0;
    let evs = run_with(
        scenario_timing("avnrt_150"),
        40.0,
        &[pac, (given, drug("adenosine", 1.0))],
    );
    let reentry = |e: &&Event| e.origin == Origin::Reentry && active(e);
    let before = evs.iter().filter(|e| e.at.0 < given).filter(reentry).count();
    assert!(
        before >= 20,
        "AVNRT running before adenosine ({before} re-entrant events)"
    );
    let last = evs
        .iter()
        .filter(reentry)
        .map(|e| e.at.0)
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(
        last < given + 10_000.0,
        "loop still running {:.0} ms after adenosine",
        last - given
    );
    // sinus rhythm afterwards
    let v = v_times(&evs);
    let r = rate_bpm(&v, given + 15_000.0, 40_000.0);
    assert!((55.0..=90.0).contains(&r), "post-adenosine rate {r:.0} bpm");
}

/// Fibrillating-atria stand-in for the timing engine: two independent (parasystolic) atrial foci
/// at 380 and 460 bpm bombard the AV node irregularly, as fibrillatory wavelets do.
fn fibrillating_atria() -> TimingParams {
    let focus = |rate_bpm, region| Focus {
        region,
        mode: FocusMode::Automatic {
            rate_bpm,
            entrance_block: true,
            exit_block_ratio: 0,
        },
        max_fires: 0,
    };
    TimingParams {
        atrial_erp_ms: 100.0,
        foci: vec![
            focus(380.0, FocusRegion::RightAtrium),
            focus(460.0, FocusRegion::LeftAtrium),
        ],
        ..TimingParams::default()
    }
}

#[test]
fn verapamil_slows_the_ventricular_response_by_20_percent() {
    let given = 60_000.0;
    let evs = run_with(fibrillating_atria(), 420.0, &[(given, drug("verapamil", 1.0))]);
    let v = v_times(&evs);
    let before = rate_bpm(&v, 10_000.0, given);
    // full effect from ≈ 90 s after the dose
    let after = rate_bpm(&v, given + 240_000.0, 420_000.0);
    assert!(
        before > 120.0,
        "rapid ventricular response before: {before:.0} bpm"
    );
    assert!(
        after <= 0.8 * before,
        "verapamil: {before:.0} → {after:.0} bpm (needs ≥ 20 % slower)"
    );
    // irregular response (no fixed ratio)
    let rr: Vec<f64> = v
        .windows(2)
        .map(|w| w[1] - w[0])
        .filter(|r| *r < 2000.0)
        .collect();
    let m = rr.iter().sum::<f64>() / rr.len() as f64;
    let sd = (rr.iter().map(|r| (r - m).powi(2)).sum::<f64>() / rr.len() as f64).sqrt();
    assert!(sd / m > 0.05, "irregularly irregular, CV {:.3}", sd / m);
}

#[test]
fn table_drugs_act_on_rate_and_conduction() {
    let pr = |evs: &[Event]| {
        let p: Vec<f64> = evs
            .iter()
            .filter(|e| e.chamber == Chamber::Atrium && e.origin == Origin::Sinus && active(e))
            .map(|e| e.at.0)
            .collect();
        let v = v_times(evs);
        let last_p = p.iter().rev().find(|t| **t < *v.last().unwrap()).unwrap();
        v.iter().find(|t| *t > last_p).unwrap() - last_p
    };
    let base = run_with(TimingParams::default(), 300.0, &[]);
    let base_rate = rate_bpm(&v_times(&base), 240_000.0, 300_000.0);
    let with = |name: &str| run_with(TimingParams::default(), 300.0, &[(1000.0, drug(name, 1.0))]);

    let atropine = with("atropine");
    let r = rate_bpm(&v_times(&atropine), 240_000.0, 300_000.0);
    assert!(r > 1.3 * base_rate, "atropine {base_rate:.0} → {r:.0} bpm");
    let adrenaline = with("adrenaline");
    let r = rate_bpm(&v_times(&adrenaline), 60_000.0, 90_000.0);
    assert!(r > 1.5 * base_rate, "adrenaline {base_rate:.0} → {r:.0} bpm");

    let verapamil = with("verapamil");
    assert!(
        pr(&verapamil) > pr(&base) + 30.0,
        "verapamil PR {} vs {}",
        pr(&verapamil),
        pr(&base)
    );
    let flecainide = with("flecainide");
    assert!(pr(&flecainide) > pr(&base) + 10.0, "flecainide PR");
    let hyper_k = with("hyperkalaemia");
    assert!(pr(&hyper_k) > pr(&base) + 15.0, "hyperkalaemia PR");

    // stopping an electrolyte disturbance: dose 0
    let mut e = TimingEngine::new(TimingParams::default());
    run_engine(&mut e, 120.0, &[(1000.0, drug("hyperkalaemia", 1.0))]);
    assert!(e.drug_effects().ventricular_apd_scale < 0.9);
    e.apply(&drug("hyperkalaemia", 0.0), e.now()).unwrap();
    e.step(Millis(1.0));
    assert_eq!(e.pharmacology().active(e.now().0).len(), 0);
    assert!((e.drug_effects().ventricular_apd_scale - 1.0).abs() < 1e-12);
}

#[test]
fn drugs_are_deterministic() {
    let actions = [(500.0, drug("digoxin", 1.0)), (2000.0, drug("adrenaline", 0.5))];
    let a = run_with(fibrillating_atria(), 30.0, &actions);
    let b = run_with(fibrillating_atria(), 30.0, &actions);
    assert_eq!(a, b);
}

// --- defibrillation ----------------------------------------------------------------------------

fn shock(joules: f64, synchronised: bool) -> Action {
    Action::Shock { joules, synchronised }
}

/// Monomorphic VT stand-in: an automatic LV focus at 170 bpm with entrance block.
fn vt() -> TimingParams {
    let mut p = TimingParams::default();
    p.foci.push(Focus {
        region: FocusRegion::LeftVentricle,
        mode: FocusMode::Automatic {
            rate_bpm: 170.0,
            entrance_block: true,
            exit_block_ratio: 0,
        },
        max_fires: 0,
    });
    p
}

#[test]
fn synchronised_shock_fires_on_the_r_wave() {
    let mut e = TimingEngine::new(vt());
    let (evs, shocks) = run_engine(&mut e, 6.0, &[(3000.0, shock(100.0, true))]);
    assert_eq!(shocks.len(), 1);
    let s = shocks[0];
    assert!(s.synchronised && !s.vulnerable);
    assert_eq!(s.rhythm, RhythmClass::VentricularTachycardia);
    let r = evs
        .iter()
        .filter(|e| e.chamber == Chamber::Ventricle && active(e) && e.at.0 >= 3000.0)
        .map(|e| e.at.0)
        .next()
        .unwrap();
    assert!(
        (s.at.0 - r - heartsim_sim::defib::SYNC_DELAY_MS).abs() < 1e-9,
        "shock {} ms after the R wave",
        s.at.0 - r
    );
    // no R wave, no shock
    let mut p = TimingParams::default();
    p.av.complete_block = true;
    p.escape.junctional_bpm = 0.0;
    p.escape.ventricular_bpm = 0.0;
    let mut e = TimingEngine::new(p);
    let (_, shocks) = run_engine(&mut e, 5.0, &[(1000.0, shock(100.0, true))]);
    assert!(
        shocks.is_empty() && e.shock_armed(),
        "sync mode waits for an R wave"
    );
    assert_eq!(e.rhythm_class(e.now()), RhythmClass::Asystole);
}

#[test]
fn a_heart_that_has_not_beaten_yet_is_not_in_asystole() {
    let mut e = TimingEngine::new(TimingParams::default());
    e.step(Millis(100.0));
    assert_eq!(e.rhythm_class(e.now()), RhythmClass::Organised);
}

#[test]
fn unsynchronised_shock_in_the_t_wave_is_flagged_vulnerable() {
    // step until an R wave after 1 s, then shock 300 ms later (the T-wave peak)
    let mut e = TimingEngine::new(TimingParams::default());
    let r = loop {
        e.step(Millis(1.0));
        let evs = e.drain_events();
        if let Some(r) = v_times(&evs).first().filter(|t| **t > 1000.0) {
            break *r;
        }
    };
    let (_, shocks) = run_engine(&mut e, 3.0, &[(r + 300.0, shock(200.0, false))]);
    assert!(shocks[0].vulnerable, "{:?}", shocks[0]);
    // the same moment in sync mode moves the shock onto the next R wave
    let mut e = TimingEngine::new(TimingParams::default());
    let (_, shocks) = run_engine(&mut e, 4.0, &[(r + 300.0, shock(200.0, true))]);
    assert!(
        !shocks[0].vulnerable,
        "synchronised shocks are never in the T wave"
    );
}

#[test]
fn shock_outcomes_follow_the_efficacy_curve() {
    // 200 shocks of 20 J on VT with different seeds: p = 0.97 · (1 − e^(−20/30)) ≈ 0.47
    let mut successes = 0;
    for seed in 0..200 {
        let mut e = TimingEngine::new(vt());
        e.set_shock_seed(seed);
        let (_, shocks) = run_engine(&mut e, 4.0, &[(3000.0, shock(20.0, true))]);
        successes += usize::from(shocks[0].success);
    }
    assert!((70..=120).contains(&successes), "{successes}/200 at 20 J");
    // same seed, same outcome
    let once = |seed| {
        let mut e = TimingEngine::new(vt());
        e.set_shock_seed(seed);
        run_engine(&mut e, 4.0, &[(3000.0, shock(20.0, true))]).1
    };
    assert_eq!(once(9), once(9));
}

fn heart() -> (
    Arc<heartsim_geometry::CellCloud>,
    std::collections::BTreeMap<String, Vec<u32>>,
) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/anatomy/default");
    let pack = AnatomyPack::load(&dir).expect("default pack loads");
    (Arc::new(pack.cells), pack.landmarks.sets)
}

#[test]
fn shock_during_vt_restores_sinus_rhythm_on_the_tier1_heart() {
    let (cloud, sets) = heart();
    let eik = EikonalEngine::with_seed_sets(cloud, &Parameters::default(), EikonalParams::default(), &sets);
    let mut t1 = Tier1Engine::new(vt(), eik);
    let mut evs = Vec::new();
    let mut shocks = Vec::new();
    let mut shocked_mesh = None;
    for step in 0..9000 {
        if step == 3000 {
            t1.apply(&shock(150.0, true), t1.now()).unwrap();
        }
        t1.step(Millis(1.0));
        evs.extend(t1.drain_events());
        let new = t1.drain_shocks();
        if let Some(s) = new.first() {
            // the mesh is depolarised at the shock: every excitable ventricular cell activates
            let e = t1.eikonal();
            let vent: Vec<usize> = (0..e.cloud().len())
                .filter(|&i| e.kinds()[i] == CellKind::Ventricular)
                .collect();
            let at_shock = vent
                .iter()
                .filter(|&&i| (e.activation_times()[i] - s.at.0).abs() < 1.5)
                .count();
            shocked_mesh = Some(at_shock as f64 / vent.len() as f64);
        }
        shocks.extend(new);
    }
    assert_eq!(shocks.len(), 1);
    let s = shocks[0];
    assert_eq!(s.rhythm, RhythmClass::VentricularTachycardia);
    assert!(s.success, "150 J on VT (p ≈ 0.96) with the default seed");
    let f = shocked_mesh.unwrap();
    assert!(
        f > 0.99,
        "{:.1} % of the ventricles depolarised by the shock",
        100.0 * f
    );
    let before: Vec<&Event> = evs
        .iter()
        .filter(|e| e.chamber == Chamber::Ventricle && active(e) && e.at.0 < s.at.0)
        .collect();
    // VT (with the odd sinus capture or fusion beat of AV dissociation) before
    let vt_beats = before.iter().filter(|e| e.origin == Origin::Focus(0)).count();
    assert!(
        vt_beats * 10 >= before.len() * 7,
        "VT before: {vt_beats} of {} beats",
        before.len()
    );
    let after: Vec<&Event> = evs
        .iter()
        .filter(|e| e.chamber == Chamber::Ventricle && active(e) && e.at.0 > s.at.0 + 1000.0)
        .collect();
    assert!(after.len() >= 4, "{} beats after the shock", after.len());
    assert!(
        after.iter().all(|e| e.origin == Origin::Sinus),
        "sinus resumes: {:?}",
        after.iter().map(|e| e.origin).collect::<Vec<_>>()
    );
    let v = v_times(&evs);
    let r = rate_bpm(&v, s.at.0 + 1000.0, 9000.0);
    assert!((55.0..=85.0).contains(&r), "sinus rate {r:.0} bpm");
}
