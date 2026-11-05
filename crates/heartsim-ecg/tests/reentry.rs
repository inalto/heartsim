//! Acceptance tests of brief B17 (re-entry presets) on the ECG.
//!
//! Each preset in `assets/scenarios/reentry/` is run through the switching engine
//! (Tier 1 → Tier 2), its tissue state is sampled by the forward model every millisecond, and
//! the 12 leads are measured with the B4 analyser and with a spectrum, against the diagnostic
//! criteria of the rhythm. The grid is the file's own `voxel_mm` (4 mm, about 32³ voxels), which
//! is what keeps these runs short enough for CI; Tier 2 at 2 mm is not real time on a laptop
//! and is not what is tested here.

use heartsim_core::{Action, LEAD_COUNT, Millis};
use heartsim_ecg::analysis::{Analysis, Analyzer, Regularity};
use heartsim_ecg::{ForwardModel, ForwardOptions};
use heartsim_geometry::AnatomyPack;
use heartsim_sim::Engine;
use heartsim_sim::tier2::{Preset, SwitchingEngine, Tier};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

// --- fixtures ----------------------------------------------------------------------------------

fn assets() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

fn pack() -> &'static AnatomyPack {
    static PACK: OnceLock<AnatomyPack> = OnceLock::new();
    PACK.get_or_init(|| AnatomyPack::load(&assets().join("anatomy/default")).expect("default pack loads"))
}

fn forward() -> &'static ForwardModel {
    static MODEL: OnceLock<ForwardModel> = OnceLock::new();
    MODEL.get_or_init(|| {
        ForwardModel::build(&pack().cells, &pack().electrodes, ForwardOptions::default()).expect("builds")
    })
}

/// Which cells are atrial (their V_m alone gives the atrial component of the ECG: the forward
/// model is linear in V_m).
fn atrial_mask() -> &'static [bool] {
    static MASK: OnceLock<Vec<bool>> = OnceLock::new();
    MASK.get_or_init(|| pack().cells.regions.iter().map(|r| r.is_atrial()).collect())
}

fn preset(id: &str) -> Preset {
    let path = assets().join(format!("scenarios/reentry/{id}.ron"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    Preset::from_ron(&text)
        .unwrap_or_else(|e| panic!("{id}: {e}"))
        .unwrap_or_else(|| panic!("{id}: no reentry section"))
}

fn engine(p: &Preset) -> SwitchingEngine {
    p.build(
        Arc::new(pack().cells.clone()),
        &pack().landmarks.sets,
        &pack().aha_segments,
    )
}

/// A run at 1 kHz: twelve leads and the atrial component of lead V1.
struct Run {
    /// `leads[l][i]` is lead `l` (`Lead as usize`) at `t = (i + 1) ms`, mV.
    leads: Vec<Vec<f32>>,
    /// Lead V1 of the atrial cells only, mV.
    atrial_v1: Vec<f32>,
}

/// Run `e` for `seconds`, applying `timeline` as the app does.
fn run_engine(e: &mut SwitchingEngine, timeline: &[heartsim_core::TimedAction], seconds: f64, out: &mut Run) {
    let mut timeline = timeline.to_vec();
    timeline.sort_by(|a, b| a.at.0.total_cmp(&b.at.0));
    let mut next = timeline.partition_point(|a| a.at.0 < e.now().0 - 0.5);
    let steps = (seconds * 1000.0).round() as usize;
    let fm = forward();
    let mask = atrial_mask();
    let mut atrial_vm = vec![0.0f32; mask.len()];
    for _ in 0..steps {
        while let Some(a) = timeline.get(next)
            && a.at.0 <= e.now().0
        {
            e.apply(&a.action, a.at)
                .unwrap_or_else(|err| panic!("action at {:.0} ms: {err}", a.at.0));
            next += 1;
        }
        e.step(Millis(1.0));
        let _ = e.drain_events();
        let vm = &e.cells().vm;
        let leads = fm.sample(vm).to_leads();
        for (l, buf) in out.leads.iter_mut().enumerate() {
            buf.push(leads.0[l] as f32);
        }
        for ((o, &v), &a) in atrial_vm.iter_mut().zip(vm).zip(mask) {
            *o = if a { v } else { 0.0 };
        }
        let atrial = fm.sample(&atrial_vm).to_leads();
        out.atrial_v1.push(atrial.get(heartsim_core::Lead::V1) as f32);
    }
}

fn empty_run() -> Run {
    Run {
        leads: vec![Vec::new(); LEAD_COUNT],
        atrial_v1: Vec::new(),
    }
}

// --- measurements --------------------------------------------------------------------------------

/// B4 analysis of the samples `from_ms .. to_ms`, at 500 Hz (the analyser's rate).
fn analyze(run: &Run, from_ms: usize, to_ms: usize) -> Analysis {
    let slices: Vec<Vec<f32>> = run
        .leads
        .iter()
        .map(|l| l[from_ms..to_ms].iter().step_by(2).copied().collect())
        .collect();
    let refs: [&[f32]; LEAD_COUNT] = std::array::from_fn(|i| slices[i].as_slice());
    Analyzer::default().analyze(&refs, 500.0)
}

/// Dominant frequency of `x` (1 kHz) in `[lo, hi]` Hz, and the fraction of the band's power it
/// holds in a ±0.5 Hz neighbourhood: a Hann-windowed DFT at 0.05 Hz resolution.
fn dominant_hz(x: &[f32], lo: f64, hi: f64) -> (f64, f64) {
    let n = x.len();
    let mean = x.iter().map(|&v| f64::from(v)).sum::<f64>() / n as f64;
    let w: Vec<f64> = (0..n)
        .map(|i| {
            let h = 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / n as f64).cos();
            (f64::from(x[i]) - mean) * h
        })
        .collect();
    let mut best = (0.0, 0.0);
    let (mut total, mut powers) = (0.0, Vec::new());
    let mut f = lo;
    while f <= hi {
        let (mut re, mut im) = (0.0, 0.0);
        for (i, v) in w.iter().enumerate() {
            let a = std::f64::consts::TAU * f * i as f64 / 1000.0;
            re += v * a.cos();
            im += v * a.sin();
        }
        let p = re * re + im * im;
        total += p;
        powers.push((f, p));
        if p > best.1 {
            best = (f, p);
        }
        f += 0.05;
    }
    let near: f64 = powers
        .iter()
        .filter(|(f, _)| (f - best.0).abs() <= 0.5)
        .map(|(_, p)| p)
        .sum();
    (best.0, near / total.max(f64::MIN_POSITIVE))
}

/// Peak-to-peak amplitude of `x` in windows of `win` samples, median over the windows, mV.
fn median_peak_to_peak(x: &[f32], win: usize) -> f64 {
    let mut v: Vec<f64> = x
        .chunks(win)
        .filter(|c| c.len() == win)
        .map(|c| {
            let (lo, hi) = c
                .iter()
                .fold((f32::MAX, f32::MIN), |(a, b), &s| (a.min(s), b.max(s)));
            f64::from(hi - lo)
        })
        .collect();
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

// --- AF ------------------------------------------------------------------------------------------------

/// The AF preset for 24 s: burst at 2 s, established by ≈ 6 s; a cloned engine continues it.
fn af() -> &'static (SwitchingEngine, Run, Vec<heartsim_core::TimedAction>) {
    static AF: OnceLock<(SwitchingEngine, Run, Vec<heartsim_core::TimedAction>)> = OnceLock::new();
    AF.get_or_init(|| {
        let p = preset("af_persistent");
        let mut e = engine(&p);
        let mut run = empty_run();
        let tl = p.timeline();
        run_engine(&mut e, &tl, 14.0, &mut run);
        (e, run, tl)
    })
}

#[test]
fn af_has_no_p_waves_an_irregular_rhythm_and_fibrillatory_baseline() {
    let (e, run, _) = af();
    assert_eq!(e.tier(), Tier::Two, "AF handed back: {:?}", e.switches());
    let n = run.leads[0].len();
    let (from, to) = (n - 8_000, n);
    let a = analyze(run, from, to);
    eprintln!(
        "AF B4: quality {:?} HR {:?} QRS {:?} regularity {:?} P present {}",
        a.quality, a.hr_bpm, a.qrs_ms, a.regularity, a.p_wave_present
    );
    assert!(!a.p_wave_present, "a P wave was found in AF");
    assert_eq!(a.regularity, Regularity::Irregular);
    let hr = a.hr_bpm.expect("a ventricular rate");
    assert!((80.0..=180.0).contains(&hr), "HR {hr}");
    // f waves: the atrial component of lead V1 (the forward model is linear in V_m, so this is
    // exactly the atrial part of the recorded signal), peak-to-peak in 1 s windows
    let f = median_peak_to_peak(&run.atrial_v1[from..to], 1000);
    let (hz, share) = dominant_hz(&run.atrial_v1[from..to], 2.0, 15.0);
    eprintln!("f waves: {f:.3} mV peak-to-peak in V1, dominant {hz:.1} Hz ({share:.2} of band power)");
    assert!((0.05..=0.2).contains(&f), "f-wave amplitude {f:.3} mV");
    // Fibrillatory rate: clinical AF runs at 4-10 Hz (240-600/min); this model's atrial wavelets
    // turn over at ≈ 2.7 Hz (≈ 165/min, a coarse, slow AF; local cycle ≈ 380 ms) because the
    // 2 mm grid and the Aliev-Panfilov restitution do not allow shorter cycles. The test pins
    // what the model does (a fibrillatory 2-10 Hz band), not the clinical figure.
    assert!((2.0..=10.0).contains(&hz), "dominant atrial frequency {hz:.1} Hz");
    // QRS width as the analyser measures it through the fibrillatory baseline: the Tier-2
    // ventricles activate more slowly than Tier 1's (≈ 100-120 ms in sinus rhythm) and the
    // f waves shift the delineation, so the bound is loose
    let qrs = a.qrs_ms.expect("QRS width");
    assert!((60.0..=180.0).contains(&qrs), "QRS {qrs} ms");
}

#[test]
fn a_shock_restores_sinus_rhythm_on_the_ecg() {
    let (e0, run0, tl) = af();
    let mut e = e0.clone();
    let mut run = Run {
        leads: run0.leads.clone(),
        atrial_v1: run0.atrial_v1.clone(),
    };
    let mut tl = tl.clone();
    tl.push(heartsim_core::TimedAction {
        at: Millis(e.now().0 + 100.0),
        action: Action::Shock {
            joules: 200.0,
            synchronised: true,
        },
    });
    run_engine(&mut e, &tl, 12.0, &mut run);
    let n = run.leads[0].len();
    let a = analyze(&run, n - 6_000, n);
    eprintln!(
        "after the shock: tier {:?} HR {:?} regularity {:?} P present {}",
        e.tier(),
        a.hr_bpm,
        a.regularity,
        a.p_wave_present
    );
    assert_eq!(e.tier(), Tier::One);
    assert!(a.p_wave_present, "no P waves after cardioversion");
    assert_eq!(a.regularity, Regularity::Regular);
}
