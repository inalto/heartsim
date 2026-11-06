//! The timing engine's own sustained arrhythmias (`atrial_driver`, `ventricular_driver`): what each
//! rhythm does, from engine events alone and on the 3-D heart, and how a shock ends it. The values
//! the tests check are the criteria in `docs/research/tier0-rhythm-criteria.md`.

use heartsim_core::scenario::Action;
use heartsim_core::{Millis, Parameters, Region};
use heartsim_geometry::AnatomyPack;
use heartsim_sim::defib::{RhythmClass, ShockRecord};
use heartsim_sim::tier1::{CellKind, EikonalEngine, EikonalParams, Tier1Engine};
use heartsim_sim::{Chamber, Circuit, Engine, Event, EventKind, Origin, TimingEngine, TimingParams};
use std::path::Path;
use std::sync::Arc;

fn timing(id: &str) -> TimingParams {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../assets/scenarios/{id}.ron"));
    TimingParams::from_scenario_ron(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn run(p: TimingParams, seed: u64, seconds: f64, actions: &[(f64, Action)]) -> (Vec<Event>, Vec<ShockRecord>) {
    let mut e = TimingEngine::new(p);
    e.set_shock_seed(seed);
    let (mut evs, mut shocks) = (Vec::new(), Vec::new());
    let mut next = 0;
    while e.now().0 < seconds * 1000.0 {
        while let Some((at, a)) = actions.get(next)
            && *at <= e.now().0
        {
            e.apply(a, Millis(*at)).unwrap();
            next += 1;
        }
        e.step(Millis(1.0));
        evs.extend(e.drain_events());
        shocks.extend(e.drain_shocks());
    }
    (evs, shocks)
}

fn shock(joules: f64, synchronised: bool) -> Action {
    Action::Shock { joules, synchronised }
}

/// Atrial activations of the circuit, ms.
fn atrial_cycles(evs: &[Event], from: f64) -> Vec<f64> {
    evs.iter()
        .filter(|e| e.at.0 >= from && e.chamber == Chamber::Atrium && matches!(e.origin, Origin::Circuit(_)))
        .map(|e| e.at.0)
        .collect()
}

/// Ventricular activations with their fusion fronts merged, ms.
fn beats(evs: &[Event], from: f64) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    for e in evs
        .iter()
        .filter(|e| e.at.0 >= from && e.chamber == Chamber::Ventricle && e.kind != EventKind::Blocked)
    {
        if out.last().is_none_or(|l| e.at.0 - l > 150.0) {
            out.push(e.at.0);
        }
    }
    out
}

fn diffs(t: &[f64]) -> Vec<f64> {
    t.windows(2).map(|w| w[1] - w[0]).collect()
}

fn mean(x: &[f64]) -> f64 {
    x.iter().sum::<f64>() / x.len() as f64
}

fn cv(x: &[f64]) -> f64 {
    let m = mean(x);
    (x.iter().map(|v| (v - m).powi(2)).sum::<f64>() / x.len() as f64).sqrt() / m
}

// --- atrial flutter ----------------------------------------------------------------------------

/// Flutter rate 250–350/min (typical about 300).
fn assert_flutter_rate(a: &[f64]) {
    let bpm = 60_000.0 / mean(&diffs(a));
    assert!((250.0..=350.0).contains(&bpm), "flutter rate {bpm}");
    assert!(cv(&diffs(a)) < 0.02, "flutter is regular");
}

#[test]
fn flutter_2to1_conducts_every_second_wave() {
    let (evs, _) = run(timing("flutter_2to1"), 42, 20.0, &[]);
    let (a, v) = (atrial_cycles(&evs, 5000.0), beats(&evs, 5000.0));
    assert_flutter_rate(&a);
    let ratio = a.len() as f64 / v.len() as f64;
    assert!((ratio - 2.0).abs() < 0.1, "AV ratio {ratio}");
    assert!(cv(&diffs(&v)) < 0.02, "fixed ratio is regular");
    let bpm = 60_000.0 / mean(&diffs(&v));
    assert!((145.0..=155.0).contains(&bpm), "rate {bpm}");
}

#[test]
fn flutter_4to1_conducts_every_fourth_wave() {
    let (evs, _) = run(timing("flutter_4to1"), 42, 20.0, &[]);
    let (a, v) = (atrial_cycles(&evs, 5000.0), beats(&evs, 5000.0));
    assert_flutter_rate(&a);
    let ratio = a.len() as f64 / v.len() as f64;
    assert!((ratio - 4.0).abs() < 0.2, "AV ratio {ratio}");
    assert!(cv(&diffs(&v)) < 0.02);
}

#[test]
fn flutter_with_variable_block_has_a_regular_atrium_and_an_irregular_ventricle() {
    let (evs, _) = run(timing("flutter_variable"), 42, 40.0, &[]);
    let (a, v) = (atrial_cycles(&evs, 5000.0), beats(&evs, 5000.0));
    assert_flutter_rate(&a);
    let rr = diffs(&v);
    assert!(cv(&rr) > 0.1, "RR CV {}", cv(&rr));
    let ratio = a.len() as f64 / v.len() as f64;
    assert!((2.0..=4.0).contains(&ratio), "mean AV ratio {ratio}");
}

#[test]
fn the_atrial_circuit_replaces_the_sinus_node() {
    let (evs, _) = run(timing("flutter_4to1"), 42, 10.0, &[]);
    let sinus = evs
        .iter()
        .filter(|e| e.at.0 > 1000.0 && e.origin == Origin::Sinus && e.kind != EventKind::Blocked)
        .count();
    assert_eq!(sinus, 0, "no sinus P waves in flutter");
}

#[test]
fn adenosine_raises_the_block_of_flutter_and_the_circuit_carries_on() {
    let dose = Action::Drug {
        name: "adenosine".into(),
        dose: 1.0,
    };
    let (evs, _) = run(timing("flutter_2to1"), 42, 20.0, &[(8000.0, dose)]);
    let a = atrial_cycles(&evs, 8000.0);
    assert_flutter_rate(&a);
    let during = beats(&evs, 9500.0).iter().filter(|t| **t < 14_000.0).count();
    assert!(during <= 5, "{during} QRS complexes in 4.5 s of adenosine (2:1 gives 11)");
}

#[test]
fn a_drug_that_slows_atrial_conduction_slows_the_circuit() {
    let dose = Action::Drug {
        name: "flecainide".into(),
        dose: 1.0,
    };
    let (evs, _) = run(timing("flutter_4to1"), 42, 200.0, &[(1000.0, dose)]);
    let before = atrial_cycles(&evs, 0.0);
    let early = 60_000.0 / mean(&diffs(&before[..4]));
    let late = 60_000.0 / mean(&diffs(&atrial_cycles(&evs, 190_000.0)));
    assert!(late < early, "flutter rate {early} then {late}");
}

// --- atrial fibrillation -----------------------------------------------------------------------

#[test]
fn af_has_irregular_atrial_cycles_and_an_irregularly_irregular_ventricular_rhythm() {
    let (evs, _) = run(timing("atrial_fibrillation"), 42, 40.0, &[]);
    let (a, v) = (atrial_cycles(&evs, 2000.0), beats(&evs, 2000.0));
    assert!(cv(&diffs(&a)) > 0.15, "atrial CV {}", cv(&diffs(&a)));
    let rate = 60_000.0 / mean(&diffs(&a));
    assert!((250.0..=600.0).contains(&rate), "atrial rate {rate}");
    assert!(cv(&diffs(&v)) > 0.12, "RR CV {}", cv(&diffs(&v)));
    assert!(
        evs.iter()
            .any(|e| e.region == Region::LeftAtrium && e.origin == Origin::Circuit(Circuit::AtrialFibrillation))
    );
}

// --- pre-excited AF ----------------------------------------------------------------------------

#[test]
fn pre_excited_af_is_fast_and_mostly_conducted_over_the_pathway() {
    let (evs, _) = run(timing("af_preexcited"), 42, 40.0, &[]);
    let v = beats(&evs, 2000.0);
    let rr = diffs(&v);
    let shortest = rr.iter().copied().fold(f64::INFINITY, f64::min);
    // the marker of a dangerous pathway: shortest pre-excited RR ≤ 250 ms
    assert!(shortest <= 250.0, "shortest RR {shortest}");
    let bpm = 60_000.0 / mean(&rr);
    assert!(bpm >= 150.0, "rate {bpm}");
    assert!(cv(&rr) > 0.08, "RR CV {}", cv(&rr));
    let over_pathway = evs
        .iter()
        .filter(|e| {
            e.chamber == Chamber::Ventricle && e.kind == EventKind::Conducted && e.origin == Origin::AccessoryPathway
        })
        .count();
    assert!(over_pathway * 2 > v.len(), "{over_pathway} of {} beats", v.len());
}

// --- ventricular fibrillation ------------------------------------------------------------------

#[test]
fn during_vf_nothing_else_enters_the_ventricles_and_the_sinus_node_is_silent() {
    let (evs, _) = run(timing("vf_coarse"), 42, 12.0, &[]);
    let late = |e: &&Event| e.at.0 > 2500.0;
    assert!(
        evs.iter()
            .filter(late)
            .filter(|e| e.chamber == Chamber::Ventricle && e.kind != EventKind::Blocked)
            .all(|e| e.origin == Origin::Circuit(Circuit::VentricularFibrillation)),
        "only the circuit activates the ventricles"
    );
    assert_eq!(
        evs.iter()
            .filter(late)
            .filter(|e| e.origin == Origin::Sinus && e.kind != EventKind::Blocked)
            .count(),
        0
    );
    let cycles: Vec<f64> = evs
        .iter()
        .filter(late)
        .filter(|e| e.origin == Origin::Circuit(Circuit::VentricularFibrillation))
        .map(|e| e.at.0)
        .collect();
    let bpm = 60_000.0 / mean(&diffs(&cycles));
    assert!(bpm > 250.0, "VF rate {bpm}");
    assert!(cv(&diffs(&cycles)) > 0.1);
}

#[test]
fn sync_mode_never_fires_in_vf_and_a_defibrillation_ends_it() {
    let (_, none) = run(timing("vf_coarse"), 42, 12.0, &[(5000.0, shock(200.0, true))]);
    assert!(none.is_empty(), "{none:?}");
    // the first seed whose shock succeeds; the engine must replay it exactly
    let mut ended = false;
    for seed in 1..10 {
        let (evs, s) = run(timing("vf_coarse"), seed, 14.0, &[(6000.0, shock(200.0, false))]);
        assert_eq!(s[0].rhythm, RhythmClass::VentricularFibrillation);
        let circuit_after = evs
            .iter()
            .any(|e| e.at.0 > 6000.0 && e.origin == Origin::Circuit(Circuit::VentricularFibrillation));
        assert_eq!(s[0].success, !circuit_after, "seed {seed}");
        if s[0].success {
            let sinus = beats(&evs, 8000.0);
            assert!(sinus.len() >= 3, "sinus rhythm returns: {} beats", sinus.len());
            ended = true;
        }
    }
    assert!(ended, "200 J succeeds on most seeds");
}

// --- torsades de pointes and polymorphic VT -----------------------------------------------------

/// `(t, cycle)` of the first torsades events, with the RR intervals before the first one.
fn torsades_start(evs: &[Event]) -> (f64, Vec<f64>) {
    let first = evs
        .iter()
        .find(|e| e.origin == Origin::Circuit(Circuit::Torsades))
        .expect("torsades started")
        .at
        .0;
    let before: Vec<f64> = beats(evs, 0.0).into_iter().filter(|t| *t < first).collect();
    (first, diffs(&before))
}

#[test]
fn torsades_starts_on_a_short_long_short_sequence_with_an_r_on_t() {
    let p = timing("torsades");
    let erp = p.ventricular_erp_ms;
    let (evs, _) = run(p, 42, 12.0, &[]);
    let (first, rr) = torsades_start(&evs);
    assert!(first >= 5000.0, "not before the file's not_before_ms: {first}");
    let [.., first_short, long, short] = rr.as_slice() else {
        panic!("too few beats before the episode");
    };
    assert!(*long > 1.5 * short && *long > 1.5 * first_short, "{first_short} {long} {short}");
    // the triggering beat falls in the vulnerable period of the beat before it
    assert!((0.7 * erp..=1.3 * erp).contains(short), "coupling {short}");
}

#[test]
fn torsades_runs_at_160_to_240_bpm_then_slows_and_stops_by_itself() {
    let (evs, _) = run(timing("torsades"), 42, 30.0, &[]);
    let t: Vec<f64> = evs
        .iter()
        .filter(|e| e.origin == Origin::Circuit(Circuit::Torsades))
        .map(|e| e.at.0)
        .collect();
    let cycles = diffs(&t);
    let steady = &cycles[..cycles.len() - 4];
    let bpm = 60_000.0 / mean(steady);
    assert!((160.0..=240.0).contains(&bpm), "rate {bpm}");
    // Drew et al. 2010: the last 2 to 3 beats slow
    let last = &cycles[cycles.len() - 3..];
    assert!(mean(last) > 1.2 * mean(steady), "{last:?} after {}", mean(steady));
    assert!(*t.last().unwrap() < 28_000.0, "the episode ended");
    let end = *t.last().unwrap();
    assert!(
        evs.iter()
            .any(|e| e.at.0 > end && e.chamber == Chamber::Ventricle && e.origin == Origin::Sinus && e.kind != EventKind::Blocked),
        "sinus beats return"
    );
}

#[test]
fn torsades_returns_after_a_shock_because_the_long_qt_is_still_there() {
    let mut returned = false;
    for seed in 1..10 {
        let (evs, s) = run(timing("torsades"), seed, 30.0, &[(10_000.0, shock(200.0, false))]);
        assert_eq!(s[0].rhythm, RhythmClass::PolymorphicVentricularTachycardia);
        if s[0].success {
            let next = evs
                .iter()
                .find(|e| e.at.0 > 10_000.0 && e.origin == Origin::Circuit(Circuit::Torsades));
            let at = next.expect("recurs").at.0;
            assert!(at >= 18_000.0, "recurs after the file's 8 s: {at}");
            returned = true;
        }
    }
    assert!(returned);
}

#[test]
fn polymorphic_vt_is_fast_and_a_sync_shock_waits_for_an_organised_r_wave() {
    let (evs, _) = run(timing("polymorphic_vt"), 42, 12.0, &[]);
    let t: Vec<f64> = evs
        .iter()
        .filter(|e| e.origin == Origin::Circuit(Circuit::PolymorphicVt))
        .map(|e| e.at.0)
        .collect();
    let bpm = 60_000.0 / mean(&diffs(&t));
    assert!((170.0..=260.0).contains(&bpm), "rate {bpm}");
    assert!(cv(&diffs(&t)) > 0.05);
    let (_, none) = run(timing("polymorphic_vt"), 42, 12.0, &[(5000.0, shock(150.0, true))]);
    assert!(none.is_empty());
}

// --- shocks on the atrial circuits -------------------------------------------------------------

#[test]
fn a_synchronised_shock_ends_flutter_and_preexcited_af_on_the_first_seed_that_succeeds() {
    for (id, class, joules) in [
        ("flutter_2to1", RhythmClass::AtrialFlutter, 100.0),
        ("atrial_fibrillation", RhythmClass::AtrialFibrillationOrFlutter, 200.0),
        ("af_preexcited", RhythmClass::AtrialFibrillationOrFlutter, 150.0),
    ] {
        let mut ended = false;
        for seed in 1..10 {
            let (evs, s) = run(timing(id), seed, 20.0, &[(8000.0, shock(joules, true))]);
            assert!(s[0].synchronised && s[0].at.0 >= 8000.0, "{id}");
            assert_eq!(s[0].rhythm, class, "{id}");
            let after = atrial_cycles(&evs, s[0].at.0 + 1.0).len();
            if s[0].success {
                assert_eq!(after, 0, "{id}: the circuit stopped");
                assert!(
                    evs.iter()
                        .any(|e| e.at.0 > s[0].at.0 && e.origin == Origin::Sinus && e.kind == EventKind::Automatic),
                    "{id}: the sinus node took over"
                );
                ended = true;
            } else {
                assert!(after > 0, "{id}: a failed shock leaves it running");
            }
        }
        assert!(ended, "{id}");
    }
}

// --- determinism -------------------------------------------------------------------------------

#[test]
fn a_seed_fixes_the_rhythm_and_another_seed_changes_it() {
    let a = run(timing("vf_coarse"), 1, 6.0, &[]).0;
    assert_eq!(a, run(timing("vf_coarse"), 1, 6.0, &[]).0);
    assert_ne!(a, run(timing("vf_coarse"), 2, 6.0, &[]).0);
}

#[test]
fn old_files_without_drivers_keep_their_events() {
    let mut p = timing("sinus_normal");
    assert!(p.atrial_driver.is_none() && p.ventricular_driver.is_none());
    p.ventricular_driver = None;
    assert_eq!(run(p.clone(), 1, 5.0, &[]).0, run(p, 99, 5.0, &[]).0);
}

// --- the 3-D heart -----------------------------------------------------------------------------

fn eikonal() -> EikonalEngine {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/anatomy/default");
    let p = AnatomyPack::load(&dir).expect("default pack loads");
    let mut e = EikonalEngine::with_seed_sets(
        Arc::new(p.cells.clone()),
        &Parameters::default(),
        EikonalParams::default(),
        &p.landmarks.sets,
    );
    e.set_aha_segments(&p.aha_segments);
    e
}

/// Fractions of the cells of `kind` above half V_m, over a run after `settle_ms`.
fn depolarised(t1: &mut Tier1Engine, kind: CellKind, settle_ms: usize, ms: usize) -> Vec<f64> {
    for _ in 0..settle_ms {
        t1.step(Millis(1.0));
    }
    let kinds = t1.eikonal().kinds().to_vec();
    let total = kinds.iter().filter(|k| **k == kind).count() as f64;
    (0..ms)
        .map(|_| {
            t1.step(Millis(1.0));
            let vm = &t1.cells().vm;
            kinds
                .iter()
                .zip(vm)
                .filter(|(k, v)| **k == kind && **v > 0.5)
                .count() as f64
                / total
        })
        .collect()
}

#[test]
fn the_heart_agrees_with_the_strip_in_vf_and_a_shock_restores_it() {
    let base = eikonal();
    // Sinus rhythm: the ventricles rest between beats.
    let mut healthy = Tier1Engine::new(timing("sinus_normal"), base.clone());
    let f = depolarised(&mut healthy, CellKind::Ventricular, 1500, 2000);
    let rest = f.iter().copied().fold(f64::INFINITY, f64::min);
    assert!(rest < 0.02, "sinus diastole: {rest}");

    // VF: there is never a moment when the chamber is at rest, nor one when it is all excited.
    let mut vf = Tier1Engine::new(timing("vf_coarse"), base);
    let f = depolarised(&mut vf, CellKind::Ventricular, 4000, 2000);
    let (lo, hi) = (
        f.iter().copied().fold(f64::INFINITY, f64::min),
        f.iter().copied().fold(0.0, f64::max),
    );
    assert!(lo > 0.05, "VF: at least some tissue is always excited: {lo}");
    assert!(hi < 0.98, "VF: no common beat: {hi}");

    // A shock the timing engine says succeeded depolarises everything at once and then rests.
    let mut shocked = false;
    for _ in 0..40 {
        vf.apply(&shock(200.0, false), vf.now()).unwrap();
        vf.step(Millis(1.0));
        let done = vf.drain_shocks().last().is_some_and(|s| s.success);
        if done {
            shocked = true;
            break;
        }
    }
    assert!(shocked, "200 J succeeds within 40 tries");
    let f = depolarised(&mut vf, CellKind::Ventricular, 1200, 300);
    let rest = f.iter().copied().fold(f64::INFINITY, f64::min);
    assert!(rest < 0.05, "after the shock the chamber rests: {rest}");
}

#[test]
fn the_atria_follow_flutter_one_to_one_on_the_heart() {
    let mut t1 = Tier1Engine::new(timing("flutter_4to1"), eikonal());
    let f = depolarised(&mut t1, CellKind::Atrial, 2000, 2000);
    // a front at every event: 10 cycles in 2 s appear as 10 rising crossings of the mean level
    let m = mean(&f);
    let rises = f.windows(2).filter(|w| w[0] <= m && w[1] > m).count();
    assert!((8..=12).contains(&rises), "{rises} atrial fronts in 2 s, the strip has 10");
}
