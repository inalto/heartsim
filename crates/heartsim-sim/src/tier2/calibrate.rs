//! Numerical calibration of cell models against the grid they run on (research §6.3:
//! "calibrate D per axis on a strand"). On a coarse grid the plane-wave speed falls below the
//! continuum value and the single-cell APD differs from the model's nominal one, so the
//! mapping `RegionParams → (D, model)` is solved by bisection on the very discretisation the
//! solver uses: a 1-D cable with the same `h`, `dt` and explicit Euler step. Everything here
//! is deterministic.

use super::model::CellModel;
use super::solver::{ACTIVATION_THRESHOLD, Trajectory, apply_stimulus, stimulus_conductance};

/// Cable length, voxels; CV is timed between `CABLE_FROM` and `CABLE_TO`.
const CABLE_LEN: usize = 48;
const CABLE_FROM: usize = 12;
const CABLE_TO: usize = 30;
/// Give up on a cable run after this long (a front slower than ≈ 0.02 m/s counts as failure).
const CABLE_MAX_MS: f32 = 2500.0;
const BISECTIONS: usize = 14;
/// Largest `dt / max_dt` the reaction may run at (see [`solve_excitation`]).
pub const MAX_REACTION_FRACTION: f32 = 0.5;

/// Measured plane-wave speed along a uniform 1-D cable, mm/ms (= m/s); `None` if the front
/// fails to propagate.
#[must_use]
pub fn cable_cv<M: CellModel>(model: &M, d_mm2_ms: f32, h_mm: f32, dt_ms: f32) -> Option<f32> {
    let n = CABLE_LEN;
    let mut s = vec![0.0f32; n * M::VARS];
    for c in s.chunks_mut(M::VARS) {
        model.rest(c);
    }
    let w = d_mm2_ms / (h_mm * h_mm);
    let g_stim = stimulus_conductance(1.0, 2.0 * w);
    let mut u_old = vec![0.0f32; n];
    let (mut t_from, mut steps) = (None, 0u32);
    loop {
        let t = steps as f32 * dt_ms;
        if t > CABLE_MAX_MS {
            return None;
        }
        for (i, u) in u_old.iter_mut().enumerate() {
            *u = s[i * M::VARS];
        }
        for i in 0..n {
            let ui = u_old[i];
            let left = if i > 0 { u_old[i - 1] } else { ui };
            let right = if i + 1 < n { u_old[i + 1] } else { ui };
            let c = &mut s[i * M::VARS..(i + 1) * M::VARS];
            if i < 3 && t < 2.0 {
                c[0] = apply_stimulus(ui, g_stim, dt_ms);
            }
            model.step(c, w * (left + right - 2.0 * ui), dt_ms);
        }
        steps += 1;
        let t1 = steps as f32 * dt_ms;
        let cross = |i: usize, s: &[f32]| {
            let (a, b) = (u_old[i], model.vm(&s[i * M::VARS..]));
            (a < ACTIVATION_THRESHOLD && b >= ACTIVATION_THRESHOLD)
                .then(|| t1 - dt_ms + dt_ms * (ACTIVATION_THRESHOLD - a) / (b - a))
        };
        // the stimulated wave died out before reaching the timing points
        if t1 > 5.0 && t_from.is_none() && s.chunks(M::VARS).all(|c| model.vm(c) < ACTIVATION_THRESHOLD) {
            return None;
        }
        if t_from.is_none() {
            t_from = cross(CABLE_FROM, &s);
        } else if let Some(t_to) = cross(CABLE_TO, &s) {
            let dt = t_to - t_from.unwrap_or(t_to);
            return (dt > 0.0).then(|| (CABLE_TO - CABLE_FROM) as f32 * h_mm / dt);
        }
    }
}

/// Smallest `D` (mm²/ms) in `(0, d_max]` whose cable CV reaches `target`, by bisection on
/// log D. Returns `(D, achieved CV)`; when even `d_max` is too slow, `(d_max, its CV)`.
#[must_use]
pub fn solve_d<M: CellModel>(model: &M, target_m_s: f32, h_mm: f32, dt_ms: f32, d_max: f32) -> (f32, f32) {
    let cv = |d: f32| cable_cv(model, d, h_mm, dt_ms).unwrap_or(0.0);
    let top = cv(d_max);
    if top <= target_m_s {
        return (d_max, top);
    }
    // bracket around the continuum estimate (a coarse grid only ever needs a larger D)
    let lo = model
        .analytic_cv(1.0)
        .filter(|c| *c > 0.0)
        .map_or(d_max * 1e-5, |c| (target_m_s / c).powi(2) / 4.0)
        .min(d_max * 0.5);
    let (mut lo, mut hi) = (lo.ln(), d_max.ln());
    for _ in 0..BISECTIONS {
        let mid = f32::midpoint(lo, hi);
        if cv(mid.exp()) < target_m_s {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let d = hi.exp();
    (d, cv(d))
}

/// Diffusion budget left for a model at step `dt_ms`: the largest `tr D` (mm²/ms) with
/// `dt·(2·load + λ_reaction) ≤ 2·stability`, where `load ≤ 2 tr D / h²` and
/// `λ_reaction = 1/max_dt`.
#[must_use]
pub fn trace_budget<M: CellModel>(model: &M, h_mm: f32, dt_ms: f32, stability: f32) -> f32 {
    let f_r = dt_ms / model.max_dt_ms();
    ((stability - 0.5 * f_r) * h_mm * h_mm / (2.0 * dt_ms)).max(0.0)
}

/// Excitation scale `s ≥ 1` (with the largest stable diffusivity `d_of(s)` for it) whose cable
/// CV reaches `target`. Faster kinetics eat into the stability budget, so `d_of` shrinks as s
/// grows; `s` is searched up to where the reaction alone uses the whole budget. Returns
/// `(s, D, achieved CV)`, the fastest pair when the target is out of reach.
#[must_use]
pub fn solve_excitation<M: CellModel>(
    model: &M,
    target_m_s: f32,
    h_mm: f32,
    dt_ms: f32,
    d_of: impl Fn(&M) -> f32,
) -> (f32, f32, f32) {
    let eval = |s: f32| {
        let m = model.scaled_excitation(s);
        let d = d_of(&m);
        (d, cable_cv(&m, d, h_mm, dt_ms).unwrap_or(0.0))
    };
    // the upstroke must take a few steps (dt ≤ ½·max_dt): a one-step upstroke overshoots u = 1,
    // where the cubic term is stiffer still, and diverges
    let s_max = (MAX_REACTION_FRACTION * model.max_dt_ms() / dt_ms).max(1.0);
    // CV(s) rises, then falls once the front gets too thin for the grid: golden-section-free
    // scan for the peak, then bisection below it
    let samples: Vec<(f32, f32, f32)> = (0..=12)
        .map(|i| {
            let s = s_max.powf(i as f32 / 12.0);
            let (d, cv) = eval(s);
            (s, d, cv)
        })
        .collect();
    let best = samples
        .iter()
        .copied()
        .fold((1.0, 0.0, 0.0), |a, b| if b.2 > a.2 { b } else { a });
    if best.2 <= target_m_s {
        return best;
    }
    let (mut lo, mut hi) = (1.0f32, best.0);
    for _ in 0..BISECTIONS {
        let mid = (lo * hi).sqrt();
        if eval(mid).1 < target_m_s {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let (d, cv) = eval(hi);
    (hi, d, cv)
}

/// Single-cell APD (upstroke → u < 0.1) of `model` at step `dt_ms`, ms.
#[must_use]
pub fn single_cell_apd<M: CellModel>(model: &M, dt_ms: f32) -> f32 {
    Trajectory::of(model, dt_ms).apd_ms
}

/// `model.scaled_apd(s)` with the single-cell APD closest to `target_ms` (bisection on log s
/// over `[0.05, 20]`). Returns the scaled model and its APD.
#[must_use]
pub fn solve_apd<M: CellModel>(model: &M, target_ms: f32, dt_ms: f32) -> (M, f32) {
    let apd = |s: f32| single_cell_apd(&model.scaled_apd(s), dt_ms);
    let (mut lo, mut hi) = (0.05f32.ln(), 20.0f32.ln());
    for _ in 0..BISECTIONS {
        let mid = f32::midpoint(lo, hi);
        let a = apd(mid.exp());
        if a.is_nan() || a < target_ms {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let m = model.scaled_apd(hi.exp());
    let a = single_cell_apd(&m, dt_ms);
    (m, a)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tier2::model::AlievPanfilov;

    #[test]
    fn calibration_hits_its_targets() {
        let ap = AlievPanfilov::default();
        let (d, cv) = solve_d(&ap, 0.6, 2.0, 0.25, 2.4);
        assert!((cv - 0.6).abs() < 0.01, "D {d} → CV {cv}");
        let (s, d, cv) = solve_excitation(&ap, 0.9, 2.0, 0.25, |m| trace_budget(m, 2.0, 0.25, 0.9) / 3.0);
        assert!(s > 1.0 && (cv - 0.9).abs() < 0.02, "s {s} D {d} → CV {cv}");
        let (_, apd) = solve_apd(&ap, 280.0, 0.25);
        assert!((apd - 280.0).abs() < 2.0, "APD {apd}");
    }
}
