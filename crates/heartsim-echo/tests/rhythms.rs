//! Brief B30 acceptance, rhythms: an early PVC does not eject and the beat after its compensatory
//! pause ejects more; LBBB has a septal flash and an early septum, RBBB a late RV free wall; AF has
//! no atrial contraction and an EF that varies with the preceding RR; in complete heart block the
//! atria contract after the P waves; VF and asystole hold the cavity volume.

mod common;
use common::*;
use glam::Vec3;
use heartsim_ecg::Tier0Scenario;
use heartsim_ecg::pcg::BeatOrigin;
use heartsim_echo::{MotionChamber, MotionModel, MotionParams};
use std::path::PathBuf;

/// EF of the beat at index `k` of the history, from the deformed mesh at its M1 and A2.
fn beat_ef(model: &mut MotionModel, sim: &Sim, k: usize) -> Option<f64> {
    let (_, c) = &sim.history.beats()[k];
    let (m1, a2) = (c.m1?.onset.0, c.a2?.onset.0);
    let mut f = model.new_frame();
    eval_still(model, sim, m1, &mut f);
    let edv = lv_volume(model, &f);
    eval_still(model, sim, a2, &mut f);
    let esv = lv_volume(model, &f);
    Some((edv - esv) / edv)
}

fn sim_from_text(text: &str) -> Sim {
    let s = Tier0Scenario::from_ron(text).unwrap();
    let timeline = s
        .scenario
        .timeline
        .iter()
        .map(|a| (a.at.0, a.action.clone()))
        .collect();
    let rate = s.timing.sinus.respiration_rate_bpm;
    let mut sim = Sim::new(s.timing.clone(), timeline, rate);
    sim.history = sim.history.with_auscultation(&s.scenario.auscultation);
    sim
}

#[test]
fn an_early_pvc_does_not_eject_and_the_next_beat_ejects_more() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/scenarios/pvc_r_on_t.ron");
    // the library's R-on-T PVC is coupled at 300 ms; 280 ms puts it inside `min_ejecting_rr_ms`
    let text = std::fs::read_to_string(path)
        .unwrap()
        .replace("coupling_ms: 300.0", "coupling_ms: 280.0");
    let mut sim = sim_from_text(&text);
    sim.run_to(12_000.0);
    let mut model = model();
    let beats: Vec<_> = sim.history.beats().to_vec();
    let k = beats
        .iter()
        .position(|(b, _)| b.qrs_onset.0 > 3000.0 && b.origin == BeatOrigin::Ventricular)
        .expect("a PVC");
    let (pvc, c) = &beats[k];
    assert!(
        pvc.rr_ms.unwrap() < 300.0 && c.a2.is_none(),
        "the PVC is inside min_ejecting_rr_ms"
    );
    let next_q = beats[k + 1].0.qrs_onset.0;
    let mut f = model.new_frame();
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    // the PVC's own cycle: after the preceding beat's ejection (an R-on-T PVC falls inside it)
    // until its mitral valve would reopen — the samples where the model is in the PVC's
    // isovolumic phase
    let mut held = 0;
    let mut t = pvc.qrs_onset.0;
    while t < next_q {
        eval_still(&mut model, &sim, t, &mut f);
        if f.readout.lv_phase == Some(heartsim_echo::CardiacPhase::IsovolumicContraction) {
            let v = lv_volume(&model, &f);
            lo = lo.min(v);
            hi = hi.max(v);
            held += 1;
        }
        t += 5.0;
    }
    assert!(held > 10, "the PVC's cycle was sampled");
    let change = (hi - lo) / hi;
    let sinus: Vec<f64> = (k - 3..k).filter_map(|j| beat_ef(&mut model, &sim, j)).collect();
    let post = beat_ef(&mut model, &sim, k + 1).unwrap();
    eprintln!(
        "PVC RR {:.0} ms: LV volume change {:.2} % over the PVC; sinus EF {:?}; post-ectopic EF {:.3} (RR {:.0} ms)",
        pvc.rr_ms.unwrap(),
        change * 100.0,
        sinus,
        post,
        beats[k + 1].0.rr_ms.unwrap()
    );
    assert!(
        change <= 0.02,
        "the PVC does not change the LV volume by more than 2 %"
    );
    assert!(
        sinus.iter().all(|&e| post > e),
        "post-ectopic beat ejects more than the sinus beats"
    );
}

/// Mean inward motion (mm) of LV endocardial segments and the RV free wall over a beat, sampled
/// every `step` ms from its QRS to 100 ms after A2.
struct Wall {
    t: Vec<f64>,
    septum: Vec<f64>,
    lateral: Vec<f64>,
    /// Mean of all LV endocardial segments 1–16.
    mean: Vec<f64>,
    rv_free: Vec<f64>,
    /// Mean of the whole RV mesh.
    rv_mean: Vec<f64>,
    m1: f64,
    ao: f64,
    es: f64,
}

/// Each group's inward motion at end-systole of a normal sinus beat (the template at about full
/// amplitude): `[septum, lateral, mean, rv_free, rv_mean]`.
fn reference() -> [f64; 5] {
    static REF: std::sync::OnceLock<[f64; 5]> = std::sync::OnceLock::new();
    *REF.get_or_init(|| {
        let w = raw_wall_motion("sinus_normal", 5.0, true);
        let k = w.t.iter().position(|t| *t >= w.es).unwrap();
        [w.septum[k], w.lateral[k], w.mean[k], w.rv_free[k], w.rv_mean[k]]
    })
}

/// Wall motion of a beat with every group normalised by its [`reference`] value, so that 1 means
/// "as far as a normal end-systole": regional differences are then differences of contraction,
/// not of how far each part of the template moves.
fn wall_motion(id: &str, step: f64) -> Wall {
    let r = reference();
    let mut w = raw_wall_motion(id, step, false);
    for (x, k) in [
        (&mut w.septum, 0),
        (&mut w.lateral, 1),
        (&mut w.mean, 2),
        (&mut w.rv_free, 3),
        (&mut w.rv_mean, 4),
    ] {
        for v in x.iter_mut() {
            *v /= r[k];
        }
    }
    w
}

fn raw_wall_motion(id: &str, step: f64, _reference: bool) -> Wall {
    let mut sim = Sim::from_scenario(id);
    sim.run_to(5000.0);
    let mut model = model();
    let probe = LvProbe::new(&model);
    let w = beat_after(&sim, 3000.0);
    // RV free wall: RV-mesh vertices more than 12 mm from the LV mesh
    let lv_rest: Vec<Vec3> = probe.rest.clone();
    let grid = heartsim_geometry::HashGrid::new(&lv_rest, 3.0);
    let rv_rest: Vec<Vec3> = model.part_rest("rv").unwrap().iter().map(pos).collect();
    let rv_ax = *model.frame().axis(MotionChamber::RightVentricle);
    let free: Vec<(usize, Vec3)> = rv_rest
        .iter()
        .enumerate()
        .filter(|(_, p)| grid.nearest(**p).is_some_and(|(d, _)| d > 12.0))
        .map(|(i, p)| (i, rv_ax.coords(*p).radial))
        .collect();
    let mut out = Wall {
        t: Vec::new(),
        septum: Vec::new(),
        lateral: Vec::new(),
        mean: Vec::new(),
        rv_free: Vec::new(),
        rv_mean: Vec::new(),
        m1: w.m1,
        ao: w.ao,
        es: w.a2.unwrap(),
    };
    let mut f = model.new_frame();
    let mut t = w.qrs;
    while t <= w.a2.unwrap() + 100.0 {
        eval_still(&mut model, &sim, t, &mut f);
        out.t.push(t);
        out.septum.push(probe.inward_mm(&f, &[8, 9]));
        out.lateral.push(probe.inward_mm(&f, &[11, 12]));
        out.mean.push(probe.inward_mm(&f, &(1..=16).collect::<Vec<u8>>()));
        let rv = &f.part("rv").unwrap().vertices;
        let s: f64 = free
            .iter()
            .map(|(i, r)| f64::from(-(pos(&rv[*i]) - rv_rest[*i]).dot(*r)))
            .sum();
        out.rv_free.push(s / free.len().max(1) as f64);
        let all: f64 = rv_rest
            .iter()
            .enumerate()
            .map(|(i, p)| f64::from(-(pos(&rv[i]) - *p).dot(rv_ax.coords(*p).radial)))
            .sum();
        out.rv_mean.push(all / rv_rest.len() as f64);
        t += step;
    }
    out
}

fn peak_time(t: &[f64], x: &[f64]) -> f64 {
    let k = (0..x.len()).max_by(|&a, &b| x[a].total_cmp(&x[b])).unwrap();
    t[k]
}

/// Regional motion relative to the global one: a segment's inward motion minus the LV mean.
fn relative(x: &[f64], mean: &[f64]) -> Vec<f64> {
    x.iter().zip(mean).map(|(a, b)| a - b).collect()
}

/// Septal flash index, mm: inside isovolumic contraction, the septum's largest inward motion
/// relative to the LV mean (from its value at M1), minus what is left of it at aortic opening —
/// positive when the septum moves in early and is pushed back out by the late wall before
/// ejection. Also returns the largest relative inward motion itself.
fn septal_flash(w: &Wall) -> (f64, f64) {
    let rel = relative(&w.septum, &w.mean);
    let at = |t0: f64| {
        let k = w.t.iter().position(|t| *t >= t0).unwrap();
        rel[k]
    };
    let base = at(w.m1);
    let peak =
        w.t.iter()
            .zip(&rel)
            .filter(|(t, _)| **t >= w.m1 && **t <= w.ao)
            .map(|(_, r)| r - base)
            .fold(f64::NEG_INFINITY, f64::max);
    (peak, peak - (at(w.ao) - base))
}

#[test]
fn lbbb_has_a_septal_flash_and_normal_conduction_none() {
    let lbbb = wall_motion("lbbb", 2.0);
    let normal = wall_motion("sinus_normal", 2.0);
    // peak regional inward motion relative to the LV mean (the global, volume-driven part is the
    // same for every segment)
    let peak = |w: &Wall, x: &[f64]| peak_time(&w.t, &relative(x, &w.mean));
    let (lp, ls) = (peak(&lbbb, &lbbb.septum), peak(&lbbb, &lbbb.lateral));
    let (np, ns) = (peak(&normal, &normal.septum), peak(&normal, &normal.lateral));
    // in mm of septal motion: the normalised values times the septum's normal end-systolic motion
    let mm = reference()[0];
    let (l_peak, l_flash) = septal_flash(&lbbb);
    let (n_peak, n_flash) = septal_flash(&normal);
    let (l_peak, l_flash, n_peak, n_flash) = (l_peak * mm, l_flash * mm, n_peak * mm, n_flash * mm);
    eprintln!(
        "LBBB: septum peaks {:.0} ms after QRS, lateral {:.0}; flash: septal inward relative to the LV mean {l_peak:.2} mm, index {l_flash:.3} mm (septum normal ES {mm:.1} mm)",
        lp - lbbb.t[0],
        ls - lbbb.t[0]
    );
    eprintln!(
        "normal: septum peaks {:.0} ms after QRS, lateral {:.0}; flash: septal inward relative to the LV mean {n_peak:.2} mm, index {n_flash:.3} mm",
        np - normal.t[0],
        ns - normal.t[0]
    );
    assert!(
        lp < ls,
        "LBBB: septal peak inward motion precedes the lateral wall's"
    );
    assert!(
        l_flash > 0.1,
        "LBBB: septal flash index positive ({l_flash:.2} mm)"
    );
    assert!(
        n_flash < 0.05,
        "normal conduction: no septal flash ({n_flash:.2} mm)"
    );
}

#[test]
fn rbbb_has_a_late_rv_free_wall() {
    let w = wall_motion("rbbb", 2.0);
    let (sp, rp) = (
        peak_time(&w.t, &relative(&w.septum, &w.mean)),
        peak_time(&w.t, &relative(&w.rv_free, &w.rv_mean)),
    );
    eprintln!(
        "RBBB: septum (relative to the LV mean) peaks {:.0} ms after QRS, RV free wall (relative to the RV mean) {:.0}",
        sp - w.t[0],
        rp - w.t[0]
    );
    assert!(rp > sp, "RBBB: RV free wall later than the septum");
}

#[test]
fn af_has_no_atrial_contraction_and_a_varying_ef() {
    let mut sim = Sim::from_scenario("atrial_fibrillation");
    let mut model = model();
    // atrial activity, evaluated 200 ms behind the engine as the app draws it
    let mut f = model.new_frame();
    let mut max_atrial = 0.0f32;
    let mut t = 3000.0;
    while t < 20_000.0 {
        sim.run_to(t + 200.0);
        eval_still(&mut model, &sim, t, &mut f);
        let a = f.readout.chamber_activity;
        max_atrial = max_atrial
            .max(a[MotionChamber::LeftAtrium.index()])
            .max(a[MotionChamber::RightAtrium.index()]);
        t += 7.0;
    }
    sim.run_to(26_000.0);
    let beats: Vec<_> = sim.history.beats().to_vec();
    let mut pairs = Vec::new();
    for (k, (b, _)) in beats.iter().enumerate() {
        if b.qrs_onset.0 < 3000.0 || pairs.len() >= 30 {
            continue;
        }
        if let (Some(ef), Some(rr)) = (beat_ef(&mut model, &sim, k), b.rr_ms) {
            pairs.push((rr, ef));
        }
    }
    let n = pairs.len() as f64;
    let mean = pairs.iter().map(|p| p.1).sum::<f64>() / n;
    let sd = (pairs.iter().map(|p| (p.1 - mean).powi(2)).sum::<f64>() / (n - 1.0)).sqrt();
    let short: Vec<(f64, f64)> = pairs.iter().copied().filter(|p| p.0 < 600.0).collect();
    let (mx, my) = (
        short.iter().map(|p| p.0).sum::<f64>() / short.len() as f64,
        short.iter().map(|p| p.1).sum::<f64>() / short.len() as f64,
    );
    let slope = short.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum::<f64>()
        / short.iter().map(|p| (p.0 - mx).powi(2)).sum::<f64>();
    let organised_p = sim
        .history
        .atrial_onsets_ms()
        .iter()
        .filter(|&&a| a > 3000.0)
        .count();
    eprintln!(
        "AF: {} beats, EF mean {:.3} CoV {:.3}; {} beats with RR < 600 ms, EF slope {:.2e} per ms; \
         max atrial contraction {max_atrial:.3}; organised atrial activations after 3 s: {organised_p}",
        pairs.len(),
        mean,
        sd / mean,
        short.len(),
        slope
    );
    assert!(pairs.len() >= 30, "30 beats");
    assert!(sd / mean > 0.0, "EF varies beat to beat");
    assert!(
        short.len() >= 3 && slope > 0.0,
        "EF rises with the preceding RR below 600 ms"
    );
    assert_eq!(
        organised_p, 0,
        "no organised atrial activation, so no atrial push of the filling"
    );
    assert!(
        f64::from(max_atrial) <= MotionParams::default().atrial_quiver_fraction + 1e-6,
        "no atrial contraction beyond the designed quiver"
    );
}

#[test]
fn complete_block_atria_contract_after_the_p_waves() {
    let mut sim = Sim::from_scenario("avb_3rd_junctional");
    let mut model = model();
    let mut f = model.new_frame();
    let mut series = Vec::new();
    let mut t = 3000.0;
    while t < 11_000.0 {
        sim.run_to(t + 200.0);
        eval_still(&mut model, &sim, t, &mut f);
        let a = f.readout.chamber_activity;
        series.push((t, f32::midpoint(a[2], a[3])));
        t += 4.0;
    }
    sim.run_to(12_000.0);
    let qrs: Vec<f64> = sim.history.beats().iter().map(|(b, _)| b.qrs_onset.0).collect();
    let mut lags = Vec::new();
    let mut after_qrs = Vec::new();
    for &p in sim
        .history
        .atrial_onsets_ms()
        .iter()
        .filter(|&&p| p > 3000.0 && p < 10_500.0)
    {
        let (tp, peak) = series
            .iter()
            .filter(|(t, _)| *t >= p && *t <= p + 400.0)
            .fold((0.0, 0.0f32), |acc, x| if x.1 > acc.1 { (x.0, x.1) } else { acc });
        if peak < 0.2 {
            continue;
        }
        lags.push(tp - p);
        if let Some(q) = qrs.iter().rev().find(|&&q| q <= tp) {
            after_qrs.push(tp - q);
        }
    }
    let sd = |x: &[f64]| {
        let m = x.iter().sum::<f64>() / x.len() as f64;
        (x.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (x.len() as f64 - 1.0)).sqrt()
    };
    let n_p = sim
        .history
        .atrial_onsets_ms()
        .iter()
        .filter(|&&p| p > 3000.0 && p < 10_500.0)
        .count();
    eprintln!("complete block: {n_p} P waves in the window");
    eprintln!(
        "complete block: {} P waves with an atrial contraction; P-to-peak {:.0}–{:.0} ms (SD {:.1}); QRS-to-peak SD {:.0} ms",
        lags.len(),
        lags.iter().copied().fold(f64::INFINITY, f64::min),
        lags.iter().copied().fold(0.0, f64::max),
        sd(&lags),
        sd(&after_qrs)
    );
    assert!(
        lags.len() >= 8 && lags.len() == n_p,
        "every P wave contracts the atria"
    );
    assert!(sd(&lags) < 10.0, "the contraction is locked to the P wave");
    assert!(sd(&after_qrs) > 50.0, "and not to the QRS");
}

fn volume_spread(sim: &Sim, from: f64, to: f64) -> (f64, usize) {
    let mut model = model();
    let mut f = model.new_frame();
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    let mut faults = 0;
    let mut t = from;
    while t < to {
        eval_still(&mut model, sim, t, &mut f);
        let v = lv_volume(&model, &f);
        lo = lo.min(v);
        hi = hi.max(v);
        let (i, b, _) = mesh_faults(&model, &f);
        faults += i + b;
        t += 10.0;
    }
    ((hi - lo) / hi, faults)
}

#[test]
fn vf_and_asystole_hold_the_cavity_volume() {
    let mut sim = Sim::from_scenario("vf_coarse");
    sim.run_to(7000.0);
    let (vf, vf_faults) = volume_spread(&sim, 3500.0, 7000.0);
    // asystole: no event at all
    let quiet = Sim::sinus(60.0);
    let (asys, asys_faults) = volume_spread(&quiet, 0.0, 3000.0);
    eprintln!(
        "VF: LV volume change {:.3} % (faults {vf_faults}); asystole {:.3} %",
        vf * 100.0,
        asys * 100.0
    );
    assert!(vf <= 0.02 && asys <= 0.02, "no cavity-volume change above 2 %");
    assert_eq!(vf_faults + asys_faults, 0);
}
