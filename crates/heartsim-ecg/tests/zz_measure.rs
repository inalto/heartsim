#![allow(unused)]
use heartsim_core::{Action, Electrode, FrontalAxis, LEAD_COUNT, Lead, Millis, Parameters, TwelveLead};
use std::sync::Arc;
use heartsim_ecg::{Analyzer, ForwardModel, ForwardOptions, GaussianSynth, SynthModulation};
use heartsim_geometry::AnatomyPack;
use heartsim_sim::tier1::{CellKind, EikonalEngine, EikonalParams, Tier1Engine};
use heartsim_sim::timing::TimingParams;
use heartsim_sim::{Engine, TimingEngine};
use std::ops::Range;
use std::path::Path;
use std::sync::OnceLock;

// --- fixtures ----------------------------------------------------------------------------------

fn pack() -> &'static AnatomyPack {
    static PACK: OnceLock<AnatomyPack> = OnceLock::new();
    PACK.get_or_init(|| {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/anatomy/default");
        AnatomyPack::load(&dir).expect("default pack loads")
    })
}

fn model() -> &'static ForwardModel {
    static MODEL: OnceLock<ForwardModel> = OnceLock::new();
    MODEL.get_or_init(|| {
        ForwardModel::build(&pack().cells, &pack().electrodes, ForwardOptions::default()).expect("builds")
    })
}

fn engine() -> EikonalEngine {
    static ENGINE: OnceLock<EikonalEngine> = OnceLock::new();
    ENGINE
        .get_or_init(|| {
            let p = pack();
            let mut e = EikonalEngine::with_seed_sets(
                std::sync::Arc::new(p.cells.clone()),
                &Parameters::default(),
                EikonalParams::default(),
                &p.landmarks.sets,
            );
            e.set_aha_segments(&p.aha_segments);
            e
        })
        .clone()
}

/// One sinus beat of the coupled Tier-1 engine with `actions` applied at t = 0, through the
/// forward model at 1 kHz (P ≈ 200 ms, QRS ≈ 355 ms).
struct Recording {
    leads: Vec<TwelveLead>,
    /// Ventricular activation span of the beat, sample indices.
    qrs: Range<usize>,
}

impl Recording {
    fn trace(&self, lead: Lead) -> Vec<f64> {
        self.leads.iter().map(|s| s.get(lead)).collect()
    }

    /// ST deviation against the TQ baseline before the P wave, mV, 60 ms after the J point of
    /// the healthy beat (the same instant in every recording: slow-conducting injured cells
    /// prolong the activation tail, which would otherwise move the measurement into the T).
    fn st(&self, lead: Lead) -> f64 {
        let x = self.trace(lead);
        x[normal().qrs.end + 60] - x[150]
    }
}

fn record(actions: &[Action], ms: usize) -> Recording {
    let mut t1 = Tier1Engine::new(TimingParams::default(), engine());
    for a in actions {
        t1.apply(a, Millis::ZERO).expect("action accepted");
    }
    // a healed infarct is silent tissue: build the forward model with its core as scar
    let scarred = t1.eikonal().cloud_with_scar().map(|cloud| {
        ForwardModel::build(&cloud, &pack().electrodes, ForwardOptions::default()).expect("builds")
    });
    let m = scarred.as_ref().unwrap_or_else(|| model());
    let mut leads = Vec::with_capacity(ms);
    for _ in 0..ms {
        t1.step(Millis(1.0));
        leads.push(m.sample(&t1.cells().vm).to_leads());
    }
    let e = t1.eikonal();
    let (lo, hi) = (0..e.cloud().len())
        .filter(|&i| e.kinds()[i] == CellKind::Ventricular)
        .map(|i| e.activation_times()[i])
        .filter(|t| t.is_finite() && *t < 900.0)
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), t| {
            (lo.min(t), hi.max(t))
        });
    // sample k is the state at t = k + 1 ms
    Recording {
        leads,
        qrs: lo as usize - 1..hi as usize,
    }
}


fn normal() -> &'static Recording {
    static REC: OnceLock<Recording> = OnceLock::new();
    REC.get_or_init(|| record(&[], 900))
}
fn timed(timing: TimingParams) -> Vec<TwelveLead> {
    let p = pack();
    let eik = EikonalEngine::with_seed_sets(Arc::new(p.cells.clone()), &Parameters::default(), EikonalParams::default(), &p.landmarks.sets);
    let mut t1 = Tier1Engine::new(timing, eik);
    let m = ForwardModel::build(&p.cells, &p.electrodes, ForwardOptions::default()).unwrap();
    (0..1000).map(|_| { t1.step(Millis(1.0)); m.sample(&t1.cells().vm).to_leads() }).collect()
}
fn report(name: &str, l: &[TwelveLead]) {
    let w = 350..480usize;
    let tr = |x: Lead| l[w.clone()].iter().map(|s| s.get(x)).collect::<Vec<f64>>();
    let net = |x: Lead| tr(x).iter().sum::<f64>();
    let axis = FrontalAxis::from_i_avf(net(Lead::I), net(Lead::AVF)).degrees;
    print!("{name:8} axis(350-480) {axis:6.1} ");
    for x in [Lead::I, Lead::II, Lead::III, Lead::AVR, Lead::AVL, Lead::AVF, Lead::V1, Lead::V2, Lead::V3, Lead::V4, Lead::V5, Lead::V6] {
        let t = tr(x);
        print!("{} {:+.2}/{:+.2} ", x.label(), t.iter().copied().fold(f64::NEG_INFINITY, f64::max), t.iter().copied().fold(f64::INFINITY, f64::min));
    }
    println!();
}
#[test]
fn zz_report() {
    {
        let n = record(&[], 900);
        let (i, avf, ii) = (n.trace(Lead::I), n.trace(Lead::AVF), n.trace(Lead::II));
        println!("qrs {:?}", n.qrs);
        for k in (195..360).step_by(5) { println!("t{k} I {:+.3} II {:+.3} aVF {:+.3} III {:+.3} V1 {:+.3}", i[k], ii[k], avf[k], n.trace(Lead::III)[k], n.trace(Lead::V1)[k]); }
        for (a,b) in [(195,270),(195,300),(195,330)] {
            let s = |x:&[f64]| (a..b).map(|k| x[k]-x[150]).sum::<f64>();
            println!("P axis {a}-{b}: {:.1}", FrontalAxis::from_i_avf(s(&i), s(&avf)).degrees);
        }
    }
    report("sinus", &timed(TimingParams::default()));
    let mut t = TimingParams::default(); t.his_purkinje.laf.blocked = true; report("LAFB", &timed(t));
    let mut t = TimingParams::default(); t.his_purkinje.lpf.blocked = true; report("LPFB", &timed(t));
    let mut t = TimingParams::default(); t.his_purkinje.lbb.blocked = true; report("LBBB", &timed(t));
    let mut t = TimingParams::default(); t.his_purkinje.rbb.blocked = true; report("RBBB", &timed(t));
    let n = record(&[], 900);
    for (name, a) in [("LAD90", stenosis("LAD", 0.9)), ("RCA90", stenosis("RCA", 0.9))] {
        let r = record(&[a], 900);
        print!("{name} ST shift: ");
        for x in Lead::ALL { print!("{} {:+.2} ", x.label(), r.st(x) - n.st(x)); }
        println!();
    }
    let p = pack();
    let dist = |v: Electrode| { let q = p.electrodes.standard(v).unwrap(); p.cells.positions[p.cells.nearest(q).unwrap() as usize].distance(q) };
    for v in [Electrode::V1, Electrode::V2, Electrode::V3, Electrode::V4, Electrode::V5, Electrode::V6] { print!("{:?} {:.0} mm ", v, dist(v)); }
    println!();
}
fn stenosis(branch: &str, severity: f64) -> Action { Action::Stenosis { branch: branch.into(), severity } }
