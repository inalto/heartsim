//! B22 acceptance test on Tier 2: a line across the cavotricuspid isthmus terminates typical
//! flutter, and entrainment tells a site in the circuit from one outside it.
//!
//! Established flutter is computed once and every test continues from a clone of it, so the file
//! costs a few simulated seconds per test.

use glam::Vec3;
use heartsim_core::{Millis, Region};
use heartsim_geometry::AnatomyPack;
use heartsim_sim::ep::{AblationTarget, EpLab, EpPreset, Protocol, ProtocolOutcome, ProtocolStatus};
use heartsim_sim::tier2::Tier;
use heartsim_sim::{Chamber, Engine, Event, EventKind, Origin};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

fn assets() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

fn pack() -> &'static AnatomyPack {
    static PACK: OnceLock<AnatomyPack> = OnceLock::new();
    PACK.get_or_init(|| AnatomyPack::load(&assets().join("anatomy/default")).expect("default pack loads"))
}

/// Flutter started at 1 s and established by 4 s.
fn established() -> &'static EpLab {
    static LAB: OnceLock<EpLab> = OnceLock::new();
    LAB.get_or_init(|| {
        let text = std::fs::read_to_string(assets().join("scenarios/ep/ep_flutter.ron")).unwrap();
        let preset = EpPreset::from_ron(&text).unwrap().expect("ep section");
        let mut l = preset.build(
            Arc::new(pack().cells.clone()),
            &pack().landmarks.sets,
            &pack().aha_segments,
        );
        run(&mut l, 4000.0);
        l
    })
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

/// Atrial cycles the tissue produced, sensed at the AV-nodal input.
fn sensed_atrial(evs: &[Event]) -> Vec<f64> {
    evs.iter()
        .filter(|e| {
            e.chamber == Chamber::Atrium && e.origin == Origin::Reentry && e.kind == EventKind::Automatic
        })
        .map(|e| e.at.0)
        .collect()
}

#[test]
fn the_circuit_keeps_turning_at_a_flutter_rate() {
    let mut l = established().clone();
    assert_eq!(l.engine().tier(), Tier::Two);
    let evs = run(&mut l, 3000.0);
    assert_eq!(l.engine().tier(), Tier::Two, "{:?}", l.engine().switches());
    let sensed = sensed_atrial(&evs);
    assert!(sensed.len() >= 8, "{} activations in 3 s", sensed.len());
    let cycle: Vec<f64> = sensed.windows(2).map(|w| w[1] - w[0]).collect();
    let mean = cycle.iter().sum::<f64>() / cycle.len() as f64;
    let sd = (cycle.iter().map(|c| (c - mean).powi(2)).sum::<f64>() / cycle.len() as f64).sqrt();
    eprintln!("flutter cycle {mean:.0} ms ± {sd:.1}");
    assert!((180.0..=350.0).contains(&mean), "cycle {mean} ms: {cycle:?}");
    assert!(sd < 25.0, "not a regular macro-re-entry: {cycle:?}");
    // the AV node filters it: fewer ventricular than atrial beats
    let v = evs
        .iter()
        .filter(|e| e.region == Region::HisBundle && e.kind == EventKind::Conducted)
        .count();
    assert!(v > 0 && v < sensed.len(), "His {v}, atria {}", sensed.len());
    // and the ventricular rate is the one the procedure file declares
    let text = std::fs::read_to_string(assets().join("scenarios/ep/ep_flutter.ron")).unwrap();
    let preset = EpPreset::from_ron(&text).unwrap().unwrap();
    let (lo, hi) = preset.scenario.expected.unwrap().hr_bpm;
    let rate = v as f64 * 60_000.0 / 3000.0;
    eprintln!("ventricular rate {rate:.0}/min");
    assert!((lo..=hi).contains(&rate), "{rate}/min, file says {lo}-{hi}");
}

#[test]
fn a_cti_line_terminates_flutter_and_sinus_rhythm_returns() {
    let mut l = established().clone();
    assert!(l.ablate_target(AblationTarget::CtiLine) >= 3);
    let lesions = l.lesions().len();
    run(&mut l, 1000.0);
    let evs = run(&mut l, 5000.0);
    let late: Vec<f64> = sensed_atrial(&evs)
        .into_iter()
        .filter(|t| *t > l.now().0 - 3000.0)
        .collect();
    assert!(late.is_empty(), "the wave went on after the line: {late:?}");
    assert_eq!(l.engine().tier(), Tier::One, "{:?}", l.engine().switches());
    assert!(l.goal_reached());
    let sinus = evs
        .iter()
        .filter(|e| {
            e.region == Region::RightAtrium && e.origin == Origin::Sinus && e.kind == EventKind::Automatic
        })
        .count();
    assert!(sinus >= 2, "{sinus} sinus beats");

    // the line outlives an engine reset: the same seeded wave now dies against it
    l.reset();
    assert_eq!(l.lesions().len(), lesions);
    run(&mut l, 2000.0);
    let evs = run(&mut l, 1500.0);
    assert!(
        sensed_atrial(&evs).is_empty(),
        "flutter came back after the reset: {:?}",
        sensed_atrial(&evs)
    );
}

#[test]
fn a_gap_in_the_line_does_not_terminate_it() {
    let mut l = established().clone();
    let sites = l.target_sites(AblationTarget::CtiLine);
    assert!(sites.len() >= 3);
    // only the stop at one end of the line
    l.ablate_now(sites[0], 3.0);
    run(&mut l, 500.0);
    let evs = run(&mut l, 2000.0);
    assert!(
        sensed_atrial(&evs).len() >= 5,
        "a single lesion stopped the circuit"
    );
    assert_eq!(l.engine().tier(), Tier::Two);
}

#[test]
fn entrainment_tells_the_isthmus_from_a_site_outside_the_circuit() {
    let entrain = |site: Vec3| {
        let mut l = established().clone();
        let cell = l.capture_cell(site).expect("tissue");
        l.start_protocol(
            cell,
            Protocol::Entrainment {
                offset_ms: 20.0,
                count: 6,
            },
        );
        while l.protocol_status() != ProtocolStatus::Finished && l.now().0 < 16_000.0 {
            l.step(Millis(1.0));
            l.drain_events();
        }
        match l.protocol_outcome() {
            Some(ProtocolOutcome::Entrainment(e)) => *e,
            other => panic!("{other:?}"),
        }
    };
    // the isthmus cell in the middle of the circuit's width: halfway between the annulus band and
    // the outer barrier of the file's patches (30° and 70° from the annulus axis; the centre and
    // axis are the ones `ep_flutter.ron` draws its bands with)
    let (centre, axis) = (
        Vec3::new(-61.9, 16.5, -8.3),
        Vec3::new(0.684, -0.657, 0.317).normalize(),
    );
    let polar = |p: Vec3| {
        (p - centre)
            .normalize()
            .dot(axis)
            .clamp(-1.0, 1.0)
            .acos()
            .to_degrees()
    };
    let isthmus = (0..pack().cells.len())
        .filter(|&i| pack().cells.regions[i] == Region::CavotricuspidIsthmus)
        .map(|i| pack().cells.positions[i])
        .min_by(|a, b| (polar(*a) - 50.0).abs().total_cmp(&(polar(*b) - 50.0).abs()))
        .expect("isthmus cells");
    let inside = entrain(isthmus);
    eprintln!("isthmus {inside:?}");
    assert_eq!(inside.captured, inside.delivered, "{inside:?}");
    assert!(inside.in_circuit, "{inside:?}");
    // outside the circuit: the right atrial free wall just beyond the barrier (polar ≈ 100°
    // about the annulus axis; the left atrium is inert in this lesson)
    let high_ra = (0..pack().cells.len())
        .filter(|&i| pack().cells.regions[i] == Region::RightAtrium)
        .map(|i| pack().cells.positions[i])
        .min_by(|a, b| (polar(*a) - 100.0).abs().total_cmp(&(polar(*b) - 100.0).abs()))
        .expect("right atrial cells");
    let outside = entrain(high_ra);
    eprintln!("high RA {outside:?}");
    assert!(!outside.in_circuit, "{outside:?}");
}
