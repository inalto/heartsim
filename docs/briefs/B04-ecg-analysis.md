# B4 — ECG analysis: detection, intervals, QTc, heart vector
**Crate** `heartsim-ecg` (`src/analysis/*.rs`) · **Model** Sonnet · **Milestone** M1 · **Depends on** nothing (works on `LeadRing`s).

## Goal
Real-time measurements shown in the UI and used by quizzes: HR, RR, PR, QRS duration, QT, QTc (Bazett, Fridericia), frontal QRS
axis and T axis, P-wave presence, plus wave onset/offset markers for annotation and the 3-D heart vector over the last beat.

## Read first
`crates/heartsim-ecg/src/{ring.rs,synth.rs}`, `crates/heartsim-core/src/axis.rs`, `docs/research/electrophysiology-and-ecg-models.md`
§3.6 (axis), §4.1 (McSharry wave parameters, useful for test fixtures), `docs/research/reference-apps.md` §7 (axis trainer maths).

## Scope
1. QRS detector (Pan–Tompkins style: band-pass 5–15 Hz, derivative, squaring, moving window integration, adaptive thresholds,
   refractory 200 ms) on lead II (or the lead with the largest energy), with onset/offset by slope thresholds.
2. P and T search windows relative to QRS (P: −250…−80 ms, T: +120…+450 ms, rate-adaptive); onset/offset by return to baseline.
3. Intervals with median over the last 8 beats; QTc; axis of QRS and T from net areas of I and aVF (`FrontalAxis`); classification.
4. Heart vector: for each sample in the last beat, 3-D vector from limb leads + V2/V6 (Frank-like inverse, or simply the
   Tier-0/Tier-1 dipole exposed by the engine when available) → `Vec<DVec3>` for the loop display.
5. Annotations API: `Vec<WaveMark { lead, kind: P|Q|R|S|T|U, onset, peak, offset }>` for the strip to draw labels (F-12 wave labels).
6. Robustness: noise (B4 must not report HR from noise), pacing spikes (ignore < 5 ms spikes), AF (irregular RR → report "irregular").

## Acceptance tests
Synthesise with `GaussianSynth` + `TimingEngine` (deterministic): HR within 1 bpm of the sinus rate for 40–180 bpm; PR within 10 ms of
engine PR; QRS 80–100 ms for default morphology and 130–170 ms with `qrs_width_scale 1.6`; QT within 20 ms of (T offset − Q onset)
of the generator; axis within 5° of `axis_shift + 60`; AF-like irregular RR flagged; with 0.05 mV noise results unchanged within
tolerance; benchmark: analysis of 10 s × 12 leads < 5 ms.
