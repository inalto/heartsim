//! Ring buffers for strip display and export, and min/max decimation per pixel column so that
//! narrow QRS spikes are never lost when thousands of samples map to a few hundred pixels.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeadRing {
    buf: Vec<f32>,
    head: usize,
    len: usize,
    /// Samples pushed since creation: the absolute index of the next sample. Gives the strip a
    /// time base (sample `k` was taken at `k / sample_rate_hz` s) that survives wrap-around.
    #[serde(default)]
    total: u64,
    pub sample_rate_hz: f32,
}

impl LeadRing {
    #[must_use]
    pub fn new(capacity: usize, sample_rate_hz: f32) -> Self {
        Self {
            buf: vec![0.0; capacity.max(1)],
            head: 0,
            len: 0,
            total: 0,
            sample_rate_hz,
        }
    }
    /// An empty ring whose first sample will have absolute index `first_index`, so a ring derived
    /// from another (the filtered display of the strip) can line up with it sample for sample.
    #[must_use]
    pub fn starting_at(capacity: usize, sample_rate_hz: f32, first_index: u64) -> Self {
        Self {
            total: first_index,
            ..Self::new(capacity, sample_rate_hz)
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
        self.total += 1;
    }
    /// Number of samples ever pushed (not reset by [`LeadRing::clear`], so time never runs
    /// backwards). The newest sample has absolute index `pushed() - 1`.
    #[must_use]
    pub fn pushed(&self) -> u64 {
        self.total
    }
    /// Absolute index of the oldest sample still held.
    #[must_use]
    pub fn oldest_index(&self) -> u64 {
        self.total - self.len as u64
    }
    /// Sample with absolute index `abs`, if it is still held.
    #[must_use]
    pub fn at(&self, abs: u64) -> Option<f32> {
        if abs >= self.total {
            return None;
        }
        self.recent((self.total - 1 - abs) as usize)
    }
    /// Copy the samples with absolute indices in `start..end` (oldest first) into `out`, clamped
    /// to what is held. Returns the absolute index of the first sample written.
    pub fn range(&self, start: u64, end: u64, out: &mut Vec<f32>) -> u64 {
        out.clear();
        let start = start.max(self.oldest_index());
        let end = end.min(self.total);
        if start < end {
            // Two contiguous slices at most: up to the end of the buffer, then from its start.
            let cap = self.buf.len();
            let n = (end - start) as usize;
            let first = (self.head + cap - (self.total - start) as usize) % cap;
            let run = n.min(cap - first);
            out.extend_from_slice(&self.buf[first..first + run]);
            out.extend_from_slice(&self.buf[..n - run]);
        }
        start
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
        assert_eq!(r.pushed(), 6);
        assert_eq!(r.oldest_index(), 2);
        assert_eq!(r.at(3), Some(3.0));
        assert_eq!(r.at(1), None);
        assert_eq!(r.range(0, 5, &mut out), 2);
        assert_eq!(out, vec![2.0, 3.0, 4.0]);
        assert_eq!(r.range(3, 9, &mut out), 3);
        assert_eq!(out, vec![3.0, 4.0, 5.0]);
        assert_eq!(r.range(4, 4, &mut out), 4);
        assert_eq!(out, Vec::<f32>::new());
    }

    #[test]
    fn a_ring_can_start_at_an_absolute_index() {
        let mut r = LeadRing::starting_at(4, 500.0, 1000);
        assert_eq!((r.pushed(), r.len(), r.oldest_index()), (1000, 0, 1000));
        r.push(1.5);
        r.push(2.5);
        assert_eq!(r.pushed(), 1002);
        assert_eq!(r.at(1000), Some(1.5));
        assert_eq!(r.at(999), None);
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
