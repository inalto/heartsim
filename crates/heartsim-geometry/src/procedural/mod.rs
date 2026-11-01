//! Procedural anatomy: an idealised four-chamber heart and a generic torso, used as the default
//! anatomy pack until a real open-licence mesh is baked (brief B2 §3), and forever as the fast
//! deterministic fixture for tests and benchmarks.

pub mod heart;
pub mod torso;

pub use heart::{IDEALISED_REGIONS, IdealisedHeart, MeshKind, NamedMesh, Resolution, idealised_heart};
pub use torso::{Torso, procedural_electrodes, procedural_torso};
