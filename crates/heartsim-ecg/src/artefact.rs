//! Seeded ECG artefacts for the display stage (requirement F-15): baseline wander, respiration,
//! mains hum and muscle tremor.
//!
//! Every artefact is a **pure function of `(seed, lead, sample index)`**: no state, no clock, no
//! global RNG. The same seed gives the same trace on every machine and in every run, scrubbing or
//! rebuilding a view reproduces exactly what was drawn before, and none of it ever reaches the
//! simulation, the analysis or (unless the user asks for the displayed trace) an export.
//!
//! **Amplitudes.** The reference amplitudes below (`intensity = 1`) are teaching values chosen
//! inside the ranges the literature reports; they are not measurements of a particular recording.
//! * Baseline wander: components below about 1 Hz, mostly 0.15–0.3 Hz, about 15 % of the
//!   peak-to-peak QRS (≈ 0.15 mV for a 1 mV complex). Reference: three tones in 0.15–0.30 Hz,
//!   about 0.15–0.2 mV peak-to-peak.
//! * Respiration: 12–20 breaths/min (0.2–0.33 Hz) moves the baseline and modulates the QRS
//!   amplitude by a few per cent. Reference: 0.1 mV peak-to-peak baseline swing and ±6 %
//!   amplitude modulation.
//! * Mains: a 50 or 60 Hz sinusoid, reported up to about half the peak-to-peak QRS. Reference:
//!   0.2 mV peak-to-peak (20 % of a 1 mV complex), the same on every lead up to a fixed per-lead
//!   gain.
//! * Muscle (EMG) tremor: broadband, mostly above 20 Hz, bursting with the contraction. Reference:
//!   25–150 Hz, independent on every lead, about 0.02 mV rms on average.
//!
//! Sources: Friesen GM et al., "A comparison of the noise sensitivity of nine QRS detection
//! algorithms", *IEEE Trans Biomed Eng* 1990;37:85–98 (baseline wander, 50/60 Hz and EMG noise
//! models); Moody GB, Muldrow WE, Mark RG, "A noise stress test for arrhythmia detectors",
//! *Computers in Cardiology* 1984;11:381–384 (baseline wander, electrode motion and muscle
//! artefact as the three noise classes); Chatterjee S et al., "Review of noise removal techniques
//! in ECG signals", *IET Signal Process* 2020;14:569–590 (frequency ranges and relative amplitudes
//! quoted above); Kligfield P et al., *Circulation* 2007;115:1306–1324 (why the diagnostic
//! bandwidth starts at 0.05 Hz: wander lives below it).

use core::f64::consts::TAU;
use heartsim_core::LEAD_COUNT;
use serde::{Deserialize, Serialize};

/// Mains frequency of the country the simulated recording is "made" in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MainsFrequency {
    /// 50 Hz (Europe, most of Asia and Africa).
    #[default]
    Hz50,
    /// 60 Hz (North America, parts of South America and Asia).
    Hz60,
}

impl MainsFrequency {
    /// Both choices, in menu order.
    pub const ALL: [MainsFrequency; 2] = [Self::Hz50, Self::Hz60];

    /// The frequency, Hz.
    #[must_use]
    pub const fn hz(self) -> f64 {
        match self {
            Self::Hz50 => 50.0,
            Self::Hz60 => 60.0,
        }
    }
}

/// Reference peak-to-peak baseline wander at `intensity = 1`, mV (sum of three tones).
pub const WANDER_TONE_MV: f64 = 0.04;
/// Reference respiratory baseline swing amplitude, mV (half of 0.1 mV peak-to-peak).
pub const RESPIRATION_TONE_MV: f64 = 0.05;
/// Reference respiratory modulation of the QRS amplitude (fraction of the signal).
pub const RESPIRATION_DEPTH: f64 = 0.06;
/// Reference mains amplitude, mV (0.2 mV peak-to-peak).
pub const MAINS_TONE_MV: f64 = 0.10;
/// Reference muscle tremor rms before the burst envelope, mV.
pub const MUSCLE_RMS_MV: f64 = 0.03;
/// Tones summed per lead to make the muscle tremor.
const MUSCLE_TONES: usize = 24;
/// Muscle tremor band, Hz.
pub const MUSCLE_BAND_HZ: (f64, f64) = (25.0, 150.0);
/// Baseline wander band, Hz.
pub const WANDER_BAND_HZ: (f64, f64) = (0.15, 0.30);
/// Respiration rate band, Hz (12–20 breaths/min).
pub const RESPIRATION_BAND_HZ: (f64, f64) = (0.20, 1.0 / 3.0);
/// Intensity range the user interface offers.
pub const INTENSITY_RANGE: (f32, f32) = (0.25, 2.0);

/// Which artefacts are added, and how strongly.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ArtefactConfig {
    /// Slow drift of the baseline (electrode contact, movement).
    pub baseline_wander: bool,
    /// Breathing: a baseline swing plus a small modulation of the QRS amplitude.
    pub respiration: bool,
    /// Power-line pick-up.
    pub mains: bool,
    /// 50 or 60 Hz.
    pub mains_hz: MainsFrequency,
    /// Muscle tremor.
    pub muscle: bool,
    /// Multiplies every artefact amplitude; 1 is the reference amplitude.
    pub intensity: f32,
    /// Seed of all phases and frequencies.
    pub seed: u64,
}

impl Default for ArtefactConfig {
    fn default() -> Self {
        Self {
            baseline_wander: false,
            respiration: false,
            mains: false,
            mains_hz: MainsFrequency::default(),
            muscle: false,
            intensity: 1.0,
            seed: 0x4845_4152_5453_494D, // "HEARTSIM"
        }
    }
}

impl ArtefactConfig {
    /// True when at least one artefact is switched on.
    #[must_use]
    pub fn any(&self) -> bool {
        self.baseline_wander || self.respiration || self.mains || self.muscle
    }

    /// The enabled artefacts in plain ASCII, e.g. `"baseline wander, 50 Hz mains (intensity 1.0, seed 7)"`,
    /// or `None` when none is on. Used in file headers and captions, so not translated.
    #[must_use]
    pub fn describe(&self) -> Option<String> {
        if !self.any() {
            return None;
        }
        let mut parts = Vec::new();
        if self.baseline_wander {
            parts.push("baseline wander".to_owned());
        }
        if self.respiration {
            parts.push("respiration".to_owned());
        }
        if self.mains {
            parts.push(format!("{:.0} Hz mains", self.mains_hz.hz()));
        }
        if self.muscle {
            parts.push("muscle tremor".to_owned());
        }
        Some(format!(
            "{} (intensity {:.1}, seed {})",
            parts.join(", "),
            self.intensity,
            self.seed
        ))
    }
}

/// SplitMix64 finaliser: a well-mixed 64-bit hash of its input.
fn mix(z: u64) -> u64 {
    let mut z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// A uniform number in `[0, 1)` from `(seed, stream, index)`.
fn unit(seed: u64, stream: u64, index: u64) -> f64 {
    let h = mix(mix(seed ^ stream.wrapping_mul(0xA24B_AED4_963E_E407)) ^ index);
    (h >> 11) as f64 / (1u64 << 53) as f64
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Tone {
    freq_hz: f64,
    amp_mv: f64,
    phase_rad: f64,
}

impl Tone {
    fn at(&self, t_s: f64) -> f64 {
        self.amp_mv * (TAU * self.freq_hz * t_s + self.phase_rad).sin()
    }
}

/// Streams of artefact voltage for the twelve leads. See the module documentation for the model.
#[derive(Debug, Clone, PartialEq)]
pub struct ArtefactGenerator {
    config: ArtefactConfig,
    wander: [Tone; 3],
    respiration: Tone,
    mains: Tone,
    /// `muscle[lead]` — independent tones per lead.
    muscle: Vec<[Tone; MUSCLE_TONES]>,
    burst_phase: [f64; LEAD_COUNT],
    lead_gain: [f64; LEAD_COUNT],
}

impl ArtefactGenerator {
    /// Draw all tones from `config.seed`.
    #[must_use]
    pub fn new(config: ArtefactConfig) -> Self {
        let seed = config.seed;
        let lerp = |(lo, hi): (f64, f64), u: f64| lo + (hi - lo) * u;
        let wander = std::array::from_fn(|k| Tone {
            freq_hz: lerp(WANDER_BAND_HZ, unit(seed, 1, k as u64)),
            amp_mv: WANDER_TONE_MV,
            phase_rad: TAU * unit(seed, 2, k as u64),
        });
        let respiration = Tone {
            freq_hz: lerp(RESPIRATION_BAND_HZ, unit(seed, 3, 0)),
            amp_mv: RESPIRATION_TONE_MV,
            phase_rad: TAU * unit(seed, 4, 0),
        };
        let mains = Tone {
            freq_hz: config.mains_hz.hz(),
            amp_mv: MAINS_TONE_MV,
            phase_rad: TAU * unit(seed, 5, 0),
        };
        // N equal-amplitude tones have rms a·sqrt(N/2)
        let muscle_amp = MUSCLE_RMS_MV * (2.0 / MUSCLE_TONES as f64).sqrt();
        let muscle = (0..LEAD_COUNT as u64)
            .map(|lead| {
                std::array::from_fn(|k| Tone {
                    freq_hz: lerp(MUSCLE_BAND_HZ, unit(seed, 100 + lead, k as u64)),
                    amp_mv: muscle_amp,
                    phase_rad: TAU * unit(seed, 200 + lead, k as u64),
                })
            })
            .collect();
        Self {
            config,
            wander,
            respiration,
            mains,
            muscle,
            burst_phase: std::array::from_fn(|l| TAU * unit(seed, 300, l as u64)),
            lead_gain: std::array::from_fn(|l| 0.6 + 0.4 * unit(seed, 400, l as u64)),
        }
    }

    /// The configuration the generator was built from.
    #[must_use]
    pub fn config(&self) -> &ArtefactConfig {
        &self.config
    }

    /// Artefact voltage to add to `lead` (index into `Lead::ALL`) at absolute sample `sample`, mV.
    #[must_use]
    pub fn additive_mv(&self, lead: usize, sample: u64, sample_rate_hz: f32) -> f32 {
        if !self.config.any() {
            return 0.0;
        }
        let lead = lead.min(LEAD_COUNT - 1);
        let t = sample as f64 / f64::from(sample_rate_hz);
        let intensity = f64::from(self.config.intensity);
        let shared = self.lead_gain[lead] * intensity;
        let mut v = 0.0;
        if self.config.baseline_wander {
            v += shared * self.wander.iter().map(|w| w.at(t)).sum::<f64>();
        }
        if self.config.respiration {
            v += shared * self.respiration.at(t);
        }
        if self.config.mains {
            v += shared * self.mains.at(t);
        }
        if self.config.muscle {
            // bursts: the tremor swells and fades about every 2.5 s, never fully silent
            let burst = 0.65 + 0.35 * (TAU * 0.4 * t + self.burst_phase[lead]).sin();
            let tremor: f64 = self.muscle[lead].iter().map(|m| m.at(t)).sum();
            v += intensity * burst * tremor;
        }
        v as f32
    }

    /// Factor to multiply the ECG voltage by at absolute sample `sample`: 1 unless respiration is
    /// on, then a small modulation at the breathing rate.
    #[must_use]
    pub fn gain(&self, sample: u64, sample_rate_hz: f32) -> f32 {
        if !self.config.respiration {
            return 1.0;
        }
        let t = sample as f64 / f64::from(sample_rate_hz);
        let phase = TAU * self.respiration.freq_hz * t + self.respiration.phase_rad;
        (1.0 + RESPIRATION_DEPTH * f64::from(self.config.intensity) * phase.sin()) as f32
    }

    /// `ecg_mv` as it would be acquired with the artefacts present.
    #[must_use]
    pub fn apply(&self, lead: usize, sample: u64, sample_rate_hz: f32, ecg_mv: f32) -> f32 {
        ecg_mv * self.gain(sample, sample_rate_hz) + self.additive_mv(lead, sample, sample_rate_hz)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::{BandFilter, FilterMode};

    const FS: f32 = 500.0;
    const MINUTE: u64 = 60 * 500;

    fn only(f: impl FnOnce(&mut ArtefactConfig)) -> ArtefactGenerator {
        let mut c = ArtefactConfig::default();
        f(&mut c);
        ArtefactGenerator::new(c)
    }

    fn series(g: &ArtefactGenerator, lead: usize, n: u64) -> Vec<f32> {
        (0..n).map(|i| g.additive_mv(lead, i, FS)).collect()
    }

    fn peak_to_peak(v: &[f32]) -> f32 {
        v.iter().copied().fold(f32::MIN, f32::max) - v.iter().copied().fold(f32::MAX, f32::min)
    }

    fn rms(v: &[f32]) -> f32 {
        (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt()
    }

    #[test]
    fn nothing_enabled_adds_nothing() {
        let g = ArtefactGenerator::new(ArtefactConfig::default());
        assert!(!g.config().any());
        assert_eq!(g.config().describe(), None);
        for i in 0..100 {
            assert_eq!(g.additive_mv(3, i, FS), 0.0);
            assert_eq!(g.gain(i, FS), 1.0);
            assert_eq!(g.apply(3, i, FS, 0.7), 0.7);
        }
    }

    #[test]
    fn same_seed_same_trace_other_seed_other_trace() {
        let cfg = ArtefactConfig {
            baseline_wander: true,
            respiration: true,
            mains: true,
            muscle: true,
            ..ArtefactConfig::default()
        };
        let a = series(&ArtefactGenerator::new(cfg), 2, 3000);
        let b = series(&ArtefactGenerator::new(cfg), 2, 3000);
        assert_eq!(a, b);
        let c = series(
            &ArtefactGenerator::new(ArtefactConfig {
                seed: cfg.seed + 1,
                ..cfg
            }),
            2,
            3000,
        );
        assert_ne!(a, c);
    }

    #[test]
    fn a_sample_does_not_depend_on_what_was_asked_before() {
        let g = only(|c| {
            c.muscle = true;
            c.mains = true;
        });
        let forward: Vec<f32> = (0..500).map(|i| g.additive_mv(5, i, FS)).collect();
        let backward: Vec<f32> = (0..500).rev().map(|i| g.additive_mv(5, i, FS)).collect();
        let mut backward = backward;
        backward.reverse();
        assert_eq!(forward, backward);
    }

    #[test]
    fn baseline_wander_is_slow_and_within_the_cited_amplitude() {
        let g = only(|c| c.baseline_wander = true);
        let v = series(&g, 0, 2 * MINUTE);
        let pp = peak_to_peak(&v);
        assert!((0.08..=0.30).contains(&pp), "peak-to-peak {pp} mV");
        // slow: a 0.5 Hz high-pass (the monitor corner) removes nearly all of it
        let mut hp = BandFilter::new(FilterMode::Monitor, FS);
        let after: Vec<f32> = v.iter().map(|&x| hp.process(x)).collect();
        assert!(rms(&after[2000..]) < 0.25 * rms(&v), "wander is not below 0.5 Hz");
        // and the sample-to-sample step is tiny (no spikes)
        let step = v.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0, f32::max);
        assert!(step < 0.001, "step {step} mV");
    }

    #[test]
    fn respiration_breathes_at_twelve_to_twenty_per_minute() {
        let g = only(|c| c.respiration = true);
        let v = series(&g, 0, 3 * MINUTE);
        let pp = peak_to_peak(&v);
        assert!((0.04..=0.12).contains(&pp), "peak-to-peak {pp} mV");
        let crossings = v.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
        let per_min = crossings as f32 / 3.0;
        assert!((11.0..=21.0).contains(&per_min), "{per_min} breaths/min");
        // the QRS amplitude is modulated by a few per cent, symmetrically
        let gains: Vec<f32> = (0..MINUTE * 3).map(|i| g.gain(i, FS)).collect();
        let (lo, hi) = (
            gains.iter().copied().fold(f32::MAX, f32::min),
            gains.iter().copied().fold(f32::MIN, f32::max),
        );
        assert!(
            (0.93..0.96).contains(&lo) && (1.04..1.07).contains(&hi),
            "{lo}..{hi}"
        );
    }

    #[test]
    fn mains_is_a_pure_tone_at_the_chosen_frequency() {
        for hz in MainsFrequency::ALL {
            let g = only(|c| {
                c.mains = true;
                c.mains_hz = hz;
            });
            let v = series(&g, 0, 5000); // 10 s: a whole number of periods at 50 and 60 Hz
            let pp = peak_to_peak(&v);
            // 0.2 mV reference, times the lead gain of 0.6-1.0
            assert!((0.10..=0.21).contains(&pp), "{hz:?}: {pp} mV");
            // power at the mains frequency (single-bin DFT) is the whole signal
            let f = hz.hz();
            let (mut re, mut im) = (0.0f64, 0.0f64);
            for (i, x) in v.iter().enumerate() {
                let ph = TAU * f * i as f64 / f64::from(FS);
                re += f64::from(*x) * ph.cos();
                im += f64::from(*x) * ph.sin();
            }
            let amp = 2.0 * re.hypot(im) / v.len() as f64;
            let total = f64::from(rms(&v)) * 2f64.sqrt();
            assert!(
                (amp - total).abs() < 0.02 * total,
                "{hz:?}: tone {amp}, total {total}"
            );
        }
    }

    #[test]
    fn the_two_mains_frequencies_differ() {
        let at = |hz| {
            let g = only(|c| {
                c.mains = true;
                c.mains_hz = hz;
            });
            series(&g, 0, 400)
        };
        assert_ne!(at(MainsFrequency::Hz50), at(MainsFrequency::Hz60));
    }

    #[test]
    fn muscle_tremor_is_broadband_above_20_hz_and_of_the_cited_size() {
        let g = only(|c| c.muscle = true);
        let v = series(&g, 4, 2 * MINUTE);
        let r = rms(&v);
        assert!((0.01..=0.05).contains(&r), "rms {r} mV");
        // the tones start at 25 Hz, so the monitor band (40 Hz low-pass) keeps only a small part
        let mut mon = BandFilter::new(FilterMode::Monitor, FS);
        let kept: Vec<f32> = v.iter().map(|&x| mon.process(x)).collect();
        assert!(
            rms(&kept[2000..]) < 0.6 * r,
            "the monitor band should remove most of it"
        );
        // leads carry independent tremor
        let other = series(&g, 5, 2 * MINUTE);
        let corr: f32 =
            v.iter().zip(&other).map(|(a, b)| a * b).sum::<f32>() / (v.len() as f32 * r * rms(&other));
        assert!(corr.abs() < 0.1, "leads correlate: {corr}");
    }

    #[test]
    fn intensity_scales_the_artefact_linearly() {
        let base = only(|c| c.mains = true);
        let double = only(|c| {
            c.mains = true;
            c.intensity = 2.0;
        });
        let (a, b) = (series(&base, 1, 500), series(&double, 1, 500));
        for (x, y) in a.iter().zip(&b) {
            assert!((2.0 * x - y).abs() < 1e-6);
        }
    }

    #[test]
    fn description_names_what_is_on() {
        let cfg = ArtefactConfig {
            baseline_wander: true,
            mains: true,
            mains_hz: MainsFrequency::Hz60,
            seed: 7,
            ..ArtefactConfig::default()
        };
        let d = cfg.describe().unwrap();
        assert!(d.contains("baseline wander") && d.contains("60 Hz mains") && d.contains("seed 7"));
        assert!(!d.contains("muscle"));
        assert!(d.is_ascii());
        assert_eq!(ArtefactConfig::default().describe(), None);
    }
}
