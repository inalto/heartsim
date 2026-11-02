//! Cell state → per-vertex scalars through the anatomy pack's hoods (`docs/ARCHITECTURE.md` §5).
//!
//! The physics runs on the cell cloud; every render-mesh vertex owns a precomputed hood of its
//! nearest cells with weights summing to 1, so a vertex scalar is the convex combination
//! `s_v = Σ_k w_vk · x[cell_vk]`. This is the only path by which engine state reaches the 3-D
//! colour map, so the map can never disagree with the engine (the ECG is derived from the same
//! cell state).

use heartsim_geometry::Hood;
use rayon::prelude::*;

/// Vertices per rayon task: large enough to amortise scheduling, small enough to balance the
/// ~20 k-vertex chamber meshes over a handful of cores.
const CHUNK: usize = 4096;

/// Fill `out[v] = Σ_k w_vk · per_cell[cell_vk]` for every hood. `out.len()` must equal
/// `hoods.len()`. Non-finite cell values propagate (NaN in, NaN out), so callers map "never
/// activated" to a finite sentinel first.
///
/// # Panics
/// If a hood references a cell outside `per_cell`, or the lengths of `out` and `hoods` differ.
pub fn map_hoods(hoods: &[Hood], per_cell: &[f32], out: &mut [f32]) {
    assert_eq!(hoods.len(), out.len(), "one scalar per hood");
    out.par_chunks_mut(CHUNK)
        .zip(hoods.par_chunks(CHUNK))
        .for_each(|(o, h)| {
            for (o, h) in o.iter_mut().zip(h) {
                *o = h.interpolate(per_cell);
            }
        });
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
}
