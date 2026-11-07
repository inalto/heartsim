//! Core, dependency-light types shared by every heartsim crate.
//!
//! Design rules (see `docs/ARCHITECTURE.md`):
//! * no GPU, GUI or I/O dependencies — this crate must compile for `wasm32` and be usable from the CLI;
//! * every physical quantity carries its unit in the type name or the field name (`_ms`, `_mv`, `_mm`);
//! * everything that ends up in a scenario file derives `serde` and is versioned.

pub mod axis;
pub mod case;
pub mod lead;
pub mod params;
pub mod region;
pub mod scenario;
pub mod time;
pub mod tutorial;

pub use axis::{AxisClass, FrontalAxis};
pub use lead::{ELECTRODE_COUNT, Electrode, ElectrodePotentials, LEAD_COUNT, Lead, TwelveLead};
pub use params::{GlobalParams, Parameters, RegionParams};
pub use region::Region;
pub use scenario::{Action, EngineTier, ExpectedFindings, RhythmKind, Scenario, StimSite, TimedAction};
pub use time::{Millis, SimClock};
