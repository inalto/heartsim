//! Tier 1: anisotropic graph-eikonal activation on the cell cloud + action-potential templates.
//!
//! **Model** (`docs/ARCHITECTURE.md` §4.2, research §2.2). Each timing event (sinus discharge,
//! fascicle arrival, focus, pacing spike, accessory-pathway crossing) starts a wavefront at its
//! seed cells ([`SeedMap`]). Fronts spread over a radius graph ([`Graph`]) with edge delay
//!
//! ```text
//! τ_ij = d_ij / CV_long,j · sqrt(c² + (1 − c²) / ρ_j²)        c = |cos(edge, fibre_j)|
//! ```
//!
//! — the travel time of an elliptic front whose speed is `CV_long` along the fibre and
//! `ρ·CV_long` across it (ρ = `anisotropy_ratio`), so a planar wave across the fibres moves at
//! exactly `ρ·CV_long`. All fronts share one binary-heap Dijkstra; a cell is settled by the first
//! front to reach it, which is what makes colliding fronts annihilate. The heap is advanced
//! lazily, only up to the present, so a beat's front appears progressively and its cost (≈ one
//! Dijkstra per beat) is spread over the steps of that beat.
//!
//! **Refractoriness and restitution** (research §2.5.5). A cell accepts an activation at `t`
//! only if `t ≥ t_last + APD·erp_fraction`; fronts arriving earlier are walls. CV into a cell is
//! scaled by `1 − a_cv·exp(−DI/τ_cv)` with `DI = t − (t_last + APD)` — negative inside the relative
//! refractory window — so premature beats conduct slowly (aberrancy); the new APD is
//! `APD_base·(1 − a·exp(−DI/τ))`.
//!
//! **Heterogeneity** (research §3.5). `APD_base` blends `apd_endo_ms` (depth 0) to `apd_ms`
//! (depth 1) and adds an apicobasal gradient and an RV offset ([`EikonalParams`]); per-cell
//! `apd_scale`/`cv_scale` are the hooks for ischaemia and drugs (brief B14).
//!
//! **His–Purkinje.** The procedural pack has no Purkinje tree; a thin fast endocardial layer stands
//! in for it (Gillette et al. 2021). Only fronts delivered by the layer's own bundle (LAF/LPF for
//! the LV, RBB for the RV) use the fast speed, so in bundle-branch block the blocked ventricle is
//! activated slowly through the myocardium, as in vivo. Conduction-system cells (AV node, His,
//! bundles) are driven by timing events, never by neighbours: the timing engine owns those
//! delays, and the mesh cannot open a second AV path.
//!
//! **Determinism.** Fixed-order graph, `f64` arrival times, ties broken by cell index; the V_m
//! update is per-cell (parallel with the `parallel` feature, still bit-identical).

mod coupled;
/// Radius-neighbourhood propagation graph.
pub mod graph;
/// Tier-1 settings ([`EikonalParams`]).
pub mod params;
/// Timing event → seed cells ([`SeedMap`]).
pub mod seeds;

pub use coupled::{LEAD_FIELD_RADIUS_MM, LeadProbe, Tier1Engine};
pub use graph::{CellKind, Graph};
pub use params::EikonalParams;
pub use seeds::{SeedMap, lead_cells, resolve_site, side_index};

use crate::ap::ApTemplate;
use crate::engine::{CellState, Chamber, Engine, Event, EventKind, Origin};
use crate::timing::{ActionError, HisPurkinjeParams, PacerConfig, Side};
use glam::Vec3;
use heartsim_core::{Action, Millis, Parameters, Region, RegionParams, StimSite};
use heartsim_geometry::CellCloud;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BinaryHeap};
use std::sync::Arc;

const TEMPLATE_SAMPLES: usize = 256;
/// Cells per V_m work item (parallel chunks).
const VM_CHUNK: usize = 8192;
/// Resolution of the V_m lookup tables (0.3 ms at APD 300 ms).
const VM_LUT: usize = 1024;
const REGIONS: usize = Region::ALL.len();
const NO_PROBE: u8 = u8::MAX;
/// Probe indices.
const PROBE_AV: u8 = 0;
const PROBE_AP_VENTRICLE: u8 = 1;
const PROBE_AP_ATRIUM: u8 = 2;
/// Time over which a nodal-pathway event lights its cells, ms (colour map only).
const NODAL_PATHWAY_SPAN_MS: f64 = 20.0;
/// A His event lights the compact node from the last pathway event only if it is this recent.
const NODAL_PAIRING_MS: f64 = 500.0;

/// Fast endocardial layer ids (`0` = none).
const LAYER_LV: u8 = 1;
const LAYER_RV: u8 = 2;

/// Which template a cell plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum Shape {
    Atrial = 0,
    Ventricular = 1,
    Purkinje = 2,
    Nodal = 3,
}

impl Shape {
    fn of(region: Region) -> Self {
        match region {
            Region::SinoatrialNode
            | Region::AvNodeCompact
            | Region::AvNodeFastPathway
            | Region::AvNodeSlowPathway => Shape::Nodal,
            Region::HisBundle
            | Region::RightBundleBranch
            | Region::LeftBundleBranch
            | Region::LeftAnteriorFascicle
            | Region::LeftPosteriorFascicle
            | Region::PurkinjeLeft
            | Region::PurkinjeRight => Shape::Purkinje,
            r if r.is_atrial() || r == Region::AccessoryPathway => Shape::Atrial,
            _ => Shape::Ventricular,
        }
    }
}

/// A pending arrival in the shared Dijkstra heap.
#[derive(Debug, Clone, Copy)]
struct Front {
    at: f64,
    cell: u32,
    /// Fast endocardial layer this front may use (`0` = none).
    layer: u8,
    /// Probe the front started in (it does not trigger that probe again).
    src_probe: u8,
    origin: Origin,
}

impl PartialEq for Front {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for Front {}
impl PartialOrd for Front {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Front {
    /// Reversed so that `BinaryHeap` pops the earliest arrival; ties by lower cell index.
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .at
            .total_cmp(&self.at)
            .then_with(|| other.cell.cmp(&self.cell))
            .then_with(|| other.layer.cmp(&self.layer))
    }
}

/// A set of cells whose first activation per wave is reported as a mesh-level [`Event`].
#[derive(Debug, Clone)]
struct Probe {
    region: Region,
    last_fired: f64,
}

/// Per-cell state touched by every edge relaxation, packed for cache locality.
#[derive(Debug, Clone, Copy)]
struct Hot {
    /// Earliest time a new activation is accepted: `t_act + APD·erp_fraction`, `−∞` before
    /// the first activation, `+∞` while the cell is not excitable (ablated, blocked, scar).
    refractory_until: f64,
    /// Earliest arrival already queued for this cell (lazy-deletion Dijkstra).
    tentative: f64,
    /// End of the last action potential, `t_act + APD`, ms (`NaN` = never activated).
    repol_at: f64,
    /// 1 / CV modifier (ischaemia, drugs).
    inv_cv_scale: f32,
    region: u8,
    layer: u8,
    excitable: bool,
}

impl Hot {
    const REST: Hot = Hot {
        refractory_until: f64::NEG_INFINITY,
        tentative: f64::INFINITY,
        repol_at: f64::NAN,
        inv_cv_scale: 1.0,
        region: 0,
        layer: 0,
        excitable: true,
    };
}

/// Per-edge data of the relaxation loop (8 bytes: a beat streams the whole edge array once, so
/// its size sets the cost on large clouds).
#[derive(Debug, Clone, Copy)]
struct HotEdge {
    target: u32,
    /// Base delay into `target`, ms (`INFINITY` into a non-conducting region).
    tau_ms: f32,
}

/// CV restitution constants of a region: factor `1 − a·exp(−DI/τ)`, taken as 1 once
/// `DI ≥ di_cutoff_ms` (where the term is below 1e-4).
#[derive(Debug, Clone, Copy)]
struct CvRest {
    a: f64,
    inv_tau: f64,
    di_cutoff_ms: f64,
}

/// Restitution terms below this are dropped (saves an `exp` per relaxation in steady rhythm).
const RESTITUTION_EPSILON: f64 = 1e-4;

/// Which seed set to start a front from (avoids cloning sets on the hot path).
#[derive(Debug, Clone, Copy)]
enum SeedSet {
    Sinus,
    AvInput,
    Rbb,
    Laf,
    Lpf,
    AtrialLead,
    VentricularLead,
    LvLead,
    VentricularEscape,
    ApAtrial(usize),
    ApVentricular(usize),
    Focus(usize),
    Chamber(Region),
}

/// The Tier-1 activation engine. See the module documentation for the model.
#[derive(Debug, Clone)]
pub struct EikonalEngine {
    cloud: Arc<CellCloud>,
    params: Parameters,
    eik: EikonalParams,
    graph: Graph,
    kinds: Vec<CellKind>,
    /// Apex–base coordinate, 0 at `lv_apex` … 1 at the `his` landmark (0.5 without landmarks).
    apex_base: Vec<f32>,
    /// Position along a conduction-system structure, 0 proximal … 1 distal.
    structure_s: Vec<f32>,
    /// Region parameters indexed by `Region as usize`.
    rp: [RegionParams; REGIONS],
    /// APD at long diastolic interval before modifiers, ms.
    base_apd: Vec<f32>,
    apd_scale: Vec<f32>,
    /// Relaxation data per graph edge (same order as `graph.targets`).
    edges: Vec<HotEdge>,
    cv_rest: [CvRest; REGIONS],
    /// Scenario ablation (survives `reset`) and the current one (adds `Action::Ablate`).
    ablated_base: Vec<bool>,
    ablated: Vec<bool>,
    region_blocked: [bool; REGIONS],
    shape: Vec<Shape>,
    /// AP templates sampled at `VM_LUT` points, indexed by `Shape`.
    vm_lut: Vec<[f32; VM_LUT]>,
    seeds: SeedMap,
    his_purkinje: HisPurkinjeParams,
    region_cells: Vec<Vec<u32>>,
    probe_of: Vec<u8>,
    probes: Vec<Probe>,
    accessory: Option<(Side, Side)>,

    // --- dynamic state ---
    now: f64,
    hot: Vec<Hot>,
    /// Last activation per cell, ms (`NaN` = never), for read-out and the V_m update.
    t_act: Vec<f64>,
    /// 1 / APD of the last activation (`NaN` = ablated) for the V_m update.
    inv_apd: Vec<f32>,
    origin: Vec<Origin>,
    heap: BinaryHeap<Front>,
    /// External stimuli `(time, cell)` from actions, ascending.
    stimuli: Vec<(f64, u32)>,
    last_av_pathway: Option<f64>,
    cells: CellState,
    events: Vec<Event>,
}

impl EikonalEngine {
    /// Build the engine on `cloud` with seed sets derived from the cloud's landmarks only (no
    /// pack PMJ sets: they are rebuilt geometrically). Prefer [`EikonalEngine::with_seed_sets`]
    /// with `AnatomyPack::landmarks.sets`.
    #[must_use]
    pub fn new(cloud: Arc<CellCloud>, params: &Parameters) -> Self {
        Self::with_seed_sets(cloud, params, EikonalParams::default(), &BTreeMap::new())
    }

    /// Build the engine with explicit Tier-1 settings and the pack's named seed sets
    /// (`rbb`, `laf`, `lpf`).
    #[must_use]
    pub fn with_seed_sets(
        cloud: Arc<CellCloud>,
        params: &Parameters,
        eik: EikonalParams,
        sets: &BTreeMap<String, Vec<u32>>,
    ) -> Self {
        let n = cloud.len();
        let kinds: Vec<CellKind> = cloud.regions.iter().map(|r| CellKind::of(*r)).collect();
        let graph = Graph::build(&cloud, &kinds, eik.neighbourhood_factor, eik.max_edges);
        let apex_base = apex_base_coordinate(&cloud);
        let layer = fast_layer(&cloud, &kinds, &apex_base, &eik);
        let structure_s = structure_coordinate(&cloud);
        let seeds = SeedMap::derive(&cloud, sets, &kinds, &apex_base, eik.av_input_radius_mm);
        let mut region_cells = vec![Vec::new(); REGIONS];
        for (i, r) in cloud.regions.iter().enumerate() {
            region_cells[*r as usize].push(i as u32);
        }
        let shape = cloud.regions.iter().map(|r| Shape::of(*r)).collect();
        let mut probe_of = vec![NO_PROBE; n];
        for &c in &seeds.av_input {
            probe_of[c as usize] = PROBE_AV;
        }
        let mut e = Self {
            rp: [RegionParams::working_myocardium(300.0, 0.6); REGIONS],
            params: params.clone(),
            eik,
            edges: Vec::new(),
            cv_rest: [CvRest {
                a: 0.0,
                inv_tau: 0.0,
                di_cutoff_ms: f64::NEG_INFINITY,
            }; REGIONS],
            base_apd: vec![0.0; n],
            apd_scale: vec![1.0; n],
            ablated_base: vec![false; n],
            ablated: vec![false; n],
            region_blocked: [false; REGIONS],
            shape,
            vm_lut: [
                ApTemplate::atrial(TEMPLATE_SAMPLES),
                ApTemplate::ventricular(TEMPLATE_SAMPLES),
                ApTemplate::purkinje(TEMPLATE_SAMPLES),
                ApTemplate::nodal(TEMPLATE_SAMPLES),
            ]
            .iter()
            .map(|t| std::array::from_fn(|i| t.at(i as f32 / VM_LUT as f32)))
            .collect(),
            seeds,
            his_purkinje: HisPurkinjeParams::default(),
            region_cells,
            probe_of,
            probes: vec![
                Probe {
                    region: Region::AvNodeCompact,
                    last_fired: f64::NEG_INFINITY,
                },
                Probe {
                    region: Region::AccessoryPathway,
                    last_fired: f64::NEG_INFINITY,
                },
                Probe {
                    region: Region::AccessoryPathway,
                    last_fired: f64::NEG_INFINITY,
                },
            ],
            accessory: None,
            now: 0.0,
            hot: (0..n)
                .map(|i| Hot {
                    region: cloud.regions[i] as u8,
                    layer: layer[i],
                    ..Hot::REST
                })
                .collect(),
            t_act: vec![f64::NAN; n],
            inv_apd: vec![0.0; n],
            origin: vec![Origin::Sinus; n],
            heap: BinaryHeap::with_capacity(4096),
            stimuli: Vec::new(),
            last_av_pathway: None,
            cells: CellState::new(n),
            events: Vec::with_capacity(16),
            kinds,
            graph,
            apex_base,
            structure_s,
            cloud,
        };
        e.refresh_parameters();
        e
    }

    // --- configuration ----------------------------------------------------------------------

    /// Recompute per-region tables, base APDs and edge delays from `self.params`.
    fn refresh_parameters(&mut self) {
        for r in Region::ALL {
            let p = self.params.region(r);
            self.rp[r as usize] = p;
            let a = p.cv_rest_amplitude;
            self.cv_rest[r as usize] = CvRest {
                a,
                inv_tau: 1.0 / p.cv_rest_tau_ms.max(1e-6),
                di_cutoff_ms: if a > 0.0 {
                    p.cv_rest_tau_ms * (a / RESTITUTION_EPSILON).ln()
                } else {
                    f64::NEG_INFINITY
                },
            };
        }
        let eik = &self.eik;
        for i in 0..self.cloud.len() {
            let region = self.cloud.regions[i];
            let p = self.rp[region as usize];
            let depth = f64::from(self.cloud.depth[i].clamp(0.0, 1.0));
            let mut apd = match p.apd_endo_ms {
                Some(endo) => endo + (p.apd_ms - endo) * depth,
                None => p.apd_ms,
            };
            if self.kinds[i] == CellKind::Ventricular {
                let z = f64::from(self.apex_base[i].clamp(0.0, 1.0));
                apd += eik.apd_apicobasal_ms * (z - 0.5);
                if region == Region::RightVentricle {
                    apd += eik.apd_rv_offset_ms;
                }
            }
            self.base_apd[i] = apd.max(1.0) as f32;
        }
        let g = &self.graph;
        self.edges = (0..g.edge_count())
            .map(|e| {
                let j = g.targets[e] as usize;
                let p = self.rp[self.cloud.regions[j] as usize];
                let scale = if self.kinds[j] == CellKind::Atrial {
                    self.eik.atrial_cv_scale
                } else {
                    1.0
                };
                let (v, rho) = (p.cv_long_m_s * scale, p.anisotropy_ratio);
                let tau_ms = if v <= 0.0 || rho <= 0.0 {
                    f32::INFINITY
                } else {
                    let c = f64::from(g.alignment[e]);
                    let stretch = (c * c + (1.0 - c * c) / (rho * rho)).sqrt();
                    (f64::from(g.distance_mm[e]) / v * stretch) as f32
                };
                HotEdge {
                    target: g.targets[e],
                    tau_ms,
                }
            })
            .collect();
        self.refresh_excitable();
    }

    /// Recompute the per-cell excitability flag (ablation, region block, `conducts`).
    fn refresh_excitable(&mut self) {
        for (j, h) in self.hot.iter_mut().enumerate() {
            let r = h.region as usize;
            h.excitable = !self.ablated[j] && !self.region_blocked[r] && self.rp[r].conducts;
            let t = self.t_act[j];
            h.refractory_until = if !h.excitable {
                f64::INFINITY
            } else if t.is_nan() {
                f64::NEG_INFINITY
            } else {
                t + f64::from(self.cells.apd_ms[j]) * self.rp[r].erp_fraction
            };
            if self.ablated[j] {
                self.inv_apd[j] = f32::NAN;
            }
        }
    }

    /// Replace the tissue parameters (e.g. after a scenario change); state is kept.
    pub fn set_parameters(&mut self, params: &Parameters) {
        self.params = params.clone();
        self.refresh_parameters();
    }

    /// His–Purkinje delays of the timing engine, used to spread a branch event over its cells.
    pub fn set_his_purkinje(&mut self, hp: HisPurkinjeParams) {
        self.his_purkinje = hp;
    }

    /// Configure the accessory pathway `(ventricular side, atrial side)` so that the mesh
    /// reports fronts reaching its insertions (`None` = no pathway).
    pub fn set_accessory(&mut self, sides: Option<(Side, Side)>) {
        for p in &mut self.probe_of {
            if *p == PROBE_AP_VENTRICLE || *p == PROBE_AP_ATRIUM {
                *p = NO_PROBE;
            }
        }
        self.accessory = sides;
        if let Some((v, a)) = sides {
            for &c in &self.seeds.ap_ventricular[side_index(v)] {
                self.probe_of[c as usize] = PROBE_AP_VENTRICLE;
            }
            for &c in &self.seeds.ap_atrial[side_index(a)] {
                self.probe_of[c as usize] = PROBE_AP_ATRIUM;
            }
        }
    }

    /// Mark cells permanently non-excitable (scenario `ablated_cells`; kept across `reset`).
    pub fn set_ablated_cells(&mut self, cells: &[u32]) {
        self.ablated_base.iter_mut().for_each(|a| *a = false);
        for &c in cells {
            if let Some(a) = self.ablated_base.get_mut(c as usize) {
                *a = true;
            }
        }
        self.ablated.clone_from(&self.ablated_base);
        self.refresh_excitable();
    }

    /// Place ectopic focus `id` at `site` (default: the chamber's landmark).
    pub fn set_focus_site(&mut self, id: u8, site: &StimSite) {
        let cloud = Arc::clone(&self.cloud);
        self.seeds.set_focus_site(&cloud, id, site);
    }

    /// Move the pacemaker leads.
    pub fn set_lead_sites(&mut self, atrial: Option<&StimSite>, ventricular: Option<&StimSite>) {
        if let Some(s) = atrial {
            self.seeds.atrial_lead = resolve_site(&self.cloud, s).into_iter().collect();
        }
        if let Some(s) = ventricular {
            self.seeds.ventricular_lead = resolve_site(&self.cloud, s).into_iter().collect();
        }
    }

    /// Place the pacemaker leads at the sites of `cfg` (atrial, ventricular and, for a
    /// biventricular device, LV lead; brief B13). A His-bundle lead seeds nothing itself: the
    /// timing engine routes its stimulus through the His bundle and the fascicles.
    pub fn set_pacer_leads(&mut self, cfg: &PacerConfig) {
        let cloud = Arc::clone(&self.cloud);
        self.seeds.atrial_lead = lead_cells(&cloud, cfg.atrial_lead.site);
        self.seeds.ventricular_lead = lead_cells(&cloud, cfg.ventricular_lead.site);
        if let Some(lv) = &cfg.lv_lead {
            self.seeds.lv_lead = lead_cells(&cloud, lv.site);
        }
    }

    /// Per-cell APD multipliers (ischaemia, drugs — brief B14). Applied at the next activation.
    pub fn apd_scale_mut(&mut self) -> &mut [f32] {
        &mut self.apd_scale
    }

    /// Set per-cell CV multipliers (ischaemia, drugs — brief B14); `scale[i]` applies to cell
    /// `i` immediately. Missing entries keep their value.
    pub fn set_cv_scale(&mut self, scale: &[f32]) {
        for (h, s) in self.hot.iter_mut().zip(scale) {
            h.inv_cv_scale = 1.0 / s.max(1e-6);
        }
    }

    // --- read-out ---------------------------------------------------------------------------

    /// The cell cloud the engine runs on (indices of every per-cell slice refer to it).
    #[must_use]
    pub fn cloud(&self) -> &Arc<CellCloud> {
        &self.cloud
    }

    /// The propagation graph (radius neighbourhood, insulated atria/ventricles).
    #[must_use]
    pub fn graph(&self) -> &Graph {
        &self.graph
    }

    /// Cells each kind of timing event starts a front from.
    #[must_use]
    pub fn seeds(&self) -> &SeedMap {
        &self.seeds
    }

    /// How each cell takes part in propagation.
    #[must_use]
    pub fn kinds(&self) -> &[CellKind] {
        &self.kinds
    }

    /// Apex–base coordinate per cell (0 apex … 1 base).
    #[must_use]
    pub fn apex_base(&self) -> &[f32] {
        &self.apex_base
    }

    /// Last activation time per cell, ms (`NaN` = never), at full precision.
    #[must_use]
    pub fn activation_times(&self) -> &[f64] {
        &self.t_act
    }

    /// Origin of the front that last activated each cell.
    #[must_use]
    pub fn activation_origin(&self) -> &[Origin] {
        &self.origin
    }

    /// Mesh-level events since the last drain (also returned by [`Engine::drain_events`]).
    #[must_use]
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    // --- seeding ----------------------------------------------------------------------------

    /// Start a front at `cells` at time `at` (ms). Fronts never travel back in time: `at`
    /// earlier than the last step is processed at the next step with its own time stamp.
    pub fn stimulate_cells(&mut self, cells: &[u32], at: Millis, origin: Origin) {
        for &c in cells {
            self.push_seed(c, at.0, 0, origin);
        }
    }

    fn push_seed(&mut self, cell: u32, at: f64, layer: u8, origin: Origin) {
        let j = cell as usize;
        if j >= self.hot.len() {
            return;
        }
        let h = &mut self.hot[j];
        h.tentative = h.tentative.min(at);
        self.heap.push(Front {
            at,
            cell,
            layer,
            src_probe: self.probe_of[j],
            origin,
        });
    }

    fn seed_set(&mut self, which: SeedSet, at: f64, layer: u8, origin: Origin) {
        let len = self.set(which).len();
        for k in 0..len {
            let c = self.set(which)[k];
            self.push_seed(c, at, layer, origin);
        }
    }

    fn set(&self, which: SeedSet) -> &[u32] {
        let s = &self.seeds;
        match which {
            SeedSet::Sinus => &s.sinus,
            SeedSet::AvInput => &s.av_input,
            SeedSet::Rbb => &s.rbb,
            SeedSet::Laf => &s.laf,
            SeedSet::Lpf => &s.lpf,
            SeedSet::AtrialLead => &s.atrial_lead,
            SeedSet::VentricularLead => &s.ventricular_lead,
            SeedSet::LvLead => &s.lv_lead,
            SeedSet::VentricularEscape => &s.ventricular_escape,
            SeedSet::ApAtrial(i) => &s.ap_atrial[i],
            SeedSet::ApVentricular(i) => &s.ap_ventricular[i],
            SeedSet::Focus(id) => match s.foci.get(id) {
                Some(Some(v)) => v,
                _ => &[],
            },
            SeedSet::Chamber(r) => s.chamber_default(r),
        }
    }

    /// Light the cells of a conduction-system region progressively over `[t0, t1]`.
    fn drive_region(&mut self, region: Region, t0: f64, t1: f64, origin: Origin) {
        let len = self.region_cells[region as usize].len();
        for k in 0..len {
            let c = self.region_cells[region as usize][k];
            let at = t0 + f64::from(self.structure_s[c as usize]) * (t1 - t0);
            self.push_seed(c, at, 0, origin);
        }
    }

    /// Start the wavefronts implied by timing events (see [`SeedMap`] for the mapping). Blocked
    /// events start nothing; `External` stimuli are taken from the actions themselves
    /// ([`EikonalEngine::apply`]) because only those carry the site.
    pub fn ingest(&mut self, events: &[Event]) {
        for e in events {
            if e.kind == EventKind::Blocked {
                continue;
            }
            let t = e.at.0;
            match e.chamber {
                Chamber::Atrium => self.ingest_atrial(e, t),
                Chamber::AvJunction => self.ingest_junction(e, t),
                Chamber::Ventricle => self.ingest_ventricular(e, t),
            }
        }
    }

    fn ingest_atrial(&mut self, e: &Event, t: f64) {
        let set = match (e.kind, e.origin) {
            (EventKind::Stimulus, Origin::External) => return,
            (EventKind::Automatic, Origin::Sinus) => SeedSet::Sinus,
            (EventKind::Stimulus, Origin::Pacer(_)) => SeedSet::AtrialLead,
            (_, Origin::Focus(id)) => self.focus_or_chamber(id, e.region),
            (EventKind::Conducted, Origin::AccessoryPathway) => {
                SeedSet::ApAtrial(usize::from(e.region != Region::LeftAtrium))
            }
            (EventKind::Conducted, _) => SeedSet::AvInput,
            _ => SeedSet::Chamber(e.region),
        };
        self.seed_set(set, t, 0, e.origin);
    }

    fn ingest_junction(&mut self, e: &Event, t: f64) {
        let hp = self.his_purkinje.clone();
        match (e.region, e.kind) {
            (Region::AvNodeFastPathway | Region::AvNodeSlowPathway, EventKind::Conducted) => {
                self.drive_region(e.region, t, t + NODAL_PATHWAY_SPAN_MS, e.origin);
                self.last_av_pathway = Some(t);
            }
            (Region::AvNodeCompact, _) => {
                self.drive_region(Region::AvNodeCompact, t, t, e.origin);
                self.last_av_pathway = None;
            }
            (Region::HisBundle, _) => {
                if let Some(t0) = self.last_av_pathway.take()
                    && t - t0 < NODAL_PAIRING_MS
                {
                    self.drive_region(Region::AvNodeCompact, t0, t, e.origin);
                }
                self.drive_region(Region::HisBundle, t, t + hp.his_ms, e.origin);
            }
            (Region::RightBundleBranch, _) => {
                self.drive_region(e.region, t - hp.rbb.delay_ms, t, e.origin);
                self.drive_region(Region::PurkinjeRight, t, t, e.origin);
                self.seed_set(SeedSet::Rbb, t, LAYER_RV, e.origin);
            }
            (Region::LeftBundleBranch, _) => {
                self.drive_region(e.region, t - hp.lbb.delay_ms, t, e.origin);
            }
            (Region::LeftAnteriorFascicle, _) => {
                self.drive_region(e.region, t - hp.laf.delay_ms, t, e.origin);
                self.drive_region(Region::PurkinjeLeft, t, t, e.origin);
                self.seed_set(SeedSet::Laf, t, LAYER_LV, e.origin);
            }
            (Region::LeftPosteriorFascicle, _) => {
                self.drive_region(e.region, t - hp.lpf.delay_ms, t, e.origin);
                self.drive_region(Region::PurkinjeLeft, t, t, e.origin);
                self.seed_set(SeedSet::Lpf, t, LAYER_LV, e.origin);
            }
            _ => {}
        }
    }

    fn ingest_ventricular(&mut self, e: &Event, t: f64) {
        let set = match (e.kind, e.origin) {
            // Delivered by the His–Purkinje system: the fascicle events already seeded it.
            (EventKind::Conducted, Origin::AccessoryPathway) => {
                SeedSet::ApVentricular(usize::from(e.region == Region::RightVentricle))
            }
            (EventKind::Conducted, _) | (EventKind::Stimulus, Origin::External) => return,
            (EventKind::Stimulus, Origin::Pacer(2)) => SeedSet::LvLead,
            (EventKind::Stimulus, Origin::Pacer(_)) => SeedSet::VentricularLead,
            (_, Origin::Focus(id)) => self.focus_or_chamber(id, e.region),
            (_, Origin::Escape) if e.region != Region::RightVentricle => SeedSet::VentricularEscape,
            _ => SeedSet::Chamber(e.region),
        };
        self.seed_set(set, t, 0, e.origin);
    }

    fn focus_or_chamber(&self, id: u8, region: Region) -> SeedSet {
        if self.set(SeedSet::Focus(id as usize)).is_empty() {
            SeedSet::Chamber(region)
        } else {
            SeedSet::Focus(id as usize)
        }
    }

    // --- actions ----------------------------------------------------------------------------

    /// Apply the spatial part of a scenario action. Returns `Unsupported` for actions that
    /// belong only to the timing engine (rates, drugs, accessory-pathway switches).
    pub fn apply(&mut self, action: &Action, at: Millis) -> Result<(), ActionError> {
        let at = at.0.max(self.now);
        match action {
            Action::Stimulate { site, .. } => {
                let cell = self.site(site)?;
                self.queue_stimulus(at, cell);
                Ok(())
            }
            Action::PacingTrain {
                site,
                s1_cycle_ms,
                s1_count,
                s2_ms,
                s3_ms,
            } => {
                let cell = self.site(site)?;
                let mut t = at;
                for i in 0..*s1_count {
                    t = at + f64::from(i) * s1_cycle_ms;
                    self.queue_stimulus(t, cell);
                }
                if let Some(s2) = s2_ms {
                    t += s2;
                    self.queue_stimulus(t, cell);
                    if let Some(s3) = s3_ms {
                        t += s3;
                        self.queue_stimulus(t, cell);
                    }
                }
                Ok(())
            }
            Action::Ablate { site, radius_mm } => {
                let centre = self.cloud.positions[self.site(site)? as usize];
                let r2 = (*radius_mm as f32).powi(2);
                for (i, p) in self.cloud.positions.iter().enumerate() {
                    if p.distance_squared(centre) <= r2 {
                        self.ablated[i] = true;
                        self.cells.vm[i] = 0.0;
                    }
                }
                self.refresh_excitable();
                Ok(())
            }
            Action::Block { region, blocked } => {
                self.region_blocked[*region as usize] = *blocked;
                self.refresh_excitable();
                Ok(())
            }
            Action::SetRegion { region, params } => {
                self.params.regions.insert(*region, *params);
                self.refresh_parameters();
                Ok(())
            }
            Action::Shock { .. } => {
                // Every excitable cell depolarises at once; fronts in flight die.
                self.heap.clear();
                self.hot.iter_mut().for_each(|h| h.tentative = f64::INFINITY);
                for c in 0..self.hot.len() as u32 {
                    self.push_seed(c, at, 0, Origin::External);
                }
                Ok(())
            }
            Action::SetGlobalSinusRate { .. } => Err(ActionError::Unsupported("SetGlobalSinusRate")),
            Action::AccessoryPathway { .. } => Err(ActionError::Unsupported("AccessoryPathway")),
            Action::Stenosis { .. } => Err(ActionError::Unsupported("Stenosis")),
            Action::Drug { .. } => Err(ActionError::Unsupported("Drug")),
            Action::MoveElectrode { .. } => Err(ActionError::Unsupported("MoveElectrode")),
        }
    }

    fn site(&self, site: &StimSite) -> Result<u32, ActionError> {
        resolve_site(&self.cloud, site).ok_or_else(|| ActionError::UnsupportedSite(format!("{site:?}")))
    }

    fn queue_stimulus(&mut self, at: f64, cell: u32) {
        let pos = self.stimuli.partition_point(|s| s.0 <= at);
        self.stimuli.insert(pos, (at, cell));
    }

    // --- propagation ------------------------------------------------------------------------

    /// Settle one popped arrival: accept it if the cell is excitable and recovered, then relax
    /// its edges.
    fn settle(&mut self, f: Front) {
        let j = f.cell as usize;
        let h = self.hot[j];
        // exact: `tentative` holds the very value this front was queued with
        if h.tentative.to_bits() == f.at.to_bits() {
            self.hot[j].tentative = f64::INFINITY;
        }
        if f.at < h.refractory_until {
            return;
        }
        // APD restitution against the previous activation.
        let p = self.rp[h.region as usize];
        let base = f64::from(self.base_apd[j]) * f64::from(self.apd_scale[j]);
        let apd = if h.repol_at.is_nan() || p.apd_rest_amplitude == 0.0 {
            base
        } else {
            let di = f.at - h.repol_at;
            (base * (1.0 - p.apd_rest_amplitude * (-di / p.apd_rest_tau_ms.max(1e-6)).exp()))
                .max(base * self.eik.min_apd_fraction)
        };
        {
            let h = &mut self.hot[j];
            h.repol_at = f.at + apd;
            h.refractory_until = f.at + apd * p.erp_fraction;
        }
        self.t_act[j] = f.at;
        self.inv_apd[j] = (1.0 / apd) as f32;
        self.origin[j] = f.origin;
        self.cells.activation_ms[j] = f.at as f32;
        self.cells.apd_ms[j] = apd as f32;

        let probe = self.probe_of[j];
        if probe != NO_PROBE && probe != f.src_probe {
            let rearm = self.eik.probe_rearm_ms;
            let pr = &mut self.probes[probe as usize];
            let ap_self = probe != PROBE_AV && f.origin == Origin::AccessoryPathway;
            if !ap_self && f.at - pr.last_fired >= rearm {
                pr.last_fired = f.at;
                let region = pr.region;
                self.events
                    .push(Event::new(Millis(f.at), region, EventKind::Conducted, f.origin));
            }
        }
        self.relax(j, f);
    }

    /// Queue arrivals at the neighbours of `j`, activated by front `f`.
    fn relax(&mut self, j: usize, f: Front) {
        let fast_ms_per_mm = (1.0 / self.eik.purkinje_layer_cv_m_s.max(1e-6)) as f32;
        let own_layer = f.layer != 0 && self.hot[j].layer == f.layer;
        let min_factor = self.eik.min_cv_factor;
        // arrival ≤ at + base / min_factor = at + base + base · slowest
        let slowest = 1.0 / min_factor.max(1e-6) - 1.0;
        let range = self.graph.offsets[j] as usize..self.graph.offsets[j + 1] as usize;
        let edges = &self.edges[range];
        let hot = &mut self.hot;
        let cv_rest = &self.cv_rest;
        let pos = &self.cloud.positions;
        let pj = pos[j];
        for e in edges {
            let k = e.target as usize;
            let h = &hot[k];
            let base = if own_layer && h.layer == f.layer {
                pj.distance(pos[k]) * fast_ms_per_mm
            } else {
                e.tau_ms
            } * h.inv_cv_scale;
            // Cheap rejection before restitution: the slowest possible arrival is still
            // inside the refractory period, or the fastest cannot beat the queued one.
            let fastest = f.at + f64::from(base);
            if fastest >= h.tentative || fastest + f64::from(base) * slowest < h.refractory_until {
                continue;
            }
            // CV restitution of the destination for a front leaving now.
            let mut factor = 1.0;
            if !h.repol_at.is_nan() {
                let r = cv_rest[h.region as usize];
                let di = f.at - h.repol_at;
                if di < r.di_cutoff_ms {
                    factor = (1.0 - r.a * (-di * r.inv_tau).exp()).max(min_factor);
                }
            }
            let arrive = f.at + f64::from(base) / factor;
            if !(arrive >= h.refractory_until && arrive < h.tentative) {
                continue;
            }
            hot[k].tentative = arrive;
            self.heap.push(Front {
                at: arrive,
                cell: k as u32,
                ..f
            });
        }
    }

    /// Settle every arrival up to `until` (ms).
    fn advance_to(&mut self, until: f64) {
        while let Some((at, cell)) = self.stimuli.first().copied()
            && at <= until
        {
            self.stimuli.remove(0);
            self.push_seed(cell, at, 0, Origin::External);
        }
        while self.heap.peek().is_some_and(|f| f.at <= until) {
            let f = self.heap.pop().expect("peeked");
            self.settle(f);
        }
    }

    /// Recompute V_m of every cell at `self.now` from its activation time and APD.
    fn update_vm(&mut self) {
        let now = self.now;
        let t_act = &self.t_act;
        let inv_apd = &self.inv_apd;
        let shape = &self.shape;
        let lut = &self.vm_lut;
        let kernel = |offset: usize, vm: &mut [f32]| {
            let r = offset..offset + vm.len();
            let cells = t_act[r.clone()].iter().zip(&inv_apd[r.clone()]).zip(&shape[r]);
            for (v, ((&t, &inv), &s)) in vm.iter_mut().zip(cells) {
                // NaN (never activated / ablated) fails the range test and gives rest.
                let phase = (now - t) as f32 * inv;
                *v = if (0.0..1.0).contains(&phase) {
                    lut[s as usize][((phase * VM_LUT as f32) as usize).min(VM_LUT - 1)]
                } else {
                    0.0
                };
            }
        };
        #[cfg(feature = "parallel")]
        {
            use rayon::prelude::*;
            self.cells
                .vm
                .par_chunks_mut(VM_CHUNK)
                .enumerate()
                .for_each(|(c, vm)| kernel(c * VM_CHUNK, vm));
        }
        #[cfg(not(feature = "parallel"))]
        for (c, vm) in self.cells.vm.chunks_mut(VM_CHUNK).enumerate() {
            kernel(c * VM_CHUNK, vm);
        }
    }
}

impl Engine for EikonalEngine {
    fn step(&mut self, dt: Millis) {
        self.now += dt.0;
        self.advance_to(self.now);
        self.update_vm();
    }

    fn now(&self) -> Millis {
        Millis(self.now)
    }

    fn cells(&self) -> &CellState {
        &self.cells
    }

    fn drain_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    fn reset(&mut self) {
        let n = self.t_act.len();
        self.now = 0.0;
        for h in &mut self.hot {
            *h = Hot {
                region: h.region,
                layer: h.layer,
                inv_cv_scale: h.inv_cv_scale,
                ..Hot::REST
            };
        }
        self.t_act.iter_mut().for_each(|t| *t = f64::NAN);
        self.inv_apd.iter_mut().for_each(|a| *a = 0.0);
        self.origin.iter_mut().for_each(|o| *o = Origin::Sinus);
        self.heap.clear();
        self.stimuli.clear();
        self.last_av_pathway = None;
        self.ablated.clone_from(&self.ablated_base);
        self.region_blocked = [false; REGIONS];
        self.refresh_excitable();
        for p in &mut self.probes {
            p.last_fired = f64::NEG_INFINITY;
        }
        self.cells = CellState::new(n);
        self.events.clear();
    }
}

// --- static per-cell geometry -----------------------------------------------------------------

/// 0 at the `lv_apex` landmark, 1 at the `his` landmark (top of the septum), by projection on
/// that axis. Without both landmarks every cell gets 0.5 (no apicobasal structure).
fn apex_base_coordinate(cloud: &CellCloud) -> Vec<f32> {
    let lm = |n: &str| cloud.landmarks.get(n).map(|&i| cloud.positions[i as usize]);
    match (lm("lv_apex"), lm("his").or_else(|| lm("av_node"))) {
        (Some(apex), Some(base)) if apex.distance(base) > 1.0 => {
            let axis = base - apex;
            let l2 = axis.length_squared();
            cloud
                .positions
                .iter()
                .map(|p| (*p - apex).dot(axis) / l2)
                .collect()
        }
        _ => vec![0.5; cloud.len()],
    }
}

/// Fast endocardial layer membership: LV (and LV septal surface) → [`LAYER_LV`], RV (and RV
/// septal surface) → [`LAYER_RV`]; none above the base limit.
fn fast_layer(cloud: &CellCloud, kinds: &[CellKind], apex_base: &[f32], eik: &EikonalParams) -> Vec<u8> {
    let d = eik.purkinje_layer_depth;
    (0..cloud.len())
        .map(|i| {
            if d <= 0.0 || kinds[i] != CellKind::Ventricular || apex_base[i] > eik.purkinje_layer_base_limit {
                return 0;
            }
            let depth = cloud.depth[i];
            match cloud.regions[i] {
                Region::LeftVentricle | Region::VentricularSeptum if depth <= d => LAYER_LV,
                Region::VentricularSeptum if depth >= 1.0 - d => LAYER_RV,
                Region::RightVentricle if depth <= d => LAYER_RV,
                _ => 0,
            }
        })
        .collect()
}

/// Upstream structure of each conduction-system region (where its proximal end attaches).
fn parent_regions(region: Region) -> &'static [Region] {
    use Region as R;
    match region {
        R::AvNodeCompact => &[R::AvNodeFastPathway, R::AvNodeSlowPathway],
        R::HisBundle => &[R::AvNodeCompact],
        R::RightBundleBranch | R::LeftBundleBranch => &[R::HisBundle],
        R::LeftAnteriorFascicle | R::LeftPosteriorFascicle => &[R::LeftBundleBranch],
        R::PurkinjeLeft => &[R::LeftAnteriorFascicle, R::LeftPosteriorFascicle],
        R::PurkinjeRight => &[R::RightBundleBranch],
        _ => &[],
    }
}

/// Normalised distance of each conduction-system cell from the proximal end of its structure
/// (the cell closest to the parent structure); 0 for all other cells.
fn structure_coordinate(cloud: &CellCloud) -> Vec<f32> {
    let mut s = vec![0.0f32; cloud.len()];
    for region in Region::ALL {
        let parents = parent_regions(region);
        if parents.is_empty() {
            continue;
        }
        let own: Vec<usize> = (0..cloud.len()).filter(|&i| cloud.regions[i] == region).collect();
        let up: Vec<Vec3> = (0..cloud.len())
            .filter(|&i| parents.contains(&cloud.regions[i]))
            .map(|i| cloud.positions[i])
            .collect();
        if own.is_empty() || up.is_empty() {
            continue;
        }
        let gap = |i: usize| {
            up.iter()
                .map(|q| q.distance_squared(cloud.positions[i]))
                .fold(f32::INFINITY, f32::min)
        };
        let proximal = own
            .iter()
            .copied()
            .min_by(|a, b| gap(*a).total_cmp(&gap(*b)).then(a.cmp(b)))
            .expect("non-empty");
        let p0 = cloud.positions[proximal];
        let max = own
            .iter()
            .map(|&i| cloud.positions[i].distance(p0))
            .fold(0.0f32, f32::max)
            .max(1e-6);
        for &i in &own {
            s[i] = cloud.positions[i].distance(p0) / max;
        }
    }
    s
}
