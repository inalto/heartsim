//! History of 3-D view state for scrubbing: while paused, the viewport shows the snapshot taken
//! at the strip's cursor time instead of the live state, so the ECG and the heart stay in step.

use heartsim_core::Millis;
use std::collections::VecDeque;

/// Snapshots taken every `period` of simulation time, the last `span` kept.
#[derive(Debug, Clone)]
pub struct SnapshotRing<T> {
    period: Millis,
    span: Millis,
    entries: VecDeque<(Millis, T)>,
}

impl<T> SnapshotRing<T> {
    /// 10 Hz for the last 60 s (the strip history length).
    #[must_use]
    pub fn standard() -> Self {
        Self::new(Millis(100.0), Millis(60_000.0))
    }

    #[must_use]
    pub fn new(period: Millis, span: Millis) -> Self {
        Self {
            period,
            span,
            entries: VecDeque::new(),
        }
    }

    /// Take a snapshot with `make` if a period has passed since the last one. Time running
    /// backwards (scenario reset) clears the history.
    pub fn record(&mut self, now: Millis, make: impl FnOnce() -> T) {
        if let Some((last, _)) = self.entries.back() {
            if now.0 < last.0 {
                self.entries.clear();
            } else if now.0 - last.0 < self.period.0 {
                return;
            }
        }
        self.entries.push_back((now, make()));
        while self
            .entries
            .front()
            .is_some_and(|(t, _)| now.0 - t.0 > self.span.0)
        {
            self.entries.pop_front();
        }
    }

    /// The latest snapshot taken at or before `t` (the oldest one if `t` predates the history).
    #[must_use]
    pub fn at(&self, t: Millis) -> Option<&T> {
        let i = self.entries.partition_point(|(s, _)| s.0 <= t.0);
        self.entries.get(i.saturating_sub(1)).map(|(_, v)| v)
    }

    /// The snapshots bracketing `t` with their times: the latest at or before `t` and the next
    /// one (the same one twice at either end of the history), for interpolation.
    #[must_use]
    pub fn around(&self, t: Millis) -> Option<((Millis, &T), (Millis, &T))> {
        let i = self.entries.partition_point(|(s, _)| s.0 <= t.0);
        let a = self.entries.get(i.saturating_sub(1))?;
        let b = self.entries.get(i).unwrap_or(a);
        Some(((a.0, &a.1), (b.0, &b.1)))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_ten_hz_for_sixty_seconds_and_looks_up_by_time() {
        let mut r = SnapshotRing::standard();
        for ms in 0..=90_000 {
            r.record(Millis(f64::from(ms)), || ms);
        }
        assert_eq!(r.len(), 601);
        assert_eq!(r.at(Millis(85_049.0)), Some(&85_000));
        assert_eq!(
            r.at(Millis(0.0)),
            Some(&30_000),
            "older than the history: oldest kept"
        );
        r.record(Millis(10.0), || 10);
        assert_eq!(r.len(), 1, "time went backwards: history cleared");
    }
}
