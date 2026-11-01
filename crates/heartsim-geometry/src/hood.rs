//! Vertex → cell "hoods": for every render-mesh vertex the `K_NEIGHBOURS` nearest cells with
//! inverse-distance weights, so per-vertex colour can be interpolated from cell state every frame
//! without a spatial query.

use crate::cloud::{CellCloud, K_NEIGHBOURS};
use crate::mesh::TriMesh;
use crate::spatial::HashGrid;
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Neighbourhood of one mesh vertex. Weights are non-negative and sum to 1.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Hood {
    pub cells: [u32; K_NEIGHBOURS],
    pub weights: [f32; K_NEIGHBOURS],
}

impl Hood {
    /// Interpolate a per-cell scalar at this vertex.
    #[must_use]
    pub fn interpolate(&self, per_cell: &[f32]) -> f32 {
        self.cells
            .iter()
            .zip(self.weights)
            .map(|(c, w)| per_cell[*c as usize] * w)
            .sum()
    }
}

/// Compute hoods for every vertex of `mesh` against `cells`. Inverse-distance weights
/// (`1/d`, normalised); a vertex coincident with a cell gets weight 1 on that cell.
#[must_use]
pub fn compute_hoods(mesh: &TriMesh, cells: &CellCloud) -> Vec<Hood> {
    let grid = HashGrid::with_auto_cell(&cells.positions);
    let mut found = Vec::with_capacity(K_NEIGHBOURS);
    mesh.vertices
        .iter()
        .map(|v| {
            grid.knn(Vec3::from(v.position), K_NEIGHBOURS, None, &mut found);
            let mut cells_idx = [0u32; K_NEIGHBOURS];
            let mut weights = [0f32; K_NEIGHBOURS];
            if let Some(exact) = found.iter().position(|(d, _)| *d <= 1e-6) {
                for (k, (_, j)) in found.iter().enumerate() {
                    cells_idx[k] = *j;
                }
                weights[exact] = 1.0;
            } else {
                let mut sum = 0.0;
                for (k, (d, j)) in found.iter().enumerate() {
                    cells_idx[k] = *j;
                    weights[k] = 1.0 / d;
                    sum += weights[k];
                }
                for w in &mut weights {
                    *w /= sum;
                }
            }
            // fewer than K cells (tiny test clouds): repeat the nearest so indices stay valid
            for k in found.len()..K_NEIGHBOURS {
                cells_idx[k] = cells_idx[0];
            }
            Hood {
                cells: cells_idx,
                weights,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::icosphere;
    use heartsim_core::Region;

    #[test]
    fn weights_sum_to_one_and_prefer_the_nearest_cell() {
        let cells = CellCloud::test_sphere(1.0, 4.0, Region::LeftVentricle);
        let mut mesh = icosphere(2);
        for v in &mut mesh.vertices {
            v.position = (Vec3::from(v.position) * 3.3).to_array();
        }
        let hoods = compute_hoods(&mesh, &cells);
        assert_eq!(hoods.len(), mesh.vertex_count());
        for (v, h) in mesh.vertices.iter().zip(&hoods) {
            let s: f32 = h.weights.iter().sum();
            assert!((s - 1.0).abs() < 1e-5);
            assert!(h.weights[0] >= h.weights[K_NEIGHBOURS - 1]);
            let p = Vec3::from(v.position);
            assert_eq!(h.cells[0], cells.nearest(p).unwrap());
        }
        let per_cell = vec![2.0; cells.len()];
        assert!((hoods[0].interpolate(&per_cell) - 2.0).abs() < 1e-5);
    }
}
