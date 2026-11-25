//! Brief B34 acceptance: calipers arithmetic, reference feedback, the M-mode line against the
//! sector's own line.

mod common;

use common::{Sim, model, pack};
use heartsim_echo::measure::{LineSample, column_samples, sample_segment};
use heartsim_echo::mmode::{MModeScan, StepBuffer, SweepSpeed};
use heartsim_echo::phases::Side;
use heartsim_echo::{
    AcousticTable, DesignedParams, EchoAnatomy, EchoImager, ImageSettings, MotionInput, Named, Plane2,
    ProbePose, ScanContext, SliceState, Structure, TraceScale, ViewId, ViewTargets, cursor_toward, feedback,
    line_angle_deg, polygon_area_cm2, reference_on_plane, slope_cm_s, structure_extent,
};
use heartsim_geometry::Label;
use std::sync::OnceLock;

fn anatomy() -> &'static EchoAnatomy {
    static A: OnceLock<EchoAnatomy> = OnceLock::new();
    A.get_or_init(|| EchoAnatomy::new(pack(), &model(), DesignedParams::default()).expect("anatomy"))
}

fn targets() -> &'static ViewTargets {
    static T: OnceLock<ViewTargets> = OnceLock::new();
    T.get_or_init(|| ViewTargets::new(pack(), &model(), anatomy()).expect("view targets"))
}

#[test]
fn distance_time_and_slope_match_hand_computed_cases_at_each_speed_and_depth() {
    // (speed, depth, screen dx mm, screen dy mm, height) -> hand-computed ms, mm, cm/s
    let cases = [
        (SweepSpeed::Mm25, 150.0, 10.0, 20.0, 100.0, 400.0, 30.0, 7.5),
        (SweepSpeed::Mm50, 150.0, 10.0, 20.0, 100.0, 200.0, 30.0, 15.0),
        (SweepSpeed::Mm100, 150.0, 10.0, 20.0, 100.0, 100.0, 30.0, 30.0),
        (SweepSpeed::Mm100, 100.0, 40.0, 10.0, 50.0, 400.0, 20.0, 5.0),
        (SweepSpeed::Mm25, 200.0, 25.0, 5.0, 80.0, 1000.0, 12.5, 1.25),
    ];
    for (speed, depth, dx, dy, h, want_ms, want_mm, want_slope) in cases {
        let scale = TraceScale {
            speed,
            depth_mm: depth,
            height_mm: h,
        };
        assert!((scale.time_ms(dx) - want_ms).abs() < 1e-6, "{speed:?} time");
        assert!((scale.depth_mm(dy) - want_mm).abs() < 1e-4, "{speed:?} depth");
        let sl = scale.slope_cm_s(dx, dy).unwrap();
        assert!(
            (sl - want_slope).abs() < 1e-4,
            "{speed:?} slope {sl} vs {want_slope}"
        );
    }
    assert_eq!(slope_cm_s(0.0, 5.0), None);
    // 3-4-5 on the sector
    assert!((Plane2::new(0.0, 0.0).distance(Plane2::new(30.0, 40.0)) - 50.0).abs() < 1e-5);
    assert!((SweepSpeed::Mm50.screen_mm(1000.0) - 50.0).abs() < 1e-5);
}

#[test]
fn the_area_of_a_traced_circle_is_within_2_percent() {
    for (r, n) in [(20.0f32, 48usize), (35.0, 72), (10.0, 36)] {
        let pts: Vec<Plane2> = (0..n)
            .map(|i| {
                let a = i as f32 / n as f32 * std::f32::consts::TAU;
                Plane2::new(30.0 + r * a.cos(), 80.0 + r * a.sin())
            })
            .collect();
        let want = std::f32::consts::PI * r * r / 100.0;
        let got = polygon_area_cm2(&pts);
        assert!(
            (got - want).abs() / want < 0.02,
            "r {r}, {n} points: {got} vs {want}"
        );
    }
    assert_eq!(
        polygon_area_cm2(&[Plane2::new(0.0, 0.0), Plane2::new(1.0, 1.0)]),
        0.0
    );
}

#[test]
fn the_difference_shown_is_measured_minus_reference_and_no_reference_means_none() {
    let f = feedback(41.0, Some(45.3));
    assert_eq!(f.reference, Some(45.3));
    assert!((f.difference.unwrap() - (41.0 - 45.3)).abs() < 1e-6);
    let g = feedback(12.0, None);
    assert_eq!((g.reference, g.difference), (None, None));
}

fn line(spec: &[(Label, usize)]) -> Vec<Option<LineSample>> {
    spec.iter()
        .flat_map(|(l, n)| {
            std::iter::repeat_n(
                Some(LineSample {
                    label: *l as u8,
                    hidden: false,
                }),
                *n,
            )
        })
        .collect()
}

#[test]
fn extents_come_from_the_label_line_and_none_where_the_model_has_none() {
    use Label::*;
    // anterior wall, RV, septum 10, LV 40 (a leaflet inside), posterior wall 10, pericardium
    let l = line(&[
        (RvMyocardium, 6),
        (RvBlood, 20),
        (LvMyocardium, 10),
        (LvBlood, 15),
        (ValveLeaflet, 3),
        (LvBlood, 22),
        (LvMyocardium, 11),
        (Pericardium, 2),
        (Lung, 5),
    ]);
    let step = 0.5;
    let ext = |mid: usize, s| structure_extent(&l, step, mid, s);
    assert_eq!(
        ext(60, Structure::LvCavity),
        Some(40.0 * step),
        "the leaflet does not end the cavity"
    );
    assert_eq!(ext(31, Structure::Septum), Some(10.0 * step));
    assert_eq!(
        ext(31, Structure::PosteriorWall),
        None,
        "the septum has the cavity behind it, not in front"
    );
    assert_eq!(ext(85, Structure::PosteriorWall), Some(11.0 * step));
    assert_eq!(ext(85, Structure::Septum), None);
    assert_eq!(ext(10, Structure::LvCavity), None, "the RV is not the LV");
    assert_eq!(ext(60, Structure::AorticRoot), None);
    // aortic root: wall 3, lumen 30, wall 3: leading edge to leading edge = wall + lumen
    let ao = line(&[
        (SoftTissue, 5),
        (AortaWall, 3),
        (AortaLumen, 30),
        (AortaWall, 3),
        (LaBlood, 4),
    ]);
    assert_eq!(structure_extent(&ao, 1.0, 20, Structure::AorticRoot), Some(33.0));
    // hidden samples are not trusted
    let mut hidden = l.clone();
    for s in hidden.iter_mut().skip(40).take(30) {
        *s = s.map(|x| LineSample { hidden: true, ..x });
    }
    assert_eq!(structure_extent(&hidden, step, 60, Structure::LvCavity), None);
}

fn sinus(until_ms: f64) -> Sim {
    let mut sim = Sim::sinus(60.0);
    sim.run_to(until_ms);
    sim
}

#[test]
fn a_sector_caliper_reference_is_the_structure_along_its_own_line() {
    let sim = sinus(6000.0);
    let pose = targets().reference(anatomy(), ViewId::ParasternalLongAxis).pose;
    let s = ImageSettings::default();
    let pose = s.apply_to(&pose);
    let mut m = model();
    let bt = m.beat_times(&sim.history, 5300.0, Side::Left).unwrap();
    let t = bt.end_diastole_ms + 20.0;
    let input = MotionInput {
        history: &sim.history,
        t_ms: t,
        resp_phase: sim.history.respiratory_phase(t),
        enabled: true,
        exaggeration: 1.0,
    };
    let mut frame = m.new_frame();
    m.evaluate(&input, &mut frame);
    let mut st = SliceState::new(anatomy());
    st.update(anatomy(), &m, &frame, &input);
    let mut image = s.label_image();
    st.slice(anatomy(), &pose, &mut image);
    let pf = pose.frame(anatomy().designed.contact_limit_deg);
    // the LV: a caliper across the cavity along the beam toward the mitral tips
    let k = &targets().landmarks;
    let toward = (k.apex - k.mitral).normalize();
    let p = k.mitral + toward * 18.0;
    let (q, _) = Plane2::of_world(&pf, p);
    let (a, b) = (
        Plane2::new(q.x_mm, q.z_mm - 15.0),
        Plane2::new(q.x_mm, q.z_mm + 15.0),
    );
    let lvid = reference_on_plane(&image, Named::Lvidd, a, b);
    eprintln!("sector reference LVIDd {lvid:?}");
    assert!(lvid.is_some_and(|v| (35.0..60.0).contains(&v)));
    // the same caliper named as a wall has no reference: the middle is blood
    assert_eq!(reference_on_plane(&image, Named::Ivsd, a, b), None);
    assert_eq!(reference_on_plane(&image, Named::Tapse, a, b), None);
    // the line sampler is consistent with the image
    let seg = sample_segment(&image, a, b, 1.0);
    assert!(seg.iter().flatten().any(|s| s.label == Label::LvBlood as u8));
}

#[test]
fn the_m_mode_line_is_the_sectors_own_line() {
    let sim = sinus(6000.0);
    let s = ImageSettings::default();
    let pose = s.apply_to(&targets().reference(anatomy(), ViewId::ParasternalLongAxis).pose);
    let mut m = model();
    let bt = m.beat_times(&sim.history, 5300.0, Side::Left).unwrap();
    let step = ((bt.end_diastole_ms + 100.0) / 25.0).round() as i64;
    let t = step as f64 * 25.0;
    let line_no = 64usize;
    let cursor = line_angle_deg(line_no, s.lines, s.sector_deg);
    // the sector as the page makes it
    let input = MotionInput {
        history: &sim.history,
        t_ms: t,
        resp_phase: sim.history.respiratory_phase(t),
        enabled: true,
        exaggeration: 1.0,
    };
    let mut frame = m.new_frame();
    m.evaluate(&input, &mut frame);
    let mut st = SliceState::new(anatomy());
    st.update(anatomy(), &m, &frame, &input);
    let mut labels = s.label_image();
    st.slice(anatomy(), &pose, &mut labels);
    let mut imager = EchoImager::new(AcousticTable::builtin());
    let mut grey = heartsim_echo::GreyImage::new(s.lines, s.samples);
    imager.image(anatomy(), &st, &pose, &labels, &s, &mut grey);
    // the M-mode column of the same instant at that line's angle
    let mut scan = MModeScan::new(anatomy(), &m, AcousticTable::builtin());
    let ctx = ScanContext {
        anatomy: anatomy(),
        history: &sim.history,
        pose,
        cursor_deg: cursor,
        settings: &s,
        enabled: true,
        exaggeration: 1.0,
    };
    scan.advance(&mut m, &ctx, t, 1);
    let col = scan.buffer.get(step).unwrap();
    let n = s.samples;
    assert_eq!(&col.labels[..], &labels.labels[line_no * n..(line_no + 1) * n]);
    let want = &grey.pixels[line_no * n..(line_no + 1) * n];
    let worst = col
        .grey
        .iter()
        .zip(want)
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap();
    let same = col.grey.iter().zip(want).filter(|(a, b)| a == b).count();
    eprintln!("grey: {same}/{n} identical, worst difference {worst} levels");
    for (i, (a, b)) in col.grey.iter().zip(want).enumerate() {
        if a != b {
            eprintln!("sample {i}: column {a} sector {b}, label {}", col.labels[i]);
        }
    }
    // the thin sector's directions are the sector's up to float rounding, which can flip one
    // sample of a neighbouring line at a tissue boundary and so move a few samples of an interface
    // echo; everything else is the same bytes
    assert!(
        same >= n - 8,
        "the M-mode line is the sector's line ({same}/{n}, worst {worst})"
    );
    let _ = worst;
    assert_eq!(column_samples(col).len(), n);
}

#[test]
fn the_buffer_thins_out_then_skips_ahead_when_far_behind_and_restarts_when_the_clock_goes_back() {
    let mut b: StepBuffer<u32> = StepBuffer::new(25.0, 100);
    assert_eq!(b.plan(1000.0, 10, 16), vec![40]);
    b.push(40, 1);
    assert_eq!(b.plan(1100.0, 10, 16), vec![41, 42, 43, 44]);
    for k in [41, 42, 43, 44] {
        b.push(k, 1);
    }
    // 20 steps behind (more than 16): only the even steps, every other column
    let p = b.plan(1525.0, 100, 16);
    assert_eq!(p, vec![46, 48, 50, 52, 54, 56, 58, 60]);
    // 16 behind is still kept up with: every step
    assert_eq!(b.plan(1500.0, 100, 16).len(), 16);
    // far behind (more than twice 16): it skips ahead, leaving a gap, still thinned out
    let q = b.plan(3100.0, 100, 16);
    assert_eq!(q.first().copied(), Some(94));
    assert_eq!(q.last().copied(), Some(124));
    assert!(q.iter().all(|k| k % 2 == 0));
    b.push(q[0], 1);
    assert!(b.get(60).is_none() && b.get(q[0]).is_some());
    // a budget takes the oldest first
    assert_eq!(b.plan(3300.0, 2, 16).len(), 2);
    // the clock went back: start again
    assert_eq!(b.plan(500.0, 10, 16), vec![20]);
    assert!(b.is_empty());
}

#[test]
fn a_half_rate_plan_takes_the_even_steps_and_nearest_fills_the_odd_ones() {
    let mut b: StepBuffer<u32> = StepBuffer::new(25.0, 100);
    // an empty buffer on an odd step waits for the next even one
    assert_eq!(b.plan_every(1025.0, 10, 16, 2), Vec::<i64>::new());
    assert_eq!(b.plan_every(1050.0, 10, 16, 2), vec![42]);
    b.push(42, 7);
    assert_eq!(b.plan_every(1200.0, 10, 16, 2), vec![44, 46, 48]);
    for k in [44, 46, 48] {
        b.push(k, k as u32);
    }
    // behind, it is no thinner than every other step
    assert_eq!(b.plan_every(1750.0, 3, 16, 2), vec![50, 52, 54]);
    // every step: `plan`
    assert_eq!(b.plan_every(1250.0, 10, 16, 1), b.clone().plan(1250.0, 10, 16));
    // an odd step reads its earlier neighbour, an even one itself, and a far one nothing
    assert_eq!(b.nearest(45, 2), Some(&44));
    assert_eq!(b.nearest(46, 2), Some(&46));
    assert_eq!(b.nearest(43, 0), None);
    assert_eq!(b.nearest(60, 2), None);
}

#[test]
fn a_cursor_toward_a_point_is_the_angle_of_that_point_in_the_plane() {
    let pose = ProbePose::default();
    let f = pose.frame(60.0);
    let p = f.apex_mm + f.beam * 100.0 + f.marker * 100.0 * 30f32.to_radians().tan();
    assert!((cursor_toward(&f, p) - 30.0).abs() < 1e-3);
}
