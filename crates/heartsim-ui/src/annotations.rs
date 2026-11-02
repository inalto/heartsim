//! Wave labels and intervals for the strips, from the B4 analyser (`heartsim_ecg::analysis`).
//!
//! The analyser works on a window of sample indices starting at 0; here the window's first sample
//! is pinned to its absolute ring index so labels stay on their waves while the strip scrolls.

use egui::Ui;
use heartsim_core::{AxisClass, LEAD_COUNT, Lead};
use heartsim_ecg::analysis::{DEFAULT_WINDOW_S, marks::marks_for_lead};
use heartsim_ecg::{Analysis, Analyzer, Beat, LeadRing, Regularity, SignalQuality, WaveMark};

/// One analysis of the twelve rings plus per-lead wave marks, in absolute sample indices.
#[derive(Debug, Clone)]
pub struct EcgAnnotations {
    /// Measurements and delineated beats (indices relative to `first_sample`).
    pub analysis: Analysis,
    /// Absolute ring index of window sample 0.
    pub first_sample: u64,
    /// Absolute index one past the last analysed sample.
    pub head: u64,
    /// P/Q/R/S/T marks of every beat, per lead (indexed by `Lead as usize`).
    pub marks: Vec<Vec<WaveMark>>,
}

/// The marks of one lead, borrowed for drawing.
#[derive(Debug, Clone, Copy)]
pub struct LeadMarks<'a> {
    /// Absolute ring index that the mark and beat indices are relative to.
    pub first_sample: u64,
    /// Delineated beats (shared by all leads).
    pub beats: &'a [Beat],
    /// This lead's wave marks.
    pub marks: &'a [WaveMark],
    pub sample_rate_hz: f32,
}

impl EcgAnnotations {
    /// Analyse the [`DEFAULT_WINDOW_S`] seconds of `rings` (indexed by `Lead as usize`) that end
    /// just before absolute sample `head`.
    #[must_use]
    pub fn compute(analyzer: &Analyzer, rings: &[LeadRing], head: u64) -> Self {
        let fs = rings.first().map_or(500.0, |r| r.sample_rate_hz);
        let want = (DEFAULT_WINDOW_S * fs) as u64;
        let mut bufs: Vec<Vec<f32>> = vec![Vec::new(); LEAD_COUNT];
        for (r, b) in rings.iter().zip(&mut bufs) {
            r.range(head.saturating_sub(want), head, b);
        }
        // Leads are pushed together, so they end at the same index; trim to the shortest.
        let n = bufs.iter().map(Vec::len).min().unwrap_or(0);
        let refs: [&[f32]; LEAD_COUNT] = std::array::from_fn(|i| &bufs[i][bufs[i].len() - n..]);
        let analysis = analyzer.analyze(&refs, fs);
        let marks = Lead::ALL
            .iter()
            .map(|l| marks_for_lead(*l, refs[*l as usize], &analysis.beats))
            .collect();
        Self {
            analysis,
            first_sample: head - n as u64,
            head,
            marks,
        }
    }

    /// Borrow one lead's marks for drawing.
    #[must_use]
    pub fn lead(&self, lead: Lead) -> LeadMarks<'_> {
        LeadMarks {
            first_sample: self.first_sample,
            beats: &self.analysis.beats,
            marks: &self.marks[lead as usize],
            sample_rate_hz: self.analysis.sample_rate_hz,
        }
    }
}

fn fmt_ms(v: Option<f64>) -> String {
    v.map_or_else(|| "—".to_owned(), |v| format!("{v:.0} ms"))
}

/// The interval read-out panel: HR, RR, PR, QRS, QT, QTc (Bazett and Fridericia), frontal QRS
/// and T axes with the axis class, and rhythm regularity. Unmeasurable values show a dash, never
/// a guess.
pub fn interval_readout(ui: &mut Ui, analysis: Option<&Analysis>) {
    let Some(a) = analysis else {
        ui.weak("Measuring…");
        return;
    };
    match a.quality {
        SignalQuality::NoSignal => {
            ui.weak("No signal");
            return;
        }
        SignalQuality::NoQrs => {
            ui.weak("No QRS complexes detected");
            return;
        }
        SignalQuality::Good => {}
    }
    egui::Grid::new("interval_readout")
        .num_columns(2)
        .striped(true)
        .show(ui, |ui| {
            let mut row = |k: &str, v: String| {
                ui.label(k);
                ui.monospace(v);
                ui.end_row();
            };
            row(
                "HR",
                a.hr_bpm.map_or_else(|| "—".to_owned(), |v| format!("{v:.0} bpm")),
            );
            row("RR", fmt_ms(a.rr_ms));
            row(
                "Rhythm",
                match a.regularity {
                    Regularity::Unknown => "—",
                    Regularity::Regular => "regular",
                    Regularity::Irregular => "irregular",
                }
                .to_owned(),
            );
            row(
                "PR",
                if a.p_wave_present {
                    fmt_ms(a.pr_ms)
                } else {
                    "no P waves".to_owned()
                },
            );
            row("QRS", fmt_ms(a.qrs_ms));
            row("QT", fmt_ms(a.qt_ms));
            row("QTc (Bazett)", fmt_ms(a.qtc_bazett_ms));
            row("QTc (Fridericia)", fmt_ms(a.qtc_fridericia_ms));
            row(
                "QRS axis",
                a.qrs_axis.map_or_else(
                    || "—".to_owned(),
                    |ax| {
                        let class = match ax.classify() {
                            AxisClass::Normal => "normal",
                            AxisClass::LeftDeviation => "left deviation",
                            AxisClass::RightDeviation => "right deviation",
                            AxisClass::ExtremeDeviation => "extreme",
                        };
                        format!("{:+.0}° ({class})", ax.degrees)
                    },
                ),
            );
            row(
                "T axis",
                a.t_axis
                    .map_or_else(|| "—".to_owned(), |ax| format!("{:+.0}°", ax.degrees)),
            );
        });
}
