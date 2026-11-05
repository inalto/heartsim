//! Validation bridge (brief B23): file formats and meshing that connect heartsim to research-grade
//! monodomain solvers without re-implementing them. heartsim only writes and reads files here;
//! running openCARP or MonoAlg3D is the user's step, under those programs' own licences.
//!
//! * [`tetra`] — conforming tetrahedral meshes: one voxel per cell of a cell cloud, or a regular box.
//! * [`opencarp`] — the openCARP mesh, vertex and activation-time text formats.
//! * [`igb`] — the IGB binary series format openCARP writes V_m in.
//! * [`vtk`] — VTU (XML) and legacy VTK unstructured grids, the output of MonoAlg3D and of `meshtool`.
//! * [`nearest_map`] — nearest-point mapping of solver samples onto a cell cloud.
//!
//! **Units.** Heart coordinates are millimetres everywhere in heartsim; openCARP's `.pts` files are
//! micrometres, and the conversion happens only inside [`opencarp`].

pub mod igb;
pub mod opencarp;
pub mod tetra;
pub mod vtk;

use crate::spatial::HashGrid;
use glam::Vec3;

/// Micrometres per millimetre: `.pts` coordinate = heart coordinate × this.
pub const UM_PER_MM: f64 = 1000.0;

/// A malformed or unsupported solver file. Messages name the format and what was found.
#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("{format}: {detail}")]
    Format { format: &'static str, detail: String },
}

impl BridgeError {
    pub(crate) fn new(format: &'static str, detail: impl Into<String>) -> Self {
        Self::Format {
            format,
            detail: detail.into(),
        }
    }
}

/// Where one target point takes its value from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NearestSample {
    /// Index of the nearest source point.
    pub source: u32,
    /// Distance to it, mm.
    pub distance_mm: f32,
}

/// For every target (cell), the nearest source point (solver node or element centre); ties go to the
/// lower source index, so the mapping is deterministic. Empty when there are no sources.
#[must_use]
pub fn nearest_map(sources: &[Vec3], targets: &[Vec3]) -> Vec<NearestSample> {
    if sources.is_empty() {
        return Vec::new();
    }
    let grid = HashGrid::with_auto_cell(sources);
    targets
        .iter()
        .map(|t| {
            let (distance_mm, source) = grid.nearest(*t).unwrap_or((f32::INFINITY, 0));
            NearestSample { source, distance_mm }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_map_finds_the_closest_source_and_its_distance() {
        let sources = [Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0)];
        let m = nearest_map(&sources, &[Vec3::new(9.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)]);
        assert_eq!(m[0].source, 1);
        assert_eq!(m[1].source, 0);
        assert!((m[0].distance_mm - 1.0).abs() < 1e-6);
    }
}
