//! ECG signal layer.
//!
//! * [`synth`] — Tier 0: event-driven Gaussian-sum waveform synthesis projected onto 12 leads
//!   through fixed lead vectors (McSharry-style morphology, Sameni-style 3-D dipole).
//! * [`leadvec`] — lead vectors of the 12 leads in heart coordinates (used by Tier 0 and as the
//!   far-field fallback of Tier 1).
//! * [`ring`] — fixed-size ring buffers and per-pixel-column min/max decimation for the strip.
//! * [`filter`] — simple first-order IIR filters (baseline high-pass, low-pass) and noise.

pub mod filter;
pub mod leadvec;
pub mod ring;
pub mod synth;

pub use leadvec::LeadVectors;
pub use ring::{LeadRing, MinMax};
pub use synth::{GaussianSynth, WaveSpec};
