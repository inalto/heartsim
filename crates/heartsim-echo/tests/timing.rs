//! Brief B30 acceptance: the LV volume of the deformed mesh against the valve schedule — ejection
//! starts within 5 ms of M1 + IVCT and stops within 5 ms of A2, the volume is constant (±1 %) in
//! both isovolumic phases, and filling shows E, diastasis and A at 60 bpm and fused E/A at 120 bpm.

mod common;
use common::*;
use heartsim_echo::MotionParams;

fn volume_series(
    model: &mut heartsim_echo::MotionModel,
    sim: &Sim,
    from: f64,
    to: f64,
    step: f64,
) -> Vec<(f64, f64)> {
    let mut f = model.new_frame();
    let mut out = Vec::new();
    let mut t = from;
    while t <= to + 1e-9 {
        eval_still(model, sim, t, &mut f);
        out.push((t, lv_volume(model, &f)));
        t += step;
    }
    out
}

#[test]
fn ejection_follows_the_valve_schedule() {
    let mut model = model();
    let p = MotionParams::default();
    let mut sim = Sim::sinus(60.0);
    sim.run_to(5000.0);
    let w = beat_after(&sim, 2500.0);
    let a2 = w.a2.unwrap();
    let ao = w.ao;
    let mo = a2 + p.lv_ivrt_ms;
    let v = volume_series(&mut model, &sim, w.m1, mo, 1.0);
    let edv = v[0].1;
    let esv = v.iter().map(|x| x.1).fold(f64::INFINITY, f64::min);
    let sv = edv - esv;
    // falling: the volume drops by more than 5 µL in the next millisecond
    let falling: Vec<f64> = v
        .windows(2)
        .filter(|x| x[0].1 - x[1].1 > 0.005)
        .map(|x| x[0].0)
        .collect();
    let start = falling[0];
    let stop = falling[falling.len() - 1] + 1.0;
    eprintln!("EDV {edv:.1} mL ESV {esv:.1} mL SV {sv:.1} mL");
    let ivc: Vec<f64> = v.iter().filter(|(t, _)| *t <= ao).map(|x| x.1).collect();
    let ivr: Vec<f64> = v.iter().filter(|(t, _)| *t >= a2).map(|x| x.1).collect();
    let spread = |x: &[f64]| {
        let (lo, hi) = x.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), y| {
            (l.min(*y), h.max(*y))
        });
        (hi - lo) / lo
    };
    eprintln!(
        "M1 {:.1}  AO {:.1}  fall starts {:.0} ({:+.1} ms)  A2 {:.1}  fall stops {:.0} ({:+.1} ms)  IVC spread {:.3} %  IVR spread {:.3} %",
        w.m1,
        ao,
        start,
        start - ao,
        a2,
        stop,
        stop - a2,
        spread(&ivc) * 100.0,
        spread(&ivr) * 100.0
    );
    assert!(
        (start - ao).abs() <= 5.0,
        "ejection starts at {start}, valve opens at {ao}"
    );
    assert!((stop - a2).abs() <= 5.0, "ejection stops at {stop}, A2 at {a2}");
    assert!(
        spread(&ivc) <= 0.01 && spread(&ivr) <= 0.01,
        "isovolumic phases hold the volume"
    );
}

/// Peaks of the filling rate between mitral opening and the next M1: `(time, rate)` of the local
/// maxima, and the lowest rate between the first and the last.
fn filling_peaks(bpm: f64) -> (Vec<(f64, f64)>, f64) {
    let mut model = model();
    let p = MotionParams::default();
    let mut sim = Sim::sinus(bpm);
    sim.run_to(6000.0);
    let w = beat_after(&sim, 3000.0);
    let mo = w.a2.unwrap() + p.lv_ivrt_ms;
    let next = beat_after(&sim, w.qrs + 1.0);
    let v = volume_series(&mut model, &sim, mo, next.m1, 2.0);
    let rate: Vec<(f64, f64)> = v
        .windows(2)
        .map(|x| (x[1].0, (x[1].1 - x[0].1) / (x[1].0 - x[0].0)))
        .collect();
    let max = rate.iter().map(|r| r.1).fold(0.0, f64::max);
    let mut peaks = Vec::new();
    for k in 1..rate.len() - 1 {
        if rate[k].1 > rate[k - 1].1 && rate[k].1 >= rate[k + 1].1 && rate[k].1 > 0.1 * max {
            peaks.push(rate[k]);
        }
    }
    let valley = match (peaks.first(), peaks.last()) {
        (Some(a), Some(b)) if b.0 > a.0 => rate
            .iter()
            .filter(|r| r.0 > a.0 && r.0 < b.0)
            .map(|r| r.1)
            .fold(f64::INFINITY, f64::min),
        _ => f64::NAN,
    };
    eprintln!("{bpm} bpm filling: peaks {peaks:.3?} mL/ms, lowest rate between {valley:.4} mL/ms");
    (peaks, valley)
}

#[test]
fn filling_has_e_diastasis_and_a_at_60_and_fuses_at_120() {
    let (p60, valley60) = filling_peaks(60.0);
    assert_eq!(p60.len(), 2, "E and A at 60 bpm");
    let smaller = p60[0].1.min(p60[1].1);
    assert!(
        valley60 < 0.25 * p60[0].1,
        "diastasis: the rate falls below a quarter of E between E and A"
    );
    assert!(p60[1].0 - p60[0].0 > 250.0 && smaller > 0.0);
    let (p120, valley120) = filling_peaks(120.0);
    let fused = p120.len() == 1 || valley120 > 0.5 * p120[0].1.min(p120[p120.len() - 1].1);
    assert!(fused, "E and A fuse at 120 bpm");
}
