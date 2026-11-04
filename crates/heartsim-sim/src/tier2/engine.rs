//! [`Tier2Engine`]: the timing engine + the monodomain solver on the voxelised cell cloud.
//!
//! **Coupling** (brief B16 §4).
//! * *Timing → tissue.* Timing events are routed to seed cells exactly as in Tier 1 (the
//!   [`EikonalEngine`] serves as router, [`EikonalEngine::route_events`]); every seed cell becomes
//!   a stimulus current on the voxels within `stim_radius_mm` of it. Conduction-system cells
//!   (AV node, His, bundles) are not part of the tissue: as in Tier 1 the timing engine owns
//!   their delays, and the fascicles deliver their impulse at the PMJ seed cells.
//! * *Tissue → timing.* Activations the timing engine did not cause (functional re-entry) are
//!   sensed at two sites and fed back with [`TimingEngine::sense_tissue`]: the atrial cells
//!   around the AV node (flutter / AF waves reach the node and conduct or block there) and the
//!   ventricular lead-tip cells (VT / VF reset the escape timers and inhibit a pacemaker).
//! * *Tissue → cells.* Voxel V_m is sampled back to every cell by trilinear interpolation over
//!   the voxels of the cell's own compartment, so the forward ECG model and the colour maps read
//!   the same [`CellState`] as with Tier 1.
//! * *Tier 1 → Tier 2.* [`Tier2Engine::from_tier1`] continues a running Tier-1 simulation: the
//!   timing engine is taken over as is and each voxel starts at the model state of its nearest
//!   cell's template phase (`(t − t_act)/APD`), mapped through the single-cell trajectory.
//!
//! **Parameters.** Each voxel takes region, fibre, base APD and fast-layer membership from its
//! nearest myocardial cell. Region CV (× `atrial_cv_scale` in the atria, `purkinje_layer_cv_m_s`
//! isotropic in the fast endocardial layer) and anisotropy ratio are turned into `(D_l, D_t)` by
//! cable calibration on the solver's own grid ([`super::calibrate`]); where the explicit limit
//! `tr D ≤ stability · h²/(2 dt)` caps D, the excitation dial supplies the remaining speed. The
//! base APD becomes the model's APD dial (single-cell bisection). Voxels with equal quantised
//! targets share one tissue class.

use super::calibrate;
use super::grid::{Compartment, NO_VOXEL, VoxelGrid, VoxelSpec};
use super::model::{AlievPanfilov, CellModel};
use super::solver::{Monodomain, Tissue};
use crate::engine::{CellState, Chamber, Engine, Event, EventKind, Origin};
use crate::tier1::{CellKind, EikonalEngine, Tier1Engine, resolve_site};
use crate::timing::{ActionError, TimingEngine, TimingParams};
use glam::Vec3;
use heartsim_core::{Action, Millis, Region};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Tier-2 settings (numerics and coupling), separate from the tissue `Parameters`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tier2Params {
    /// Voxel edge, mm (research §6.5: 2 mm, a 64³ box holds both ventricles and atria).
    pub voxel_mm: f32,
    /// Explicit Euler substep, ms.
    pub dt_ms: f32,
    /// Fibre tensor (`false` = isotropic at the longitudinal CV).
    pub fibres: bool,
    /// Fraction of the explicit diffusion limit `tr D ≤ h²/(2 dt)` the calibration may use.
    pub stability: f32,
    /// Stimulus current on top of the per-voxel load share, u per ms.
    pub stim_amplitude: f32,
    pub stim_duration_ms: f64,
    /// Seed cells excite the voxels within this radius, mm (≤ 0 → `voxel_mm`). The slow
    /// Aliev–Panfilov front is ≈ 6 mm wide and its critical nucleus has a radius of ≈ 2D/CV
    /// ≈ 5–8 mm, so a smaller stimulated volume dies out.
    pub stim_radius_mm: f32,
    /// APD targets are quantised to this step when forming tissue classes, ms.
    pub apd_step_ms: f32,
    /// A tissue activation at a sensing site within this time of a timing-engine event of the
    /// same chamber is that event's own beat and is not fed back, ms.
    pub sense_window_ms: f64,
}

impl Default for Tier2Params {
    fn default() -> Self {
        Self {
            voxel_mm: 2.0,
            dt_ms: 0.25,
            fibres: true,
            stability: 0.9,
            stim_amplitude: 1.0,
            stim_duration_ms: 2.0,
            stim_radius_mm: 5.0,
            apd_step_ms: 4.0,
            sense_window_ms: 150.0,
        }
    }
}

/// Calibrated conduction of one `(CV, anisotropy)` target.
#[derive(Debug, Clone, Copy)]
struct Conduction {
    excitation: f32,
    d_long: f32,
    d_trans: f32,
}

/// Per-cell sampling of the voxel field (CSR, trilinear weights within the compartment).
#[derive(Debug, Clone, Default)]
struct Sampler {
    row: Vec<u32>,
    voxel: Vec<u32>,
    weight: Vec<f32>,
    /// Highest-weight voxel per cell (`NO_VOXEL` = none): activation time and APD read-out.
    main: Vec<u32>,
}

/// A tissue activation sensing site (voxels) and its last feedback.
#[derive(Debug, Clone)]
struct SenseSite {
    voxels: Vec<u32>,
    region: Region,
    chamber: Chamber,
    last_timing_ms: f64,
    last_sensed_ms: f64,
}

/// Tier 2 as one [`Engine`]. See the module documentation.
#[derive(Debug, Clone)]
pub struct Tier2Engine<M: CellModel = AlievPanfilov> {
    timing: TimingEngine,
    router: EikonalEngine,
    solver: Monodomain<M>,
    params: Tier2Params,
    /// Nearest myocardial cell of each voxel.
    voxel_cell: Vec<u32>,
    /// Voxels each cell stimulates (CSR).
    stim_row: Vec<u32>,
    stim_voxel: Vec<u32>,
    sampler: Sampler,
    sense: [SenseSite; 2],
    /// Fast endocardial layer drive per bundle seed set (`rbb`, `laf`, `lpf`): layer voxels
    /// and their arrival delay after the bundle event, ms.
    layer_drive: [Vec<(u32, f32)>; 3],
    /// Seed set (index into `layer_drive`) each PMJ cell belongs to, `NO_SET` otherwise.
    pmj_set: Vec<u8>,
    last_drive_ms: [f64; 3],
    base_conducting: Vec<bool>,
    ingested: usize,
    /// Shocks already applied to the tissue (index into `timing.shocks()`).
    shocks_seen: usize,
    routed: Vec<(f64, u32)>,
    fresh: Vec<Event>,
    cells: CellState,
}

fn compartment_of(kind: CellKind) -> Option<Compartment> {
    match kind {
        CellKind::Atrial | CellKind::SinusNode => Some(Compartment::Atrial),
        CellKind::Ventricular | CellKind::Purkinje => Some(Compartment::Ventricular),
        CellKind::Driven => None,
    }
}

/// Events the tissue itself caused (fed back by sensing): never routed back as stimuli.
fn is_tissue_echo(e: &Event) -> bool {
    e.origin == Origin::Reentry && e.kind == EventKind::Automatic && e.chamber != Chamber::AvJunction
}

impl<M: CellModel> Tier2Engine<M> {
    /// Build Tier 2 on the cloud of `eikonal` (which also routes timing events to seed cells;
    /// it inherits the His–Purkinje delays, accessory pathway and pacing leads of `timing`).
    ///
    /// # Panics
    /// If `params.dt_ms` exceeds the reaction limit of `model`.
    pub fn new(timing: TimingParams, eikonal: EikonalEngine, model: &M, params: Tier2Params) -> Self {
        let timing = TimingEngine::new(timing);
        Self::assemble(timing, eikonal, model, params)
    }

    /// Continue a running Tier-1 simulation in Tier 2 (see the module documentation). The
    /// Tier-1 engine is left untouched.
    pub fn from_tier1(t1: &Tier1Engine, model: &M, params: Tier2Params) -> Self {
        let mut e = Self::assemble(t1.timing().clone(), t1.eikonal().clone(), model, params);
        e.ingested = e.timing.events().len();
        e.shocks_seen = e.timing.shocks().len();
        let now = e.timing.now().0;
        e.solver.set_time(now);
        let (t_act, apd) = (t1.eikonal().activation_times(), &t1.cells().apd_ms);
        for v in 0..e.voxel_cell.len() {
            let c = e.voxel_cell[v] as usize;
            let (t, a) = (t_act[c], apd[c]);
            if t.is_finite() && a > 0.0 {
                e.solver
                    .set_voxel_phase(v, ((now - t) / f64::from(a)) as f32, t as f32);
            }
        }
        e.sample_cells();
        e
    }

    fn assemble(timing: TimingEngine, mut router: EikonalEngine, model: &M, mut params: Tier2Params) -> Self {
        assert!(
            model.max_dt_ms() >= params.dt_ms,
            "dt {} ms exceeds the {} reaction limit {} ms",
            params.dt_ms,
            M::NAME,
            model.max_dt_ms()
        );
        if params.stim_radius_mm <= 0.0 {
            params.stim_radius_mm = params.voxel_mm;
        }
        router.set_his_purkinje(timing.params.his_purkinje.clone());
        router.set_accessory(
            timing
                .params
                .accessory
                .as_ref()
                .map(|ap| (ap.ventricular_side, ap.atrial_side)),
        );
        if let Some(cfg) = timing.params.pacer.clone() {
            router.set_pacer_leads(&cfg);
        }
        let (grid, voxel_cell) = voxelise(&router, &params);
        let (tissue, class) = calibrate_tissue(&router, &grid, &voxel_cell, model, &params);
        let solver = Monodomain::new(grid, tissue, class, params.dt_ms);
        let mut e = Self {
            base_conducting: solver.conducting().to_vec(),
            sense: [
                SenseSite {
                    voxels: Vec::new(),
                    region: Region::RightAtrium,
                    chamber: Chamber::Atrium,
                    last_timing_ms: f64::NEG_INFINITY,
                    last_sensed_ms: f64::NEG_INFINITY,
                },
                SenseSite {
                    voxels: Vec::new(),
                    region: Region::RightVentricle,
                    chamber: Chamber::Ventricle,
                    last_timing_ms: f64::NEG_INFINITY,
                    last_sensed_ms: f64::NEG_INFINITY,
                },
            ],
            layer_drive: [Vec::new(), Vec::new(), Vec::new()],
            pmj_set: Vec::new(),
            last_drive_ms: [f64::NEG_INFINITY; 3],
            timing,
            router,
            solver,
            params,
            voxel_cell,
            stim_row: Vec::new(),
            stim_voxel: Vec::new(),
            sampler: Sampler::default(),
            ingested: 0,
            shocks_seen: 0,
            routed: Vec::new(),
            fresh: Vec::new(),
            cells: CellState::default(),
        };
        e.build_maps();
        e.build_layer_drive();
        e.sample_cells();
        e
    }

    /// Arrival delays over the fast endocardial layer from each bundle's PMJ voxels: a
    /// Dijkstra on the 18-neighbour lattice of the layer's voxels at `purkinje_layer_cv_m_s`.
    /// A 2 mm grid cannot carry a 1.4 m/s, sub-voxel-thin layer as tissue (the explicit
    /// stability limit caps D), so the layer is a driver, as Purkinje networks are in hybrid
    /// models: when a bundle fires, its layer voxels are stimulated at these delays; tissue that
    /// is still refractory ignores them.
    fn build_layer_drive(&mut self) {
        use std::cmp::Reverse;
        use std::collections::BinaryHeap;
        const NO_SET: u8 = u8::MAX;
        let layers = self.router.fast_layers();
        let cv = self.router.eikonal_params().purkinje_layer_cv_m_s as f32;
        let g = self.solver.grid();
        let layer_of: Vec<u8> = self.voxel_cell.iter().map(|&c| layers[c as usize]).collect();
        let seeds = self.router.seeds();
        self.pmj_set = vec![NO_SET; self.router.cloud().len()];
        let sets = [(&seeds.rbb, 2u8), (&seeds.laf, 1), (&seeds.lpf, 1)];
        for (k, (cells, layer)) in sets.into_iter().enumerate() {
            for &c in cells {
                self.pmj_set[c as usize] = k as u8;
            }
            if cv <= 0.0 {
                continue;
            }
            let mut best = vec![f32::INFINITY; g.len()];
            let mut heap = BinaryHeap::new();
            for &c in cells {
                let r = self.stim_row[c as usize] as usize..self.stim_row[c as usize + 1] as usize;
                for &v in &self.stim_voxel[r] {
                    if layer_of[v as usize] == layer && best[v as usize] > 0.0 {
                        best[v as usize] = 0.0;
                        heap.push(Reverse((0u32, v)));
                    }
                }
            }
            while let Some(Reverse((bits, v))) = heap.pop() {
                let d = f32::from_bits(bits);
                if d > best[v as usize] {
                    continue;
                }
                let [i, j, kk] = g.ijk[v as usize].map(i64::from);
                for e in super::grid::DIRECTIONS {
                    for sign in [1i64, -1] {
                        let (di, dj, dk) = (
                            sign * i64::from(e[0]),
                            sign * i64::from(e[1]),
                            sign * i64::from(e[2]),
                        );
                        let Some(n) = g.voxel_at(i + di, j + dj, kk + dk) else {
                            continue;
                        };
                        if layer_of[n as usize] != layer {
                            continue;
                        }
                        let len = ((di * di + dj * dj + dk * dk) as f32).sqrt() * g.h_mm;
                        let nd = d + len / cv;
                        if nd < best[n as usize] {
                            best[n as usize] = nd;
                            heap.push(Reverse((nd.to_bits(), n)));
                        }
                    }
                }
            }
            self.layer_drive[k] = (0..g.len() as u32)
                .filter(|&v| best[v as usize].is_finite())
                .map(|v| (v, best[v as usize]))
                .collect();
        }
    }

    /// Cell → stimulus voxels, cell → sampling voxels, sensing sites.
    fn build_maps(&mut self) {
        let cloud = self.router.cloud().clone();
        let kinds = self.router.kinds().to_vec();
        let g = self.solver.grid();
        let h = g.h_mm;
        let n = cloud.len();
        self.stim_row = Vec::with_capacity(n + 1);
        self.stim_row.push(0);
        let mut s = Sampler {
            row: vec![0],
            main: vec![NO_VOXEL; n],
            ..Sampler::default()
        };
        for (c, (&p, &kind)) in cloud.positions.iter().zip(&kinds).enumerate() {
            let comp = compartment_of(kind);
            if let Some(comp) = comp {
                let mut vox = g.voxels_within(p, self.params.stim_radius_mm, Some(comp));
                if vox.is_empty() {
                    vox.extend(g.nearest(p, 2.0 * h, Some(comp)));
                }
                self.stim_voxel.extend(vox);
                // trilinear corners
                let l = g.lattice(p);
                let b = l.floor();
                let f = l - b;
                let start = s.voxel.len();
                for corner in 0..8u8 {
                    let d = [corner & 1, (corner >> 1) & 1, (corner >> 2) & 1].map(i64::from);
                    let Some(v) = g.voxel_at(b.x as i64 + d[0], b.y as i64 + d[1], b.z as i64 + d[2]) else {
                        continue;
                    };
                    if g.compartment[v as usize] != comp {
                        continue;
                    }
                    let w = (0..3)
                        .map(|a| if d[a] == 1 { f[a] } else { 1.0 - f[a] })
                        .product::<f32>();
                    if w > 0.0 {
                        s.voxel.push(v);
                        s.weight.push(w);
                    }
                }
                if s.voxel.len() == start
                    && let Some(v) = g.nearest(p, 2.0 * h, Some(comp))
                {
                    s.voxel.push(v);
                    s.weight.push(1.0);
                }
                let total: f32 = s.weight[start..].iter().sum();
                for w in &mut s.weight[start..] {
                    *w /= total;
                }
                s.main[c] = (start..s.voxel.len())
                    .max_by(|&a, &b| s.weight[a].total_cmp(&s.weight[b]).then(b.cmp(&a)))
                    .map_or(NO_VOXEL, |k| s.voxel[k]);
            }
            self.stim_row.push(self.stim_voxel.len() as u32);
            s.row.push(s.voxel.len() as u32);
        }
        self.sampler = s;
        let seeds = self.router.seeds();
        let ventricular_site = if seeds.ventricular_lead.is_empty() {
            &seeds.right_ventricle
        } else {
            &seeds.ventricular_lead
        };
        for (site, cells) in self.sense.iter_mut().zip([&seeds.av_input, ventricular_site]) {
            site.voxels = cells
                .iter()
                .flat_map(|&c| {
                    let r = self.stim_row[c as usize] as usize..self.stim_row[c as usize + 1] as usize;
                    self.stim_voxel[r].iter().copied()
                })
                .collect();
            site.voxels.sort_unstable();
            site.voxels.dedup();
            if let Some(&c) = cells.first() {
                site.region = cloud.regions[c as usize];
            }
        }
    }

    // --- read-out ---------------------------------------------------------------------------

    /// The reaction–diffusion state.
    #[must_use]
    pub fn solver(&self) -> &Monodomain<M> {
        &self.solver
    }

    /// Mutable solver, for the re-entry tools ([`super::reentry`]).
    pub fn solver_mut(&mut self) -> &mut Monodomain<M> {
        &mut self.solver
    }

    #[must_use]
    pub fn timing(&self) -> &TimingEngine {
        &self.timing
    }

    pub fn timing_mut(&mut self) -> &mut TimingEngine {
        &mut self.timing
    }

    /// The event router (Tier-1 engine on the same cloud; its propagation state is unused).
    #[must_use]
    pub fn router(&self) -> &EikonalEngine {
        &self.router
    }

    #[must_use]
    pub fn params(&self) -> &Tier2Params {
        &self.params
    }

    /// Nearest myocardial cell of each voxel.
    #[must_use]
    pub fn voxel_cells(&self) -> &[u32] {
        &self.voxel_cell
    }

    /// Voxels that seed cell `cell` stimulates.
    #[must_use]
    pub fn stimulus_voxels(&self, cell: u32) -> &[u32] {
        let c = cell as usize;
        &self.stim_voxel[self.stim_row[c] as usize..self.stim_row[c + 1] as usize]
    }

    /// Sample voxel V_m back to the cells (`to_cells`): trilinear within the compartment.
    fn sample_cells(&mut self) {
        let n = self.sampler.main.len();
        if self.cells.vm.len() != n {
            self.cells = CellState::new(n);
        }
        let (u, t_act) = (self.solver.u(), self.solver.activation_ms());
        let (classes, traj) = (self.solver.classes(), self.solver.trajectories());
        let s = &self.sampler;
        for c in 0..n {
            let r = s.row[c] as usize..s.row[c + 1] as usize;
            let vm: f32 = s.voxel[r.clone()]
                .iter()
                .zip(&s.weight[r])
                .map(|(&v, &w)| w * u[v as usize])
                .sum();
            self.cells.vm[c] = vm.clamp(0.0, 1.0);
            let m = s.main[c];
            if m != NO_VOXEL {
                self.cells.activation_ms[c] = t_act[m as usize];
                self.cells.apd_ms[c] = traj[usize::from(classes[m as usize])].apd_ms;
            }
        }
    }

    // --- coupling ---------------------------------------------------------------------------

    /// Route the timing events of this step to stimuli and note the chambers they drive.
    fn ingest(&mut self) {
        let evs = self.timing.events();
        if evs.len() <= self.ingested {
            return;
        }
        self.fresh.clear();
        for e in &evs[self.ingested..] {
            if is_tissue_echo(e) {
                continue;
            }
            self.fresh.push(*e);
            if e.kind != EventKind::Blocked {
                let chamber = match e.chamber {
                    Chamber::Atrium => Chamber::Atrium,
                    Chamber::AvJunction | Chamber::Ventricle => Chamber::Ventricle,
                };
                for site in &mut self.sense {
                    if site.chamber == chamber {
                        site.last_timing_ms = site.last_timing_ms.max(e.at.0);
                    }
                }
            }
        }
        self.ingested = evs.len();
        let mut routed = std::mem::take(&mut self.routed);
        routed.clear();
        self.router.route_events(&self.fresh, &mut routed);
        let now = self.solver.now_ms();
        for &(t, cell) in &routed {
            let c = cell as usize;
            if c + 1 >= self.stim_row.len() {
                continue;
            }
            if let Some(&k) = self.pmj_set.get(c)
                && let Some(drive) = self.layer_drive.get(usize::from(k))
                && (t - self.last_drive_ms[usize::from(k)]).abs() > 1.0
            {
                self.last_drive_ms[usize::from(k)] = t;
                for &(v, delay) in drive {
                    self.solver.stimulate(
                        &[v],
                        t.max(now) + f64::from(delay),
                        self.params.stim_duration_ms,
                        self.params.stim_amplitude,
                    );
                }
            }
            let r = self.stim_row[c] as usize..self.stim_row[c + 1] as usize;
            if r.is_empty() {
                continue;
            }
            let vox = &self.stim_voxel[r];
            self.solver.stimulate(
                vox,
                t.max(now),
                self.params.stim_duration_ms,
                self.params.stim_amplitude,
            );
        }
        self.routed = routed;
    }

    /// Feed activations at the sensing sites that no timing event explains back to the timing
    /// engine.
    fn sense_tissue(&mut self, from_ms: f64) {
        let t_act = self.solver.activation_ms();
        let window = self.params.sense_window_ms;
        for site in &mut self.sense {
            let first = site
                .voxels
                .iter()
                .map(|&v| f64::from(t_act[v as usize]))
                .filter(|t| t.is_finite() && *t > from_ms)
                .fold(f64::INFINITY, f64::min);
            if first.is_finite()
                && first - site.last_timing_ms > window
                && first - site.last_sensed_ms > window
            {
                site.last_sensed_ms = first;
                self.timing.sense_tissue(site.chamber, site.region, Millis(first));
            }
        }
        // the echo events appear in the timing engine's next step and are skipped by `ingest`
    }

    /// Shocks delivered since the last call (see [`crate::defib`]).
    pub fn drain_shocks(&mut self) -> Vec<crate::defib::ShockRecord> {
        self.shocks_seen = 0;
        self.timing.drain_shocks()
    }

    /// Apply an action. Spatial actions act on the tissue (stimuli at their site, ablation,
    /// regional block); the timing engine sees every action. Succeeds if either honours
    /// it, otherwise returns the timing engine's error.
    pub fn apply(&mut self, action: &Action, at: Millis) -> Result<(), ActionError> {
        let t = self.timing.apply(action, at);
        if matches!(action, Action::AccessoryPathway { .. }) {
            self.router.set_accessory(
                self.timing
                    .params
                    .accessory
                    .as_ref()
                    .map(|ap| (ap.ventricular_side, ap.atrial_side)),
            );
        }
        let start = at.0.max(self.solver.now_ms());
        let site_voxels = |e: &Self, site| {
            resolve_site(e.router.cloud(), site)
                .map(|c| e.stimulus_voxels(c).to_vec())
                .filter(|v| !v.is_empty())
                .ok_or_else(|| ActionError::UnsupportedSite(format!("{site:?}")))
        };
        let (dur, amp) = (self.params.stim_duration_ms, self.params.stim_amplitude);
        let spatial = match action {
            Action::Stimulate {
                site, duration_ms, ..
            } => site_voxels(self, site).map(|vox| {
                self.solver.stimulate(&vox, start, duration_ms.max(dur), amp);
            }),
            Action::PacingTrain {
                site,
                s1_cycle_ms,
                s1_count,
                s2_ms,
                s3_ms,
            } => site_voxels(self, site).map(|vox| {
                let mut t = start;
                for i in 0..*s1_count {
                    t = start + f64::from(i) * s1_cycle_ms;
                    self.solver.stimulate(&vox, t, dur, amp);
                }
                for extra in [s2_ms, s3_ms].into_iter().map_while(|x| *x) {
                    t += extra;
                    self.solver.stimulate(&vox, t, dur, amp);
                }
            }),
            Action::Ablate { site, radius_mm } => resolve_site(self.router.cloud(), site)
                .ok_or_else(|| ActionError::UnsupportedSite(format!("{site:?}")))
                .map(|c| {
                    let p = self.router.cloud().positions[c as usize];
                    let vox = self.solver.grid().voxels_within(p, *radius_mm as f32, None);
                    self.solver.set_conducting(&vox, false);
                }),
            Action::Block { region, blocked } => {
                let cloud = self.router.cloud().clone();
                let vox: Vec<u32> = (0..self.voxel_cell.len() as u32)
                    .filter(|&v| cloud.regions[self.voxel_cell[v as usize] as usize] == *region)
                    .collect();
                if vox.is_empty() {
                    Err(ActionError::UnsupportedRegion(*region))
                } else {
                    let keep: Vec<u32> = vox
                        .into_iter()
                        .filter(|&v| self.base_conducting[v as usize])
                        .collect();
                    self.solver.set_conducting(&keep, !*blocked);
                    Ok(())
                }
            }
            // The timing engine decides delivery (sync on R) and outcome; `step` applies it.
            Action::Shock { .. } => Err(ActionError::Unsupported("Shock")),
            Action::SetGlobalSinusRate { .. } => Err(ActionError::Unsupported("SetGlobalSinusRate")),
            Action::SetRegion { .. } => Err(ActionError::Unsupported("SetRegion")),
            Action::AccessoryPathway { .. } => Err(ActionError::Unsupported("AccessoryPathway")),
            Action::Stenosis { .. } => Err(ActionError::Unsupported("Stenosis")),
            Action::Drug { .. } => Err(ActionError::Unsupported("Drug")),
            Action::MoveElectrode { .. } => Err(ActionError::Unsupported("MoveElectrode")),
        };
        match (t, spatial) {
            (Ok(()), _) | (_, Ok(())) => Ok(()),
            (Err(err), Err(_)) => Err(err),
        }
    }
}

impl<M: CellModel> Engine for Tier2Engine<M> {
    fn step(&mut self, dt: Millis) {
        let from = self.solver.now_ms();
        self.timing.step(dt);
        self.ingest();
        for i in self.shocks_seen..self.timing.shocks().len() {
            let s = self.timing.shocks()[i];
            if s.success {
                self.solver.advance_to(s.at.0);
                self.solver.shock();
            }
        }
        self.shocks_seen = self.timing.shocks().len();
        self.solver.advance_to(self.timing.now().0);
        self.sense_tissue(from);
        self.sample_cells();
    }

    fn now(&self) -> Millis {
        self.timing.now()
    }

    fn cells(&self) -> &CellState {
        &self.cells
    }

    fn drain_events(&mut self) -> Vec<Event> {
        self.ingested = 0;
        self.timing.drain_events()
    }

    fn reset(&mut self) {
        self.timing.reset();
        self.router.reset();
        self.ingested = 0;
        self.shocks_seen = 0;
        let ablated: Vec<u32> = (0..self.base_conducting.len() as u32)
            .filter(|&v| self.solver.conducting()[v as usize] != self.base_conducting[v as usize])
            .collect();
        if !ablated.is_empty() {
            self.solver.set_conducting(&ablated, true);
        }
        self.solver.reset();
        for site in &mut self.sense {
            site.last_timing_ms = f64::NEG_INFINITY;
            site.last_sensed_ms = f64::NEG_INFINITY;
        }
        self.last_drive_ms = [f64::NEG_INFINITY; 3];
        self.sample_cells();
    }
}

// --- construction helpers ---------------------------------------------------------------------

/// Median nearest-neighbour distance of the cloud, mm.
fn cell_spacing(router: &EikonalEngine) -> f32 {
    let cloud = router.cloud();
    let mut d: Vec<f32> = cloud
        .neighbours
        .iter()
        .map(|n| n[0].distance_mm)
        .filter(|d| *d > 0.0)
        .collect();
    if d.is_empty() {
        return 1.0;
    }
    let mid = d.len() / 2;
    *d.select_nth_unstable_by(mid, f32::total_cmp).1
}

/// Voxelise the conducting myocardial cells: a voxel is active when a cell lies within
/// 0.9 × the cloud spacing of its centre (≥ the half-diagonal of the cloud lattice, so the
/// tissue has no holes), and takes that cell's compartment and fibre.
fn voxelise(router: &EikonalEngine, params: &Tier2Params) -> (VoxelGrid, Vec<u32>) {
    let cloud = router.cloud();
    let kinds = router.kinds();
    let rp = router.parameters();
    let myo: Vec<u32> = (0..cloud.len() as u32)
        .filter(|&c| {
            let r = rp.region(cloud.regions[c as usize]);
            compartment_of(kinds[c as usize]).is_some() && r.conducts && r.cv_long_m_s > 0.0
        })
        .collect();
    let pts: Vec<Vec3> = myo.iter().map(|&c| cloud.positions[c as usize]).collect();
    let h = params.voxel_mm;
    let (lo, hi) = pts.iter().fold(
        (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
        |(a, b), p| (a.min(*p), b.max(*p)),
    );
    if pts.is_empty() {
        return (
            VoxelGrid::from_fn([1, 1, 1], h, Vec3::ZERO, |_, _| None),
            Vec::new(),
        );
    }
    let origin = lo - Vec3::splat(h);
    let ext = ((hi - lo) / h).ceil();
    let dims = [ext.x as usize + 3, ext.y as usize + 3, ext.z as usize + 3];
    let r_max = 0.9 * cell_spacing(router);
    // nearest myocardial cell of every voxel centre within r_max: each cell stamps the voxels
    // around it (cells in index order, strict `<`, so ties go to the lower cell index)
    let mut best: Vec<(f32, u32)> = vec![(f32::INFINITY, u32::MAX); dims[0] * dims[1] * dims[2]];
    let reach = (r_max / h).ceil() as i64;
    // dims < u16::MAX (VoxelGrid invariant)
    let last = dims.map(|d| i64::try_from(d).unwrap_or(i64::MAX) - 1);
    for (k, p) in pts.iter().enumerate() {
        let l = (*p - origin) / h;
        let (ci, cj, ck) = (l.x.round() as i64, l.y.round() as i64, l.z.round() as i64);
        for z in (ck - reach).max(0)..=(ck + reach).min(last[2]) {
            for y in (cj - reach).max(0)..=(cj + reach).min(last[1]) {
                for x in (ci - reach).max(0)..=(ci + reach).min(last[0]) {
                    let centre = origin + Vec3::new(x as f32, y as f32, z as f32) * h;
                    let d = centre.distance(*p);
                    let slot = &mut best[x as usize + dims[0] * (y as usize + dims[1] * z as usize)];
                    if d <= r_max && d < slot.0 {
                        *slot = (d, k as u32);
                    }
                }
            }
        }
    }
    let mut voxel_cell = Vec::new();
    let grid = VoxelGrid::from_fn(dims, h, origin, |[x, y, z], _| {
        let (_, k) = best[x + dims[0] * (y + dims[1] * z)];
        if k == u32::MAX {
            return None;
        }
        let c = myo[k as usize];
        voxel_cell.push(c);
        Some(VoxelSpec {
            compartment: compartment_of(kinds[c as usize])?,
            fibre: if params.fibres {
                cloud.fibres[c as usize]
            } else {
                Vec3::ZERO
            },
        })
    });
    (grid, voxel_cell)
}

/// Tissue classes for the voxels (module documentation, *Parameters*).
fn calibrate_tissue<M: CellModel>(
    router: &EikonalEngine,
    grid: &VoxelGrid,
    voxel_cell: &[u32],
    model: &M,
    params: &Tier2Params,
) -> (Vec<Tissue<M>>, Vec<u16>) {
    let cloud = router.cloud();
    let kinds = router.kinds();
    let rp = router.parameters();
    let eik = router.eikonal_params();
    let base_apd = router.base_apd_ms();
    let (h, dt) = (params.voxel_mm, params.dt_ms);
    let mut conduction: BTreeMap<(u32, u32), Conduction> = BTreeMap::new();
    let mut classes: BTreeMap<(u32, u32, u32), u16> = BTreeMap::new();
    let mut tissue = Vec::new();
    let mut class = Vec::with_capacity(grid.len());
    for &c in voxel_cell {
        let c = c as usize;
        let p = rp.region(cloud.regions[c]);
        let (cv, rho) = if kinds[c] == CellKind::Atrial {
            (p.cv_long_m_s * eik.atrial_cv_scale, p.anisotropy_ratio)
        } else {
            (p.cv_long_m_s, p.anisotropy_ratio)
        };
        let rho = if params.fibres { rho.clamp(0.05, 1.0) } else { 1.0 };
        let (cv, rho) = (cv as f32, rho as f32);
        let cv_key = ((cv * 1000.0).round() as u32, (rho * 100.0).round() as u32);
        let apd_key = (base_apd[c] / params.apd_step_ms).round() as u32;
        let key = (cv_key.0, cv_key.1, apd_key);
        let id = *classes.entry(key).or_insert_with(|| {
            let cond = *conduction.entry(cv_key).or_insert_with(|| {
                calibrate_conduction(
                    model,
                    cv_key.0 as f32 / 1000.0,
                    cv_key.1 as f32 / 100.0,
                    h,
                    dt,
                    params.stability,
                )
            });
            let excited = model.scaled_excitation(cond.excitation);
            let (m, _) = calibrate::solve_apd(&excited, apd_key as f32 * params.apd_step_ms, dt);
            tissue.push(Tissue {
                model: m,
                d_long: cond.d_long,
                d_trans: cond.d_trans,
            });
            (tissue.len() - 1) as u16
        });
        class.push(id);
    }
    if tissue.is_empty() {
        tissue.push(Tissue {
            model: model.clone(),
            d_long: 0.0,
            d_trans: 0.0,
        });
    }
    (tissue, class)
}

fn calibrate_conduction<M: CellModel>(
    model: &M,
    cv: f32,
    rho: f32,
    h: f32,
    dt: f32,
    stability: f32,
) -> Conduction {
    let share = 1.0 + 2.0 * rho * rho;
    let d_max = calibrate::trace_budget(model, h, dt, stability) / share;
    let (mut d_long, achieved) = calibrate::solve_d(model, cv, h, dt, d_max);
    let mut excitation = 1.0;
    if achieved < 0.98 * cv {
        let (s, d, _) = calibrate::solve_excitation(model, cv, h, dt, |m| {
            calibrate::trace_budget(m, h, dt, stability) / share
        });
        excitation = s;
        d_long = d;
    }
    let excited = model.scaled_excitation(excitation);
    let d_trans = if rho >= 0.999 {
        d_long
    } else {
        let room = ((calibrate::trace_budget(&excited, h, dt, stability) - d_long) / 2.0).max(1e-6);
        calibrate::solve_d(&excited, rho * cv, h, dt, room.min(d_long)).0
    };
    Conduction {
        excitation,
        d_long,
        d_trans,
    }
}
