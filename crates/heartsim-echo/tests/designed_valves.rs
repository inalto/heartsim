//! Brief B36: the designed mitral, aortic and pulmonary valves of the default pack — the fit and
//! orientation on this heart, containment in each valve's blood, competence shut and the orifice
//! open, the keyframes, the ride on the beating heart, the timing against the valve schedule and
//! the heart sounds, and the echo's use of the same posed meshes.
#![allow(
    clippy::float_cmp,
    reason = "a valve must be exactly shut at opening 0 and open at 1"
)]

mod common;

use common::{Sim, model, pack};
use glam::Vec3;
use heartsim_ecg::Valve;
use heartsim_echo::mesh_valve::thickness_mm;
use heartsim_echo::phases::Side;
use heartsim_echo::{
    DesignedParams, ECHO_CONE_VALVES, EchoAnatomy, EchoWindow, LabelImage, MeshValve, MotionInput,
    PosedMeshValve, ProbePose, SliceState, valve_openings,
};
use heartsim_geometry::valve_design::{DiscFrame, checks};
use heartsim_geometry::{AnimatedValve, Label, LabelVolume};
use std::sync::OnceLock;

const DESIGNED: [Valve; 3] = [Valve::Mitral, Valve::Aortic, Valve::Pulmonic];

fn anatomy() -> &'static EchoAnatomy {
    static A: OnceLock<EchoAnatomy> = OnceLock::new();
    A.get_or_init(|| EchoAnatomy::new(pack(), &model(), DesignedParams::default()).expect("anatomy"))
}

/// Every pack valve on its disc, as the 3-D heart has them (the echo keeps the cone of the
/// valves in `ECHO_CONE_VALVES`).
fn mesh(v: Valve) -> &'static MeshValve {
    static M: OnceLock<Vec<MeshValve>> = OnceLock::new();
    M.get_or_init(|| MeshValve::all(pack(), &model(), &DesignedParams::default()))
        .iter()
        .find(|m| m.valve_id == v)
        .unwrap_or_else(|| panic!("the default pack has the {v:?} valve"))
}

fn data(v: Valve) -> &'static AnimatedValve {
    &mesh(v).data
}

fn heart() -> &'static LabelVolume {
    &pack().volumes.as_ref().expect("volumes").heart
}

fn shape(v: &AnimatedValve, k: usize) -> Vec<Vec3> {
    let mut p = Vec::new();
    v.keyframe_positions(k, &mut p);
    p
}

/// The labels a valve's leaflets must stay in (brief B36, item 3).
fn blood_of(v: Valve) -> [Label; 2] {
    match v {
        Valve::Mitral => [Label::LvBlood, Label::LaBlood],
        Valve::Aortic => [Label::LvBlood, Label::AortaLumen],
        Valve::Pulmonic => [Label::RvBlood, Label::PulmonaryArteryLumen],
        Valve::Tricuspid => [Label::RvBlood, Label::RaBlood],
    }
}

/// The rest disc of a valve as the generator saw it.
fn disc(v: Valve) -> DiscFrame {
    let m = mesh(v);
    let rest = model().part_rest("valve_planes").expect("valve planes").to_vec();
    let pts: Vec<Vec3> = m
        .disc_vertices()
        .iter()
        .map(|&i| Vec3::from(rest[i as usize].position))
        .collect();
    DiscFrame::new(&pts, m.data.coaptation_mm).expect("disc")
}

fn segment_distance(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-12)).clamp(0.0, 1.0);
    (a + ab * t).distance(p)
}

fn rim_distance(rim: &[Vec3], p: Vec3) -> f32 {
    (0..rim.len())
        .map(|b| segment_distance(p, rim[b], rim[(b + 1) % rim.len()]))
        .fold(f32::INFINITY, f32::min)
}

/// Area-weighted centroid of leaflet `l` of `pos`.
fn leaflet_centroid(v: &AnimatedValve, pos: &[Vec3], l: u8) -> Vec3 {
    let (mut s, mut a) = (Vec3::ZERO, 0.0f32);
    for (t, x) in v.triangles.iter().zip(&v.triangle_leaflet) {
        if *x == l {
            let [p, q, r] = t.map(|i| pos[i as usize]);
            let area = (q - p).cross(r - p).length() * 0.5;
            s += (p + q + r) / 3.0 * area;
            a += area;
        }
    }
    s / a
}

/// Angle between the in-plane parts of the directions `a` and `b` (vectors, not points), degrees.
fn plane_angle(d: &DiscFrame, a: Vec3, b: Vec3) -> f32 {
    let flat = |w: Vec3| glam::Vec2::new(w.dot(d.rim.u), w.dot(d.rim.v)).normalize();
    let (x, y) = (flat(a), flat(b));
    x.dot(y).clamp(-1.0, 1.0).acos().to_degrees()
}

// --- the bake --------------------------------------------------------------------------------------

#[test]
fn every_designed_valve_is_in_the_pack_with_its_leaflets_and_keyframes() {
    let names = [
        (Valve::Mitral, vec!["anterior", "posterior"]),
        (
            Valve::Aortic,
            vec!["non-coronary", "right coronary", "left coronary"],
        ),
        (
            Valve::Pulmonic,
            vec!["posterior", "left anterior", "right anterior"],
        ),
    ];
    for (valve, expect) in names {
        let v = data(valve);
        let mut got = v.leaflet_names.clone();
        let mut want: Vec<String> = expect.iter().map(|s| (*s).to_string()).collect();
        got.sort();
        want.sort();
        assert_eq!(got, want, "{valve:?} leaflet names");
        assert_eq!(v.triangle_leaflet.len(), v.triangles.len());
        assert!(
            v.triangle_leaflet
                .iter()
                .all(|l| usize::from(*l) < v.leaflet_names.len())
        );
        assert!((2..=8).contains(&v.keyframe_count()));
        let areas = v.leaflet_areas_mm2(&shape(v, v.keyframe_count() - 1));
        eprintln!(
            "{valve:?}: {} vertices ({} leaflet), {} triangles, {} chordae, {} shapes, {} bytes; shut areas {}",
            v.rest_mm.len(),
            v.leaflet_vertices,
            v.triangles.len(),
            v.chordae.len(),
            v.keyframe_count(),
            v.encode().len(),
            v.leaflet_names
                .iter()
                .zip(&areas)
                .map(|(n, a)| format!("{n} {a:.0} mm²"))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    // K4 (De Kerchove 2018 Table 1), a cross-check, not a gate: 297 ± 41 mm² per flattened cusp on
    // a 24.2 mm junction, scaled by the disc diameter squared
    let v = data(Valve::Aortic);
    let dia = 2.0 * disc(Valve::Aortic).mean_radius_mm();
    let k = (dia / 24.2).powi(2);
    eprintln!(
        "aortic cusp areas (shut) {:.0?} mm² against {:.0} ± {:.0} mm² (297 ± 41 × ({dia:.1} / 24.2)²)",
        v.leaflet_areas_mm2(&shape(v, v.keyframe_count() - 1)),
        297.0 * k,
        41.0 * k
    );
    // the tricuspid is still the measured one
    assert_eq!(
        anatomy()
            .mesh_valve(Valve::Tricuspid)
            .expect("tricuspid")
            .data
            .leaflet_names,
        ["anterior", "septal", "posterior"]
    );
    // the echo draws the meshes, except the pulmonary valve's (its cost gate failed: its cone stays)
    assert_eq!(anatomy().mesh_valves().len(), 4 - ECHO_CONE_VALVES.len());
    for v in Valve::ALL {
        assert_eq!(anatomy().cone_drawn(v), ECHO_CONE_VALVES.contains(&v), "{v:?}");
    }
}

// --- the fit -------------------------------------------------------------------------------------------

#[test]
fn the_annuli_lie_on_their_disc_rims_at_rest() {
    for valve in DESIGNED {
        let v = data(valve);
        let rim = mesh(valve).carrier().rest_rim();
        let d = disc(valve);
        let n = d.normal();
        let mut worst: f32 = 0.0;
        for &a in &v.annulus {
            let p = v.rest_mm[a as usize];
            // the semilunar attachment rises on the crown: its projection on the disc plane
            let q = if valve == Valve::Mitral {
                p
            } else {
                p - n * (p - d.centre()).dot(n)
            };
            worst = worst.max(rim_distance(rim, q));
        }
        eprintln!("{valve:?}: annulus (in projection for the cusps) to the rest rim: worst {worst:.3} mm");
        if valve == Valve::Mitral {
            assert!(worst <= 0.1, "{worst}");
        } else {
            // the attachment is moved toward the axis where the vessel is narrower than the disc
            assert!(worst <= 8.5, "{worst}");
        }
    }
}

#[test]
fn the_mitral_anterior_leaflet_faces_the_aortic_valve_and_the_commissures_their_muscles() {
    let v = data(Valve::Mitral);
    let m = model();
    let aortic = m.frame().valve_centres_mm[Valve::Aortic.index()];
    let (ant, post) = (
        leaflet_centroid(v, &v.rest_mm, 0),
        leaflet_centroid(v, &v.rest_mm, 1),
    );
    let (da, dp) = (ant.distance(aortic), post.distance(aortic));
    eprintln!("leaflet centroid to the aortic disc centre: anterior {da:.1} mm, posterior {dp:.1} mm");
    assert!(
        da < dp,
        "the anterior leaflet is the one nearest the aortic valve"
    );
    // the anterolateral commissure (column 60 of 144: the end of the anterior arc toward the
    // anterolateral side) is nearer the anterolateral muscle than the posteromedial commissure
    // (column 0) is, and the other way round
    let per = 11;
    let (al, pm) = (v.rest_mm[60 * per], v.rest_mm[0]);
    let pap = anatomy().papillary_muscles();
    let (al_base, pm_base) = (pap[0].a_mm, pap[1].a_mm);
    eprintln!(
        "commissures to the muscle bases: anterolateral {:.1} / {:.1} mm, posteromedial {:.1} / {:.1} mm",
        al.distance(al_base),
        al.distance(pm_base),
        pm.distance(al_base),
        pm.distance(pm_base)
    );
    assert!(al.distance(al_base) < pm.distance(al_base));
    assert!(pm.distance(pm_base) < al.distance(pm_base));
    // each muscle's chordae reach the half of the leaflets on its side
    for (k, c) in v.chordae.iter().enumerate() {
        let muscle = k / 6;
        let ins = v.rest_mm[c[1] as usize];
        let (to_own, to_other) = if muscle == 0 {
            (ins.distance(al), ins.distance(pm))
        } else {
            (ins.distance(pm), ins.distance(al))
        };
        assert!(to_own < to_other, "chord {k}: insertion not on its muscle's half");
    }
}

#[test]
fn the_aortic_and_pulmonary_cusps_are_named_by_their_neighbours() {
    let vol = heart();
    let centroid = |l: Label| -> Vec3 {
        let (mut s, mut n) = (Vec3::ZERO, 0.0f32);
        for (i, x) in vol.labels.iter().enumerate() {
            if *x == l as u8 {
                let [a, b, c] = vol.coords(i);
                s += vol.centre(a, b, c);
                n += 1.0;
            }
        }
        s / n
    };
    let m = model();
    let centres = m.frame().valve_centres_mm;
    let a = data(Valve::Aortic);
    let d = disc(Valve::Aortic);
    let id = |v: &AnimatedValve, name: &str| v.leaflet_names.iter().position(|x| x == name).unwrap() as u8;
    // a cusp's direction: its nadir (the attachment point of its centre column, on the disc rim;
    // vertex layout of heartsim_geometry::valve_design: `columns` columns of `rows + 1` vertices
    // per cusp)
    let cd = heartsim_geometry::valve_design::CuspDesign::default();
    let nadir =
        |v: &AnimatedValve, l: u8| v.rest_mm[(usize::from(l) * cd.columns + cd.columns / 2) * (cd.rows + 1)];
    let cdir = |v: &AnimatedValve, name: &str| nadir(v, id(v, name)) - d.centre();
    let septum = (centroid(Label::LaBlood) + centroid(Label::RaBlood)) * 0.5 - d.centre();
    let rv = centroid(Label::RvBlood) - d.centre();
    let ang_ncc =
        ["non-coronary", "right coronary", "left coronary"].map(|n| plane_angle(&d, cdir(a, n), septum));
    let ang_rv = ["right coronary", "left coronary"].map(|n| plane_angle(&d, cdir(a, n), rv));
    eprintln!(
        "aortic cusps to the atrial septum direction (NCC, RCC, LCC): {ang_ncc:.0?}°; to the RV (RCC, LCC): {ang_rv:.0?}°"
    );
    assert!(ang_ncc[0] < ang_ncc[1] && ang_ncc[0] < ang_ncc[2]);
    assert!(ang_rv[0] < ang_rv[1]);
    // the commissure between the non-coronary and the left coronary cusp points at the mitral
    let comm = cdir(a, "non-coronary").normalize() + cdir(a, "left coronary").normalize();
    let to_mitral = centres[Valve::Mitral.index()] - d.centre();
    let ang = plane_angle(&d, comm, to_mitral);
    eprintln!("NCC/LCC commissure to the mitral direction: {ang:.0}° (rule: within 45°)");
    assert!(ang <= 45.0);
    // Ranganathan 1970 (R13): the anterior mitral leaflet shares its attachment with the left
    // coronary cusp and half the non-coronary cusp, so those two are the cusps nearest it
    // (the anterior leaflet's attachment: the middle of its annulus, column 30 of 144, row 0)
    let mv = data(Valve::Mitral);
    let ant = mv.rest_mm[30 * 11];
    let to_ant = ["non-coronary", "right coronary", "left coronary"]
        .map(|n| plane_angle(&d, cdir(a, n), ant - d.centre()));
    eprintln!("aortic cusps to the anterior mitral leaflet's attachment (NCC, RCC, LCC): {to_ant:.0?}°");
    assert!(
        to_ant[1] > to_ant[0] && to_ant[1] > to_ant[2],
        "the right coronary cusp is the farthest"
    );
    // pulmonary (Pu-R1 revised, Lis 2023): the posterior cusp the most posterior, then by x
    let p = data(Valve::Pulmonic);
    let dp = disc(Valve::Pulmonic);
    let pnadir = |name: &str| nadir(p, id(p, name));
    let z = ["posterior", "left anterior", "right anterior"].map(|n| pnadir(n).z);
    eprintln!("pulmonary cusp nadirs, z (posterior, left anterior, right anterior): {z:.1?} mm");
    assert!(z[0] < z[1] && z[0] < z[2]);
    assert!(pnadir("left anterior").x > pnadir("right anterior").x);
    let _ = &dp;
}

#[test]
fn no_leaflet_vertex_leaves_its_valves_blood_by_more_than_1_mm() {
    let vol = heart();
    for valve in DESIGNED {
        let v = data(valve);
        let labels = blood_of(valve);
        let mask = heartsim_geometry::valve_design::BloodMask {
            volume: vol,
            labels: &labels,
        };
        let mut worst = (0.0f32, 0usize, 0usize, Vec3::ZERO);
        for k in 0..v.keyframe_count() {
            let pos = shape(v, k);
            for (i, p) in pos[..v.leaflet_vertices as usize].iter().enumerate() {
                let d = mask.distance_outside(*p, 6.0);
                if d > worst.0 {
                    worst = (d, k, i, *p);
                }
            }
        }
        eprintln!(
            "{valve:?}: worst leaflet vertex outside {labels:?}: {:.2} mm (shape {}, vertex {}, at {:.1?}, label {:?})",
            worst.0,
            worst.1,
            worst.2,
            worst.3,
            vol.get(worst.3).and_then(Label::from_u8)
        );
        assert!(worst.0 <= 1.0);
    }
}

#[test]
fn shut_each_valve_is_competent_and_open_it_leaves_its_orifice() {
    for valve in DESIGNED {
        let v = data(valve);
        let (gap, at) = checks::free_edge_gap_mm(v, &shape(v, v.keyframe_count() - 1));
        let open = checks::orifice_fraction(v, &shape(v, 0), &disc(valve));
        eprintln!(
            "{valve:?}: largest shut free-edge gap {gap:.3} mm (vertex {at}); open orifice {open:.3} of the disc"
        );
        assert!(gap < 1.0, "{valve:?}: gap {gap}");
        // stated fractions (all Designed; research note A14 and the mitral equivalent): the mitral
        // opening leaves at least half the disc; the semilunar cusps 0.75 on a free disc, less here
        // where the containment fit shortened the opening in the tube vessels, never under 0.4
        let need = if valve == Valve::Mitral { 0.5 } else { 0.4 };
        assert!(open >= need, "{valve:?}: open orifice {open}");
    }
}

// --- the keyframes ---------------------------------------------------------------------------------

#[test]
fn the_keyframes_close_monotonically_without_a_snap_or_a_crossing() {
    for valve in DESIGNED {
        let v = data(valve);
        let (mut a, mut prev) = (Vec::new(), 0.0f32);
        for k in &v.keyframes {
            assert!(k.closure > prev);
            prev = k.closure;
        }
        v.pose_rest(1.0, &mut a);
        assert_eq!(a, shape(v, 0), "{valve:?}: opening 1 is the first keyframe");
        v.pose_rest(0.0, &mut a);
        assert_eq!(
            a,
            shape(v, v.keyframe_count() - 1),
            "{valve:?}: opening 0 is the last"
        );
        let (fall, fall_at) = checks::closure_metric_largest_fall(v, 1000);
        let (share, at) = checks::largest_step_share(v);
        let thickness = thickness_mm(valve, &DesignedParams::default());
        let mut deepest = (0.0f32, 0.0f32);
        for k in 0..=1000 {
            let o = k as f32 / 1000.0;
            v.pose_rest(o, &mut a);
            let d = checks::deepest_crossing_mm(v, &a);
            if d > deepest.0 {
                deepest = (d, o);
            }
        }
        eprintln!(
            "{valve:?}: closures {:?}; closure metric largest fall {fall:.5} mm (opening {fall_at:.3}); largest \
             keyframe step {share:.3} of a vertex's path (vertex {at}); deepest crossing {:.3} mm at opening {:.3} \
             (thickness {thickness} mm)",
            v.keyframes
                .iter()
                .map(|k| (k.closure * 1000.0).round() / 1000.0)
                .collect::<Vec<_>>(),
            deepest.0,
            deepest.1
        );
        // monotone up to 5 µm (five quanta of the offsets): on the pack's mitral disc the mean
        // free-edge distance rises by about 2 µm over the last 0.1 % of the closure
        assert!(
            fall <= 5e-3,
            "{valve:?}: the free edge does not approach the coaptation point monotonically"
        );
        assert!(
            share <= 0.25,
            "{valve:?}: a keyframe step takes {share} of a vertex's path"
        );
        assert!(deepest.0 <= thickness, "{valve:?}: leaflets cross");
    }
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
fn each_annulus_follows_its_deformed_disc_over_a_sinus_beat_and_poses_are_bit_identical() {
    let mut m = model();
    let mut sim = Sim::sinus(70.0);
    sim.run_to(4200.0);
    let d = DesignedParams::default();
    let mut frame = m.new_frame();
    let (mut scratch, mut out, mut again) = (Vec::new(), Vec::new(), Vec::new());
    for valve in DESIGNED {
        let mv = mesh(valve);
        let v = &mv.data;
        // the annulus vertices on the rest rim (all of the mitral's; the cusps' nadirs)
        let rest_rim = mv.carrier().rest_rim();
        let on_rim: Vec<u32> = v
            .annulus
            .iter()
            .copied()
            .filter(|&a| rim_distance(rest_rim, v.rest_mm[a as usize]) <= 0.1)
            .collect();
        assert!(!on_rim.is_empty(), "{valve:?}");
        let (mut worst, mut largest_move) = (0.0f32, 0.0f32);
        let mut t = 3000.0;
        while t < 3900.0 {
            m.evaluate(&input(&sim, t), &mut frame);
            let o = valve_openings(&sim.history, m.params(), m.ef_scale(), t, &d)[valve.index()];
            mv.pose(o, Some(&frame), &mut scratch, &mut out);
            mv.pose(o, Some(&frame), &mut scratch, &mut again);
            assert!(
                out.iter()
                    .zip(&again)
                    .all(|(a, b)| a.to_array().map(f32::to_bits) == b.to_array().map(f32::to_bits)),
                "{valve:?}: two poses of the instant {t} differ"
            );
            let disc_now = mv.disc_now(&frame).expect("disc");
            let rim = mv.carrier().rim().rim_points(&disc_now);
            for &a in &on_rim {
                worst = worst.max(rim_distance(&rim, out[a as usize]));
                largest_move = largest_move.max(out[a as usize].distance(v.rest_mm[a as usize]));
            }
            t += 10.0;
        }
        eprintln!(
            "{valve:?}: {} annulus vertices on the rim follow the deformed rim within {worst:.4} mm over a beat (they \
             move up to {largest_move:.1} mm)",
            on_rim.len()
        );
        assert!(
            largest_move > 2.0,
            "{valve:?}: the annulus should move with the heart"
        );
        assert!(worst <= 0.5, "{valve:?}");
    }
}

// --- timing ------------------------------------------------------------------------------------------

/// Over `[from, to)` in 1 ms steps, for each designed valve: the mesh is the shut keyframe exactly
/// when its opening is 0 and the open one exactly when it is 1; the mitral valve starts to close at
/// the M1 the heart sound uses and the aortic is shut at the A2, within one step; the aortic and
/// mitral valves are never both open.
fn check_timing(sim: &Sim, from: f64, to: f64, label: &str) {
    let m = model();
    let d = DesignedParams::default();
    let openings = |t: f64| valve_openings(&sim.history, m.params(), m.ef_scale(), t, &d);
    let mut pos = Vec::new();
    let mut counts = [[0usize; 3]; 3];
    let mut t = from;
    while t < to {
        let o = openings(t);
        assert!(
            o[Valve::Mitral.index()] == 0.0 || o[Valve::Aortic.index()] == 0.0,
            "{label}: mitral and aortic both open at {t}"
        );
        for (k, valve) in DESIGNED.iter().enumerate() {
            let v = data(*valve);
            let x = o[valve.index()];
            v.pose_rest(x, &mut pos);
            if x == 0.0 {
                assert_eq!(
                    pos,
                    shape(v, v.keyframe_count() - 1),
                    "{label}: {valve:?} opening 0 at {t}"
                );
                counts[k][0] += 1;
            } else if x == 1.0 {
                assert_eq!(pos, shape(v, 0), "{label}: {valve:?} opening 1 at {t}");
                counts[k][1] += 1;
            } else {
                assert!(
                    pos != shape(v, 0) && pos != shape(v, v.keyframe_count() - 1),
                    "{label}: {valve:?} partly open at {t} but drawn shut or open"
                );
                counts[k][2] += 1;
            }
        }
        t += 1.0;
    }
    let mut beats = (0, 0);
    for (b, c) in sim.history.beats() {
        let q = b.qrs_onset.0;
        if q < from || q > to - 500.0 {
            continue;
        }
        let Some(bt) = m.beat_times(&sim.history, q + 300.0, Side::Left) else {
            continue;
        };
        // M1: the mitral closure starts there and is done within the designed closing time
        let m1 = bt.end_diastole_ms;
        if let Some(c1) = c.m1 {
            assert!(
                (c1.onset.0 - m1).abs() < 1e-9 || m1 > c1.onset.0,
                "{label}: mitral closure {m1} before M1 {}",
                c1.onset.0
            );
        }
        let om = |t: f64| openings(t)[Valve::Mitral.index()];
        if om(m1) > 0.0 {
            assert!(
                om(m1 + 1.0) < om(m1),
                "{label}: the mitral valve does not start to close at M1 {m1}"
            );
        }
        assert_eq!(
            om(m1 + d.av_closing_ms + 1.0),
            0.0,
            "{label}: mitral not shut after M1 {m1}"
        );
        beats.0 += 1;
        // A2: the aortic valve is shut from the A2 the heart sound uses, open just before it
        if let (Some((_, close)), Some(a2)) = (bt.ejection_ms, c.a2) {
            let oa = |t: f64| openings(t)[Valve::Aortic.index()];
            assert!(
                (close - a2.onset.0).abs() <= 1.0,
                "{label}: aortic closure {close} vs A2 {}",
                a2.onset.0
            );
            assert_eq!(
                oa(close + 1.0),
                0.0,
                "{label}: the aortic valve is not shut at A2 {close}"
            );
            assert!(
                oa(close - 2.0) > 0.0,
                "{label}: the aortic valve is shut before A2 {close}"
            );
            beats.1 += 1;
        }
    }
    eprintln!(
        "{label}: (shut, open, between) ms for mitral, aortic, pulmonary {counts:?}; {} beats checked at M1, {} at A2",
        beats.0, beats.1
    );
    assert!(beats.0 > 0 && beats.1 > 0, "{label}");
    for (k, c) in counts.iter().enumerate() {
        assert!(
            c[0] > 0 && c[2] > 0,
            "{label}: {:?} never shut or never moving",
            DESIGNED[k]
        );
    }
}

#[test]
fn the_designed_valves_follow_the_schedule_in_sinus_av_block_and_af() {
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

// --- the echo ----------------------------------------------------------------------------------------

/// The pose a valve is looked at from (the gates' poses of `benches/valves.rs`).
fn pose_for(valve: Valve) -> ProbePose {
    let a = anatomy();
    let centres = model().frame().valve_centres_mm;
    let para = a.window_pose(EchoWindow::Parasternal);
    let aimed = |target: Vec3, marker: Vec3| {
        ProbePose::aimed(
            para.skin_mm,
            para.normal,
            target,
            marker,
            a.designed.contact_limit_deg,
            para.depth_mm,
            para.sector_deg,
        )
    };
    match valve {
        Valve::Mitral | Valve::Tricuspid => a.window_pose(EchoWindow::Apical),
        Valve::Aortic => aimed(
            centres[Valve::Aortic.index()],
            centres[Valve::Aortic.index()] - centres[Valve::Mitral.index()],
        ),
        Valve::Pulmonic => aimed(centres[Valve::Pulmonic.index()], Vec3::Y),
    }
}

#[test]
fn the_echo_labels_leaflet_exactly_within_the_thickness_of_each_posed_mesh() {
    let a = anatomy();
    let mut m = model();
    let mut sim = Sim::sinus(70.0);
    sim.run_to(3500.0);
    let mut frame = m.new_frame();
    let inp = input(&sim, 3300.0);
    m.evaluate(&inp, &mut frame);
    let vol = heart();
    for valve in DESIGNED.into_iter().filter(|v| !ECHO_CONE_VALVES.contains(v)) {
        let mut st = SliceState::new(a);
        st.update(a, &m, &frame, &inp);
        let mv = mesh(valve);
        let h = mv.half_thickness_mm();
        let base = pose_for(valve);
        let (mut checked, mut leaflet, mut poses_with_leaflet) = (0usize, 0usize, 0usize);
        let mut posed = PosedMeshValve::default();
        for opening in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
            let mut o = st.openings();
            o[valve.index()] = opening;
            st.set_openings(a, o);
            posed.update(mv, opening);
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
                    let Some(base_label) = vol.get(x).and_then(Label::from_u8) else {
                        continue;
                    };
                    if !base_label.is_blood() {
                        continue;
                    }
                    // points another valve holds are not this valve's business
                    let others = a
                        .mesh_valves()
                        .iter()
                        .zip(&st.posed().meshes)
                        .any(|(om, p)| om.valve_id != valve && p.contains(om, x))
                        || a.valves()
                            .iter()
                            .any(|c| a.cone_drawn(c.valve) && c.contains(x, st.openings()[c.valve.index()]));
                    if others {
                        continue;
                    }
                    let near = checks::within_of_leaflets(&mv.data, posed.positions(), x, h);
                    assert_eq!(
                        l == Label::ValveLeaflet as u8,
                        near,
                        "{valve:?}, opening {opening}, pose {k}: sample at {x} labelled {:?}, within the thickness {near}",
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
            "{valve:?}: {checked} samples checked in the valve's box, {leaflet} leaflet, {poses_with_leaflet} of 100 images show it"
        );
        assert!(leaflet > 50 && poses_with_leaflet >= 30, "{valve:?}");
    }
}

#[test]
fn a_pack_with_only_the_tricuspid_valve_draws_the_other_three_as_cones() {
    let mut p = pack().clone();
    p.valves.retain(|v| v.name == "tricuspid");
    let m = model();
    let a = EchoAnatomy::new(&p, &m, DesignedParams::default()).expect("anatomy");
    assert_eq!(a.mesh_valves().len(), 1);
    assert!(!a.cone_drawn(Valve::Tricuspid));
    for v in DESIGNED {
        assert!(a.cone_drawn(v), "{v:?}");
        assert!(a.mesh_valve(v).is_none());
    }
    // the cone of each designed valve labels leaflet at its own points, as before B36
    let posed = a.pose(&[0.0; 4]);
    for c in a.valves().iter().filter(|c| c.valve != Valve::Tricuspid) {
        let x = c.coaptation_mm();
        assert!(c.contains(x, 0.0) && a.leaflet_at(x, &posed), "{:?}", c.valve);
    }
    let mut st = SliceState::new(&a);
    st.set_openings(&a, [0.5; 4]);
    let mut img = LabelImage::new(96, 384);
    st.slice(&a, &pose_for(Valve::Aortic), &mut img);
    let n = img
        .labels
        .iter()
        .fold(0usize, |n, l| n + usize::from(*l == Label::ValveLeaflet as u8));
    eprintln!("tricuspid-only pack, slice through the aortic valve: {n} leaflet samples (cones)");
    assert!(n > 0);
}

/// Brief B36, option (b) of B35 for the pulmonary valve: the echo keeps its designed cone, which
/// must agree with the mesh the 3-D heart draws on the opening (both follow `valve_openings`: shut
/// exactly at 0, fully open at 1) and on the tips within a stated tolerance: the shut meeting
/// point within 5 mm (the mesh's cusps are lowered to fit the pack's short pulmonary lumen, the
/// cone keeps the designed 8.6 mm effective height), the open tips' mean distance from the axis
/// within 4 mm.
#[test]
fn the_echo_s_pulmonary_cone_agrees_with_the_mesh_on_opening_and_tips() {
    if !ECHO_CONE_VALVES.contains(&Valve::Pulmonic) {
        eprintln!("the echo draws the pulmonary mesh: no cone to compare");
        return;
    }
    let a = anatomy();
    let cone = a
        .valves()
        .iter()
        .find(|c| c.valve == Valve::Pulmonic)
        .expect("pulmonary cone");
    assert!(a.cone_drawn(Valve::Pulmonic));
    let v = data(Valve::Pulmonic);
    let d = disc(Valve::Pulmonic);
    let free_centre =
        |pos: &[Vec3]| v.free_edge.iter().map(|&i| pos[i as usize]).sum::<Vec3>() / v.free_edge.len() as f32;
    // shut: the cone's coaptation point against the mesh's shut free-edge centre
    let shut = shape(v, v.keyframe_count() - 1);
    let gap_shut = cone.coaptation_mm().distance(free_centre(&shut));
    // open: mean distance of the tips from the valve axis (the cone's segments at opening 1)
    let open = shape(v, 0);
    let axis_r = |p: Vec3| d.to2(p).length();
    let mesh_r =
        v.free_edge.iter().map(|&i| axis_r(open[i as usize])).sum::<f32>() / v.free_edge.len() as f32;
    let segs = cone.pose(1.0);
    let rim = cone.mean_rim_mm();
    let cone_r = rim + segs.iter().map(|s| s.x).sum::<f32>() / segs.len() as f32;
    eprintln!(
        "pulmonary: shut meeting point cone vs mesh {gap_shut:.1} mm; open tips from the axis: cone {cone_r:.1} mm, \
         mesh {mesh_r:.1} mm"
    );
    assert!(gap_shut <= 5.0, "{gap_shut}");
    assert!((cone_r - mesh_r).abs() <= 4.0, "{cone_r} vs {mesh_r}");
    // the opening: both shut exactly when the schedule's opening is 0, open when it is 1
    let mut sim = Sim::sinus(70.0);
    sim.run_to(5000.0);
    let m = model();
    let dp = DesignedParams::default();
    let mut pos = Vec::new();
    let mut t = 2000.0;
    let mut seen = [false; 2];
    while t < 5000.0 {
        let o = valve_openings(&sim.history, m.params(), m.ef_scale(), t, &dp)[Valve::Pulmonic.index()];
        v.pose_rest(o, &mut pos);
        let mesh_shut = pos == shut;
        let cone_shut = cone.pose(o) == cone.pose(0.0);
        assert_eq!(
            mesh_shut, cone_shut,
            "pulmonary at {t}: mesh shut {mesh_shut}, cone shut {cone_shut} ({o})"
        );
        seen[usize::from(mesh_shut)] = true;
        t += 1.0;
    }
    assert!(seen[0] && seen[1]);
}
