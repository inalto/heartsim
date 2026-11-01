# B1 — Timing-engine extensions
**Crate** `heartsim-sim` (`src/timing.rs`, new `src/timing/*.rs`) · **Model** Opus · **Milestone** M1 · **Depends on** nothing.

## Goal
Extend `TimingEngine` so that *every non-re-entrant rhythm* in `docs/REQUIREMENTS.md` F-07 can be produced from parameters alone,
with events that drive both the Tier-0 synthesiser and (later) the Tier-1 spatial engine. Keep it deterministic and allocation-free
per step.

## Read first
`crates/heartsim-sim/src/timing.rs` (current engine + tests), `docs/research/electrophysiology-and-ecg-models.md` §2.5 (AV node
recovery curve, dual pathways, WPW, foci, pacemakers, refractoriness), `docs/reverse-engineering/EPICARDIO-ANALYSIS.md` §4 items 1–4
and 9–10 (what the reference product parameterises), `docs/ARCHITECTURE.md` §4.1.

## Scope
1. **Sinus node**: rate, respiration modulation (exists), sinus arrest/pauses (`pause_every_n`, `pause_ms`), sick-sinus
   (brady-tachy alternation), SA exit block, overdrive suppression (after an ectopic/paced beat the next sinus cycle is prolonged by
   `suppression_ms`; resetting vs non-resetting behaviour).
2. **Ectopic foci** (list): region (atrial, junctional, ventricular left/right), coupling interval or automatic rate, count
   (single/couplet/run), pattern (bigeminy/trigeminy), parasystole (fixed rate, entrance block). Emits `EventKind::Automatic` at
   the focus region with an `origin` tag.
3. **AV node**: dual pathways (fast: short delay, long ERP; slow: long delay, short ERP). Antegrade conduction takes the fast
   pathway unless refractory; a premature beat blocked in the fast pathway conducts slowly, and if `retrograde_fast` is enabled the
   impulse re-enters → **AVNRT loop** (self-sustaining events alternating atrial/His until a parameter or action breaks it).
   Keep the existing recovery curve; add concealed conduction (a blocked impulse still resets the ERP timer partially,
   `concealment_fraction`).
4. **Accessory pathway**: optional edge atria↔ventricle with fixed delay, ERP, antegrade/retrograde flags → pre-excitation events
   (ventricular `Conducted` with `origin = AccessoryPathway` *before* the His-conducted activation, so the Tier-1 engine can fuse)
   and **AVRT** orthodromic/antidromic loops.
5. **His–Purkinje**: separate RBB/LBB/LAF/LPF edges with delays and block flags (events at `Region::RightBundleBranch`, …); rate-dependent
   (phase-3) block: `block_above_rate_bpm`.
6. **Ventricular escape/idioventricular/AIVR** rates; junctional rhythm with retrograde P (emit atrial event after `retro_delay_ms`
   when `av_retrograde`).
7. **Pacemaker channels** (used by B13): `PacerConfig { mode (AOO/AAI/VOO/VVI/DOO/DDD/DDI), lower_rate, upper_rate, av_delay,
   pvarp, blanking, output_capture: bool }` implemented as timing cycles that *sense* engine events and *emit* `EventKind::Stimulus`
   events at the paced chamber; capture only if the chamber is not refractory.
8. **Event enrichment**: add `origin: Origin` (Sinus, Focus(id), Pathway, Pacer(ch), Escape) and `chamber: Chamber` to `Event`
   (keep `region`); add a `LadderSample` export (atrial / AV / ventricular rows with slopes) for the ladder-diagram UI.

## API sketch
```rust
pub struct TimingParams { /* existing */ pub foci: Vec<Focus>, pub av: AvParams, pub accessory: Option<AccessoryPathway>,
    pub his_purkinje: HisPurkinjeParams, pub sinus: SinusParams, pub pacer: Option<PacerConfig> }
impl TimingEngine { pub fn apply(&mut self, action: &heartsim_core::scenario::Action, at: Millis) -> Result<(), ActionError>; }
```
Actions already defined in `heartsim-core::scenario` (Stimulate, PacingTrain, Block, AccessoryPathway, SetGlobalSinusRate).

## Acceptance tests (in `crates/heartsim-sim/tests/rhythms.rs`)
For each, run 30 s at 1 ms and assert on event statistics:
* sinus 70 → 35 ± 1 P and V, PR 120–200 ms, RR CV < 1 %.
* sinus arrhythmia depth 0.1 → RR max/min ratio 1.15–1.25 synchronous with respiration.
* PAC every 5th beat → 6 atrial `Automatic` events with `origin = Focus`, next sinus beat delayed (non-compensatory pause).
* PVC bigeminy → V count ≈ 2 × P-conducted count; compensatory pause present.
* Wenckebach at atrial 110 bpm with `av_recovery_amplitude 200, tau 150` → progressive PR lengthening then a dropped beat; ratio
  between 3:2 and 5:4.
* Mobitz II 3:1, complete block with junctional 45 and ventricular 30 escape (the faster wins), AVNRT sustained ≥ 10 s after a
  single PAC with dual pathways enabled and terminated by `Action::Drug{adenosine}` (implement adenosine as transient AV ERP ↑).
* WPW: ventricular pre-excitation event precedes His-conducted event by `his_delay − pathway_delay`; AVRT orthodromic loop at
  150–220 bpm after a PAC.
* VVI at 60 with sinus 40 → pacer stimuli fill gaps, inhibited when sinus > 60; DDD tracks atrial up to upper rate; PVARP blocks
  retrograde P tracking.
* Determinism: two runs identical; serialisation roundtrip of `TimingParams` via RON.
Document each parameter with physiological units and typical ranges.
