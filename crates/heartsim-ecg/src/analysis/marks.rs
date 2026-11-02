//! Wave annotations for the strip (feature F-12 wave labels).

use super::{Beat, dsp};
use heartsim_core::Lead;

/// Which deflection a [`WaveMark`] labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaveKind {
    /// Atrial depolarisation.
    P,
    /// First negative deflection of the QRS, before the R wave.
    Q,
    /// First positive deflection of the QRS.
    R,
    /// Negative deflection after the R wave.
    S,
    /// Ventricular repolarisation.
    T,
    /// Late repolarisation hump. Reserved: the current detector never emits it.
    U,
}

impl WaveKind {
    /// Single-letter label for the strip.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            WaveKind::P => "P",
            WaveKind::Q => "Q",
            WaveKind::R => "R",
            WaveKind::S => "S",
            WaveKind::T => "T",
            WaveKind::U => "U",
        }
    }
}

/// One labelled wave on one lead. Indices are sample positions in the analysed window, oldest
/// sample = 0, so they line up with `LeadRing::latest(n)` output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaveMark {
    /// Lead the label belongs to.
    pub lead: Lead,
    /// Which wave.
    pub kind: WaveKind,
    /// First sample of the wave.
    pub onset: usize,
    /// Sample of the extremum (the label anchor).
    pub peak: usize,
    /// Last sample of the wave.
    pub offset: usize,
}

/// Smallest deflection (mV) that deserves its own Q or S label.
const MIN_QS_MV: f32 = 0.05;

/// Build the P/Q/R/S/T marks of every beat on `sig` (the signal of `lead`, oldest first).
///
/// Within the QRS the R wave is the largest positive deflection above the isoelectric `baseline`;
/// Q is the negative dip before it and S the negative dip after it. A purely negative complex
/// gets an S mark only (QS morphology is labelled S).
#[must_use]
pub fn marks_for_lead(lead: Lead, sig: &[f32], beats: &[Beat]) -> Vec<WaveMark> {
    let baseline = dsp::median(sig).unwrap_or(0.0);
    let mut out = Vec::new();
    let at = |i: usize| sig.get(i).copied().unwrap_or(baseline) - baseline;
    for b in beats {
        if let Some(p) = b.p {
            let peak = extremum(sig, baseline, p.onset, p.offset);
            out.push(mark(lead, WaveKind::P, p.onset, peak, p.offset));
        }
        let (lo, hi) = (b.qrs_onset, b.qrs_offset.min(sig.len().saturating_sub(1)));
        if lo < hi {
            let hi_i = (lo..=hi).max_by(|&a, &c| at(a).total_cmp(&at(c))).unwrap_or(lo);
            let has_r = at(hi_i) > MIN_QS_MV;
            let low_in = |a: usize, c: usize| (a..=c).min_by(|&x, &y| at(x).total_cmp(&at(y)));
            if has_r {
                // Walk to the baseline crossings on either side of the R apex.
                let mut r_on = hi_i;
                while r_on > lo && at(r_on - 1) > 0.0 {
                    r_on -= 1;
                }
                let mut r_off = hi_i;
                while r_off < hi && at(r_off + 1) > 0.0 {
                    r_off += 1;
                }
                out.push(mark(lead, WaveKind::R, r_on, hi_i, r_off));
                if r_on > lo
                    && let Some(q) = low_in(lo, r_on - 1).filter(|&q| at(q) < -MIN_QS_MV)
                {
                    out.push(mark(lead, WaveKind::Q, lo, q, r_on - 1));
                }
                if r_off < hi
                    && let Some(s) = low_in(r_off + 1, hi).filter(|&s| at(s) < -MIN_QS_MV)
                {
                    out.push(mark(lead, WaveKind::S, r_off + 1, s, hi));
                }
            } else if let Some(s) = low_in(lo, hi).filter(|&s| at(s) < -MIN_QS_MV) {
                out.push(mark(lead, WaveKind::S, lo, s, hi));
            }
        }
        if let Some(t) = b.t {
            let peak = extremum(sig, baseline, t.onset, t.offset);
            out.push(mark(lead, WaveKind::T, t.onset, peak, t.offset));
        }
    }
    out
}

fn mark(lead: Lead, kind: WaveKind, onset: usize, peak: usize, offset: usize) -> WaveMark {
    WaveMark {
        lead,
        kind,
        onset,
        peak: peak.clamp(onset, offset.max(onset)),
        offset: offset.max(onset),
    }
}

/// Index of the largest |deviation from baseline| in `[lo, hi]`.
fn extremum(sig: &[f32], baseline: f32, lo: usize, hi: usize) -> usize {
    let hi = hi.min(sig.len().saturating_sub(1));
    (lo..=hi.max(lo))
        .max_by(|&a, &b| {
            let da = (sig.get(a).copied().unwrap_or(baseline) - baseline).abs();
            let db = (sig.get(b).copied().unwrap_or(baseline) - baseline).abs();
            da.total_cmp(&db)
        })
        .unwrap_or(lo)
}
