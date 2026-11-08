//! The monodomain solver on a [`VoxelGrid`]:
//!
//! ```text
//! ∂u/∂t = ∇·(D ∇u) − I_ion(u, g)/C_m + I_stim          (research §2.3)
//! ```
//!
//! **Discretisation.** The diffusion term is a symmetric graph Laplacian over the 18-neighbour
//! stencil, `(∇·D∇u)_i ≈ Σ_j w_ij (u_j − u_i)`, with `w_ij = ½(c_k(i) + c_k(j))/h²` and `c_k` the
//! stencil coefficients of each voxel's fibre tensor ([`VoxelGrid::stencil`]). Links exist only
//! between conducting voxels of the same compartment, so boundaries, obstacles, scar and the AV
//! ring are no-flux by construction and charge is conserved. One explicit Euler step advances
//! diffusion and reaction together (Godunov splitting with a common dt; research §6.3); the
//! diffusion current enters the cell model as an applied current.
//!
//! **Determinism.** Each voxel's update reads only the previous state (ping-pong buffer for u,
//! own gates in place) and sums its links in a fixed (CSR) order, so the result is
//! bit-identical whatever the number of rayon threads. Time is `substeps · dt`, never an
//! accumulated float.

use super::grid::{Compartment, DIRECTIONS, VoxelGrid};
use super::model::CellModel;
use glam::Vec3;

/// Active voxels per parallel work item (whole slabs on a 64³ box are 4096 voxels).
const CHUNK: usize = 4096;
/// Membrane-variable level that counts as activation (upstroke crossing).
pub const ACTIVATION_THRESHOLD: f32 = 0.5;
/// Samples of the single-cell trajectory table over phase `[0, TRAJECTORY_PHASES)`.
const TRAJECTORY_SAMPLES: usize = 256;
/// Phase (in APDs after the upstroke) beyond which a cell counts as fully recovered.
pub const TRAJECTORY_PHASES: f32 = 2.0;
/// State magnitudes below this are set to exactly 0 (see `substep`).
const FLUSH_BELOW: f32 = 1e-20;
/// Upper bound on model state size (stack buffer of the kernel).
const MAX_VARS: usize = 4;

/// One tissue class: a parameterised cell model and its diffusivities.
#[derive(Debug, Clone, PartialEq)]
pub struct Tissue<M> {
    pub model: M,
    /// Diffusivity along / across the fibre, mm²/ms.
    pub d_long: f32,
    pub d_trans: f32,
}

/// Compartment-wide change of the tissue: a drug effect or remodelling (see
/// [`Monodomain::set_modifiers`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TissueModifier {
    /// APD (and so refractory period) multiplier.
    pub apd_scale: f32,
    /// Conduction-velocity multiplier (≤ 1).
    pub cv_scale: f32,
}

impl TissueModifier {
    /// No change.
    pub const NONE: Self = Self {
        apd_scale: 1.0,
        cv_scale: 1.0,
    };
}

/// A single-cell action potential sampled by phase (time since upstroke / APD), used to map a
/// Tier-1 phase onto model state and to define the post-shock state.
#[derive(Debug, Clone)]
pub struct Trajectory {
    /// Single-cell APD (upstroke → u < 0.1), ms.
    pub apd_ms: f32,
    vars: usize,
    /// `TRAJECTORY_SAMPLES` states, `vars` floats each.
    states: Vec<f32>,
    rest: Vec<f32>,
}

impl Trajectory {
    /// Simulate one stimulated action potential of `model` at step `dt_ms`.
    pub fn of<M: CellModel>(model: &M, dt_ms: f32) -> Self {
        let mut s = vec![0.0; M::VARS];
        let mut rest = vec![0.0; M::VARS];
        model.rest(&mut rest);
        s.copy_from_slice(&rest);
        let mut raw: Vec<f32> = Vec::new();
        let (mut t, mut up, mut apd) = (0.0f32, None, None);
        let mut n = 0u32;
        while t < 6000.0 {
            let before = model.vm(&s);
            let i = if t < 2.0 { 0.5 * (1.0 - before) } else { 0.0 };
            model.step(&mut s, i, dt_ms);
            n += 1;
            t = n as f32 * dt_ms;
            let after = model.vm(&s);
            match (up, apd) {
                (None, _) if after >= ACTIVATION_THRESHOLD => {
                    up = Some(t - dt_ms * (after - ACTIVATION_THRESHOLD) / (after - before).max(1e-9));
                }
                (Some(t0), None) if after < 0.1 => apd = Some(t - t0),
                _ => {}
            }
            if up.is_some() {
                raw.extend_from_slice(&s);
            }
            if let (Some(t0), Some(a)) = (up, apd)
                && t - t0 > a * TRAJECTORY_PHASES + dt_ms
            {
                break;
            }
        }
        let t0 = up.unwrap_or(0.0);
        let apd_ms = apd.unwrap_or(f32::NAN);
        let vars = M::VARS;
        let samples_n = raw.len() / vars;
        // raw[k] is the state at t_k = t_first + k·dt, with t_first the first step at/after t0
        let t_first = (t0 / dt_ms).ceil() * dt_ms;
        let mut states = Vec::with_capacity(TRAJECTORY_SAMPLES * vars);
        for q in 0..TRAJECTORY_SAMPLES {
            let phase = q as f32 / TRAJECTORY_SAMPLES as f32 * TRAJECTORY_PHASES;
            let x = ((t0 + phase * apd_ms - t_first) / dt_ms).max(0.0);
            let k = (x.floor() as usize).min(samples_n.saturating_sub(1));
            let k1 = (k + 1).min(samples_n.saturating_sub(1));
            let f = (x - k as f32).clamp(0.0, 1.0);
            for (v, &r) in rest.iter().enumerate() {
                let a = raw.get(k * vars + v).copied().unwrap_or(r);
                let b = raw.get(k1 * vars + v).copied().unwrap_or(r);
                states.push(a + (b - a) * f);
            }
        }
        Self {
            apd_ms,
            vars,
            states,
            rest,
        }
    }

    /// State at `phase` APDs after the upstroke (rest outside `[0, TRAJECTORY_PHASES)`).
    pub fn state_at(&self, phase: f32, out: &mut [f32]) {
        if !(0.0..TRAJECTORY_PHASES).contains(&phase) || !phase.is_finite() {
            out.copy_from_slice(&self.rest);
            return;
        }
        let x = phase / TRAJECTORY_PHASES * TRAJECTORY_SAMPLES as f32;
        let k = (x as usize).min(TRAJECTORY_SAMPLES - 1);
        out.copy_from_slice(&self.states[k * self.vars..(k + 1) * self.vars]);
    }

    /// Ranges of (vm, recovery) over the trajectory: the centre of the phase plane.
    fn phase_centre<M: CellModel>(&self, model: &M) -> (f32, f32) {
        let (mut lo, mut hi) = ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]);
        for s in self
            .states
            .chunks(self.vars)
            .chain(std::iter::once(&self.rest[..]))
        {
            for (d, x) in [model.vm(s), model.recovery(s)].into_iter().enumerate() {
                lo[d] = lo[d].min(x);
                hi[d] = hi[d].max(x);
            }
        }
        (f32::midpoint(lo[0], hi[0]), f32::midpoint(lo[1], hi[1]))
    }
}

/// A stimulus pulse on one voxel: a conductance `g` pulling u towards 1, `I = g·(1 − u)`.
/// Unlike a fixed current it cannot drive u beyond the peak, where the cubic reaction term of
/// the phenomenological models is stiff and explicit Euler would diverge.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Pulse {
    start_ms: f64,
    end_ms: f64,
    /// Conductance, 1/ms.
    g: f32,
    voxel: u32,
}

/// Stimulus conductance for a voxel with diffusive `load` (1/ms): `drive + load`, which holds
/// the voxel above u = ½ even when all its neighbours are at rest. The stimulus is integrated
/// implicitly before the explicit step ([`apply_stimulus`]), so any `g` is stable.
#[must_use]
pub fn stimulus_conductance(drive: f32, load: f32) -> f32 {
    drive + load
}

/// Backward-Euler relaxation of u towards 1 under the stimulus conductance `g` over `dt`:
/// `u ← (u + g·dt)/(1 + g·dt)`. Unconditionally stable, never overshoots 1, and IEEE-exact
/// (no libm), so it adds no platform dependence to the Aliev–Panfilov and Mitchell–Schaeffer
/// kernels. Calibration uses `ln`/`exp` and Fenton–Karma uses `tanh`, so runs are guaranteed
/// bit-identical only on one platform.
#[inline]
#[must_use]
pub fn apply_stimulus(u: f32, g: f32, dt_ms: f32) -> f32 {
    if g > 0.0 {
        (u + g * dt_ms) / (1.0 + g * dt_ms)
    } else {
        u
    }
}

/// The reaction–diffusion state on a voxel grid. See the module documentation.
#[derive(Debug, Clone)]
pub struct Monodomain<M: CellModel> {
    grid: VoxelGrid,
    tissue: Vec<Tissue<M>>,
    trajectories: Vec<Trajectory>,
    class: Vec<u16>,
    /// Compartment-wide modifiers (drugs), `[atrial, ventricular]`.
    modifiers: [TissueModifier; 2],
    /// `tissue` with the modifiers applied, two entries per class: `2·class + compartment`.
    effective: Vec<Tissue<M>>,
    /// Index into `effective` of every voxel.
    eff_class: Vec<u16>,
    /// The modifiers changed since `effective` was built.
    effective_dirty: bool,
    /// `false` = inert (ablated, obstacle): no reaction, no links, held at rest.
    conducting: Vec<bool>,
    /// CSR links: `col[row[i]..row[i+1]]`, weights in 1/ms.
    row: Vec<u32>,
    col: Vec<u32>,
    weight: Vec<f32>,
    /// Σ_j w_ij per voxel (diffusive load, sizes stimuli).
    load: Vec<f32>,
    u: Vec<f32>,
    u_next: Vec<f32>,
    /// Non-membrane state variables, `M::VARS − 1` per voxel.
    gates: Vec<f32>,
    stim: Vec<f32>,
    pulses: Vec<Pulse>,
    /// Last upstroke per voxel, ms (`NaN` = never).
    t_act: Vec<f32>,
    dt_ms: f32,
    /// Simulation time of substep 0, ms (non-zero when entered from Tier 1).
    origin_ms: f64,
    substeps: u64,
    phase_centre: (f32, f32),
}

impl<M: CellModel> Monodomain<M> {
    /// Build the solver. `class[v]` indexes `tissue`; all voxels start at rest.
    ///
    /// # Panics
    /// If `dt_ms` exceeds a tissue's reaction limit, or the class table is inconsistent.
    pub fn new(grid: VoxelGrid, tissue: Vec<Tissue<M>>, class: Vec<u16>, dt_ms: f32) -> Self {
        assert!(M::VARS <= MAX_VARS && M::VARS >= 2);
        assert_eq!(class.len(), grid.len());
        assert!(class.iter().all(|&c| usize::from(c) < tissue.len()));
        let trajectories = tissue
            .iter()
            .map(|t| Trajectory::of(&t.model, dt_ms))
            .collect::<Vec<_>>();
        let phase_centre = trajectories
            .first()
            .zip(tissue.first())
            .map_or((0.5, 0.5), |(tr, t)| tr.phase_centre(&t.model));
        let n = grid.len();
        let mut m = Self {
            conducting: vec![true; n],
            row: Vec::new(),
            col: Vec::new(),
            weight: Vec::new(),
            load: vec![0.0; n],
            u: vec![0.0; n],
            u_next: vec![0.0; n],
            gates: vec![0.0; n * (M::VARS - 1)],
            stim: vec![0.0; n],
            pulses: Vec::new(),
            t_act: vec![f32::NAN; n],
            dt_ms,
            origin_ms: 0.0,
            substeps: 0,
            phase_centre,
            grid,
            tissue,
            trajectories,
            class,
            modifiers: [TissueModifier::NONE; 2],
            effective: Vec::new(),
            eff_class: Vec::new(),
            effective_dirty: true,
        };
        m.rebuild_links();
        m.reset();
        m
    }

    /// Uniform tissue on `grid`.
    pub fn uniform(grid: VoxelGrid, tissue: Tissue<M>, dt_ms: f32) -> Self {
        let n = grid.len();
        Self::new(grid, vec![tissue], vec![0; n], dt_ms)
    }

    /// Recompute the link table from classes, fibres and the conducting mask.
    ///
    /// # Panics
    /// If a voxel's diffusive load violates the explicit stability limit `dt·Σw ≤ 1`.
    pub fn rebuild_links(&mut self) {
        self.refresh_effective();
        let g = &self.grid;
        let inv_h2 = 1.0 / (g.h_mm * g.h_mm);
        let coeffs: Vec<[f32; 9]> = (0..g.len())
            .map(|v| {
                let t = &self.effective[usize::from(self.eff_class[v])];
                VoxelGrid::stencil(g.fibre[v], t.d_long, t.d_trans)
            })
            .collect();
        self.row.clear();
        self.col.clear();
        self.weight.clear();
        self.row.push(0);
        for v in 0..g.len() {
            let [i, j, k] = g.ijk[v].map(i64::from);
            let mut load = 0.0;
            if self.conducting[v] {
                for (d, e) in DIRECTIONS.iter().enumerate() {
                    for sign in [1i64, -1] {
                        let Some(n) = g.voxel_at(
                            i + sign * i64::from(e[0]),
                            j + sign * i64::from(e[1]),
                            k + sign * i64::from(e[2]),
                        ) else {
                            continue;
                        };
                        let nu = n as usize;
                        if !self.conducting[nu] || g.compartment[nu] != g.compartment[v] {
                            continue;
                        }
                        // The weaker of the two voxels' coefficients, not their mean: a voxel's
                        // load then never exceeds the load of a uniform field with its own fibre,
                        // so the stability bound below holds however sharply the fibres turn
                        // between neighbours (the mean can exceed it where the fibre field of a
                        // real heart rotates by tens of degrees per voxel).
                        let w = coeffs[v][d].min(coeffs[nu][d]) * inv_h2;
                        if w > 0.0 {
                            self.col.push(n);
                            self.weight.push(w);
                            load += w;
                        }
                    }
                }
            }
            self.load[v] = load;
            self.row.push(self.col.len() as u32);
        }
        // Gershgorin bound on the Jacobian of one Euler step: diffusion contributes ±load
        // around −load, the reaction up to 1/max_dt; stable while dt·(2·load + 1/max_dt) ≤ 2.
        let worst = (0..g.len())
            .map(|v| {
                let m = &self.effective[usize::from(self.eff_class[v])].model;
                self.dt_ms * (2.0 * self.load[v] + 1.0 / m.max_dt_ms())
            })
            .fold(0.0f32, f32::max);
        assert!(
            worst <= 2.0,
            "explicit step unstable: dt·(2·load + λ_reaction) = {worst} > 2"
        );
        let fastest = self
            .effective
            .iter()
            .map(|t| t.model.max_dt_ms())
            .fold(f32::INFINITY, f32::min);
        assert!(
            self.dt_ms <= fastest * 0.5 + 1e-6,
            "dt {} ms > ½ of the fastest reaction limit {fastest} ms (upstroke would overshoot)",
            self.dt_ms
        );
    }

    /// Every voxel to rest, activation history and stimuli cleared, t = 0.
    pub fn reset(&mut self) {
        let g = M::VARS - 1;
        let mut s = [0.0f32; MAX_VARS];
        for v in 0..self.grid.len() {
            self.tissue[usize::from(self.class[v])]
                .model
                .rest(&mut s[..M::VARS]);
            self.u[v] = s[0];
            self.gates[v * g..(v + 1) * g].copy_from_slice(&s[1..M::VARS]);
        }
        self.u_next.copy_from_slice(&self.u);
        self.t_act.iter_mut().for_each(|t| *t = f32::NAN);
        self.stim.iter_mut().for_each(|s| *s = 0.0);
        self.pulses.clear();
        self.substeps = 0;
        self.origin_ms = 0.0;
    }

    /// Restart the clock at `t_ms` (state untouched), e.g. when taking over from Tier 1.
    pub fn set_time(&mut self, t_ms: f64) {
        self.origin_ms = t_ms;
        self.substeps = 0;
    }

    // --- read-out ---------------------------------------------------------------------------

    #[must_use]
    pub fn grid(&self) -> &VoxelGrid {
        &self.grid
    }

    #[must_use]
    pub fn tissue(&self) -> &[Tissue<M>] {
        &self.tissue
    }

    /// Tissue class of each voxel.
    #[must_use]
    pub fn classes(&self) -> &[u16] {
        &self.class
    }

    /// Single-cell trajectory (and APD) of each tissue class.
    #[must_use]
    pub fn trajectories(&self) -> &[Trajectory] {
        &self.trajectories
    }

    /// Membrane variable per voxel.
    #[must_use]
    pub fn u(&self) -> &[f32] {
        &self.u
    }

    /// Last upstroke time per voxel, ms (`NaN` = never).
    #[must_use]
    pub fn activation_ms(&self) -> &[f32] {
        &self.t_act
    }

    #[must_use]
    pub fn conducting(&self) -> &[bool] {
        &self.conducting
    }

    #[must_use]
    pub fn now_ms(&self) -> f64 {
        self.origin_ms + self.substeps as f64 * f64::from(self.dt_ms)
    }

    #[must_use]
    pub fn dt_ms(&self) -> f32 {
        self.dt_ms
    }

    /// Number of diffusion links (each pair counted from both ends).
    #[must_use]
    pub fn link_count(&self) -> usize {
        self.col.len()
    }

    /// Full state of voxel `v` (membrane variable first).
    pub fn voxel_state(&self, v: usize, out: &mut [f32]) {
        let g = M::VARS - 1;
        out[0] = self.u[v];
        out[1..M::VARS].copy_from_slice(&self.gates[v * g..(v + 1) * g]);
    }

    /// Overwrite the state of voxel `v` (membrane variable first).
    pub fn set_voxel_state(&mut self, v: usize, s: &[f32]) {
        let g = M::VARS - 1;
        self.u[v] = s[0];
        self.u_next[v] = s[0];
        self.gates[v * g..(v + 1) * g].copy_from_slice(&s[1..M::VARS]);
    }

    /// Copy of the dynamic state: membrane variable, the other variables (`M::VARS − 1` per
    /// voxel) and the last activation times relative to now (`NaN` = never).
    #[must_use]
    pub fn state_copy(&self) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
        let now = self.now_ms() as f32;
        (
            self.u.clone(),
            self.gates.clone(),
            self.t_act.iter().map(|t| t - now).collect(),
        )
    }

    /// Load a state taken with [`Monodomain::state_copy`]; activation times are re-anchored to
    /// the current time. Pending stimuli are dropped.
    ///
    /// # Panics
    /// If the slices do not match this grid and model.
    pub fn load_state(&mut self, u: &[f32], gates: &[f32], t_act_rel: &[f32]) {
        assert_eq!(u.len(), self.u.len());
        assert_eq!(gates.len(), self.gates.len());
        assert_eq!(t_act_rel.len(), self.t_act.len());
        let now = self.now_ms() as f32;
        self.u.copy_from_slice(u);
        self.u_next.copy_from_slice(u);
        self.gates.copy_from_slice(gates);
        for (t, r) in self.t_act.iter_mut().zip(t_act_rel) {
            *t = now + r;
        }
        self.pulses.clear();
        self.stim.iter_mut().for_each(|x| *x = 0.0);
    }

    /// Put voxel `v` at `phase` APDs after an upstroke at `t_act_ms` (rest for NaN/late phases).
    pub fn set_voxel_phase(&mut self, v: usize, phase: f32, t_act_ms: f32) {
        let mut s = [0.0f32; MAX_VARS];
        self.trajectories[usize::from(self.class[v])].state_at(phase, &mut s[..M::VARS]);
        self.set_voxel_state(v, &s[..M::VARS]);
        self.t_act[v] = t_act_ms;
    }

    // --- interventions ----------------------------------------------------------------------

    /// Stimulate `voxels` during `[start, start + dur)` with drive `drive` (1/ms; see
    /// [`stimulus_conductance`] and [`Pulse`]).
    pub fn stimulate(&mut self, voxels: &[u32], start_ms: f64, duration_ms: f64, drive: f32) {
        for &v in voxels {
            self.pulses.push(Pulse {
                start_ms,
                end_ms: start_ms + duration_ms,
                g: stimulus_conductance(drive, self.load[v as usize]),
                voxel: v,
            });
        }
    }

    /// Defibrillation: every voxel's membrane variable is set to rest while its recovery
    /// variables take the end-of-plateau values (phase 1 of its trajectory), i.e. all tissue is
    /// simultaneously repolarised *and* refractory, which annihilates every wavefront — the
    /// phenomenological picture of a successful countershock (all excitable gaps captured,
    /// refractoriness prolonged). Pending stimuli are dropped.
    pub fn shock(&mut self) {
        let g = M::VARS - 1;
        let mut s = [0.0f32; MAX_VARS];
        for v in 0..self.grid.len() {
            let c = usize::from(self.class[v]);
            self.trajectories[c].state_at(1.0, &mut s[..M::VARS]);
            let mut rest = [0.0f32; MAX_VARS];
            self.tissue[c].model.rest(&mut rest[..M::VARS]);
            self.u[v] = rest[0];
            self.u_next[v] = rest[0];
            self.gates[v * g..(v + 1) * g].copy_from_slice(&s[1..M::VARS]);
        }
        self.pulses.clear();
    }

    /// Make voxels inert (ablation lesion, obstacle) or conducting again, and relink.
    pub fn set_conducting(&mut self, voxels: &[u32], conducting: bool) {
        let mut s = [0.0f32; MAX_VARS];
        for &v in voxels {
            let v = v as usize;
            self.conducting[v] = conducting;
            if !conducting {
                self.tissue[usize::from(self.class[v])]
                    .model
                    .rest(&mut s[..M::VARS]);
                self.set_voxel_state(v, &s[..M::VARS]);
            }
        }
        self.rebuild_links();
    }

    /// Assign tissue class `class` to `voxels` (new classes are appended with
    /// [`Monodomain::add_tissue`]) and relink.
    pub fn set_class(&mut self, voxels: &[u32], class: u16) {
        assert!(usize::from(class) < self.tissue.len());
        for &v in voxels {
            self.class[v as usize] = class;
        }
        self.rebuild_links();
    }

    /// Several class assignments at once (one relink).
    pub fn set_classes(&mut self, assign: &[(u16, Vec<u32>)]) {
        for (class, voxels) in assign {
            assert!(usize::from(*class) < self.tissue.len());
            for &v in voxels {
                self.class[v as usize] = *class;
            }
        }
        self.rebuild_links();
    }

    /// Scale APD and measured CV of every atrial / ventricular voxel (drug effects,
    /// remodelling), re-solved on this grid by [`super::calibrate::retarget`]. `cv_scale` is
    /// clamped to ≤ 1. The classes themselves and the single-cell trajectories (hand-over, shock
    /// state) are left as they were.
    pub fn set_modifiers(&mut self, atrial: TissueModifier, ventricular: TissueModifier) {
        self.modifiers = [atrial, ventricular];
        self.effective_dirty = true;
        self.rebuild_links();
    }

    /// The modifiers in force, `[atrial, ventricular]`.
    #[must_use]
    pub fn modifiers(&self) -> [TissueModifier; 2] {
        self.modifiers
    }

    /// Rebuild the effective class table when classes or modifiers changed, and the per-voxel
    /// index into it.
    fn refresh_effective(&mut self) {
        if self.effective_dirty || self.effective.len() != 2 * self.tissue.len() {
            self.effective.clear();
            for t in &self.tissue {
                for m in self.modifiers {
                    self.effective.push(if m == TissueModifier::NONE {
                        t.clone()
                    } else {
                        super::calibrate::retarget(
                            t,
                            m.apd_scale.max(0.05),
                            m.cv_scale.clamp(0.05, 1.0),
                            self.grid.h_mm,
                            self.dt_ms,
                        )
                    });
                }
            }
            self.effective_dirty = false;
        }
        self.eff_class.clear();
        self.eff_class
            .extend((0..self.grid.len()).map(|v| {
                2 * self.class[v] + u16::from(self.grid.compartment[v] == Compartment::Ventricular)
            }));
    }

    /// Append a tissue class; returns its index.
    pub fn add_tissue(&mut self, tissue: Tissue<M>) -> u16 {
        self.trajectories.push(Trajectory::of(&tissue.model, self.dt_ms));
        self.tissue.push(tissue);
        (self.tissue.len() - 1) as u16
    }

    // --- time stepping ----------------------------------------------------------------------

    /// Advance by whole substeps until `now_ms() ≥ t_ms − dt/2`.
    pub fn advance_to(&mut self, t_ms: f64) {
        let dt = f64::from(self.dt_ms);
        while self.now_ms() + 0.5 * dt <= t_ms {
            self.substep();
        }
    }

    /// One explicit Euler substep of diffusion + reaction + stimuli.
    pub fn substep(&mut self) {
        let dt = self.dt_ms;
        let t0 = self.now_ms();
        let t1 = t0 + f64::from(dt);
        // stimulus currents: exact charge of each pulse that overlaps [t0, t1)
        for p in &self.pulses {
            let overlap = (p.end_ms.min(t1) - p.start_ms.max(t0)).max(0.0);
            if overlap > 0.0 {
                self.stim[p.voxel as usize] += p.g * (overlap / f64::from(dt)) as f32;
            }
        }
        let now = t0 as f32;
        let (u, row, col, w, stim) = (&self.u, &self.row, &self.col, &self.weight, &self.stim);
        let (tissue, class, conducting) = (&self.effective, &self.eff_class, &self.conducting);
        let g = M::VARS - 1;
        let kernel = |start: usize, un: &mut [f32], gs: &mut [f32], ta: &mut [f32]| {
            let mut s = [0.0f32; MAX_VARS];
            for (o, out) in un.iter_mut().enumerate() {
                let i = start + o;
                let ui = u[i];
                if !conducting[i] {
                    *out = ui;
                    continue;
                }
                let mut lap = 0.0f32;
                for e in row[i] as usize..row[i + 1] as usize {
                    lap += w[e] * (u[col[e] as usize] - ui);
                }
                s[0] = apply_stimulus(ui, stim[i], dt);
                s[1..M::VARS].copy_from_slice(&gs[o * g..(o + 1) * g]);
                tissue[usize::from(class[i])]
                    .model
                    .step(&mut s[..M::VARS], lap, dt);
                // resting tissue decays exponentially towards 0: flush before the values
                // become subnormal (an order of magnitude slower on x86, and noise anyway)
                for x in &mut s[..M::VARS] {
                    if x.abs() < FLUSH_BELOW {
                        *x = 0.0;
                    }
                }
                *out = s[0];
                gs[o * g..(o + 1) * g].copy_from_slice(&s[1..M::VARS]);
                if ui < ACTIVATION_THRESHOLD && s[0] >= ACTIVATION_THRESHOLD {
                    ta[o] = now + dt * (ACTIVATION_THRESHOLD - ui) / (s[0] - ui);
                }
            }
        };
        #[cfg(feature = "parallel")]
        {
            use rayon::prelude::*;
            self.u_next
                .par_chunks_mut(CHUNK)
                .zip(self.gates.par_chunks_mut(CHUNK * g))
                .zip(self.t_act.par_chunks_mut(CHUNK))
                .enumerate()
                .for_each(|(c, ((un, gs), ta))| kernel(c * CHUNK, un, gs, ta));
        }
        #[cfg(not(feature = "parallel"))]
        for (c, ((un, gs), ta)) in self
            .u_next
            .chunks_mut(CHUNK)
            .zip(self.gates.chunks_mut(CHUNK * g))
            .zip(self.t_act.chunks_mut(CHUNK))
            .enumerate()
        {
            kernel(c * CHUNK, un, gs, ta);
        }
        std::mem::swap(&mut self.u, &mut self.u_next);
        for p in &self.pulses {
            self.stim[p.voxel as usize] = 0.0;
        }
        self.pulses.retain(|p| p.end_ms > t1);
        self.substeps += 1;
    }

    // --- diagnostics ------------------------------------------------------------------------

    /// Phase singularities: 2×2 plaquettes of conducting voxels in xy planes around which the
    /// phase `θ = atan2(r − r*, u − u*)` winds by ±2π (Gray et al. 1998; `(u*, r*)` = centre
    /// of the single-cell phase-plane loop). In a 2-D sheet this is the number of rotor tips; in
    /// 3-D it counts filament piercings of the xy planes.
    #[must_use]
    pub fn phase_singularities(&self) -> usize {
        let g = &self.grid;
        let (uc, rc) = self.phase_centre;
        let gv = M::VARS - 1;
        let mut s = [0.0f32; MAX_VARS];
        let theta: Vec<f32> = (0..g.len())
            .map(|v| {
                s[0] = self.u[v];
                s[1..M::VARS].copy_from_slice(&self.gates[v * gv..(v + 1) * gv]);
                let m = &self.tissue[usize::from(self.class[v])].model;
                (m.recovery(&s[..M::VARS]) - rc).atan2(m.vm(&s[..M::VARS]) - uc)
            })
            .collect();
        let wrap = |d: f32| {
            let tau = std::f32::consts::TAU;
            d - tau * (d / tau).round()
        };
        let mut count = 0;
        for v in 0..g.len() {
            let [i, j, k] = g.ijk[v].map(i64::from);
            let corners = [
                Some(v as u32),
                g.voxel_at(i + 1, j, k),
                g.voxel_at(i + 1, j + 1, k),
                g.voxel_at(i, j + 1, k),
            ];
            let Some(c) = corners.into_iter().collect::<Option<Vec<u32>>>() else {
                continue;
            };
            let comp = g.compartment[v];
            if c.iter()
                .any(|&x| !self.conducting[x as usize] || g.compartment[x as usize] != comp)
            {
                continue;
            }
            let winding: f32 = (0..4)
                .map(|q| wrap(theta[c[(q + 1) % 4] as usize] - theta[c[q] as usize]))
                .sum();
            if winding.abs() > std::f32::consts::PI {
                count += 1;
            }
        }
        count
    }

    /// Voxels inside an axis-aligned box (mm, inclusive), optionally of one compartment.
    #[must_use]
    pub fn voxels_in_box(&self, lo: Vec3, hi: Vec3, compartment: Option<Compartment>) -> Vec<u32> {
        (0..self.grid.len() as u32)
            .filter(|&v| {
                let p = self.grid.position(v);
                p.cmpge(lo).all()
                    && p.cmple(hi).all()
                    && compartment.is_none_or(|c| self.grid.compartment[v as usize] == c)
            })
            .collect()
    }
}
