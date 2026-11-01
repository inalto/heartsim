//! Ring buffers for strip display and export, and min/max decimation per pixel column so that
//! narrow QRS spikes are never lost when thousands of samples map to a few hundred pixels.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeadRing {
    buf: Vec<f32>,
    head: usize,
    len: usize,
    pub sample_rate_hz: f32,
}

impl LeadRing {
    #[must_use]
    pub fn new(capacity: usize, sample_rate_hz: f32) -> Self {
        Self {
            buf: vec![0.0; capacity.max(1)],
            head: 0,
            len: 0,
            sample_rate_hz,
        }
    }
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.buf.len()
    }
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn push(&mut self, v: f32) {
        self.buf[self.head] = v;
        self.head = (self.head + 1) % self.buf.len();
        self.len = (self.len + 1).min(self.buf.len());
    }
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }
    /// i-th most recent sample (0 = newest).
    #[must_use]
    pub fn recent(&self, i: usize) -> Option<f32> {
        if i >= self.len {
            return None;
        }
        let idx = (self.head + self.buf.len() - 1 - i) % self.buf.len();
        Some(self.buf[idx])
    }
    /// Oldest-to-newest iterator.
    pub fn iter(&self) -> impl Iterator<Item = f32> + '_ {
        (0..self.len).rev().map(move |i| self.recent(i).unwrap_or(0.0))
    }
    /// Copy the newest `n` samples (oldest first) into `out`; returns how many were written.
    pub fn latest(&self, n: usize, out: &mut Vec<f32>) -> usize {
        out.clear();
        let n = n.min(self.len);
        for i in (0..n).rev() {
            out.push(self.recent(i).unwrap_or(0.0));
        }
        n
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MinMax {
    pub min: f32,
    pub max: f32,
}

/// Decimate `samples` (oldest first) into `columns` (min, max) pairs. Columns beyond the data are
/// `None`. Every sample is visited exactly once; global extremes are preserved by construction.
#[must_use]
pub fn minmax_decimate(samples: &[f32], columns: usize) -> Vec<Option<MinMax>> {
    let mut out = vec![None; columns];
    if samples.is_empty() || columns == 0 {
        return out;
    }
    let per = samples.len() as f64 / columns as f64;
    for (i, s) in samples.iter().enumerate() {
        let c = ((i as f64 / per) as usize).min(columns - 1);
        out[c] = Some(match out[c] {
            None => MinMax { min: *s, max: *s },
            Some(m) => MinMax {
                min: m.min.min(*s),
                max: m.max.max(*s),
            },
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn ring_keeps_latest() {
        let mut r = LeadRing::new(4, 1.0);
        for v in 0..6 {
            r.push(v as f32);
        }
        assert_eq!(r.len(), 4);
        assert_eq!(r.recent(0), Some(5.0));
        assert_eq!(r.iter().collect::<Vec<_>>(), vec![2.0, 3.0, 4.0, 5.0]);
        let mut out = Vec::new();
        assert_eq!(r.latest(2, &mut out), 2);
        assert_eq!(out, vec![4.0, 5.0]);
    }

    proptest! {
        #[test]
        fn decimation_preserves_global_extremes(samples in proptest::collection::vec(-5.0f32..5.0, 1..2000), cols in 1usize..300) {
            let d = minmax_decimate(&samples, cols);
            let gmax = samples.iter().copied().fold(f32::MIN, f32::max);
            let gmin = samples.iter().copied().fold(f32::MAX, f32::min);
            let dmax = d.iter().flatten().map(|m| m.max).fold(f32::MIN, f32::max);
            let dmin = d.iter().flatten().map(|m| m.min).fold(f32::MAX, f32::min);
            prop_assert!(gmax.to_bits() == dmax.to_bits(), "max {gmax} vs {dmax}");
            prop_assert!(gmin.to_bits() == dmin.to_bits(), "min {gmin} vs {dmin}");
            prop_assert!(d.iter().flatten().all(|m| m.min <= m.max));
        }
    }
}
