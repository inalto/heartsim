//! Brief B34 acceptance (annular tissue Doppler, TAPSE and MAPSE on an apical M-mode).

mod common;

use common::{Sim, model, pack};
use glam::Vec3;
use heartsim_echo::phases::Side;
use heartsim_echo::{
    AnnulusSite, DesignedParams, DopplerContext, DopplerScan, EchoAnatomy, ExcursionContext, Gate,
    MotionInput, Named, Plane2, ProbeFrame, ViewId, ViewTargets, WaveKind, cursor_frame, cursor_toward,
    reference_excursion, snap_gate,
};
use heartsim_echo::{MotionChamber, MotionModel};
use std::sync::OnceLock;

fn anatomy() -> &'static EchoAnatomy {
    static A: OnceLock<EchoAnatomy> = OnceLock::new();
    A.get_or_init(|| EchoAnatomy::new(pack(), &model(), DesignedParams::default()).expect("anatomy"))
}

fn targets() -> &'static ViewTargets {
    static T: OnceLock<ViewTargets> = OnceLock::new();
    T.get_or_init(|| ViewTargets::new(pack(), &model(), anatomy()).expect("view targets"))
}

fn input(sim: &Sim, t_ms: f64) -> MotionInput<'_> {
    MotionInput {
        history: &sim.history,
        t_ms,
        resp_phase: sim.history.respiratory_phase(t_ms),
        enabled: true,
        exaggeration: 1.0,
    }
}

fn a4c() -> (heartsim_echo::ProbePose, ProbeFrame) {
    let pose = targets().reference(anatomy(), ViewId::ApicalFourChamber).pose;
    let frame = pose.frame(anatomy().designed.contact_limit_deg);
    (pose, frame)
}

fn gate_at(sim: &Sim, frame: &ProbeFrame, site: AnnulusSite, t_ms: f64) -> heartsim_echo::Gate {
    let mut m = model();
    let p = m.annulus_position(&input(sim, t_ms), site).unwrap();
    let (q, _) = Plane2::of_world(frame, p);
    let g = snap_gate(&mut m, &input(sim, t_ms), frame, q).expect("a gate on the annulus");
    assert_eq!(g.site, site);
    g
}

fn side_of(site: AnnulusSite) -> Side {
    site.side()
}

/// Run a scan over `[from, to]` the way frames would, every `dt` ms.
fn run(scan: &mut DopplerScan, m: &mut MotionModel, sim: &Sim, from: f64, to: f64, dt: f64) {
    let ctx = DopplerContext {
        history: &sim.history,
        enabled: true,
        exaggeration: 1.0,
    };
    let mut t = from;
    while t <= to {
        scan.advance(m, &ctx, t, usize::MAX);
        t += dt;
    }
    scan.advance(m, &ctx, to, usize::MAX);
}

fn sinus_sim() -> Sim {
    let mut sim = Sim::sinus(60.0);
    sim.run_to(7000.0);
    sim
}

#[test]
fn the_trace_is_the_derivative_of_the_annular_displacement_along_the_beam() {
    let sim = sinus_sim();
    let (_, frame) = a4c();
    let mut m = model();
    let g = gate_at(&sim, &frame, AnnulusSite::MitralLateral, 5000.0);
    let mut scan = DopplerScan::new();
    scan.set_gate(Some(g));
    run(&mut scan, &mut m, &sim, 4300.0, 5500.0, 50.0);
    let mut worst = 0.0f32;
    let mut n = 0;
    for k in 440..540 {
        let Some(v) = scan.velocity_cm_s(k) else { continue };
        let t = k as f64 * 10.0;
        // the displacement along the beam at 1 ms spacing, from the model itself
        let at = |m: &mut MotionModel, t: f64| m.annulus_position(&input(&sim, t), g.site).unwrap();
        let (a, b) = (at(&mut m, t - 1.0), at(&mut m, t + 1.0));
        let fd = -(b - a).dot(g.beam) / 2.0 * 100.0;
        worst = worst.max((v - fd).abs());
        n += 1;
    }
    eprintln!("worst difference from the 1 ms finite difference: {worst:.3} cm/s over {n} samples");
    assert!(n > 90);
    assert!(worst < 0.5, "{worst}");
}

#[test]
fn s_is_toward_an_apical_probe_and_e_and_a_are_opposite_in_sinus_rhythm() {
    let sim = sinus_sim();
    let (_, frame) = a4c();
    let mut m = model();
    for site in AnnulusSite::ALL {
        let g = gate_at(&sim, &frame, site, 5000.0);
        let mut scan = DopplerScan::new();
        scan.set_gate(Some(g));
        run(&mut scan, &mut m, &sim, 4300.0, 5500.0, 50.0);
        let bt = m.beat_times(&sim.history, 5300.0, side_of(site)).unwrap();
        let mk = scan.markers(&bt);
        let get = |k: WaveKind| mk.iter().find(|x| x.kind == k).map(|x| x.cm_s);
        let (s, e, a) = (get(WaveKind::S), get(WaveKind::E), get(WaveKind::A));
        eprintln!(
            "{site:?}: S′ {s:?} e′ {e:?} a′ {a:?} cm/s (unsigned beam angle at S′ {:?}°)",
            mk.iter()
                .find(|x| x.kind == WaveKind::S)
                .and_then(|x| scan.angle_deg((x.t_ms / 10.0) as i64))
        );
        assert!(s.unwrap() > 3.0, "{site:?} S′ toward the probe");
        assert!(e.unwrap() < -3.0, "{site:?} e′ away from it");
        assert!(a.unwrap() < -1.0, "{site:?} a′ away from it");
    }
}

#[test]
fn there_is_no_a_prime_in_atrial_fibrillation() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/scenarios/atrial_fibrillation.ron");
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
    let (_, frame) = a4c();
    let mut m = model();
    let g = gate_at(&sim, &frame, AnnulusSite::MitralLateral, 7000.0);
    let mut scan = DopplerScan::new();
    scan.set_gate(Some(g));
    run(&mut scan, &mut m, &sim, 5000.0, 8000.0, 50.0);
    let mut beats = 0;
    for b in sim
        .history
        .beats()
        .iter()
        .filter(|(b, _)| b.qrs_onset.0 > 5200.0 && b.qrs_onset.0 < 7600.0)
    {
        let bt = m
            .beat_times(&sim.history, b.0.qrs_onset.0 + 100.0, Side::Left)
            .unwrap();
        let mk = scan.markers(&bt);
        assert!(
            bt.atrial_onset_ms.is_none(),
            "no organised atrial contraction in AF: {bt:?}"
        );
        assert!(mk.iter().all(|x| x.kind != WaveKind::A), "no a′ in AF");
        if mk.iter().any(|x| x.kind == WaveKind::S) {
            beats += 1;
        }
    }
    assert!(beats >= 2, "S′ is still marked on the AF beats");
}

#[test]
fn two_frame_rates_record_the_same_trace() {
    let sim = sinus_sim();
    let (_, frame) = a4c();
    let g = gate_at(&sim, &frame, AnnulusSite::TricuspidLateral, 5000.0);
    let mut m = model();
    let mut a = DopplerScan::new();
    let mut b = DopplerScan::new();
    a.set_gate(Some(g));
    b.set_gate(Some(g));
    run(&mut a, &mut m, &sim, 4400.0, 5200.0, 16.7);
    run(&mut b, &mut m, &sim, 4400.0, 5200.0, 41.0);
    let (a0, a1) = a.buffer.span().unwrap();
    assert_eq!(Some((a0, a1)), b.buffer.span());
    for k in a0..=a1 {
        assert_eq!(a.buffer.get(k), b.buffer.get(k));
        assert_eq!(a.velocity_cm_s(k), b.velocity_cm_s(k));
    }
}

/// What is rotated from what: the beam is first set along the annulus's own motion at the S'
/// peak of the lateral mitral annulus (the unit vector of the position difference across that
/// step, pointing away from the probe against the motion), which reads the full velocity; the
/// second beam is that one turned 60 degrees about an axis perpendicular to it. The same recorded
/// positions are read on both beams at the same step, so the ratio is exactly cos 60 = 0.5.
#[test]
fn at_60_degrees_to_the_motion_the_peak_is_half() {
    let sim = sinus_sim();
    let (_, frame) = a4c();
    let mut m = model();
    let site = AnnulusSite::MitralLateral;
    let bt = m.beat_times(&sim.history, 5300.0, Side::Left).unwrap();
    let g = gate_at(&sim, &frame, site, 5000.0);
    let mut scan = DopplerScan::new();
    scan.set_gate(Some(g));
    run(&mut scan, &mut m, &sim, 4300.0, 4900.0, 50.0);
    let s_peak = scan
        .markers(&bt)
        .into_iter()
        .find(|x| x.kind == WaveKind::S)
        .unwrap();
    let k = (s_peak.t_ms / 10.0).round() as i64;
    let w = scan.buffer.get(k + 1).unwrap().position_mm - scan.buffer.get(k - 1).unwrap().position_mm;
    let along = -w.normalize();
    let f = along.any_orthonormal_vector();
    let turned = along * 60f32.to_radians().cos() + f * 60f32.to_radians().sin();
    let velocity = |beam: Vec3, m: &mut MotionModel| {
        let mut sc = DopplerScan::new();
        sc.set_gate(Some(Gate { beam, ..g }));
        run(&mut sc, m, &sim, 4300.0, 4900.0, 50.0);
        sc.velocity_cm_s(k).unwrap()
    };
    let (full, half) = (velocity(along, &mut m), velocity(turned, &mut m));
    eprintln!(
        "S' step {k}: along the motion {full:.2} cm/s, 60 degrees from it {half:.2}: ratio {:.4}",
        half / full
    );
    assert!(full > 3.0);
    assert!((half / full - 0.5).abs() < 0.02, "ratio {}", half / full);
}

#[test]
fn a_gate_snaps_to_an_annulus_and_nowhere_else() {
    let sim = sinus_sim();
    let (_, frame) = a4c();
    let mut m = model();
    let inp = input(&sim, 5000.0);
    for site in AnnulusSite::ALL {
        let p = m.annulus_position(&inp, site).unwrap();
        let (q, _) = Plane2::of_world(&frame, p);
        let near = Plane2::new(q.x_mm + 4.0, q.z_mm - 3.0);
        assert_eq!(snap_gate(&mut m, &inp, &frame, near).map(|g| g.site), Some(site));
        let far = Plane2::new(q.x_mm + 30.0, q.z_mm + 30.0);
        assert!(snap_gate(&mut m, &inp, &frame, far).is_none(), "{site:?}");
    }
    // the middle of the LV cavity is not an annulus
    let k = &targets().landmarks;
    let (c, _) = Plane2::of_world(&frame, k.lv_blood);
    assert!(snap_gate(&mut m, &inp, &frame, c).is_none());
}

#[test]
fn tapse_and_mapse_on_an_apical_m_mode_are_the_read_out_times_the_beam_cosine() {
    let sim = sinus_sim();
    let (_, frame) = a4c();
    let mut m = model();
    for (what, site, chamber) in [
        (
            Named::Tapse,
            AnnulusSite::TricuspidLateral,
            MotionChamber::RightVentricle,
        ),
        (
            Named::Mapse,
            AnnulusSite::MitralLateral,
            MotionChamber::LeftVentricle,
        ),
    ] {
        let bt = m.beat_times(&sim.history, 5300.0, site.side()).unwrap();
        let es = bt.ejection_ms.unwrap().1;
        let p_ed = m
            .annulus_position(&input(&sim, bt.end_diastole_ms), site)
            .unwrap();
        let cursor = cursor_toward(&frame, p_ed);
        let ctx = ExcursionContext {
            history: &sim.history,
            frame,
            cursor_deg: cursor,
            enabled: true,
            exaggeration: 1.0,
        };
        let ex = reference_excursion(&mut m, &ctx, what, bt.end_diastole_ms, es)
            .expect("the cursor passes the annulus");
        assert_eq!(ex.site, site);
        let r = m.readout(&input(&sim, es + 60.0));
        let value = if what == Named::Tapse {
            r.tapse_mm
        } else {
            r.mapse_mm
        }
        .expect("a read-out");
        let axis = m.frame().axis(chamber).axis;
        let cos = f64::from(axis.dot(cursor_frame(&frame, cursor).beam).abs());
        eprintln!(
            "{what:?}: along the beam {:.2} mm, read-out {value:.2} mm x cos {cos:.3} = {:.2} mm; motion angle {:.1} deg",
            ex.along_beam_mm,
            value * cos,
            ex.angle_deg
        );
        assert!(
            (f64::from(ex.along_beam_mm) - value * cos).abs() < 1.0,
            "{what:?}: {} vs {}",
            ex.along_beam_mm,
            value * cos
        );
    }
}

#[test]
fn no_reference_is_offered_where_the_model_has_none() {
    let sim = sinus_sim();
    let (_, frame) = a4c();
    let mut m = model();
    let bt = m.beat_times(&sim.history, 5300.0, Side::Right).unwrap();
    let es = bt.ejection_ms.unwrap().1;
    // a cursor down the middle of the sector does not pass the lateral tricuspid annulus
    let ctx = ExcursionContext {
        history: &sim.history,
        frame,
        cursor_deg: 0.0,
        enabled: true,
        exaggeration: 1.0,
    };
    assert!(reference_excursion(&mut m, &ctx, Named::Tapse, bt.end_diastole_ms, es).is_none());
    // and a name that is no excursion has none whatever the cursor
    let p = m
        .annulus_position(&input(&sim, bt.end_diastole_ms), AnnulusSite::TricuspidLateral)
        .unwrap();
    let on = ExcursionContext {
        cursor_deg: cursor_toward(&frame, p),
        ..ctx
    };
    assert!(reference_excursion(&mut m, &on, Named::Lvidd, bt.end_diastole_ms, es).is_none());
    assert!(reference_excursion(&mut m, &on, Named::Tapse, bt.end_diastole_ms, es).is_some());
    // a velocity with no wave marked: no reference
    assert_eq!(heartsim_echo::reference_velocity(Named::APrime, &[]), None);
    assert_eq!(heartsim_echo::reference_velocity(Named::Lvidd, &[]), None);
}
