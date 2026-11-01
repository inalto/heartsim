# B13 — Pacing devices
**Crates** `heartsim-sim` (pacer timing, from B1), `heartsim-ui` (`src/pacer_panel.rs`), `heartsim-app` · **Model** Sonnet ·
**Milestone** M3 · **Depends on** B1, B3, B9.

## Goal
A temporary external pacemaker panel and implanted-device presets that pace the 3-D heart through `Action::Stimulate` events with
realistic timing cycles, capture thresholds and sensing, so learners see pacing spikes, capture/loss of capture, fusion and
pseudo-fusion, undersensing/oversensing, and the paced morphology that corresponds to the lead site.

## Read first
`docs/research/electrophysiology-and-ecg-models.md` §2.5.4 (pacemaker timing cycles), the private reference-product analysis notes (not published)
§1 "Pacing" (what the reference exposes: rate, output, sensitivity, AV delay, modes, wires), B1 `PacerConfig`.

## Scope
1. Panel: mode knob (VOO/VVI/AOO/AAI/DOO/DDD/DDI), rate 30–180, A/V output 0.1–20 mA, A/V sensitivity 0.5–20 mV, AV delay,
   upper rate, PVARP, pause/asynchronous button, battery/indicator LEDs (sense/pace flashes).
2. Capture model: lead site (RA appendage, RV apex, RV septum, LV lateral/posterior via CS) has a threshold (mA) that may drift
   (scenario parameter); output below threshold → spike without capture. Sensing: the engine reports intracardiac amplitude at
   the lead site (from local V_m slope); sensitivity above it → undersensing.
3. Pacing spikes rendered on the strip (short vertical lines) and the lead sites shown as markers on the heart (B6 instances).
4. Implanted presets: VVI 60, DDD 60/130, BiV (LV lateral + RV apex with VV offset) → scenario files.
5. Scenarios: complete heart block + VVI; sinus brady + AAI; loss of capture on exit block; undersensing with competition; PMT
   (pacemaker-mediated tachycardia) with DDD and retrograde conduction, terminated by PVARP extension.

## Acceptance tests
* Timing tests from B1 for each mode; capture threshold logic unit-tested; integration: VVI at 70 on the idealised heart yields
  paced QRS ≥ 140 ms with LBBB-like morphology when pacing RV apex (axis superior), narrower with His-area site (if landmark exists).
* Spike markers align with stimulus events within 1 sample.
