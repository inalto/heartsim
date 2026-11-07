//! Real-time ECG measurement: QRS/P/T detection, intervals, QTc, axes, rhythm regularity,
//! wave annotations and the 3-D heart vector.
//!
//! The analyser works on a recorded window of the 12 leads (typically the last 10 s of the
//! strip rings) and is a pure function of that window: no state, no I/O, deterministic. The
//! stages are
//!
//! 1. de-spike pacing artefacts (< 5 ms) on every lead,
//! 2. Pan–Tompkins QRS detection on the reference lead ([`qrs`]),
//! 3. spatial delineation: QRS onset/offset from the 12-lead *spatial velocity* (slope
//!    threshold) and P/T boundaries from the heart-vector magnitude (return to baseline), so a
//!    wave that is isoelectric in one lead is still found,
//! 4. median measurements over the last [`AnalysisConfig::max_beats`] beats.
//!
//! Wave boundary conventions (documented because "onset" is a convention, not a physical fact):
//! QRS onset/offset are where the spatial velocity has fallen to
//! [`AnalysisConfig::qrs_tail_fraction`] of its maximum (or stopped falling); P/T onset/offset are where the
//! smoothed heart-vector magnitude falls below [`AnalysisConfig::wave_end_fraction`] of the
//! wave's peak. QT runs from QRS onset to T offset.

pub mod dsp;
pub mod heart_vector;
pub mod marks;
pub mod qrs;
pub mod rhythm;

use crate::axis_est;
use crate::leadvec::LeadVectors;
use crate::ring::LeadRing;
use glam::DVec3;
pub use heart_vector::HeartVectorSolver;
use heartsim_core::{AxisClass, FrontalAxis, LEAD_COUNT, Lead, RhythmKind};
pub use marks::{WaveKind, WaveMark};

/// Onset, extremum and offset of one P or T wave, as sample indices in the analysed window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaveBounds {
    /// First sample of the wave.
    pub onset: usize,
    /// Sample of the largest heart-vector magnitude.
    pub peak: usize,
    /// Last sample of the wave.
    pub offset: usize,
}

/// One delineated beat. Indices are sample positions in the analysed window (oldest = 0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Beat {
    /// R peak of the reference lead.
    pub r_peak: usize,
    /// QRS onset (earliest of all leads).
    pub qrs_onset: usize,
    /// QRS offset (J point, latest of all leads).
    pub qrs_offset: usize,
    /// P wave, if one was found before this QRS.
    pub p: Option<WaveBounds>,
    /// T wave, if one was found and its end could be located.
    pub t: Option<WaveBounds>,
    /// Interval from the previous R peak, ms (`None` for the first beat).
    pub rr_before_ms: Option<f64>,
}

/// Regularity of the ventricular rhythm over the measured beats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Regularity {
    /// Fewer than three RR intervals: cannot tell.
    Unknown,
    /// RR intervals vary less than [`AnalysisConfig::irregular_cv`].
    Regular,
    /// "Irregularly irregular" or otherwise erratic RR (AF-like).
    Irregular,
}

/// How trustworthy the window is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalQuality {
    /// Flat line or sub-threshold signal.
    NoSignal,
    /// Signal present but no QRS complexes stand out of the background (noise, VF-like).
    NoQrs,
    /// QRS complexes found.
    Good,
}

/// Tunable detector settings. The defaults are the validated ones.
#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisConfig {
    /// Lead vectors for the heart-vector inverse.
    pub lead_vectors: LeadVectors,
    /// Number of most recent beats the medians are taken over.
    pub max_beats: usize,
    /// QRS core: fraction of the peak spatial velocity that is certainly inside the complex.
    pub qrs_slope_fraction: f32,
    /// QRS tails: the boundary then extends outwards while the velocity keeps falling, down to this
    /// fraction of its peak, so the slowly decaying end of the S wave is included.
    pub qrs_tail_fraction: f32,
    /// P/T boundary: fraction of the wave's peak magnitude.
    pub wave_end_fraction: f32,
    /// Coefficient of variation of RR above which the rhythm is called irregular.
    pub irregular_cv: f64,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            lead_vectors: LeadVectors::standard(),
            max_beats: 8,
            qrs_slope_fraction: 0.03,
            qrs_tail_fraction: 0.005,
            wave_end_fraction: 0.05,
            irregular_cv: 0.10,
        }
    }
}

/// Everything measured in one window. Optional fields are `None` when they cannot be measured
/// reliably (never a guess).
#[derive(Debug, Clone)]
pub struct Analysis {
    /// Sampling rate of the analysed window, Hz.
    pub sample_rate_hz: f32,
    /// Number of samples analysed per lead.
    pub samples: usize,
    /// Window quality.
    pub quality: SignalQuality,
    /// Lead the QRS detector ran on.
    pub reference_lead: Lead,
    /// Ventricular rate from the mean RR of the last beats, beats/min.
    pub hr_bpm: Option<f64>,
    /// Mean RR interval, ms.
    pub rr_ms: Option<f64>,
    /// Rhythm regularity.
    pub regularity: Regularity,
    /// Median PR interval (P onset → QRS onset), ms. `None` when P waves are absent.
    pub pr_ms: Option<f64>,
    /// Whether a P wave precedes most (≥ 60 %) of the measured QRS complexes.
    pub p_wave_present: bool,
    /// Median QRS duration, ms.
    pub qrs_ms: Option<f64>,
    /// Median QT (QRS onset → T offset), ms.
    pub qt_ms: Option<f64>,
    /// Median Bazett-corrected QT, `QT / √RR`, ms.
    pub qtc_bazett_ms: Option<f64>,
    /// Median Fridericia-corrected QT, `QT / ∛RR`, ms.
    pub qtc_fridericia_ms: Option<f64>,
    /// Frontal QRS axis from the net QRS area of leads I and aVF.
    pub qrs_axis: Option<FrontalAxis>,
    /// Frontal T axis from the net T-wave area of leads I and aVF.
    pub t_axis: Option<FrontalAxis>,
    /// All delineated beats of the window (oldest first), for annotation.
    pub beats: Vec<Beat>,
    /// P/Q/R/S/T marks of every beat on the reference lead.
    pub marks: Vec<WaveMark>,
    /// Heart-vector loop over the last complete beat (mV, heart coordinates).
    pub heart_vector: Vec<DVec3>,
    /// Window index of the first `heart_vector` sample.
    pub heart_vector_start: usize,
    /// The rhythm, for the few the waveform alone names (see [`rhythm`]); `None` otherwise.
    pub rhythm: Option<RhythmKind>,
    /// Atrial rate of flutter, per minute.
    pub atrial_rate_bpm: Option<f64>,
    /// Fixed AV ratio of flutter (2 = 2:1); `None` when the block varies or for other rhythms.
    pub av_ratio: Option<u32>,
    /// Dominant frequency of a polymorphic or fibrillating ventricular rhythm, Hz.
    pub dominant_frequency_hz: Option<f64>,
}

impl Analysis {
    /// Clinical class of the QRS axis.
    #[must_use]
    pub fn qrs_axis_class(&self) -> Option<AxisClass> {
        self.qrs_axis.map(|a| a.classify())
    }

    fn empty(fs: f32, samples: usize, quality: SignalQuality, reference_lead: Lead) -> Self {
        Self {
            sample_rate_hz: fs,
            samples,
            quality,
            reference_lead,
            hr_bpm: None,
            rr_ms: None,
            regularity: Regularity::Unknown,
            pr_ms: None,
            p_wave_present: false,
            qrs_ms: None,
            qt_ms: None,
            qtc_bazett_ms: None,
            qtc_fridericia_ms: None,
            qrs_axis: None,
            t_axis: None,
            beats: Vec::new(),
            marks: Vec::new(),
            heart_vector: Vec::new(),
            heart_vector_start: 0,
            rhythm: None,
            atrial_rate_bpm: None,
            av_ratio: None,
            dominant_frequency_hz: None,
        }
    }
}

/// The analyser. Cheap to construct; holds only configuration and the heart-vector inverse.
#[derive(Debug, Clone)]
pub struct Analyzer {
    config: AnalysisConfig,
    solver: Option<HeartVectorSolver>,
}

impl Default for Analyzer {
    fn default() -> Self {
        Self::new(AnalysisConfig::default())
    }
}

/// Window used by [`Analyzer::analyze_rings`], seconds.
pub const DEFAULT_WINDOW_S: f32 = 10.0;

fn ms(samples: usize, fs: f32) -> f64 {
    samples as f64 * 1000.0 / f64::from(fs)
}

fn samples_of(ms: f32, fs: f32) -> usize {
    (ms * 1e-3 * fs).round() as usize
}

fn median_f64(v: &mut [f64]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    let m = v.len() / 2;
    Some(if v.len() % 2 == 1 {
        v[m]
    } else {
        v[m - 1].midpoint(v[m])
    })
}

/// `p`-quantile (0..1) of `x` without sorting everything.
fn quantile(x: &[f32], p: f32) -> f32 {
    if x.is_empty() {
        return 0.0;
    }
    let mut v = x.to_vec();
    let k = ((v.len() - 1) as f32 * p) as usize;
    *v.select_nth_unstable_by(k, f32::total_cmp).1
}

impl Analyzer {
    /// Analyser with the given configuration.
    #[must_use]
    pub fn new(config: AnalysisConfig) -> Self {
        let solver = HeartVectorSolver::new(&config.lead_vectors);
        Self { config, solver }
    }

    /// Analyse the newest [`DEFAULT_WINDOW_S`] seconds of twelve rings (indexed by `Lead as usize`).
    #[must_use]
    pub fn analyze_rings(&self, rings: &[LeadRing; LEAD_COUNT]) -> Analysis {
        let fs = rings[0].sample_rate_hz;
        let want = (DEFAULT_WINDOW_S * fs) as usize;
        let mut bufs: Vec<Vec<f32>> = vec![Vec::new(); LEAD_COUNT];
        for (r, b) in rings.iter().zip(&mut bufs) {
            r.latest(want, b);
        }
        let refs: [&[f32]; LEAD_COUNT] = std::array::from_fn(|i| bufs[i].as_slice());
        self.analyze(&refs, fs)
    }

    /// Analyse twelve equally sampled leads (mV, oldest first, indexed by `Lead as usize`).
    #[must_use]
    pub fn analyze(&self, leads: &[&[f32]; LEAD_COUNT], fs: f32) -> Analysis {
        let cfg = &self.config;
        let n = leads.iter().map(|l| l.len()).min().unwrap_or(0);
        let reference_guess = Lead::II;
        if n < (1.5 * fs) as usize || fs < 100.0 {
            return Analysis::empty(fs, n, SignalQuality::NoSignal, reference_guess);
        }
        // 1. Pacing-spike removal on every lead.
        let raw: Vec<Vec<f32>> = leads.iter().map(|l| dsp::despike(&l[..n], fs)).collect();

        // 2. Reference lead: II unless another lead carries clearly more QRS energy.
        let energy: Vec<f32> = raw
            .iter()
            .map(|x| x.windows(2).map(|w| (w[1] - w[0]).powi(2)).sum::<f32>())
            .collect();
        let emax = energy.iter().copied().fold(0.0_f32, f32::max);
        let ref_idx = if energy[Lead::II as usize] >= 0.5 * emax {
            Lead::II as usize
        } else {
            energy
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .map_or(1, |(i, _)| i)
        };
        let ref_lead = Lead::ALL[ref_idx];

        // Polymorphic and fibrillating rhythms have no stable complex to delineate.
        if let Some(v) = rhythm::ventricular_chaos(&raw, fs) {
            let quality = if v.rhythm == RhythmKind::VentricularFibrillation {
                SignalQuality::NoQrs
            } else {
                SignalQuality::Good
            };
            let mut a = Analysis::empty(fs, n, quality, ref_lead);
            a.rhythm = Some(v.rhythm);
            a.dominant_frequency_hz = Some(v.dominant_hz);
            if v.rhythm != RhythmKind::VentricularFibrillation {
                a.hr_bpm = Some(v.rate_bpm);
                a.rr_ms = Some(60_000.0 / v.rate_bpm);
                a.regularity = v.regularity;
            }
            return a;
        }

        let mut det = qrs::detect(&raw[ref_idx], fs);
        if det.peaks.is_empty() {
            let q = if emax * fs * fs < 1e-4 {
                SignalQuality::NoSignal
            } else {
                SignalQuality::NoQrs
            };
            return Analysis::empty(fs, n, q, ref_lead);
        }
        // One lead's energy threshold can hide a run of broad beats behind a few narrow, much
        // steeper ones: pre-excited AF is such a mixture. All 12 leads together still see them.
        let deflections = rhythm::qrs_sized(&raw, &rhythm::deflections(&raw, fs), &det.peaks, fs);
        if deflections.len() >= rhythm::MIN_DEFLECTIONS && det.peaks.len() * 5 < deflections.len() {
            det.peaks = qrs::refine_peaks(&raw[ref_idx], &deflections, fs);
        }

        // 3. Spatial signals.
        let smooth_w = samples_of(10.0, fs).max(1) | 1;
        let sm: Vec<Vec<f32>> = raw.iter().map(|x| dsp::moving_average(x, smooth_w)).collect();
        let vel: Vec<f32> = {
            let d: Vec<Vec<f32>> = sm.iter().map(|x| dsp::derivative_per_ms(x, fs)).collect();
            (0..n)
                .map(|i| d.iter().map(|l| l[i] * l[i]).sum::<f32>().sqrt())
                .collect()
        };
        // Isoelectric reference per lead: median over the quieter half of the window.
        let vel_med = quantile(&vel, 0.5);
        let base: Vec<f32> = sm
            .iter()
            .map(|x| {
                let quiet: Vec<f32> = x
                    .iter()
                    .zip(&vel)
                    .filter(|(_, v)| **v <= vel_med)
                    .map(|(a, _)| *a)
                    .collect();
                dsp::median(&quiet).unwrap_or(0.0)
            })
            .collect();
        // Heart-vector magnitude (needs the 3-D inverse; falls back to the largest |lead|).
        let hv: Vec<DVec3> = (0..n)
            .map(|i| {
                let v: [f64; LEAD_COUNT] = std::array::from_fn(|l| f64::from(sm[l][i] - base[l]));
                self.solver.map_or(DVec3::ZERO, |s| s.solve(&v))
            })
            .collect();
        // P and T are slow: smooth the dipole components (not its magnitude) over 30 ms and remove
        // the residual noise in quadrature, |d|² − E|noise|², which stays unbiased at low SNR.
        let w_pt = samples_of(30.0, fs).max(1) | 1;
        let comp = |f: fn(&DVec3) -> f64| -> Vec<f32> {
            let c: Vec<f32> = hv.iter().map(|v| f(v) as f32).collect();
            dsp::moving_average(&c, w_pt)
        };
        let (cx, cy, cz) = (comp(|v| v.x), comp(|v| v.y), comp(|v| v.z));
        let sigma2: f32 = raw.iter().map(|x| dsp::white_noise_sigma(x).powi(2)).sum();
        let w1 = smooth_w as f32;
        let w2 = w_pt as f32;
        let gain = self.solver.map_or(1.0, |s| s.noise_gain()) as f32;
        let var_pt = gain * gain * (sigma2 / LEAD_COUNT as f32) * (w2 - w1 / 3.0).max(1.0) / (w2 * w2);
        // Standard deviation of one smoothed dipole component: the hysteresis of the P/T boundaries.
        let noise_m = var_pt.sqrt();
        let mag_sm: Vec<f32> = (0..n)
            .map(|i| {
                (cx[i] * cx[i] + cy[i] * cy[i] + cz[i] * cz[i] - 3.0 * var_pt)
                    .max(0.0)
                    .sqrt()
            })
            .collect();
        // RMS noise of the spatial velocity (12 independent central differences of an MA(w1)).
        let noise_v = sigma2.sqrt() * 2.0 * fs / (2000.0 * w1);
        let vel: Vec<f32> = vel
            .iter()
            .map(|v| (v * v - noise_v * noise_v).max(0.0).sqrt())
            .collect();

        // 4. Delineation.
        let mut beats = self.delineate_qrs(&det.peaks, &vel, noise_v, 0.0, fs, n);
        self.delineate_t(&mut beats, &mag_sm, noise_m, fs, n);
        self.delineate_p(&mut beats, &mag_sm, noise_m, fs);

        // 5. Measurements over the last beats.
        let mut a = Analysis::empty(fs, n, SignalQuality::Good, ref_lead);
        let k = cfg.max_beats.max(2);
        let last: &[Beat] = &beats[beats.len().saturating_sub(k + 1)..];
        let rr: Vec<f64> = last.iter().filter_map(|b| b.rr_before_ms).collect();
        if last.len() >= 2 {
            let span = ms(last[last.len() - 1].r_peak - last[0].r_peak, fs);
            let mean_rr = span / (last.len() - 1) as f64;
            a.rr_ms = Some(mean_rr);
            a.hr_bpm = Some(60_000.0 / mean_rr);
        }
        if rr.len() >= 3 {
            let mean = rr.iter().sum::<f64>() / rr.len() as f64;
            let var = rr.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / rr.len() as f64;
            a.regularity = if var.sqrt() / mean > cfg.irregular_cv {
                Regularity::Irregular
            } else {
                Regularity::Regular
            };
        }
        let qrs_of = |beats: &[Beat]| {
            median_f64(
                &mut beats[beats.len().saturating_sub(k)..]
                    .iter()
                    .map(|b| ms(b.qrs_offset - b.qrs_onset, fs))
                    .collect::<Vec<_>>(),
            )
        };
        // The atria first: flutter and f waves are not P waves, and the QRS has to be told from
        // them, not from a flat line.
        let found = rhythm::atrial_activity(&raw, &beats, fs, a.regularity, a.hr_bpm);
        if found.is_some() || a.regularity == Regularity::Irregular {
            let floor = rhythm::baseline_velocity(&vel, &beats, fs);
            let mut remeasured = self.delineate_qrs(&det.peaks, &vel, noise_v, floor, fs, n);
            let excited = rhythm::pre_excited(a.regularity, a.hr_bpm, qrs_of(&remeasured));
            if let Some(f) = excited.or(found) {
                self.delineate_t(&mut remeasured, &mag_sm, noise_m, fs, n);
                beats = remeasured;
                a.rhythm = Some(f.rhythm);
                a.atrial_rate_bpm = f.atrial_rate_bpm;
                a.av_ratio = f.av_ratio;
                // Flutter waves leave no T end to find.
                if f.rhythm == RhythmKind::AtrialFlutter {
                    for b in &mut beats {
                        b.t = None;
                    }
                }
            }
        }
        let measured_from = beats.len().saturating_sub(k);
        let measured = &beats[measured_from..];
        a.qrs_ms = qrs_of(&beats);
        let with_p: Vec<&Beat> = measured.iter().filter(|b| b.p.is_some()).collect();
        a.p_wave_present = !measured.is_empty() && with_p.len() * 5 >= measured.len() * 3;
        if a.p_wave_present {
            a.pr_ms = median_f64(
                &mut with_p
                    .iter()
                    .filter_map(|b| b.p.map(|p| ms(b.qrs_onset - p.onset, fs)))
                    .collect::<Vec<_>>(),
            );
        }
        let (mut qt, mut bz, mut fr) = (Vec::new(), Vec::new(), Vec::new());
        for b in measured {
            if let (Some(t), Some(rr_ms)) = (b.t, b.rr_before_ms) {
                let q = ms(t.offset - b.qrs_onset, fs);
                let rr_s = rr_ms / 1000.0;
                qt.push(q);
                bz.push(q / rr_s.sqrt());
                fr.push(q / rr_s.cbrt());
            }
        }
        a.qt_ms = median_f64(&mut qt);
        a.qtc_bazett_ms = median_f64(&mut bz);
        a.qtc_fridericia_ms = median_f64(&mut fr);

        let (li, lf) = (Lead::I as usize, Lead::AVF as usize);
        let qrs_win: Vec<(usize, usize)> = measured.iter().map(|b| (b.qrs_onset, b.qrs_offset)).collect();
        a.qrs_axis = axis_est::net_area_axis(&raw[li], &raw[lf], &qrs_win);
        let t_win: Vec<(usize, usize)> = measured
            .iter()
            .filter_map(|b| b.t.map(|t| (t.onset, t.offset)))
            .collect();
        a.t_axis = axis_est::net_area_axis(&raw[li], &raw[lf], &t_win);

        // 6. Annotations and heart-vector loop.
        a.marks = marks::marks_for_lead(ref_lead, &raw[ref_idx], &beats);
        if self.solver.is_some()
            && let Some(b) = beats
                .iter()
                .rev()
                .find(|b| b.t.is_some() || b.rr_before_ms.is_some())
        {
            let start =
                b.p.map_or(b.qrs_onset.saturating_sub(samples_of(40.0, fs)), |p| p.onset);
            let end =
                b.t.map_or((b.qrs_offset + samples_of(100.0, fs)).min(n - 1), |t| t.offset);
            a.heart_vector_start = start;
            a.heart_vector = hv[start..=end.max(start)].to_vec();
        }
        a.beats = beats;
        a
    }

    /// QRS onset/offset from the spatial velocity: walk outwards from the R peak until the
    /// velocity stays below the threshold for 12 ms.
    #[allow(clippy::cast_possible_wrap)]
    fn delineate_qrs(
        &self,
        peaks: &[usize],
        vel: &[f32],
        noise_v: f32,
        floor: f32,
        fs: f32,
        n: usize,
    ) -> Vec<Beat> {
        let hold = samples_of(12.0, fs).max(1);
        let core = samples_of(60.0, fs);
        let reach_l = samples_of(150.0, fs);
        let reach_r = samples_of(170.0, fs);
        let mut beats = Vec::with_capacity(peaks.len());
        for (k, &r) in peaks.iter().enumerate() {
            let lo_lim = if k > 0 { peaks[k - 1].midpoint(r) } else { 0 }.max(r.saturating_sub(reach_l));
            let hi_lim = if k + 1 < peaks.len() {
                peaks[k + 1].midpoint(r)
            } else {
                n - 1
            }
            .min(r + reach_r);
            let lo_core = r.saturating_sub(core);
            let hi_core = (r + core).min(n - 1);
            let smax = vel[lo_core..=hi_core].iter().copied().fold(0.0_f32, f32::max);
            let thr = (self.config.qrs_slope_fraction * smax)
                .max(0.4 * noise_v)
                .max(floor);
            let tail_thr = (self.config.qrs_tail_fraction * smax)
                .max(0.4 * noise_v)
                .max(floor);
            let in_range = |i: i64, dir: i64, limit: usize| {
                i >= 0
                    && (i as usize) < n
                    && (if dir < 0 {
                        i as usize >= limit
                    } else {
                        i as usize <= limit
                    })
            };
            let walk = |dir: i64, limit: usize| {
                // Core: outwards until the velocity stays below `thr` for `hold` samples.
                let mut edge = r;
                let mut run = 0;
                let mut i = r as i64;
                while in_range(i, dir, limit) {
                    if vel[i as usize] >= thr {
                        edge = i as usize;
                        run = 0;
                    } else {
                        run += 1;
                        if run >= hold {
                            break;
                        }
                    }
                    i += dir;
                }
                // Tail: keep going while the velocity is still decaying towards the baseline.
                let mut i = edge as i64 + dir;
                while in_range(i, dir, limit) {
                    let (cur, prev) = (vel[i as usize], vel[(i - dir) as usize]);
                    if cur < tail_thr || cur > prev {
                        break;
                    }
                    edge = i as usize;
                    i += dir;
                }
                edge
            };
            let onset = walk(-1, lo_lim);
            let offset = walk(1, hi_lim);
            // Beats whose QRS touches the window edge are partial: skip them.
            if onset < 2 || offset + 2 >= n || offset <= onset {
                continue;
            }
            let rr_before_ms = beats.last().map(|b: &Beat| ms(r - b.r_peak, fs));
            beats.push(Beat {
                r_peak: r,
                qrs_onset: onset,
                qrs_offset: offset,
                p: None,
                t: None,
                rr_before_ms,
            });
        }
        beats
    }

    /// Walk from `from` towards `limit` (either direction) until `mag` falls below `thr`.
    /// `None` when the limit is reached first.
    fn walk_to_baseline(mag: &[f32], from: usize, limit: usize, thr: f32) -> Option<usize> {
        if limit >= from {
            (from..=limit.min(mag.len() - 1)).find(|&i| mag[i] < thr)
        } else {
            (limit..=from).rev().find(|&i| mag[i] < thr)
        }
    }

    /// T wave: largest smoothed magnitude between R + 120 ms and min(R + 450 ms, 0.7·RR), ended by
    /// the return to baseline (or, if the next P wave rises first, by the valley before it).
    fn delineate_t(&self, beats: &mut [Beat], mag: &[f32], noise_m: f32, fs: f32, n: usize) {
        let t_min = (6.0 * noise_m).max(0.04);
        let edge = samples_of(10.0, fs).max(2);
        for k in 0..beats.len() {
            let r = beats[k].r_peak;
            let next_onset = beats.get(k + 1).map(|b| b.qrs_onset);
            let rr = match (next_onset, beats[k].rr_before_ms) {
                (Some(_), _) => ms(beats[k + 1].r_peak - r, fs),
                (None, Some(prev)) => prev,
                (None, None) => 900.0,
            };
            let lo = r + samples_of(120.0, fs);
            let mut hi = r + samples_of((450.0_f64).min(0.7 * rr) as f32, fs);
            let hard = next_onset.map_or(n - 1, |o| o.saturating_sub(samples_of(30.0, fs)));
            hi = hi.min(hard).min(n - 1);
            if hi <= lo + 2 * edge {
                continue;
            }
            let peak = (lo..=hi).max_by(|&a, &b| mag[a].total_cmp(&mag[b])).unwrap_or(lo);
            if mag[peak] < t_min || peak < lo + edge || peak + edge > hi {
                continue;
            }
            let thr = (self.config.wave_end_fraction * mag[peak]).max(3.0 * noise_m);
            let offset = Self::walk_to_baseline(mag, peak, hard, thr).or_else(|| {
                // The next P wave rises before the T tail has decayed: use the valley.
                let v = (peak..=hard).min_by(|&a, &b| mag[a].total_cmp(&mag[b]))?;
                (mag[v] < 0.5 * mag[peak] && v > peak + edge).then_some(v)
            });
            let onset = Self::walk_to_baseline(mag, peak, beats[k].qrs_offset, thr);
            if let (Some(on), Some(off)) = (onset, offset) {
                beats[k].t = Some(WaveBounds {
                    onset: on,
                    peak,
                    offset: off,
                });
            }
        }
    }

    /// P wave: largest smoothed magnitude in the 250 ms before QRS onset (after the previous T),
    /// accepted only as a genuine interior bump above the noise floor.
    fn delineate_p(&self, beats: &mut [Beat], mag: &[f32], noise_m: f32, fs: f32) {
        let p_min = (6.0 * noise_m).max(0.03);
        let edge = samples_of(10.0, fs).max(2);
        let gap = samples_of(15.0, fs);
        for k in 0..beats.len() {
            let on = beats[k].qrs_onset;
            let hi = on.saturating_sub(samples_of(40.0, fs));
            let mut lo = on.saturating_sub(samples_of(250.0, fs));
            if k > 0 {
                let prev = &beats[k - 1];
                lo = lo.max(
                    prev.t
                        .map_or(prev.qrs_offset + samples_of(60.0, fs), |t| t.offset),
                );
            }
            if hi <= lo + 2 * edge {
                continue;
            }
            let peak = (lo..=hi).max_by(|&a, &b| mag[a].total_cmp(&mag[b])).unwrap_or(lo);
            if mag[peak] < p_min || peak < lo + edge || peak + edge > hi {
                continue;
            }
            let thr = (self.config.wave_end_fraction * mag[peak]).max(3.0 * noise_m);
            let Some(p_on) = Self::walk_to_baseline(mag, peak, lo, thr) else {
                continue;
            };
            let p_off = Self::walk_to_baseline(mag, peak, on.saturating_sub(gap), thr)
                .unwrap_or(on.saturating_sub(gap));
            beats[k].p = Some(WaveBounds {
                onset: p_on,
                peak,
                offset: p_off.max(peak),
            });
        }
    }
}

#[cfg(test)]
mod tests;
