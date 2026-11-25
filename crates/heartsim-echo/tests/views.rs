//! Brief B33 acceptance (parts 1 to 4): the view targets, the score, the hints and the checklist, on
//! the default pack.

mod common;

use common::{Sim, model, pack};
use glam::Vec3;
use heartsim_echo::view_hints::PatientDir;
use heartsim_echo::{
    DesignedParams, Direction, EchoAnatomy, FoundTracker, HintKind, ImageStats, LabelImage, MotionInput,
    Movement, ProbePose, SliceState, ViewId, ViewTargets,
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

fn shipped() -> Vec<ViewId> {
    ViewId::ALL
        .into_iter()
        .filter(|id| targets().reference(anatomy(), *id).reachable)
        .collect()
}

fn limit() -> f32 {
    anatomy().designed.contact_limit_deg
}

#[test]
fn report_of_every_view() {
    let t = targets();
    for id in ViewId::ALL {
        let r = t.reference(anatomy(), id);
        eprintln!(
            "{:<15} reachable {:<5} score {:.3} hidden {:.2} x {:.0} depth {} evals {} | plane {:.2} sector {:.2} absent {:.2} marker {:.2} window {:.2} fore {:.2}",
            id.id(),
            r.reachable,
            r.score.total,
            r.hidden_fraction,
            r.pose.skin_mm.x,
            r.pose.depth_mm,
            r.evaluations,
            r.score.plane,
            r.score.sector,
            r.score.absent,
            r.score.marker,
            r.score.window,
            r.score.foreshortening
        );
    }
    let n = shipped().len();
    assert!(n >= 12, "only {n} views are reachable");
    // the views the supine anatomy cannot show are reported, with their best scores
    for id in [ViewId::SubcostalIvc, ViewId::SuprasternalArch] {
        assert!(!t.reference(anatomy(), id).reachable);
    }
}

#[test]
fn reference_poses_have_contact_limits_and_score_above_0_9() {
    let t = targets();
    for id in shipped() {
        let r = t.reference(anatomy(), id);
        let f = r.pose.frame(limit());
        assert!(f.contact, "{id:?}");
        assert_eq!(r.pose, r.pose.clamped(), "{id:?} within the probe limits");
        let start = anatomy().window_pose(id.window());
        assert!(
            r.pose.skin_mm.distance(start.skin_mm) < 70.0,
            "{id:?}: {} mm from its window",
            r.pose.skin_mm.distance(start.skin_mm)
        );
        assert!(r.score.total > 0.9, "{id:?}: {}", r.score.total);
        // rotated about the beam: the same skin point and beam, the marker turned
        let f = r.pose.frame(limit());
        for d in [-20.0f32, 20.0] {
            let marker = glam::Quat::from_axis_angle(f.beam, d.to_radians()) * f.marker;
            let p = ProbePose::aimed(
                r.pose.skin_mm,
                r.pose.normal,
                r.pose.skin_mm + f.beam * 100.0,
                marker,
                limit(),
                r.pose.depth_mm,
                r.pose.sector_deg,
            );
            assert!(p.frame(limit()).beam.dot(f.beam) > 0.999);
            let s = t.score(anatomy(), id, &p).total;
            assert!(s < 0.5, "{id:?} rotated {d} about the beam: {s}");
        }
        for d in [-15.0, 15.0] {
            let p = ProbePose {
                tilt_deg: r.pose.tilt_deg + d,
                ..r.pose
            };
            let s = t.score(anatomy(), id, &p).total;
            assert!(s < 0.5, "{id:?} tilted {d}: {s}");
        }
    }
}

#[test]
fn the_score_is_a_pure_function_of_the_pose() {
    let t = targets();
    for id in shipped() {
        let p = t.reference(anatomy(), id).pose;
        let a = t.score(anatomy(), id, &p);
        let b = t.score(anatomy(), id, &p);
        assert_eq!(a, b);
    }
    // no frame rate and no beat time in the signature; two calls around a slice of another time agree
    let st = SliceState::new(anatomy());
    let mut img = LabelImage::new(32, 64);
    let p = t.reference(anatomy(), ViewId::ApicalFourChamber).pose;
    let before = t.score(anatomy(), ViewId::ApicalFourChamber, &p);
    st.slice(anatomy(), &p, &mut img);
    assert_eq!(before, t.score(anatomy(), ViewId::ApicalFourChamber, &p));
}

#[test]
fn plax_with_the_apex_in_the_sector_scores_lower() {
    let t = targets();
    let r = t.reference(anatomy(), ViewId::ParasternalLongAxis);
    assert!(
        r.score.present_absent.is_empty() && r.score.absent > 0.9,
        "reference: apex absent {}",
        r.score.absent
    );
    let apex = t.landmarks.apex;
    let p = ProbePose::aimed(
        r.pose.skin_mm,
        r.pose.normal,
        apex,
        r.pose.frame(limit()).marker,
        limit(),
        r.pose.depth_mm.max(200.0),
        r.pose.sector_deg,
    );
    let s = t.score(anatomy(), ViewId::ParasternalLongAxis, &p);
    assert!(
        s.absent < r.score.absent,
        "absent {} vs {}",
        s.absent,
        r.score.absent
    );
    assert!(s.total < r.score.total, "{} vs {}", s.total, r.score.total);
}

#[test]
fn two_chamber_is_poor_when_an_rv_landmark_is_in_the_plane() {
    let t = targets();
    // the four-chamber plane contains the tricuspid centre: scored as a two-chamber it must be poor
    let a4c = t.reference(anatomy(), ViewId::ApicalFourChamber).pose;
    let s = t.score(anatomy(), ViewId::ApicalTwoChamber, &a4c);
    assert!(s.total < 0.5, "{}", s.total);
    assert!(
        s.absent < 0.5,
        "an RV landmark is in the plane: absent term {}",
        s.absent
    );
    assert_ne!(s.present_absent, Vec::new());
    let r = t.reference(anatomy(), ViewId::ApicalTwoChamber);
    assert!(r.score.absent > 0.9, "{}", r.score.absent);
}

#[test]
fn a4c_missing_the_true_apex_is_foreshortened() {
    let t = targets();
    let id = ViewId::ApicalFourChamber;
    let r = t.reference(anatomy(), id);
    let f = r.pose.frame(limit());
    let target = t.target(id);
    // slide the transducer across the plane, keeping the beam on the valves and the marker
    let mut best: Option<(f32, ProbePose)> = None;
    for sgn in [-1.0f32, 1.0] {
        for k in 2..14 {
            let want = r.pose.skin_mm + f.elevation * (sgn * 2.5 * k as f32);
            let moved = anatomy().slide_to(&r.pose, want);
            let p = ProbePose::aimed(
                moved.skin_mm,
                moved.normal,
                target.aim_mm,
                f.marker,
                limit(),
                r.pose.depth_mm,
                r.pose.sector_deg,
            );
            let s = t.score(anatomy(), id, &p);
            let miss = (s.apex_off_plane_mm - 15.0).abs();
            let pf = p.frame(limit());
            let (bone, lung) =
                heartsim_echo::views::beam_occlusion(anatomy(), pf.apex_mm, pf.beam, p.depth_mm);
            if s.contact && bone < 2.0 && lung < 3.0 && best.as_ref().is_none_or(|b| miss < b.0) {
                best = Some((miss, p));
            }
        }
    }
    let (_, p) = best.expect("a pose");
    let s = t.score(anatomy(), id, &p);
    eprintln!(
        "apex off the plane {:.1} mm: total {:.2} (reference {:.2}), fore {:.2} plane {:.2} marker {:.2}",
        s.apex_off_plane_mm, s.total, r.score.total, s.foreshortening, s.plane, s.marker
    );
    assert!(
        (s.apex_off_plane_mm - 15.0).abs() < 4.0,
        "{}",
        s.apex_off_plane_mm
    );
    assert!(s.foreshortening < r.score.foreshortening - 0.3);
    assert!(s.total < r.score.total);
    let h = t.hint(anatomy(), id, &p);
    assert_eq!(h.kind, HintKind::Foreshortened, "{h:?}");
}

// ---- the label slice -------------------------------------------------------------------------------

fn slice_at_rest(id: ViewId) -> (LabelImage, ProbePose) {
    let p = targets().reference(anatomy(), id).pose;
    let st = SliceState::new(anatomy());
    let mut img = LabelImage::new(128, 512);
    st.slice(anatomy(), &p, &mut img);
    (img, p)
}

#[test]
fn every_checklist_is_ticked_at_its_reference_pose() {
    let t = targets();
    for id in shipped() {
        let (img, p) = slice_at_rest(id);
        let stats = ImageStats::of(&img, anatomy());
        let ticks = t.checklist(id, &stats, &p, anatomy());
        for (item, ok) in t.target(id).checklist.iter().zip(&ticks) {
            assert!(*ok, "{id:?}: `{}` not ticked at rest", item.key);
        }
    }
}

#[test]
fn a4c_slice_holds_all_four_blood_labels_and_both_av_valves() {
    let (img, _) = slice_at_rest(ViewId::ApicalFourChamber);
    for l in [
        Label::LvBlood,
        Label::RvBlood,
        Label::LaBlood,
        Label::RaBlood,
        Label::ValveLeaflet,
    ] {
        assert!(img.count(l) > 50, "{l:?}: {}", img.count(l));
    }
    let stats = ImageStats::of(&img, anatomy());
    let t = targets();
    let ticks = t.checklist(
        ViewId::ApicalFourChamber,
        &stats,
        &t.reference(anatomy(), ViewId::ApicalFourChamber).pose,
        anatomy(),
    );
    assert!(ticks.iter().all(|x| *x));
}

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
fn checklists_stay_ticked_through_the_beat() {
    let mut m = model();
    let mut sim = Sim::sinus(60.0);
    sim.run_to(3200.0);
    let mut frame = m.new_frame();
    let mut st = SliceState::new(anatomy());
    let mut img = LabelImage::new(128, 256);
    let t = targets();
    let mut worst: Vec<(ViewId, &'static str)> = Vec::new();
    for t_ms in [2100.0, 2250.0, 2400.0, 2550.0, 2700.0, 2850.0] {
        let inp = input(&sim, t_ms);
        m.evaluate(&inp, &mut frame);
        st.update(anatomy(), &m, &frame, &inp);
        for id in shipped() {
            let p = t.reference(anatomy(), id).pose;
            st.slice(anatomy(), &p, &mut img);
            let stats = ImageStats::of(&img, anatomy());
            let ticks = t.checklist(id, &stats, &p, anatomy());
            for (item, ok) in t.target(id).checklist.iter().zip(&ticks) {
                if !*ok {
                    worst.push((id, item.key));
                }
            }
        }
    }
    assert!(worst.is_empty(), "not ticked in the beating heart: {worst:?}");
}

// ---- hints -----------------------------------------------------------------------------------------

#[test]
fn the_reference_pose_is_found_and_hints_name_the_perturbed_movement() {
    let t = targets();
    for id in shipped() {
        let r = t.reference(anatomy(), id);
        assert_eq!(t.hint(anatomy(), id, &r.pose).kind, HintKind::Found, "{id:?}");
        // rotation: the correcting direction is the opposite turn
        let cw = ProbePose {
            rotation_deg: r.pose.rotation_deg + 30.0,
            ..r.pose
        };
        let h = t.hint(anatomy(), id, &cw);
        // (a rotation about the normal also swings a tilted beam: it can land on a rib)
        if matches!(h.kind, HintKind::Move | HintKind::WrongMarkerSide) {
            assert_eq!(h.movement, Movement::Rotate, "{id:?} {h:?}");
            assert_eq!(h.direction, Direction::CounterClockwise, "{id:?}");
            assert!((h.amount - 30.0).abs() < 0.5);
        }
        let ccw = ProbePose {
            rotation_deg: r.pose.rotation_deg - 30.0,
            ..r.pose
        };
        let h = t.hint(anatomy(), id, &ccw);
        if matches!(h.kind, HintKind::Move | HintKind::WrongMarkerSide) {
            assert_eq!(h.direction, Direction::Clockwise, "{id:?} {h:?}");
        }
        // tilt and rock name themselves and point where the beam has to go
        for (movement, p) in [
            (
                Movement::Tilt,
                ProbePose {
                    tilt_deg: r.pose.tilt_deg + 10.0,
                    ..r.pose
                },
            ),
            (
                Movement::Rock,
                ProbePose {
                    rock_deg: r.pose.rock_deg - 10.0,
                    ..r.pose
                },
            ),
        ] {
            if !p.frame(limit()).contact {
                continue;
            }
            let h = t.hint(anatomy(), id, &p);
            if matches!(
                h.kind,
                HintKind::OverRib | HintKind::OverLung | HintKind::LostContact | HintKind::Found
            ) {
                continue;
            }
            assert_eq!(h.movement, movement, "{id:?} {h:?}");
            let tip = |q: &ProbePose| {
                let f = q.frame(limit());
                f.apex_mm + f.beam * 100.0
            };
            let want = PatientDir::of(tip(&r.pose) - tip(&p));
            assert_eq!(h.direction, Direction::Patient(want), "{id:?} {movement:?}");
        }
        // a slide: name the direction back to the window
        let (cranial, _) = (Vec3::Y, Vec3::X);
        let want = r.pose.skin_mm + cranial * 18.0;
        let moved = anatomy().slide_to(&r.pose, want);
        let h = t.hint(anatomy(), id, &moved);
        if h.kind == HintKind::Move {
            assert_eq!(h.movement, Movement::Slide, "{id:?} {h:?}");
            let back = r.pose.skin_mm - moved.skin_mm;
            assert_eq!(h.direction, Direction::Patient(PatientDir::of(back)));
        }
    }
}

#[test]
fn over_a_rib_the_hint_is_the_rib_hint() {
    let t = targets();
    let id = ViewId::ApicalFourChamber;
    let r = t.reference(anatomy(), id);
    let f0 = r.pose.frame(limit());
    let mut hit = None;
    'scan: for k in 1..40 {
        for s in [1.0f32, -1.0] {
            let want = r.pose.skin_mm + Vec3::Y * (s * 2.0 * k as f32);
            let moved = anatomy().slide_to(&r.pose, want);
            let f = moved.frame(limit());
            let (bone, _) =
                heartsim_echo::views::beam_occlusion(anatomy(), f.apex_mm, f.beam, moved.depth_mm);
            if bone >= t.tolerances.rib_bone_mm && f.contact {
                hit = Some(moved);
                break 'scan;
            }
        }
    }
    let _ = f0;
    let p = hit.expect("a rib within 80 mm of the apical reference");
    let h = t.hint(anatomy(), id, &p);
    assert_eq!(h.kind, HintKind::OverRib, "{h:?}");
    assert_eq!(h.movement, Movement::Slide);
    assert!(matches!(
        h.direction,
        Direction::Patient(PatientDir::Head | PatientDir::Foot)
    ));
}

#[test]
fn lost_contact_has_its_own_hint() {
    let t = targets();
    let id = ViewId::ApicalFourChamber;
    let r = t.reference(anatomy(), id);
    let p = ProbePose {
        tilt_deg: 80.0,
        rock_deg: 40.0,
        ..r.pose
    };
    assert!(!p.frame(limit()).contact);
    assert_eq!(t.hint(anatomy(), id, &p).kind, HintKind::LostContact);
    assert_eq!(t.score(anatomy(), id, &p).total, 0.0);
}

#[test]
fn the_marker_on_the_wrong_side_has_its_own_hint() {
    let t = targets();
    let id = ViewId::ApicalFourChamber;
    let r = t.reference(anatomy(), id);
    // the same skin point and beam, the marker the other way
    let f = r.pose.frame(limit());
    let p = ProbePose::aimed(
        r.pose.skin_mm,
        r.pose.normal,
        r.pose.skin_mm + f.beam * 100.0,
        -f.marker,
        limit(),
        r.pose.depth_mm,
        r.pose.sector_deg,
    );
    assert!(p.frame(limit()).beam.dot(f.beam) > 0.999);
    let h = t.hint(anatomy(), id, &p);
    assert_eq!(h.kind, HintKind::WrongMarkerSide, "{h:?}");
    assert_eq!(h.movement, Movement::Rotate);
}

fn step_toward(p: &ProbePose, r: &ProbePose, h: heartsim_echo::Hint) -> ProbePose {
    let a = h.amount;
    match h.movement {
        Movement::Slide => {
            let d = r.skin_mm - p.skin_mm;
            let step_mm = a.clamp(0.5, 4.0);
            anatomy().slide_to(p, p.skin_mm + d.normalize_or_zero() * step_mm)
        }
        Movement::Rotate => {
            let d = (r.rotation_deg - p.rotation_deg + 180.0).rem_euclid(360.0) - 180.0;
            ProbePose {
                rotation_deg: p.rotation_deg + d.clamp(-5.0, 5.0),
                ..*p
            }
        }
        Movement::Tilt => ProbePose {
            tilt_deg: p.tilt_deg + (r.tilt_deg - p.tilt_deg).clamp(-3.0, 3.0),
            ..*p
        },
        Movement::Rock => ProbePose {
            rock_deg: p.rock_deg + (r.rock_deg - p.rock_deg).clamp(-3.0, 3.0),
            ..*p
        },
        Movement::Angle => ProbePose {
            tilt_deg: p.tilt_deg + (r.tilt_deg - p.tilt_deg).clamp(-3.0, 3.0),
            rock_deg: p.rock_deg + (r.rock_deg - p.rock_deg).clamp(-3.0, 3.0),
            ..*p
        },
        Movement::Depth => ProbePose {
            depth_mm: p.depth_mm + (r.depth_mm - p.depth_mm).clamp(-20.0, 20.0),
            ..*p
        },
    }
    .clamped()
}

fn follow(id: ViewId, from: ProbePose, max_steps: usize) -> Option<usize> {
    let t = targets();
    let r = t.reference(anatomy(), id).pose;
    let mut p = from;
    for step in 0..max_steps {
        let h = t.hint(anatomy(), id, &p);
        if h.kind == HintKind::Found {
            return Some(step);
        }
        p = step_toward(&p, &r, h);
    }
    None
}

#[test]
fn following_the_hints_from_the_window_start_finds_every_view() {
    for id in shipped() {
        let start = anatomy().window_pose(id.window());
        let steps = follow(id, start, 250);
        eprintln!("{id:?}: found after {steps:?} steps");
        assert!(steps.is_some(), "{id:?} not found within 250 hint steps");
    }
}

#[test]
fn found_needs_the_threshold_held_for_one_second() {
    let tol = &targets().tolerances;
    let mut f = FoundTracker::default();
    assert!(!f.update(tol, 0.95, 0.5));
    assert!(!f.update(tol, 0.95, 0.4));
    assert!(f.update(tol, 0.95, 0.2));
    assert!(f.update(tol, 0.9, 0.01), "stays found while above");
    assert!(!f.update(tol, 0.2, 0.01), "a drop below ends it");
    assert!(!f.update(tol, 0.95, 0.9), "and the hold starts over");
    f.reset();
    assert_eq!(f.held_s(), 0.0);
    // a frame rate does not change the verdict: 1 s in 60 steps or in 4
    let (mut a, mut b) = (FoundTracker::default(), FoundTracker::default());
    let ra = (0..60).map(|_| a.update(tol, 0.9, 1.0 / 60.0)).last().unwrap();
    let rb = (0..4).map(|_| b.update(tol, 0.9, 0.25)).last().unwrap();
    assert_eq!(ra, rb);
}

// --- images for review ------------------------------------------------------------------------------

fn sector_png(g: &heartsim_echo::GreyImage, w: u32, h: u32) -> image::GrayImage {
    let half = g.sector_deg.to_radians() * 0.5;
    let scale = g.depth_mm / (h as f32 - 2.0);
    image::GrayImage::from_fn(w, h, |px, py| {
        let x = (px as f32 - w as f32 * 0.5) * scale;
        let y = py as f32 * scale;
        let r = (x * x + y * y).sqrt();
        let th = x.atan2(y);
        if r >= g.depth_mm || th.abs() >= half {
            return image::Luma([0]);
        }
        let lf = ((th / (2.0 * half)) + 0.5) * g.lines as f32 - 0.5;
        let sf = r / g.depth_mm * g.samples as f32 - 0.5;
        let (l0, s0) = (lf.floor().max(0.0) as usize, sf.floor().max(0.0) as usize);
        let (l1, s1) = ((l0 + 1).min(g.lines - 1), (s0 + 1).min(g.samples - 1));
        let (tl, ts) = ((lf - l0 as f32).clamp(0.0, 1.0), (sf - s0 as f32).clamp(0.0, 1.0));
        let p = |l: usize, s: usize| f32::from(g.pixel(l, s));
        let v = (p(l0, s0) * (1.0 - ts) + p(l0, s1) * ts) * (1.0 - tl)
            + (p(l1, s0) * (1.0 - ts) + p(l1, s1) * ts) * tl;
        image::Luma([v.round() as u8])
    })
}

/// Writes the grey sector of every shipped view at its reference pose (heart at rest) as a PNG into
/// the directory `HEARTSIM_ECHO_PNG` names (for review and for the user docs; not a test).
#[test]
#[ignore = "writes review images"]
fn write_view_pngs() {
    let Some(dir) = std::env::var_os("HEARTSIM_ECHO_PNG") else {
        return;
    };
    let t = targets();
    let st = SliceState::new(anatomy());
    let mut img = LabelImage::new(128, 512);
    let mut imager = heartsim_echo::EchoImager::new(heartsim_echo::AcousticTable::builtin());
    for id in shipped() {
        let p = t.reference(anatomy(), id).pose;
        st.slice(anatomy(), &p, &mut img);
        let mut g = heartsim_echo::GreyImage::new(128, 512);
        let s = heartsim_echo::ImageSettings {
            lines: 128,
            samples: 512,
            depth_mm: p.depth_mm,
            sector_deg: p.sector_deg,
            ..heartsim_echo::ImageSettings::default()
        };
        imager.image(anatomy(), &st, &p, &img, &s, &mut g);
        sector_png(&g, 640, 520)
            .save(std::path::Path::new(&dir).join(format!("{}.png", id.id())))
            .expect("png");
    }
}

#[test]
fn costs_of_the_search_and_of_a_frame() {
    use std::time::Instant;
    let t = targets();
    // every reference was searched by the first use above or now (deterministic: same poses twice)
    let t0 = Instant::now();
    let fresh = ViewTargets::new(pack(), &model(), anatomy()).expect("targets");
    let setup = t0.elapsed();
    let t0 = Instant::now();
    let mut evals = 0u64;
    for id in ViewId::ALL {
        let r = fresh.reference(anatomy(), id);
        evals += u64::from(r.evaluations);
        assert_eq!(
            r.pose,
            t.reference(anatomy(), id).pose,
            "{id:?}: the search is deterministic"
        );
    }
    let all = t0.elapsed();
    eprintln!(
        "targets {setup:?}; all {} references {all:?} ({evals} score evaluations, {:?} each on average)",
        ViewId::ALL.len(),
        all / evals.max(1) as u32
    );
    // the per-frame cost: the score and the hint of one pose, the checklist of one slice
    let id = ViewId::ApicalFourChamber;
    let mut p = t.reference(anatomy(), id).pose;
    let n = 200;
    let t0 = Instant::now();
    for k in 0..n {
        p.rotation_deg += 0.01 * f32::from(k as u8 % 3);
        let _ = t.score(anatomy(), id, &p);
    }
    let score = t0.elapsed() / n;
    let t0 = Instant::now();
    for k in 0..n {
        p.tilt_deg += 0.01 * f32::from(k as u8 % 3);
        let _ = t.hint(anatomy(), id, &p);
    }
    let hint = t0.elapsed() / n;
    let (img, pose) = slice_at_rest(id);
    let t0 = Instant::now();
    for _ in 0..n {
        let stats = ImageStats::of(&img, anatomy());
        let _ = t.checklist(id, &stats, &pose, anatomy());
    }
    let list = t0.elapsed() / n;
    eprintln!("per frame: score {score:?}, score+hint {hint:?}, checklist {list:?}");
    assert!(score.as_micros() < 2000 && hint.as_micros() < 5000 && list.as_micros() < 5000);
}

#[test]
#[ignore = "exploration"]
fn zz_breakdown() {
    let t = targets();
    let axis = t.landmarks.lv_axis;
    let show = |label: &str, id: ViewId, p: &ProbePose| {
        let s = t.score(anatomy(), id, p);
        let f = p.frame(limit());
        let nang = f.elevation.dot(axis).abs().clamp(0.0, 1.0).acos().to_degrees();
        eprintln!(
            "{label:<28} {:<14} total {:.3} contact {} plane {:.2} sector {:.2} marker {:.2} ({:.0} deg) circularity {:.2} (ratio {:.2}) normal {:.2} (plane normal {:.0} deg off the LV axis) window {:.2} skin x {:.0} y {:.0} hidden {:.2}",
            id.id(),
            s.total,
            s.contact,
            s.plane,
            s.sector,
            s.marker,
            s.marker_error_deg,
            s.circularity,
            s.circularity_ratio,
            s.normal,
            nang,
            s.window,
            p.skin_mm.x,
            p.skin_mm.y,
            anatomy().hidden_share_at_rest(p)
        );
    };
    let plax = t.reference(anatomy(), ViewId::ParasternalLongAxis).pose;
    let f = plax.frame(limit());
    let rot = ProbePose {
        rotation_deg: plax.rotation_deg + 90.0,
        ..plax
    };
    for id in [
        ViewId::ShortAxisAorticValve,
        ViewId::ShortAxisMitralValve,
        ViewId::ShortAxisPapillary,
        ViewId::ShortAxisApex,
    ] {
        show("PLAX ref rotated +90 deg", id, &rot);
    }
    let _ = f;
    for id in [
        ViewId::ShortAxisAorticValve,
        ViewId::ShortAxisMitralValve,
        ViewId::ShortAxisPapillary,
        ViewId::ShortAxisApex,
    ] {
        let r = t.reference(anatomy(), id);
        show("best pose", id, &r.pose);
    }
}

#[test]
fn parasternal_references_lie_left_of_the_sternum_within_the_slide_limit() {
    let t = targets();
    let tol = &t.tolerances;
    let mid = anatomy().midline_x_mm();
    for id in ViewId::ALL
        .into_iter()
        .filter(|v| v.window() == heartsim_echo::EchoWindow::Parasternal)
    {
        let r = t.reference(anatomy(), id);
        let start = anatomy().window_pose(id.window()).skin_mm;
        assert!(
            r.pose.skin_mm.x >= mid + tol.parasternal_left_margin_mm - 0.5,
            "{id:?}: x {} is not left of the sternal midline {mid} by {}",
            r.pose.skin_mm.x,
            tol.parasternal_left_margin_mm
        );
        assert!(r.pose.skin_mm.distance(start) <= tol.max_slide_mm + 0.5, "{id:?}");
    }
    for id in ViewId::ALL {
        let r = t.reference(anatomy(), id);
        let start = anatomy().window_pose(id.window()).skin_mm;
        assert!(
            r.pose.skin_mm.distance(start) <= tol.max_slide_mm + 0.5,
            "{id:?}: slide limit"
        );
    }
}

/// The bound on the hidden share of a shipped view's reference (bone or lung in front of the
/// heart, at rest in expiration). The best trade-off the search found: no pose it found scores 0.9
/// with less in the worst views (parasternal long axis, right ventricular outflow).
const HIDDEN_BOUND: f32 = 0.6;

#[test]
fn every_shipped_reference_has_a_bounded_hidden_share() {
    let t = targets();
    for id in shipped() {
        let h = t.reference(anatomy(), id).hidden_fraction;
        assert!(h <= HIDDEN_BOUND, "{id:?}: {h}");
    }
}

#[test]
fn the_four_short_axis_levels_are_reachable_and_round() {
    let t = targets();
    for id in [
        ViewId::ShortAxisAorticValve,
        ViewId::ShortAxisMitralValve,
        ViewId::ShortAxisPapillary,
        ViewId::ShortAxisApex,
    ] {
        let r = t.reference(anatomy(), id);
        assert!(r.reachable && r.score.total > 0.9, "{id:?}: {}", r.score.total);
        if id != ViewId::ShortAxisAorticValve {
            assert!(
                r.score.circularity_ratio >= t.tolerances.circularity_min - 0.05,
                "{id:?}: circularity {}",
                r.score.circularity_ratio
            );
        }
    }
    // the papillary level: the LV blood enclosed by myocardium, with both papillary bodies
    let (img, _) = slice_at_rest(ViewId::ShortAxisPapillary);
    for l in [Label::LvBlood, Label::LvMyocardium, Label::PapillaryMuscle] {
        assert!(img.count(l) > 50, "{l:?}: {}", img.count(l));
    }
    let stats = ImageStats::of(&img, anatomy());
    let p = t.reference(anatomy(), ViewId::ShortAxisPapillary).pose;
    let ticks = t.checklist(ViewId::ShortAxisPapillary, &stats, &p, anatomy());
    for (item, ok) in t.target(ViewId::ShortAxisPapillary).checklist.iter().zip(&ticks) {
        assert!(*ok, "papillary level: `{}`", item.key);
    }
}
