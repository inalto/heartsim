//! A 12-lead excerpt: the one thing every exporter writes and every importer reads, so the app
//! (which cuts it from its strip rings) and the CLI (which cuts it from a scenario run) hand the
//! writers identical samples.

use heartsim_core::{LEAD_COUNT, Lead};
use heartsim_ecg::{LeadRing, Recording};

/// Sampling rate of every export, Hz: the strip rings' rate, and WFDB's customary 500 Hz.
pub const EXPORT_RATE_HZ: f64 = 500.0;

/// Twelve leads sampled at a common rate, oldest first, in [`Lead::ALL`] order.
#[derive(Debug, Clone, PartialEq)]
pub struct EcgRecord {
    pub sample_rate_hz: f64,
    /// `leads[l][i]` = lead `l` (`Lead as usize`) at `start_ms + i / sample_rate_hz`, mV.
    pub leads: Vec<Vec<f32>>,
    /// Simulation time of the first sample, ms (0 for an imported recording that carries none).
    pub start_ms: f64,
}

impl EcgRecord {
    /// An empty record at `sample_rate_hz`.
    #[must_use]
    pub fn empty(sample_rate_hz: f64) -> Self {
        Self {
            sample_rate_hz,
            leads: vec![Vec::new(); LEAD_COUNT],
            start_ms: 0.0,
        }
    }

    /// The last `seconds` before absolute ring index `head` (exclusive). `newest_ms` is the
    /// simulation time of sample `head - 1`; it fixes `start_ms`. Shorter when the rings hold less.
    #[must_use]
    pub fn from_rings(rings: &[LeadRing], head: u64, seconds: f64, newest_ms: f64) -> Self {
        let fs = rings
            .first()
            .map_or(EXPORT_RATE_HZ, |r| f64::from(r.sample_rate_hz));
        let want = (seconds * fs).round() as u64;
        let mut leads: Vec<Vec<f32>> = rings
            .iter()
            .take(LEAD_COUNT)
            .map(|ring| {
                let mut buf = Vec::new();
                ring.range(head.saturating_sub(want), head, &mut buf);
                buf
            })
            .collect();
        leads.resize(LEAD_COUNT, Vec::new());
        let n = leads[0].len();
        Self {
            sample_rate_hz: fs,
            leads,
            start_ms: newest_ms - (n.saturating_sub(1)) as f64 * 1000.0 / fs,
        }
    }

    /// The last `seconds` of a scenario run, resampled to `rate_hz` with the same alignment the
    /// app's strip uses (sample times are whole multiples of the period, the first at one period).
    #[must_use]
    pub fn from_recording(rec: &Recording, seconds: f64, rate_hz: f64) -> Self {
        let leads: Vec<Vec<f32>> = (0..LEAD_COUNT).map(|l| rec.tail(l, seconds, rate_hz)).collect();
        let n = leads[0].len();
        let period_ms = 1000.0 / rate_hz;
        let newest_ms = (rec.len() as f64 * heartsim_ecg::scenario::STEP_MS / period_ms).floor() * period_ms;
        Self {
            sample_rate_hz: rate_hz,
            leads,
            start_ms: newest_ms - n.saturating_sub(1) as f64 * period_ms,
        }
    }

    /// Samples per lead.
    #[must_use]
    pub fn len(&self) -> usize {
        self.leads.first().map_or(0, Vec::len)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Length, s.
    #[must_use]
    pub fn duration_s(&self) -> f64 {
        self.len() as f64 / self.sample_rate_hz
    }

    /// Samples of one lead.
    #[must_use]
    pub fn lead(&self, lead: Lead) -> &[f32] {
        &self.leads[lead as usize]
    }

    /// The last `seconds` of this record (all of it when shorter).
    #[must_use]
    pub fn tail(&self, seconds: f64) -> Self {
        let want = (seconds * self.sample_rate_hz).round() as usize;
        let from = self.len().saturating_sub(want);
        Self {
            sample_rate_hz: self.sample_rate_hz,
            leads: self.leads.iter().map(|l| l[from..].to_vec()).collect(),
            start_ms: self.start_ms + from as f64 * 1000.0 / self.sample_rate_hz,
        }
    }

    /// Strip rings holding the whole record (absolute indices `0..len`), at least
    /// `min_capacity` samples each so a reviewer can keep the usual 60 s of scrub history.
    #[must_use]
    pub fn to_rings(&self, min_capacity: usize) -> Vec<LeadRing> {
        self.leads
            .iter()
            .map(|l| {
                let mut r = LeadRing::new(l.len().max(min_capacity), self.sample_rate_hz as f32);
                for v in l {
                    r.push(*v);
                }
                r
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rings_and_tail_round_trip() {
        let mut rec = EcgRecord::empty(500.0);
        for (l, lead) in rec.leads.iter_mut().enumerate() {
            *lead = (0..1000).map(|i| (i + l) as f32 * 0.001).collect();
        }
        rec.start_ms = 2.0;
        let rings = rec.to_rings(10);
        let back = EcgRecord::from_rings(&rings, 1000, 2.0, 2.0 + 999.0 * 2.0);
        assert_eq!(back, rec);
        let t = rec.tail(1.0);
        assert_eq!(t.len(), 500);
        assert!((t.start_ms - (2.0 + 500.0 * 2.0)).abs() < 1e-9);
        assert_eq!(t.lead(Lead::II)[0], rec.lead(Lead::II)[500]);
    }
}
