//! Acceptance tests for brief B16 (Tier-2 reaction–diffusion engine).

use glam::Vec3;
use heartsim_core::{Action, Millis, Parameters};
use heartsim_geometry::AnatomyPack;
use heartsim_sim::tier1::{CellKind, EikonalEngine, EikonalParams, Tier1Engine};
use heartsim_sim::tier2::calibrate::{cable_cv, solve_d};
use heartsim_sim::tier2::reentry::{breakup_substrate, s1_s2_cross_field};
use heartsim_sim::tier2::{
    AlievPanfilov, CellModel, FentonKarma, MitchellSchaeffer, Monodomain, Tier2Engine, Tier2Params, Tissue,
    VoxelGrid,
};
use heartsim_sim::timing::TimingParams;
use heartsim_sim::{Chamber, Engine, EventKind, Origin};
use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

// --- 1-D cable ---------------------------------------------------------------------------------

#[test]
fn cable_cv_follows_the_front_speed_relation() {
    // fine grid: the bistable-front speed c = sqrt(A D / 2)(β − 2α) of each model
    let ap = AlievPanfilov::default();
    let ms = MitchellSchaeffer::default();
    for d in [0.25f32, 0.5, 1.0, 2.0] {
        // h ≈ 1/4.5 of the front's length scale L = sqrt(2 D T / k) ≈ 1.8·sqrt(D), so the
        // cable's timing window sits 3–7 L from the stimulus; dt = h²/(5 D) (stable)
        let h = 0.4 * d.sqrt();
        let dt = h * h / (5.0 * d);
        let want = ap.analytic_cv(d).unwrap();
        let got = cable_cv(&ap, d, h, dt).unwrap();
        eprintln!("AP D {d}: CV {got:.3} analytic {want:.3}");
        assert!((got / want - 1.0).abs() < 0.10, "AP D {d}: {got} vs {want}");
    }
    for d in [0.05f32, 0.1, 0.2] {
        let h = 0.15 * d.sqrt();
        let dt = (h * h / (5.0 * d)).min(0.01);
        let want = ms.analytic_cv(d).unwrap();
        let got = cable_cv(&ms, d, h, dt).unwrap();
        eprintln!("MS D {d}: CV {got:.3} analytic {want:.3}");
        assert!((got / want - 1.0).abs() < 0.10, "MS D {d}: {got} vs {want}");
    }
    // the production grid (2 mm, 0.25 ms): calibration inverts the measured relation
    let (h, dt) = (2.0, 0.25);
    let (d, cv) = solve_d(&ap, 0.6, h, dt, 2.4);
    assert!((cv - 0.6).abs() < 0.01, "D {d} → {cv}");
}

/// S1 at rest, S2 `di_ms` after the S1 action potential ended (v < v_gate); APD and DI are
/// measured on the v_gate crossings, as in Mitchell & Schaeffer 2003. Returns (APD, h when the
/// S1 action potential ended, h at the S2 upstroke).
fn ms_s2(m: &MitchellSchaeffer, di_ms: f32) -> (f32, f32, f32) {
    let dt = m.tau_in_ms / 60.0;
    let mut s = [0.0f32; 2];
    m.rest(&mut s);
    let (mut phase, mut t_up, mut t_s2, mut h_end, mut h_s2) = (0, 0.0f32, f32::INFINITY, 0.0, 0.0);
    let mut n = 0u32;
    loop {
        let t = n as f32 * dt;
        let stim = if t < 1.0 || (t >= t_s2 && t < t_s2 + 1.0) {
            0.5
        } else {
            0.0
        };
        let before = s[0];
        m.step(&mut s, stim, dt);
        n += 1;
        let t = n as f32 * dt;
        let up = before < m.v_gate && s[0] >= m.v_gate;
        let down = before >= m.v_gate && s[0] < m.v_gate;
        match phase {
            0 if down => {
                (t_s2, h_end, phase) = (t + di_ms, s[1], 1);
            }
            1 if up && t >= t_s2 => {
                (t_up, h_s2, phase) = (t, s[1], 2);
            }
            2 if down => return (t - t_up, h_end, h_s2),
            _ => {}
        }
        assert!(t < 5000.0, "no S2 response at DI {di_ms}");
    }
}

#[test]
fn mitchell_schaeffer_restitution_matches_the_analytic_curve() {
    // The closed form APD = τ_close·ln(h_max/h_min) is the τ_in ≪ τ_out ≪ τ_close limit: at the
    // paper's τ_in = 0.3 ms the slow passage through the saddle-node at h_min adds a nearly
    // DI-independent ≈ 40 ms (the cell repolarises at h ≈ 0.15, not 0.2). Test the closed form
    // in its regime (τ_in = 0.01, τ_out = 0.2: same h_min) …
    let fast = MitchellSchaeffer {
        tau_in_ms: 0.01,
        tau_out_ms: 0.2,
        ..MitchellSchaeffer::default()
    };
    for di in [40.0f32, 80.0, 150.0, 300.0, 600.0] {
        let (got, _, _) = ms_s2(&fast, di);
        let want = fast.analytic_apd_ms(di);
        eprintln!("MS τ_in 0.01, DI {di}: APD {got:.1} analytic {want:.1}");
        assert!((got / want - 1.0).abs() < 0.10, "DI {di}: {got} vs {want}");
    }
    // … and its mechanism at the paper's parameters: APD = τ_close·ln(h(S2)/h(end of S1 AP)).
    let paper = MitchellSchaeffer::default();
    for di in [20.0f32, 40.0, 80.0, 150.0, 300.0, 600.0] {
        let (got, h_end, h_s2) = ms_s2(&paper, di);
        let want = paper.tau_close_ms * (h_s2 / h_end).ln();
        eprintln!("MS paper, DI {di}: APD {got:.1}, τ_close·ln(h_S2/h_end) {want:.1}");
        assert!((got / want - 1.0).abs() < 0.03, "DI {di}: {got} vs {want}");
    }
}

// --- 2-D sheet ---------------------------------------------------------------------------------

/// Run the S1–S2 protocol on an `n × n` sheet (h = 1 mm) with S2 at APD + `s2_after_apd_ms`
/// and sample the phase-singularity count every 50 ms for `ms` after S2 + 100 ms.
fn spiral<M: CellModel>(sim: &mut Monodomain<M>, s2_after_apd_ms: f64, ms: f64) -> Vec<usize> {
    let s2 = f64::from(sim.trajectories()[0].apd_ms) + s2_after_apd_ms;
    let t2 = s1_s2_cross_field(sim, s2, 1.0);
    let mut counts = Vec::new();
    let mut t = t2 + 100.0;
    sim.advance_to(t);
    while t < t2 + 100.0 + ms {
        t += 50.0;
        sim.advance_to(t);
        counts.push(sim.phase_singularities());
    }
    counts
}

fn sheet<M: CellModel>(model: M, n: usize, d: f32, dt: f32) -> Monodomain<M> {
    Monodomain::uniform(
        VoxelGrid::sheet(n, n, 1.0),
        Tissue {
            model,
            d_long: d,
            d_trans: d,
        },
        dt,
    )
}

// Sheets are scaled in units of the Aliev–Panfilov front length L = sqrt(2 D T / k): h = L =
// 1 mm (D = 0.31 mm²/ms, CV ≈ 0.2 m/s) puts ≈ 75 voxels in a wavelength, so a 256² sheet holds
// more than three wavelengths at dt = 0.5 ms.
const AP_SHEET_D: f32 = 0.31;

#[test]
fn s1_s2_spiral_persists_for_two_seconds() {
    let mut sim = sheet(AlievPanfilov::default(), 192, AP_SHEET_D, 0.5);
    let c = spiral(&mut sim, 100.0, 2000.0);
    eprintln!("AP default PS counts {c:?}");
    assert!(c.iter().all(|&k| (1..=2).contains(&k)), "not one rotor: {c:?}");
}

#[test]
fn breakup_substrate_multiplies_rotors() {
    // same cell and protocol as the persistent single rotor above, on the break-up preset
    let mut sim = sheet(AlievPanfilov::default(), 256, AP_SHEET_D, 0.5);
    breakup_substrate(&mut sim, 7);
    let c = spiral(&mut sim, 100.0, 1250.0);
    eprintln!("AP break-up PS counts {c:?}");
    let mean = c.iter().sum::<usize>() as f64 / c.len() as f64;
    let max = c.iter().copied().max().unwrap_or(0);
    assert!(
        mean >= 2.5 && max >= 6,
        "no break-up (mean {mean:.1}, max {max}): {c:?}"
    );
}

#[test]
fn mitchell_schaeffer_flat_restitution_gives_one_stable_rotor() {
    // Restitution slope ∝ τ_close/τ_open: τ_close = 100 < τ_open = 120 keeps it flat enough
    // for one rotor (τ_close = 300 breaks up). Coarse variant (τ_in 2.4, τ_out 48 ms) so that
    // h = 1 mm resolves the front and the sheet holds ≈ 3 wavelengths.
    let m = MitchellSchaeffer {
        tau_close_ms: 100.0,
        ..MitchellSchaeffer::coarse(8.0)
    };
    let mut sim = sheet(m, 256, 0.42, 0.5);
    let c = spiral(&mut sim, 100.0, 2000.0);
    eprintln!("MS PS counts {c:?}");
    let ones = c.iter().filter(|&&k| k == 1).count();
    assert!(ones * 10 >= c.len() * 8 && c.iter().all(|&k| k <= 3), "{c:?}");
}

// --- 3-D idealised heart -------------------------------------------------------------------

fn pack_engine() -> EikonalEngine {
    static ENGINE: OnceLock<EikonalEngine> = OnceLock::new();
    ENGINE
        .get_or_init(|| {
            let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/anatomy/default");
            let pack = AnatomyPack::load(&dir).expect("default pack loads");
            EikonalEngine::with_seed_sets(
                Arc::new(pack.cells),
                &Parameters::default(),
                EikonalParams::default(),
                &pack.landmarks.sets,
            )
        })
        .clone()
}

fn heart_tier2() -> Tier2Engine {
    static ENGINE: OnceLock<Tier2Engine> = OnceLock::new();
    ENGINE
        .get_or_init(|| {
            let t0 = Instant::now();
            let e = Tier2Engine::new(
                TimingParams::default(),
                pack_engine(),
                &AlievPanfilov::default(),
                Tier2Params::default(),
            );
            let s = e.solver();
            eprintln!(
                "tier2 heart: {} voxels in {:?} box, {} classes, {} links, built in {:.2?}",
                s.grid().len(),
                s.grid().dims,
                s.tissue().len(),
                s.link_count(),
                t0.elapsed()
            );
            e
        })
        .clone()
}

fn run<E: Engine>(e: &mut E, ms: usize) {
    for _ in 0..ms {
        e.step(Millis(1.0));
    }
}

#[test]
fn sinus_beat_matches_tier1_activation() {
    let mut t1 = Tier1Engine::new(TimingParams::default(), pack_engine());
    let mut t2 = heart_tier2();
    run(&mut t1, 600);
    let t0 = Instant::now();
    run(&mut t2, 600);
    let per_ms = t0.elapsed().as_secs_f64() * 1000.0 / 600.0;
    let e = t1.eikonal();
    let (a1, a2) = (e.activation_times(), &t2.cells().activation_ms);
    let mut sq = [0.0f64; 2];
    let mut n = [0usize; 2];
    let mut missing = 0;
    for c in 0..a1.len() {
        let k = match e.kinds()[c] {
            CellKind::Atrial => 0,
            CellKind::Ventricular => 1,
            _ => continue,
        };
        let (x, y) = (a1[c], f64::from(a2[c]));
        if !x.is_finite() {
            continue;
        }
        if !y.is_finite() {
            missing += 1;
            continue;
        }
        sq[k] += (x - y).powi(2);
        n[k] += 1;
    }
    let rms = |k: usize| (sq[k] / n[k].max(1) as f64).sqrt();
    let all = ((sq[0] + sq[1]) / (n[0] + n[1]).max(1) as f64).sqrt();
    eprintln!(
        "RMS atria {:.1} ms ({}), ventricles {:.1} ms ({}), all {all:.1} ms; {missing} cells not activated; {per_ms:.2} ms wall per simulated ms",
        rms(0),
        n[0],
        rms(1),
        n[1]
    );
    assert!(missing * 100 < n[0] + n[1], "{missing} cells never activated");
    assert!(all <= 8.0, "activation RMS {all:.1} ms");
}

#[test]
fn shock_returns_every_voxel_to_rest_within_50_ms() {
    let mut t2 = heart_tier2();
    run(&mut t2, 330); // ventricles depolarised, atria repolarising
    let up = t2.solver().u().iter().filter(|&&u| u > 0.5).count();
    assert!(up > 1000, "only {up} voxels excited before the shock");
    let now = t2.now();
    t2.apply(
        &Action::Shock {
            joules: 200.0,
            synchronised: false,
        },
        now,
    )
    .unwrap();
    run(&mut t2, 50);
    let worst = t2.solver().u().iter().fold(0.0f32, |a, &b| a.max(b));
    assert!(worst < 0.1, "max u {worst} 50 ms after the shock");
}

#[test]
fn tissue_activations_are_sensed_by_the_timing_engine() {
    let mut t2 = heart_tier2();
    run(&mut t2, 850); // first sinus beat over, ventricles recovered
    t2.drain_events();
    // an activation the timing engine knows nothing about (as a re-entrant wave would be)
    let site = t2.router().seeds().ventricular_lead.clone();
    let vox: Vec<u32> = site
        .iter()
        .flat_map(|&c| t2.stimulus_voxels(c).to_vec())
        .collect();
    let now = t2.now().0;
    t2.solver_mut().stimulate(&vox, now, 2.0, 1.0);
    run(&mut t2, 20);
    let evs = t2.drain_events();
    assert!(
        evs.iter().any(|e| e.chamber == Chamber::Ventricle
            && e.origin == Origin::Reentry
            && e.kind == EventKind::Automatic),
        "{evs:?}"
    );
}

#[test]
fn tier1_hand_over_continues_the_beat() {
    let mut t1 = Tier1Engine::new(TimingParams::default(), pack_engine());
    run(&mut t1, 330); // mid-QRS
    let mut t2 = Tier2Engine::from_tier1(&t1, &AlievPanfilov::default(), Tier2Params::default());
    let excited_t1 = t1.cells().vm.iter().filter(|&&v| v > 0.5).count();
    let excited_t2 = t2.cells().vm.iter().filter(|&&v| v > 0.5).count();
    eprintln!("excited cells at hand-over: tier1 {excited_t1}, tier2 {excited_t2}");
    assert!(excited_t2 * 10 > excited_t1 * 7);
    run(&mut t2, 120);
    run(&mut t1, 120);
    // the ventricles finish depolarising in both
    let e = t1.eikonal();
    let vent: Vec<usize> = (0..e.kinds().len())
        .filter(|&c| e.kinds()[c] == CellKind::Ventricular)
        .collect();
    let done = vent
        .iter()
        .filter(|&&c| t2.cells().activation_ms[c].is_finite())
        .count();
    assert!(done * 100 >= vent.len() * 97, "{done}/{}", vent.len());
}

// --- determinism and budget --------------------------------------------------------------------

#[test]
fn deterministic_across_runs_and_thread_counts() {
    let go = || {
        let c = spiral_state(AlievPanfilov::default());
        let mut t2 = heart_tier2();
        run(&mut t2, 450);
        (c, t2.solver().u().to_vec(), t2.drain_events())
    };
    let a = go();
    let b = go();
    let pool = rayon::ThreadPoolBuilder::new().num_threads(1).build().unwrap();
    let c = pool.install(go);
    for (x, y) in [(&a, &b), (&a, &c)] {
        assert!(x.0.iter().zip(&y.0).all(|(p, q)| p.to_bits() == q.to_bits()));
        assert!(x.1.iter().zip(&y.1).all(|(p, q)| p.to_bits() == q.to_bits()));
        assert_eq!(x.2, y.2);
    }
}

fn spiral_state<M: CellModel>(model: M) -> Vec<f32> {
    let mut sim = Monodomain::uniform(
        VoxelGrid::block([40, 40, 4], 1.0, Vec3::new(1.0, 1.0, 0.0)),
        Tissue {
            model,
            d_long: 1.0,
            d_trans: 0.3,
        },
        0.1,
    );
    let t2 = s1_s2_cross_field(&mut sim, 300.0, 1.0);
    sim.advance_to(t2 + 200.0);
    sim.u().to_vec()
}

#[test]
fn fenton_karma_conducts_on_the_coarse_grid() {
    let fk = FentonKarma::coarse(2.0);
    let (d, cv) = solve_d(&fk, 0.5, 2.0, 0.25, 2.4);
    assert!((cv - 0.5).abs() < 0.02, "FK D {d} → CV {cv}");
}

/// Resting tissue decays exponentially towards 0; the kernel flushes it before it turns
/// subnormal (an order of magnitude slower on x86).
#[test]
fn resting_tissue_never_goes_subnormal() {
    let mut sim = sheet(AlievPanfilov::default(), 16, AP_SHEET_D, 0.5);
    sim.stimulate(&(0..32).collect::<Vec<u32>>(), 0.0, 2.0, 1.0);
    for t in 1..=40 {
        sim.advance_to(f64::from(t) * 250.0);
        let mut state = [0.0f32; 2];
        for v in 0..sim.grid().len() {
            sim.voxel_state(v, &mut state);
            assert!(
                state.iter().all(|x| !x.is_subnormal()),
                "voxel {v} at {t}: {state:?}"
            );
        }
    }
    assert!(
        sim.u().iter().all(|&u| u == 0.0),
        "u has not settled to exactly 0"
    );
}
