//! Small offline DSP helpers shared by the analysis stages. All of them are zero-phase (they
//! look both ways in time) because analysis runs on a recorded window, never on a live stream,
//! and a phase lag would shift every fiducial point.

use crate::filter::{HighPass, LowPass};

/// Spikes shorter than this are treated as pacemaker artefacts and removed, ms.
pub(crate) const SPIKE_MAX_MS: f32 = 5.0;
/// A sample-to-sample jump larger than this (mV) marks a candidate spike. Physiological QRS slopes
/// are below ~0.15 mV per 2 ms; pacing stimuli are volts.
const SPIKE_JUMP_MV: f32 = 0.3;

/// Median of a small slice (sorts a copy; used for ≤ 33 elements).
fn small_median(buf: &mut [f32]) -> f32 {
    buf.sort_by(f32::total_cmp);
    buf[buf.len() / 2]
}

/// Median of `x` (not modified). `None` when empty.
pub(crate) fn median(x: &[f32]) -> Option<f32> {
    if x.is_empty() {
        return None;
    }
    let mut v = x.to_vec();
    let mid = v.len() / 2;
    Some(*v.select_nth_unstable_by(mid, f32::total_cmp).1)
}

/// Remove pacing-stimulus spikes: wherever a sample jumps by more than [`SPIKE_JUMP_MV`] the
/// neighbourhood is replaced by a running median whose half-width covers [`SPIKE_MAX_MS`]. Real
/// QRS slopes never trigger it, so ordinary signals pass through bit-exact.
pub(crate) fn despike(x: &[f32], fs: f32) -> Vec<f32> {
    let mut out = x.to_vec();
    let h = (SPIKE_MAX_MS * 1e-3 * fs).floor() as usize;
    if h == 0 || x.len() < 2 * h + 3 {
        return out;
    }
    let mut mark = vec![false; x.len()];
    for i in 1..x.len() {
        if (x[i] - x[i - 1]).abs() > SPIKE_JUMP_MV {
            let lo = i.saturating_sub(h + 1);
            let hi = (i + h + 1).min(x.len() - 1);
            mark[lo..=hi].iter_mut().for_each(|m| *m = true);
        }
    }
    let mut win = Vec::with_capacity(2 * h + 1);
    for i in 0..x.len() {
        if mark[i] {
            let lo = i.saturating_sub(h);
            let hi = (i + h).min(x.len() - 1);
            win.clear();
            win.extend_from_slice(&x[lo..=hi]);
            out[i] = small_median(&mut win);
        }
    }
    out
}

/// Standard deviation of white noise riding on a smooth signal, from the median absolute
/// second difference (robust to the QRS, whose curvature occupies a small fraction of samples).
/// Zero for noise-free input.
pub(crate) fn white_noise_sigma(x: &[f32]) -> f32 {
    if x.len() < 5 {
        return 0.0;
    }
    let d2: Vec<f32> = x.windows(3).map(|w| (w[0] - 2.0 * w[1] + w[2]).abs()).collect();
    // Var(d2) = 6σ²; median|N(0, s²)| = 0.6745 s.
    median(&d2).unwrap_or(0.0) / (0.6745 * 6.0_f32.sqrt())
}

/// Centred moving average of odd width `w` (edges use the available samples).
pub(crate) fn moving_average(x: &[f32], w: usize) -> Vec<f32> {
    let n = x.len();
    let w = w.max(1) | 1;
    if w == 1 || n == 0 {
        return x.to_vec();
    }
    let half = w / 2;
    let mut prefix = Vec::with_capacity(n + 1);
    let mut acc = 0.0_f64;
    prefix.push(0.0);
    for v in x {
        acc += f64::from(*v);
        prefix.push(acc);
    }
    (0..n)
        .map(|i| {
            let lo = i.saturating_sub(half);
            let hi = (i + half + 1).min(n);
            ((prefix[hi] - prefix[lo]) / (hi - lo) as f64) as f32
        })
        .collect()
}

/// Central-difference derivative in units per millisecond.
pub(crate) fn derivative_per_ms(x: &[f32], fs: f32) -> Vec<f32> {
    let n = x.len();
    let mut d = vec![0.0; n];
    if n < 3 {
        return d;
    }
    let k = fs / 2000.0;
    for i in 1..n - 1 {
        d[i] = (x[i + 1] - x[i - 1]) * k;
    }
    d[0] = d[1];
    d[n - 1] = d[n - 2];
    d
}

/// 5–15 Hz band-pass of the Pan–Tompkins detector, run forward and backward so that the energy
/// peaks stay centred on the QRS.
pub(crate) fn bandpass_5_15(x: &[f32], fs: f32) -> Vec<f32> {
    let base = median(x).unwrap_or(0.0);
    let mut y: Vec<f32> = x.iter().map(|v| v - base).collect();
    for _ in 0..2 {
        let mut hp = HighPass::new(5.0, fs);
        let mut lp = LowPass::new(15.0, fs);
        for v in &mut y {
            *v = lp.process(hp.process(*v));
        }
        y.reverse();
    }
    y
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn despike_removes_short_spikes_only() {
        let mut x = vec![0.0_f32; 200];
        x[100] = 4.0;
        x[101] = 4.0;
        for (i, v) in x.iter_mut().enumerate().take(160).skip(140) {
            *v = (i - 139) as f32 * 0.01;
        }
        let y = despike(&x, 500.0);
        assert!(y[100].abs() < 1e-6 && y[101].abs() < 1e-6);
        assert_eq!(&y[140..160], &x[140..160], "slow ramps pass through untouched");
    }

    #[test]
    fn moving_average_of_constant_is_constant() {
        let y = moving_average(&[2.0; 50], 7);
        assert!(y.iter().all(|v| (v - 2.0).abs() < 1e-6));
    }

    #[test]
    fn median_works() {
        assert_eq!(median(&[3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&[]), None);
    }
}
