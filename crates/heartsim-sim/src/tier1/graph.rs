//! Propagation graph of the Tier-1 engine: a radius neighbourhood over the cell cloud, stored in
//! compressed-sparse-row form with per-edge distance and fibre alignment.
//!
//! **Why not the pack's 8-NN graph.** On a regular lattice the 8 nearest neighbours are the 6 face
//! neighbours plus two arbitrary diagonals, so a graph geodesic towards a body diagonal is up to
//! √3 ≈ 1.7× too long (the metrication error of Wallman et al. 2012). Connecting every cell to all
//! cells within ≈ 2.3 lattice spacings (the 56-neighbourhood on a cubic lattice) brings the error
//! down to a few per cent at ≈ 7× the edges, still well inside the per-beat budget.
//!
//! **Which edges exist.** Destinations are always working myocardium (see [`CellKind`]); the
//! conduction system is driven by timing events, never by neighbours, so the mesh cannot open a
//! second AV path. The annulus fibrosus insulates atria from ventricles: no edge crosses between
//! the two classes. SA-node cells feed the atrial myocardium and Purkinje cells feed the
//! ventricular myocardium (exits); all other conduction-system cells have no outgoing edge.

use glam::Vec3;
use heartsim_core::Region;
use heartsim_geometry::CellCloud;

/// How a cell takes part in propagation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellKind {
    /// Atrial working myocardium: activated by neighbours, conducts to atrial neighbours.
    Atrial,
    /// Ventricular working myocardium (including scar, border zone, ischaemic tissue).
    Ventricular,
    /// SA-node cell: activated by sinus events, exits into the atrial myocardium.
    SinusNode,
    /// Purkinje cell (packs with an explicit network): activated by events, exits into the
    /// ventricular myocardium.
    Purkinje,
    /// AV node, His bundle, bundle branches, fascicles, accessory pathway: activated by events
    /// only, never connected to the myocardium (the timing engine owns those delays).
    Driven,
}

impl CellKind {
    /// Classification of a region.
    #[must_use]
    pub fn of(region: Region) -> Self {
        match region {
            Region::SinoatrialNode => CellKind::SinusNode,
            Region::PurkinjeLeft | Region::PurkinjeRight => CellKind::Purkinje,
            Region::Scar | Region::BorderZone | Region::Ischaemic => CellKind::Ventricular,
            r if r.is_atrial() => CellKind::Atrial,
            r if r.is_ventricular() => CellKind::Ventricular,
            _ => CellKind::Driven,
        }
    }

    /// Whether neighbours can activate a cell of this kind.
    #[must_use]
    pub fn is_myocardium(self) -> bool {
        matches!(self, CellKind::Atrial | CellKind::Ventricular)
    }

    /// Whether an edge `self → dst` exists.
    #[must_use]
    pub fn connects_to(self, dst: CellKind) -> bool {
        matches!(
            (self, dst),
            (CellKind::Atrial | CellKind::SinusNode, CellKind::Atrial)
                | (CellKind::Ventricular | CellKind::Purkinje, CellKind::Ventricular)
        )
    }
}

/// CSR adjacency: edges of cell `i` are `offsets[i]..offsets[i + 1]`.
#[derive(Debug, Clone, Default)]
pub struct Graph {
    /// `n + 1` offsets into the edge arrays.
    pub offsets: Vec<u32>,
    /// Destination cell of each edge.
    pub targets: Vec<u32>,
    /// Edge length, mm.
    pub distance_mm: Vec<f32>,
    /// |cos| between the edge and the *destination* cell's fibre (1 = along the fibre).
    pub alignment: Vec<f32>,
    /// Neighbourhood radius used to build the graph, mm.
    pub radius_mm: f32,
}

impl Graph {
    /// Edge index range of cell `i`.
    #[must_use]
    pub fn edges(&self, i: usize) -> std::ops::Range<usize> {
        self.offsets[i] as usize..self.offsets[i + 1] as usize
    }

    /// Number of directed edges.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.targets.len()
    }

    /// Build the radius graph. `radius_mm` ≤ 0 selects `radius_factor` × the median
    /// nearest-neighbour distance of the cloud. At most `max_edges` nearest edges are kept per cell.
    #[must_use]
    pub fn build(cloud: &CellCloud, kinds: &[CellKind], radius_factor: f32, max_edges: usize) -> Self {
        Self::build_with_links(cloud, kinds, radius_factor, max_edges, &[])
    }

    /// [`Graph::build`] with extra undirected edges for muscular connections longer than the
    /// neighbourhood radius (the coronary-sinus sleeve joining the right atrium at the ostium).
    #[must_use]
    pub fn build_with_links(
        cloud: &CellCloud,
        kinds: &[CellKind],
        radius_factor: f32,
        max_edges: usize,
        links: &[(u32, u32)],
    ) -> Self {
        let n = cloud.len();
        let spacing = median_spacing(cloud);
        let radius = (radius_factor * spacing).max(1e-3);
        let grid = BinGrid::new(&cloud.positions, radius);
        let mut offsets = Vec::with_capacity(n + 1);
        let mut targets = Vec::new();
        let mut distance_mm = Vec::new();
        let mut alignment = Vec::new();
        let mut found: Vec<(f32, u32)> = Vec::with_capacity(128);
        offsets.push(0u32);
        for i in 0..n {
            let src = kinds[i];
            if matches!(src, CellKind::Driven) {
                offsets.push(targets.len() as u32);
                continue;
            }
            let p = cloud.positions[i];
            grid.within(&cloud.positions, p, radius, &mut found);
            found.retain(|&(_, j)| j as usize != i && src.connects_to(kinds[j as usize]));
            found.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            found.truncate(max_edges);
            for &(a, b) in links {
                let other = match (a as usize == i, b as usize == i) {
                    (true, false) => b,
                    (false, true) => a,
                    _ => continue,
                };
                if !found.iter().any(|&(_, j)| j == other) {
                    found.push((p.distance(cloud.positions[other as usize]), other));
                }
            }
            for &(d, j) in &found {
                let e = (cloud.positions[j as usize] - p) / d.max(1e-6);
                let f = cloud.fibres[j as usize];
                let c = if f.length_squared() > 0.0 {
                    e.dot(f.normalize()).abs().min(1.0)
                } else {
                    1.0
                };
                targets.push(j);
                distance_mm.push(d);
                alignment.push(c);
            }
            offsets.push(targets.len() as u32);
        }
        Self {
            offsets,
            targets,
            distance_mm,
            alignment,
            radius_mm: radius,
        }
    }
}

/// Median distance to the nearest neighbour (robust to the dense conduction polylines).
#[must_use]
pub fn median_spacing(cloud: &CellCloud) -> f32 {
    let mut d: Vec<f32> = cloud
        .neighbours
        .iter()
        .map(|nb| nb[0].distance_mm)
        .filter(|d| d.is_finite() && *d > 0.0)
        .collect();
    if d.is_empty() {
        return 1.0;
    }
    let mid = d.len() / 2;
    d.select_nth_unstable_by(mid, f32::total_cmp);
    d[mid]
}

/// Dense uniform bins of side `bin_mm` for fixed-radius queries. Bins hold indices in ascending
/// order, so results are deterministic.
struct BinGrid {
    lo: Vec3,
    bin_mm: f32,
    dims: [usize; 3],
    start: Vec<u32>,
    items: Vec<u32>,
}

impl BinGrid {
    fn new(points: &[Vec3], bin_mm: f32) -> Self {
        let (lo, hi) = points.iter().fold(
            (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
            |(l, h), p| (l.min(*p), h.max(*p)),
        );
        if points.is_empty() {
            return Self {
                lo: Vec3::ZERO,
                bin_mm,
                dims: [1, 1, 1],
                start: vec![0, 0],
                items: Vec::new(),
            };
        }
        // Keep the dense grid bounded for very sparse clouds by enlarging the bins.
        let mut bin = bin_mm;
        let dims = loop {
            let ext = ((hi - lo) / bin).floor() + Vec3::ONE;
            let d = [ext.x as usize, ext.y as usize, ext.z as usize];
            if d[0] * d[1] * d[2] <= 8 * points.len() + 1024 {
                break d;
            }
            bin *= 1.5;
        };
        let mut s = Self {
            lo,
            bin_mm: bin,
            dims,
            start: Vec::new(),
            items: Vec::new(),
        };
        let nbins = dims[0] * dims[1] * dims[2];
        let mut counts = vec![0u32; nbins + 1];
        let keys: Vec<usize> = points.iter().map(|p| s.flat(s.key(*p))).collect();
        for &k in &keys {
            counts[k + 1] += 1;
        }
        for b in 0..nbins {
            counts[b + 1] += counts[b];
        }
        let mut fill = counts.clone();
        let mut items = vec![0u32; points.len()];
        for (i, &k) in keys.iter().enumerate() {
            items[fill[k] as usize] = i as u32;
            fill[k] += 1;
        }
        s.start = counts;
        s.items = items;
        s
    }

    fn key(&self, p: Vec3) -> [usize; 3] {
        let q = ((p - self.lo) / self.bin_mm).floor().max(Vec3::ZERO);
        [
            (q.x as usize).min(self.dims[0] - 1),
            (q.y as usize).min(self.dims[1] - 1),
            (q.z as usize).min(self.dims[2] - 1),
        ]
    }

    fn flat(&self, k: [usize; 3]) -> usize {
        (k[2] * self.dims[1] + k[1]) * self.dims[0] + k[0]
    }

    /// All points within `r` of `p` (including `p` itself if it is in the set), unsorted.
    fn within(&self, points: &[Vec3], p: Vec3, r: f32, out: &mut Vec<(f32, u32)>) {
        out.clear();
        let reach = ((r / self.bin_mm).ceil() as usize).cast_signed();
        let k = self.key(p);
        let r2 = r * r;
        for dz in -reach..=reach {
            let z = k[2].cast_signed() + dz;
            if z < 0 || z >= self.dims[2].cast_signed() {
                continue;
            }
            for dy in -reach..=reach {
                let y = k[1].cast_signed() + dy;
                if y < 0 || y >= self.dims[1].cast_signed() {
                    continue;
                }
                for dx in -reach..=reach {
                    let x = k[0].cast_signed() + dx;
                    if x < 0 || x >= self.dims[0].cast_signed() {
                        continue;
                    }
                    let b = self.flat([x as usize, y as usize, z as usize]);
                    for &j in &self.items[self.start[b] as usize..self.start[b + 1] as usize] {
                        let d2 = points[j as usize].distance_squared(p);
                        if d2 <= r2 {
                            out.push((d2.sqrt(), j));
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lattice_gets_the_56_neighbourhood() {
        let c = CellCloud::test_sphere(1.0, 5.0, Region::LeftVentricle);
        let kinds = vec![CellKind::Ventricular; c.len()];
        let g = Graph::build(&c, &kinds, 2.3, 64);
        let centre = c.nearest(Vec3::ZERO).unwrap() as usize;
        assert_eq!(g.edges(centre).len(), 56);
        let along = g
            .edges(centre)
            .filter(|&e| g.alignment[e] > 0.999 && (g.distance_mm[e] - 1.0).abs() < 1e-5)
            .count();
        assert_eq!(along, 2, "±x unit edges align with fibre x");
    }

    #[test]
    fn atria_and_ventricles_are_insulated() {
        assert!(!CellKind::Atrial.connects_to(CellKind::Ventricular));
        assert!(!CellKind::Ventricular.connects_to(CellKind::Atrial));
        assert!(CellKind::SinusNode.connects_to(CellKind::Atrial));
        assert!(!CellKind::Driven.connects_to(CellKind::Ventricular));
        assert_eq!(CellKind::of(Region::HisBundle), CellKind::Driven);
        assert_eq!(CellKind::of(Region::Scar), CellKind::Ventricular);
    }
}
