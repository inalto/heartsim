//! Simulation cell cloud with k-nearest-neighbour graph.

use glam::Vec3;
use heartsim_core::Region;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const K_NEIGHBOURS: usize = 8;

/// One directed edge of the neighbourhood graph.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Neighbour {
    pub index: u32,
    /// Euclidean distance, mm.
    pub distance_mm: f32,
    /// |cos| of the angle between the edge and the local fibre direction (1 = along fibre).
    pub fibre_alignment: f32,
}

/// Structure-of-arrays cell cloud. Indices are stable; everything is in heart coordinates (mm).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CellCloud {
    pub positions: Vec<Vec3>,
    /// Unit fibre direction per cell.
    pub fibres: Vec<Vec3>,
    pub regions: Vec<Region>,
    /// Transmural depth 0 = endocardium … 1 = epicardium (atria: 0.5).
    pub depth: Vec<f32>,
    /// `K_NEIGHBOURS` entries per cell, nearest first.
    pub neighbours: Vec<[Neighbour; K_NEIGHBOURS]>,
    /// Named landmarks → cell index (e.g. "sa_node", "rv_apex", "his_prox").
    pub landmarks: std::collections::BTreeMap<String, u32>,
}

impl CellCloud {
    #[must_use]
    pub fn len(&self) -> usize {
        self.positions.len()
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    /// Build a cloud from positions, fibres and regions; computes the k-NN graph.
    #[must_use]
    pub fn build(positions: Vec<Vec3>, fibres: Vec<Vec3>, regions: Vec<Region>, depth: Vec<f32>) -> Self {
        assert_eq!(positions.len(), fibres.len());
        assert_eq!(positions.len(), regions.len());
        assert_eq!(positions.len(), depth.len());
        let mut cloud = Self {
            positions,
            fibres,
            regions,
            depth,
            neighbours: Vec::new(),
            landmarks: std::collections::BTreeMap::default(),
        };
        cloud.rebuild_neighbours();
        cloud
    }

    pub fn rebuild_neighbours(&mut self) {
        // Uniform hash grid: cell size = typical spacing estimate; search the 27 surrounding bins,
        // widening if fewer than K candidates were found (sparse regions).
        let n = self.positions.len();
        let empty = Neighbour {
            index: u32::MAX,
            distance_mm: f32::INFINITY,
            fibre_alignment: 0.0,
        };
        if n < 2 {
            self.neighbours = vec![[empty; K_NEIGHBOURS]; n];
            return;
        }
        let (lo, hi) = self.positions.iter().fold(
            (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
            |(l, h), p| (l.min(*p), h.max(*p)),
        );
        let extent = (hi - lo).max(Vec3::splat(1e-3));
        let volume = extent.x * extent.y * extent.z;
        let spacing = (volume / n as f32).cbrt().max(1e-3) * 1.5;
        let key = |p: Vec3| -> (i32, i32, i32) {
            let q = (p - lo) / spacing;
            (q.x as i32, q.y as i32, q.z as i32)
        };
        let mut grid: HashMap<(i32, i32, i32), Vec<u32>> = HashMap::with_capacity(n / 2 + 1);
        for (i, p) in self.positions.iter().enumerate() {
            grid.entry(key(*p)).or_default().push(i as u32);
        }
        let mut out = Vec::with_capacity(n);
        let mut cand: Vec<(f32, u32)> = Vec::new();
        for i in 0..n {
            let p = self.positions[i];
            let f = self.fibres[i];
            let (kx, ky, kz) = key(p);
            let mut radius = 1;
            loop {
                cand.clear();
                for dx in -radius..=radius {
                    for dy in -radius..=radius {
                        for dz in -radius..=radius {
                            if let Some(bin) = grid.get(&(kx + dx, ky + dy, kz + dz)) {
                                for &j in bin {
                                    if j as usize != i {
                                        cand.push((self.positions[j as usize].distance_squared(p), j));
                                    }
                                }
                            }
                        }
                    }
                }
                // enough candidates and the K-th is inside the guaranteed-complete sphere?
                let complete_r2 = (radius as f32 * spacing).powi(2);
                if cand.len() >= K_NEIGHBOURS {
                    cand.select_nth_unstable_by(K_NEIGHBOURS - 1, |a, b| a.0.total_cmp(&b.0));
                    if cand[K_NEIGHBOURS - 1].0 <= complete_r2 || radius > 8 {
                        break;
                    }
                } else if radius > 8 {
                    break;
                }
                radius += 1;
            }
            cand.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
            let mut nb = [empty; K_NEIGHBOURS];
            for (k, (d2, j)) in cand.iter().take(K_NEIGHBOURS).enumerate() {
                let d = self.positions[*j as usize] - p;
                let dist = d2.sqrt();
                let align = if dist > 0.0 { (d / dist).dot(f).abs() } else { 1.0 };
                nb[k] = Neighbour {
                    index: *j,
                    distance_mm: dist,
                    fibre_alignment: align,
                };
            }
            out.push(nb);
        }
        self.neighbours = out;
    }

    /// Nearest cell to a point (for picking, stimulation, ablation).
    #[must_use]
    pub fn nearest(&self, p: Vec3) -> Option<u32> {
        let mut best = None;
        let mut best_d = f32::INFINITY;
        for (i, q) in self.positions.iter().enumerate() {
            let d = q.distance_squared(p);
            if d < best_d {
                best_d = d;
                best = Some(i as u32);
            }
        }
        best
    }

    /// A regular lattice of cells inside a unit sphere — test fixture and benchmark input.
    #[must_use]
    pub fn test_sphere(spacing_mm: f32, radius_mm: f32, region: Region) -> Self {
        let n = (2.0 * radius_mm / spacing_mm).ceil() as i32;
        let mut pos = Vec::new();
        for x in -n..=n {
            for y in -n..=n {
                for z in -n..=n {
                    let p = Vec3::new(x as f32, y as f32, z as f32) * spacing_mm;
                    if p.length() <= radius_mm {
                        pos.push(p);
                    }
                }
            }
        }
        let len = pos.len();
        Self::build(pos, vec![Vec3::X; len], vec![region; len], vec![0.5; len])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knn_is_symmetric_enough_and_sorted() {
        let c = CellCloud::test_sphere(1.0, 4.0, Region::LeftVentricle);
        assert!(c.len() > 200);
        for (i, nb) in c.neighbours.iter().enumerate() {
            assert!(nb.iter().all(|n| n.index != i as u32));
            for w in nb.windows(2) {
                assert!(w[0].distance_mm <= w[1].distance_mm);
            }
            // on a unit lattice the 6 nearest are at distance 1 for interior cells
            if c.positions[i].length() < 2.0 {
                assert!((nb[0].distance_mm - 1.0).abs() < 1e-5);
                assert!((nb[5].distance_mm - 1.0).abs() < 1e-5);
            }
        }
    }

    #[test]
    fn fibre_alignment_is_cosine() {
        let c = CellCloud::test_sphere(1.0, 3.0, Region::LeftVentricle);
        let centre = c.nearest(Vec3::ZERO).unwrap() as usize;
        let along: Vec<_> = c.neighbours[centre]
            .iter()
            .filter(|n| n.fibre_alignment > 0.99)
            .collect();
        assert_eq!(along.len(), 2, "±X neighbours align with fibre X");
    }
}
