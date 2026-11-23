# heartsim — roadmap and work breakdown

Milestones are cumulative; each has an exit criterion a reviewer can check. Briefs (`docs/briefs/B-xx-*.md`) are the unit of
delegation; "model" is the recommended Claude model (Opus = design-heavy / numerics, Sonnet = standard feature work, Haiku =
mechanical, well-specified tasks). Parallel groups can run concurrently in separate worktrees.

| milestone | exit criterion | briefs |
|---|---|---|
| **M0 Foundation** (done 2026-10-31) | Workspace builds on Linux; pure crates tested; app shows live Tier-0 strip + viewport placeholder; CI defined. | — |
| **M1 Rhythm trainer** | Timing engine covers all non-re-entrant rhythms; Tier-0 12-lead presets for 25 rhythms with golden tests; strip with calipers, intervals, axis wheel; quiz mode; CLI export; packaged builds for 3 OSes. | B1, B4, B7, B8, B11, B12 |
| **M2 Anatomy + Tier 1** | Open-licence anatomy pack baked (heart, torso, electrodes, hoods, fibres, Purkinje seeds); Tier-1 eikonal engine produces V_m per cell; forward model gives 12 leads with correct axis and morphologies for sinus, BBB, hemiblocks, WPW, PVC, STEMI; colour map on the heart. | B2, B3, B5, B6, B9 |
| **M3 Interventions + devices** | Movable electrodes; pacing (temporary device UI, modes); defibrillation; drugs; ischaemia by coronary branch; scenario timeline editor; tutorials engine. | B10, B13, B14, B15 |
| **M4 Re-entry (Tier 2)** | AF, flutter, VT, VF, torsades presets via reaction–diffusion; defibrillation terminates them; performance budget met. | B16, B17 |
| **M5 Education content + polish** | 40+ scenarios with explanations, 10 tutorials, 3 patient cases, i18n EN/IT, accessibility pass, exports (PDF/PNG/WFDB), docs site. | B18, B19, B20 |
| **M6 Web build** | WASM build with Tier 0/1 (WebGPU, WebGL2 fallback) deployed via GitHub Pages. | B21 |
| **M7 EP lab (P2)** | Catheters, EGMs, PES, ablation. | B22 |
| **M8 Validation bridge (P2)** | openCARP/VTK export + import; Niederer-style comparison report. | B23 |
| **M9 Heart sounds (audio)** | The phonocardiogram drawn under the ECG and heard, in five phases: (1) silent PCG trace with S1/S2 from engine events (B24, done 0.16.0); (2) desktop audio with a fixed display delay, slow-motion re-synthesis and play-from-cursor (B25); (3) web audio through Web Audio, gesture-started, latency-compensated (B26, done 0.18.0); (4) S3, S4, clicks, snaps and murmurs, the `auscultation:` scenario section, listening sites, bell and diaphragm, valve highlights (B27, done 0.19.0); (5) teaching: auscultation tutorials, quiz, repetition trainer, breath-hold, small-speaker mode, WAV export; reference recordings listed for approval (B28, done 0.20.0). | B24, B25, B26, B27, B28 |
| **M10 Clinical calculators** | A Tools group with the Echo calculators page: closed-formula calculators for valves, filling pressures, ventricular and right-heart function, shunts, prostheses, pericardium and ultrasound physics (B29 part A), then the guideline decision tools (part B); every result cites its primary source and is marked verified or formula-only (done 0.22.0). | B29 |
| **M11 Echo simulator** | A transthoracic probe on the figure and a simulated ultrasound sector of the beating heart, in five phases (research: `docs/research/echo-simulator.md`): (1) beating heart from engine timing with an LVEF read-out (B30, done 0.23.0); (2) label volume, designed leaflets and papillary muscles, probe on the figure, anatomical cross-section; (3) ultrasound appearance (attenuation, reflection, shadowing, tissue-locked speckle); (4) the standard views with a geometric score, tutorial and self-test; (5) M-mode, annular tissue Doppler, calipers feeding the calculators. No blood Doppler. | B30 |

## Dependency graph of briefs
```
B1 timing-engine-extensions ─┐
B4 ecg-analysis ─────────────┼─► B7 strip-and-twelve-lead ─► B8 axis-wheel-and-quiz ─► B11 scenario-library-tier0
B12 packaging-ci (independent)                                                            │
B2 anatomy-pipeline ─► B3 tier1-eikonal ─► B5 forward-model ─► B9 colour-maps-and-picking ◄┘
                                    └────► B6 multi-mesh-render ─────────────────────────┘
B10 electrodes-editor (needs B5)   B13 pacing-devices (needs B1, B3)   B14 drugs-ischaemia-defib (needs B3, B5)
B15 tutorials-engine (needs B11)   B16 tier2-reaction-diffusion (needs B2, B5)   B17 reentry-presets (needs B16)
B18 content-scenarios (needs B11, B14)   B19 i18n-a11y   B20 exports-docs   B21 wasm   B22 ep-lab   B23 validation-bridge
```

## Parallel execution plan (suggested waves)
1. Wave 1 (after M0): B1 (Opus), B2 (Opus), B4 (Sonnet), B12 (Sonnet), B19 scaffolding (Haiku).
2. Wave 2: B3 (Opus), B7 (Sonnet), B6 (Sonnet), B8 (Sonnet), B11 (Haiku, data entry from literature tables).
3. Wave 3: B5 (Opus), B9 (Sonnet), B10 (Sonnet), B13 (Sonnet), B15 (Sonnet).
4. Wave 4: B14 (Sonnet), B16 (Opus), B18 (Haiku/Sonnet), B20 (Sonnet).
5. Wave 5: B17 (Sonnet), B21 (Sonnet), B22 (Opus), B23 (Opus).

## Definition of done (every brief)
* Code + tests in the named crates; `cargo xtask ci` green on Linux; no new `unsafe`; docs comments on public items.
* Golden snapshots updated deliberately (`cargo insta review`), never blindly accepted.
* Brief's acceptance checklist ticked in the PR description; screenshots/CSV attached where the brief asks.
* No proprietary material (see `CLAUDE.md`).
