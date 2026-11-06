//! `heartsim-cli export-opencarp`: a scenario as an openCARP experiment directory.
//!
//! **Mesh.** One cube of six tetrahedra per tissue cell (`heartsim_geometry::bridge::tetra`); atrial and
//! ventricular nodes are never shared. **Tags.** One tag per (region, fast layer, APD band); `tags.csv`
//! lists them. **Stimuli.** The timing engine's events, routed to seed cells exactly as for Tier 1 and
//! Tier 2, become vertex electrodes at the event times.
//!
//! **Parameters are a first guess.** A conduction velocity needs calibrating on the mesh and model that
//! will run it, which only the solver can do (openCARP has a tutorial for it): see the README written
//! next to the files.

use super::niederer;
use super::{eikonal_for, load_anatomy, load_scenario, tissue_group};
use anyhow::{Context, Result, bail, ensure};
use glam::Vec3;
use heartsim_core::{Millis, Region};
use heartsim_geometry::AnatomyPack;
use heartsim_geometry::bridge::opencarp::{elem_text, lon_text, pts_text, vtx_text};
use heartsim_geometry::bridge::tetra::{CellInput, TetMesh, VoxelMesh, box_mesh, check_mesh, voxel_mesh};
use heartsim_geometry::bridge::vtk::vtu_text;
use heartsim_sim::engine::{Chamber, EventKind, Origin};
use heartsim_sim::tier2::calibrate::solve_apd;
use heartsim_sim::tier2::{AlievPanfilov, CellModel};
use heartsim_sim::{EikonalParams, Engine, TimingEngine};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Monodomain conductivity per unit diffusivity: β·C_m with β = 0.14 µm⁻¹ and C_m = 1 µF/cm²
/// (Niederer 2011), S/m per mm²/ms.
pub const SIGMA_PER_D: f64 = 1.4;
/// Intra- and extracellular conductivities of the Niederer benchmark, S/m.
pub const G_IL: f64 = 0.17;
pub const G_EL: f64 = 0.62;
pub const G_IT: f64 = 0.019;
pub const G_ET: f64 = 0.24;
/// Stimulus of the benchmark, 50 000 µA/cm³, as the transmembrane density openCARP's `crct.type = 0`
/// takes: divided by β = 1400 cm⁻¹, µA/cm².
pub const STIM_STRENGTH_UA_CM2: f64 = 50_000.0 / 1400.0;
/// Conduction velocities of TT06 tissue with the benchmark conductivities, along and across the fibres,
/// from the reference activation times (`assets/validation/niederer_slab_reference.csv`), m/s.
pub const TT06_CV_LONG_M_S: f64 = 18.5 / 28.0;
pub const TT06_CV_TRANS_M_S: f64 = 5.5 / 26.0;

/// Harmonic mean: the monodomain conductivity of an intra/extra pair.
#[must_use]
pub fn monodomain_sigma(g_i: f64, g_e: f64) -> f64 {
    g_i * g_e / (g_i + g_e)
}

/// The cell model the solver is told to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Ionic {
    /// ten Tusscher & Panfilov 2006 in the ventricles, Courtemanche 1998 in the atria. Needs a fine mesh.
    Tt06,
    /// Aliev & Panfilov 1996 everywhere: the same equations as the Tier-2 default, with its APD dial.
    Ap,
}

/// Settings of `export-opencarp`.
#[derive(Debug)]
pub struct ExportArgs {
    pub scenario: String,
    pub out: PathBuf,
    pub pack: Option<PathBuf>,
    pub spacing_mm: f32,
    pub ionic: Ionic,
    pub tend_ms: f64,
    pub stim_radius_mm: Option<f32>,
    pub apd_step_ms: f32,
    pub dx_mm: f32,
}

/// Conductivities and cell model of one element tag.
#[derive(Debug, Clone, PartialEq)]
struct TagParams {
    tag: u32,
    name: String,
    cv_long_m_s: f64,
    cv_trans_m_s: f64,
    apd_ms: f64,
    im: &'static str,
    im_param: String,
    elements: usize,
}

/// `g_i`, `g_e` along or across the fibres reaching monodomain conductivity `sigma_m`: the benchmark's
/// pair scaled, which keeps its intra/extra ratio.
fn scaled_pair(sigma_m: f64, g_i: f64, g_e: f64) -> (f64, f64) {
    let f = sigma_m / monodomain_sigma(g_i, g_e);
    (g_i * f, g_e * f)
}

/// Monodomain conductivities (long, trans) expected to give these velocities on a fine mesh.
fn conductivities(ionic: Ionic, cv_long: f64, cv_trans: f64) -> (f64, f64) {
    match ionic {
        Ionic::Ap => {
            // the bistable front speed: CV = c₁·√D, with c₁ the speed at D = 1 mm²/ms
            let c1 = f64::from(AlievPanfilov::default().analytic_cv(1.0).unwrap_or(0.39));
            let sigma = |cv: f64| SIGMA_PER_D * (cv / c1).powi(2);
            (sigma(cv_long), sigma(cv_trans))
        }
        // CV ∝ √σ around the benchmark's operating point; Courtemanche gets the same first guess
        Ionic::Tt06 => (
            monodomain_sigma(G_IL, G_EL) * (cv_long / TT06_CV_LONG_M_S).powi(2),
            monodomain_sigma(G_IT, G_ET) * (cv_trans / TT06_CV_TRANS_M_S).powi(2),
        ),
    }
}

fn write(dir: &Path, name: &str, contents: impl AsRef<[u8]>) -> Result<()> {
    let p = dir.join(name);
    std::fs::write(&p, contents).with_context(|| format!("writing {}", p.display()))
}

fn write_mesh(dir: &Path, base: &str, m: &TetMesh) -> Result<()> {
    write(dir, &format!("{base}.pts"), pts_text(&m.points_mm))?;
    write(dir, &format!("{base}.elem"), elem_text(m))?;
    write(dir, &format!("{base}.lon"), lon_text(&m.fibres))?;
    let points: Vec<[f64; 3]> = m.points_mm.iter().map(|p| p.as_dvec3().to_array()).collect();
    let tags: Vec<f64> = m.tags.iter().map(|t| f64::from(*t)).collect();
    write(
        dir,
        &format!("{base}_preview.vtu"),
        vtu_text(&points, &m.tets, &[], &[("tag", &tags)]),
    )
}

/// Tag per tissue cell and the table of tags: one per (region, fast layer, APD band, TT06 endo flag).
fn tag_cells(
    pack: &AnatomyPack,
    eik: &heartsim_sim::EikonalEngine,
    s: &heartsim_ecg::Tier0Scenario,
    groups: &[Option<u8>],
    a: &ExportArgs,
) -> (Vec<u32>, Vec<TagParams>) {
    let cells = &pack.cells;
    let params = EikonalParams::default();
    let layers = eik.fast_layers();
    let base_apd = eik.base_apd_ms();
    let step = a.apd_step_ms.max(1.0);
    let key_of = |i: usize| {
        let band = (base_apd[i] / step).round() as i32;
        let endo = a.ionic == Ionic::Tt06 && cells.regions[i].is_ventricular() && cells.depth[i] < 0.5;
        (cells.regions[i], layers[i] != 0, band, endo)
    };
    let mut keys: BTreeMap<(Region, bool, i32, bool), (u32, usize, f64)> = BTreeMap::new();
    for (i, g) in groups.iter().enumerate() {
        if g.is_some() {
            let e = keys.entry(key_of(i)).or_insert((0, 0, 0.0));
            e.1 += 1;
            e.2 += f64::from(base_apd[i]);
        }
    }
    let mut table = Vec::with_capacity(keys.len());
    let mut apd_cache: BTreeMap<i32, f32> = BTreeMap::new();
    for (next, (key, entry)) in keys.iter_mut().enumerate() {
        let (region, fast, band, endo) = *key;
        // tag 0 is what a solver assumes for an untagged element, so tags start at 1
        entry.0 = next as u32 + 1;
        let rp = s.scenario.parameters.region(region);
        let scale = if region.is_atrial() {
            params.atrial_cv_scale
        } else {
            1.0
        };
        let cv_long = if fast {
            params.purkinje_layer_cv_m_s
        } else {
            rp.cv_long_m_s * scale
        };
        let cv_trans = if fast {
            cv_long
        } else {
            cv_long * rp.anisotropy_ratio
        };
        let apd = entry.2 / entry.1 as f64;
        let (im, im_param) = match a.ionic {
            Ionic::Tt06 if region.is_atrial() => ("Courtemanche", String::new()),
            Ionic::Tt06 => (
                "tenTusscherPanfilov",
                format!("flags={}", if endo { "ENDO" } else { "EPI" }),
            ),
            Ionic::Ap => {
                // the recovery dial of `AlievPanfilov`, which multiplies ε₀ and μ₁ alike
                let r = *apd_cache.entry(band).or_insert_with(|| {
                    solve_apd(&AlievPanfilov::default(), band as f32 * step, 0.1)
                        .0
                        .recovery
                });
                ("AlievPanfilov", format!("epsilon*{r:.4},mu1*{r:.4}"))
            }
        };
        table.push(TagParams {
            tag: entry.0,
            name: format!(
                "{}{}{}",
                region.label(),
                if fast { " fast layer" } else { "" },
                if endo { " endo" } else { "" }
            ),
            cv_long_m_s: cv_long,
            cv_trans_m_s: cv_trans,
            apd_ms: apd,
            im,
            im_param,
            elements: entry.1 * 6,
        });
    }
    let tags = (0..cells.len())
        .map(|i| {
            if groups[i].is_some() {
                keys[&key_of(i)].0
            } else {
                0
            }
        })
        .collect();
    (tags, table)
}

/// Timing-engine stimuli of the first `tend_ms`: `(time, cell)`, in time order.
fn routed_stimuli(
    eik: &mut heartsim_sim::EikonalEngine,
    s: &heartsim_ecg::Tier0Scenario,
    tend_ms: f64,
) -> Result<Vec<(f64, u32)>> {
    let mut timing = TimingEngine::new(s.timing.clone());
    timing.set_shock_seed(s.scenario.seed);
    let mut timeline = s.scenario.timeline.clone();
    timeline.sort_by(|a, b| a.at.0.total_cmp(&b.at.0));
    let mut next = 0;
    let mut routed = Vec::new();
    while timing.now().0 < tend_ms {
        while let Some(a) = timeline.get(next)
            && a.at.0 <= timing.now().0
        {
            timing
                .apply(&a.action, a.at)
                .map_err(|e| anyhow::anyhow!("timeline action at {} ms: {e:?}", a.at.0))?;
            next += 1;
        }
        timing.step(Millis(1.0));
        let events: Vec<_> = timing
            .drain_events()
            .into_iter()
            // activity the tissue causes itself is the solver's job, not a stimulus
            .filter(|e| !(e.origin == Origin::Reentry && e.kind == EventKind::Automatic && e.chamber != Chamber::AvJunction))
            .collect();
        eik.route_events(&events, &mut routed);
    }
    routed.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    Ok(routed)
}

/// Electrodes: stimuli are binned to 1 ms, each bin one vertex electrode. Returns `(start_ms, nodes)`.
fn electrodes(
    routed: &[(f64, u32)],
    v: &VoxelMesh,
    pack: &AnatomyPack,
    groups: &[Option<u8>],
    radius_mm: f32,
    tend_ms: f64,
) -> Vec<(f64, Vec<u32>)> {
    let mut bins: BTreeMap<i64, Vec<u32>> = BTreeMap::new();
    for &(t, cell) in routed {
        if t >= tend_ms {
            continue;
        }
        if let Some(g) = groups[cell as usize] {
            let nodes = v.nodes_within(pack.cells.positions[cell as usize], radius_mm, g);
            bins.entry(t.floor() as i64).or_default().extend(nodes);
        }
    }
    bins.into_iter()
        .filter_map(|(t, mut nodes)| {
            nodes.sort_unstable();
            nodes.dedup();
            (!nodes.is_empty()).then_some((t as f64, nodes))
        })
        .collect()
}

fn par_regions(par: &mut String, table: &[TagParams], ionic: Ionic) {
    let _ = writeln!(
        par,
        "\n# one conductivity and one cell-model region per element tag (tags.csv)"
    );
    let _ = writeln!(par, "num_gregions = {}", table.len());
    for (i, t) in table.iter().enumerate() {
        let (sl, st) = conductivities(ionic, t.cv_long_m_s, t.cv_trans_m_s);
        let (il, el) = scaled_pair(sl, G_IL, G_EL);
        let (it, et) = scaled_pair(st, G_IT, G_ET);
        let _ = writeln!(par, "gregion[{i}].name = \"{}\"", t.name);
        let _ = writeln!(par, "gregion[{i}].ID = {}", t.tag);
        for (axis, gi, ge) in [("l", il, el), ("t", it, et), ("n", it, et)] {
            let _ = writeln!(par, "gregion[{i}].g_i{axis} = {gi:.6}");
            let _ = writeln!(par, "gregion[{i}].g_e{axis} = {ge:.6}");
        }
    }
    let _ = writeln!(par, "\nnum_imp_regions = {}", table.len());
    for (i, t) in table.iter().enumerate() {
        let _ = writeln!(par, "imp_region[{i}].name = \"{}\"", t.name);
        let _ = writeln!(par, "imp_region[{i}].ID = {}", t.tag);
        let _ = writeln!(par, "imp_region[{i}].im = {}", t.im);
        if !t.im_param.is_empty() {
            let _ = writeln!(par, "imp_region[{i}].im_param = \"{}\"", t.im_param);
        }
        let _ = writeln!(par, "imp_region[{i}].cellSurfVolRatio = 0.14");
    }
}

/// Parameters common to both kinds of experiment. `lat_threshold_mv` is where activation is timed.
fn par_header(par: &mut String, id: &str, dt_us: f64, tend_ms: f64, lat_threshold_mv: f64) {
    let _ = writeln!(
        par,
        "# Written by heartsim-cli export-opencarp. See README.md: the conductivities are a first guess."
    );
    let _ = writeln!(par, "simID = heartsim_{id}\nmeshname = heart\nbidomain = 0");
    let _ = writeln!(
        par,
        "# time step, microseconds\ndt = {dt_us}\ntend = {tend_ms}\nspacedt = 1.0\ntimedt = 10.0"
    );
    let _ = writeln!(par, "\n# first crossing of the threshold, rising, on V_m");
    let _ = writeln!(
        par,
        "num_LATs = 1\nlats[0].measurand = 0\nlats[0].all = 0\nlats[0].mode = 0\nlats[0].method = 1"
    );
    let _ = writeln!(par, "lats[0].threshold = {lat_threshold_mv}");
}

fn carputils_script(id: &str, tend_ms: f64, stimuli: usize) -> String {
    format!(
        r#"#!/usr/bin/env python3
"""Run this heartsim export with openCARP through carputils: python3 run.py --np 4

Written by heartsim-cli export-opencarp. openCARP's own licence applies to running it; see README.md.
"""
import os
from datetime import date

from carputils import tools

HERE = os.path.dirname(os.path.abspath(__file__))
STIMULI = {stimuli}


def parser():
    parser = tools.standard_parser()
    group = parser.add_argument_group('heartsim experiment options')
    group.add_argument('--duration', type=float, default={tend_ms:?},
                       help='simulated time, ms (default {tend_ms:?})')
    return parser


def jobID(args):
    return '{{}}_heartsim_{id}'.format(date.today().isoformat())


@tools.carpexample(parser, jobID)
def run(args, job):
    cmd = tools.carp_cmd(os.path.join(HERE, 'parameters.par'))
    cmd += ['-simID', job.ID,
            '-meshname', os.path.join(HERE, 'heart'),
            '-tend', args.duration]
    # vertex electrodes are found relative to the working directory, so name them in full
    for i in range(STIMULI):
        cmd += ['-stim[{{}}].elec.vtx_file'.format(i),
                os.path.join(HERE, 'stim_{{:04d}}'.format(i))]
    job.carp(cmd)


if __name__ == '__main__':
    run()
"#
    )
}

fn readme(id: &str, ionic: Ionic, spacing_mm: f32, summary: &str) -> String {
    let warning = match ionic {
        Ionic::Tt06 if spacing_mm > 0.5 => format!(
            "\n**This mesh is too coarse for TT06.** Its {spacing_mm} mm spacing cannot resolve a TT06 wavefront, which needs about \
             0.1-0.2 mm; expect slow conduction or none. Use `--ionic ap` here, or a finer mesh.\n"
        ),
        _ => String::new(),
    };
    format!(
        "# heartsim export `{id}`\n\n{summary}\n{warning}\n\
## Files\n\n\
| file | contents |\n|---|---|\n\
| `heart.pts` | nodes, micrometres |\n\
| `heart.elem` | tetrahedra with region tags |\n\
| `heart.lon` | fibre direction per element |\n\
| `heart_preview.vtu` | the same mesh for ParaView, millimetres |\n\
| `stim_*.vtx` | vertex electrodes, if any |\n\
| `parameters.par` | openCARP parameter file |\n\
| `run.py` | carputils experiment script |\n\
| `tags.csv` | what each element tag means, with its target CV and APD |\n\
| `stimuli.csv` | electrode times and sizes, if any |\n\
| `mesh_summary.json` | node, element and cell counts, if the source is a cell cloud |\n\n\
## Checking the mesh (manual step, not run by heartsim)\n\n\
heartsim checks indices, orientation, conformity and fibres itself and refuses to write a mesh that fails. To confirm \
with openCARP's own tools:\n\n\
```bash\nmeshtool query quality -msh=heart\nmeshtool query tags -msh=heart\nmeshtool convert -imsh=heart -ifmt=carp_txt -omsh=heart_check -ofmt=vtk\n```\n\n\
## Running it\n\n\
```bash\npython3 run.py --np 4\n```\n\n\
Then read the results back and compare:\n\n\
```bash\nheartsim-cli import-vtk <output-dir> --mesh heart --out imported\n```\n\n\
## What the parameters are\n\n\
Conductivities are a **first guess**. They come from the target velocities in `tags.csv` by CV ∝ √σ, but the real \
velocity depends on the mesh, the cell model and the time step, so calibrate them on this mesh before comparing \
velocities (openCARP's conduction-velocity tutorial). The cell model differs from heartsim's unless `--ionic ap` is used. \
With `--ionic tt06` the target APDs in `tags.csv` are recorded but **not** applied: heartsim has no validated mapping \
from an APD to TT06 channel scalings.\n\n\
## Licence\n\n\
openCARP's academic public licence applies to running it, and commercial use needs a separate agreement. heartsim only \
writes and reads files; it contains no openCARP code.\n"
    )
}

/// Run `export-opencarp`.
pub fn run(a: &ExportArgs) -> Result<()> {
    std::fs::create_dir_all(&a.out).with_context(|| format!("creating {}", a.out.display()))?;
    if a.scenario == super::NIEDERER_SCENARIO {
        return export_slab(a);
    }
    ensure!(a.tend_ms > 0.0, "--tend-ms must be positive");
    let s = load_scenario(&a.scenario)?;
    let pack = load_anatomy(a.pack.as_deref(), a.spacing_mm)?;
    let spacing = if a.pack.is_some() {
        pack.manifest.cell_spacing_mm
    } else {
        a.spacing_mm
    };
    let mut eik = eikonal_for(&pack, &s);
    let kinds = eik.kinds().to_vec();
    let groups: Vec<Option<u8>> = (0..pack.cells.len())
        .map(|i| {
            let rp = s.scenario.parameters.region(pack.cells.regions[i]);
            tissue_group(kinds[i], rp.conducts && rp.cv_long_m_s > 0.0)
        })
        .collect();
    let (tags, mut table) = tag_cells(&pack, &eik, &s, &groups, a);
    let v = voxel_mesh(
        CellInput {
            positions: &pack.cells.positions,
            fibres: &pack.cells.fibres,
            tags: &tags,
            groups: &groups,
        },
        spacing,
    );
    // merged cells lost their cube: count the elements that really carry each tag
    let check = check_mesh(&v.mesh);
    if !check.is_valid() {
        bail!("generated mesh failed its check: {check:?}");
    }
    for t in &mut table {
        t.elements = check.tag_counts.get(&t.tag).copied().unwrap_or(0);
    }
    table.retain(|t| t.elements > 0);
    let routed = routed_stimuli(&mut eik, &s, a.tend_ms)?;
    let radius = a.stim_radius_mm.unwrap_or(1.5 * spacing);
    let stimuli = electrodes(&routed, &v, &pack, &groups, radius, a.tend_ms);

    let id = s.scenario.id.clone();
    write_mesh(&a.out, "heart", &v.mesh)?;
    let threshold = if a.ionic == Ionic::Ap { -30.0 } else { 0.0 };
    let mut par = String::new();
    par_header(
        &mut par,
        &id,
        if a.ionic == Ionic::Ap { 50.0 } else { 10.0 },
        a.tend_ms,
        threshold,
    );
    par_regions(&mut par, &table, a.ionic);
    let _ = writeln!(
        par,
        "\n# timing-engine events as vertex electrodes; 50000 uA/cm3 over beta = 1400 /cm"
    );
    let _ = writeln!(par, "num_stim = {}", stimuli.len());
    let mut stim_csv = String::from("electrode,start_ms,duration_ms,nodes\n");
    for (i, (t, nodes)) in stimuli.iter().enumerate() {
        let name = format!("stim_{i:04}");
        write(&a.out, &format!("{name}.vtx"), vtx_text(nodes))?;
        let _ = writeln!(par, "stim[{i}].crct.type = 0");
        let _ = writeln!(par, "stim[{i}].pulse.strength = {STIM_STRENGTH_UA_CM2:.2}");
        let _ = writeln!(par, "stim[{i}].ptcl.start = {t}");
        let _ = writeln!(par, "stim[{i}].ptcl.duration = 2.0");
        let _ = writeln!(par, "stim[{i}].elec.vtx_file = {name}");
        let _ = writeln!(stim_csv, "{name},{t},2,{}", nodes.len());
    }
    write(&a.out, "parameters.par", par)?;
    write(&a.out, "stimuli.csv", stim_csv)?;
    write(&a.out, "run.py", carputils_script(&id, a.tend_ms, stimuli.len()))?;
    let mut tag_csv = String::from(
        "tag,name,elements,target_cv_long_m_s,target_cv_trans_m_s,target_apd_ms,model,model_params\n",
    );
    for t in &table {
        let _ = writeln!(
            tag_csv,
            "{},{},{},{:.3},{:.3},{:.1},{},{}",
            t.tag,
            t.name,
            t.elements,
            t.cv_long_m_s,
            t.cv_trans_m_s,
            t.apd_ms,
            t.im,
            t.im_param.replace(',', ";")
        );
    }
    write(&a.out, "tags.csv", tag_csv)?;
    let summary = format!(
        "Scenario `{id}`, {} ms. {} nodes, {} tetrahedra, {} tags, {} electrodes, lattice spacing {spacing} mm. \
         {} tissue cells have their own cube; {} fell into an occupied cube; {} conduction-system or non-conducting cells have \
         no elements, because the timing engine owns their delays and delivers them as the electrodes here. Mesh volume {:.0} mm³ in {} connected pieces.",
        a.tend_ms,
        check.nodes,
        check.elements,
        table.len(),
        stimuli.len(),
        v.cells_meshed,
        v.cells_merged,
        v.cells_excluded,
        check.volume_mm3,
        check.components
    );
    write(&a.out, "README.md", readme(&id, a.ionic, spacing, &summary))?;
    write(
        &a.out,
        "mesh_summary.json",
        serde_json::to_string_pretty(&serde_json::json!({
            "nodes": check.nodes,
            "elements": check.elements,
            "components": check.components,
            "boundary_faces": check.boundary_faces,
            "volume_mm3": check.volume_mm3,
            "cells_meshed": v.cells_meshed,
            "cells_merged": v.cells_merged,
            "cells_excluded": v.cells_excluded,
            "tags": table.len(),
            "electrodes": stimuli.len(),
        }))?,
    )?;
    eprintln!("{summary}");
    eprintln!("wrote {}", a.out.display());
    if a.ionic == Ionic::Tt06 && spacing > 0.5 {
        eprintln!(
            "warning: {spacing} mm cannot resolve a TT06 wavefront; use --ionic ap or a finer mesh (see README.md)"
        );
    }
    Ok(())
}

/// The benchmark itself, so that a solver can be run on it and compared with `niederer --external`.
fn export_slab(a: &ExportArgs) -> Result<()> {
    let dims = niederer::slab_nodes(a.dx_mm);
    let mesh = box_mesh(dims, a.dx_mm, Vec3::ZERO, Vec3::X, 1);
    let check = check_mesh(&mesh);
    if !check.is_valid() {
        bail!("generated mesh failed its check: {check:?}");
    }
    write_mesh(&a.out, "heart", &mesh)?;
    let dt_us = niederer::dt_for(a.dx_mm) * 1000.0;
    let mut par = String::new();
    par_header(&mut par, super::NIEDERER_SCENARIO, f64::from(dt_us), 100.0, 0.0);
    let _ = writeln!(
        par,
        "\nnum_gregions = 1\ngregion[0].name = \"slab\"\ngregion[0].ID = 1"
    );
    for (axis, gi, ge) in [("l", G_IL, G_EL), ("t", G_IT, G_ET), ("n", G_IT, G_ET)] {
        let _ = writeln!(par, "gregion[0].g_i{axis} = {gi}\ngregion[0].g_e{axis} = {ge}");
    }
    let _ = writeln!(
        par,
        "\nnum_imp_regions = 1\nimp_region[0].name = \"slab\"\nimp_region[0].ID = 1\nimp_region[0].im = tenTusscherPanfilov\n\
         imp_region[0].im_param = \"flags=EPI\"\nimp_region[0].cellSurfVolRatio = 0.14\nimp_region[0].volFrac = 1"
    );
    let corner_um = niederer::STIM_EDGE_MM * 1000.0;
    let _ = writeln!(
        par,
        "\n# the benchmark's stimulus: a block on the x = y = z = 0 corner, 50000 uA/cm3 over beta = 1400 /cm\n\
         num_stim = 1\nstim[0].crct.type = 0\nstim[0].pulse.strength = {STIM_STRENGTH_UA_CM2:.2}\nstim[0].ptcl.start = 0\n\
         stim[0].ptcl.duration = {}\nstim[0].elec.geom_type = 2\nstim[0].elec.p0[0] = 0\nstim[0].elec.p0[1] = 0\nstim[0].elec.p0[2] = 0\n\
         stim[0].elec.p1[0] = {corner_um}\nstim[0].elec.p1[1] = {corner_um}\nstim[0].elec.p1[2] = {corner_um}",
        niederer::STIM_MS
    );
    write(&a.out, "parameters.par", par)?;
    write(
        &a.out,
        "run.py",
        carputils_script(super::NIEDERER_SCENARIO, 100.0, 0),
    )?;
    write(
        &a.out,
        "tags.csv",
        format!(
            "tag,name,elements,model,model_params\n1,slab,{},tenTusscherPanfilov,flags=EPI\n",
            check.elements
        ),
    )?;
    let summary = format!(
        "The Niederer et al. 2011 slab: 20 x 7 x 3 mm, fibres along x, dx {} mm, dt {} us. {} nodes, {} tetrahedra.",
        a.dx_mm, dt_us, check.nodes, check.elements
    );
    write(
        &a.out,
        "README.md",
        readme(super::NIEDERER_SCENARIO, Ionic::Tt06, a.dx_mm, &summary),
    )?;
    eprintln!("{summary}");
    eprintln!("wrote {}", a.out.display());
    Ok(())
}
