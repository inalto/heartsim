//! Simulation cell cloud with k-nearest-neighbour graph.

use crate::spatial::HashGrid;
use glam::Vec3;
use heartsim_core::Region;
use serde::{Deserialize, Serialize};

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

    /// Recompute the k-NN graph (exact, via a uniform hash grid). Fibre alignment is the |cos| of
    /// the angle between each edge and the source cell's fibre.
    pub fn rebuild_neighbours(&mut self) {
        let n = self.positions.len();
        let empty = Neighbour {
            index: u32::MAX,
            distance_mm: f32::INFINITY,
            fibre_alignment: 0.0,
        };
        let grid = HashGrid::with_auto_cell(&self.positions);
        let mut out = Vec::with_capacity(n);
        let mut found = Vec::with_capacity(K_NEIGHBOURS);
        for i in 0..n {
            let p = self.positions[i];
            let f = self.fibres[i];
            grid.knn(p, K_NEIGHBOURS, Some(i as u32), &mut found);
            let mut nb = [empty; K_NEIGHBOURS];
            for (k, (dist, j)) in found.iter().enumerate() {
                let d = self.positions[*j as usize] - p;
                let align = if *dist > 0.0 {
                    (d / *dist).dot(f).abs()
                } else {
                    1.0
                };
                nb[k] = Neighbour {
                    index: *j,
                    distance_mm: *dist,
                    fibre_alignment: align,
                };
            }
            out.push(nb);
        }
        self.neighbours = out;
    }

    /// Mean distance to the nearest neighbour (a proxy for the sampling spacing).
    #[must_use]
    pub fn mean_nearest_distance_mm(&self) -> f32 {
        let valid: Vec<f32> = self
            .neighbours
            .iter()
            .map(|nb| nb[0].distance_mm)
            .filter(|d| d.is_finite())
            .collect();
        if valid.is_empty() {
            0.0
        } else {
            valid.iter().sum::<f32>() / valid.len() as f32
        }
    }

    /// Number of cells per region, in `Region::ALL` order (regions with zero cells omitted).
    #[must_use]
    pub fn region_counts(&self) -> Vec<(Region, usize)> {
        Region::ALL
            .iter()
            .map(|r| (*r, self.regions.iter().filter(|x| *x == r).count()))
            .filter(|(_, n)| *n > 0)
            .collect()
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
