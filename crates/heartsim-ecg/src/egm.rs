//! Intracardiac electrograms (brief B22, F-16): unipolar and bipolar local potentials at catheter
//! electrodes, and the deflections read from them.
//!
//! **Physics** (research §3.1). The same volume dipole as the 12-lead forward model,
//! `φ_e = −K Σ_i vol·∇V_i·(r_e − x_i)/|r_e − x_i|³`, restricted to the cells within
//! [`EgmOptions::radius_mm`] of the electrode. A front coming towards the electrode gives a positive
//! potential, one going away a negative one, so a unipolar electrogram is the familiar RS with its
//! steepest slope when the front passes under the electrode. A bipole is the difference of two such
//! potentials millimetres apart, which cancels what both see alike (the far field) and leaves the
//! local activation. A row costs a few hundred cells, so all catheters together stay a small
//! fraction of a Tier-1 step.
//!
//! **Sources are kept apart** ([`EgmSource`]). Every electrode's potential is summed per electrical
//! compartment. Added together they are the trace on the screen; apart, they label its A, H and V
//! deflections. On a noise-free model this is an exact stand-in for the eye of the electrophysiologist,
//! who tells them apart by timing and shape. AV-nodal cells contribute to the trace and to no label:
//! the node's slow upstroke is the "silent" stretch of the AH interval.
//!
//! **Determinism.** No clock, no threads, fixed summation order; a catheter move only recomputes rows.

use crate::forward::{outer, regularised_inverse};
use crate::ring::LeadRing;
use glam::{DMat3, DVec3, Vec3};
use heartsim_core::Region;
use heartsim_geometry::{Catheter, CatheterKind, CellCloud};
use std::collections::VecDeque;

/// Electrograms are sampled once per 1 ms engine step (their deflections last a few ms).
pub const EGM_RATE_HZ: f32 = 1000.0;
/// Seconds of electrogram kept.
const HISTORY_S: usize = 30;
/// Deflections kept.
const DEFLECTION_HISTORY: usize = 4096;
/// Number of [`EgmSource`] values.
const SOURCES: usize = 4;
/// A deflection ends after the slope has stayed below threshold this long, ms.
const QUIET_MS: f64 = 12.0;
/// A slope counts as activity above this fraction of the channel's recent steepest slope.
const RELATIVE_SLOPE: f32 = 0.1;
/// The steepest-slope memory decays by this factor per ms (time constant 5 s).
const SLOPE_DECAY: f32 = 0.9998;
/// Deflections of the same wave across the pairs of one catheter lie within this, ms.
const GROUP_MS: f64 = 30.0;
/// Earliest and latest onset of the A before an H, ms.
const AH_WINDOW_MS: (f64, f64) = (20.0, 400.0);
/// Earliest and latest onset of the V after an H, ms.
const HV_WINDOW_MS: (f64, f64) = (10.0, 150.0);

/// Whose activity a part of the potential is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EgmSource {
    /// Atrial muscle, including the coronary-sinus sleeve: the A.
    Atrial = 0,
    /// His bundle, bundle branches, fascicles: the H.
    His = 1,
    /// Ventricular muscle: the V.
    Ventricular = 2,
    /// AV node: part of the trace, never a labelled deflection.
    Nodal = 3,
}

impl EgmSource {
    /// Source of a region; `None` for silent tissue (scar).
    #[must_use]
    pub fn of(region: Region) -> Option<Self> {
        match region {
            Region::Scar => None,
            Region::AvNodeCompact | Region::AvNodeFastPathway | Region::AvNodeSlowPathway => {
                Some(EgmSource::Nodal)
            }
            r if r.is_atrial() => Some(EgmSource::Atrial),
            r if r.is_conduction_system() => Some(EgmSource::His),
            _ => Some(EgmSource::Ventricular),
        }
    }

    /// Smallest slope that can open a deflection, mV/ms. The His is small and is the only deflection
    /// of its source, so it gets a lower floor than muscle, where T waves must stay out.
    fn slope_floor(self) -> f32 {
        match self {
            EgmSource::His => 0.002,
            _ => 0.02,
        }
    }
}

/// Settings of the local-potential rows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EgmOptions {
    /// Cells farther than this from an electrode do not contribute, mm.
    pub radius_mm: f32,
    /// Forward-model scale, mV per unit of normalised V_m (as [`crate::ForwardOptions`]).
    pub gain_mv: f32,
    /// Below this the electrode sees a cell as if at this distance (its own size), mm.
    pub min_distance_mm: f32,
}

impl Default for EgmOptions {
    fn default() -> Self {
        Self {
            radius_mm: 15.0,
            gain_mv: 5.0,
            min_distance_mm: 1.2,
        }
    }
}

/// One electrode's weights, grouped by source: `cells[ends[s−1]..ends[s]]` belong to source `s`.
#[derive(Debug, Clone, Default)]
struct Row {
    cells: Vec<u32>,
    w: Vec<f32>,
    ends: [u32; SOURCES],
}

/// Cell potentials → electrode potentials. See the module documentation.
#[derive(Debug, Clone)]
pub struct ElectrogramModel {
    opts: EgmOptions,
    positions: Vec<Vec3>,
    source: Vec<Option<EgmSource>>,
    cell_volume_mm3: f32,
    /// Least-squares gradient per cell (CSR): `∇V_i = Σ_k c_ik (V_k − V_i)`.
    offsets: Vec<u32>,
    neighbour: Vec<u32>,
    coef: Vec<Vec3>,
    coef_sum: Vec<Vec3>,
    rows: Vec<Row>,
    /// Reusable dense weight per cell of the row being built.
    scratch: Vec<f32>,
}

impl ElectrogramModel {
    /// Build the gradient operator of `cloud`; electrodes come with [`Self::set_electrodes`].
    #[must_use]
    pub fn new(cloud: &CellCloud, opts: EgmOptions) -> Self {
        let n = cloud.len();
        let source: Vec<Option<EgmSource>> = cloud.regions.iter().map(|r| EgmSource::of(*r)).collect();
        let mut offsets = Vec::with_capacity(n + 1);
        offsets.push(0u32);
        let (mut neighbour, mut coef) = (Vec::new(), Vec::new());
        let mut coef_sum = vec![Vec3::ZERO; n];
        for i in 0..n {
            if source[i].is_some() {
                let p = cloud.positions[i];
                let nbrs: Vec<(u32, DVec3)> = cloud.neighbours.get(i).map_or_else(Vec::new, |nb| {
                    nb.iter()
                        .filter(|k| (k.index as usize) < n && source[k.index as usize] == source[i])
                        .map(|k| (k.index, (cloud.positions[k.index as usize] - p).as_dvec3()))
                        .collect()
                });
                let m = nbrs.iter().fold(DMat3::ZERO, |m, (_, d)| m + outer(*d, *d));
                let inv = regularised_inverse(m);
                for (k, d) in nbrs {
                    let c = (inv * d).as_vec3();
                    coef_sum[i] += c;
                    neighbour.push(k);
                    coef.push(c);
                }
            }
            offsets.push(neighbour.len() as u32);
        }
        let h = crate::forward::spacing_mm(cloud);
        Self {
            opts,
            positions: cloud.positions.clone(),
            source,
            cell_volume_mm3: h * h * h,
            offsets,
            neighbour,
            coef,
            coef_sum,
            rows: Vec::new(),
            scratch: vec![0.0; n],
        }
    }

    /// Put the electrodes at `sites` (heart frame, mm). Rows are rebuilt.
    pub fn set_electrodes(&mut self, sites: &[Vec3]) {
        self.rows = sites.iter().map(|s| self.build_row(*s)).collect();
    }

    fn build_row(&mut self, site: Vec3) -> Row {
        fn add(scratch: &mut [f32], touched: &mut Vec<u32>, cell: u32, dw: f32) {
            // an exact cancellation back to 0 only re-lists the cell: deduplicated below
            if scratch[cell as usize] == 0.0 {
                touched.push(cell);
            }
            scratch[cell as usize] += dw;
        }
        let (k, vol) = (self.opts.gain_mv, self.cell_volume_mm3);
        let (radius2, min2) = (
            self.opts.radius_mm * self.opts.radius_mm,
            self.opts.min_distance_mm * self.opts.min_distance_mm,
        );
        let mut scratch = std::mem::take(&mut self.scratch);
        let mut touched: [Vec<u32>; SOURCES] = Default::default();
        for i in 0..self.positions.len() {
            let Some(src) = self.source[i] else { continue };
            let r = site - self.positions[i];
            let d2 = r.length_squared();
            if d2 > radius2 {
                continue;
            }
            let d2 = d2.max(min2);
            let u = r * (vol / (d2 * d2.sqrt()));
            let list = &mut touched[src as usize];
            for e in self.offsets[i] as usize..self.offsets[i + 1] as usize {
                add(&mut scratch, list, self.neighbour[e], -k * self.coef[e].dot(u));
            }
            add(&mut scratch, list, i as u32, k * self.coef_sum[i].dot(u));
        }
        let mut row = Row::default();
        for (s, list) in touched.iter_mut().enumerate() {
            list.sort_unstable();
            list.dedup();
            for &c in list.iter() {
                row.cells.push(c);
                row.w.push(scratch[c as usize]);
                scratch[c as usize] = 0.0;
            }
            row.ends[s] = row.cells.len() as u32;
        }
        self.scratch = scratch;
        row
    }

    /// Number of electrodes.
    #[must_use]
    pub fn electrode_count(&self) -> usize {
        self.rows.len()
    }

    /// Potential of each electrode per source, mV; `out` has one entry per electrode.
    pub fn sample(&self, vm: &[f32], out: &mut [[f32; SOURCES]]) {
        for (row, o) in self.rows.iter().zip(out.iter_mut()) {
            let mut start = 0usize;
            for (potential, &end) in o.iter_mut().zip(&row.ends) {
                let end = end as usize;
                *potential = row.cells[start..end]
                    .iter()
                    .zip(&row.w[start..end])
                    .map(|(&c, &w)| w * vm.get(c as usize).copied().unwrap_or(0.0))
                    .sum();
                start = end;
            }
        }
    }
}

/// One labelled activity on a bipole.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Deflection {
    /// Index into [`EgmRecorder::channels`].
    pub channel: usize,
    pub source: EgmSource,
    /// Where the slope first rises above threshold, ms.
    pub onset_ms: f64,
    /// Where the bipolar amplitude peaks: the local activation time, ms.
    pub peak_ms: f64,
    /// Peak amplitude, mV.
    pub amplitude_mv: f32,
}

#[derive(Debug, Clone, Copy)]
struct OpenDeflection {
    onset_ms: f64,
    peak_ms: f64,
    peak: f32,
    quiet_ms: f64,
}

/// Slope-gated activity detector on one source of one bipole.
#[derive(Debug, Clone)]
struct Detector {
    floor: f32,
    previous: f32,
    steepest: f32,
    open: Option<OpenDeflection>,
}

impl Detector {
    fn new(floor: f32) -> Self {
        Self {
            floor,
            previous: 0.0,
            steepest: 0.0,
            open: None,
        }
    }

    /// One sample; the finished deflection `(onset, peak, amplitude)` when one ends here.
    fn push(&mut self, x: f32, t_ms: f64) -> Option<(f64, f64, f32)> {
        let slope = (x - self.previous).abs();
        self.previous = x;
        self.steepest *= SLOPE_DECAY;
        let threshold = self.floor.max(RELATIVE_SLOPE * self.steepest);
        let active = slope > threshold;
        if active {
            self.steepest = self.steepest.max(slope);
        }
        match &mut self.open {
            Some(d) => {
                if x.abs() > d.peak {
                    d.peak = x.abs();
                    d.peak_ms = t_ms;
                }
                if active {
                    d.quiet_ms = 0.0;
                } else {
                    d.quiet_ms += 1.0;
                    if d.quiet_ms >= QUIET_MS {
                        let done = (d.onset_ms, d.peak_ms, d.peak);
                        self.open = None;
                        return Some(done);
                    }
                }
            }
            None => {
                if active {
                    self.open = Some(OpenDeflection {
                        onset_ms: t_ms,
                        peak_ms: t_ms,
                        peak: x.abs(),
                        quiet_ms: 0.0,
                    });
                }
            }
        }
        None
    }
}

/// One displayed electrogram: bipole `pair` of catheter `catheter`.
#[derive(Debug, Clone, PartialEq)]
pub struct EgmChannel {
    /// Clinical name, e.g. `CS 1-2`.
    pub label: String,
    pub kind: CatheterKind,
    /// Index of the catheter in the set given to [`EgmRecorder`].
    pub catheter: usize,
    /// Bipole index on the catheter, 0 = distal.
    pub pair: usize,
    electrode_a: usize,
    electrode_b: usize,
}

/// AH and HV of one beat on the His catheter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConductionIntervals {
    /// Onset of the A on the His catheter, ms.
    pub a_ms: f64,
    /// Onset of the H, ms.
    pub h_ms: f64,
    /// Earliest onset of the V on any catheter, ms.
    pub v_ms: f64,
    /// A to H, the AV-nodal conduction time, ms.
    pub ah_ms: f64,
    /// H to V, the His–Purkinje conduction time, ms.
    pub hv_ms: f64,
}

/// Records every bipole of a set of catheters. See the module documentation.
#[derive(Debug, Clone)]
pub struct EgmRecorder {
    model: ElectrogramModel,
    channels: Vec<EgmChannel>,
    potentials: Vec<[f32; SOURCES]>,
    bipolar: Vec<LeadRing>,
    unipolar: Vec<LeadRing>,
    detectors: Vec<[Detector; 3]>,
    deflections: VecDeque<Deflection>,
    stimuli: VecDeque<f64>,
    /// Time of the first sample of the rings, ms.
    first_ms: Option<f64>,
}

impl EgmRecorder {
    /// Record `catheters` on `cloud`.
    #[must_use]
    pub fn new(cloud: &CellCloud, catheters: &[Catheter], opts: EgmOptions) -> Self {
        let mut rec = Self {
            model: ElectrogramModel::new(cloud, opts),
            channels: Vec::new(),
            potentials: Vec::new(),
            bipolar: Vec::new(),
            unipolar: Vec::new(),
            detectors: Vec::new(),
            deflections: VecDeque::with_capacity(256),
            stimuli: VecDeque::new(),
            first_ms: None,
        };
        rec.set_catheters(catheters);
        rec
    }

    /// The catheters moved. Channels that still exist keep their recording, so the trace runs on
    /// across a move, as it does on a real recording system.
    pub fn set_catheters(&mut self, catheters: &[Catheter]) {
        let mut sites = Vec::new();
        let mut channels = Vec::new();
        for (ci, c) in catheters.iter().enumerate() {
            let base = sites.len();
            sites.extend_from_slice(&c.electrodes);
            for k in 0..c.pair_count() {
                let (a, b) = c.pair(k);
                channels.push(EgmChannel {
                    label: c.pair_label(k),
                    kind: c.kind,
                    catheter: ci,
                    pair: k,
                    electrode_a: base + a,
                    electrode_b: base + b,
                });
            }
        }
        self.model.set_electrodes(&sites);
        self.potentials = vec![[0.0; SOURCES]; sites.len()];
        let capacity = HISTORY_S * EGM_RATE_HZ as usize;
        if channels.len() != self.channels.len() {
            let n = channels.len();
            self.bipolar = (0..n).map(|_| LeadRing::new(capacity, EGM_RATE_HZ)).collect();
            self.unipolar = (0..n).map(|_| LeadRing::new(capacity, EGM_RATE_HZ)).collect();
            self.detectors = (0..n)
                .map(|_| {
                    [EgmSource::Atrial, EgmSource::His, EgmSource::Ventricular]
                        .map(|s| Detector::new(s.slope_floor()))
                })
                .collect();
        }
        self.channels = channels;
    }

    /// The recorded channels, in catheter order.
    #[must_use]
    pub fn channels(&self) -> &[EgmChannel] {
        &self.channels
    }

    /// Bipolar trace of `channel`: distal minus proximal electrode, mV.
    #[must_use]
    pub fn bipolar(&self, channel: usize) -> &LeadRing {
        &self.bipolar[channel]
    }

    /// Unipolar trace of the distal electrode of `channel`, mV.
    #[must_use]
    pub fn unipolar(&self, channel: usize) -> &LeadRing {
        &self.unipolar[channel]
    }

    /// Labelled deflections, oldest first (those that have ended).
    pub fn deflections(&self) -> impl Iterator<Item = &Deflection> {
        self.deflections.iter()
    }

    /// A stimulus was delivered at `at_ms` (the mark on the traces).
    pub fn mark_stimulus(&mut self, at_ms: f64) {
        self.stimuli.push_back(at_ms);
        while self
            .stimuli
            .front()
            .is_some_and(|t| at_ms - *t > HISTORY_S as f64 * 1000.0)
        {
            self.stimuli.pop_front();
        }
    }

    /// Stimulus times still in the history, ms.
    pub fn stimuli(&self) -> impl Iterator<Item = f64> + '_ {
        self.stimuli.iter().copied()
    }

    /// Time of sample `abs` of the rings, ms.
    #[must_use]
    pub fn time_of_sample(&self, abs: u64) -> f64 {
        self.first_ms.unwrap_or(0.0) + abs as f64
    }

    /// Absolute sample nearest to `t_ms` (may be off the end of the rings).
    #[must_use]
    pub fn sample_of_time(&self, t_ms: f64) -> u64 {
        (t_ms - self.first_ms.unwrap_or(0.0)).max(0.0) as u64
    }

    /// Record one engine step: `vm` is the cell state at time `t_ms`.
    pub fn push(&mut self, vm: &[f32], t_ms: f64) {
        self.first_ms.get_or_insert(t_ms);
        self.model.sample(vm, &mut self.potentials);
        for (c, channel) in self.channels.iter().enumerate() {
            let a = self.potentials[channel.electrode_a];
            let b = self.potentials[channel.electrode_b];
            self.unipolar[c].push(a.iter().sum());
            self.bipolar[c].push(a.iter().zip(&b).map(|(x, y)| x - y).sum());
            for (s, source) in [EgmSource::Atrial, EgmSource::His, EgmSource::Ventricular]
                .into_iter()
                .enumerate()
            {
                let x = a[source as usize] - b[source as usize];
                if let Some((onset_ms, peak_ms, amplitude_mv)) = self.detectors[c][s].push(x, t_ms) {
                    self.deflections.push_back(Deflection {
                        channel: c,
                        source,
                        onset_ms,
                        peak_ms,
                        amplitude_mv,
                    });
                    if self.deflections.len() > DEFLECTION_HISTORY {
                        self.deflections.pop_front();
                    }
                }
            }
        }
    }

    fn on_kind(&self, kind: CatheterKind) -> impl Iterator<Item = &Deflection> {
        self.deflections
            .iter()
            .filter(move |d| self.channels[d.channel].kind == kind)
    }

    /// AH and HV of the last complete beat on the His catheter. HV ends at the earliest V on any
    /// catheter, as clinically it ends at the earliest ventricular activation in any lead.
    #[must_use]
    pub fn conduction_intervals(&self) -> Option<ConductionIntervals> {
        let onsets = |s: EgmSource| -> Vec<f64> {
            let mut v: Vec<f64> = self
                .on_kind(CatheterKind::His)
                .filter(|d| d.source == s)
                .map(|d| d.onset_ms)
                .collect();
            v.sort_by(f64::total_cmp);
            v
        };
        let (his, atrial) = (onsets(EgmSource::His), onsets(EgmSource::Atrial));
        let mut ventricular: Vec<f64> = self
            .deflections
            .iter()
            .filter(|d| d.source == EgmSource::Ventricular)
            .map(|d| d.onset_ms)
            .collect();
        ventricular.sort_by(f64::total_cmp);
        // The first H of a wave across the pairs; later ones in the group are the same wave or a
        // bundle branch further down.
        let mut starts: Vec<f64> = Vec::new();
        for &h in &his {
            if starts.last().is_none_or(|l| h - l > GROUP_MS) {
                starts.push(h);
            }
        }
        starts.iter().rev().find_map(|&h_ms| {
            let v_ms = *ventricular
                .iter()
                .find(|v| (HV_WINDOW_MS.0..=HV_WINDOW_MS.1).contains(&(**v - h_ms)))?;
            let last_a = atrial
                .iter()
                .rev()
                .find(|a| (AH_WINDOW_MS.0..=AH_WINDOW_MS.1).contains(&(h_ms - **a)))?;
            let a_ms = *atrial.iter().find(|a| *last_a - **a <= GROUP_MS)?;
            Some(ConductionIntervals {
                a_ms,
                h_ms,
                v_ms,
                ah_ms: h_ms - a_ms,
                hv_ms: v_ms - h_ms,
            })
        })
    }

    /// Local activation time per bipole of a catheter in `from_ms..to_ms`, distal bipole first:
    /// the peak of the first deflection of `source`, `None` where there was none. On the CS this is
    /// the activation sequence along the mitral annulus.
    #[must_use]
    pub fn activation_sequence(
        &self,
        catheter: usize,
        source: EgmSource,
        from_ms: f64,
        to_ms: f64,
    ) -> Vec<Option<f64>> {
        self.channels
            .iter()
            .enumerate()
            .filter(|(_, c)| c.catheter == catheter)
            .map(|(i, _)| {
                self.deflections
                    .iter()
                    .filter(|d| d.channel == i && d.source == source)
                    .map(|d| d.peak_ms)
                    .find(|t| (from_ms..to_ms).contains(t))
            })
            .collect()
    }

    /// Time between the last two activations of `source` on `channel`, ms: the cycle length there.
    #[must_use]
    pub fn cycle_length_ms(&self, channel: usize, source: EgmSource) -> Option<f64> {
        let mut last = self
            .deflections
            .iter()
            .rev()
            .filter(|d| d.channel == channel && d.source == source);
        let a = last.next()?;
        let b = last.next()?;
        Some(a.peak_ms - b.peak_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use heartsim_geometry::CatheterSite;

    /// A slab of atrial cells with a planar front moving along +x, as a V_m field.
    fn slab() -> CellCloud {
        CellCloud::test_sphere(1.5, 12.0, Region::RightAtrium)
    }

    fn front(cloud: &CellCloud, x_front: f32) -> Vec<f32> {
        cloud
            .positions
            .iter()
            .map(|p| ((x_front - p.x) / 1.5).clamp(0.0, 1.0))
            .collect()
    }

    #[test]
    fn uniform_vm_gives_no_potential() {
        let cloud = slab();
        let mut m = ElectrogramModel::new(&cloud, EgmOptions::default());
        m.set_electrodes(&[Vec3::new(0.0, 0.0, 14.0)]);
        let mut out = [[0.0; SOURCES]];
        m.sample(&vec![0.7; cloud.len()], &mut out);
        assert!(out[0].iter().all(|x| x.abs() < 1e-3), "{out:?}");
    }

    #[test]
    fn a_passing_front_gives_positive_then_negative() {
        let cloud = slab();
        let mut m = ElectrogramModel::new(&cloud, EgmOptions::default());
        m.set_electrodes(&[Vec3::new(0.0, 0.0, 14.0)]);
        let mut out = [[0.0; SOURCES]];
        m.sample(&front(&cloud, -6.0), &mut out);
        let approaching = out[0][EgmSource::Atrial as usize];
        m.sample(&front(&cloud, 6.0), &mut out);
        let leaving = out[0][EgmSource::Atrial as usize];
        assert!(
            approaching > 0.0 && leaving < 0.0,
            "approaching {approaching}, leaving {leaving}"
        );
    }

    #[test]
    fn far_cells_do_not_contribute_and_sources_stay_apart() {
        let cloud = slab();
        let mut m = ElectrogramModel::new(&cloud, EgmOptions::default());
        m.set_electrodes(&[Vec3::new(0.0, 0.0, 100.0)]);
        assert!(m.rows[0].cells.is_empty(), "nothing within 15 mm");
        m.set_electrodes(&[Vec3::new(0.0, 0.0, 14.0)]);
        let e = m.rows[0].ends;
        assert_eq!(e[0] as usize, m.rows[0].cells.len(), "all atrial");
        assert_eq!(e[0], e[3]);
    }

    #[test]
    fn the_detector_times_a_deflection_and_ignores_slow_drift() {
        let mut d = Detector::new(0.02);
        let mut found = Vec::new();
        for t in 0..200 {
            let t_ms = f64::from(t);
            // a sharp biphasic deflection at 50 ms, then a slow T-like drift
            let x = match t {
                48..=50 => (t - 47) as f32 * 0.5,
                51..=53 => (54 - t) as f32 * -0.5,
                _ if t > 100 => (t - 100) as f32 * 0.002,
                _ => 0.0,
            };
            found.extend(d.push(x, t_ms));
        }
        assert_eq!(found.len(), 1, "{found:?}");
        let (onset, peak, amp) = found[0];
        assert_eq!(onset, 48.0);
        assert!((50.0..=51.0).contains(&peak), "peak at {peak}");
        assert!(amp >= 1.5);
    }

    #[test]
    fn the_bipole_is_the_difference_of_its_electrodes() {
        let cloud = slab();
        let catheter = Catheter {
            kind: CatheterKind::Hra,
            site: CatheterSite::Default,
            body: vec![Vec3::new(0.0, 0.0, 30.0), Vec3::new(0.0, 0.0, 14.0)],
            electrodes: vec![Vec3::new(0.0, 0.0, 14.0), Vec3::new(2.0, 0.0, 14.0)],
        };
        let mut rec = EgmRecorder::new(&cloud, std::slice::from_ref(&catheter), EgmOptions::default());
        assert_eq!(rec.channels()[0].label, "HRA 1-2");
        let vm = front(&cloud, 0.0);
        rec.push(&vm, 0.0);
        let mut m = ElectrogramModel::new(&cloud, EgmOptions::default());
        m.set_electrodes(&catheter.electrodes);
        let mut out = [[0.0; SOURCES]; 2];
        m.sample(&vm, &mut out);
        let expected: f32 = out[0].iter().sum::<f32>() - out[1].iter().sum::<f32>();
        let got = rec.bipolar(0).at(0).unwrap();
        assert!((got - expected).abs() < 1e-4, "{got} vs {expected}");
    }
}
