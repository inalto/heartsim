//! Brief B31 part A: the label volumes, the designed parts, the displacement inverse, the probe
//! and the cross-section, on the default pack driven by the coupled Tier-1 engine.

mod common;

use common::{Sim, model, pack};
use glam::Vec3;
use heartsim_ecg::Tier0Scenario;
use heartsim_ecg::Valve;
use heartsim_echo::phases::{CardiacPhase, Side, VolumeInputs, volume_at};
use heartsim_echo::{
    DesignedParams, EchoAnatomy, EchoWindow, LabelImage, MotionChamber, MotionFrame, MotionInput,
    MotionModel, ProbePose, SliceState, valve_openings,
};
use heartsim_geometry::Label;
use std::path::PathBuf;
use std::sync::OnceLock;

fn anatomy() -> &'static EchoAnatomy {
    static A: OnceLock<EchoAnatomy> = OnceLock::new();
    A.get_or_init(|| EchoAnatomy::new(pack(), &model(), DesignedParams::default()).expect("anatomy"))
}

fn input(sim: &Sim, t_ms: f64, enabled: bool) -> MotionInput<'_> {
    MotionInput {
        history: &sim.history,
        t_ms,
        resp_phase: sim.resp_phase(t_ms),
        enabled,
        exaggeration: 1.0,
    }
}

/// Evaluate the model at `t` and take that instant into a slice state.
fn at(
    m: &mut MotionModel,
    sim: &Sim,
    t_ms: f64,
    frame: &mut MotionFrame,
    st: &mut SliceState,
    enabled: bool,
) {
    let inp = input(sim, t_ms, enabled);
    m.evaluate(&inp, frame);
    st.update(anatomy(), m, frame, &inp);
}

fn sinus_sim(until_ms: f64) -> Sim {
    let mut sim = Sim::sinus(60.0);
    sim.run_to(until_ms);
    sim
}

/// The left side's cycle times of the beat whose QRS is the first after `after_ms`.
fn cycle_after(m: &MotionModel, sim: &Sim, after_ms: f64, side: Side) -> heartsim_echo::CycleTimes {
    let inp = VolumeInputs {
        beats: sim.history.beats(),
        atrial_onsets_ms: sim.history.atrial_onsets_ms(),
        params: m.params(),
        pcg: sim.history.pcg_params(),
        ef_scale: m.ef_scale(),
    };
    let q = sim
        .history
        .beats()
        .iter()
        .find(|(b, _)| b.qrs_onset.0 > after_ms)
        .expect("a beat")
        .0
        .qrs_onset
        .0;
    // inside that beat, after its ejection
    let st = volume_at(q + 450.0, side, &inp);
    st.cycle.expect("a cycle")
}

fn heart_centroid() -> Vec3 {
    let h = anatomy().heart_volume();
    let mut s = Vec3::ZERO;
    let mut n = 0usize;
    for idx in 0..h.len() {
        if Label::from_u8(h.labels[idx]).is_some_and(|l| l.is_heart() && l != Label::Pericardium) {
            let [i, j, k] = h.coords(idx);
            s += h.centre(i, j, k);
            n += 1;
        }
    }
    s / n as f32
}

/// The baked heart-box label of a rest point (no designed parts), `None` outside the heart.
fn heart_rest(x: Vec3) -> Option<Label> {
    let l = pack().volumes.as_ref().expect("volumes").heart.label_at(x);
    (l != Label::Outside).then_some(l)
}

// --- heart box -------------------------------------------------------------------------------------

#[test]
fn lv_blood_volume_matches_the_closed_shell() {
    let vols = pack()
        .volumes
        .as_ref()
        .expect("the default pack has its label volumes");
    let voxel_ml = vols.heart.voxel_volume_mm3() / 1000.0;
    let blood = vols.heart.count(Label::LvBlood) as f64 * voxel_ml;
    let shell = model().rest_lv_volume_ml();
    let err = (blood - shell).abs() / shell;
    eprintln!(
        "LV blood {blood:.1} mL, closed shell {shell:.1} mL ({:+.1} %)",
        100.0 * (blood - shell) / shell
    );
    assert!(err < 0.05, "LV blood {blood:.1} mL vs shell {shell:.1} mL");
}

/// Every cell lies in a myocardium voxel of its own chamber. The cells are points of a 2.3 mm
/// lattice inside the tets and the voxels are claimed by their 1 mm centres, so a cell within a
/// millimetre of the wall's surface can sit in the voxel beside it: strictly (the voxel containing
/// the cell) 5–6 % of the cells miss, almost all in the outermost voxel layer, plus the designed
/// appendage lobe (the dataset crops the appendage, so its cells lie outside the heart box's
/// tissue). The brief's 2 % "at boundaries" is checked as: at most 2 % of the cells have no voxel
/// of their chamber's myocardium within one voxel (the voxel or a face neighbour); the strict
/// figure is bounded at 7 % so a shifted volume still fails.
#[test]
fn cells_lie_in_their_own_chambers_myocardium() {
    let vols = pack().volumes.as_ref().expect("volumes");
    let h = &vols.heart;
    let m = model();
    let chamber = &m.coords().chamber;
    let (mut strict, mut far) = (0usize, 0usize);
    for (i, p) in pack().cells.positions.iter().enumerate() {
        let want = match chamber[i] {
            MotionChamber::LeftVentricle => Label::LvMyocardium,
            MotionChamber::RightVentricle => Label::RvMyocardium,
            MotionChamber::LeftAtrium => Label::LaMyocardium,
            MotionChamber::RightAtrium => Label::RaMyocardium,
        };
        if h.label_at(*p) == want {
            continue;
        }
        strict += 1;
        let s = h.spacing_mm;
        let near = [Vec3::X, Vec3::NEG_X, Vec3::Y, Vec3::NEG_Y, Vec3::Z, Vec3::NEG_Z]
            .iter()
            .any(|d| h.label_at(*p + *d * s) == want);
        if !near {
            far += 1;
        }
    }
    let n = pack().cells.len() as f64;
    eprintln!(
        "cells outside their chamber's myocardium: {strict} in the strict voxel ({:.2} %), {far} with none within one voxel ({:.2} %)",
        100.0 * strict as f64 / n,
        100.0 * far as f64 / n
    );
    assert!(
        far as f64 / n <= 0.02,
        "{far} cells far from their chamber's wall"
    );
    assert!(strict as f64 / n <= 0.07, "{strict} cells outside their voxel");
}

#[test]
fn no_blood_voxel_touches_the_outside() {
    let h = &pack().volumes.as_ref().expect("volumes").heart;
    let [nx, ny, nz] = h.dims.map(i64::from);
    let mut leaks = 0usize;
    for idx in 0..h.len() {
        if !Label::from_u8(h.labels[idx]).is_some_and(Label::is_blood) {
            continue;
        }
        let [i, j, k] = h.coords(idx).map(i64::from);
        for (dx, dy, dz) in [
            (1, 0, 0),
            (-1, 0, 0),
            (0, 1, 0),
            (0, -1, 0),
            (0, 0, 1),
            (0, 0, -1),
        ] {
            let (a, b, c) = (i + dx, j + dy, k + dz);
            if a < 0 || b < 0 || c < 0 || a >= nx || b >= ny || c >= nz {
                leaks += 1;
                continue;
            }
            if h.labels[(a + nx * (b + ny * c)) as usize] == Label::Outside as u8 {
                leaks += 1;
            }
        }
    }
    assert_eq!(leaks, 0, "blood voxels face the outside: the fill leaked");
    for l in [
        Label::LvBlood,
        Label::RvBlood,
        Label::LaBlood,
        Label::RaBlood,
        Label::AortaLumen,
        Label::PulmonaryArteryLumen,
    ] {
        assert!(h.count(l) > 1000, "{} has {} voxels", l.name(), h.count(l));
    }
}

// --- thorax box ------------------------------------------------------------------------------------

/// First 0.5 mm sample index along `a → b` that satisfies `f`.
fn first_on(a: Vec3, b: Vec3, f: impl Fn(Vec3) -> bool) -> Option<usize> {
    let n = (a.distance(b) * 2.0).ceil() as usize;
    (0..=n).find(|&i| f(a.lerp(b, i as f32 / n as f32)))
}

#[test]
fn precordial_rays_meet_ribs_and_spaces() {
    let a = anatomy();
    let c = heart_centroid();
    let mut with_bone = Vec::new();
    for name in ["V1", "V2", "V3", "V4", "V5", "V6"] {
        let e = pack().electrodes.get(name).expect("electrode");
        // to where the ray enters the pack heart
        let enter = first_on(e, c, |p| heart_rest(p).is_some()).expect("the ray reaches the heart");
        let n = (e.distance(c) * 2.0).ceil() as usize;
        let end = e.lerp(c, enter as f32 / n as f32);
        let bone = a.segment_crosses(e, end, Label::Bone);
        eprintln!("{name}: {:.0} mm to the heart, bone {bone}", e.distance(end));
        with_bone.push(bone);
    }
    assert!(with_bone.iter().any(|b| *b), "no precordial ray crosses a rib");
    assert!(
        with_bone.iter().any(|b| !*b),
        "every precordial ray crosses bone: no intercostal space"
    );
}

#[test]
fn subxiphoid_ray_crosses_liver_before_heart() {
    let a = anatomy();
    let p = a.window_pose(EchoWindow::Subcostal).skin_mm;
    let c = heart_centroid();
    let liver =
        first_on(p, c, |x| a.thorax_label_at(x) == Label::Liver).expect("the subxiphoid ray crosses liver");
    let heart = first_on(p, c, |x| heart_rest(x).is_some()).expect("and reaches the heart");
    eprintln!(
        "subxiphoid {p:.0?}: liver at {:.0} mm, heart at {:.0} mm",
        liver as f32 * 0.5,
        heart as f32 * 0.5
    );
    assert!(liver < heart);
}

#[test]
fn sternum_is_on_the_midline_in_front_of_the_heart() {
    let a = anatomy();
    let t = a.thorax_volume();
    let h = a.heart_volume();
    let mid = a.midline_x_mm();
    // the most anterior heart voxel at heart level
    let mut front = f32::NEG_INFINITY;
    for idx in 0..h.len() {
        if Label::from_u8(h.labels[idx]).is_some_and(|l| l.is_heart() && l != Label::Pericardium) {
            let [i, j, k] = h.coords(idx);
            front = front.max(h.centre(i, j, k).z);
        }
    }
    let mut found = 0usize;
    for idx in 0..t.len() {
        if t.labels[idx] == Label::Bone as u8 {
            let [i, j, k] = t.coords(idx);
            let c = t.centre(i, j, k);
            if (c.x - mid).abs() < 10.0 && (-20.0..40.0).contains(&c.y) && c.z > front {
                found += 1;
            }
        }
    }
    eprintln!(
        "{found} sternal voxels on the midline (x {mid:.0} mm) in front of the heart (z > {front:.0} mm)"
    );
    assert!(found > 20);
}

#[test]
fn each_lung_has_an_adult_volume() {
    let a = anatomy();
    let t = a.thorax_volume();
    let mid = a.midline_x_mm();
    let (mut l, mut r) = (0usize, 0usize);
    for idx in 0..t.len() {
        if t.labels[idx] == Label::Lung as u8 {
            let [i, j, k] = t.coords(idx);
            if t.centre(i, j, k).x > mid {
                l += 1;
            } else {
                r += 1;
            }
        }
    }
    let v = t.voxel_volume_mm3() / 1e6;
    let (left, right) = (l as f64 * v, r as f64 * v);
    eprintln!("lungs: left {left:.2} L, right {right:.2} L");
    assert!((0.8..=3.5).contains(&left) && (0.8..=3.5).contains(&right));
}

// --- designed parts --------------------------------------------------------------------------------

#[test]
fn leaflets_stay_within_reach_of_their_disc() {
    let a = anatomy();
    let h = a.heart_volume();
    for v in a.valves() {
        let reach = v.reach_mm() + 1.0;
        let c = v.axis_foot_mm();
        let mut hits = 0usize;
        for o in [0.0f32, 0.5, 1.0] {
            let r = reach.ceil() as i32 + 2;
            for dz in -r..=r {
                for dy in -r..=r {
                    for dx in -r..=r {
                        let x = c + Vec3::new(dx as f32, dy as f32, dz as f32) * h.spacing_mm * 0.75;
                        if v.contains(x, o) {
                            hits += 1;
                            assert!(
                                x.distance(c) <= reach,
                                "{:?} leaflet at {:.1} mm > {reach:.1}",
                                v.valve,
                                x.distance(c)
                            );
                        }
                    }
                }
            }
        }
        eprintln!(
            "{:?}: rim {:.1} mm, reach {:.1} mm, coaptation {:.1?}, {hits} leaflet points",
            v.valve,
            v.mean_rim_mm(),
            v.reach_mm(),
            v.coaptation_mm()
        );
        assert!(hits > 0);
    }
}

#[test]
fn closed_leaflets_separate_the_bloods_and_open_ones_do_not() {
    let a = anatomy();
    for v in a.valves() {
        let line = |o: f32| -> (bool, Vec<Label>) {
            let mut leaflet = false;
            let mut bloods = Vec::new();
            let mut openings = [0.0f32; 4];
            openings[v.valve.index()] = o;
            for k in -80..=80 {
                let x = v.centre_mm + v.normal * (k as f32 * 0.25);
                match a.heart_label_at(x, &openings) {
                    Some(Label::ValveLeaflet) => leaflet = true,
                    Some(l) if l.is_blood() && !bloods.contains(&l) => bloods.push(l),
                    _ => {}
                }
            }
            (leaflet, bloods)
        };
        let (shut, bloods) = line(0.0);
        let (open, _) = line(1.0);
        eprintln!(
            "{:?}: closed crosses a leaflet {shut}, open {open}, bloods {bloods:?}",
            v.valve
        );
        assert!(
            shut,
            "{:?}: the closed valve does not separate the two bloods",
            v.valve
        );
        assert!(
            bloods.len() >= 2,
            "{:?}: the line through the centre sees {bloods:?}",
            v.valve
        );
        assert!(
            !open,
            "{:?}: the open valve still blocks its centre line",
            v.valve
        );
    }
}

#[test]
fn valve_timing_follows_the_cycle() {
    let m = model();
    let d = DesignedParams::default();
    let sim = sinus_sim(8000.0);
    for k in 0..3 {
        let c = cycle_after(&m, &sim, 3000.0 + 1000.0 * f64::from(k), Side::Left);
        let (open, close) = c.ejection_ms.expect("a sinus beat ejects");
        let next = cycle_after(&m, &sim, c.qrs_ms + 1.0, Side::Left);
        let prev_close = cycle_after(&m, &sim, c.qrs_ms - 1500.0, Side::Left)
            .ejection_ms
            .map_or(c.closure_ms - 200.0, |e| e.1);
        // from the previous beat's A2 to just before the next beat's ejection
        let mut t = prev_close + 1.0;
        let end = next.ejection_ms.map_or(next.closure_ms + 30.0, |e| e.0);
        while t < end {
            let o = valve_openings(&sim.history, m.params(), m.ef_scale(), t, &d);
            let (mitral, aortic) = (o[Valve::Mitral.index()], o[Valve::Aortic.index()]);
            if t > open && t < close {
                assert_eq!(mitral, 0.0, "mitral open during ejection at {t}");
                assert!(aortic > 0.0, "aortic shut during ejection at {t}");
            } else {
                assert_eq!(aortic, 0.0, "aortic open outside ejection at {t}");
            }
            if t > c.inflow_open_ms && t < next.closure_ms {
                assert!(mitral > 0.0, "mitral shut in filling at {t}");
            }
            if t >= close && t <= c.inflow_open_ms {
                assert_eq!(mitral, 0.0, "mitral open in isovolumic relaxation at {t}");
            }
            t += 2.0;
        }
    }
}

#[test]
fn a_beat_that_does_not_eject_keeps_the_aortic_valve_shut() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/scenarios/pvc_r_on_t.ron");
    let text = std::fs::read_to_string(path)
        .unwrap()
        .replace("coupling_ms: 300.0", "coupling_ms: 280.0");
    let s = Tier0Scenario::from_ron(&text).unwrap();
    let timeline = s
        .scenario
        .timeline
        .iter()
        .map(|a| (a.at.0, a.action.clone()))
        .collect();
    let mut sim = Sim::new(s.timing.clone(), timeline, s.timing.sinus.respiration_rate_bpm);
    sim.history = sim.history.with_auscultation(&s.scenario.auscultation);
    sim.run_to(12_000.0);
    let m = model();
    let d = DesignedParams::default();
    let beats: Vec<_> = sim.history.beats().to_vec();
    let k = beats
        .iter()
        .position(|(b, c)| b.qrs_onset.0 > 3000.0 && c.a2.is_none() && b.organised)
        .expect("a non-ejecting beat");
    // from the previous beat's A2 (an early beat does not cut that ejection short) to the next QRS
    let from = beats[k - 1].1.a2.map_or(beats[k].0.qrs_onset.0, |c| c.onset.0);
    let to = beats[k + 1].0.qrs_onset.0;
    let mut t = from;
    while t < to {
        let o = valve_openings(&sim.history, m.params(), m.ef_scale(), t, &d);
        assert_eq!(
            o[Valve::Aortic.index()],
            0.0,
            "aortic valve opens at {t} in a beat that does not eject"
        );
        t += 1.0;
    }
}

/// Largest AV opening late in each filling (after the E wave and the mid-diastolic closure).
fn late_filling_peaks(sim: &Sim, from: f64, to: f64) -> Vec<f32> {
    let m = model();
    let d = DesignedParams::default();
    let inp = VolumeInputs {
        beats: sim.history.beats(),
        atrial_onsets_ms: sim.history.atrial_onsets_ms(),
        params: m.params(),
        pcg: sim.history.pcg_params(),
        ef_scale: m.ef_scale(),
    };
    let p = m.params();
    let late = p.e_acceleration_ms + p.e_deceleration_ms + d.av_mid_closure_ms + 10.0;
    let mut peaks = Vec::new();
    let mut cur: Option<(f64, f32)> = None;
    let mut t = from;
    while t < to {
        let st = volume_at(t, Side::Left, &inp);
        if st.phase == CardiacPhase::Filling
            && let Some(c) = st.cycle
            && t - c.inflow_open_ms > late
        {
            let o = valve_openings(&sim.history, m.params(), m.ef_scale(), t, &d)[Valve::Mitral.index()];
            cur = Some(match cur {
                Some((k, x)) if k.to_bits() == c.inflow_open_ms.to_bits() => (k, x.max(o)),
                Some((_, x)) => {
                    peaks.push(x);
                    (c.inflow_open_ms, o)
                }
                None => (c.inflow_open_ms, o),
            });
        }
        t += 2.0;
    }
    peaks
}

#[test]
fn atrial_contraction_reopens_the_mitral_valve_except_in_af() {
    let d = DesignedParams::default();
    let sinus = sinus_sim(8000.0);
    let s = late_filling_peaks(&sinus, 3000.0, 8000.0);
    assert!(
        !s.is_empty() && s.iter().all(|x| *x > 0.9),
        "sinus late-filling peaks {s:?}"
    );
    let mut af = Sim::from_scenario("atrial_fibrillation");
    af.run_to(10_000.0);
    let a = late_filling_peaks(&af, 3000.0, 10_000.0);
    eprintln!("late-filling mitral peaks: sinus {s:?}, AF {a:?}");
    assert!(!a.is_empty(), "AF has no late filling");
    assert!(
        a.iter().all(|x| *x <= d.av_diastasis_opening as f32 + 1e-4),
        "AF reopens the mitral valve: {a:?}"
    );
}

#[test]
fn papillary_muscles_stay_in_the_cavity_on_the_wall() {
    let a = anatomy();
    let mut m = model();
    let sim = sinus_sim(5000.0);
    let mut frame = m.new_frame();
    let mut st = SliceState::new(a);
    for pm in a.papillary_muscles() {
        let mid = pm.a_mm.lerp(pm.b_mm, 0.5);
        assert_eq!(
            heart_rest(mid),
            Some(Label::LvBlood),
            "the muscle's middle is not in the LV cavity"
        );
        assert_eq!(
            heart_rest(pm.a_mm),
            Some(Label::LvMyocardium),
            "the muscle's base is not in the LV wall"
        );
        eprintln!(
            "papillary muscle: {:.1} mm long, base {:.0?}",
            pm.length_mm(),
            pm.a_mm
        );
    }
    let mut t = 3000.0;
    while t < 4000.0 {
        at(&mut m, &sim, t, &mut frame, &mut st, true);
        for pm in a.papillary_muscles() {
            let field = st.field();
            let push = |x: Vec3| x + field.at(x);
            let mid = pm.a_mm.lerp(pm.b_mm, 0.5);
            assert_eq!(
                st.label_at(a, push(mid)),
                Label::PapillaryMuscle,
                "t {t}: muscle body lost"
            );
            assert_eq!(
                st.label_at(a, push(pm.a_mm)),
                Label::LvMyocardium,
                "t {t}: muscle base off the wall"
            );
            let tip_side = pm.b_mm + (pm.b_mm - pm.a_mm).normalize() * (pm.radius_b_mm + 1.5);
            // free in the cavity: blood, or an open mitral leaflet reaching toward it
            let l = st.label_at(a, push(tip_side));
            assert!(
                l.is_blood() || l == Label::ValveLeaflet,
                "t {t}: beyond the tip is {l:?}, not the cavity"
            );
        }
        t += 20.0;
    }
}

#[test]
fn moderator_band_crosses_the_rv() {
    let a = anatomy();
    let mb = a.moderator_band();
    let mid = mb.a_mm.lerp(mb.b_mm, 0.5);
    eprintln!(
        "moderator band {:.1} mm long between {:.0?} and {:.0?}",
        mb.length_mm(),
        mb.a_mm,
        mb.b_mm
    );
    assert_eq!(heart_rest(mid), Some(Label::RvBlood));
    assert!(matches!(
        heart_rest(mb.a_mm + (mb.a_mm - mb.b_mm).normalize() * 0.5),
        Some(Label::LvMyocardium | Label::RvMyocardium)
    ));
    assert!(matches!(heart_rest(mb.b_mm), Some(Label::RvMyocardium)));
    let st = SliceState::new(a);
    assert_eq!(st.label_at(a, mid), Label::ModeratorBand);
}

// --- displacement ------------------------------------------------------------------------------------

#[test]
fn inverse_map_recovers_wall_points() {
    let a = anatomy();
    let mut m = model();
    let sim = sinus_sim(5000.0);
    let mut frame = m.new_frame();
    let mut st = SliceState::new(a);
    let c = cycle_after(&m, &sim, 3000.0, Side::Left);
    let (_, es) = c.ejection_ms.expect("ejects");
    let rest = m.part_rest("lv").expect("lv").to_vec();
    let mut worst = 0.0f32;
    for t in [c.closure_ms, es - 20.0, es + 60.0] {
        at(&mut m, &sim, t, &mut frame, &mut st, true);
        let f = st.field();
        // a deterministic spread of LV wall vertices
        for k in (0..rest.len()).step_by(97) {
            let x0 = Vec3::from(rest[k].position);
            let x = x0 + f.at(x0);
            let back = f.rest_of(x);
            let again = back + f.at(back);
            worst = worst.max(again.distance(x));
        }
    }
    eprintln!("inverse map: worst push-forward error {worst:.3} mm");
    assert!(worst < 0.3, "worst {worst} mm");
}

#[test]
fn with_motion_off_the_slice_is_the_rest_volume() {
    let a = anatomy();
    let mut m = model();
    let sim = sinus_sim(4000.0);
    let mut frame = m.new_frame();
    let mut st = SliceState::new(a);
    at(&mut m, &sim, 3500.0, &mut frame, &mut st, false);
    let pose = a.window_pose(EchoWindow::Apical);
    let mut img = LabelImage::new(64, 256);
    st.slice(a, &pose, &mut img);
    let f = pose.frame(a.designed.contact_limit_deg);
    let openings = st.openings();
    for line in 0..img.lines {
        let dir = f.line_direction(line, img.lines, img.sector_deg);
        let mut skin = false;
        for s in 0..img.samples {
            let x = f.apex_mm + dir * img.depth_of(s);
            let mut want = a
                .heart_label_at(x, &openings)
                .unwrap_or_else(|| a.thorax_label_retracted(x, 1.0));
            if !skin {
                if want == Label::Outside && img.depth_of(s) <= a.designed.skin_gap_mm {
                    want = Label::SoftTissue;
                } else {
                    skin = true;
                }
            }
            assert_eq!(img.label(line, s), want, "line {line} sample {s}");
        }
    }
}

// --- probe --------------------------------------------------------------------------------------------

#[test]
fn windows_have_contact_and_see_the_heart() {
    let a = anatomy();
    let st = SliceState::new(a);
    for w in EchoWindow::ALL {
        let pose = a.window_pose(w);
        let f = pose.frame(a.designed.contact_limit_deg);
        assert!(
            f.contact,
            "{w:?} starts without contact ({:.0}°)",
            f.beam_from_normal_deg
        );
        let mut img = LabelImage::new(128, 512);
        st.slice(a, &pose, &mut img);
        let centre = [img.lines / 2 - 1, img.lines / 2];
        let hit = centre.iter().find_map(|&l| {
            (0..img.samples)
                .find(|&s| img.label(l, s).is_heart() && img.label(l, s) != Label::Pericardium)
                .map(|s| (img.depth_of(s), img.is_hidden(l, s)))
        });
        eprintln!(
            "{w:?}: skin {:.0?}, beam {:.0}° from the normal, clock {:.1} h, central line meets the heart at {:?} (depth mm, hidden)",
            pose.skin_mm,
            f.beam_from_normal_deg,
            pose.clock_hours(),
            hit
        );
        assert!(
            matches!(hit, Some((_, false))),
            "{w:?}: the central line reaches no visible heart label within the default depth"
        );
    }
}

#[test]
fn sliding_keeps_the_probe_on_the_skin() {
    let a = anatomy();
    let pose = a.window_pose(EchoWindow::Parasternal);
    let moved = a.slide_to(&pose, pose.skin_mm + Vec3::new(20.0, -15.0, 30.0));
    assert!(moved.skin_mm.distance(pose.skin_mm) > 10.0);
    assert_eq!(moved.tilt_deg, pose.tilt_deg);
    let (q, _) = a.skin().place(moved.skin_mm).expect("skin");
    assert!(q.distance(moved.skin_mm) < 1e-3);
    let f = ProbePose {
        rotation_deg: moved.rotation_deg + 360.0,
        ..moved
    }
    .frame(60.0);
    let g = moved.frame(60.0);
    assert!(f.beam.distance(g.beam) < 1e-4 && f.marker.distance(g.marker) < 1e-4);
}

// --- the image --------------------------------------------------------------------------------------

fn image(st: &SliceState, pose: &ProbePose) -> LabelImage {
    image_of(anatomy(), st, pose)
}

fn image_of(a: &EchoAnatomy, st: &SliceState, pose: &ProbePose) -> LabelImage {
    let mut img = LabelImage::new(128, 512);
    st.slice(a, pose, &mut img);
    img
}

#[test]
fn apical_rotation_finds_all_four_chambers() {
    let a = anatomy();
    let st = SliceState::new(a);
    let base = a.window_pose(EchoWindow::Apical);
    let mut found = None;
    for k in -18..=18 {
        let pose = ProbePose {
            rotation_deg: base.rotation_deg + 5.0 * k as f32,
            ..base
        };
        let img = image(&st, &pose);
        let min = [Label::LvBlood, Label::RvBlood, Label::LaBlood, Label::RaBlood]
            .map(|l| img.count(l))
            .into_iter()
            .min()
            .unwrap_or(0);
        if min > 200 && found.is_none_or(|(_, m)| min > m) {
            found = Some((5 * k, min));
        }
    }
    eprintln!(
        "apical: four chambers at rotation {found:?} (degrees from the start, smallest chamber in samples)"
    );
    assert!(
        found.is_some(),
        "no rotation within ±90° shows all four blood pools"
    );
}

#[test]
fn short_axis_at_the_papillary_level() {
    let a = anatomy();
    let st = SliceState::new(a);
    let m = model();
    let ax = m.frame().axis(MotionChamber::LeftVentricle);
    let pms = a.papillary_muscles();
    let level = pms.iter().map(|p| p.a_mm.lerp(p.b_mm, 0.5)).sum::<Vec3>() / 2.0;
    let h = ax.coords(level).height;
    let centre = ax.point_at(h);
    // from the front of the chest, in the plane perpendicular to the long axis
    let up = (Vec3::Z - ax.axis * ax.axis.dot(Vec3::Z)).normalize();
    let (skin, n) = a.skin().ray_exit(centre, up).expect("skin");
    let marker = (-n).cross(ax.axis);
    let pose = ProbePose::aimed(skin, n, centre, marker, 89.0, 200.0, 90.0);
    let mut img = LabelImage::new(256, 512);
    st.slice(a, &pose, &mut img);
    // LV blood is enclosed: its neighbours are LV wall, papillary muscle or LV blood only
    let ok = |l: Label| matches!(l, Label::LvBlood | Label::LvMyocardium | Label::PapillaryMuscle);
    let mut lv = 0usize;
    let mut leaks = 0usize;
    for line in 0..img.lines {
        for s in 0..img.samples {
            if img.label(line, s) != Label::LvBlood {
                continue;
            }
            lv += 1;
            if line == 0 || line + 1 == img.lines || s + 1 == img.samples {
                leaks += 1;
                continue;
            }
            for (l2, s2) in [(line - 1, s), (line + 1, s), (line, s - 1), (line, s + 1)] {
                if !ok(img.label(l2, s2)) {
                    leaks += 1;
                }
            }
        }
    }
    // the papillary muscles as connected regions
    let mut seen = vec![false; img.labels.len()];
    let mut bodies = Vec::new();
    for start in 0..img.labels.len() {
        if seen[start] || img.labels[start] != Label::PapillaryMuscle as u8 {
            continue;
        }
        let mut n = 0usize;
        let mut stack = vec![start];
        seen[start] = true;
        while let Some(i) = stack.pop() {
            n += 1;
            let (l, s) = (i / img.samples, i % img.samples);
            let mut nb = Vec::new();
            if l > 0 {
                nb.push(i - img.samples);
            }
            if l + 1 < img.lines {
                nb.push(i + img.samples);
            }
            if s > 0 {
                nb.push(i - 1);
            }
            if s + 1 < img.samples {
                nb.push(i + 1);
            }
            for j in nb {
                if !seen[j] && img.labels[j] == Label::PapillaryMuscle as u8 {
                    seen[j] = true;
                    stack.push(j);
                }
            }
        }
        bodies.push(n);
    }
    eprintln!("short axis: {lv} LV blood samples, {leaks} facing other tissue; papillary bodies {bodies:?}");
    assert!(lv > 1000);
    assert!(
        leaks as f64 <= 0.002 * lv as f64,
        "LV blood not enclosed by its wall: {leaks}"
    );
    assert!(
        bodies.iter().filter(|n| **n > 20).count() >= 2,
        "papillary bodies {bodies:?}"
    );
}

#[test]
fn lv_blood_area_shrinks_in_systole() {
    let a = anatomy();
    let mut m = model();
    let sim = sinus_sim(5000.0);
    let c = cycle_after(&m, &sim, 3000.0, Side::Left);
    let (_, es) = c.ejection_ms.expect("ejects");
    let pose = a.window_pose(EchoWindow::Apical);
    let mut frame = m.new_frame();
    let mut st = SliceState::new(a);
    let area = |img: &LabelImage| -> f64 {
        let mut s = 0.0;
        for line in 0..img.lines {
            for k in 0..img.samples {
                if img.label(line, k) == Label::LvBlood {
                    s += f64::from(img.depth_of(k)); // polar cell area ∝ radius
                }
            }
        }
        s
    };
    at(&mut m, &sim, c.closure_ms, &mut frame, &mut st, true);
    let ed = area(&image(&st, &pose));
    at(&mut m, &sim, es, &mut frame, &mut st, true);
    let es_a = area(&image(&st, &pose));
    eprintln!(
        "apical LV blood area: end-systole / end-diastole = {:.2}",
        es_a / ed
    );
    assert!(es_a < 0.9 * ed);
}

/// Hidden and total samples of the image that show the heart (pericardium excluded).
fn in_heart(img: &LabelImage) -> (usize, usize) {
    in_heart_central(img, img.lines)
}

/// [`in_heart`] over the `half` lines each side of the centre.
fn in_heart_central(img: &LabelImage, half: usize) -> (usize, usize) {
    let (mut hidden, mut total) = (0, 0);
    let c = img.lines / 2;
    for line in c.saturating_sub(half)..(c + half).min(img.lines) {
        for s in 0..img.samples {
            let l = img.label(line, s);
            if l.is_heart() && l != Label::Pericardium {
                total += 1;
                if img.is_hidden(line, s) {
                    hidden += 1;
                }
            }
        }
    }
    (hidden, total)
}

#[test]
fn a_rib_hides_more_than_the_space_beside_it() {
    let a = anatomy();
    let st = SliceState::new(a);
    let mid = a.midline_x_mm();
    // poses down the left parasternal line, each aimed at the heart's centre
    let target = heart_centroid();
    let mut poses = Vec::new();
    for k in 0..50 {
        let y = 60.0 - 2.0 * k as f32;
        if let Some((p, n)) = a.skin().ray_exit(Vec3::new(mid + 30.0, y, -40.0), Vec3::Z) {
            let pose = ProbePose::aimed(p, n, target, Vec3::Y, 60.0, 160.0, 75.0);
            let f = pose.frame(60.0);
            let rib = (1..40).any(|d| a.thorax_label_at(p + f.beam * d as f32) == Label::Bone);
            poses.push((y, pose, rib));
        }
    }
    let mut checked = 0;
    for (y, pose, rib) in &poses {
        if !rib {
            continue;
        }
        let Some((ys, space, _)) = poses
            .iter()
            .filter(|(y2, _, r)| !r && (y2 - y).abs() <= 30.0)
            .min_by(|p, q| (p.0 - y).abs().total_cmp(&(q.0 - y).abs()))
        else {
            continue;
        };
        // the central 16 of 128 lines (about ±5°): the wide sector crosses several ribs either way
        let (hr, tr) = in_heart_central(&image(&st, pose), 8);
        let (hs, ts) = in_heart_central(&image(&st, space), 8);
        if tr < 300 || ts < 300 {
            continue; // not over the heart
        }
        eprintln!(
            "rib at y {y:.0}: {hr} of {tr} central heart samples hidden; space at y {ys:.0}: {hs} of {ts}"
        );
        assert!(hr as f64 >= 1.5 * hs as f64, "rib {hr} vs space {hs}");
        checked += 1;
        if checked == 3 {
            break;
        }
    }
    assert!(
        checked > 0,
        "no rib with a space beside it over the heart on the parasternal line"
    );
}

#[test]
fn parasternal_window_sees_the_heart_through_an_interspace() {
    let a = anatomy();
    let st = SliceState::new(a);
    let pose = a.window_pose(EchoWindow::Parasternal);
    let img = image(&st, &pose);
    let (hidden, total) = in_heart(&img);
    let frac = hidden as f64 / total.max(1) as f64;
    let centre = img.lines / 2;
    let first_blood = (0..img.samples)
        .find(|&s| matches!(img.label(centre, s), Label::LvBlood | Label::RvBlood))
        .map(|s| (img.depth_of(s), img.is_hidden(centre, s)));
    eprintln!(
        "parasternal: {hidden} of {total} heart samples hidden ({:.1} %), central line meets ventricular blood at {first_blood:?}",
        100.0 * frac
    );
    assert!(
        matches!(first_blood, Some((_, false))),
        "the central line meets no visible LV/RV blood"
    );
    assert!(frac < 0.35, "{:.1} % of the heart hidden", 100.0 * frac);
}

fn anatomy_without_retraction() -> &'static EchoAnatomy {
    static A: OnceLock<EchoAnatomy> = OnceLock::new();
    A.get_or_init(|| {
        let d = DesignedParams {
            lung_retraction_left_mm: 0.0,
            lung_retraction_right_mm: 0.0,
            ..DesignedParams::default()
        };
        EchoAnatomy::new(pack(), &model(), d).expect("anatomy")
    })
}

#[test]
fn retraction_zero_restores_the_baked_lungs() {
    let a0 = anatomy_without_retraction();
    let t = a0.thorax_volume();
    for idx in (0..t.len()).step_by(7) {
        let [i, j, k] = t.coords(idx);
        let c = t.centre(i, j, k);
        assert_eq!(a0.thorax_label_retracted(c, 1.0), a0.thorax_label_at(c));
    }
    // the default parasternal pose, imaged without retraction, is hidden as before
    let pose = anatomy().window_pose(EchoWindow::Parasternal);
    let (h, n) = in_heart(&image_of(a0, &SliceState::new(a0), &pose));
    eprintln!("parasternal pose without retraction: {h} of {n} heart samples hidden");
    assert!(h as f64 > 0.9 * n as f64);
    // and bone is never retracted
    let a = anatomy();
    let tb = a.thorax_volume();
    for idx in 0..tb.len() {
        if tb.labels[idx] == Label::Bone as u8 {
            let [i, j, k] = tb.coords(idx);
            assert_eq!(a.thorax_label_retracted(tb.centre(i, j, k), 1.0), Label::Bone);
        }
    }
}

#[test]
fn retraction_does_not_worsen_apical_and_subcostal() {
    let (a, a0) = (anatomy(), anatomy_without_retraction());
    for w in [EchoWindow::Apical, EchoWindow::Subcostal] {
        let (h1, n1) = in_heart(&image_of(a, &SliceState::new(a), &a.window_pose(w)));
        let (h0, n0) = in_heart(&image_of(a0, &SliceState::new(a0), &a0.window_pose(w)));
        let (f1, f0) = (h1 as f64 / n1 as f64, h0 as f64 / n0 as f64);
        eprintln!(
            "{w:?}: heart hidden {:.1} % with retraction, {:.1} % without",
            100.0 * f1,
            100.0 * f0
        );
        assert!(f1 <= f0 + 1e-9 && n1 > 1000);
    }
}

#[test]
fn inspiration_narrows_the_retraction() {
    let a = anatomy();
    let mut m = model();
    let mut sim = Sim::sinus(60.0);
    sim.run_to(6000.0);
    let mut frame = m.new_frame();
    let mut st = SliceState::new(a);
    let pose = a.window_pose(EchoWindow::Parasternal);
    let mut hidden = Vec::new();
    for resp in [0.75, 0.25] {
        // expiration, then inspiration (`inspiration_depth` peaks in the first half)
        let inp = MotionInput {
            history: &sim.history,
            t_ms: 5000.0,
            resp_phase: resp,
            enabled: true,
            exaggeration: 1.0,
        };
        m.evaluate(&inp, &mut frame);
        st.update(a, &m, &frame, &inp);
        hidden.push(in_heart(&image(&st, &pose)));
    }
    eprintln!(
        "parasternal heart samples hidden: expiration {:?}, inspiration {:?}",
        hidden[0], hidden[1]
    );
    let (exp, insp) = (
        hidden[0].0 as f64 / hidden[0].1 as f64,
        hidden[1].0 as f64 / hidden[1].1 as f64,
    );
    assert!(insp > exp);
    // B32 step 0: worse in inspiration, but not closed (it was 1.0 with a loss of 0.5)
    assert!(
        insp <= 0.5,
        "{insp:.3} of the parasternal heart hidden at full inspiration"
    );
}

#[test]
fn image_is_deterministic_and_cadence_free() {
    let a = anatomy();
    let pose = a.window_pose(EchoWindow::Subcostal);
    let t = 3456.0;
    // run 1: evaluated at 60 Hz all the way
    let mut s1 = Sim::sinus(60.0);
    let mut m1 = model();
    let mut f1 = m1.new_frame();
    let mut st1 = SliceState::new(a);
    let mut tt = 2000.0;
    while tt < t {
        s1.run_to(tt + 100.0);
        at(&mut m1, &s1, tt, &mut f1, &mut st1, true);
        tt += 16.7;
    }
    s1.run_to(t + 100.0);
    at(&mut m1, &s1, t, &mut f1, &mut st1, true);
    let i1 = image(&st1, &pose);
    // run 2: fresh, one evaluation
    let mut s2 = Sim::sinus(60.0);
    s2.run_to(t + 100.0);
    let mut m2 = model();
    let mut f2 = m2.new_frame();
    let mut st2 = SliceState::new(a);
    at(&mut m2, &s2, t, &mut f2, &mut st2, true);
    let i2 = image(&st2, &pose);
    assert_eq!(i1.texels(), i2.texels());
    assert!(i1.count(Label::LvBlood) > 0);
}

#[test]
fn no_contact_gives_an_empty_image() {
    let a = anatomy();
    let st = SliceState::new(a);
    let pose = ProbePose {
        tilt_deg: 70.0,
        ..a.window_pose(EchoWindow::Apical)
    };
    let img = image(&st, &pose);
    assert!(!img.contact);
    assert!(img.labels.iter().all(|l| *l == Label::Outside as u8));
    assert_eq!(img.hidden_count(), 0);
}

/// Writes every window's image at end-diastole and end-systole (raw texels, 128 × 512 per file)
/// when `HEARTSIM_ECHO_DUMP` names a directory (diagnostics for the report, not a test).
#[test]
#[ignore = "diagnostic dump"]
fn dump_window_images() {
    let Some(dir) = std::env::var_os("HEARTSIM_ECHO_DUMP") else {
        return;
    };
    let a = anatomy();
    let mut m = model();
    let sim = sinus_sim(5000.0);
    let c = cycle_after(&m, &sim, 3000.0, Side::Left);
    let (_, es) = c.ejection_ms.expect("ejects");
    let mut frame = m.new_frame();
    let mut st = SliceState::new(a);
    for (name, t) in [("ed", c.closure_ms), ("es", es)] {
        at(&mut m, &sim, t, &mut frame, &mut st, true);
        for w in EchoWindow::ALL {
            let pose = a.window_pose(w);
            let img = image(&st, &pose);
            std::fs::write(
                std::path::Path::new(&dir).join(format!("{w:?}_{name}.raw")),
                img.texels(),
            )
            .expect("write");
        }
    }
}

/// The slice solves the inverse at knots and interpolates between them, warm-started along the
/// line; every sample whose rest point lies in the heart stays within the 0.3 mm inverse
/// tolerance, pushed forward again.
#[test]
fn interpolated_inverse_stays_within_tolerance() {
    let a = anatomy();
    let mut m = model();
    let sim = sinus_sim(5000.0);
    let c = cycle_after(&m, &sim, 3000.0, Side::Left);
    let (_, es) = c.ejection_ms.expect("ejects");
    let mut frame = m.new_frame();
    let mut st = SliceState::new(a);
    let mut worst = 0.0f32;
    let mut solved = 0usize;
    for t in [c.closure_ms, es] {
        at(&mut m, &sim, t, &mut frame, &mut st, true);
        for w in [EchoWindow::Apical, EchoWindow::Parasternal] {
            let pose = a.window_pose(w);
            let f = pose.frame(60.0);
            let mut rests = Vec::new();
            for line in (0..128).step_by(5) {
                let dir = f.line_direction(line, 128, pose.sector_deg);
                st.line_rests(dir, f.apex_mm, pose.depth_mm, 512, &mut rests);
                for (s, r) in rests.iter().enumerate() {
                    // the samples that show the heart (elsewhere the thorax decides, whatever the
                    // rest point; far from the heart the field is not contractive)
                    if let Some(r) = r.filter(|r| a.heart_label_at(*r, &[0.0; 4]).is_some()) {
                        let r = &r;
                        let x = f.apex_mm + dir * ((s as f32 + 0.5) / 512.0 * pose.depth_mm);
                        let e = (*r + st.field().at(*r)).distance(x);
                        if e > 0.3 && std::env::var_os("HEARTSIM_DIAG").is_some() {
                            let ex = st.field().rest_of(x);
                            eprintln!(
                                "{w:?} t {t:.0} line {line} s {s} knot {} err {e:.2} exact-err {:.2} label {:?} exact-label {:?} |U(x)| {:.1}",
                                s % 4 == 0 || s == 511,
                                (ex + st.field().at(ex)).distance(x),
                                a.heart_label_at(*r, &[0.0; 4]),
                                a.heart_label_at(ex, &[0.0; 4]),
                                st.field().at(x).length()
                            );
                        }
                        worst = worst.max(e);
                        solved += 1;
                    }
                }
            }
        }
    }
    eprintln!("interpolated inverse: {solved} samples, worst push-forward error {worst:.3} mm");
    assert!(worst < 0.3, "{worst} mm");
}

/// Hidden fraction of the in-heart samples of each window's start pose, in expiration and at
/// full inspiration (motion on, 5 s into a sinus run).
fn window_hidden_fractions(a: &EchoAnatomy) -> Vec<(EchoWindow, f64, f64)> {
    let mut m = model();
    let mut sim = Sim::sinus(60.0);
    sim.run_to(6000.0);
    let mut frame = m.new_frame();
    let mut out = Vec::new();
    for w in EchoWindow::ALL {
        let pose = a.window_pose(w);
        let mut fr = [0.0f64; 2];
        for (k, resp) in [0.75, 0.25].into_iter().enumerate() {
            let inp = MotionInput {
                history: &sim.history,
                t_ms: 5000.0,
                resp_phase: resp,
                enabled: true,
                exaggeration: 1.0,
            };
            m.evaluate(&inp, &mut frame);
            let mut st = SliceState::new(a);
            st.update(a, &m, &frame, &inp);
            let (h, n) = in_heart(&image_of(a, &st, &pose));
            fr[k] = h as f64 / n.max(1) as f64;
        }
        eprintln!("{w:?}: hidden {:.3} expiration, {:.3} inspiration", fr[0], fr[1]);
        out.push((w, fr[0], fr[1]));
    }
    out
}

/// B32 step 0: the apical start pose sits in an interspace whose whole sector is mostly clear
/// (the long axis's own exit lay over a rib that hid about half the image), the ribs still hide
/// what lies behind them at the sector's edge, and the parasternal window at full inspiration is
/// worse than in expiration but open.
#[test]
fn start_windows_are_open_and_ribs_still_hide() {
    let a = anatomy();
    let fr = window_hidden_fractions(a);
    let get = |w| *fr.iter().find(|f| f.0 == w).expect("window");
    let apical = get(EchoWindow::Apical);
    assert!(apical.1 <= 0.25, "apical {:.3} hidden in expiration", apical.1);
    let parasternal = get(EchoWindow::Parasternal);
    assert!(
        parasternal.2 <= 0.5 && parasternal.2 > parasternal.1,
        "{parasternal:?}"
    );
    let st = SliceState::new(a);
    let img = image(&st, &a.window_pose(EchoWindow::Apical));
    let bone_lines = (0..img.lines)
        .filter(|&l| {
            let h = usize::from(img.hidden_from[l]);
            h < img.samples && img.label(l, h - 1) == Label::Bone
        })
        .count();
    eprintln!(
        "apical start image: {bone_lines} of {} lines shadowed by a rib",
        img.lines
    );
    assert!(bone_lines > 0, "no rib shadow left in the apical image");
}

#[test]
fn window_poses_are_fixed_points_of_clamping() {
    // the page compares a re-clamped pose with the stored one to see whether the probe moved
    for w in EchoWindow::ALL {
        let p = anatomy().window_pose(w);
        assert_eq!(p.clamped(), p, "{w:?}");
        assert_eq!(p.clamped().clamped(), p.clamped(), "{w:?}");
    }
}
