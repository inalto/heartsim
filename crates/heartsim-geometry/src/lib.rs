//! Geometry for heartsim: render meshes, simulation cell clouds, neighbourhood graphs.
//!
//! Two resolutions coexist on purpose (the design every mature simulator converges on):
//! * [`TriMesh`] — what is drawn (tens of thousands of vertices per chamber);
//! * [`CellCloud`] — where the physics runs (a few thousand to ~50 k points) with a precomputed
//!   k-nearest-neighbour graph carrying distances and fibre alignment, and a vertex→cell "hood"
//!   so mesh vertices can be coloured from cell state.

pub mod cloud;
pub mod electrodes;
pub mod hood;
pub mod implicit;
pub mod mesh;
pub mod pack;
pub mod primitives;
pub mod procedural;
pub mod spatial;
pub mod territory;

pub use cloud::{CellCloud, K_NEIGHBOURS, Neighbour};
pub use electrodes::ElectrodeSet;
pub use hood::{Hood, compute_hoods};
pub use mesh::{TriMesh, Vertex};
pub use pack::{AnatomyPack, Landmarks, PACK_FORMAT_VERSION, PackError, PackManifest};
pub use procedural::{IdealisedHeart, MeshKind, NamedMesh, Resolution, idealised_heart};
pub use spatial::HashGrid;
pub use territory::Territory;
