//! Pan–Tompkins style QRS detector (Pan & Tompkins 1985): band-pass 5–15 Hz → derivative →
//! squaring → moving-window integration → adaptive signal/noise thresholds with a 200 ms
//! refractory period and a search-back for missed beats.
//!
//! Because the analysed window is recorded, every stage is zero-phase and the integrated-energy
//! maxima sit on the QRS itself; the R peak is then refined on the raw lead.

use super::dsp;

/// Refractory period after a detected QRS, ms (ventricular ERP is far longer, so nothing
/// physiological is lost).
pub const REFRACTORY_MS: f32 = 200.0;
/// Moving-window integration width, ms (≈ the widest normal QRS).
const INTEGRATION_MS: f32 = 150.0;
/// A candidate this close after the previous QRS must be at least half as strong to be a QRS rather
/// than a T wave.
const T_WAVE_GUARD_MS: f32 = 360.0;
/// Peaks must stand out of the integrated-energy background by this factor, otherwise the lead
/// holds noise rather than QRS complexes.
const MIN_PEAK_TO_BACKGROUND: f32 = 5.0;
/// Smallest peak-to-peak deflection (mV) that is accepted as a QRS complex.
const MIN_QRS_PP_MV: f32 = 0.1;

/// Result of running the detector on one lead.
#[derive(Debug, Clone, PartialEq)]
pub struct QrsDetection {
    /// Sample index of the R peak (largest deflection from baseline) of each complex.
    pub peaks: Vec<usize>,
    /// Ratio of the mean integrated-energy peak to the energy background; ≥ 5 for a clean ECG,
    /// about 1–2 for noise.
    pub peak_to_background: f32,
}

/// Detect QRS complexes in `sig` (mV, oldest first, already de-spiked).
///
/// Returns no peaks when the lead holds only noise or a flat line, so callers never report a
/// heart rate from noise.
#[must_use]
pub fn detect(sig: &[f32], fs: f32) -> QrsDetection {
    let none = QrsDetection {
        peaks: Vec::new(),
        peak_to_background: 0.0,
    };
    let n = sig.len();
    if n < (0.8 * fs) as usize || fs < 100.0 {
        return none;
    }
    let bp = dsp::bandpass_5_15(sig, fs);
    let d = dsp::derivative_per_ms(&bp, fs);
    let sq: Vec<f32> = d.iter().map(|v| v * v).collect();
    let mwi = dsp::moving_average(&sq, (INTEGRATION_MS * 1e-3 * fs) as usize);

    // Energy background: the lower quartile of the integrated signal.
    let mut sorted = mwi.clone();
    sorted.sort_by(f32::total_cmp);
    let background = sorted[n / 4].max(1e-12);

    let refractory = (REFRACTORY_MS * 1e-3 * fs) as usize;
    let guard = (T_WAVE_GUARD_MS * 1e-3 * fs) as usize;
    // Local maxima, strongest first within each refractory neighbourhood.
    let mut cand: Vec<usize> = (1..n - 1)
        .filter(|&i| mwi[i] > mwi[i - 1] && mwi[i] >= mwi[i + 1])
        .collect();
    cand.retain(|&i| {
        let lo = i.saturating_sub(refractory);
        let hi = (i + refractory).min(n - 1);
        mwi[lo..=hi].iter().all(|v| *v <= mwi[i])
    });

    // Adaptive thresholds, seeded from the strongest peak of the first 2.5 s.
    let seed_end = (2.5 * fs) as usize;
    let seed = cand
        .iter()
        .filter(|&&i| i < seed_end)
        .map(|&i| mwi[i])
        .fold(0.0_f32, f32::max)
        .max(cand.iter().map(|&i| mwi[i]).fold(0.0_f32, f32::max) * 0.5);
    let (mut spki, mut npki) = (seed, 0.0_f32);
    let mut found: Vec<usize> = Vec::new();
    let mut rr_avg: Option<f32> = None;
    let mut last_noise: Vec<usize> = Vec::new();
    for &i in &cand {
        let thr = npki + 0.25 * (spki - npki);
        // Search-back: a long gap means a beat was probably missed below the threshold.
        if let (Some(&last), Some(rr)) = (found.last(), rr_avg)
            && (i - last) as f32 > 1.66 * rr
            && let Some(&best) = last_noise
                .iter()
                .filter(|&&j| j > last + refractory && j + refractory < i && mwi[j] > 0.5 * thr)
                .max_by(|&&a, &&b| mwi[a].total_cmp(&mwi[b]))
        {
            found.push(best);
            spki = 0.25 * mwi[best] + 0.75 * spki;
        }
        let too_close_weak = found
            .last()
            .is_some_and(|&l| i - l < guard && mwi[i] < 0.5 * spki);
        if mwi[i] > thr && !too_close_weak {
            if let Some(&l) = found.last() {
                let rr = (i - l) as f32;
                rr_avg = Some(rr_avg.map_or(rr, |a| 0.875 * a + 0.125 * rr));
            }
            found.push(i);
            spki = 0.125 * mwi[i] + 0.875 * spki;
            last_noise.clear();
        } else {
            npki = 0.125 * mwi[i] + 0.875 * npki;
            last_noise.push(i);
        }
    }
    found.sort_unstable();

    if found.is_empty() {
        return none;
    }
    let mean_peak = found.iter().map(|&i| mwi[i]).sum::<f32>() / found.len() as f32;
    let peak_to_background = mean_peak / background;
    if peak_to_background < MIN_PEAK_TO_BACKGROUND {
        return QrsDetection {
            peaks: Vec::new(),
            peak_to_background,
        };
    }

    QrsDetection {
        peaks: refine_peaks(sig, &found, fs),
        peak_to_background,
    }
}

/// Refine candidates on the raw lead: R = largest excursion from the local baseline within ±80 ms.
#[must_use]
pub fn refine_peaks(sig: &[f32], candidates: &[usize], fs: f32) -> Vec<usize> {
    let n = sig.len();
    let refractory = (REFRACTORY_MS * 1e-3 * fs) as usize;
    let base = dsp::median(sig).unwrap_or(0.0);
    let reach = (0.08 * fs) as usize;
    let mut peaks: Vec<usize> = Vec::with_capacity(candidates.len());
    for &c in candidates {
        let lo = c.saturating_sub(reach);
        let hi = (c + reach).min(n - 1);
        let (mut best, mut best_dev) = (c, 0.0_f32);
        for (k, v) in sig.iter().enumerate().take(hi + 1).skip(lo) {
            let dev = (v - base).abs();
            if dev > best_dev {
                best = k;
                best_dev = dev;
            }
        }
        let pp = sig[lo..=hi].iter().copied().fold(f32::MIN, f32::max)
            - sig[lo..=hi].iter().copied().fold(f32::MAX, f32::min);
        if pp >= MIN_QRS_PP_MV && peaks.last().is_none_or(|&p| best > p + refractory / 2) {
            peaks.push(best);
        }
    }
    peaks
}
