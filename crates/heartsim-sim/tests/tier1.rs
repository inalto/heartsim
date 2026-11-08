//! Acceptance tests for brief B03 (Tier-1 eikonal activation engine).

use glam::Vec3;
use heartsim_core::{Action, Millis, Parameters, Region, StimSite};
use heartsim_geometry::{AnatomyPack, CellCloud};
use heartsim_sim::tier1::{CellKind, EikonalEngine, EikonalParams, Tier1Engine};
use heartsim_sim::timing::{Focus, FocusRegion, PacerConfig, PacerMode, TimingParams};
use heartsim_sim::{Engine, EventKind, Origin};
use std::path::Path;
use std::sync::{Arc, OnceLock};

// --- fixtures ----------------------------------------------------------------------------------

/// A 60 × 20 × 10 mm lattice (1 mm spacing) of LV myocardium with fibres along x.
fn slab() -> Arc<CellCloud> {
    let mut pos = Vec::new();
    for z in 0..=10 {
        for y in 0..=20 {
            for x in 0..=60 {
                pos.push(Vec3::new(x as f32, y as f32, z as f32));
            }
        }
    }
    let n = pos.len();
    Arc::new(CellCloud::build(
        pos,
        vec![Vec3::X; n],
        vec![Region::LeftVentricle; n],
        vec![0.5; n],
    ))
}

fn slab_engine() -> EikonalEngine {
    EikonalEngine::new(slab(), &Parameters::default())
}

/// The default anatomy pack's engine, built once and cloned per test.
fn heart() -> EikonalEngine {
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

fn cells_where(e: &EikonalEngine, pred: impl Fn(usize) -> bool) -> Vec<usize> {
    (0..e.cloud().len()).filter(|&i| pred(i)).collect()
}

/// (first, last) activation among `cells` within `[from, to)` ms.
fn span(e: &EikonalEngine, cells: &[usize], from: f64, to: f64) -> (f64, f64) {
    let t = e.activation_times();
    cells
        .iter()
        .map(|&i| t[i])
        .filter(|t| t.is_finite() && *t >= from && *t < to)
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), t| {
            (lo.min(t), hi.max(t))
        })
}

fn mean(e: &EikonalEngine, cells: &[usize]) -> f64 {
    let t = e.activation_times();
    let v: Vec<f64> = cells.iter().map(|&i| t[i]).filter(|t| t.is_finite()).collect();
    v.iter().sum::<f64>() / v.len().max(1) as f64
}

fn ventricular(e: &EikonalEngine) -> Vec<usize> {
    cells_where(e, |i| e.kinds()[i] == CellKind::Ventricular)
}

fn atrial(e: &EikonalEngine) -> Vec<usize> {
    cells_where(e, |i| e.kinds()[i] == CellKind::Atrial)
}

/// Run the coupled Tier-1 engine for `ms` milliseconds at 1 ms.
fn run(timing: TimingParams, eik: EikonalEngine, ms: usize) -> Tier1Engine {
    let mut t1 = Tier1Engine::new(timing, eik);
    for _ in 0..ms {
        t1.step(Millis(1.0));
    }
    t1
}

/// One sinus beat: the first P at 200 ms, ventricles by ≈ 400 ms, next P at ≈ 1057 ms.
fn sinus_beat(timing: TimingParams) -> Tier1Engine {
    run(timing, heart(), 1000)
}

/// Total ventricular activation time of the first beat, ms.
fn qrs_ms(e: &EikonalEngine) -> f64 {
    let (lo, hi) = span(e, &ventricular(e), 0.0, 1000.0);
    hi - lo
}

// --- slab: CV calibration and planar waves --------------------------------------------------------

/// Stimulate the plane `axis = 0` at t = 0 and return the engine after `ms`.
fn planar(e: &mut EikonalEngine, axis: usize, ms: usize) {
    let seeds: Vec<u32> = (0..e.cloud().len())
        .filter(|&i| e.cloud().positions[i][axis] == 0.0)
        .map(|i| i as u32)
        .collect();
    e.stimulate_cells(&seeds, Millis(0.0), Origin::External);
    for _ in 0..ms {
        e.step(Millis(1.0));
    }
}

/// Mean arrival time at the plane `axis = coord`, restricted to the interior.
fn plane_time(e: &EikonalEngine, axis: usize, coord: f32) -> f64 {
    let c = e.cloud();
    let cells = cells_where(e, |i| {
        let p = c.positions[i];
        (p[axis] - coord).abs() < 1e-3
            && (2.0..=8.0).contains(&p.z)
            && (if axis == 0 {
                (2.0..=18.0).contains(&p.y)
            } else {
                (10.0..=50.0).contains(&p.x)
            })
    });
    mean(e, &cells)
}

#[test]
fn slab_cv_along_and_across_fibres() {
    let p = Parameters::default().region(Region::LeftVentricle);
    let mut e = slab_engine();
    planar(&mut e, 0, 200);
    let cv_long = 40.0 / (plane_time(&e, 0, 50.0) - plane_time(&e, 0, 10.0));
    assert!(
        (cv_long / p.cv_long_m_s - 1.0).abs() < 0.03,
        "CV along {cv_long:.3} m/s vs {}",
        p.cv_long_m_s
    );

    let mut e = slab_engine();
    planar(&mut e, 1, 200);
    let cv_trans = 14.0 / (plane_time(&e, 1, 17.0) - plane_time(&e, 1, 3.0));
    let expected = p.anisotropy_ratio * p.cv_long_m_s;
    assert!(
        (cv_trans / expected - 1.0).abs() < 0.05,
        "CV across {cv_trans:.3} m/s vs {expected}"
    );
}

#[test]
fn planar_wave_matches_analytic_arrival_times() {
    let cv = Parameters::default().region(Region::LeftVentricle).cv_long_m_s;
    let mut e = slab_engine();
    planar(&mut e, 0, 200);
    let c = e.cloud();
    let t = e.activation_times();
    let sq: f64 = (0..c.len())
        .map(|i| (t[i] - f64::from(c.positions[i].x) / cv).powi(2))
        .sum();
    let rms = (sq / c.len() as f64).sqrt();
    assert!(rms < 1.0, "planar-wave RMS error {rms:.3} ms");
}

#[test]
fn point_source_metrication_error_is_small() {
    // Isotropic tissue: arrival = distance / CV for any direction on the 56-neighbourhood graph.
    let mut params = Parameters::default();
    let mut lv = params.region(Region::LeftVentricle);
    lv.anisotropy_ratio = 1.0;
    params.regions.insert(Region::LeftVentricle, lv);
    let cloud = slab();
    let mut e = EikonalEngine::new(Arc::clone(&cloud), &params);
    let centre = cloud.nearest(Vec3::new(30.0, 10.0, 5.0)).unwrap();
    e.stimulate_cells(&[centre], Millis(0.0), Origin::External);
    for _ in 0..200 {
        e.step(Millis(1.0));
    }
    let c0 = cloud.positions[centre as usize];
    let err: Vec<f64> = (0..cloud.len())
        .filter(|&i| cloud.positions[i].distance(c0) > 8.0)
        .map(|i| e.activation_times()[i] * lv.cv_long_m_s / f64::from(cloud.positions[i].distance(c0)) - 1.0)
        .collect();
    let worst = err.iter().copied().fold(0.0f64, f64::max);
    let mean = err.iter().sum::<f64>() / err.len() as f64;
    assert!(
        err.iter().all(|e| *e > -1e-6),
        "a graph geodesic is never shorter than the straight line"
    );
    assert!(
        worst < 0.10,
        "worst metrication overestimate {:.1} %",
        worst * 100.0
    );
    // 56-neighbourhood: measured ≈ 9 % worst, 4.4 % mean (8-NN would be up to 70 %).
    assert!(mean < 0.05, "mean metrication overestimate {:.1} %", mean * 100.0);
}

// --- refractoriness and aberrancy ---------------------------------------------------------------

#[test]
fn premature_stimulus_inside_erp_is_ignored_and_after_erp_conducts_slowly() {
    let p = Parameters::default().region(Region::LeftVentricle);
    let apd = p.apd_endo_ms.unwrap() + (p.apd_ms - p.apd_endo_ms.unwrap()) * 0.5;
    let erp = apd * p.erp_fraction;
    let s1_cv = {
        let mut e = slab_engine();
        planar(&mut e, 0, 200);
        30.0 / (plane_time(&e, 0, 40.0) - plane_time(&e, 0, 10.0))
    };

    // S2 10 ms inside the ERP: nothing happens.
    let mut e = slab_engine();
    planar(&mut e, 0, 1);
    let seeds: Vec<u32> = (0..e.cloud().len())
        .filter(|&i| e.cloud().positions[i].x == 0.0)
        .map(|i| i as u32)
        .collect();
    let s2 = (erp - 10.0).floor();
    for _ in 1..s2 as usize {
        e.step(Millis(1.0));
    }
    let first = e.activation_times().to_vec();
    e.stimulate_cells(&seeds, Millis(s2), Origin::External);
    for _ in 0..400 {
        e.step(Millis(1.0));
    }
    assert_eq!(e.activation_times(), &first[..], "S2 inside ERP must not capture");

    // S2 just after the ERP: it captures and conducts the whole slab, more slowly than S1.
    let mut e = slab_engine();
    planar(&mut e, 0, 1);
    let s2 = (erp + 2.0).ceil();
    for _ in 1..s2 as usize {
        e.step(Millis(1.0));
    }
    e.stimulate_cells(&seeds, Millis(s2), Origin::External);
    for _ in 0..400 {
        e.step(Millis(1.0));
    }
    let t10 = plane_time(&e, 0, 10.0);
    let t40 = plane_time(&e, 0, 40.0);
    assert!(t10 > s2 && t40 > t10, "S2 conducted ({t10:.1}, {t40:.1})");
    let s2_cv = 30.0 / (t40 - t10);
    assert!(s2_cv < 0.95 * s1_cv, "aberrant CV {s2_cv:.3} vs {s1_cv:.3} m/s");
    // and its APD is shortened by restitution
    let mid = e.cloud().nearest(Vec3::new(30.0, 10.0, 5.0)).unwrap() as usize;
    assert!(e.cells().apd_ms[mid] < apd as f32 * 0.9);
}

// --- idealised heart --------------------------------------------------------------------------

#[test]
fn sinus_beat_has_physiological_activation_sequence() {
    let t1 = sinus_beat(TimingParams::default());
    let e = t1.eikonal();
    let (a0, a1) = span(e, &atrial(e), 0.0, 1000.0);
    let p_ms = a1 - a0;
    assert!((80.0..=120.0).contains(&p_ms), "atrial activation {p_ms:.1} ms");
    let qrs = qrs_ms(e);
    assert!(
        (70.0..=100.0).contains(&qrs),
        "ventricular activation {qrs:.1} ms"
    );

    let z = e.apex_base();
    let lv = |pred: &dyn Fn(usize) -> bool| {
        cells_where(e, |i| {
            matches!(
                e.cloud().regions[i],
                Region::LeftVentricle | Region::VentricularSeptum
            ) && pred(i)
        })
    };
    let apex = mean(e, &lv(&|i| z[i] < 0.3));
    let base = mean(e, &lv(&|i| z[i] > 0.8));
    assert!(apex < base, "apex {apex:.1} before base {base:.1}");
    let depth = &e.cloud().depth;
    let endo = mean(e, &lv(&|i| depth[i] < 0.25 && z[i] < 0.8));
    let epi = mean(e, &lv(&|i| depth[i] > 0.75 && z[i] < 0.8));
    assert!(
        endo + 5.0 < epi,
        "endocardium {endo:.1} before epicardium {epi:.1}"
    );
    // atria are activated before the ventricles, the mesh reported the nodal input
    let (v0, _) = span(e, &ventricular(e), 0.0, 1000.0);
    assert!(a0 < v0);
}

#[test]
fn bundle_branch_blocks_widen_qrs_and_delay_the_blocked_side() {
    let free_wall = |e: &EikonalEngine, r: Region| cells_where(e, |i| e.cloud().regions[i] == r);

    let mut lbbb = TimingParams::default();
    lbbb.his_purkinje.lbb.blocked = true;
    let t1 = sinus_beat(lbbb);
    let e = t1.eikonal();
    let qrs = qrs_ms(e);
    assert!((140.0..=180.0).contains(&qrs), "LBBB activation {qrs:.1} ms");
    let lv = mean(e, &free_wall(e, Region::LeftVentricle));
    let rv = mean(e, &free_wall(e, Region::RightVentricle));
    assert!(lv > rv + 20.0, "LBBB: LV {lv:.1} last, RV {rv:.1}");

    let mut rbbb = TimingParams::default();
    rbbb.his_purkinje.rbb.blocked = true;
    let t1 = sinus_beat(rbbb);
    let e = t1.eikonal();
    let qrs = qrs_ms(e);
    assert!(qrs >= 120.0, "RBBB activation {qrs:.1} ms");
    let lv = mean(e, &free_wall(e, Region::LeftVentricle));
    let rv = mean(e, &free_wall(e, Region::RightVentricle));
    assert!(rv > lv + 20.0, "RBBB: RV {rv:.1} last, LV {lv:.1}");
}

#[test]
fn left_anterior_fascicular_block_delays_the_superior_lv() {
    let normal = sinus_beat(TimingParams::default());
    let mut lafb = TimingParams::default();
    lafb.his_purkinje.laf.blocked = true;
    let blocked = sinus_beat(lafb);
    let e = normal.eikonal();
    // the anterosuperior LV: the LAF's Purkinje territory
    let laf: Vec<usize> = e.seeds().laf.iter().map(|&c| c as usize).collect();
    assert!(laf.len() > 10, "LAF PMJ set");
    let d = mean(blocked.eikonal(), &laf) - mean(e, &laf);
    assert!(d > 15.0, "LAF territory delayed by {d:.1} ms");
    let lpf: Vec<usize> = e.seeds().lpf.iter().map(|&c| c as usize).collect();
    let d_lpf = mean(blocked.eikonal(), &lpf) - mean(e, &lpf);
    assert!(d_lpf.abs() < 2.0, "LPF territory unchanged ({d_lpf:.1} ms)");
    let qrs = qrs_ms(blocked.eikonal());
    assert!(qrs < 120.0, "LAFB barely widens the QRS ({qrs:.1} ms)");
}

#[test]
fn rv_apex_pvc_and_pacing_activate_slowly() {
    // PVC: a single ventricular focus 600 ms after the first sinus QRS, placed at the RV apex.
    let mut timing = TimingParams::default();
    let mut pvc = Focus::pvc(600.0, 1);
    pvc.region = FocusRegion::RightVentricle;
    pvc.max_fires = 1;
    timing.foci.push(pvc);
    let mut eik = heart();
    eik.set_focus_site(0, &StimSite::Landmark("rv_apex".into()));
    let t1 = run(timing, eik, 1180);
    let e = t1.eikonal();
    let pvc_cells: Vec<usize> = ventricular(e)
        .into_iter()
        .filter(|&i| e.activation_origin()[i] == Origin::Focus(0))
        .collect();
    assert!(
        pvc_cells.len() > ventricular(e).len() * 9 / 10,
        "the PVC captures the ventricles"
    );
    let (lo, hi) = span(e, &pvc_cells, 0.0, 2000.0);
    assert!(hi - lo > 130.0, "PVC activation {:.1} ms", hi - lo);
    let apex = e.cloud().landmarks["rv_apex"] as usize;
    assert!(
        (e.activation_times()[apex] - lo).abs() < 1e-9,
        "earliest at the RV apex"
    );

    // Pacing: VOO at 100 ppm with the lead at the RV apex (default lead site), complete AV block
    // so that no conducted beat fuses with the paced one.
    let mut timing = TimingParams::default();
    timing.av.complete_block = true;
    timing.pacer = Some(PacerConfig::new(PacerMode::Voo, 100.0));
    let t1 = run(timing, heart(), 1150);
    let e = t1.eikonal();
    let paced: Vec<usize> = ventricular(e)
        .into_iter()
        .filter(|&i| e.activation_origin()[i] == Origin::Pacer(1))
        .collect();
    assert!(
        paced.len() > ventricular(e).len() * 9 / 10,
        "{} paced",
        paced.len()
    );
    let last = paced
        .iter()
        .map(|&i| e.activation_times()[i])
        .fold(0.0f64, f64::max);
    let (lo, hi) = span(e, &paced, last - 400.0, 2000.0);
    assert!(hi - lo > 130.0, "paced activation {:.1} ms", hi - lo);
}

#[test]
fn mesh_reports_nodal_input_and_ablation_blocks() {
    let mut t1 = Tier1Engine::new(TimingParams::default(), heart());
    for _ in 0..400 {
        t1.step(Millis(1.0));
    }
    let mesh = t1.drain_mesh_events();
    assert!(
        mesh.iter().any(|e| e.region == Region::AvNodeCompact
            && e.kind == EventKind::Conducted
            && e.origin == Origin::Sinus),
        "atrial front reaches the AV-node input: {mesh:?}"
    );

    // Ablating a 12 mm sphere around the LV apex leaves those cells silent.
    let mut t1 = Tier1Engine::new(TimingParams::default(), heart());
    t1.apply(
        &Action::Ablate {
            site: StimSite::Landmark("lv_apex".into()),
            radius_mm: 12.0,
        },
        Millis(0.0),
    )
    .expect("ablation is spatial");
    for _ in 0..1000 {
        t1.step(Millis(1.0));
    }
    let e = t1.eikonal();
    let apex = e.cloud().positions[e.cloud().landmarks["lv_apex"] as usize];
    let burnt = cells_where(e, |i| e.cloud().positions[i].distance(apex) <= 12.0);
    assert!(burnt.len() > 10, "ablated volume");
    assert!(
        burnt
            .iter()
            .all(|&i| e.activation_times()[i].is_nan() && e.cells().vm[i] == 0.0)
    );
    assert!(qrs_ms(e) > 0.0, "the rest of the ventricles still activates");
}

#[test]
fn scenario_ablated_cells_stay_silent_across_reset() {
    let mut eik = heart();
    let c = Arc::clone(eik.cloud());
    let apex = c.positions[c.landmarks["rv_apex"] as usize];
    let scar: Vec<u32> = (0..c.len() as u32)
        .filter(|&i| c.positions[i as usize].distance(apex) <= 8.0)
        .collect();
    assert!(scar.len() > 10, "scar volume");
    eik.set_ablated_cells(&scar);
    let mut t1 = Tier1Engine::new(TimingParams::default(), eik);
    for round in 0..2 {
        for _ in 0..1000 {
            t1.step(Millis(1.0));
        }
        let e = t1.eikonal();
        assert!(
            scar.iter().all(|&i| e.activation_times()[i as usize].is_nan()),
            "round {round}: ablated cells never activate"
        );
        assert!(qrs_ms(e) > 0.0);
        t1.reset();
    }
}

#[test]
fn vm_follows_the_wavefront() {
    let mut t1 = Tier1Engine::new(TimingParams::default(), heart());
    let mut peak_depolarised = 0usize;
    for _ in 0..700 {
        t1.step(Millis(1.0));
        let n = t1.cells().vm.iter().filter(|v| **v > 0.5).count();
        peak_depolarised = peak_depolarised.max(n);
    }
    let n = t1.cells().vm.len();
    assert!(
        peak_depolarised > n / 2,
        "the whole heart depolarises during the beat"
    );
    for _ in 0..450 {
        t1.step(Millis(1.0));
    }
    // ≈ 1150 ms: ventricles repolarised, second P wave under way
    let e = t1.eikonal();
    let v = ventricular(e);
    let still = v.iter().filter(|&&i| e.cells().vm[i] > 0.1).count();
    assert!(
        still < v.len() / 50,
        "{still} ventricular cells still depolarised"
    );
}

#[test]
fn deterministic_and_reset_reproduces() {
    let a = sinus_beat(TimingParams::default());
    let mut b = sinus_beat(TimingParams::default());
    let bits = |t: &[f64]| t.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    assert_eq!(
        bits(a.eikonal().activation_times()),
        bits(b.eikonal().activation_times())
    );
    assert_eq!(a.cells().vm, b.cells().vm);
    b.reset();
    for _ in 0..1000 {
        b.step(Millis(1.0));
    }
    assert_eq!(
        bits(a.eikonal().activation_times()),
        bits(b.eikonal().activation_times())
    );
}

/// Golden snapshot: histogram of the default sinus beat's activation times (5 ms bins relative to
/// the first activation), hashed. Update `GOLDEN` only for an intended model change, and say so in
/// the commit message (last: the baked anatomy pack of 0.7.0 with its re-tuned fast layer, patches
/// and atrial conduction).
#[test]
fn golden_sinus_activation_histogram() {
    const GOLDEN: u64 = 13_156_204_337_655_351_566;
    let t1 = sinus_beat(TimingParams::default());
    let t = t1.eikonal().activation_times();
    let t0 = t
        .iter()
        .copied()
        .filter(|x| x.is_finite())
        .fold(f64::INFINITY, f64::min);
    let mut bins = vec![0u32; 80];
    let mut never = 0u32;
    for x in t {
        if x.is_finite() {
            bins[(((x - t0) / 5.0) as usize).min(79)] += 1;
        } else {
            never += 1;
        }
    }
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for v in bins.iter().chain(std::iter::once(&never)) {
        for b in v.to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    assert_eq!(h, GOLDEN, "histogram {bins:?}, never activated {never}");
}
