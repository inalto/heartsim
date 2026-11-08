//! `cargo xtask bake-anatomy` — build an anatomy pack (`docs/anatomy-pack.md`).
//!
//! Two paths:
//! * **real mesh** (default): download the open-licence sources into the git-ignored
//!   `assets/anatomy-src/` (checksums verified), bake the Rodero 2021 average four-chamber heart
//!   and the KCL whole-torso CT skin into the pack, with their CC BY 4.0 attribution in `LICENSE`;
//! * **procedural** (`--procedural`): `AnatomyPack::procedural`, the idealised heart that was the
//!   default until 0.7 and remains the fast test fixture.

mod fetch;
mod heart;
mod md5;
mod surface;
mod tet;
mod torso;

use anyhow::{Context, Result};
use heartsim_geometry::pack::{FileEntry, HEART_FRAME, MeshEntry};
use heartsim_geometry::procedural::Resolution;
use heartsim_geometry::{
    AnatomyPack, K_NEIGHBOURS, Landmarks, PACK_FORMAT_VERSION, PackManifest, compute_hoods,
};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

#[derive(clap::Args, Debug)]
pub struct BakeArgs {
    /// Output pack directory (created / overwritten).
    #[arg(long, default_value = "assets/anatomy/default")]
    pub out: PathBuf,
    /// Bake the procedural idealised heart instead of the open meshes.
    #[arg(long)]
    pub procedural: bool,
    /// Where the downloaded sources live (git-ignored).
    #[arg(long, default_value = "assets/anatomy-src")]
    pub source_root: PathBuf,
    /// Never download: fail if a source archive is missing or corrupt.
    #[arg(long)]
    pub offline: bool,
    /// Pack name written to the manifest.
    #[arg(long, default_value = "default")]
    pub name: String,
    /// Render-mesh voxel size (procedural path).
    #[arg(long, default_value_t = Resolution::DEFAULT.mesh_voxel_mm)]
    pub voxel_mm: f32,
    /// Cell lattice spacing (both paths).
    #[arg(long, default_value_t = Resolution::DEFAULT.cell_spacing_mm)]
    pub spacing_mm: f32,
    /// Triangles per chamber surface after decimation.
    #[arg(long, default_value_t = 30_000)]
    pub chamber_triangles: usize,
    /// Triangles per vessel surface after decimation.
    #[arg(long, default_value_t = 6_000)]
    pub vessel_triangles: usize,
    /// Triangles of the body surface after decimation.
    #[arg(long, default_value_t = 12_000)]
    pub torso_triangles: usize,
}

/// Attribution the CC BY 4.0 licences of both sources require, written to the pack's `LICENSE`.
pub const REAL_LICENSE: &str = "heartsim default anatomy pack — baked from open, CC BY 4.0 meshes\n\
\n\
HEART\n\
Rodero C, Strocchi M, Marciniak M, Longobardi S, Whitaker J, O'Neill MD, Gillette K, Augustin C,\n\
Plank G, Vigmond EJ, Lamata P, Niederer SA. Virtual cohort of extreme and average four-chamber\n\
heart meshes from statistical shape model (v1.0.0). Zenodo, 2021. doi:10.5281/zenodo.4593739\n\
(file average.tar.gz). Licence: Creative Commons Attribution 4.0 International,\n\
https://creativecommons.org/licenses/by/4.0/\n\
Paper: Rodero C et al. Linking statistical shape models and simulated function in the healthy\n\
adult human heart. PLoS Comput Biol 2021;17(4):e1008851. doi:10.1371/journal.pcbi.1008851\n\
Changes by heartsim: the `average` mesh was re-oriented to the measured long-axis pose\n\
(Engblom 2005 / Kayyali 2025, Narumi 2022), re-centred on the ventricular centroid, its tagged\n\
boundary surfaces extracted, smoothed and decimated, its myocardium resampled to a 2.3 mm cell\n\
cloud with the dataset's fibres and universal ventricular coordinates; vessel tubes beyond the\n\
cropped inlets, an appendage body, the coronary sinus, the conduction system, atrial sub-regions,\n\
AHA segments and landmarks were added by rule (heartsim-geometry, MIT OR Apache-2.0).\n\
\n\
TORSO\n\
Qian S, Monaci S, Mendonca-Costa C, Campos F, Gemmell P, Zaidi S, Rajani R, Whitaker J,\n\
Rinaldi CA, Bishop MJ. Whole Torso Computational Models (v1.0). King's College London, Zenodo,\n\
2022. doi:10.5281/zenodo.7253863 (file KCL_torso1.zip). Licence: Creative Commons Attribution\n\
4.0 International, https://creativecommons.org/licenses/by/4.0/\n\
Changes by heartsim: the outer skin surface of torso 1 was extracted from the tetrahedral model,\n\
closed at the CT's cut planes, smoothed, decimated, scaled uniformly about the subject's own\n\
ventricular centroid to the heart's left-ventricular length and translated into the heart frame;\n\
the electrode sites were placed on it by rule; every other tag was discarded.\n\
\n\
The pack as a whole is distributed under CC BY 4.0 for the geometry derived from the two sources\n\
above; heartsim's own additions are MIT OR Apache-2.0.\n\
Nothing from any proprietary program or data set was used.\n";

pub fn run(args: &BakeArgs) -> Result<()> {
    if args.procedural {
        bake_procedural(args)
    } else {
        bake_real(args)
    }
}

fn report_and_verify(pack: &mut AnatomyPack, out: &Path) -> Result<()> {
    let manifest = pack
        .save(out)
        .with_context(|| format!("writing pack to {}", out.display()))?;
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
        out.display()
    );
    let t = std::time::Instant::now();
    let loaded = AnatomyPack::load(out).context("re-loading the written pack")?;
    eprintln!(
        "verified: pack loads in {:.2?} ({} cells, k = {K_NEIGHBOURS})",
        t.elapsed(),
        loaded.cells.len()
    );
    Ok(())
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
    report_and_verify(&mut pack, &args.out)
}

fn bake_real(args: &BakeArgs) -> Result<()> {
    let t0 = std::time::Instant::now();
    let heart_dir = fetch::ensure(&args.source_root, &fetch::RODERO_AVERAGE, args.offline)?;
    let torso_dir = fetch::ensure(&args.source_root, &fetch::KCL_TORSO1, args.offline)?;

    eprintln!(
        "reading {}",
        heart_dir.join(fetch::RODERO_AVERAGE.marker).display()
    );
    let t = std::time::Instant::now();
    let heart_mesh = tet::TetMesh::read_vtk(&heart_dir.join(fetch::RODERO_AVERAGE.marker))?;
    eprintln!(
        "  {} nodes, {} tetrahedra, {} fields, in {:.1?}",
        heart_mesh.points.len(),
        heart_mesh.tets.len(),
        heart_mesh.point_scalars.len(),
        t.elapsed()
    );
    let t = std::time::Instant::now();
    let heart = heart::bake_heart(
        &heart_mesh,
        heart::HeartOptions {
            cell_spacing_mm: args.spacing_mm,
            chamber_triangles: args.chamber_triangles,
            vessel_triangles: args.vessel_triangles,
        },
    )?;
    for line in &heart.report {
        eprintln!("  {line}");
    }
    eprintln!("heart baked in {:.1?}", t.elapsed());
    drop(heart_mesh);

    let torso_base = torso_dir.join("KCL_torso1").join("KCL_torso1");
    eprintln!("reading {}.pts/.elem", torso_base.display());
    let t = std::time::Instant::now();
    let torso_mesh = tet::TetMesh::read_carp(&torso_base, 1e-3)?;
    eprintln!(
        "  {} nodes, {} tetrahedra, in {:.1?}",
        torso_mesh.points.len(),
        torso_mesh.tets.len(),
        t.elapsed()
    );
    let t = std::time::Instant::now();
    let torso = torso::bake_torso(&torso_mesh, args.torso_triangles, heart.lv_length_mm)?;
    for line in &torso.report {
        eprintln!("  {line}");
    }
    eprintln!("torso baked in {:.1?}", t.elapsed());
    drop(torso_mesh);

    // nearest-cell distances per precordial lead, the numbers the forward model depends on
    {
        let mut line = String::from("nearest heart cell to V1…V6:");
        for name in ["V1", "V2", "V3", "V4", "V5", "V6"] {
            if let Some(e) = torso.electrodes.get(name) {
                let d = heart
                    .cells
                    .positions
                    .iter()
                    .map(|p| p.distance(e))
                    .fold(f32::INFINITY, f32::min);
                write!(line, " {d:.0}").expect("string write");
            }
        }
        eprintln!("  {line} mm");
    }

    let hoods: BTreeMap<String, _> = heart
        .meshes
        .iter()
        .filter(|m| m.conducting())
        .map(|m| (m.name.clone(), compute_hoods(&m.mesh, &heart.cells)))
        .collect();
    let landmarks = Landmarks {
        points: heart.cells.landmarks.clone(),
        sets: heart.pmj_seeds.clone(),
    };
    let manifest = PackManifest {
        format_version: PACK_FORMAT_VERSION,
        name: args.name.clone(),
        source: format!(
            "Rodero et al. 2021 average four-chamber heart (Zenodo 4593739) + KCL whole-torso CT model 1 \
             (Zenodo 7253863), baked by heartsim xtask {} (spacing {} mm)",
            env!("CARGO_PKG_VERSION"),
            args.spacing_mm
        ),
        licence: "CC-BY-4.0 (sources) AND (MIT OR Apache-2.0) (heartsim additions)".to_string(),
        frame: HEART_FRAME.to_string(),
        cell_spacing_mm: args.spacing_mm,
        cell_count: heart.cells.len() as u32,
        meshes: heart
            .meshes
            .iter()
            .map(|m| MeshEntry {
                name: m.name.clone(),
                file: format!("mesh_{}.glb", m.name),
                kind: m.kind,
                vertices: m.mesh.vertex_count() as u32,
                triangles: m.mesh.triangle_count() as u32,
            })
            .collect(),
        files: BTreeMap::<String, FileEntry>::new(),
    };
    let mut pack = AnatomyPack {
        manifest,
        meshes: heart.meshes,
        cells: heart.cells,
        hoods,
        landmarks,
        electrodes: torso.electrodes,
        torso: torso.mesh,
        aha_segments: heart.aha_segments,
        licence: REAL_LICENSE.to_string(),
    };
    eprintln!("total bake time {:.1?}", t0.elapsed());
    report_and_verify(&mut pack, &args.out)
}
