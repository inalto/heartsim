//! Acceptance tests for brief B10 (electrode editor): cable reversals reach the ring buffers in
//! both tiers, the misplacement detector names every pattern of the reversal table and stays
//! silent on normal recordings, and placements persist in the scenario.

use glam::Vec3;
use heartsim_core::{Electrode, LEAD_COUNT, Lead, Millis, Parameters, Scenario};
use heartsim_ecg::{
    DetectorConfig, Exercise, ExtraLead, Finding, ForwardModel, ForwardOptions, GaussianSynth, LeadRing,
    Placement, Preset, Tier0Field, TorsoSurface, detect_misplacement,
};
use heartsim_geometry::AnatomyPack;
use heartsim_sim::tier1::{EikonalEngine, EikonalParams, Tier1Engine};
use heartsim_sim::{Engine, TimingEngine, TimingParams};
use std::path::Path;
use std::sync::{Arc, OnceLock};

fn pack() -> &'static AnatomyPack {
    static PACK: OnceLock<AnatomyPack> = OnceLock::new();
    PACK.get_or_init(|| {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/anatomy/default");
        AnatomyPack::load(&dir).expect("default pack loads")
    })
}

fn surface() -> &'static TorsoSurface {
    static S: OnceLock<TorsoSurface> = OnceLock::new();
    S.get_or_init(|| TorsoSurface::new(&pack().torso))
}

fn rings() -> Vec<LeadRing> {
    (0..LEAD_COUNT).map(|_| LeadRing::new(4000, 1000.0)).collect()
}

fn latest(ring: &LeadRing, n: usize) -> Vec<f32> {
    let mut v = Vec::new();
    ring.latest(n, &mut v);
    v
}

// --- Tier 0 recordings ---------------------------------------------------------------------------

/// A Tier-0 heart: timing parameters plus synthesiser knobs.
#[derive(Clone)]
struct Heart {
    timing: TimingParams,
    axis_shift_deg: f64,
    qrs_width_scale: f64,
}

impl Heart {
    fn textbook() -> Self {
        Self {
            timing: TimingParams::default(),
            axis_shift_deg: 0.0,
            qrs_width_scale: 1.0,
        }
    }
}

/// Twelve leads of `heart` recorded for 3 s at 1 kHz through the lead vectors of each placement
/// (same events, same synthesiser state), last 2 s.
fn record_tier0(heart: &Heart, placements: &[&Placement]) -> Vec<Vec<Vec<f32>>> {
    let field = Tier0Field::new(&pack().electrodes);
    let mut eng = TimingEngine::new(heart.timing.clone());
    let mut synths: Vec<GaussianSynth> = placements
        .iter()
        .map(|p| {
            let mut s = GaussianSynth::textbook();
            s.axis_shift_deg = heart.axis_shift_deg;
            s.qrs_width_scale = heart.qrs_width_scale;
            s.leads = field.lead_vectors(p);
            s
        })
        .collect();
    let mut out: Vec<Vec<LeadRing>> = placements.iter().map(|_| rings()).collect();
    for _ in 0..3000 {
        eng.step(Millis(1.0));
        let ev = eng.drain_events();
        for (s, r) in synths.iter_mut().zip(&mut out) {
            s.on_events(&ev);
            let v = s.sample(eng.now());
            for (ring, x) in r.iter_mut().zip(v.0) {
                ring.push(x as f32);
            }
        }
    }
    out.iter()
        .map(|r| r.iter().map(|ring| latest(ring, 2000)).collect())
        .collect()
}

fn detect(current: &[Vec<f32>], reference: &[Vec<f32>]) -> Vec<Finding> {
    let c: Vec<&[f32]> = current.iter().map(Vec::as_slice).collect();
    let r: Vec<&[f32]> = reference.iter().map(Vec::as_slice).collect();
    detect_misplacement(&c, &r, &DetectorConfig::default())
}

fn rms(x: &[f32]) -> f32 {
    (x.iter().map(|v| v * v).sum::<f32>() / x.len().max(1) as f32).sqrt()
}

#[test]
fn tier0_ra_la_swap_inverts_lead_i_in_the_ring_buffers() {
    let standard = Placement::standard(&pack().electrodes);
    let mut swapped = standard.clone();
    swapped.swap(Electrode::RA, Electrode::LA);
    let rec = record_tier0(&Heart::textbook(), &[&standard, &swapped]);
    let (a, b) = (&rec[0], &rec[1]);
    let i = Lead::I as usize;
    assert!(rms(&a[i]) > 0.05, "lead I carries signal");
    for (x, y) in a[i].iter().zip(&b[i]) {
        assert!((x + y).abs() < 1e-5, "lead I inverted: {x} vs {y}");
    }
    for (x, y) in a[Lead::III as usize].iter().zip(&b[Lead::II as usize]) {
        assert!((x - y).abs() < 1e-5, "II ↔ III");
    }
    assert_eq!(a[Lead::AVF as usize], b[Lead::AVF as usize], "aVF unchanged");
    assert_eq!(
        a[Lead::V5 as usize],
        b[Lead::V5 as usize],
        "chest leads unchanged"
    );
}

#[test]
fn detector_names_every_exercise_on_the_textbook_heart() {
    let sites = &pack().electrodes;
    let standard = Placement::standard(sites);
    let placements: Vec<Placement> = Exercise::ALL
        .iter()
        .map(|x| {
            let mut p = standard.clone();
            x.apply(&mut p, sites, Some(surface()));
            p
        })
        .collect();
    let mut all: Vec<&Placement> = vec![&standard];
    all.extend(placements.iter());
    let rec = record_tier0(&Heart::textbook(), &all);
    for (k, x) in Exercise::ALL.iter().enumerate() {
        let f = detect(&rec[k + 1], &rec[0]);
        assert!(x.recognised(&f), "{}: detector said {f:?}", x.label());
        for finding in &f {
            assert_ne!(finding.explanation(), "");
        }
    }
}

#[test]
fn reversal_table_patterns_in_the_leads() {
    // research §3.6: the lead-level signature of each limb reversal
    use Electrode::{LA, LL, RA, RL};
    let standard = Placement::standard(&pack().electrodes);
    let mk = |a, b| {
        let mut p = standard.clone();
        p.swap(a, b);
        p
    };
    let (ra_ll, la_ll, ra_rl, la_rl) = (mk(RA, LL), mk(LA, LL), mk(RA, RL), mk(LA, RL));
    let rec = record_tier0(&Heart::textbook(), &[&standard, &ra_ll, &la_ll, &ra_rl, &la_rl]);
    let s = &rec[0];
    let close = |x: &[f32], y: &[f32], sign: f32| x.iter().zip(y).all(|(a, b)| (a - sign * b).abs() < 1e-5);
    let l = |r: &Vec<Vec<f32>>, lead: Lead| r[lead as usize].clone();
    // RA ↔ LL: II inverted, aVR ↔ aVF, aVL unchanged
    assert!(close(&l(&rec[1], Lead::II), &l(s, Lead::II), -1.0));
    assert!(close(&l(&rec[1], Lead::AVR), &l(s, Lead::AVF), 1.0));
    assert!(close(&l(&rec[1], Lead::AVL), &l(s, Lead::AVL), 1.0));
    // LA ↔ LL: III inverted, I ↔ II, aVR unchanged
    assert!(close(&l(&rec[2], Lead::III), &l(s, Lead::III), -1.0));
    assert!(close(&l(&rec[2], Lead::I), &l(s, Lead::II), 1.0));
    assert!(close(&l(&rec[2], Lead::AVR), &l(s, Lead::AVR), 1.0));
    // RA ↔ RL: II flat; LA ↔ RL: III flat
    assert!(rms(&l(&rec[3], Lead::II)) < 1e-6);
    assert!(rms(&l(&rec[4], Lead::III)) < 1e-6);
}

/// Deterministic xorshift in [−1, 1).
fn jitter(state: &mut u64) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    ((*state >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0
}

#[test]
fn no_false_positives_on_25_normal_presets() {
    // 25 normal hearts (axes within −30…+90°, rates 50–100 bpm, QRS widths 0.9–1.1) recorded with
    // the standard placement, the chest electrodes off their exact sites by up to ±4 mm along the
    // skin as in any real recording.
    let sites = &pack().electrodes;
    let standard = Placement::standard(sites);
    let mut rng = 0x2545_F491_4F6C_DD1D_u64;
    let mut false_positives = Vec::new();
    for k in 0..25 {
        let mut heart = Heart::textbook();
        heart.axis_shift_deg = -60.0 + 3.5 * f64::from(k);
        heart.timing.sinus.rate_bpm = 50.0 + 2.0 * f64::from(k);
        heart.qrs_width_scale = 0.9 + 0.008 * f64::from(k);
        let mut placed = standard.clone();
        for e in heartsim_ecg::placement::PRECORDIAL {
            let p = placed.position(e);
            let az = surface().azimuth_deg(p) + 1.5 * jitter(&mut rng);
            let y = p.y + 4.0 * jitter(&mut rng);
            placed.set(e, surface().at(y, az).expect("on the torso"));
        }
        let rec = record_tier0(&heart, &[&standard, &placed]);
        let f = detect(&rec[1], &rec[0]);
        if !f.is_empty() {
            false_positives.push((k, f));
        }
    }
    assert!(false_positives.is_empty(), "{false_positives:?}");
}

// --- Tier 1 --------------------------------------------------------------------------------------

#[test]
fn tier1_ra_la_swap_inverts_lead_i_in_the_ring_buffers() {
    let p = pack();
    let base = ForwardModel::build(&p.cells, &p.electrodes, ForwardOptions::default()).expect("builds");
    let standard = Placement::standard(&p.electrodes);
    let mut reference = base.clone();
    standard.apply_to_forward(&mut reference);
    let mut swapped_model = base.clone();
    let mut swapped = standard.clone();
    swapped.swap(Electrode::RA, Electrode::LA);
    swapped.apply_to_forward(&mut swapped_model);

    let eik = EikonalEngine::with_seed_sets(
        Arc::new(p.cells.clone()),
        &Parameters::default(),
        EikonalParams::default(),
        &p.landmarks.sets,
    );
    let mut t1 = Tier1Engine::new(TimingParams::default(), eik);
    let (mut ra, mut rb, mut rbase) = (rings(), rings(), rings());
    for _ in 0..700 {
        t1.step(Millis(1.0));
        let vm = &t1.cells().vm;
        for (model, r) in [
            (&reference, &mut ra),
            (&swapped_model, &mut rb),
            (&base, &mut rbase),
        ] {
            for (ring, v) in r.iter_mut().zip(model.sample(vm).to_leads().0) {
                ring.push(v as f32);
            }
        }
    }
    let i = Lead::I as usize;
    let (a, b) = (latest(&ra[i], 700), latest(&rb[i], 700));
    assert_eq!(a, latest(&rbase[i], 700), "standard placement = default model");
    assert!(rms(&a) > 0.02, "lead I carries the beat");
    for (x, y) in a.iter().zip(&b) {
        assert!(
            (x + y).abs() < 1e-5 * x.abs().max(1.0),
            "lead I inverted: {x} vs {y}"
        );
    }
    assert_eq!(
        latest(&ra[Lead::III as usize], 700),
        latest(&rb[Lead::II as usize], 700),
        "II ↔ III"
    );
}

#[test]
fn tier1_extra_rows_match_a_signal_electrode_at_the_same_site() {
    let p = pack();
    let mut model = ForwardModel::build(&p.cells, &p.electrodes, ForwardOptions::default()).expect("builds");
    let mut placement = Preset::RightSided.build(&p.electrodes);
    placement.apply_to_forward(&mut model);
    assert_eq!(model.extra_electrodes().len(), 2);
    let v4r = placement.extras[&ExtraLead::V4R];
    let mut probe = model.clone();
    probe.move_electrode(Electrode::V1, v4r);
    let vm: Vec<f32> = p
        .cells
        .positions
        .iter()
        .map(|x| 0.5 - x.x / 200.0 + x.y / 300.0)
        .collect();
    let extra = model.sample_extras(&vm);
    let names: Vec<&str> = model.extra_electrodes().iter().map(|(n, _)| n.as_str()).collect();
    let k = names.iter().position(|n| *n == "V4R").expect("V4R row");
    let direct = probe.sample(&vm).get(Electrode::V1);
    assert!(
        (extra[k] - direct).abs() < 1e-6 * direct.abs().max(1.0),
        "{} vs {direct}",
        extra[k]
    );
    // switching the extras off removes the rows; signal rows are untouched
    let before = model.sample(&vm);
    placement.extras.clear();
    placement.apply_to_forward(&mut model);
    assert_eq!(model.sample_extras(&vm), Vec::<f64>::new());
    assert_eq!(model.sample(&vm), before);
}

// --- persistence and the surface -----------------------------------------------------------------

#[test]
fn placement_persists_in_the_scenario_and_reloads() {
    let sites = &pack().electrodes;
    let standard = Placement::standard(sites);
    let mut p = Preset::Posterior.build(sites);
    Exercise::V1V2TooHigh.apply(&mut p, sites, Some(surface()));
    p.swap(Electrode::RA, Electrode::LA);
    p.set(Electrode::V4, surface().at(-20.0, 50.0).unwrap());
    let mut s = Scenario::sinus_normal();
    p.write_scenario(&mut s, &standard);
    assert!(s.electrodes_mm.contains_key(&Electrode::V1));
    assert!(
        !s.electrodes_mm.contains_key(&Electrode::V6),
        "unchanged sites are not written"
    );
    assert_eq!(s.extra_electrodes_mm.len(), 3);
    let text = s.to_ron().unwrap();
    let back = Placement::from_scenario(&Scenario::from_ron(&text).unwrap(), sites);
    for e in Electrode::ALL {
        assert!(back.position(e).distance(p.position(e)) < 1e-4, "{}", e.label());
    }
    assert_eq!(back.extras.len(), 3);
    for (x, q) in &p.extras {
        assert!(back.extras[x].distance(*q) < 1e-4);
    }
    // a scenario without overrides is the standard placement
    assert_eq!(
        Placement::from_scenario(&Scenario::sinus_normal(), sites),
        standard
    );
}

#[test]
fn snapping_finds_the_intercostal_spaces() {
    let s = surface();
    let v1 = pack().electrodes.get("V1").unwrap();
    let off = s.project(v1 + Vec3::new(2.0, 5.0, 0.0)).unwrap();
    let snapped = s.snap(off);
    assert!(
        snapped.position.distance(v1) < 2.0,
        "{:?} vs {v1}",
        snapped.position
    );
    assert_eq!(
        snapped.landmark.as_deref(),
        Some("4th intercostal space, right sternal border")
    );
    let high = s.snap(s.project(v1 + Vec3::Y * 31.0).unwrap());
    assert_eq!(
        high.landmark.as_deref(),
        Some("3rd intercostal space, right sternal border")
    );
    for e in heartsim_ecg::placement::PRECORDIAL {
        let p = pack().electrodes.standard(e).unwrap();
        assert!(
            s.project(p).unwrap().distance(p) < 3.0,
            "{} on the skin",
            e.label()
        );
    }
}
