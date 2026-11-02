//! ECG signal layer.
//!
//! * [`synth`] — Tier 0: event-driven Gaussian-sum waveform synthesis projected onto 12 leads
//!   through fixed lead vectors (McSharry-style morphology, Sameni-style 3-D dipole).
//! * [`forward`] — Tier-1 forward model: weight matrix `W[electrode, cell]` from the cell cloud and
//!   electrode sites (volume dipole / equivalent double layer), V_m → electrode potentials.
//! * [`leadvec`] — lead vectors of the 12 leads in heart coordinates (used by Tier 0 and as the
//!   far-field fallback of Tier 1).
//! * [`ring`] — fixed-size ring buffers and per-pixel-column min/max decimation for the strip.
//! * [`axis_est`] — measuring the frontal QRS axis from recorded leads I and aVF.
//! * [`analysis`] — QRS/P/T detection, intervals, QTc, axes, wave marks and the heart vector.
//! * [`filter`] — simple first-order IIR filters (baseline high-pass, low-pass) and noise.

pub mod analysis;
pub mod axis_est;
pub mod filter;
pub mod forward;
pub mod leadvec;
pub mod ring;
pub mod synth;

pub use analysis::{
    Analysis, AnalysisConfig, Analyzer, Beat, HeartVectorSolver, Regularity, SignalQuality, WaveBounds,
    WaveKind, WaveMark,
};
pub use axis_est::{estimate_frontal_axis, net_area_axis};
pub use forward::{
    ForwardError, ForwardModel, ForwardOptions, LimbPlacement, SourceModel, SurfacePatches, VentricularPhase,
    WaveGains, WeightStats,
};
pub use leadvec::LeadVectors;
pub use ring::{LeadRing, MinMax};
pub use synth::{GaussianSynth, WaveSpec};
