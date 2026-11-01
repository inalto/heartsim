# B5 — Forward ECG model (cells → electrodes → 12 leads)
**Crate** `heartsim-ecg` (`src/forward.rs`) · **Model** Opus · **Milestone** M2 · **Depends on** B2 (electrode sites, cloud), B3 (V_m).

## Goal
Replace the Tier-0 lead vectors with a physically based mapping from per-cell V_m to the nine signal electrodes, so that lead
morphologies (Q waves, R/S progression across V1–V6, ST shifts, T polarity) follow from anatomy and activation.

## Read first
`docs/ARCHITECTURE.md` §4.4; `docs/research/electrophysiology-and-ecg-models.md` §3.1–3.4 (Gima–Rudy pseudo-ECG, equivalent double
layer, lead-field, WCT and augmented leads, electrode coordinates), §3.5 (what produces each wave; ST via injury current);
`docs/reverse-engineering/EPICARDIO-ANALYSIS.md` §5 (reference product: 1/r^p weighting with p = 3 and per-population gains);
`crates/heartsim-core/src/lead.rs`.

## Scope
1. `ForwardModel::build(cloud, electrodes, opts)` precomputes `W: [n_electrodes × n_cells]` (f32, row-major). Two source models:
   * **Dipole/EDL**: for cells with a surface normal and area (epicardial/endocardial surface cells), `w = k · (n·(r_e − x))/|r_e − x|³`;
   * **Volume fallback**: `w = k / |r_e − x|^p` with the sign carried by the local gradient of V_m (use neighbour differences
     `Σ_j (V_j − V_i) d_ij/|d_ij|` projected on `(r_e − x)`), which is the discrete form of `∇V_m · ∇(1/r)`.
   Pick per cell based on available data; expose both for comparison.
2. `sample(&self, vm: &[f32]) -> ElectrodePotentials` (SIMD-friendly matrix-vector product; rayon chunks with fixed-order sum).
3. Reference: right leg = 0; subtract the mean of RA/LA/LL (WCT) implicitly through `ElectrodePotentials::to_leads`.
4. Electrode moves: `move_electrode(e, pos)` recomputes one row (≤ 1 ms for 50 k cells).
5. Calibration: a scale `k` such that the default sinus beat gives R in II ≈ 1.0–1.5 mV and V1 S/V5 R ratios in the normal range;
   per-wave gain hooks (`p_gain, qrs_gain, t_gain`) default 1.0, documented as pedagogical.
6. ST segment: support a per-cell resting-potential offset (`v_rest_offset[j]`) added before sampling so ischaemic regions produce
   injury currents (used by B14).
7. Export `W` statistics for the validation bridge (B23).

## Acceptance tests
* Single dipole at the heart centre pointing +60° frontal → axis 60 ± 3° from the derived leads; pointing anterior → V1–V3 positive,
  V5–V6 near zero.
* Idealised-heart sinus beat from B3: upright P in II; qR in V5–V6; rS in V1; R-wave progression monotone V1→V5; T upright in
  I, II, V3–V6 when `apd_endo > apd_epi` and inverted when the gradient is reversed; aVR negative.
* LBBB beat: QRS ≥ 120 ms, broad notched R in V6, deep S in V1; RBBB: rsR' in V1, wide S in I/V6; LAFB: axis ≤ −45°; LPFB ≥ +90°.
* Moving V1 to V2's position changes V1 and nothing else; swapping RA/LA inverts lead I and swaps II/III.
* Benchmark: `sample` for 50 k cells × 9 electrodes ≤ 0.3 ms.
