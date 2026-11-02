//! Acceptance tests for brief B13 (pacing devices), Tier 1: the paced morphology follows from the
//! lead site on the idealised heart. VVI at 70 ppm from the RV apex gives a wide (≥ 140 ms),
//! LBBB-like QRS with a superior axis; a His-bundle lead gives a narrower one. Also: the
//! Tier-1 lead probes measure a healthy electrogram, and pacing spikes of non-captured paces
//! reach the Tier-0 strip.

use heartsim_core::{FrontalAxis, Lead, Millis, Parameters, TwelveLead};
use heartsim_ecg::{ForwardModel, ForwardOptions, GaussianSynth};
use heartsim_geometry::AnatomyPack;
use heartsim_sim::tier1::{CellKind, EikonalEngine, EikonalParams, Tier1Engine};
use heartsim_sim::timing::{
    LeadParams, LeadSite, MarkerKind, PacerChannel, PacerConfig, PacerMode, PacerPreset, TimingParams,
};
use heartsim_sim::{Engine, Origin, TimingEngine};
use std::ops::Range;
use std::path::Path;
use std::sync::{Arc, OnceLock};

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

fn engine(timing: TimingParams) -> Tier1Engine {
    let p = pack();
    let eik = EikonalEngine::with_seed_sets(
        Arc::new(p.cells.clone()),
        &Parameters::default(),
        EikonalParams::default(),
        &p.landmarks.sets,
    );
    Tier1Engine::new(timing, eik)
}

/// A paced beat recorded at 1 kHz: the 12 leads and the QRS window of the last paced beat.
struct PacedBeat {
    leads: Vec<TwelveLead>,
    /// Ventricular activation span of the analysed beat (sample indices).
    qrs: Range<usize>,
    t1: Tier1Engine,
}

impl PacedBeat {
    fn trace(&self, lead: Lead) -> Vec<f64> {
        self.leads.iter().map(|s| s.get(lead)).collect()
    }

    fn window(&self) -> Range<usize> {
        self.qrs.start.saturating_sub(2)..(self.qrs.end + 3).min(self.leads.len())
    }

    /// Net QRS area of `lead` (mV·ms).
    fn net(&self, lead: Lead) -> f64 {
        self.trace(lead)[self.window()].iter().sum()
    }

    fn axis(&self) -> f64 {
        FrontalAxis::from_i_avf(self.net(Lead::I), self.net(Lead::AVF)).degrees
    }

    /// Span over which the 12-lead spatial velocity exceeds 2 % of its peak, ms (as B5).
    fn qrs_ms(&self) -> f64 {
        let w = self.qrs.start.saturating_sub(20)..(self.qrs.end + 40).min(self.leads.len());
        let vel: Vec<f64> = (w.start + 1..w.end)
            .map(|i| {
                Lead::ALL
                    .iter()
                    .map(|l| (self.leads[i].get(*l) - self.leads[i - 1].get(*l)).powi(2))
                    .sum::<f64>()
                    .sqrt()
            })
            .collect();
        let peak = vel.iter().copied().fold(0.0, f64::max);
        let above: Vec<usize> = (0..vel.len()).filter(|&i| vel[i] > 0.02 * peak).collect();
        (above.last().unwrap() - above.first().unwrap()) as f64
    }
}

/// VVI at 70 ppm in complete AV block with the ventricular lead at `site`; records 2.2 s and
/// analyses the second paced beat (the first follows the start-up transient).
fn vvi70(site: LeadSite) -> PacedBeat {
    let mut timing = TimingParams::default();
    timing.av.complete_block = true;
    timing.escape.junctional_bpm = 0.0;
    timing.escape.ventricular_bpm = 0.0;
    let mut cfg = PacerConfig::new(PacerMode::Vvi, 70.0);
    cfg.ventricular_lead = LeadParams::at(site);
    timing.pacer = Some(cfg);
    record(timing, 2200)
}

fn record(timing: TimingParams, ms: usize) -> PacedBeat {
    let mut t1 = engine(timing);
    let mut leads = Vec::with_capacity(ms);
    let mut paces = Vec::new();
    for _ in 0..ms {
        t1.step(Millis(1.0));
        paces.extend(
            t1.timing_mut()
                .drain_pacer_markers()
                .into_iter()
                .filter(|m| m.channel == PacerChannel::Ventricular && m.is_pace())
                .map(|m| m.at.0),
        );
        leads.push(model().sample(&t1.cells().vm).to_leads());
    }
    let start = *paces.get(1).expect("two ventricular paces");
    let e = t1.eikonal();
    let (lo, hi) = (0..e.cloud().len())
        .filter(|&i| e.kinds()[i] == CellKind::Ventricular)
        .map(|i| e.activation_times()[i])
        .filter(|t| t.is_finite() && *t >= start - 1.0)
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), t| {
            (lo.min(t), hi.max(t))
        });
    PacedBeat {
        leads,
        // sample k is the state at t = k + 1 ms
        qrs: lo as usize - 1..hi as usize,
        t1,
    }
}

fn rv_apex() -> &'static PacedBeat {
    static B: OnceLock<PacedBeat> = OnceLock::new();
    B.get_or_init(|| vvi70(LeadSite::RvApex))
}

#[test]
fn rv_apex_vvi70_gives_wide_lbbb_like_superior_axis_qrs() {
    let b = rv_apex();
    let e = b.t1.eikonal();
    let paced = (0..e.cloud().len())
        .filter(|&i| e.kinds()[i] == CellKind::Ventricular)
        .filter(|&i| e.activation_origin()[i] == Origin::Pacer(1))
        .count();
    let vent = (0..e.cloud().len())
        .filter(|&i| e.kinds()[i] == CellKind::Ventricular)
        .count();
    assert!(paced > vent * 9 / 10, "the paced front captures the ventricles");
    let apex = e.cloud().landmarks["rv_apex"] as usize;
    assert!(
        (e.activation_times()[apex] - (b.qrs.start + 1) as f64).abs() <= 1.0,
        "earliest activation at the lead tip"
    );

    let qrs = b.qrs_ms();
    assert!(qrs >= 140.0, "paced QRS {qrs:.0} ms");
    // LBBB-like: dominant negative V1 (QS / rS), dominant positive I and V6
    assert!(b.net(Lead::V1) < 0.0, "V1 net {:.1}", b.net(Lead::V1));
    assert!(
        b.net(Lead::V6) > 0.0 || b.net(Lead::I) > 0.0,
        "left-sided leads positive"
    );
    // superior axis: activation spreads from the apex upwards, aVF (and II, III) negative
    let axis = b.axis();
    assert!(b.net(Lead::AVF) < 0.0, "aVF net {:.1}", b.net(Lead::AVF));
    assert!((-180.0..-0.0).contains(&axis), "superior axis, got {axis:.0}°");
}

#[test]
fn his_bundle_pacing_is_narrower_than_rv_apex() {
    let his = vvi70(LeadSite::HisBundle);
    let (h, r) = (his.qrs_ms(), rv_apex().qrs_ms());
    assert!(h + 20.0 < r, "His-paced QRS {h:.0} ms vs RV apex {r:.0} ms");
}

#[test]
fn lead_probe_measures_a_healthy_electrogram() {
    let b = rv_apex();
    let amps = b.t1.lead_amplitudes_mv();
    let v = amps[PacerChannel::Ventricular as usize].expect("measured after a beat");
    let nominal = LeadSite::RvApex.typical_intrinsic_mv();
    assert!(
        v > 0.8 * nominal && v <= nominal + 1e-9,
        "RV apex electrogram {v:.1} mV"
    );
}

#[test]
fn ablated_tissue_around_the_lead_lowers_the_electrogram_and_undersenses() {
    // Sinus rhythm with a VVI device: a lesion around the RV-apex lead tip shrinks the measured
    // R wave below the 2.5 mV sensitivity, so the device stops seeing the native beats.
    let mut timing = TimingParams::default();
    timing.sinus.rate_bpm = 75.0;
    timing.pacer = Some(PacerConfig::new(PacerMode::Vvi, 50.0));
    let mut t1 = engine(timing);
    let apex = t1.eikonal().cloud().landmarks["rv_apex"];
    t1.apply(
        &heartsim_core::Action::Ablate {
            site: heartsim_core::StimSite::Cell(apex),
            radius_mm: 4.0,
        },
        Millis::ZERO,
    )
    .expect("ablation is spatial");
    let mut paces = 0;
    for _ in 0..4000 {
        t1.step(Millis(1.0));
        paces += t1
            .timing_mut()
            .drain_pacer_markers()
            .iter()
            .filter(|m| m.is_pace())
            .count();
    }
    let v = t1.lead_amplitudes_mv()[PacerChannel::Ventricular as usize].unwrap_or(0.0);
    assert!(v < 2.5, "electrogram over scar {v:.2} mV");
    assert!(paces >= 1, "undersensing: the device paces despite sinus at 75");
}

#[test]
fn biventricular_pacing_narrows_the_paced_qrs() {
    let mut timing = TimingParams::default();
    timing.av.complete_block = true;
    timing.escape.junctional_bpm = 0.0;
    timing.escape.ventricular_bpm = 0.0;
    let mut cfg = PacerConfig::preset(PacerPreset::Biv);
    cfg.mode = PacerMode::Vvi;
    cfg.lower_rate_bpm = 70.0;
    cfg.vv_delay_ms = 0.0;
    timing.pacer = Some(cfg);
    let biv = record(timing, 2200);
    let (b, r) = (biv.qrs_ms(), rv_apex().qrs_ms());
    assert!(b < r, "BiV QRS {b:.0} ms vs RV apex {r:.0} ms");
}

// --- Tier 0 -----------------------------------------------------------------------------------------

#[test]
fn non_captured_paces_still_draw_a_spike() {
    let mut timing = TimingParams::default();
    timing.av.complete_block = true;
    timing.escape.junctional_bpm = 0.0;
    timing.escape.ventricular_bpm = 0.0;
    let mut cfg = PacerConfig::new(PacerMode::Vvi, 60.0);
    cfg.ventricular_output_ma = 0.2; // below the 0.8 mA threshold
    timing.pacer = Some(cfg);
    let mut eng = TimingEngine::new(timing);
    let mut synth = GaussianSynth::textbook();
    let mut misses = Vec::new();
    let mut trace = Vec::new();
    for _ in 0..3000 {
        eng.step(Millis(1.0));
        synth.on_events(&eng.drain_events());
        let m = eng.drain_pacer_markers();
        misses.extend(
            m.iter()
                .filter(|m| m.kind == MarkerKind::Pace { captured: false })
                .map(|m| m.at.0),
        );
        synth.on_pacer_markers(&m);
        trace.push(synth.sample(eng.now()));
    }
    misses.retain(|t| *t + 50.0 < trace.len() as f64);
    assert!(misses.len() >= 2);
    for t in misses {
        let k = t as usize - 1; // sample k is t = k + 1 ms
        let peak = Lead::ALL
            .iter()
            .map(|l| trace[k].get(*l).abs())
            .fold(0.0, f64::max);
        assert!(peak > 1.0, "spike at {t} ms: {peak:.2} mV");
        // and no QRS follows it
        let after = Lead::ALL
            .iter()
            .map(|l| trace[k + 40].get(*l).abs())
            .fold(0.0, f64::max);
        assert!(after < 0.2, "no capture, flat 40 ms later ({after:.2} mV)");
    }
}
