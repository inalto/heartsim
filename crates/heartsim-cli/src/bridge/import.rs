//! `heartsim-cli import-vtk`: solver results → the cell cloud, by nearest-point mapping.
//!
//! Accepted in the results directory: a series of `.vtu` or legacy `.vtk` files (with a `.pvd` index, or
//! `--frame-dt-ms`), and openCARP's `.igb` V_m series and activation `.dat`, which need the mesh's `.pts`
//! for their node positions. Fields per node and per cell both work (MonoAlg3D writes per cell).

use super::load_anatomy;
use anyhow::{Context, Result, bail, ensure};
use glam::Vec3;
use heartsim_geometry::bridge::igb::{igb_bytes, parse_igb};
use heartsim_geometry::bridge::nearest_map;
use heartsim_geometry::bridge::opencarp::{parse_activation_dat, parse_pts};
use heartsim_geometry::bridge::vtk::{VtkField, VtkGrid, parse_legacy_vtk, parse_pvd, parse_vtu};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Array names taken for V_m and activation time unless `--vm-array` / `--lat-array` say otherwise.
const VM_NAMES: [&str; 6] = [
    "vm",
    "v",
    "v_m",
    "transmembrane",
    "transmembrane_potential",
    "scalars_",
];
const LAT_NAMES: [&str; 8] = [
    "lat",
    "lats",
    "activation",
    "activation_time",
    "activationtime",
    "act",
    "t_act",
    "vm_act",
];

/// The unit of VTK point coordinates. `.pts` files are always micrometres.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum LengthUnit {
    /// The scale (mm, µm or cm) whose bounding box best matches the target.
    Auto,
    Mm,
    Um,
    Cm,
}

/// How to read a results directory.
#[derive(Debug, Clone)]
pub struct SourceOptions {
    /// Mesh base name, for `.igb` and `.dat` results (`<base>.pts`).
    pub mesh: Option<PathBuf>,
    pub vm_array: Option<String>,
    pub lat_array: Option<String>,
    /// Time between files of a series with no `.pvd`, ms.
    pub frame_dt_ms: Option<f64>,
    pub length_unit: LengthUnit,
    /// Threshold for timing activation from V_m when no activation array exists, in V_m's unit; by default
    /// 0 mV for a series in millivolts, 0.5 for a normalised one.
    pub lat_threshold: Option<f64>,
}

/// A V_m series at sample points.
#[derive(Debug, Clone)]
pub struct VmSeries {
    pub points_mm: Vec<Vec3>,
    pub times_ms: Vec<f64>,
    /// `times_ms.len()` × `points_mm.len()` values.
    pub data: Vec<f32>,
    pub units: String,
}

/// What a results directory holds.
#[derive(Debug, Clone, Default)]
pub struct SourceData {
    /// Activation time at sample points, ms (`NaN` = never).
    pub lat: Option<(Vec<Vec3>, Vec<f32>)>,
    pub vm: Option<VmSeries>,
    /// Whether the activation time was computed here from V_m rather than read.
    pub lat_from_vm: bool,
    pub length_scale_to_mm: f64,
    pub files: Vec<String>,
}

/// Files of a directory with this extension, in natural order: `V_it_9` before `V_it_10`.
fn files_with(dir: &Path, ext: &str) -> Result<Vec<PathBuf>> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .with_context(|| format!("reading {}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case(ext)))
        .collect();
    out.sort_by_key(|a| natural_key(a));
    Ok(out)
}

/// Sort key treating runs of digits as numbers.
fn natural_key(p: &Path) -> Vec<(u8, u64, String)> {
    let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
    let mut out = Vec::new();
    let mut rest = name.as_str();
    while !rest.is_empty() {
        let digits = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
        if digits > 0 {
            out.push((0, rest[..digits].parse().unwrap_or(u64::MAX), String::new()));
            rest = &rest[digits..];
        } else {
            let text = rest.find(|c: char| c.is_ascii_digit()).unwrap_or(rest.len());
            out.push((1, 0, rest[..text].to_string()));
            rest = &rest[text..];
        }
    }
    out
}

fn read_grid(path: &Path) -> Result<VtkGrid> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let is_xml = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("vtu"));
    let g = if is_xml {
        parse_vtu(&bytes)
    } else {
        parse_legacy_vtk(&bytes)
    };
    g.with_context(|| format!("parsing {}", path.display()))
}

fn pick<'a>(fields: &'a [VtkField], wanted: Option<&str>, names: &[&str]) -> Option<&'a VtkField> {
    let scalar = |f: &&VtkField| f.components == 1;
    match wanted {
        Some(w) => fields.iter().find(|f| f.name == w),
        None => names.iter().find_map(|n| {
            fields
                .iter()
                .filter(scalar)
                .find(|f| f.name.eq_ignore_ascii_case(n))
        }),
    }
}

/// A named field of a grid and the points it lives at (before scaling).
fn field_at(grid: &VtkGrid, wanted: Option<&str>, names: &[&str]) -> Option<(Vec<[f64; 3]>, Vec<f64>)> {
    if let Some(f) = pick(&grid.point_fields, wanted, names) {
        return Some((grid.points.clone(), f.data.clone()));
    }
    pick(&grid.cell_fields, wanted, names).map(|f| (grid.cell_centres(), f.data.clone()))
}

fn bbox_diagonal(points: impl Iterator<Item = Vec3>) -> f64 {
    let (lo, hi) = points.fold(
        (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
        |(l, h), p| (l.min(p), h.max(p)),
    );
    if lo.x.is_finite() {
        f64::from((hi - lo).length())
    } else {
        0.0
    }
}

fn length_scale(unit: LengthUnit, points: &[[f64; 3]], target_diagonal_mm: f64) -> f64 {
    match unit {
        LengthUnit::Mm => 1.0,
        LengthUnit::Um => 1e-3,
        LengthUnit::Cm => 10.0,
        LengthUnit::Auto => {
            let d = bbox_diagonal(
                points
                    .iter()
                    .map(|p| Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32)),
            );
            if d <= 0.0 || target_diagonal_mm <= 0.0 {
                return 1.0;
            }
            [1.0, 1e-3, 10.0]
                .into_iter()
                .min_by(|a: &f64, b| {
                    let miss = |s: f64| (d * s / target_diagonal_mm).ln().abs();
                    miss(*a).total_cmp(&miss(*b))
                })
                .unwrap_or(1.0)
        }
    }
}

fn scaled(points: &[[f64; 3]], s: f64) -> Vec<Vec3> {
    points
        .iter()
        .map(|p| Vec3::new((p[0] * s) as f32, (p[1] * s) as f32, (p[2] * s) as f32))
        .collect()
}

fn to_f32(v: &[f64]) -> Vec<f32> {
    v.iter().map(|x| *x as f32).collect()
}

/// Solvers mark an unactivated point with a negative time.
fn lat_values(v: &[f64]) -> Vec<f32> {
    v.iter()
        .map(|x| if *x < 0.0 { f32::NAN } else { *x as f32 })
        .collect()
}

fn load_vtk_series(
    dir: &Path,
    o: &SourceOptions,
    target_diagonal_mm: f64,
    out: &mut SourceData,
) -> Result<bool> {
    let pvd = files_with(dir, "pvd")?;
    let mut frames: Vec<(Option<f64>, PathBuf)> = if let Some(index) = pvd.first() {
        let text = std::fs::read_to_string(index).with_context(|| format!("reading {}", index.display()))?;
        parse_pvd(&text)
            .with_context(|| format!("parsing {}", index.display()))?
            .into_iter()
            .map(|(t, f)| (Some(t), dir.join(f)))
            .collect()
    } else {
        Vec::new()
    };
    if frames.is_empty() {
        let mut files = files_with(dir, "vtu")?;
        files.extend(files_with(dir, "vtk")?);
        frames = files.into_iter().map(|f| (None, f)).collect();
    }
    if frames.is_empty() {
        return Ok(false);
    }
    let names = |w: &[&str]| w.join(", ");
    let (mut vm_points, mut times, mut data) = (None, Vec::new(), Vec::new());
    for (k, (time, path)) in frames.iter().enumerate() {
        let grid = read_grid(path)?;
        out.files
            .push(path.file_name().unwrap_or_default().to_string_lossy().to_string());
        if out.length_scale_to_mm == 0.0 {
            out.length_scale_to_mm = length_scale(o.length_unit, &grid.points, target_diagonal_mm);
        }
        let s = out.length_scale_to_mm;
        if out.lat.is_none()
            && let Some((p, v)) = field_at(&grid, o.lat_array.as_deref(), &LAT_NAMES)
        {
            out.lat = Some((scaled(&p, s), lat_values(&v)));
        }
        if let Some((p, v)) = field_at(&grid, o.vm_array.as_deref(), &VM_NAMES) {
            let t = match time {
                Some(t) => *t,
                None if frames.len() == 1 => 0.0,
                None => {
                    let dt = o.frame_dt_ms.ok_or_else(|| {
                        anyhow::anyhow!(
                            "{} files and no .pvd: give the time between them with --frame-dt-ms",
                            frames.len()
                        )
                    })?;
                    k as f64 * dt
                }
            };
            let count = vm_points.get_or_insert_with(|| scaled(&p, s)).len();
            ensure!(
                v.len() == count,
                "{} has {} V_m values, the first file had {count}",
                path.display(),
                v.len()
            );
            times.push(t);
            data.extend(to_f32(&v));
        } else if out.lat.is_none() {
            bail!(
                "{}: no activation or V_m array. Point data {:?}, cell data {:?}. Looked for: {} and {}; or name them with --lat-array / --vm-array",
                path.display(),
                grid.point_fields.iter().map(|f| &f.name).collect::<Vec<_>>(),
                grid.cell_fields.iter().map(|f| &f.name).collect::<Vec<_>>(),
                names(&LAT_NAMES),
                names(&VM_NAMES)
            );
        }
    }
    if let Some(points_mm) = vm_points {
        let units = if data.iter().any(|v: &f32| v.abs() > 5.0) {
            "mV"
        } else {
            "1"
        };
        out.vm = Some(VmSeries {
            points_mm,
            times_ms: times,
            data,
            units: units.to_string(),
        });
    }
    Ok(true)
}

fn load_carp(dir: &Path, o: &SourceOptions, out: &mut SourceData) -> Result<bool> {
    let igb = files_with(dir, "igb")?;
    let vm_file = igb
        .iter()
        .find(|p| p.file_stem().is_some_and(|n| n.eq_ignore_ascii_case("vm")))
        .or(igb.first());
    let dats = files_with(dir, "dat")?;
    let dat_file = dats
        .iter()
        .find(|p| p.file_name().is_some_and(|n| n.to_string_lossy().contains("act")));
    if vm_file.is_none() && dat_file.is_none() {
        return Ok(false);
    }
    let pts_path = if let Some(base) = &o.mesh {
        let mut p = base.as_os_str().to_owned();
        p.push(".pts");
        PathBuf::from(p)
    } else {
        let found = files_with(dir, "pts")?;
        match found.as_slice() {
            [one] => one.clone(),
            [] => bail!(
                "{} holds openCARP results but no .pts: name the mesh with --mesh <base>",
                dir.display()
            ),
            _ => bail!(
                "{} holds several .pts files: choose one with --mesh <base>",
                dir.display()
            ),
        }
    };
    let text =
        std::fs::read_to_string(&pts_path).with_context(|| format!("reading {}", pts_path.display()))?;
    let points = parse_pts(&text).with_context(|| format!("parsing {}", pts_path.display()))?;
    out.length_scale_to_mm = 1e-3;
    if let Some(path) = dat_file {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let lat = parse_activation_dat(&text).with_context(|| format!("parsing {}", path.display()))?;
        ensure!(
            lat.len() == points.len(),
            "{} has {} values, the mesh has {} nodes",
            path.display(),
            lat.len(),
            points.len()
        );
        out.files
            .push(path.file_name().unwrap_or_default().to_string_lossy().to_string());
        out.lat = Some((points.clone(), lat));
    }
    if let Some(path) = vm_file {
        let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        let s = parse_igb(&bytes).with_context(|| format!("parsing {}", path.display()))?;
        ensure!(
            s.nodes == points.len(),
            "{} has {} nodes per frame, the mesh has {}",
            path.display(),
            s.nodes,
            points.len()
        );
        if s.frames < s.frames_announced {
            eprintln!(
                "warning: {} announces {} frames but holds {}: the run stopped early",
                path.display(),
                s.frames_announced,
                s.frames
            );
        }
        out.files
            .push(path.file_name().unwrap_or_default().to_string_lossy().to_string());
        out.vm = Some(VmSeries {
            points_mm: points,
            times_ms: (0..s.frames).map(|k| s.time_ms(k)).collect(),
            data: s.data,
            units: if s.units.is_empty() {
                "mV".to_string()
            } else {
                s.units
            },
        });
    }
    Ok(true)
}

/// First upward crossing of `threshold` per point, linearly interpolated (`NaN` = never).
#[must_use]
pub fn lat_from_series(vm: &VmSeries, threshold: f32) -> Vec<f32> {
    let n = vm.points_mm.len();
    let mut lat = vec![f32::NAN; n];
    for (k, w) in vm.times_ms.windows(2).enumerate() {
        for (i, t) in lat.iter_mut().enumerate() {
            let (a, b) = (vm.data[k * n + i], vm.data[(k + 1) * n + i]);
            if t.is_nan() && a < threshold && b >= threshold {
                *t = (w[0] + (w[1] - w[0]) * f64::from((threshold - a) / (b - a))) as f32;
            }
        }
    }
    lat
}

/// Read a results directory. `target_diagonal_mm` is the bounding-box diagonal of what it should cover,
/// for telling the unit of VTK coordinates.
pub fn load_source(dir: &Path, o: &SourceOptions, target_diagonal_mm: f64) -> Result<SourceData> {
    let mut out = SourceData::default();
    let vtk = load_vtk_series(dir, o, target_diagonal_mm, &mut out)?;
    let carp = !vtk && load_carp(dir, o, &mut out)?;
    ensure!(
        vtk || carp,
        "{} holds no .vtu, .vtk, .pvd, .igb or activation .dat file",
        dir.display()
    );
    if out.lat.is_none()
        && let Some(vm) = &out.vm
    {
        let threshold = o
            .lat_threshold
            .unwrap_or(if vm.units == "mV" { 0.0 } else { 0.5 }) as f32;
        out.lat = Some((vm.points_mm.clone(), lat_from_series(vm, threshold)));
        out.lat_from_vm = true;
    }
    Ok(out)
}

/// Summary of an import, `import.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportSummary {
    pub cells: usize,
    pub source_points: usize,
    /// Cells whose nearest sample is closer than the limit.
    pub cells_mapped: usize,
    pub max_distance_limit_mm: f64,
    pub mean_distance_mm: f64,
    pub max_distance_mm: f64,
    pub frames: usize,
    pub vm_units: Option<String>,
    pub length_scale_to_mm: f64,
    /// Whether activation times were computed from V_m rather than read from an array.
    pub lat_from_vm: bool,
    pub has_activation: bool,
    pub files: Vec<String>,
}

/// Settings of `import-vtk`.
#[derive(Debug)]
pub struct ImportArgs {
    pub dir: PathBuf,
    pub out: Option<PathBuf>,
    pub pack: Option<PathBuf>,
    pub spacing_mm: f32,
    pub max_distance_mm: Option<f32>,
    pub source: SourceOptions,
}

/// Run `import-vtk`: writes `cells_activation.csv`, `cells_vm.igb` and `import.json`.
pub fn run(a: &ImportArgs) -> Result<()> {
    let pack = load_anatomy(a.pack.as_deref(), a.spacing_mm)?;
    let spacing = if a.pack.is_some() {
        pack.manifest.cell_spacing_mm
    } else {
        a.spacing_mm
    };
    let cells = &pack.cells;
    let diagonal = bbox_diagonal(cells.positions.iter().copied());
    let source = load_source(&a.dir, &a.source, diagonal)?;
    let limit = a.max_distance_mm.unwrap_or(1.5 * spacing);
    let out = a.out.clone().unwrap_or_else(|| a.dir.join("heartsim_import"));
    std::fs::create_dir_all(&out).with_context(|| format!("creating {}", out.display()))?;

    // activation
    let mut lat_cells = vec![f32::NAN; cells.len()];
    let mut stats = Vec::new();
    let mut source_points = 0;
    let mut csv = String::from("cell,x_mm,y_mm,z_mm,region,lat_ms,source_point,distance_mm\n");
    if let Some((points, lat)) = &source.lat {
        source_points = points.len();
        let map = nearest_map(points, &cells.positions);
        for (i, m) in map.iter().enumerate() {
            let p = cells.positions[i];
            let inside = m.distance_mm <= limit;
            stats.push(m.distance_mm);
            if inside {
                lat_cells[i] = lat[m.source as usize];
            }
            let t = if lat_cells[i].is_finite() {
                format!("{:.4}", lat_cells[i])
            } else {
                String::new()
            };
            let _ = writeln!(
                csv,
                "{i},{:.3},{:.3},{:.3},{},{t},{},{:.4}",
                p.x,
                p.y,
                p.z,
                cells.regions[i].label().replace(',', ";"),
                m.source,
                m.distance_mm
            );
        }
        std::fs::write(out.join("cells_activation.csv"), csv).context("writing cells_activation.csv")?;
    }

    // V_m series
    let (mut frames, mut vm_units) = (0, None);
    if let Some(vm) = &source.vm {
        source_points = source_points.max(vm.points_mm.len());
        let map = nearest_map(&vm.points_mm, &cells.positions);
        let n = vm.points_mm.len();
        frames = vm.times_ms.len();
        let mut data = Vec::with_capacity(frames * cells.len());
        for k in 0..frames {
            for m in &map {
                data.push(if m.distance_mm <= limit {
                    vm.data[k * n + m.source as usize]
                } else {
                    f32::NAN
                });
            }
        }
        if stats.is_empty() {
            stats.extend(map.iter().map(|m| m.distance_mm));
        }
        let t0 = vm.times_ms.first().copied().unwrap_or(0.0);
        let dt = match vm.times_ms.as_slice() {
            [a, b, ..] => b - a,
            _ => 1.0,
        };
        let uniform = vm
            .times_ms
            .windows(2)
            .all(|w| ((w[1] - w[0]) - dt).abs() < 1e-6 * dt.abs().max(1.0));
        ensure!(
            uniform,
            "the V_m frames are not evenly spaced in time, which this import does not support"
        );
        std::fs::write(
            out.join("cells_vm.igb"),
            igb_bytes(cells.len(), &data, t0, dt, &vm.units),
        )
        .context("writing cells_vm.igb")?;
        vm_units = Some(vm.units.clone());
    }

    let mapped = stats.iter().filter(|d| **d <= limit).count();
    let summary = ImportSummary {
        cells: cells.len(),
        source_points,
        cells_mapped: mapped,
        max_distance_limit_mm: f64::from(limit),
        mean_distance_mm: stats.iter().map(|d| f64::from(*d)).sum::<f64>() / stats.len().max(1) as f64,
        max_distance_mm: stats.iter().map(|d| f64::from(*d)).fold(0.0, f64::max),
        frames,
        vm_units,
        length_scale_to_mm: source.length_scale_to_mm,
        lat_from_vm: source.lat_from_vm,
        has_activation: source.lat.is_some(),
        files: source.files,
    };
    std::fs::write(out.join("import.json"), serde_json::to_string_pretty(&summary)?)
        .context("writing import.json")?;
    eprintln!(
        "{} of {} cells mapped from {} samples (mean distance {:.3} mm, max {:.3} mm, limit {limit} mm); {frames} V_m frames; wrote {}",
        summary.cells_mapped,
        summary.cells,
        summary.source_points,
        summary.mean_distance_mm,
        summary.max_distance_mm,
        out.display()
    );
    Ok(())
}

/// An import read back: activation per cell, V_m frames per cell.
#[derive(Debug)]
pub struct Imported {
    pub summary: ImportSummary,
    pub lat_ms: Vec<f32>,
    pub vm: Option<heartsim_geometry::bridge::igb::IgbSeries>,
}

/// Read the output of [`run`].
pub fn read_import(dir: &Path) -> Result<Imported> {
    let path = dir.join("import.json");
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("reading {} (run import-vtk first)", path.display()))?;
    let summary: ImportSummary =
        serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    let mut lat_ms = vec![f32::NAN; summary.cells];
    if summary.has_activation {
        let mut r = csv::Reader::from_path(dir.join("cells_activation.csv"))
            .context("reading cells_activation.csv")?;
        for rec in r.records() {
            let rec = rec?;
            let i: usize = rec.get(0).unwrap_or_default().parse()?;
            if let Some(t) = rec.get(5).filter(|t| !t.is_empty()) {
                ensure!(
                    i < lat_ms.len(),
                    "cell {i} is past the {} cells of the import",
                    lat_ms.len()
                );
                lat_ms[i] = t.parse()?;
            }
        }
    }
    let vm = if summary.frames > 0 {
        let bytes = std::fs::read(dir.join("cells_vm.igb")).context("reading cells_vm.igb")?;
        Some(parse_igb(&bytes).context("parsing cells_vm.igb")?)
    } else {
        None
    };
    Ok(Imported { summary, lat_ms, vm })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_sort_in_natural_order() {
        let mut v = [
            PathBuf::from("V_it_10.vtu"),
            PathBuf::from("V_it_9.vtu"),
            PathBuf::from("V_it_100.vtu"),
        ];
        v.sort_by_key(|a| natural_key(a));
        assert_eq!(v[0], PathBuf::from("V_it_9.vtu"));
        assert_eq!(v[2], PathBuf::from("V_it_100.vtu"));
    }

    #[test]
    fn activation_interpolates_the_first_upward_crossing() {
        let vm = VmSeries {
            points_mm: vec![Vec3::ZERO, Vec3::X],
            times_ms: vec![0.0, 10.0, 20.0, 30.0],
            // frame by frame: point 0 rises to 20 by 10 ms; point 1 is at -10 at 20 ms and 20 at 30 ms
            data: vec![-80.0, -80.0, 20.0, -80.0, -10.0, -10.0, 20.0, 20.0],
            units: "mV".to_string(),
        };
        let lat = lat_from_series(&vm, 0.0);
        assert!((lat[0] - 8.0).abs() < 1e-4, "{}", lat[0]);
        assert!((lat[1] - (20.0 + 10.0 / 3.0)).abs() < 1e-4, "{}", lat[1]);
    }

    #[test]
    fn the_unit_is_told_from_the_size_of_the_mesh() {
        let um = [[0.0, 0.0, 0.0], [100_000.0, 0.0, 0.0]];
        assert_eq!(length_scale(LengthUnit::Auto, &um, 120.0), 1e-3);
        let mm = [[0.0, 0.0, 0.0], [100.0, 0.0, 0.0]];
        assert_eq!(length_scale(LengthUnit::Auto, &mm, 120.0), 1.0);
        let cm = [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0]];
        assert_eq!(length_scale(LengthUnit::Auto, &cm, 120.0), 10.0);
        assert_eq!(length_scale(LengthUnit::Um, &mm, 120.0), 1e-3);
    }
}
