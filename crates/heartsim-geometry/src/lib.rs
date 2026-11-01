//! Geometry for heartsim: render meshes, simulation cell clouds, neighbourhood graphs.
//!
//! Two resolutions coexist on purpose (the design every mature simulator converges on):
//! * [`TriMesh`] — what is drawn (tens of thousands of vertices per chamber);
//! * [`CellCloud`] — where the physics runs (a few thousand to ~50 k points) with a precomputed
//!   k-nearest-neighbour graph carrying distances and fibre alignment, and a vertex→cell "hood"
//!   so mesh vertices can be coloured from cell state.

pub mod cloud;
pub mod mesh;
pub mod primitives;

pub use cloud::{CellCloud, K_NEIGHBOURS, Neighbour};
pub use mesh::{TriMesh, Vertex};
