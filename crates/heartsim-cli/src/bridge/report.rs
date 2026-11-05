//! `heartsim-cli validation-report`: heartsim's own run of a scenario against an imported solver result on
//! the same cell cloud. Both V_m fields go through the same forward model, so the ECG comparison measures
//! the difference in activation and repolarisation, not a difference in forward models.

use super::import::read_import;
use super::metrics::{Agreement, agreement, cell, correlation};
use super::plots::{LeadTrace, activation_scatter, ecg_overlay};
use super::{eikonal_for, load_anatomy, load_scenario};
use anyhow::{Context, Result, ensure};
use heartsim_core::{Action, Lead, Millis};
use heartsim_ecg::{ForwardModel, ForwardOptions, WeightStats};
use heartsim_sim::tier1::CellKind;
use heartsim_sim::tier2::{Tier2Engine, Tier2Params};
use heartsim_sim::{Engine, Tier1Engine};
use serde::Serialize;
use std::fmt::Write as _;
use std::path::PathBuf;

/// QRS window margins around the ventricular activation span, ms (the forward model sees the front as it starts).
const QRS_MARGIN_MS: f64 = 3.0;
/// The T window starts this long after the last ventricular activation and spans at most `T_SPAN_MS`, ms.
const T_DELAY_MS: f64 = 60.0;
const T_SPAN_MS: f64 = 450.0;

/// Which heartsim engine to compare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Tier {
    #[value(name = "1")]
    One,
    #[value(name = "2")]
    Two,
}

/// Settings of `validation-report`.
#[derive(Debug)]
pub struct ReportArgs {
    pub scenario: String,
    pub imported: PathBuf,
    pub out: PathBuf,
    pub pack: Option<PathBuf>,
    pub spacing_mm: f32,
    pub tier: Tier,
    /// V_m at rest and at peak of the imported series; by default the first frame's median and the maximum.
    pub vm_rest: Option<f32>,
    pub vm_peak: Option<f32>,
}

enum Heartsim {
    One(Box<Tier1Engine>),
    Two(Box<Tier2Engine>),
}

impl Heartsim {
    fn engine(&mut self) -> &mut dyn Engine {
        match self {
            Self::One(e) => e.as_mut(),
            Self::Two(e) => e.as_mut(),
        }
    }

    fn apply(&mut self, action: &Action, at: Millis) -> Result<()> {
        match self {
            Self::One(e) => e.apply(action, at),
            Self::Two(e) => e.apply(action, at),
        }
        .map_err(|err| anyhow::anyhow!("timeline action at {} ms: {err:?}", at.0))
    }
}

/// Per-lead ECG agreement, mV and Pearson r.
#[derive(Debug, Clone, Serialize)]
pub struct LeadMetrics {
    pub lead: String,
    pub rmse_mv: f64,
    pub qrs_r: Option<f64>,
    pub t_r: Option<f64>,
}

/// `metrics.json`.
#[derive(Debug, Serialize)]
pub struct ReportMetrics {
    pub scenario: String,
    pub tier: String,
    pub activation_ms: Agreement,
    pub vm_rest: Option<f32>,
    pub vm_peak: Option<f32>,
    pub leads: Vec<LeadMetrics>,
    pub qrs_window_ms: Option<[f64; 2]>,
    pub t_window_ms: Option<[f64; 2]>,
    pub cells_compared: usize,
    pub weight_stats: Vec<WeightStats>,
}

fn window(times: &[f64], from: f64, to: f64) -> std::ops::Range<usize> {
    let start = times.partition_point(|t| *t < from);
    let end = times.partition_point(|t| *t <= to);
    start..end.max(start)
}

fn median(mut v: Vec<f32>) -> f32 {
    v.retain(|x| x.is_finite());
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f32::total_cmp);
    v[v.len() / 2]
}

/// Run `validation-report`.
pub fn run(a: &ReportArgs) -> Result<()> {
    let s = load_scenario(&a.scenario)?;
    let pack = load_anatomy(a.pack.as_deref(), a.spacing_mm)?;
    let imported = read_import(&a.imported)?;
    ensure!(
        imported.summary.cells == pack.cells.len(),
        "the import was mapped onto {} cells but this anatomy has {}: use the same --pack or --spacing-mm",
        imported.summary.cells,
        pack.cells.len()
    );
    let model = ForwardModel::build(&pack.cells, &pack.electrodes, ForwardOptions::default())
        .map_err(|e| anyhow::anyhow!("forward model: {e}"))?;
    let eik = eikonal_for(&pack, &s);
    let kinds = eik.kinds().to_vec();
    let mut engine = match a.tier {
        Tier::One => Heartsim::One(Box::new(Tier1Engine::new(s.timing.clone(), eik))),
        Tier::Two => Heartsim::Two(Box::new(Tier2Engine::new(
            s.timing.clone(),
            eik,
            &heartsim_sim::tier2::AlievPanfilov::default(),
            Tier2Params::default(),
        ))),
    };

    // when to sample: the imported frames, or a plain 1 kHz over the imported activation span
    let frame_times: Vec<f64> = imported
        .vm
        .as_ref()
        .map_or_else(Vec::new, |vm| (0..vm.frames).map(|k| vm.time_ms(k)).collect());
    let lat_end = imported
        .lat_ms
        .iter()
        .filter(|t| t.is_finite())
        .fold(0.0f64, |m, t| m.max(f64::from(*t)));
    let tend = frame_times.last().copied().unwrap_or(lat_end + 10.0);

    let mut timeline = s.scenario.timeline.clone();
    timeline.sort_by(|x, y| x.at.0.total_cmp(&y.at.0));
    let mut next_action = 0;
    let n = pack.cells.len();
    let mut first = vec![f32::NAN; n];
    let mut own: Vec<[f64; 12]> = Vec::with_capacity(frame_times.len());
    let mut next_frame = 0;
    let lead_values = |vm: &[f32]| {
        let l = model.sample(vm).to_leads();
        let mut out = [0.0; 12];
        for (slot, lead) in out.iter_mut().zip(Lead::ALL) {
            *slot = l.get(lead);
        }
        out
    };
    while frame_times.get(next_frame).is_some_and(|t| *t <= 0.0) {
        own.push(lead_values(&engine.engine().cells().vm));
        next_frame += 1;
    }
    while engine.engine().now().0 < tend {
        let now = engine.engine().now();
        while let Some(act) = timeline.get(next_action)
            && act.at.0 <= now.0
        {
            engine.apply(&act.action, act.at)?;
            next_action += 1;
        }
        engine.engine().step(Millis(1.0));
        let now = engine.engine().now().0;
        let e = engine.engine();
        for (f, t) in first.iter_mut().zip(&e.cells().activation_ms) {
            if f.is_nan() && t.is_finite() {
                *f = *t;
            }
        }
        while frame_times.get(next_frame).is_some_and(|t| *t <= now) {
            own.push(lead_values(&e.cells().vm));
            next_frame += 1;
        }
    }

    // activation, tissue cells only: conduction-system cells have no elements in the export
    let is_tissue = |i: usize| kinds[i] != CellKind::Driven;
    let (mut own_lat, mut their_lat) = (Vec::new(), Vec::new());
    for i in (0..n).filter(|i| is_tissue(*i)) {
        own_lat.push(f64::from(first[i]));
        their_lat.push(f64::from(imported.lat_ms[i]));
    }
    let activation = agreement(&own_lat, &their_lat);

    // ventricular span of heartsim's own beat sets the windows
    let (mut qrs_lo, mut qrs_hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for i in (0..n).filter(|i| matches!(kinds[*i], CellKind::Ventricular | CellKind::Purkinje)) {
        if first[i].is_finite() {
            qrs_lo = qrs_lo.min(f64::from(first[i]));
            qrs_hi = qrs_hi.max(f64::from(first[i]));
        }
    }
    let has_qrs = qrs_lo.is_finite();
    let qrs_window = has_qrs.then_some([qrs_lo - QRS_MARGIN_MS, qrs_hi + QRS_MARGIN_MS]);
    let t_window = has_qrs.then_some([qrs_hi + T_DELAY_MS, qrs_hi + T_DELAY_MS + T_SPAN_MS]);

    // the imported V_m through the same forward model
    let (mut rest, mut peak) = (None, None);
    let mut theirs: Vec<[f64; 12]> = Vec::with_capacity(frame_times.len());
    if let Some(vm) = &imported.vm {
        let r = a.vm_rest.unwrap_or_else(|| median(vm.frame(0).to_vec()));
        let p = a.vm_peak.unwrap_or_else(|| {
            vm.data
                .iter()
                .copied()
                .filter(|v| v.is_finite())
                .fold(f32::NEG_INFINITY, f32::max)
        });
        ensure!(p > r, "imported V_m has no swing: rest {r}, peak {p}");
        rest = Some(r);
        peak = Some(p);
        let mut normal = vec![0.0f32; n];
        for k in 0..vm.frames {
            for (out, v) in normal.iter_mut().zip(vm.frame(k)) {
                // unmapped cells stay at rest
                *out = if v.is_finite() { (v - r) / (p - r) } else { 0.0 };
            }
            theirs.push(lead_values(&normal));
        }
    }

    let mut leads = Vec::new();
    let (mut own_traces, mut their_traces): (Vec<Vec<f64>>, Vec<Vec<f64>>) = (Vec::new(), Vec::new());
    if !theirs.is_empty() {
        ensure!(
            own.len() == theirs.len(),
            "{} heartsim frames for {} imported ones",
            own.len(),
            theirs.len()
        );
        for (l, lead) in Lead::ALL.iter().enumerate() {
            let x: Vec<f64> = own.iter().map(|f| f[l]).collect();
            let y: Vec<f64> = theirs.iter().map(|f| f[l]).collect();
            let r_in = |w: Option<[f64; 2]>| {
                w.and_then(|[from, to]| {
                    let k = window(&frame_times, from, to);
                    correlation(&x[k.clone()], &y[k])
                })
            };
            leads.push(LeadMetrics {
                lead: lead.label().to_string(),
                rmse_mv: agreement(&x, &y).rms,
                qrs_r: r_in(qrs_window),
                t_r: r_in(t_window),
            });
            own_traces.push(x);
            their_traces.push(y);
        }
    }

    std::fs::create_dir_all(&a.out).with_context(|| format!("creating {}", a.out.display()))?;
    let scatter: Vec<(f64, f64)> = own_lat.iter().copied().zip(their_lat.iter().copied()).collect();
    let tier_label = if a.tier == Tier::One { "Tier 1" } else { "Tier 2" };
    if activation.n > 0 {
        activation_scatter(
            &a.out.join("activation_scatter.png"),
            &format!("{}: activation, imported against {tier_label}", s.scenario.id),
            &format!("{tier_label} activation, ms"),
            "imported activation, ms",
            &scatter,
        )?;
    }
    if !own_traces.is_empty() {
        let traces: Vec<LeadTrace<'_>> = Lead::ALL
            .iter()
            .enumerate()
            .map(|(l, lead)| LeadTrace {
                label: lead.label(),
                reference_mv: &own_traces[l],
                test_mv: &their_traces[l],
            })
            .collect();
        ecg_overlay(
            &a.out.join("ecg_overlay.png"),
            &format!(
                "{}: {tier_label} (blue) and imported (orange), same forward model",
                s.scenario.id
            ),
            &frame_times,
            &traces,
        )?;
    }
    let metrics = ReportMetrics {
        scenario: s.scenario.id.clone(),
        tier: tier_label.to_string(),
        activation_ms: activation,
        vm_rest: rest,
        vm_peak: peak,
        leads,
        qrs_window_ms: qrs_window,
        t_window_ms: t_window,
        cells_compared: activation.n,
        weight_stats: model.weight_stats(),
    };
    std::fs::write(
        a.out.join("metrics.json"),
        serde_json::to_string_pretty(&metrics)?,
    )
    .context("writing metrics.json")?;
    std::fs::write(a.out.join("report.md"), markdown(&metrics, &imported.summary))
        .context("writing report.md")?;
    eprintln!(
        "activation RMS {} ms over {} cells; wrote {}",
        cell(Some(activation.rms), 2),
        activation.n,
        a.out.join("report.md").display()
    );
    Ok(())
}

fn markdown(m: &ReportMetrics, import: &super::import::ImportSummary) -> String {
    let mut s = format!(
        "# Validation report: `{}`, {} against an imported solver result\n\n\
Solver files: {}. {} of {} cells were mapped by nearest point from {} samples (mean distance {:.3} mm, largest {:.3} mm, limit {} mm). \
Imported activation times were {}.\n\n",
        m.scenario,
        m.tier,
        import.files.join(", "),
        import.cells_mapped,
        import.cells,
        import.source_points,
        import.mean_distance_mm,
        import.max_distance_mm,
        import.max_distance_limit_mm,
        if import.lat_from_vm {
            "computed here from the V_m series"
        } else {
            "read from the solver's files"
        },
    );
    let a = &m.activation_ms;
    let _ = writeln!(
        s,
        "## Activation time, imported minus {}\n\n| cells compared | cells missing | RMS ms | bias ms | max abs ms | Pearson r |\n|---|---|---|---|---|---|\n| {} | {} | {} | {} | {} | {} |\n\n![activation scatter](activation_scatter.png)\n",
        m.tier,
        a.n,
        a.missing,
        cell(Some(a.rms), 2),
        cell(Some(a.bias), 2),
        cell(Some(a.max_abs), 2),
        cell(a.r, 4)
    );
    if m.leads.is_empty() {
        s.push_str("## ECG\n\nNot compared: the import holds no V_m series.\n\n");
    } else {
        let _ = writeln!(
            s,
            "## ECG, per lead\n\nBoth V_m fields went through heartsim's same forward model, so these numbers compare activation and repolarisation only. \
Imported V_m normalised with rest {} and peak {}. QRS window {} ms, T window {} ms, from heartsim's own beat. A dash means the window holds too few samples or a flat signal.\n",
            cell(m.vm_rest.map(f64::from), 2),
            cell(m.vm_peak.map(f64::from), 2),
            m.qrs_window_ms
                .map_or("-".to_string(), |w| format!("{:.0}-{:.0}", w[0], w[1])),
            m.t_window_ms
                .map_or("-".to_string(), |w| format!("{:.0}-{:.0}", w[0], w[1])),
        );
        s.push_str("| lead | RMSE mV | QRS r | T r |\n|---|---|---|---|\n");
        for l in &m.leads {
            let _ = writeln!(
                s,
                "| {} | {:.4} | {} | {} |",
                l.lead,
                l.rmse_mv,
                cell(l.qrs_r, 4),
                cell(l.t_r, 4)
            );
        }
        s.push_str("\n![ECG overlay](ecg_overlay.png)\n\n");
    }
    s.push_str("## Forward-model weights per electrode\n\n| electrode | Σ w | Σ |w| | effective cells | centroid mm |\n|---|---|---|---|---|\n");
    for w in &m.weight_stats {
        let _ = writeln!(
            s,
            "| {:?} | {:.3e} | {:.3} | {:.0} | {:.1}, {:.1}, {:.1} |",
            w.electrode,
            w.sum,
            w.abs_sum,
            w.effective_cells,
            w.centroid_mm[0],
            w.centroid_mm[1],
            w.centroid_mm[2]
        );
    }
    s
}
