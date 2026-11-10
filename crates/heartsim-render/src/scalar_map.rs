//! Cell state → per-vertex scalars through the anatomy pack's hoods (`docs/ARCHITECTURE.md` §5).
//!
//! The physics runs on the cell cloud; every render-mesh vertex owns a precomputed hood of its
//! nearest cells with weights summing to 1, so a vertex scalar is the convex combination
//! `s_v = Σ_k w_vk · x[cell_vk]`. This is the only path by which engine state reaches the 3-D
//! colour map, so the map can never disagree with the engine (the ECG is derived from the same
//! cell state).

use heartsim_geometry::{Hood, TriMesh};
#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Vertices per rayon task: large enough to amortise scheduling, small enough to balance the
/// ~20 k-vertex chamber meshes over a handful of cores.
#[cfg(feature = "parallel")]
const CHUNK: usize = 4096;

/// Fill `out[v] = Σ_k w_vk · per_cell[cell_vk]` for every hood. `out.len()` must equal
/// `hoods.len()`. Non-finite cell values propagate (NaN in, NaN out), so callers map "never
/// activated" to a finite sentinel first.
///
/// # Panics
/// If a hood references a cell outside `per_cell`, or the lengths of `out` and `hoods` differ.
pub fn map_hoods(hoods: &[Hood], per_cell: &[f32], out: &mut [f32]) {
    assert_eq!(hoods.len(), out.len(), "one scalar per hood");
    #[cfg(feature = "parallel")]
    out.par_chunks_mut(CHUNK)
        .zip(hoods.par_chunks(CHUNK))
        .for_each(|(o, h)| {
            for (o, h) in o.iter_mut().zip(h) {
                *o = h.interpolate(per_cell);
            }
        });
    // Same per-vertex arithmetic, so the result is identical with and without the feature.
    #[cfg(not(feature = "parallel"))]
    for (o, h) in out.iter_mut().zip(hoods) {
        *o = h.interpolate(per_cell);
    }
}

/// Edge smoothing of a per-vertex field: each pass moves every vertex halfway to the mean of
/// its edge neighbours (Jacobi, so the result does not depend on vertex order). Built once per
/// mesh from its triangles; isolated vertices keep their value.
#[derive(Debug, Clone)]
pub struct VertexSmoother {
    /// CSR offsets into `neighbours`, one more than the vertex count.
    offsets: Vec<u32>,
    neighbours: Vec<u32>,
}

impl VertexSmoother {
    /// The edge graph of `mesh`'s triangles.
    #[must_use]
    pub fn new(mesh: &TriMesh) -> Self {
        let n = mesh.vertex_count();
        let mut adjacency: Vec<Vec<u32>> = vec![Vec::new(); n];
        for &[a, b, c] in mesh.indices.as_chunks::<3>().0 {
            for (p, q) in [(a, b), (b, c), (c, a)] {
                adjacency[p as usize].push(q);
                adjacency[q as usize].push(p);
            }
        }
        let mut offsets = Vec::with_capacity(n + 1);
        let mut neighbours = Vec::new();
        offsets.push(0);
        for mut adj in adjacency {
            adj.sort_unstable();
            adj.dedup();
            neighbours.extend_from_slice(&adj);
            offsets.push(u32::try_from(neighbours.len()).expect("fewer than 4 G edges"));
        }
        Self { offsets, neighbours }
    }

    /// Applies `passes` smoothing passes to `field` in place; `scratch` is reused between calls.
    pub fn smooth(&self, field: &mut [f32], passes: usize, scratch: &mut Vec<f32>) {
        assert_eq!(field.len() + 1, self.offsets.len(), "one value per vertex");
        for _ in 0..passes {
            scratch.clear();
            scratch.extend_from_slice(field);
            for (i, out) in field.iter_mut().enumerate() {
                let (lo, hi) = (self.offsets[i] as usize, self.offsets[i + 1] as usize);
                if hi > lo {
                    let sum: f32 = self.neighbours[lo..hi].iter().map(|&j| scratch[j as usize]).sum();
                    *out = 0.5 * scratch[i] + 0.5 * sum / (hi - lo) as f32;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use heartsim_geometry::K_NEIGHBOURS;

    #[test]
    fn convex_weights_reproduce_constant_and_delta_fields() {
        let hoods: Vec<Hood> = (0..10_000u32)
            .map(|v| {
                let mut cells = [0u32; K_NEIGHBOURS];
                let mut weights = [0f32; K_NEIGHBOURS];
                for k in 0..K_NEIGHBOURS {
                    cells[k] = (v + k as u32) % 50;
                    weights[k] = 1.0 / K_NEIGHBOURS as f32;
                }
                Hood { cells, weights }
            })
            .collect();
        let mut out = vec![0.0; hoods.len()];
        map_hoods(&hoods, &[-85.0; 50], &mut out);
        assert!(out.iter().all(|s| (s + 85.0).abs() < 1e-4));
        let mut delta = vec![0.0; 50];
        delta[7] = 1.0;
        map_hoods(&hoods, &delta, &mut out);
        for (h, s) in hoods.iter().zip(&out) {
            let expect: f32 = h
                .cells
                .iter()
                .zip(h.weights)
                .filter(|(c, _)| **c == 7)
                .map(|(_, w)| w)
                .sum();
            assert!((s - expect).abs() < 1e-6);
        }
    }

    #[test]
    fn smoothing_keeps_a_constant_field_and_flattens_a_spike() {
        // two triangles sharing an edge: a square with vertices 0-1-2-3
        let mesh = TriMesh {
            vertices: vec![
                heartsim_geometry::Vertex {
                    position: [0.0; 3],
                    normal: [0.0; 3],
                };
                4
            ],
            indices: vec![0, 1, 2, 0, 2, 3],
        };
        let sm = VertexSmoother::new(&mesh);
        let mut scratch = Vec::new();
        let mut flat = vec![-85.0; 4];
        sm.smooth(&mut flat, 4, &mut scratch);
        assert!(flat.iter().all(|v| (v + 85.0).abs() < 1e-5), "{flat:?}");
        let mut spike = vec![0.0, 0.0, 0.0, 0.0];
        spike[0] = 1.0;
        sm.smooth(&mut spike, 1, &mut scratch);
        // vertex 0 has neighbours 1, 2, 3 (all 0) → halfway to 0; its neighbours rise
        assert!((spike[0] - 0.5).abs() < 1e-6, "{spike:?}");
        assert!(spike[1] > 0.0 && spike[2] > 0.0 && spike[3] > 0.0, "{spike:?}");
        assert!(spike.iter().all(|v| *v <= 0.5), "nothing overshoots: {spike:?}");
    }
}
