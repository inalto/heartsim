//! The heart is one continuous muscle: wherever two parts touch at rest they move together.
//! Over a whole sinus beat, an LBBB beat and an anterior STEMI beat (with its injury), sampled
//! every 10 ms:
//! * vertex pairs of different parts closer than [`CONTACT_MM`] at rest keep the same
//!   displacement within [`MAX_SLIP_MM`] between the ventricles and the discs, within
//!   [`MAX_THIN_SLIP_MM`] where an atrium or a vessel is one of the pair;
//! * every valve-disc vertex on the annulus (within [`RIM_MM`] of the ventricles at rest) keeps
//!   its distance to the ventricles within [`MAX_DISC_GAP_GROWTH_MM`] of its rest distance (the
//!   inner vertices move with an affine fit of the annulus; their nearest ventricle vertex is a
//!   centimetre away and changes as the walls move, so their distance measures nothing);
//! * the root of each great vessel stays on what it rests on: the worst growth of a root vertex's
//!   distance to the myocardium and the valve discs within [`MAX_ROOT_GAP_GROWTH_MM`] (its mean
//!   over the root within [`MAX_ROOT_MEAN_GAP_GROWTH_MM`]);
//!
//! and TAPSE is the same every beat of a steady rhythm with breathing.

mod common;
use common::*;
use glam::Vec3;
use heartsim_echo::{MotionFrame, MotionModel};
use heartsim_geometry::MeshKind;
use std::collections::HashMap;

/// Two vertices of different parts this close at rest are in contact, mm.
const CONTACT_MM: f32 = 1.5;
/// Largest displacement difference of two vertices in contact, mm, between the ventricles and the
/// valve discs (the RV and the discs take their parents' displacement exactly on the contact; not
/// 0, because a vertex takes the interpolated displacement of its parent's vertices nearby, which
/// differs from a contact partner's up to 1.5 mm away by the gradient over that distance).
const MAX_SLIP_MM: f32 = 1.25;
/// The same for any pair with an atrium or a vessel, mm: those thin walls take only part of the
/// exact correction (`THIN_EXACT_SHARE` in the model; the full one folds their sub-millimetre
/// triangles) and the rest smoothed over a few millimetres. Before the parts were attached the
/// worst slips were 8–10 mm (LA–LV, RV–aorta, aorta–pulmonary artery).
const MAX_THIN_SLIP_MM: f32 = 4.0;
/// Disc vertices this close to a ventricle vertex at rest lie on the annulus, mm.
const RIM_MM: f32 = 3.0;
/// Largest growth of a disc rim vertex's distance to the nearest ventricle vertex, mm.
const MAX_DISC_GAP_GROWTH_MM: f32 = 2.0;
/// Root vertices of a vessel: within this distance of a myocardium or valve part at rest, mm.
const ROOT_MM: f32 = 3.0;
/// Largest growth of a root vertex's distance to the myocardium and the valve discs, mm (worst
/// vertex), and of its mean over the root.
const MAX_ROOT_GAP_GROWTH_MM: f32 = 4.0;
const MAX_ROOT_MEAN_GAP_GROWTH_MM: f32 = 0.5;

/// One contact: `(part a, vertex a, part b, vertex b)` with a < b.
type Contact = (usize, u32, usize, u32);

fn rest_positions(m: &MotionModel) -> Vec<Vec<Vec3>> {
    m.part_names()
        .map(|n| m.part_rest(n).unwrap().iter().map(pos).collect())
        .collect()
}

fn frame_positions(f: &MotionFrame) -> Vec<Vec<Vec3>> {
    f.parts
        .iter()
        .map(|p| p.vertices.iter().map(pos).collect())
        .collect()
}

fn cell_of(p: Vec3, h: f32) -> (i32, i32, i32) {
    let q = (p / h).floor();
    (q.x as i32, q.y as i32, q.z as i32)
}

/// A uniform grid of the vertices of some parts, for radius queries.
struct Bins {
    h: f32,
    cells: HashMap<(i32, i32, i32), Vec<(usize, u32)>>,
}

impl Bins {
    fn new(pts: &[Vec<Vec3>], parts: &[usize], h: f32) -> Self {
        let mut cells: HashMap<(i32, i32, i32), Vec<(usize, u32)>> = HashMap::new();
        for &k in parts {
            for (v, p) in pts[k].iter().enumerate() {
                cells.entry(cell_of(*p, h)).or_default().push((k, v as u32));
            }
        }
        Self { h, cells }
    }

    /// The nearest binned vertex within the bin size of `p`.
    fn nearest(&self, pts: &[Vec<Vec3>], p: Vec3) -> Option<(usize, u32, f32)> {
        let c = cell_of(p, self.h);
        let mut best: Option<(usize, u32, f32)> = None;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    for &(k, v) in self
                        .cells
                        .get(&(c.0 + dx, c.1 + dy, c.2 + dz))
                        .into_iter()
                        .flatten()
                    {
                        let d = pts[k][v as usize].distance(p);
                        if d <= self.h && best.is_none_or(|b| d < b.2) {
                            best = Some((k, v, d));
                        }
                    }
                }
            }
        }
        best
    }
}

/// Every vertex paired with its nearest vertex of each later part within `radius`.
fn contacts(rest: &[Vec<Vec3>], radius: f32) -> Vec<Contact> {
    let all: Vec<usize> = (0..rest.len()).collect();
    let bins = Bins::new(rest, &all, radius);
    let mut out = Vec::new();
    for (a, ps) in rest.iter().enumerate() {
        for (va, p) in ps.iter().enumerate() {
            let c = cell_of(*p, radius);
            let mut best: Vec<(usize, u32, f32)> = Vec::new();
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        for &(b, vb) in bins
                            .cells
                            .get(&(c.0 + dx, c.1 + dy, c.2 + dz))
                            .into_iter()
                            .flatten()
                        {
                            let d = rest[b][vb as usize].distance(*p);
                            if b <= a || d > radius {
                                continue;
                            }
                            match best.iter_mut().find(|x| x.0 == b) {
                                Some(x) if (d, vb) < (x.2, x.1) => *x = (b, vb, d),
                                Some(_) => {}
                                None => best.push((b, vb, d)),
                            }
                        }
                    }
                }
            }
            out.extend(best.iter().map(|&(b, vb, _)| (a, va as u32, b, vb)));
        }
    }
    out
}

/// What one scenario's beat gives.
struct Continuity {
    names: Vec<String>,
    /// Worst slip per part pair, mm, and the number of contacts.
    slip: Vec<((usize, usize), f32, usize)>,
    /// Worst growth of each disc's rim vertices' distance to the ventricles, mm (valve, rim
    /// vertices, growth).
    disc: Vec<(String, usize, f32)>,
    /// Per vessel: root vertices, worst growth of a root vertex's distance to the myocardium and
    /// discs, mm, and the worst growth of the mean over the root.
    root: Vec<(usize, usize, f32, f32)>,
}

impl Continuity {
    fn report(&self, id: &str) {
        eprintln!("== {id}");
        for ((a, b), s, n) in &self.slip {
            eprintln!(
                "  {} – {}: {n} contacts, worst slip {s:.2} mm",
                self.names[*a], self.names[*b]
            );
        }
        for (v, n, g) in &self.disc {
            eprintln!("  {v} disc: {n} rim vertices, worst gap growth {g:.2} mm");
        }
        for (k, n, worst, mean) in &self.root {
            eprintln!(
                "  {} root: {n} vertices, worst gap growth {worst:.2} mm (mean {mean:.2} mm)",
                self.names[*k]
            );
        }
    }

    fn check(&self, id: &str) {
        self.report(id);
        let stiff = |k: usize| matches!(self.names[k].as_str(), "lv" | "rv" | "valve_planes");
        for ((a, b), s, _) in &self.slip {
            let bound = if stiff(*a) && stiff(*b) {
                MAX_SLIP_MM
            } else {
                MAX_THIN_SLIP_MM
            };
            assert!(
                *s <= bound,
                "{id}: {} – {} slip {s:.2} mm",
                self.names[*a],
                self.names[*b]
            );
        }
        for (v, _, g) in &self.disc {
            assert!(*g <= MAX_DISC_GAP_GROWTH_MM, "{id}: {v} disc gap +{g:.2} mm");
        }
        for (k, _, worst, mean) in &self.root {
            assert!(
                *worst <= MAX_ROOT_GAP_GROWTH_MM && *mean <= MAX_ROOT_MEAN_GAP_GROWTH_MM,
                "{id}: {} root gap +{worst:.2} mm (mean +{mean:.2})",
                self.names[*k]
            );
        }
    }
}

/// One whole beat of scenario `id` (the first QRS after 2500 ms to the next), every 10 ms, drawn
/// 200 ms behind the engine as the app does; with the injury of its lesions if `injury`.
fn beat(id: &str, injury: bool) -> Continuity {
    let mut m = model();
    let mut sim = Sim::from_scenario(id);
    sim.run_to(5000.0);
    if injury {
        let inj = heartsim_echo::mechanical_injury(sim.engine.eikonal(), &pack().aha_segments);
        m.set_injury(&inj);
    }
    let w = beat_after(&sim, 2500.0);
    let names: Vec<String> = m.part_names().map(str::to_string).collect();
    let kind = |k: usize| m.part_kind(&names[k]).unwrap();
    let rest = rest_positions(&m);
    let cs = contacts(&rest, CONTACT_MM);
    let ventricles: Vec<usize> = (0..names.len())
        .filter(|&k| names[k] == "lv" || names[k] == "rv")
        .collect();
    let solids: Vec<usize> = (0..names.len())
        .filter(|&k| matches!(kind(k), MeshKind::Myocardium | MeshKind::Valve))
        .collect();
    let vp = (0..names.len())
        .find(|&k| kind(k) == MeshKind::Valve)
        .expect("valve part");
    let discs: Vec<(String, Vec<u32>)> = m
        .valve_discs()
        .iter()
        .map(|d| (format!("{:?}", d.valve), d.vertices.clone()))
        .collect();
    // rest distances: disc vertices to the ventricles, root vertices to the solids
    let disc_reach = 6.0;
    let vent_bins = Bins::new(&rest, &ventricles, disc_reach);
    let disc_rest: Vec<Vec<Option<f32>>> = discs
        .iter()
        .map(|(_, vs)| {
            vs.iter()
                .map(|&v| {
                    vent_bins
                        .nearest(&rest, rest[vp][v as usize])
                        .map(|x| x.2)
                        .filter(|&d| d <= RIM_MM)
                })
                .collect()
        })
        .collect();
    let root_reach = 6.0;
    let solid_bins = Bins::new(&rest, &solids, root_reach);
    let roots: Vec<(usize, Vec<(u32, f32)>)> = (0..names.len())
        .filter(|&k| kind(k) == MeshKind::Vessel)
        .map(|k| {
            let r = rest[k]
                .iter()
                .enumerate()
                .filter_map(|(v, p)| {
                    solid_bins
                        .nearest(&rest, *p)
                        .filter(|x| x.2 <= ROOT_MM)
                        .map(|x| (v as u32, x.2))
                })
                .collect();
            (k, r)
        })
        .collect();

    let mut slip: HashMap<(usize, usize), f32> = HashMap::new();
    let mut disc = vec![0.0f32; discs.len()];
    let mut root = vec![(0.0f32, 0.0f32); roots.len()];
    let mut f = m.new_frame();
    let mut t = w.qrs;
    while t < w.next_qrs {
        sim.run_to(t + 200.0);
        eval(&mut m, &sim, t, &mut f);
        let now = frame_positions(&f);
        for &(a, va, b, vb) in &cs {
            let da = now[a][va as usize] - rest[a][va as usize];
            let db = now[b][vb as usize] - rest[b][vb as usize];
            let s = slip.entry((a, b)).or_insert(0.0);
            *s = s.max((da - db).length());
        }
        let vent_now = Bins::new(&now, &ventricles, disc_reach);
        for (k, (_, vs)) in discs.iter().enumerate() {
            for (&v, d0) in vs.iter().zip(&disc_rest[k]) {
                let Some(d0) = d0 else { continue };
                let d1 = vent_now
                    .nearest(&now, now[vp][v as usize])
                    .map_or(disc_reach, |x| x.2);
                disc[k] = disc[k].max(d1 - d0);
            }
        }
        let solid_now = Bins::new(&now, &solids, root_reach);
        for (k, (vk, rs)) in roots.iter().enumerate() {
            let (mut sum, mut worst) = (0.0f32, 0.0f32);
            for &(v, d0) in rs {
                let d1 = solid_now
                    .nearest(&now, now[*vk][v as usize])
                    .map_or(root_reach, |x| x.2);
                sum += d1 - d0;
                worst = worst.max(d1 - d0);
            }
            let mean = sum / rs.len().max(1) as f32;
            root[k] = (root[k].0.max(worst), root[k].1.max(mean));
        }
        t += 10.0;
    }
    let mut slip: Vec<((usize, usize), f32, usize)> = slip
        .into_iter()
        .map(|(p, s)| (p, s, cs.iter().filter(|c| (c.0, c.2) == p).count()))
        .collect();
    slip.sort_by_key(|x| x.0);
    Continuity {
        names,
        slip,
        disc: discs
            .iter()
            .zip(&disc_rest)
            .zip(disc)
            .map(|(((n, _), r), g)| (n.clone(), r.iter().flatten().count(), g))
            .collect(),
        root: roots
            .iter()
            .zip(&root)
            .map(|((k, rs), (worst, mean))| (*k, rs.len(), *worst, *mean))
            .collect(),
    }
}

#[test]
fn a_sinus_beat_moves_as_one_heart() {
    beat("sinus_normal", false).check("sinus_normal");
}

#[test]
fn an_lbbb_beat_moves_as_one_heart() {
    beat("lbbb", false).check("lbbb");
}

#[test]
fn an_anterior_stemi_beat_moves_as_one_heart() {
    beat("stemi_anterior", true).check("stemi_anterior");
}

/// TAPSE is read at the right ventricle's own end-systole (pulmonic closure), so the respiratory
/// splitting of S2 does not change it from beat to beat (at A2 the RV is still ejecting by a
/// breath-dependent amount).
#[test]
fn tapse_is_the_same_every_beat_of_a_steady_rhythm() {
    let mut m = model();
    let mut sim = Sim::from_scenario("sinus_normal");
    let mut f = m.new_frame();
    let mut per_beat: Vec<(f64, f64)> = Vec::new();
    let mut t = 3000.0;
    while t < 14000.0 {
        sim.run_to(t + 200.0);
        eval(&mut m, &sim, t, &mut f);
        if let (Some(a), Some(b)) = (f.readout.tapse_mm, f.readout.mapse_mm)
            && per_beat.last().is_none_or(|x| x.0.to_bits() != a.to_bits())
        {
            per_beat.push((a, b));
        }
        t += 20.0;
    }
    let splits: Vec<f64> = sim
        .history
        .beats()
        .iter()
        .filter_map(|(_, c)| c.split_ms())
        .collect();
    let (lo, hi) = per_beat
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), x| {
            (lo.min(x.0), hi.max(x.0))
        });
    eprintln!(
        "TAPSE / MAPSE per change {per_beat:.2?}; S2 split {:.0}…{:.0} ms",
        splits.iter().copied().fold(f64::INFINITY, f64::min),
        splits.iter().copied().fold(f64::NEG_INFINITY, f64::max)
    );
    assert!(hi - lo < 0.2, "TAPSE {lo:.2}…{hi:.2} mm");
}
