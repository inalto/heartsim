//! Brief B35: the measured tricuspid valve of the default pack — the bake's output, the fit to this
//! heart, the pose, its timing against the valve schedule and the heart sounds, and the echo's use
//! of the same posed mesh (option (a) of the brief).
#![allow(
    clippy::float_cmp,
    reason = "the valve must be exactly shut at opening 0 and open at 1"
)]

mod common;

use common::{Sim, model, pack};
use glam::Vec3;
use heartsim_ecg::Valve;
use heartsim_echo::phases::Side;
use heartsim_echo::{
    DesignedParams, EchoAnatomy, EchoWindow, LabelImage, MeshValve, MotionInput, PosedMeshValve, SliceState,
    valve_openings,
};
use heartsim_geometry::mesh::closest_point_on_triangle;
use heartsim_geometry::valve::RIM_BINS;
use heartsim_geometry::{AnimatedValve, Label, LabelVolume};
use std::sync::OnceLock;

fn anatomy() -> &'static EchoAnatomy {
    static A: OnceLock<EchoAnatomy> = OnceLock::new();
    A.get_or_init(|| EchoAnatomy::new(pack(), &model(), DesignedParams::default()).expect("anatomy"))
}

fn valve() -> &'static AnimatedValve {
    pack()
        .valve("tricuspid")
        .expect("the default pack has the tricuspid valve")
}

fn mesh() -> &'static MeshValve {
    anatomy()
        .mesh_valve()
        .expect("the echo anatomy uses the measured valve")
}

fn heart() -> &'static LabelVolume {
    &pack().volumes.as_ref().expect("volumes").heart
}

/// Distance from `p` to the nearest voxel box whose label passes `want`, within `reach` mm.
fn distance_to(vol: &LabelVolume, p: Vec3, reach: f32, want: impl Fn(u8) -> bool) -> f32 {
    if vol.get(p).is_some_and(&want) {
        return 0.0;
    }
    let s = vol.spacing_mm;
    let r = (reach / s).ceil() as i64;
    let rel = (p - vol.origin_mm) / s;
    let c = [rel.x.floor() as i64, rel.y.floor() as i64, rel.z.floor() as i64];
    let d = vol.dims.map(i64::from);
    let mut best = f32::INFINITY;
    for k in (c[2] - r).max(0)..=(c[2] + r).min(d[2] - 1) {
        for j in (c[1] - r).max(0)..=(c[1] + r).min(d[1] - 1) {
            for i in (c[0] - r).max(0)..=(c[0] + r).min(d[0] - 1) {
                if want(vol.labels[vol.index(i as u32, j as u32, k as u32)]) {
                    let lo = vol.origin_mm + Vec3::new(i as f32, j as f32, k as f32) * s;
                    best = best.min(p.clamp(lo, lo + Vec3::splat(s)).distance(p));
                }
            }
        }
    }
    best
}

fn segment_distance(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-12)).clamp(0.0, 1.0);
    (a + ab * t).distance(p)
}

/// Distance from `p` to the closed rim polyline.
fn rim_distance(rim: &[Vec3], p: Vec3) -> f32 {
    (0..rim.len())
        .map(|b| segment_distance(p, rim[b], rim[(b + 1) % rim.len()]))
        .fold(f32::INFINITY, f32::min)
}

// --- the bake ----------------------------------------------------------------------------------------

#[test]
fn every_triangle_keeps_a_leaflet_and_the_areas_are_the_sources_scaled() {
    let v = valve();
    assert_eq!(v.leaflet_names, ["anterior", "septal", "posterior"]);
    assert_eq!(v.triangle_leaflet.len(), v.triangles.len());
    assert!(v.triangle_leaflet.iter().all(|l| *l < 3));
    for l in 0..3u8 {
        assert!(v.triangle_leaflet.contains(&l), "leaflet {l} has no triangle");
    }
    assert!(v.keyframe_count() <= 21 && v.keyframe_count() >= 2);
    let areas = v.leaflet_areas_mm2(&v.rest_mm);
    let s2 = v.fit_scale * v.fit_scale;
    for (k, name) in v.leaflet_names.iter().enumerate() {
        let ratio = areas[k] / (v.source_leaflet_area_mm2[k] * s2);
        eprintln!(
            "{name}: {:.0} mm² fitted, {:.0} mm² source, ratio to source × scale² ({s2:.2}) {ratio:.2}",
            areas[k], v.source_leaflet_area_mm2[k]
        );
        // stated factor: the fitted area is the source's times the similarity scale squared,
        // within 0.8–1.25 (the warp and the decimation change it a little)
        assert!((0.8..=1.25).contains(&ratio), "{name}: ratio {ratio:.2}");
    }
    eprintln!(
        "{} vertices ({} leaflet), {} triangles, {} chordae, {} shapes, {} bytes",
        v.rest_mm.len(),
        v.leaflet_vertices,
        v.triangles.len(),
        v.chordae.len(),
        v.keyframe_count(),
        v.encode().len()
    );
}

// --- the fit -------------------------------------------------------------------------------------------

#[test]
fn the_annulus_lies_on_the_disc_rim_at_rest() {
    let m = mesh();
    let rim = m.carrier().rest_rim();
    assert_eq!(rim.len(), RIM_BINS);
    let worst = valve()
        .annulus
        .iter()
        .map(|&a| rim_distance(rim, valve().rest_mm[a as usize]))
        .fold(0.0f32, f32::max);
    eprintln!(
        "annulus to disc rim at rest: worst {worst:.4} mm over {} vertices",
        valve().annulus.len()
    );
    assert!(worst <= 0.5, "{worst}");
    // every keyframe keeps the annulus on the rest rim too (the donor's annulus motion is
    // replaced by this heart's)
    let mut pos = Vec::new();
    for k in 0..valve().keyframe_count() {
        valve().keyframe_positions(k, &mut pos);
        let w = valve()
            .annulus
            .iter()
            .map(|&a| rim_distance(rim, pos[a as usize]))
            .fold(0.0f32, f32::max);
        assert!(w <= 0.5, "keyframe {k}: {w}");
    }
}

#[test]
fn the_septal_leaflet_is_on_the_septum_and_the_anterior_toward_the_outflow() {
    let v = valve();
    let vol = heart();
    // the septum's right-ventricular face: LV myocardium beside RV blood
    let mut septum = Vec::new();
    for idx in 0..vol.len() {
        if vol.labels[idx] != Label::LvMyocardium as u8 {
            continue;
        }
        let [i, j, k] = vol.coords(idx);
        let c = vol.centre(i, j, k);
        let beside = [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z]
            .iter()
            .any(|d| vol.get(c + *d * vol.spacing_mm) == Some(Label::RvBlood as u8));
        if beside {
            septum.push(c);
        }
    }
    assert_ne!(septum.len(), 0);
    let centroid = |l: u8| -> Vec3 {
        let (mut s, mut a) = (Vec3::ZERO, 0.0f32);
        for (t, x) in v.triangles.iter().zip(&v.triangle_leaflet) {
            if *x == l {
                let [p, q, r] = t.map(|i| v.rest_mm[i as usize]);
                let area = (q - p).cross(r - p).length() * 0.5;
                s += (p + q + r) / 3.0 * area;
                a += area;
            }
        }
        s / a
    };
    let to_septum: Vec<f32> = (0..3u8)
        .map(|l| {
            let c = centroid(l);
            septum.iter().map(|s| s.distance(c)).fold(f32::INFINITY, f32::min)
        })
        .collect();
    eprintln!("leaflet centroid to the septum (anterior, septal, posterior): {to_septum:.1?} mm");
    assert!(to_septum[1] < to_septum[0] && to_septum[1] < to_septum[2]);
    // the outflow: the pulmonary disc; angles about the tricuspid disc's axis
    let m = model();
    let rim = mesh().carrier().rim();
    let pv = m.frame().valve_centres_mm[Valve::Pulmonic.index()];
    let ang = |p: Vec3| {
        let d = p - rim.centre_mm;
        d.dot(rim.v).atan2(d.dot(rim.u))
    };
    let dist = |a: f32, b: f32| {
        let d = (a - b).rem_euclid(std::f32::consts::TAU);
        d.min(std::f32::consts::TAU - d).to_degrees()
    };
    let out = ang(pv);
    let (a, p) = (dist(ang(centroid(0)), out), dist(ang(centroid(2)), out));
    eprintln!(
        "angle about the annulus to the outflow (pulmonary valve): anterior {a:.0}°, posterior {p:.0}°"
    );
    assert!(a < p, "the anterior leaflet is not on the outflow side");
}

#[test]
fn no_leaflet_vertex_leaves_the_right_heart_blood_by_more_than_1_mm() {
    let v = valve();
    let vol = heart();
    let blood = |l: u8| l == Label::RvBlood as u8 || l == Label::RaBlood as u8;
    let mut worst = (0.0f32, 0usize, 0usize, Vec3::ZERO);
    let mut pos = Vec::new();
    for k in 0..v.keyframe_count() {
        v.keyframe_positions(k, &mut pos);
        for (i, p) in pos[..v.leaflet_vertices as usize].iter().enumerate() {
            let d = distance_to(vol, *p, 6.0, blood);
            if d > worst.0 {
                worst = (d, k, i, *p);
            }
        }
    }
    eprintln!(
        "worst leaflet vertex outside RA/RV blood: {:.2} mm (keyframe {}, vertex {}, at {:.1?}, label {:?})",
        worst.0,
        worst.1,
        worst.2,
        worst.3,
        vol.get(worst.3).and_then(Label::from_u8)
    );
    assert!(worst.0 <= 1.0);
}

// --- the pose ------------------------------------------------------------------------------------------

#[test]
fn the_pose_is_the_keyframes_at_the_ends_continuous_and_monotone_between() {
    let v = valve();
    let (mut a, mut b) = (Vec::new(), Vec::new());
    v.pose_rest(1.0, &mut a);
    v.keyframe_positions(0, &mut b);
    assert_eq!(a, b, "opening 1 is the first keyframe");
    v.pose_rest(0.0, &mut a);
    v.keyframe_positions(v.keyframe_count() - 1, &mut b);
    assert_eq!(a, b, "opening 0 is the last keyframe");
    let steps = 1000;
    let mut prev_metric = f32::NEG_INFINITY;
    let mut prev: Vec<Vec3> = Vec::new();
    let mut largest_jump = 0.0f32;
    for k in 0..=steps {
        let o = k as f32 / steps as f32;
        v.pose_rest(o, &mut a);
        let m = v.closure_metric_mm(&a);
        assert!(
            m >= prev_metric - 1e-4,
            "closure metric falls as the valve opens at {o}: {prev_metric} → {m}"
        );
        prev_metric = m;
        if !prev.is_empty() {
            largest_jump = largest_jump.max(
                a.iter()
                    .zip(&prev)
                    .map(|(x, y)| x.distance(*y))
                    .fold(0.0, f32::max),
            );
        }
        prev.clone_from(&a);
    }
    eprintln!("largest vertex move per 0.001 of opening: {largest_jump:.3} mm");
    assert!(largest_jump < 0.5, "the pose jumps ({largest_jump} mm)");
}

/// Penetration of triangle `t` through the plane of `u`: the smaller of its two excursions.
fn excursion(t: [Vec3; 3], u: [Vec3; 3]) -> f32 {
    let n = (u[1] - u[0]).cross(u[2] - u[0]).normalize_or_zero();
    let d = t.map(|p| (p - u[0]).dot(n));
    let up = d.iter().copied().fold(0.0f32, f32::max);
    let down = -d.iter().copied().fold(0.0f32, f32::min);
    up.min(down)
}

fn segment_hits_triangle(p: Vec3, q: Vec3, t: [Vec3; 3]) -> bool {
    // Möller–Trumbore on the segment
    let (e1, e2) = (t[1] - t[0], t[2] - t[0]);
    let dir = q - p;
    let h = dir.cross(e2);
    let a = e1.dot(h);
    if a.abs() < 1e-9 {
        return false;
    }
    let f = 1.0 / a;
    let s = p - t[0];
    let u = f * s.dot(h);
    if !(0.0..=1.0).contains(&u) {
        return false;
    }
    let qv = s.cross(e1);
    let w = f * dir.dot(qv);
    if w < 0.0 || u + w > 1.0 {
        return false;
    }
    let tt = f * e2.dot(qv);
    (0.0..=1.0).contains(&tt)
}

#[test]
fn no_leaflet_passes_through_another_by_more_than_the_drawn_thickness() {
    let v = valve();
    let thickness = DesignedParams::default().av_leaflet_thickness_mm;
    let mut pos = Vec::new();
    let mut worst = (0.0f32, 0.0f32);
    for k in 0..=40 {
        let o = k as f32 / 40.0;
        v.pose_rest(o, &mut pos);
        // a 3 mm grid of triangle boxes
        let cell = 3.0f32;
        let mut grid: std::collections::HashMap<[i32; 3], Vec<usize>> = std::collections::HashMap::new();
        for (ti, t) in v.triangles.iter().enumerate() {
            let [a, b, c] = t.map(|i| pos[i as usize]);
            let (lo, hi) = (a.min(b).min(c) / cell, a.max(b).max(c) / cell);
            for z in lo.z.floor() as i32..=hi.z.floor() as i32 {
                for y in lo.y.floor() as i32..=hi.y.floor() as i32 {
                    for x in lo.x.floor() as i32..=hi.x.floor() as i32 {
                        grid.entry([x, y, z]).or_default().push(ti);
                    }
                }
            }
        }
        let mut checked = std::collections::HashSet::new();
        for list in grid.values() {
            for (i, &ta) in list.iter().enumerate() {
                for &tb in &list[i + 1..] {
                    let (la, lb) = (v.triangle_leaflet[ta], v.triangle_leaflet[tb]);
                    if la == lb || !checked.insert((ta.min(tb), ta.max(tb))) {
                        continue;
                    }
                    let (a, b) = (v.triangles[ta], v.triangles[tb]);
                    if a.iter().any(|x| b.contains(x)) {
                        continue;
                    }
                    let pa = a.map(|i| pos[i as usize]);
                    let pb = b.map(|i| pos[i as usize]);
                    let hit = (0..3).any(|e| segment_hits_triangle(pa[e], pa[(e + 1) % 3], pb))
                        || (0..3).any(|e| segment_hits_triangle(pb[e], pb[(e + 1) % 3], pa));
                    if hit {
                        let pen = excursion(pa, pb).min(excursion(pb, pa));
                        if pen > worst.0 {
                            worst = (pen, o);
                        }
                    }
                }
            }
        }
    }
    eprintln!(
        "deepest crossing of one leaflet through another: {:.2} mm (at opening {:.3}); drawn thickness {thickness} mm",
        worst.0, worst.1
    );
    assert!(worst.0 <= thickness);
}

// --- riding the beating heart --------------------------------------------------------------------------

fn input(sim: &Sim, t_ms: f64) -> MotionInput<'_> {
    MotionInput {
        history: &sim.history,
        t_ms,
        resp_phase: sim.resp_phase(t_ms),
        enabled: true,
        exaggeration: 1.0,
    }
}

#[test]
fn the_annulus_follows_the_deformed_disc_over_a_sinus_beat_and_poses_are_bit_identical() {
    let mut m = model();
    let mv = mesh();
    let mut sim = Sim::sinus(70.0);
    sim.run_to(4200.0);
    let d = DesignedParams::default();
    let mut frame = m.new_frame();
    let (mut scratch, mut out, mut again) = (Vec::new(), Vec::new(), Vec::new());
    let mut worst = 0.0f32;
    let mut largest_move = 0.0f32;
    let mut t = 3000.0;
    while t < 3900.0 {
        m.evaluate(&input(&sim, t), &mut frame);
        let o = valve_openings(&sim.history, m.params(), m.ef_scale(), t, &d)[Valve::Tricuspid.index()];
        mv.pose(o, Some(&frame), &mut scratch, &mut out);
        mv.pose(o, Some(&frame), &mut scratch, &mut again);
        assert!(
            out.iter()
                .zip(&again)
                .all(|(a, b)| a.to_array().map(f32::to_bits) == b.to_array().map(f32::to_bits)),
            "two poses of the instant {t} differ"
        );
        let disc = mv.disc_now(&frame).expect("disc");
        let rim = mv.carrier().rim().rim_points(&disc);
        for &a in &valve().annulus {
            worst = worst.max(rim_distance(&rim, out[a as usize]));
            largest_move = largest_move.max(out[a as usize].distance(valve().rest_mm[a as usize]));
        }
        t += 10.0;
    }
    eprintln!(
        "annulus to the deformed disc rim over a beat: worst {worst:.4} mm (annulus moves up to {largest_move:.1} mm)"
    );
    assert!(largest_move > 2.0, "the annulus should move with the heart");
    assert!(worst <= 0.5);
    // a second, separately evolved model gives the same bits at the same instant
    let mut m2 = model();
    let mut f2 = m2.new_frame();
    m2.evaluate(&input(&sim, 3456.0), &mut f2);
    m.evaluate(&input(&sim, 3456.0), &mut frame);
    let o = valve_openings(&sim.history, m.params(), m.ef_scale(), 3456.0, &d)[Valve::Tricuspid.index()];
    mv.pose(o, Some(&frame), &mut scratch, &mut out);
    mv.pose(o, Some(&f2), &mut scratch, &mut again);
    assert_eq!(out, again);
}

// --- timing ------------------------------------------------------------------------------------------

/// Over `[from, to)` in 1 ms steps: the mesh is the shut keyframe exactly when the tricuspid
/// opening is 0 and the open one exactly when it is 1; and each beat's closure starts at the T1
/// the heart sound uses, within one step.
fn check_timing(sim: &Sim, from: f64, to: f64, label: &str) {
    let m = model();
    let v = valve();
    let d = DesignedParams::default();
    let (mut shut, mut open, mut pos) = (Vec::new(), Vec::new(), Vec::new());
    v.keyframe_positions(v.keyframe_count() - 1, &mut shut);
    v.keyframe_positions(0, &mut open);
    let opening =
        |t: f64| valve_openings(&sim.history, m.params(), m.ef_scale(), t, &d)[Valve::Tricuspid.index()];
    let (mut n_shut, mut n_open, mut n_between) = (0, 0, 0);
    let mut t = from;
    while t < to {
        let o = opening(t);
        v.pose_rest(o, &mut pos);
        if o == 0.0 {
            assert_eq!(pos, shut, "{label}: opening 0 at {t} but not the shut shape");
            n_shut += 1;
        } else if o == 1.0 {
            assert_eq!(pos, open, "{label}: opening 1 at {t} but not the open shape");
            n_open += 1;
        } else {
            assert!(
                pos != shut && pos != open,
                "{label}: partly open at {t} ({o}) but drawn shut or open"
            );
            n_between += 1;
        }
        t += 1.0;
    }
    // T1: the tricuspid closure the heart sound places (or, without one, what the schedule uses)
    let mut beats = 0;
    for (b, c) in sim.history.beats() {
        let q = b.qrs_onset.0;
        if q < from || q > to - 400.0 {
            continue;
        }
        let Some(bt) = m.beat_times(&sim.history, q + 300.0, Side::Right) else {
            continue;
        };
        let t1 = bt.end_diastole_ms;
        if let Some(c1) = c.t1 {
            assert!(
                (c1.onset.0 - t1).abs() < 1e-9 || t1 > c1.onset.0,
                "{label}: closure {t1} before T1 {}",
                c1.onset.0
            );
        }
        // the closure starts at T1: from there the opening only falls, at once, and it is 0 by
        // the end of the designed closing time
        let at = opening(t1);
        if at > 0.0 {
            assert!(
                opening(t1 + 1.0) < at,
                "{label}: the valve does not start to close at T1 {t1}"
            );
            let mut prev = at;
            let mut s = t1 + 1.0;
            while s <= t1 + d.av_closing_ms + 1.0 {
                let o = opening(s);
                assert!(o <= prev, "{label}: the valve reopens at {s} after T1 {t1}");
                prev = o;
                s += 1.0;
            }
        }
        assert_eq!(
            opening(t1 + d.av_closing_ms + 1.0),
            0.0,
            "{label}: not shut after T1 {t1}"
        );
        beats += 1;
    }
    eprintln!(
        "{label}: {n_shut} ms shut, {n_open} ms fully open, {n_between} ms between; {beats} beats checked at T1"
    );
    assert!(n_shut > 0 && n_between > 0 && beats > 0, "{label}");
}

#[test]
fn the_mesh_valve_is_shut_and_open_with_the_schedule_in_sinus_av_block_and_af() {
    let mut sinus = Sim::sinus(70.0);
    sinus.run_to(6000.0);
    check_timing(&sinus, 2000.0, 6000.0, "sinus 70/min");
    let mut block = Sim::from_scenario("avb_3rd_junctional");
    block.run_to(8000.0);
    check_timing(&block, 2000.0, 8000.0, "complete AV block");
    let mut af = Sim::from_scenario("atrial_fibrillation");
    af.run_to(8000.0);
    check_timing(&af, 2000.0, 8000.0, "atrial fibrillation");
}

// --- the echo (option (a)) -------------------------------------------------------------------------------

#[test]
fn the_echo_labels_leaflet_exactly_within_the_thickness_of_the_posed_mesh() {
    let a = anatomy();
    let mv = mesh();
    let mut m = model();
    let mut sim = Sim::sinus(70.0);
    sim.run_to(3500.0);
    let mut frame = m.new_frame();
    let inp = input(&sim, 3300.0);
    m.evaluate(&inp, &mut frame);
    let mut st = SliceState::new(a);
    st.update(a, &m, &frame, &inp);
    let base = a.window_pose(EchoWindow::Apical);
    let h = mv.half_thickness_mm();
    let vol = heart();
    let (mut checked, mut leaflet, mut poses_with_leaflet) = (0usize, 0usize, 0usize);
    let mut posed = PosedMeshValve::default();
    for opening in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
        let mut o = st.openings();
        o[Valve::Tricuspid.index()] = opening;
        st.set_openings(a, o);
        posed.update(mv, opening);
        let tris = &valve().triangles;
        for k in 0..20 {
            let mut pose = base;
            pose.rotation_deg += -30.0 + 3.0 * k as f32;
            pose.tilt_deg += if k % 2 == 0 { -6.0 } else { 6.0 };
            let mut img = LabelImage::new(96, 384);
            st.slice(a, &pose, &mut img);
            let mut here = 0usize;
            for (i, &l) in img.labels.iter().enumerate() {
                if !img.from_heart[i] {
                    continue;
                }
                let x = img.rest_mm[i];
                let (lo, hi) = posed.bounds_mm();
                if !(x.cmpge(lo).all() && x.cmple(hi).all()) {
                    continue;
                }
                // only blood is overdrawn by leaflets; points the other valves' cones hold are not
                // this valve's business
                let Some(base_label) = vol.get(x).and_then(Label::from_u8) else {
                    continue;
                };
                if !base_label.is_blood() {
                    continue;
                }
                let others = a
                    .valves()
                    .iter()
                    .any(|c| c.valve != Valve::Tricuspid && c.contains(x, st.openings()[c.valve.index()]));
                if others {
                    continue;
                }
                let near = tris.iter().any(|t| {
                    let [p, q, r] = t.map(|j| posed.positions()[j as usize]);
                    closest_point_on_triangle(x, p, q, r).distance_squared(x) <= h * h
                });
                assert_eq!(
                    l == Label::ValveLeaflet as u8,
                    near,
                    "opening {opening}, pose {k}: sample at {x} labelled {:?}, within the thickness {near}",
                    Label::from_u8(l)
                );
                checked += 1;
                if near {
                    leaflet += 1;
                    here += 1;
                }
            }
            if here > 0 {
                poses_with_leaflet += 1;
            }
        }
    }
    eprintln!(
        "{checked} samples checked in the valve's box, {leaflet} leaflet, {poses_with_leaflet} of 100 images show it"
    );
    assert!(leaflet > 100 && poses_with_leaflet >= 50);
}
