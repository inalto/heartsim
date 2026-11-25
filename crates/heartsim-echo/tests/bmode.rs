//! Brief B32 items 1–5: the acoustic table, the line model, speckle, image formation and the
//! settings of the grey ultrasound image — on hand-made label images (uniform blocks, a rib, lung,
//! a tilted interface) and on the default pack driven by the coupled Tier-1 engine.

#![allow(clippy::cast_possible_wrap, clippy::cast_lossless)] // small image indices and test arithmetic

mod common;

use common::{Sim, model, pack};
use glam::Vec3;
use heartsim_echo::phases::{Side, VolumeInputs, volume_at};
use heartsim_echo::{
    AcousticError, AcousticTable, DesignedParams, EchoAnatomy, EchoImager, EchoWindow, GreyImage,
    ImageSettings, LabelImage, MotionFrame, MotionInput, MotionModel, ProbeFrame, ProbePose, SliceState,
    Sourced, Status,
};
use heartsim_geometry::Label;
use std::sync::{Arc, OnceLock};

// --- helpers ---------------------------------------------------------------------------------------

fn anatomy() -> &'static EchoAnatomy {
    static A: OnceLock<EchoAnatomy> = OnceLock::new();
    A.get_or_init(|| EchoAnatomy::new(pack(), &model(), DesignedParams::default()).expect("anatomy"))
}

fn table() -> Arc<AcousticTable> {
    AcousticTable::builtin()
}

/// A probe at the origin looking down −z (marker +y, elevation ±x), and its frame.
fn synth_pose(depth_mm: f32, sector_deg: f32) -> (ProbePose, ProbeFrame) {
    let pose = ProbePose {
        skin_mm: Vec3::ZERO,
        normal: Vec3::Z,
        tilt_deg: 0.0,
        rock_deg: 0.0,
        rotation_deg: 0.0,
        depth_mm,
        sector_deg,
    };
    (pose, pose.frame(60.0))
}

/// A label image of `f` (rest = current position, thorax-box samples) for the settings' size.
fn synth(f: &impl Fn(Vec3) -> Label, s: &ImageSettings) -> (LabelImage, ProbeFrame) {
    let (pose, frame) = synth_pose(s.depth_mm, s.sector_deg);
    let mut img = s.label_image();
    img.contact = true;
    img.depth_mm = pose.depth_mm;
    img.sector_deg = pose.sector_deg;
    for line in 0..img.lines {
        let dir = frame.line_direction(line, img.lines, img.sector_deg);
        for smp in 0..img.samples {
            let x = frame.apex_mm + dir * img.depth_of(smp);
            let i = line * img.samples + smp;
            img.labels[i] = f(x) as u8;
            img.rest_mm[i] = x;
            img.from_heart[i] = false;
        }
    }
    (img, frame)
}

/// Render `f` with `table` and `s`: the grey image and the envelope.
fn render(
    f: impl Fn(Vec3) -> Label,
    s: &ImageSettings,
    table: Arc<AcousticTable>,
) -> (GreyImage, Vec<f32>, LabelImage) {
    let (img, frame) = synth(&f, s);
    let mut imager = EchoImager::new(table);
    let mut out = GreyImage::new(1, 1);
    imager.image_with(&img, &frame, s, |x, _| f(x) as u8, &mut out);
    (out, imager.envelope().to_vec(), img)
}

/// The table with the default compensation set to `alpha` dB/cm/MHz.
fn table_with_tgc_reference(alpha: f32) -> Arc<AcousticTable> {
    let mut t = (*table()).clone();
    t.tgc_reference_db_cm_mhz = Sourced(alpha, Status::Designed, "test".into());
    Arc::new(t)
}

fn db(x: f32) -> f64 {
    20.0 * f64::from(x.max(1e-30)).log10()
}

/// Mean of 20·log10(envelope) over `lines` at samples `s0..s1`.
fn mean_db(
    env: &[f32],
    samples: usize,
    lines: impl Iterator<Item = usize> + Clone,
    s0: usize,
    s1: usize,
) -> f64 {
    let mut sum = 0.0;
    let mut n = 0usize;
    for l in lines {
        for s in s0..s1 {
            sum += db(env[l * samples + s]);
            n += 1;
        }
    }
    sum / n as f64
}

fn sample_at(s: &ImageSettings, depth_mm: f32) -> usize {
    ((depth_mm / s.depth_mm) * s.samples as f32) as usize
}

fn pearson(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len() as f64;
    let (ma, mb) = (a.iter().sum::<f64>() / n, b.iter().sum::<f64>() / n);
    let (mut sab, mut saa, mut sbb) = (0.0, 0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        sab += (x - ma) * (y - mb);
        saa += (x - ma) * (x - ma);
        sbb += (y - mb) * (y - mb);
    }
    sab / (saa * sbb).sqrt().max(1e-30)
}

fn input(sim: &Sim, t_ms: f64, enabled: bool) -> MotionInput<'_> {
    MotionInput {
        history: &sim.history,
        t_ms,
        resp_phase: 0.75, // expiration
        enabled,
        exaggeration: 1.0,
    }
}

/// Evaluate the model at `t`, take the instant into `st`, slice `pose` and image it.
#[allow(clippy::too_many_arguments)]
fn frame_at(
    m: &mut MotionModel,
    sim: &Sim,
    t_ms: f64,
    enabled: bool,
    frame: &mut MotionFrame,
    st: &mut SliceState,
    pose: &ProbePose,
    s: &ImageSettings,
) -> (LabelImage, GreyImage, Vec<f32>) {
    let a = anatomy();
    let inp = input(sim, t_ms, enabled);
    m.evaluate(&inp, frame);
    st.update(a, m, frame, &inp);
    let pose = s.apply_to(pose);
    let mut labels = s.label_image();
    st.slice(a, &pose, &mut labels);
    let mut imager = EchoImager::new(table());
    let mut grey = GreyImage::new(1, 1);
    imager.image(a, st, &pose, &labels, s, &mut grey);
    (labels, grey, imager.envelope().to_vec())
}

fn sinus_sim(until_ms: f64) -> Sim {
    let mut sim = Sim::sinus(60.0);
    sim.run_to(until_ms);
    sim
}

fn cycle_after(m: &MotionModel, sim: &Sim, after_ms: f64) -> heartsim_echo::CycleTimes {
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
    volume_at(q + 450.0, Side::Left, &inp).cycle.expect("a cycle")
}

fn fnv(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325_u64;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// --- the acoustic table -----------------------------------------------------------------------------

#[test]
fn acoustic_table_round_trips_and_rejects_bad_rows() {
    let t = table();
    t.validate().expect("builtin valid");
    let again = AcousticTable::from_ron(&t.to_ron()).expect("round trip");
    assert_eq!(*t, again);
    for l in Label::ALL {
        assert!(t.rows.contains_key(l.name()), "{}", l.name());
    }
    // a missing label
    let mut m = (*t).clone();
    m.rows.remove("lv_blood");
    assert!(matches!(m.validate(), Err(AcousticError::Label(_))));
    assert!(matches!(
        AcousticTable::from_ron(&m.to_ron()),
        Err(AcousticError::Label(_))
    ));
    // a non-positive impedance
    for z in [0.0f32, -1.0, f32::NAN] {
        let mut m = (*t).clone();
        m.rows.get_mut("bone").unwrap().impedance_mrayl.0 = z;
        assert!(matches!(m.validate(), Err(AcousticError::Value(_))), "Z = {z}");
    }
    // an unknown row, an empty source, a wrong version
    let mut m = (*t).clone();
    let r = m.rows["bone"].clone();
    m.rows.insert("cartilage".into(), r);
    assert!(matches!(m.validate(), Err(AcousticError::Label(_))));
    let mut m = (*t).clone();
    m.rows.get_mut("liver").unwrap().attenuation_db_cm_mhz.2 = " ".into();
    assert!(matches!(m.validate(), Err(AcousticError::Value(_))));
    let mut m = (*t).clone();
    m.format_version = 99;
    assert!(matches!(m.validate(), Err(AcousticError::FormatVersion(99))));
    // every unverified value of the research note stays marked so
    assert_eq!(
        t.row(Label::LvMyocardium).attenuation_db_cm_mhz.1,
        Status::Unverified
    );
    assert_eq!(t.aperture_mm.1, Status::Unverified);
}

// --- determinism ------------------------------------------------------------------------------------

#[test]
fn grey_image_is_deterministic_and_cadence_free() {
    let a = anatomy();
    let pose = a.window_pose(EchoWindow::Subcostal);
    let s = ImageSettings {
        seed: 7,
        ..ImageSettings::default()
    };
    let t = 3456.0;
    // run 1: evaluated at 60 Hz all the way
    let mut s1 = Sim::sinus(60.0);
    let mut m1 = model();
    let mut f1 = m1.new_frame();
    let mut st1 = SliceState::new(a);
    let mut tt = 2000.0;
    while tt < t {
        s1.run_to(tt + 100.0);
        let inp = input(&s1, tt, true);
        m1.evaluate(&inp, &mut f1);
        st1.update(a, &m1, &f1, &inp);
        tt += 16.7;
    }
    s1.run_to(t + 100.0);
    let (_, g1, _) = frame_at(&mut m1, &s1, t, true, &mut f1, &mut st1, &pose, &s);
    // run 2: fresh, one evaluation, imaged twice by one imager
    let mut s2 = Sim::sinus(60.0);
    s2.run_to(t + 100.0);
    let mut m2 = model();
    let mut f2 = m2.new_frame();
    let mut st2 = SliceState::new(a);
    let (labels2, g2, _) = frame_at(&mut m2, &s2, t, true, &mut f2, &mut st2, &pose, &s);
    let mut imager = EchoImager::new(table());
    let mut g3 = GreyImage::new(1, 1);
    imager.image(a, &st2, &s.apply_to(&pose), &labels2, &s, &mut g3);
    imager.image(a, &st2, &s.apply_to(&pose), &labels2, &s, &mut g3);
    let h = fnv(&g1.pixels);
    eprintln!(
        "grey image hash {h:016x}, mean grey {:.1}",
        g1.pixels.iter().map(|&p| f64::from(p)).sum::<f64>() / g1.pixels.len() as f64
    );
    assert_eq!(g1.pixels, g2.pixels);
    assert_eq!(g2.pixels, g3.pixels);
    assert!(g1.contact && g1.pixels.iter().any(|&p| p > 100));
    // another seed gives other speckle
    let s7 = ImageSettings { seed: 8, ..s.clone() };
    let mut g4 = GreyImage::new(1, 1);
    imager.image(a, &st2, &s.apply_to(&pose), &labels2, &s7, &mut g4);
    assert_ne!(g4.pixels, g2.pixels);
}

// --- speckle statistics -----------------------------------------------------------------------------

#[test]
fn speckle_in_uniform_myocardium_is_rayleigh() {
    let s = ImageSettings::default();
    // the compensation exactly cancels myocardium's attenuation
    let alpha = table().row(Label::LvMyocardium).attenuation_db_cm_mhz.0;
    let (_, env, _) = render(|_| Label::LvMyocardium, &s, table_with_tgc_reference(alpha));
    let (s0, s1) = (sample_at(&s, 60.0), sample_at(&s, 120.0));
    // nearly independent samples: every 4th sample of every 4th line, away from the sector's edges;
    // each 16-sample band normalised by its own mean power
    let mut xs = Vec::new();
    let mut band = s0;
    while band + 16 <= s1 {
        let mut vals = Vec::new();
        for l in (12..s.lines - 12).step_by(4) {
            for smp in (band..band + 16).step_by(4) {
                vals.push(f64::from(env[l * s.samples + smp]));
            }
        }
        let p = vals.iter().map(|v| v * v).sum::<f64>() / vals.len() as f64;
        xs.extend(vals.iter().map(|v| v / p.sqrt()));
        band += 16;
    }
    xs.sort_by(f64::total_cmp);
    let n = xs.len() as f64;
    // Rayleigh with E[x²] = 1: F(x) = 1 − exp(−x²)
    let mut d = 0.0f64;
    for (i, x) in xs.iter().enumerate() {
        let f = 1.0 - (-x * x).exp();
        d = d
            .max((f - i as f64 / n).abs())
            .max(((i + 1) as f64 / n - f).abs());
    }
    let crit = 1.628 / n.sqrt();
    let mean = xs.iter().sum::<f64>() / n;
    let sd = (xs.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / (n - 1.0)).sqrt();
    eprintln!(
        "Rayleigh: n {n}, KS D {d:.4} (1 % critical {crit:.4}), mean/sd {:.3}",
        mean / sd
    );
    assert!(d < crit, "KS {d} ≥ {crit}");
    assert!((mean / sd - 1.91).abs() <= 0.2, "mean/sd {}", mean / sd);
}

// --- contrast, shadow, enhancement, attenuation -----------------------------------------------------

/// End-diastole and end-systole of the beat after 3 s, and the run.
fn ed_es() -> (Sim, f64, f64) {
    let m = model();
    let sim = sinus_sim(5000.0);
    let c = cycle_after(&m, &sim, 3000.0);
    let (_, es) = c.ejection_ms.expect("ejects");
    (sim, c.closure_ms, es)
}

#[test]
fn blood_is_dark_and_leaflets_and_pericardium_bright() {
    let a = anatomy();
    let mut m = model();
    let (sim, ed, _) = ed_es();
    let mut frame = m.new_frame();
    let mut st = SliceState::new(a);
    let s = ImageSettings::default();
    let mut stats = Vec::new();
    for w in [EchoWindow::Apical, EchoWindow::Parasternal] {
        let (labels, grey, _) = frame_at(&mut m, &sim, ed, true, &mut frame, &mut st, &a.window_pose(w), &s);
        let (lines, samples) = (labels.lines, labels.samples);
        let mean_of = |want: &dyn Fn(usize, usize) -> bool| {
            let (mut sum, mut n) = (0.0f64, 0usize);
            for l in 0..lines {
                for smp in 0..samples {
                    if !labels.is_hidden(l, smp) && want(l, smp) {
                        sum += f64::from(grey.pixel(l, smp));
                        n += 1;
                    }
                }
            }
            (sum / n.max(1) as f64, n)
        };
        let is = |l: usize, smp: usize, x: Label| labels.label(l, smp) == x;
        // LV blood away from its border (2 lines, 6 samples), myocardium within 5 mm of it
        let r_ax = 6usize;
        let r_mm = 5.0f32;
        let near = |l: usize, smp: usize, x: Label, rl: usize, rs: usize| {
            (l.saturating_sub(rl)..=(l + rl).min(lines - 1))
                .any(|l2| (smp.saturating_sub(rs)..=(smp + rs).min(samples - 1)).any(|s2| is(l2, s2, x)))
        };
        let (blood, nb) = mean_of(&|l, smp| {
            is(l, smp, Label::LvBlood)
                && (l.saturating_sub(2)..=(l + 2).min(lines - 1)).all(|l2| {
                    (smp.saturating_sub(r_ax)..=(smp + r_ax).min(samples - 1))
                        .all(|s2| is(l2, s2, Label::LvBlood))
                })
        });
        let rs = (r_mm / labels.depth_mm * samples as f32) as usize;
        let (myo, nm) =
            mean_of(&|l, smp| is(l, smp, Label::LvMyocardium) && near(l, smp, Label::LvBlood, 3, rs));
        let (leaf, nl) = mean_of(&|l, smp| is(l, smp, Label::ValveLeaflet));
        let (peri, np) = mean_of(&|l, smp| is(l, smp, Label::Pericardium));
        let (myo_all, _) =
            mean_of(&|l, smp| is(l, smp, Label::LvMyocardium) || is(l, smp, Label::RvMyocardium));
        eprintln!(
            "{w:?}: LV blood {blood:.1} (n {nb}), adjacent LV myocardium {myo:.1} (n {nm}), all myocardium {myo_all:.1}, leaflets {leaf:.1} (n {nl}), pericardium {peri:.1} (n {np})"
        );
        stats.push((w, blood, myo, leaf, peri, myo_all, nb, nl, np));
    }
    for (w, blood, myo, leaf, peri, myo_all, nb, nl, np) in stats {
        assert!(nb > 200, "{w:?}: too little LV blood");
        assert!(blood < 0.2 * myo, "{w:?}: blood {blood} vs myocardium {myo}");
        if nl > 50 {
            assert!(leaf > myo_all, "{w:?}: leaflets {leaf} vs myocardium {myo_all}");
        }
        assert!(
            np > 200 && peri > myo_all,
            "{w:?}: pericardium {peri} vs myocardium {myo_all}"
        );
    }
}

/// Lateral position (mm along the marker) and depth (mm along the beam) of `x` for the synthetic
/// probe.
fn lat_depth(x: Vec3) -> (f32, f32) {
    let (_, f) = synth_pose(160.0, 75.0);
    let v = x - f.apex_mm;
    (v.dot(f.marker), v.dot(f.beam))
}

/// The lines whose ray is at lateral offset `lo..hi` mm at `depth_mm` (synthetic probe).
fn lines_through(s: &ImageSettings, depth_mm: f32, lo: f32, hi: f32) -> Vec<usize> {
    let (_, f) = synth_pose(s.depth_mm, s.sector_deg);
    (0..s.lines)
        .filter(|&l| {
            let d = f.line_direction(l, s.lines, s.sector_deg);
            let off = d.dot(f.marker) / d.dot(f.beam) * depth_mm;
            off > lo && off < hi
        })
        .collect()
}

#[test]
fn rib_and_lung_shadow_and_the_pleura_reverberates() {
    let s = ImageSettings::default();
    // a rib 15–25 mm deep, 10–30 mm to the marker side; soft tissue elsewhere
    let rib = |x: Vec3| {
        let (lat, d) = lat_depth(x);
        if (15.0..25.0).contains(&d) && (10.0..30.0).contains(&lat) {
            Label::Bone
        } else {
            Label::SoftTissue
        }
    };
    let (_, env, _) = render(rib, &s, table());
    let behind = lines_through(&s, 20.0, 13.0, 27.0);
    let space = lines_through(&s, 20.0, -27.0, -13.0);
    let (a, b) = (sample_at(&s, 60.0), sample_at(&s, 80.0));
    let (lr, ls) = (
        mean_db(&env, s.samples, behind.iter().copied(), a, b),
        mean_db(&env, s.samples, space.iter().copied(), a, b),
    );
    eprintln!(
        "rib: behind {lr:.1} dB, interspace {ls:.1} dB ({} and {} lines)",
        behind.len(),
        space.len()
    );
    assert!(
        !behind.is_empty() && lr <= ls - 20.0,
        "rib shadow {:.1} dB",
        ls - lr
    );
    // lung from 30 mm on the marker side
    let lung_side = |x: Vec3| {
        let (lat, d) = lat_depth(x);
        if d >= 30.0 && lat > 0.0 {
            Label::Lung
        } else {
            Label::SoftTissue
        }
    };
    let (_, env, _) = render(lung_side, &s, table());
    let behind = lines_through(&s, 70.0, 10.0, 40.0);
    let space = lines_through(&s, 70.0, -40.0, -10.0);
    let (lr, ls) = (
        mean_db(&env, s.samples, behind.iter().copied(), a, b),
        mean_db(&env, s.samples, space.iter().copied(), a, b),
    );
    eprintln!("lung: behind {lr:.1} dB, beside {ls:.1} dB");
    assert!(lr <= ls - 20.0, "lung shadow {:.1} dB", ls - lr);
    // a pleural plane at 30 mm across the whole sector: A-lines at 60, 90, 120 mm on the central line
    let lung_plane = |x: Vec3| {
        if lat_depth(x).1 >= 30.0 {
            Label::Lung
        } else {
            Label::SoftTissue
        }
    };
    let (_, env, img) = render(lung_plane, &s, table());
    let c = s.lines / 2;
    let row: Vec<f64> = (0..s.samples).map(|smp| db(env[c * s.samples + smp])).collect();
    let sp = (0..s.samples)
        .find(|&smp| img.label(c, smp) == Label::Lung)
        .expect("pleura");
    let pleura_mm = img.depth_of(sp);
    for k in 2..=4 {
        let at = k * sp;
        let peak = (at.saturating_sub(3)..=(at + 3).min(s.samples - 1))
            .map(|i| row[i])
            .fold(f64::MIN, f64::max);
        // between this repeat and the previous one
        let mid = (at - sp / 2 - 4..at - sp / 2 + 4).map(|i| row[i]).sum::<f64>() / 8.0;
        eprintln!(
            "A-line {k} at {:.0} mm (pleura {pleura_mm:.1} mm): peak {peak:.1} dB, between {mid:.1} dB",
            k as f32 * pleura_mm
        );
        assert!(peak >= mid + 20.0, "no reverberation at {k}× the pleural depth");
    }
}

#[test]
fn tissue_behind_blood_is_brighter_than_behind_myocardium() {
    let s = ImageSettings::default();
    // blood (marker-away side) or myocardium (marker side) from 40 to 80 mm, soft tissue elsewhere
    let f = |x: Vec3| {
        let (lat, d) = lat_depth(x);
        if (40.0..80.0).contains(&d) {
            if lat < 0.0 {
                Label::LvBlood
            } else {
                Label::LvMyocardium
            }
        } else {
            Label::SoftTissue
        }
    };
    let (_, env, _) = render(f, &s, table());
    let (a, b) = (sample_at(&s, 90.0), sample_at(&s, 110.0));
    let lb = mean_db(
        &env,
        s.samples,
        lines_through(&s, 60.0, -45.0, -10.0).into_iter(),
        a,
        b,
    );
    let lm = mean_db(
        &env,
        s.samples,
        lines_through(&s, 60.0, 10.0, 45.0).into_iter(),
        a,
        b,
    );
    eprintln!("behind blood {lb:.1} dB, behind myocardium {lm:.1} dB");
    assert!(lb > lm + 3.0);
}

/// Per-centimetre means of 20·log10(envelope) over the central lines, 20–140 mm.
fn depth_profile(env: &[f32], s: &ImageSettings) -> Vec<(f64, f64)> {
    let lines = (s.lines / 4..3 * s.lines / 4).collect::<Vec<_>>();
    (2..14)
        .map(|cm| {
            let (a, b) = (
                sample_at(s, cm as f32 * 10.0),
                sample_at(s, cm as f32 * 10.0 + 10.0),
            );
            (
                cm as f64 + 0.5,
                mean_db(env, s.samples, lines.iter().copied(), a, b),
            )
        })
        .collect()
}

#[test]
fn attenuation_without_compensation_and_flat_with_it() {
    let t = table();
    let alpha = f64::from(t.row(Label::SoftTissue).attenuation_db_cm_mhz.0);
    for f in [2.5f32, 4.0] {
        let s = ImageSettings {
            frequency_mhz: f,
            tgc_auto: false,
            ..ImageSettings::default()
        };
        let p = depth_profile(&render(|_| Label::SoftTissue, &s, t.clone()).1, &s);
        let n = p.len() as f64;
        let (mx, my) = (
            p.iter().map(|q| q.0).sum::<f64>() / n,
            p.iter().map(|q| q.1).sum::<f64>() / n,
        );
        let slope = p.iter().map(|q| (q.0 - mx) * (q.1 - my)).sum::<f64>()
            / p.iter().map(|q| (q.0 - mx).powi(2)).sum::<f64>();
        let want = -2.0 * alpha * f64::from(f);
        eprintln!("{f} MHz, TGC off: {slope:.3} dB/cm (2·α·f = {:.3})", -want);
        assert!(
            (slope - want).abs() <= 0.1 * want.abs(),
            "slope {slope} vs {want}"
        );
        let s = ImageSettings {
            frequency_mhz: f,
            ..ImageSettings::default()
        };
        let p = depth_profile(&render(|_| Label::SoftTissue, &s, t.clone()).1, &s);
        let (lo, hi) = p
            .iter()
            .fold((f64::MAX, f64::MIN), |(lo, hi), q| (lo.min(q.1), hi.max(q.1)));
        eprintln!("{f} MHz, default TGC: per-cm means {lo:.1} … {hi:.1} dB");
        assert!(hi - lo <= 3.0, "default TGC not flat: {:.1} dB", hi - lo);
    }
}

// --- tissue-locked speckle --------------------------------------------------------------------------

fn patch(g: &GreyImage, l: usize, smp: usize, hl: usize, hs: usize) -> Vec<f64> {
    let mut v = Vec::new();
    for l2 in l - hl..=l + hl {
        for s2 in smp - hs..=smp + hs {
            v.push(f64::from(g.pixel(l2, s2)));
        }
    }
    v
}

/// (line, sample) of current-position point `x` for `pose`'s frame, rounded.
fn polar_of(x: Vec3, pose: &ProbePose, labels: &LabelImage) -> (f32, f32) {
    let f = pose.frame(60.0);
    let v = x - f.apex_mm;
    let r = v.length();
    let th = v.dot(f.marker).atan2(v.dot(f.beam));
    let line = (th / labels.sector_deg.to_radians() + 0.5) * labels.lines as f32 - 0.5;
    let smp = r / labels.depth_mm * labels.samples as f32 - 0.5;
    (line, smp)
}

#[test]
fn speckle_rides_with_the_tissue() {
    let a = anatomy();
    let mut m = model();
    let mut sim = sinus_sim(7000.0);
    let s = ImageSettings::default();
    let pose = s.apply_to(&a.window_pose(EchoWindow::Apical));
    let mut frame = m.new_frame();
    let mut st = SliceState::new(a);
    // motion off, one beat apart: the same image
    let (_, g1, _) = frame_at(&mut m, &sim, 4000.0, false, &mut frame, &mut st, &pose, &s);
    let (_, g2, _) = frame_at(&mut m, &sim, 5000.0, false, &mut frame, &mut st, &pose, &s);
    let all = |g: &GreyImage| g.pixels.iter().map(|&p| f64::from(p)).collect::<Vec<_>>();
    let c_off = pearson(&all(&g1), &all(&g2));
    eprintln!("motion off, frames 1 s apart: correlation {c_off:.6}");
    assert!(c_off > 0.9999);
    // a wall region tracked over 40 ms: of the instants tried in one beat, the patch that moves
    // the most for its speckle size
    let c = cycle_after(&m, &sim, 3000.0);
    let (open, close) = c.ejection_ms.expect("ejects");
    sim.run_to(c.inflow_open_ms + 600.0);
    let (hl, hs) = (4usize, 16usize);
    let mut best: Option<(f32, f32, usize, usize, usize, usize, GreyImage, GreyImage, f64)> = None;
    for t1 in [
        open + 10.0,
        open + 50.0,
        close - 80.0,
        c.inflow_open_ms + 10.0,
        c.inflow_open_ms + 50.0,
    ] {
        let (l1, g1, _) = frame_at(&mut m, &sim, t1, true, &mut frame, &mut st, &pose, &s);
        let st1 = st.clone();
        let (l2, g2, _) = frame_at(&mut m, &sim, t1 + 40.0, true, &mut frame, &mut st, &pose, &s);
        let max_s = (120.0 / l1.depth_mm * l1.samples as f32) as usize;
        let dr = l1.depth_mm / l1.samples as f32;
        let dth = l1.sector_deg.to_radians() / l1.lines as f32;
        for l in (hl + 2..l1.lines - hl - 2).step_by(2) {
            for smp in (hs + 2..max_s.min(l1.samples - hs - 2)).step_by(4) {
                let wall = (l - hl..=l + hl)
                    .flat_map(|a2| (smp - hs..=smp + hs).map(move |b2| (a2, b2)))
                    .filter(|&(a2, b2)| {
                        matches!(l1.label(a2, b2), Label::LvMyocardium | Label::RvMyocardium)
                            && !l1.is_hidden(a2, b2)
                    })
                    .count();
                if wall < (2 * hl + 1) * (2 * hs + 1) * 9 / 10 {
                    continue;
                }
                let x_rest = l1.rest_mm[l * l1.samples + smp];
                let moved = st.field().at(x_rest) - st1.field().at(x_rest);
                let grain = dr.max(l1.depth_of(smp) * dth);
                let ratio = moved.length() / grain;
                if best.as_ref().is_some_and(|b| ratio <= b.0) {
                    continue;
                }
                // the moved position: the frame-2 sample whose rest point is nearest (around the
                // push-forward of the rest point)
                let (pl, ps) = polar_of(x_rest + st.field().at(x_rest), &pose, &l2);
                let (pl, ps) = (pl.round() as isize, ps.round() as isize);
                let mut near: Option<(f32, usize, usize)> = None;
                for dl in -3isize..=3 {
                    for ds in -12isize..=12 {
                        let (a2, b2) = (pl + dl, ps + ds);
                        if a2 < (hl + 1) as isize
                            || b2 < (hs + 1) as isize
                            || a2 as usize + hl + 1 >= l2.lines
                            || b2 as usize + hs + 1 >= l2.samples
                        {
                            continue;
                        }
                        let e = l2.rest_mm[a2 as usize * l2.samples + b2 as usize].distance(x_rest);
                        if near.is_none_or(|n| e < n.0) {
                            near = Some((e, a2 as usize, b2 as usize));
                        }
                    }
                }
                if let Some((e, pl, ps)) = near.filter(|n| n.0 < 0.5) {
                    best = Some((
                        ratio,
                        moved.length(),
                        l,
                        smp,
                        pl,
                        ps,
                        g1.clone(),
                        g2.clone(),
                        f64::from(e) + t1 - open,
                    ));
                }
            }
        }
    }
    let (ratio, d, l, smp, pl, ps, g1, g2, info) = best.expect("a wall patch");
    eprintln!("best patch: {ratio:.2} speckle spacings, (rest mismatch + t1 − AVO) {info:.2}");
    let p1 = patch(&g1, l, smp, hl, hs);
    let tracked = pearson(&p1, &patch(&g2, pl, ps, hl, hs));
    let fixed = pearson(&p1, &patch(&g2, l, smp, hl, hs));
    eprintln!(
        "wall patch at line {l}, sample {smp} moved {d:.2} mm to line {pl}, sample {ps} in 40 ms: tracked {tracked:.3}, fixed window {fixed:.3}"
    );
    assert!(tracked > 0.7, "tracked {tracked}");
    assert!(fixed < 0.5 && fixed < tracked - 0.2, "fixed {fixed}");
    // out of plane: tilting decorrelates, more the further
    let (base_l, base, _) = frame_at(&mut m, &sim, 4000.0, false, &mut frame, &mut st, &pose, &s);
    let heart: Vec<usize> = (0..base.pixels.len())
        .filter(|&i| {
            base_l.labels[i] == Label::LvMyocardium as u8
                && base_l.texels()[i] & heartsim_echo::HIDDEN_BIT == 0
        })
        .collect();
    let pick = |g: &GreyImage| heart.iter().map(|&i| f64::from(g.pixels[i])).collect::<Vec<_>>();
    let mut last = 1.0f64;
    for tilt in [0.5f32, 1.0, 2.0, 4.0] {
        let p = ProbePose {
            tilt_deg: pose.tilt_deg + tilt,
            ..pose
        };
        let (_, g, _) = frame_at(&mut m, &sim, 4000.0, false, &mut frame, &mut st, &p, &s);
        let c = pearson(&pick(&base), &pick(&g));
        eprintln!("tilt +{tilt}°: correlation {c:.3}");
        assert!(c < last, "correlation did not fall at +{tilt}°");
        last = c;
    }
    assert!(last < 0.5);
}

// --- angle dependence -------------------------------------------------------------------------------

#[test]
fn a_perpendicular_interface_is_brighter_than_an_oblique_one() {
    let s = ImageSettings::default();
    let (_, f) = synth_pose(s.depth_mm, s.sector_deg);
    // soft tissue over bone through the point 60 mm down the beam, normal at `deg` from the beam,
    // tilted in the image plane (toward the marker) or across it (toward the elevation axis)
    let peak = |deg: f32, across: bool| {
        let c = f.apex_mm + f.beam * 60.0;
        let side = if across { f.elevation } else { f.marker };
        let (sn, cs) = deg.to_radians().sin_cos();
        let n = f.beam * cs + side * sn;
        let g = move |x: Vec3| {
            if (x - c).dot(n) >= 0.0 {
                Label::Bone
            } else {
                Label::SoftTissue
            }
        };
        let (_, env, _) = render(g, &s, table());
        let centre = s.lines / 2;
        let at = sample_at(&s, 60.0);
        let lines = centre - 1..=centre;
        lines
            .flat_map(|l| (at - 6..at + 6).map(move |smp| (l, smp)))
            .map(|(l, smp)| db(env[l * s.samples + smp]))
            .fold(f64::MIN, f64::max)
    };
    let (p0, p60, a60) = (peak(0.0, false), peak(60.0, false), peak(60.0, true));
    eprintln!("interface peak: perpendicular {p0:.1} dB, 60° in plane {p60:.1} dB, 60° across {a60:.1} dB");
    assert!(p0 > p60 + 6.0 && p0 > a60 + 6.0);
}

// --- settings --------------------------------------------------------------------------------------

fn mean_grey(g: &GreyImage) -> f64 {
    g.pixels.iter().map(|&p| f64::from(p)).sum::<f64>() / g.pixels.len() as f64
}

#[test]
fn settings_change_the_image_in_the_expected_direction() {
    let half = |x: Vec3| {
        let (lat, d) = lat_depth(x);
        if d > 30.0 && lat < 0.0 {
            Label::LvBlood
        } else {
            Label::LvMyocardium
        }
    };
    let _ = &half;
    // gain: monotonic
    let mut last = -1.0;
    for gain in [-20.0f32, -10.0, 0.0, 10.0, 20.0] {
        let s = ImageSettings {
            gain_db: gain,
            ..ImageSettings::default()
        };
        let g = mean_grey(&render(half, &s, table()).0);
        eprintln!("gain {gain} dB: mean grey {g:.1}");
        assert!(g > last);
        last = g;
    }
    // dynamic range: smaller → more contrast (grey levels per dB): between myocardium and the
    // chest wall's soft tissue, and the speckle's own spread, at 20–50 mm
    let two = |x: Vec3| {
        if lat_depth(x).0 < 0.0 {
            Label::SoftTissue
        } else {
            Label::LvMyocardium
        }
    };
    let contrast = |dr: f32| {
        let s = ImageSettings {
            dynamic_range_db: dr,
            ..ImageSettings::default()
        };
        let (g, _, img) = render(two, &s, table());
        let m = |want: Label| {
            let v: Vec<f64> = (0..g.pixels.len())
                .filter(|&i| {
                    img.labels[i] == want as u8 && (20.0..50.0).contains(&img.depth_of(i % s.samples))
                })
                .map(|i| f64::from(g.pixels[i]))
                .collect();
            let mean = v.iter().sum::<f64>() / v.len() as f64;
            let sd = (v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / v.len() as f64).sqrt();
            (mean, sd)
        };
        let ((myo, sd), (soft, _)) = (m(Label::LvMyocardium), m(Label::SoftTissue));
        ((myo - soft).abs(), sd)
    };
    let ((c40, sd40), (c60, sd60)) = (contrast(40.0), contrast(60.0));
    eprintln!(
        "|myocardium − soft tissue|: {c40:.1} grey levels at 40 dB, {c60:.1} at 60 dB; speckle sd {sd40:.1} and {sd60:.1}"
    );
    assert!(c40 > c60);
    // frequency: finer lateral speckle and less penetration at 5 MHz than at 2 MHz
    let lateral_width = |f: f32| {
        let s = ImageSettings {
            frequency_mhz: f,
            ..ImageSettings::default()
        };
        let (_, env, _) = render(|_| Label::LvMyocardium, &s, table());
        // the lag (in mm) where the lateral autocorrelation of the envelope at 60–80 mm falls below ½
        let (a, b) = (sample_at(&s, 60.0), sample_at(&s, 80.0));
        let lat_mm = 70.0 * s.sector_deg.to_radians() / s.lines as f32;
        let mut prev = 1.0;
        for lag in 1..20usize {
            let (mut x, mut y) = (Vec::new(), Vec::new());
            for l in 10..s.lines - 10 - lag {
                for smp in a..b {
                    x.push(f64::from(env[l * s.samples + smp]));
                    y.push(f64::from(env[(l + lag) * s.samples + smp]));
                }
            }
            let c = pearson(&x, &y);
            if c < 0.5 {
                return (lag as f64 - 1.0 + (prev - 0.5) / (prev - c)) * f64::from(lat_mm);
            }
            prev = c;
        }
        f64::INFINITY
    };
    let (w2, w5) = (lateral_width(2.0), lateral_width(5.0));
    eprintln!("lateral speckle half-correlation width: {w2:.2} mm at 2 MHz, {w5:.2} mm at 5 MHz");
    assert!(w5 < w2);
    let deep = |f: f32| {
        let s = ImageSettings {
            frequency_mhz: f,
            ..ImageSettings::default()
        };
        let (g, _, _) = render(|_| Label::LvMyocardium, &s, table());
        let (a, b) = (sample_at(&s, 110.0), sample_at(&s, 150.0));
        (0..s.lines)
            .flat_map(|l| (a..b).map(move |smp| (l, smp)))
            .map(|(l, smp)| f64::from(g.pixel(l, smp)))
            .sum::<f64>()
            / ((b - a) * s.lines) as f64
    };
    let (d2, d5) = (deep(2.0), deep(5.0));
    eprintln!("mean grey at 11–15 cm in myocardium: {d2:.1} at 2 MHz, {d5:.1} at 5 MHz");
    assert!(d5 < d2);
}

#[test]
fn every_setting_in_range_gives_a_finite_image() {
    let a = anatomy();
    let st = SliceState::new(a);
    let pose = a.window_pose(EchoWindow::Parasternal);
    let r = ImageSettings::default();
    let lo = ImageSettings {
        frequency_mhz: ImageSettings::FREQUENCY_RANGE_MHZ.0,
        depth_mm: heartsim_echo::probe::DEPTH_RANGE_MM.0,
        sector_deg: heartsim_echo::probe::SECTOR_RANGE_DEG.0,
        focus_mm: ImageSettings::FOCUS_RANGE_MM.0,
        gain_db: ImageSettings::GAIN_RANGE_DB.0,
        tgc_auto: false,
        tgc_db: [ImageSettings::TGC_RANGE_DB.0; heartsim_echo::TGC_BANDS],
        dynamic_range_db: ImageSettings::DYNAMIC_RANGE_DB.0,
        lines: ImageSettings::LINES_RANGE.0,
        samples: ImageSettings::SAMPLES_RANGE.0,
        seed: u64::MAX,
    };
    let hi = ImageSettings {
        frequency_mhz: ImageSettings::FREQUENCY_RANGE_MHZ.1,
        depth_mm: heartsim_echo::probe::DEPTH_RANGE_MM.1,
        sector_deg: heartsim_echo::probe::SECTOR_RANGE_DEG.1,
        focus_mm: ImageSettings::FOCUS_RANGE_MM.1,
        gain_db: ImageSettings::GAIN_RANGE_DB.1,
        tgc_auto: true,
        tgc_db: [ImageSettings::TGC_RANGE_DB.1; heartsim_echo::TGC_BANDS],
        dynamic_range_db: ImageSettings::DYNAMIC_RANGE_DB.1,
        lines: ImageSettings::LINES_RANGE.1,
        samples: ImageSettings::SAMPLES_RANGE.1,
        seed: 0,
    };
    let wild = ImageSettings {
        frequency_mhz: f32::NAN,
        depth_mm: f32::INFINITY,
        focus_mm: -5.0,
        gain_db: 1e9,
        dynamic_range_db: 0.0,
        lines: 0,
        samples: usize::MAX,
        ..ImageSettings::default()
    };
    let mut imager = EchoImager::new(table());
    let mut grey = GreyImage::new(1, 1);
    for s in [r, lo, hi, wild] {
        let p = s.apply_to(&pose);
        let mut labels = s.label_image();
        st.slice(a, &p, &mut labels);
        imager.image(a, &st, &p, &labels, &s, &mut grey);
        let c = s.clamped();
        assert_eq!((grey.lines, grey.samples), (c.lines, c.samples));
        assert!(imager.envelope().iter().all(|e| e.is_finite()), "{s:?}");
        assert!(grey.contact);
    }
    // no contact: black
    let p = ProbePose {
        tilt_deg: 70.0,
        ..pose
    };
    let mut labels = ImageSettings::default().label_image();
    st.slice(a, &p, &mut labels);
    imager.image(a, &st, &p, &labels, &ImageSettings::default(), &mut grey);
    assert!(!grey.contact && grey.pixels.iter().all(|&g| g == 0));
}

// --- images for review ------------------------------------------------------------------------------

/// Scan-convert a polar grey image to a `w × h` sector (apex at the top centre, marker side on
/// the right), grey outside the sector 0.
fn sector_png(g: &GreyImage, w: u32, h: u32) -> image::GrayImage {
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

/// Writes the four windows' grey sectors at end-diastole and end-systole as PNGs when
/// `HEARTSIM_ECHO_PNG` names a directory (for review of the look, not a test).
#[test]
#[ignore = "writes review images"]
fn write_window_pngs() {
    let Some(dir) = std::env::var_os("HEARTSIM_ECHO_PNG") else {
        return;
    };
    let a = anatomy();
    let mut m = model();
    let (sim, ed, es) = ed_es();
    let mut frame = m.new_frame();
    let mut st = SliceState::new(a);
    let s = ImageSettings::default();
    for (name, t) in [("ed", ed), ("es", es)] {
        for w in EchoWindow::ALL {
            let (_, g, _) = frame_at(&mut m, &sim, t, true, &mut frame, &mut st, &a.window_pose(w), &s);
            let img = sector_png(&g, 640, 520);
            img.save(std::path::Path::new(&dir).join(format!("{w:?}_{name}.png")))
                .expect("png");
        }
    }
}
