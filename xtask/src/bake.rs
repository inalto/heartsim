//! `cargo xtask bake-anatomy` — build an anatomy pack (`docs/anatomy-pack.md`).
//!
//! Two paths:
//! * **procedural** (default, no `--source`): `AnatomyPack::procedural` at the requested resolution.
//! * **real mesh** (`--source <dir>`): the open-licence four-chamber cohort path. The source
//!   inspection and plan are implemented; the geometry stages are scaffolded as named steps and
//!   report what is missing so the pipeline can be completed stage by stage (brief B2 §3).

use anyhow::{Context, Result, bail};
use heartsim_geometry::procedural::Resolution;
use heartsim_geometry::{AnatomyPack, K_NEIGHBOURS};
use std::path::{Path, PathBuf};

#[derive(clap::Args, Debug)]
pub struct BakeArgs {
    /// Output pack directory (created / overwritten).
    #[arg(long, default_value = "assets/anatomy/default")]
    pub out: PathBuf,
    /// Source directory of an open-licence heart mesh (Strocchi/Rodero cohort case). Omit for the
    /// procedural idealised heart.
    #[arg(long)]
    pub source: Option<PathBuf>,
    /// Pack name written to the manifest.
    #[arg(long, default_value = "default")]
    pub name: String,
    /// Render-mesh voxel size (procedural path).
    #[arg(long, default_value_t = Resolution::DEFAULT.mesh_voxel_mm)]
    pub voxel_mm: f32,
    /// Cell lattice spacing (procedural path).
    #[arg(long, default_value_t = Resolution::DEFAULT.cell_spacing_mm)]
    pub spacing_mm: f32,
}

pub fn run(args: &BakeArgs) -> Result<()> {
    match &args.source {
        None => bake_procedural(args),
        Some(src) => bake_from_source(src, args),
    }
}

fn bake_procedural(args: &BakeArgs) -> Result<()> {
    let t = std::time::Instant::now();
    let res = Resolution {
        mesh_voxel_mm: args.voxel_mm,
        cell_spacing_mm: args.spacing_mm,
    };
    let mut pack = AnatomyPack::procedural(res);
    pack.manifest.name.clone_from(&args.name);
    eprintln!(
        "procedural heart: {} cells, {} meshes, built in {:.2?}",
        pack.cells.len(),
        pack.meshes.len(),
        t.elapsed()
    );
    let manifest = pack
        .save(&args.out)
        .with_context(|| format!("writing pack to {}", args.out.display()))?;
    let total: u64 = manifest.files.values().map(|f| f.bytes).sum();
    for m in &manifest.meshes {
        eprintln!(
            "  {:<18} {:>7} vertices {:>7} triangles",
            m.name, m.vertices, m.triangles
        );
    }
    eprintln!(
        "wrote {} files, {:.1} MB, to {}",
        manifest.files.len(),
        total as f64 / 1e6,
        args.out.display()
    );

    let t = std::time::Instant::now();
    let loaded = AnatomyPack::load(&args.out).context("re-loading the written pack")?;
    eprintln!(
        "verified: pack loads in {:.2?} ({} cells, k = {K_NEIGHBOURS})",
        t.elapsed(),
        loaded.cells.len()
    );
    Ok(())
}

/// What we recognise in a source directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceFormat {
    /// openCARP text mesh: `<case>.pts` / `.elem` / `.lon` (Rodero 2021 cohort, Zenodo 4590294).
    Carp,
    /// Legacy VTK or XML unstructured grid (Strocchi 2020 fine meshes, Zenodo 3890034).
    Vtk,
    /// EnSight Gold `.case` (Strocchi 2020 coarse meshes).
    EnSight,
    /// glTF binary (any surface-only source).
    Glb,
}

fn detect_format(dir: &Path) -> Result<(SourceFormat, Vec<PathBuf>)> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .with_context(|| format!("reading {}", dir.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    files.sort();
    let has = |ext: &str| {
        files
            .iter()
            .any(|f| f.extension().is_some_and(|e| e.eq_ignore_ascii_case(ext)))
    };
    let fmt = if has("pts") && has("elem") {
        SourceFormat::Carp
    } else if has("vtk") || has("vtu") {
        SourceFormat::Vtk
    } else if has("case") {
        SourceFormat::EnSight
    } else if has("glb") || has("gltf") {
        SourceFormat::Glb
    } else {
        bail!(
            "{}: no recognised mesh (expected CARP .pts/.elem/.lon, VTK .vtk/.vtu, EnSight .case or glTF .glb). \
             See docs/anatomy-pack.md for the download steps.",
            dir.display()
        );
    };
    Ok((fmt, files))
}

/// The stages of the real-mesh pipeline, in order. Each returns `Ok(())` when done or an error
/// naming what is missing, so `bake-anatomy --source` always prints an accurate plan.
const STAGES: [(&str, &str); 9] = [
    (
        "read",
        "parse the volume mesh (nodes, tetrahedra, element tags, fibres if present)",
    ),
    (
        "frame",
        "re-centre on the ventricular centroid and rotate into the heart frame (mm; +x left, +y cranial, +z anterior)",
    ),
    (
        "surfaces",
        "extract tagged boundary surfaces per chamber and decimate to ~15 k vertices (quadric simplification)",
    ),
    (
        "cells",
        "sample ~2.5 mm cells inside the myocardial tags; UVC apex–base and transmural coordinates by Laplace solves (sprs CG)",
    ),
    (
        "fibres",
        "copy dataset fibres or run the Bayer LDRB rule (α 40°/−50°, β −65°/25°) from the Laplace fields",
    ),
    (
        "regions",
        "map dataset tags to heartsim_core::Region; AHA segments from UVC; coronary territories",
    ),
    (
        "conduction",
        "SA/AV node, His, bundle branches, fascicle landmarks; Costabal fractal tree on the endocardium → PMJ seeds",
    ),
    (
        "hoods",
        "8 nearest cells with inverse-distance weights per render vertex (heartsim_geometry::compute_hoods)",
    ),
    (
        "write",
        "AnatomyPack::save with the dataset's CC BY 4.0 attribution in LICENSE",
    ),
];

fn bake_from_source(src: &Path, args: &BakeArgs) -> Result<()> {
    let (fmt, files) = detect_format(src)?;
    eprintln!(
        "source {} → format {fmt:?}, {} files:",
        src.display(),
        files.len()
    );
    for f in files.iter().take(20) {
        let size = f.metadata().map_or(0, |m| m.len());
        eprintln!(
            "  {:<40} {:>10.1} MB",
            f.file_name().unwrap_or_default().to_string_lossy(),
            size as f64 / 1e6
        );
    }
    if files.len() > 20 {
        eprintln!("  … {} more", files.len() - 20);
    }
    eprintln!("\nplan for {} → {}:", src.display(), args.out.display());
    for (i, (stage, what)) in STAGES.iter().enumerate() {
        let status = match (fmt, *stage) {
            (SourceFormat::Glb, "read") => "ok   (heartsim_geometry::pack::read_glb)",
            (_, "hoods" | "write") => "ok   (shared with the procedural path)",
            _ => "todo",
        };
        eprintln!("  {:>2}. {:<11} {status:<46} {what}", i + 1, stage);
    }
    bail!(
        "real-mesh baking is scaffolded but not implemented for {fmt:?} sources yet; \
         run without --source for the procedural pack, see docs/anatomy-pack.md for the plan"
    )
}
