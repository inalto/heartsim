//! Shared driver of the acceptance tests: the default pack, the coupled Tier-1 engine (timing
//! engine + eikonal mesh, as the app runs them), the motion history fed every step, and the model.
#![allow(dead_code)]

use heartsim_core::{Action, Millis, Parameters};
use heartsim_ecg::Tier0Scenario;
use heartsim_ecg::pcg::PcgParams;
use heartsim_echo::{MotionFrame, MotionHistory, MotionInput, MotionModel, MotionParams};
use heartsim_geometry::AnatomyPack;
use heartsim_sim::{EikonalEngine, EikonalParams, Engine, Tier1Engine, TimingParams};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

pub fn pack() -> &'static AnatomyPack {
    static PACK: OnceLock<AnatomyPack> = OnceLock::new();
    PACK.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/anatomy/default");
        AnatomyPack::load(&dir).expect("default pack")
    })
}

pub fn model() -> MotionModel {
    static MODEL: OnceLock<MotionModel> = OnceLock::new();
    MODEL
        .get_or_init(|| MotionModel::new(pack(), MotionParams::default()).expect("model"))
        .clone()
}

pub fn scenario(id: &str) -> Tier0Scenario {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../assets/scenarios/{id}.ron"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    Tier0Scenario::from_ron(&text).unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// The timing engine of `scenario` coupled to the eikonal engine on the default pack, with a
/// motion history fed every 1 ms step.
pub struct Sim {
    pub engine: Tier1Engine,
    pub history: MotionHistory,
    pub timeline: Vec<(f64, Action)>,
    next: usize,
}

impl Sim {
    pub fn new(timing: TimingParams, timeline: Vec<(f64, Action)>, respiration_rate_bpm: f64) -> Self {
        let p = pack();
        let mut eikonal = EikonalEngine::with_seed_sets(
            Arc::new(p.cells.clone()),
            &Parameters::default(),
            EikonalParams::default(),
            &p.landmarks.sets,
        );
        eikonal.set_aha_segments(&p.aha_segments);
        let engine = Tier1Engine::new(timing, eikonal);
        let history = MotionHistory::new(PcgParams::default(), respiration_rate_bpm, p.cells.len());
        let mut timeline = timeline;
        timeline.sort_by(|a, b| a.0.total_cmp(&b.0));
        Self {
            engine,
            history,
            timeline,
            next: 0,
        }
    }

    pub fn from_scenario(id: &str) -> Self {
        let s = scenario(id);
        let timeline = s
            .scenario
            .timeline
            .iter()
            .map(|a| (a.at.0, a.action.clone()))
            .collect();
        let rate = s.timing.sinus.respiration_rate_bpm;
        let mut sim = Self::new(s.timing.clone(), timeline, rate);
        sim.history = sim.history.with_auscultation(&s.scenario.auscultation);
        sim
    }

    pub fn sinus(bpm: f64) -> Self {
        let mut t = TimingParams::default();
        t.sinus.rate_bpm = bpm;
        t.sinus.respiration_depth = 0.0;
        let rate = t.sinus.respiration_rate_bpm;
        Self::new(t, Vec::new(), rate)
    }

    pub fn now_ms(&self) -> f64 {
        self.engine.now().0
    }

    /// Advance to `t_ms` in 1 ms steps.
    pub fn run_to(&mut self, t_ms: f64) {
        while self.engine.now().0 < t_ms - 1e-9 {
            while let Some((at, a)) = self.timeline.get(self.next)
                && *at <= self.engine.now().0
            {
                let _ = self.engine.apply(a, Millis(*at));
                self.next += 1;
            }
            self.engine.step(Millis(1.0));
            let evs = self.engine.drain_events();
            self.history.set_respiration(self.engine.timing().respiration());
            self.history.on_events(&evs);
            self.history.record_cells(self.engine.eikonal().cells());
        }
    }

    pub fn resp_phase(&self, t_ms: f64) -> f64 {
        self.engine.timing().respiratory_phase(Millis(t_ms))
    }
}

pub fn eval(model: &mut MotionModel, sim: &Sim, t_ms: f64, frame: &mut MotionFrame) {
    let input = MotionInput {
        history: &sim.history,
        t_ms,
        resp_phase: sim.resp_phase(t_ms),
        enabled: true,
        exaggeration: 1.0,
    };
    model.evaluate(&input, frame);
}

// --- measurements on the deformed mesh ------------------------------------------------------------

use glam::Vec3;
use heartsim_echo::MotionChamber;
use heartsim_geometry::{HashGrid, Vertex};

/// Evaluate without the respiratory shift (expiration), for the measurements about the rest axes.
pub fn eval_still(model: &mut MotionModel, sim: &Sim, t_ms: f64, frame: &mut MotionFrame) {
    let input = MotionInput {
        history: &sim.history,
        t_ms,
        resp_phase: 0.75,
        enabled: true,
        exaggeration: 1.0,
    };
    model.evaluate(&input, frame);
}

pub fn pos(v: &Vertex) -> Vec3 {
    Vec3::from(v.position)
}

/// LV cavity volume of a frame, mL.
pub fn lv_volume(model: &MotionModel, frame: &MotionFrame) -> f64 {
    model.lv_cavity_volume_ml(&frame.part("lv").expect("lv").vertices)
}

/// Rest triangles whose smallest altitude is below this fraction of their longest edge are
/// near-degenerate slivers (the decimated pack meshes have some): any non-rigid motion can flip
/// them, so they are counted apart.
pub const SLIVER_QUALITY: f32 = 0.2;

/// Mesh faults of a frame over all parts: inverted proper triangles (deformed normal against the
/// rest normal), non-finite vertices, and inverted slivers.
pub fn mesh_faults(model: &MotionModel, frame: &MotionFrame) -> (usize, usize, usize) {
    let (mut inverted, mut bad, mut slivers) = (0, 0, 0);
    for part in &frame.parts {
        let rest = model.part_rest(&part.name).unwrap();
        let idx = model.part_indices(&part.name).unwrap();
        for v in &part.vertices {
            if !v.position.iter().chain(&v.normal).all(|x| x.is_finite()) {
                bad += 1;
            }
        }
        for t in idx.as_chunks::<3>().0 {
            let [a, b, c] = [t[0] as usize, t[1] as usize, t[2] as usize];
            let (pa, pb, pc) = (pos(&rest[a]), pos(&rest[b]), pos(&rest[c]));
            let n0 = (pb - pa).cross(pc - pa);
            let longest = (pb - pa).length().max((pc - pb).length()).max((pa - pc).length());
            let v = &part.vertices;
            let n1 = (pos(&v[b]) - pos(&v[a])).cross(pos(&v[c]) - pos(&v[a]));
            if n1.dot(n0) <= 0.0 {
                if n0.length() < SLIVER_QUALITY * longest * longest {
                    slivers += 1;
                } else {
                    inverted += 1;
                }
            }
        }
    }
    (inverted, bad, slivers)
}

/// The proper (non-sliver) triangles of a frame that are inverted, as `(part, triangle index)`.
pub fn inverted_triangles(model: &MotionModel, frame: &MotionFrame) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    for part in &frame.parts {
        let rest = model.part_rest(&part.name).unwrap();
        let idx = model.part_indices(&part.name).unwrap();
        for (k, t) in idx.as_chunks::<3>().0.iter().enumerate() {
            let [a, b, c] = [t[0] as usize, t[1] as usize, t[2] as usize];
            let (pa, pb, pc) = (pos(&rest[a]), pos(&rest[b]), pos(&rest[c]));
            let n0 = (pb - pa).cross(pc - pa);
            let longest = (pb - pa).length().max((pc - pb).length()).max((pa - pc).length());
            let v = &part.vertices;
            let n1 = (pos(&v[b]) - pos(&v[a])).cross(pos(&v[c]) - pos(&v[a]));
            if n1.dot(n0) <= 0.0 && n0.length() >= SLIVER_QUALITY * longest * longest {
                out.push((part.name.clone(), k));
            }
        }
    }
    out
}

/// Number of near-degenerate rest triangles of each part.
pub fn sliver_count(model: &MotionModel) -> Vec<(String, usize, usize)> {
    model
        .part_names()
        .map(|name| {
            let rest = model.part_rest(name).unwrap();
            let idx = model.part_indices(name).unwrap();
            let n = idx
                .as_chunks::<3>()
                .0
                .iter()
                .filter(|t| {
                    let (pa, pb, pc) = (
                        pos(&rest[t[0] as usize]),
                        pos(&rest[t[1] as usize]),
                        pos(&rest[t[2] as usize]),
                    );
                    let longest = (pb - pa).length().max((pc - pb).length()).max((pa - pc).length());
                    (pb - pa).cross(pc - pa).length() < SLIVER_QUALITY * longest * longest
                })
                .count();
            (name.to_string(), n, idx.len() / 3)
        })
        .collect()
}

/// Fixed data about the LV mesh for the measurements.
pub struct LvProbe {
    pub rest: Vec<Vec3>,
    pub endo: Vec<bool>,
    pub segment: Vec<u8>,
    pub origin: Vec3,
    pub axis: Vec3,
    pub length: f32,
}

impl LvProbe {
    pub fn new(model: &MotionModel) -> Self {
        let ax = *model.frame().axis(MotionChamber::LeftVentricle);
        Self {
            rest: model.part_rest("lv").unwrap().iter().map(pos).collect(),
            endo: model.lv_endocardial_vertices(),
            segment: model.vertex_segments("lv"),
            origin: ax.origin_mm,
            axis: ax.axis,
            length: ax.length_mm,
        }
    }

    pub fn height(&self, v: usize) -> f32 {
        (self.rest[v] - self.origin).dot(self.axis) / self.length
    }

    fn perp(&self, p: Vec3) -> Vec3 {
        let d = p - self.origin;
        d - self.axis * d.dot(self.axis)
    }

    /// Mean rotation (degrees, right-handed about the apex-to-base axis) of the LV vertices whose
    /// rest height lies in `band`.
    pub fn rotation_deg(&self, frame: &MotionFrame, band: (f32, f32)) -> f64 {
        let lv = &frame.part("lv").unwrap().vertices;
        let (mut s, mut n) = (0.0f64, 0usize);
        for (v, p) in lv.iter().enumerate() {
            let h = self.height(v);
            if h < band.0 || h > band.1 {
                continue;
            }
            let a = self.perp(self.rest[v]);
            let b = self.perp(pos(p));
            if a.length() < 5.0 {
                continue;
            }
            let ang = f64::from(self.axis.dot(a.cross(b))).atan2(f64::from(a.dot(b)));
            s += ang.to_degrees();
            n += 1;
        }
        s / n.max(1) as f64
    }

    /// Mean inward (toward the LV axis) displacement of the endocardial vertices of `segments`, mm.
    pub fn inward_mm(&self, frame: &MotionFrame, segments: &[u8]) -> f64 {
        let lv = &frame.part("lv").unwrap().vertices;
        let (mut s, mut n) = (0.0f64, 0usize);
        for (v, p) in lv.iter().enumerate() {
            if !self.endo[v] || !segments.contains(&self.segment[v]) {
                continue;
            }
            let r = self.perp(self.rest[v]).normalize_or_zero();
            s += f64::from(-(pos(p) - self.rest[v]).dot(r));
            n += 1;
        }
        s / n.max(1) as f64
    }

    /// Mean displacement magnitude of the endocardial vertices of `segments`, mm.
    pub fn excursion_mm(&self, frame: &MotionFrame, segments: &[u8]) -> f64 {
        let lv = &frame.part("lv").unwrap().vertices;
        let (mut s, mut n) = (0.0f64, 0usize);
        for (v, p) in lv.iter().enumerate() {
            if self.endo[v] && segments.contains(&self.segment[v]) {
                s += f64::from((pos(p) - self.rest[v]).length());
                n += 1;
            }
        }
        s / n.max(1) as f64
    }

    /// Mean wall thickness of each AHA segment 1–17 (index = segment), mm: the mean, over the
    /// segment's vertices, of the distance from each endocardial vertex to the nearest
    /// non-endocardial one and from each non-endocardial vertex to the nearest endocardial one.
    pub fn thickness_mm(&self, frame: &MotionFrame) -> [f64; 18] {
        let lv = &frame.part("lv").unwrap().vertices;
        let side = |endo: bool| -> Vec<Vec3> {
            lv.iter()
                .zip(&self.endo)
                .filter(|(_, e)| **e == endo)
                .map(|(p, _)| pos(p))
                .collect()
        };
        let (endo_pts, epi_pts) = (side(true), side(false));
        let (g_endo, g_epi) = (HashGrid::new(&endo_pts, 3.0), HashGrid::new(&epi_pts, 3.0));
        let mut sum = [0.0f64; 18];
        let mut n = [0usize; 18];
        for (v, p) in lv.iter().enumerate() {
            let s = self.segment[v] as usize;
            if s == 0 || s > 17 {
                continue;
            }
            let other = if self.endo[v] { &g_epi } else { &g_endo };
            if let Some((d, _)) = other.nearest(pos(p)) {
                sum[s] += f64::from(d);
                n[s] += 1;
            }
        }
        let mut out = [0.0; 18];
        for s in 1..18 {
            out[s] = sum[s] / n[s].max(1) as f64;
        }
        out
    }
}

/// The beat (index into the history) whose QRS is the first at or after `t_ms`, with its M1, A2,
/// and the next beat's QRS.
pub struct BeatWindow {
    pub qrs: f64,
    pub m1: f64,
    pub a2: Option<f64>,
    /// P2 onset: the right ventricle's end-systole (TAPSE is read there).
    pub p2: Option<f64>,
    pub next_qrs: f64,
    /// Aortic opening as the model places it: M1 + IVCT + the part of a late-LV delay M1 does not
    /// carry.
    pub ao: f64,
}

pub fn beat_after(sim: &Sim, t_ms: f64) -> BeatWindow {
    let beats = sim.history.beats();
    let k = beats
        .iter()
        .position(|(b, _)| b.qrs_onset.0 >= t_ms)
        .expect("a beat");
    let (b, c) = &beats[k];
    BeatWindow {
        qrs: b.qrs_onset.0,
        m1: c.m1.unwrap().onset.0,
        a2: c.a2.map(|a| a.onset.0),
        p2: c.p2.map(|a| a.onset.0),
        next_qrs: beats
            .get(k + 1)
            .map_or(b.qrs_onset.0 + 1000.0, |n| n.0.qrs_onset.0),
        ao: c.m1.unwrap().onset.0
            + MotionParams::default().lv_ivct_ms
            + (1.0 - PcgParams::default().m1_lv_delay_fraction) * b.ventricle_delays_ms().0,
    }
}
