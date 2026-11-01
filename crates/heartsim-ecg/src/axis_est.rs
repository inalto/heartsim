//! Measuring the frontal QRS axis from recorded leads, the way a clinician does it: find the QRS
//! complexes, take the net (area) deflection of leads I and aVF inside them, and combine them as
//! `atan2(aVF, I)`. I and aVF are orthogonal, so no further correction is needed.

use heartsim_core::FrontalAxis;

/// Estimate the frontal QRS axis from equally sampled leads I and aVF (oldest sample first).
///
/// QRS samples are those where the combined slope `|dI| + |daVF|` exceeds a fraction of its
/// maximum (the QRS is by far the steepest part of the cycle), widened by `pad_ms` on each side
/// so the whole complex is integrated while the much slower P and T waves are left out. Returns
/// `None` when the record has no clear complexes (asystole, flat line).
#[must_use]
pub fn estimate_frontal_axis(lead_i: &[f32], lead_avf: &[f32], sample_rate_hz: f32) -> Option<FrontalAxis> {
    let n = lead_i.len().min(lead_avf.len());
    if n < 8 || sample_rate_hz <= 0.0 {
        return None;
    }
    let slope: Vec<f32> = (1..n)
        .map(|k| (lead_i[k] - lead_i[k - 1]).abs() + (lead_avf[k] - lead_avf[k - 1]).abs())
        .collect();
    let peak = slope.iter().copied().fold(0.0_f32, f32::max);
    // Below ~0.2 mV/s of steepest slope there is no QRS worth measuring.
    if peak * sample_rate_hz < 0.2 {
        return None;
    }
    let threshold = 0.25 * peak;
    let pad = (0.035 * sample_rate_hz).ceil() as usize;
    let mut in_qrs = vec![false; n];
    for (k, s) in slope.iter().enumerate() {
        if *s >= threshold {
            let lo = k.saturating_sub(pad);
            let hi = (k + 1 + pad).min(n - 1);
            in_qrs[lo..=hi].iter_mut().for_each(|b| *b = true);
        }
    }
    // Baseline: median of the samples outside the complexes (the isoelectric segments).
    let baseline = |x: &[f32]| {
        let mut rest: Vec<f32> = x
            .iter()
            .zip(&in_qrs)
            .filter(|(_, q)| !**q)
            .map(|(v, _)| *v)
            .collect();
        if rest.is_empty() {
            return 0.0;
        }
        rest.sort_by(f32::total_cmp);
        rest[rest.len() / 2]
    };
    let (bi, bf) = (baseline(&lead_i[..n]), baseline(&lead_avf[..n]));
    let (mut ai, mut af) = (0.0_f64, 0.0_f64);
    for k in 0..n {
        if in_qrs[k] {
            ai += f64::from(lead_i[k] - bi);
            af += f64::from(lead_avf[k] - bf);
        }
    }
    if ai.hypot(af) < 1e-6 {
        return None;
    }
    Some(FrontalAxis::from_i_avf(ai, af))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GaussianSynth;
    use heartsim_core::{Lead, Millis};
    use heartsim_sim::{Engine, TimingEngine, TimingParams};

    fn record(shift: f64, seconds: f64) -> (GaussianSynth, Vec<f32>, Vec<f32>) {
        let mut eng = TimingEngine::new(TimingParams::default());
        let mut syn = GaussianSynth::textbook();
        syn.axis_shift_deg = shift;
        let (mut i, mut avf) = (Vec::new(), Vec::new());
        let mut acc = 0.0;
        for _ in 0..(seconds * 1000.0) as usize {
            eng.step(Millis(1.0));
            syn.on_events(&eng.drain_events());
            acc += 1.0;
            if acc >= 2.0 {
                acc -= 2.0;
                let s = syn.sample(eng.now());
                i.push(s.get(Lead::I) as f32);
                avf.push(s.get(Lead::AVF) as f32);
            }
        }
        (syn, i, avf)
    }

    #[test]
    fn measured_axis_matches_the_waves_for_any_rotation() {
        for shift in [-100.0, -60.0, -20.0, 0.0, 30.0, 75.0, 120.0] {
            let (syn, i, avf) = record(shift, 6.0);
            let want = syn.qrs_axis_deg();
            let got = estimate_frontal_axis(&i[500..], &avf[500..], 500.0)
                .expect("axis")
                .degrees;
            let err = FrontalAxis::normalise(got - want).abs();
            assert!(err < 12.0, "shift {shift}: wanted {want:.1}, measured {got:.1}");
        }
    }

    #[test]
    fn textbook_qrs_axis_is_normal() {
        let a = GaussianSynth::textbook().qrs_axis_deg();
        assert!((30.0..=90.0).contains(&a), "{a}");
    }

    #[test]
    fn flat_line_has_no_axis() {
        assert!(estimate_frontal_axis(&[0.0; 500], &[0.0; 500], 500.0).is_none());
    }
}
