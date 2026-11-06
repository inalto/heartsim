//! Validation bridge (brief B23, `docs/validation-bridge.md`): write heartsim scenarios as openCARP
//! experiments, read solver results back onto the cell cloud, and report how the two compare.
//! heartsim only writes and reads files here; it never runs a solver.

pub mod export;
pub mod import;
pub mod metrics;
pub mod niederer;
pub mod plots;
pub mod report;

use anyhow::{Context, Result};
use heartsim_ecg::Tier0Scenario;
use heartsim_geometry::AnatomyPack;
use heartsim_geometry::procedural::Resolution;
use heartsim_sim::{EikonalEngine, EikonalParams, tier1::CellKind};
use std::path::Path;
use std::sync::Arc;

/// Scenario id of the Niederer benchmark slab in `export-opencarp`.
pub const NIEDERER_SCENARIO: &str = "niederer-slab";

/// The anatomy of a run: a pack from disk, or the procedural heart at a lattice spacing.
pub fn load_anatomy(pack: Option<&Path>, spacing_mm: f32) -> Result<AnatomyPack> {
    match pack {
        Some(dir) => AnatomyPack::load(dir).with_context(|| format!("loading pack {}", dir.display())),
        None => Ok(AnatomyPack::procedural(Resolution {
            mesh_voxel_mm: spacing_mm.max(2.5),
            cell_spacing_mm: spacing_mm,
        })),
    }
}

/// A scenario from a file or the library.
pub fn load_scenario(arg: &str) -> Result<Tier0Scenario> {
    let path = crate::scenario_path(arg)?;
    let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    Tier0Scenario::from_ron(&text).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))
}

/// The eikonal engine of a scenario on a pack: it knows each cell's kind, fast layer and base APD and
/// routes timing events to seed cells, exactly as it does for Tier 1 and Tier 2.
pub fn eikonal_for(pack: &AnatomyPack, s: &Tier0Scenario) -> EikonalEngine {
    let mut e = EikonalEngine::with_seed_sets(
        Arc::new(pack.cells.clone()),
        &s.scenario.parameters,
        EikonalParams::default(),
        &pack.landmarks.sets,
    );
    e.set_his_purkinje(s.timing.his_purkinje.clone());
    e.set_accessory(
        s.timing
            .accessory
            .as_ref()
            .map(|ap| (ap.ventricular_side, ap.atrial_side)),
    );
    if let Some(cfg) = &s.timing.pacer {
        e.set_pacer_leads(cfg);
    }
    e
}

/// The electrical compartment of a cell (1 atrial, 2 ventricular), `None` for cells that are not tissue:
/// conduction-system cells, whose delays the timing engine owns, and regions that do not conduct.
#[must_use]
pub fn tissue_group(kind: CellKind, conducts: bool) -> Option<u8> {
    if !conducts {
        return None;
    }
    match kind {
        CellKind::Atrial | CellKind::SinusNode => Some(1),
        CellKind::Ventricular | CellKind::Purkinje => Some(2),
        CellKind::Driven => None,
    }
}
