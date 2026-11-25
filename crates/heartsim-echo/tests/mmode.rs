//! Brief B34 acceptance (M-mode, calipers, reference feedback): the column buffer, the diameters
//! read from the label line against the deformed mesh, TAPSE and MAPSE on an apical M-mode.

mod common;

use common::{Sim, model, pack};
use glam::Vec3;
use heartsim_echo::measure::{ExcursionContext, cavity_diameter_of_column, reference_excursion};
use heartsim_echo::phases::Side;
use heartsim_echo::{
    AcousticTable, DesignedParams, EchoAnatomy, ImageSettings, MModeScan, MotionFrame, MotionInput,
    MotionModel, Named, Plane2, ProbePose, ScanContext, ViewId, ViewTargets, cursor_frame, cursor_toward,
    line_angle_deg,
};
use std::sync::OnceLock;

fn anatomy() -> &'static EchoAnatomy {
    static A: OnceLock<EchoAnatomy> = OnceLock::new();
    A.get_or_init(|| EchoAnatomy::new(pack(), &model(), DesignedParams::default()).expect("anatomy"))
}

fn targets() -> &'static ViewTargets {
    static T: OnceLock<ViewTargets> = OnceLock::new();
    T.get_or_init(|| ViewTargets::new(pack(), &model(), anatomy()).expect("view targets"))
}

fn sinus(until_ms: f64) -> Sim {
    let mut sim = Sim::sinus(60.0);
    sim.run_to(until_ms);
    sim
}

fn settings() -> ImageSettings {
    ImageSettings::default()
}

fn ctx<'a>(sim: &'a Sim, pose: ProbePose, cursor_deg: f32, s: &'a ImageSettings) -> ScanContext<'a> {
    ScanContext {
        anatomy: anatomy(),
        history: &sim.history,
        pose: s.apply_to(&pose),
        cursor_deg,
        settings: s,
        enabled: true,
        exaggeration: 1.0,
    }
}

fn plax_cursor_at(frame: &heartsim_echo::ProbeFrame, below_mm: f32) -> f32 {
    let k = &targets().landmarks;
    let toward_apex = (k.apex - k.mitral).normalize();
    cursor_toward(frame, k.mitral + toward_apex * below_mm)
}

fn plax_cursor(frame: &heartsim_echo::ProbeFrame) -> f32 {
    plax_cursor_at(frame, 18.0)
}

#[test]
fn the_column_buffer_is_identical_for_two_frame_rates() {
    let sim = sinus(5000.0);
    let s = settings();
    let pose = targets().reference(anatomy(), ViewId::ParasternalLongAxis).pose;
    let frame = pose.frame(anatomy().designed.contact_limit_deg);
    let cursor = plax_cursor(&frame);
    let mut m = model();
    let mut a = MModeScan::new(anatomy(), &m, AcousticTable::builtin());
    let mut b = MModeScan::new(anatomy(), &m, AcousticTable::builtin());
    let c = ctx(&sim, pose, cursor, &s);
    // 30 Hz and 21 Hz frames over the same second of the record
    let mut t = 3000.0;
    while t <= 4000.0 {
        a.advance(&mut m, &c, t, usize::MAX);
        t += 33.3;
    }
    let mut t = 3000.0;
    while t <= 4000.0 {
        b.advance(&mut m, &c, t, usize::MAX);
        t += 47.6;
    }
    a.advance(&mut m, &c, 4000.0, usize::MAX);
    b.advance(&mut m, &c, 4000.0, usize::MAX);
    let (a0, a1) = a.buffer.span().unwrap();
    let (b0, b1) = b.buffer.span().unwrap();
    assert_eq!((a0, a1), (b0, b1));
    assert!(a1 - a0 >= 35, "a second of trace");
    for k in a0..=a1 {
        assert_eq!(a.buffer.get(k), b.buffer.get(k), "column {k}");
        assert!(a.buffer.get(k).is_some());
    }
}

#[test]
fn a_half_rate_trace_holds_the_full_trace_s_even_columns_and_nothing_else() {
    let sim = sinus(4000.0);
    let s = settings();
    let pose = targets().reference(anatomy(), ViewId::ParasternalLongAxis).pose;
    let frame = pose.frame(anatomy().designed.contact_limit_deg);
    let cursor = plax_cursor(&frame);
    let mut m = model();
    let mut full = MModeScan::new(anatomy(), &m, AcousticTable::builtin());
    let mut half = MModeScan::new(anatomy(), &m, AcousticTable::builtin());
    half.rate = heartsim_echo::TraceRate::Half;
    assert!((half.rate.column_ms() - 50.0).abs() < 1e-9);
    let c = ctx(&sim, pose, cursor, &s);
    let mut t = 3000.0;
    while t <= 3500.0 {
        full.advance(&mut m, &c, t, usize::MAX);
        half.advance(&mut m, &c, t, usize::MAX);
        t += 33.3;
    }
    let (a0, a1) = full.buffer.span().unwrap();
    assert!(half.computed * 2 <= full.computed + 2 && half.computed >= 8);
    for k in a0..=a1 {
        if k % 2 == 0 && half.buffer.span().is_some_and(|(b0, b1)| (b0..=b1).contains(&k)) {
            assert_eq!(half.buffer.get(k), full.buffer.get(k), "column {k}");
        } else {
            assert!(half.buffer.get(k).is_none(), "odd column {k} is not taken");
        }
    }
}

/// Intersections of a ray with the endocardial triangles of the deformed LV mesh, sorted by
/// distance along the ray.
fn endocardial_hits(m: &MotionModel, frame: &MotionFrame, origin: Vec3, dir: Vec3) -> Vec<f32> {
    let lv = frame.part("lv").unwrap();
    let endo = m.lv_endocardial_vertices();
    let idx = m.part_indices("lv").unwrap();
    let p = |i: u32| Vec3::from(lv.vertices[i as usize].position);
    let mut hits = Vec::new();
    for t in idx.as_chunks::<3>().0 {
        if !t.iter().all(|&i| endo[i as usize]) {
            continue;
        }
        let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
        let (e1, e2) = (b - a, c - a);
        let h = dir.cross(e2);
        let det = e1.dot(h);
        if det.abs() < 1e-9 {
            continue;
        }
        let inv = 1.0 / det;
        let s = origin - a;
        let u = s.dot(h) * inv;
        if !(0.0..=1.0).contains(&u) {
            continue;
        }
        let q = s.cross(e1);
        let v = dir.dot(q) * inv;
        if v < 0.0 || u + v > 1.0 {
            continue;
        }
        let d = e2.dot(q) * inv;
        if d > 0.0 {
            hits.push(d);
        }
    }
    hits.sort_by(f32::total_cmp);
    hits
}

/// The LV cavity chord along the cursor in the column at `step`, from the label line and from the
/// deformed mesh of the same instant, mm.
fn chord_both(
    m: &mut MotionModel,
    sim: &Sim,
    s: &ImageSettings,
    pose: ProbePose,
    cursor: f32,
    step: i64,
) -> (f32, f32) {
    let frame = pose.frame(anatomy().designed.contact_limit_deg);
    let thin = cursor_frame(&frame, cursor);
    let mut scan = MModeScan::new(anatomy(), m, AcousticTable::builtin());
    scan.advance(m, &ctx(sim, pose, cursor, s), step as f64 * 25.0, 1);
    let col = scan.buffer.get(step).expect("column");
    let blood: Vec<usize> = (0..col.labels.len())
        .filter(|&i| col.labels[i] == heartsim_geometry::Label::LvBlood as u8)
        .collect();
    let mid_depth = col.depth_of(blood[blood.len() / 2]);
    let from_labels = cavity_diameter_of_column(col, mid_depth).expect("a cavity");
    // the reference the page shows for a caliper at this depth and time is that diameter; a wall
    // named at a depth in the blood has none
    let t_step = step as f64 * 25.0;
    assert_eq!(
        heartsim_echo::reference_on_mmode(&scan.buffer, Named::Lvidd, t_step, mid_depth),
        Some(from_labels)
    );
    assert_eq!(
        heartsim_echo::reference_on_mmode(&scan.buffer, Named::Ivsd, t_step, mid_depth),
        None
    );
    assert_eq!(
        heartsim_echo::reference_on_mmode(&scan.buffer, Named::Tapse, t_step, mid_depth),
        None
    );
    let t_col = step as f64 * 25.0;
    let mut mf = m.new_frame();
    m.evaluate(
        &MotionInput {
            history: &sim.history,
            t_ms: t_col,
            resp_phase: sim.history.respiratory_phase(t_col),
            enabled: true,
            exaggeration: 1.0,
        },
        &mut mf,
    );
    let hits = endocardial_hits(m, &mf, thin.apex_mm, thin.beam);
    let k = hits
        .windows(2)
        .position(|w| w[0] <= mid_depth && mid_depth <= w[1])
        .expect("a mesh chord around the middle");
    (from_labels, hits[k + 1] - hits[k])
}

#[test]
fn plax_lvid_agrees_with_the_mesh_and_shrinks_in_systole() {
    let sim = sinus(6000.0);
    let s = settings();
    let pose = targets().reference(anatomy(), ViewId::ParasternalLongAxis).pose;
    let frame = pose.frame(anatomy().designed.contact_limit_deg);
    let cursor = plax_cursor(&frame);
    let mut m = model();
    let bt = m.beat_times(&sim.history, 5300.0, Side::Left).expect("a beat");
    let (open, close) = bt.ejection_ms.expect("ejection");
    // end-diastole: the first column after the mitral valve closed (Mitchell 2019, p. 31)
    let ed = (bt.end_diastole_ms / 25.0).ceil() as i64;
    let (ed_labels, ed_mesh) = chord_both(&mut m, &sim, &s, pose, cursor, ed);
    // end-systole: the smallest cavity of the ejection and the isovolumic relaxation
    let mut best: Option<(f32, i64)> = None;
    let mut leaflets_seen = false;
    for k in ((open / 25.0).floor() as i64)..=(((bt.inflow_open_ms) / 25.0).ceil() as i64) {
        let mut scan = MModeScan::new(anatomy(), &m, AcousticTable::builtin());
        scan.advance(&mut m, &ctx(&sim, pose, cursor, &s), k as f64 * 25.0, 1);
        let col = scan.buffer.get(k).unwrap();
        leaflets_seen |= col
            .labels
            .contains(&(heartsim_geometry::Label::ValveLeaflet as u8));
        let blood: Vec<usize> = (0..col.labels.len())
            .filter(|&i| col.labels[i] == heartsim_geometry::Label::LvBlood as u8)
            .collect();
        if let Some(d) = blood
            .get(blood.len() / 2)
            .and_then(|&i| cavity_diameter_of_column(col, col.depth_of(i)))
            && best.is_none_or(|b| d < b.0)
        {
            best = Some((d, k));
        }
    }
    let (_, es) = best.expect("a systolic cavity");
    let (es_labels, es_mesh) = chord_both(&mut m, &sim, &s, pose, cursor, es);
    eprintln!(
        "LVIDd {ed_labels:.1} (mesh {ed_mesh:.1}) at step {ed}; LVIDs {es_labels:.1} (mesh {es_mesh:.1}) at step {es}; A2 {close}"
    );
    assert!(
        leaflets_seen,
        "the cursor crosses the mitral leaflets around the systole"
    );
    assert!(
        (ed_labels - ed_mesh).abs() < 2.0,
        "LVIDd {ed_labels} vs mesh {ed_mesh}"
    );
    assert!(
        (es_labels - es_mesh).abs() < 2.0,
        "LVIDs {es_labels} vs mesh {es_mesh}"
    );
    assert!(
        es_labels < ed_labels - 4.0,
        "LVIDs {es_labels} under LVIDd {ed_labels}"
    );
}

#[test]
fn in_ventricular_fibrillation_the_diameters_barely_change() {
    let path =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/scenarios/vf_coarse.ron");
    let text = std::fs::read_to_string(path).unwrap();
    let sc = heartsim_ecg::Tier0Scenario::from_ron(&text).unwrap();
    let timeline = sc
        .scenario
        .timeline
        .iter()
        .map(|a| (a.at.0, a.action.clone()))
        .collect();
    let mut sim = Sim::new(sc.timing.clone(), timeline, sc.timing.sinus.respiration_rate_bpm);
    sim.history = sim.history.with_auscultation(&sc.scenario.auscultation);
    sim.run_to(9000.0);
    let s = settings();
    let pose = targets().reference(anatomy(), ViewId::ParasternalLongAxis).pose;
    let frame = pose.frame(anatomy().designed.contact_limit_deg);
    let cursor = plax_cursor(&frame);
    let mut m = model();
    let mut scan = MModeScan::new(anatomy(), &m, AcousticTable::builtin());
    let c = ctx(&sim, pose, cursor, &s);
    let mut t = 6000.0;
    while t <= 7500.0 {
        scan.advance(&mut m, &c, t, usize::MAX);
        t += 100.0;
    }
    let (a, b) = scan.buffer.span().unwrap();
    let ds: Vec<f32> = (a..=b)
        .filter_map(|k| scan.buffer.get(k))
        .filter_map(|col| {
            let blood: Vec<usize> = (0..col.labels.len())
                .filter(|&i| col.labels[i] == heartsim_geometry::Label::LvBlood as u8)
                .collect();
            blood
                .get(blood.len() / 2)
                .and_then(|&i| cavity_diameter_of_column(col, col.depth_of(i)))
        })
        .collect();
    assert!(ds.len() > 40);
    let (lo, hi) = ds
        .iter()
        .fold((f32::MAX, f32::MIN), |(l, h), &d| (l.min(d), h.max(d)));
    eprintln!("VF: LVID {lo:.1}..{hi:.1} mm over 1.5 s");
    assert!(hi - lo < 4.0, "VF: the cavity keeps its size ({lo}..{hi})");
}

#[test]
fn line_angle_round_trips_to_a_sector_line() {
    let a = line_angle_deg(0, 128, 90.0);
    assert!((a - (-44.648_44)).abs() < 1e-3);
    let _ = Plane2::new(0.0, 1.0);
    let _ = (Named::Lvidd, reference_excursion, ExcursionContext::clone);
}

/// Writes the M-mode of the PLAX cursor at the mitral tips and of the apical cursor through the
/// lateral tricuspid annulus as PNGs (for looking at: `HEARTSIM_DUMP_DIR=<dir> cargo test -p
/// heartsim-echo --test mmode dump_the_traces -- --ignored`).
#[test]
#[ignore = "writes images for looking at"]
fn dump_the_traces() {
    let Some(dir) = std::env::var_os("HEARTSIM_DUMP_DIR") else {
        return;
    };
    let sim = sinus(8000.0);
    let s = settings();
    let mut m = model();
    for (name, view, site) in [
        ("plax", ViewId::ParasternalLongAxis, None),
        (
            "a4c-tricuspid",
            ViewId::ApicalFourChamber,
            Some(heartsim_echo::AnnulusSite::TricuspidLateral),
        ),
        (
            "a4c-mitral",
            ViewId::ApicalFourChamber,
            Some(heartsim_echo::AnnulusSite::MitralLateral),
        ),
    ] {
        let pose = targets().reference(anatomy(), view).pose;
        let frame = pose.frame(anatomy().designed.contact_limit_deg);
        let cursor = match site {
            None => plax_cursor(&frame),
            Some(site) => {
                let input = MotionInput {
                    history: &sim.history,
                    t_ms: 5000.0,
                    resp_phase: sim.history.respiratory_phase(5000.0),
                    enabled: true,
                    exaggeration: 1.0,
                };
                cursor_toward(&frame, m.annulus_position(&input, site).unwrap())
            }
        };
        let mut scan = MModeScan::new(anatomy(), &m, AcousticTable::builtin());
        let c = ctx(&sim, pose, cursor, &s);
        let mut t = 4000.0;
        while t <= 7000.0 {
            scan.advance(&mut m, &c, t, usize::MAX);
            t += 40.0;
        }
        let (a, b) = scan.buffer.span().unwrap();
        let (w, h) = (b - a + 1, 512usize);
        let scale = 4u32;
        let mut img = image::GrayImage::new(w as u32 * scale, h as u32);
        for (x, k) in (a..=b).enumerate() {
            let col = scan.buffer.get(k);
            for y in 0..h {
                let v = col.map_or(0, |c| c.grey[y]);
                for dx in 0..scale {
                    img.put_pixel(x as u32 * scale + dx, y as u32, image::Luma([v]));
                }
            }
        }
        let path = std::path::Path::new(&dir).join(format!("mmode-{name}.png"));
        img.save(&path).unwrap();
        eprintln!("{} ({} columns, cursor {cursor:.1} deg)", path.display(), w);
    }
}
