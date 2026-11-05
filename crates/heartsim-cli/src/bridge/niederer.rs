//! `heartsim-cli niederer`: the Niederer et al. 2011 slab on the Tier-2 solver, against the reference
//! activation times in `assets/validation/niederer_slab_reference.csv` (read its header on where they
//! come from).
//!
//! **What this can and cannot show.** The benchmark's cell model is TT06; Tier 2 runs phenomenological
//! Aliev–Panfilov. With the benchmark's diffusivities Aliev–Panfilov conducts far too slowly, and the
//! report says by how much. To compare wavefronts at all, the speeds are calibrated: `CV_l` and `CV_t` are
//! measured from the reference's P1, P2 and P5, turned into diffusivities on a cable of the solver's own
//! `h` and `dt`, and the excitation dial is the one in a fixed list that best fits the `fit` points. The
//! `held_out` points take no part. So a small error at the held-out points says that the anisotropic front
//! geometry, corner stimulus and no-flux boundaries behave; it is **not** a verification of the
//! monodomain equations with TT06, which is what the benchmark is for.

use super::export::{SIGMA_PER_D, monodomain_sigma};
use super::import::{LengthUnit, SourceOptions, load_source};
use super::metrics::{Agreement, agreement, cell};
use super::plots::activation_scatter;
use anyhow::{Context, Result, bail, ensure};
use glam::Vec3;
use heartsim_geometry::bridge::nearest_map;
use heartsim_sim::tier2::calibrate::{solve_d, trace_budget};
use heartsim_sim::tier2::{AlievPanfilov, CellModel, Monodomain, Tissue, VoxelGrid};
use serde::Serialize;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// The reference table, compiled in so that the runner works from anywhere.
pub const REFERENCE_CSV: &str = include_str!("../../../../assets/validation/niederer_slab_reference.csv");
/// Slab size, mm: x along the fibres.
pub const SLAB_MM: [f32; 3] = [20.0, 7.0, 3.0];
/// Edge of the stimulated corner cube, mm.
pub const STIM_EDGE_MM: f32 = 1.5;
/// Stimulus duration, ms.
pub const STIM_MS: f64 = 2.0;
/// Excitation dials tried by the calibration.
const EXCITATION_DIALS: [f32; 4] = [2.0, 4.0, 8.0, 16.0];
/// Fraction of the explicit diffusion budget the calibration may use.
const STABILITY: f32 = 0.9;
/// A calibrated run stops here if the slab is not fully activated (the reference ends at 43 ms).
const CALIBRATED_LIMIT_MS: f64 = 150.0;
/// The uncalibrated run gets longer, to see how slow it is.
const UNCALIBRATED_LIMIT_MS: f64 = 400.0;
/// Distance beyond which an external solver has no sample for a reference point, mm.
const EXTERNAL_MATCH_MM: f32 = 0.25;

/// Whether a reference point may influence the calibration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Fit,
    HeldOut,
}

/// One row of the reference table.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RefPoint {
    pub name: String,
    pub position_mm: [f32; 3],
    pub activation_ms: f64,
    pub role: Role,
}

/// Parse the reference table (`#` lines are comments).
pub fn parse_reference(text: &str) -> Result<Vec<RefPoint>> {
    let body: String = text
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    let mut r = csv::Reader::from_reader(body.as_bytes());
    let mut out = Vec::new();
    for row in r.records() {
        let row = row?;
        ensure!(row.len() == 6, "reference row {row:?} does not have 6 columns");
        let number = |i: usize| -> Result<f64> { Ok(row[i].trim().parse()?) };
        out.push(RefPoint {
            name: row[0].trim().to_string(),
            position_mm: [number(1)? as f32, number(2)? as f32, number(3)? as f32],
            activation_ms: number(4)?,
            role: match row[5].trim() {
                "fit" => Role::Fit,
                "held_out" => Role::HeldOut,
                other => bail!("unknown role `{other}`"),
            },
        });
    }
    ensure!(!out.is_empty(), "the reference table is empty");
    Ok(out)
}

/// Nodes per axis of the slab at spacing `dx_mm`.
#[must_use]
pub fn slab_nodes(dx_mm: f32) -> [usize; 3] {
    SLAB_MM.map(|l| (l / dx_mm).round() as usize + 1)
}

/// Time step paired with a spatial step: the benchmark's pairs where it has them (0.5 mm with 0.05 ms, 0.2 mm
/// with 0.005 ms); elsewhere 0.05 ms scaled with `dx²`, the explicit limit's scaling.
#[must_use]
pub fn dt_for(dx_mm: f32) -> f32 {
    if (dx_mm - 0.5).abs() < 1e-4 {
        0.05
    } else if (dx_mm - 0.2).abs() < 1e-4 {
        0.005
    } else {
        0.05 * (dx_mm / 0.5).powi(2)
    }
}

/// Activation at each reference point, ms (`NaN` = not by `limit_ms`).
fn run_slab(
    dx_mm: f32,
    dt_ms: f32,
    model: AlievPanfilov,
    d: (f32, f32),
    points: &[RefPoint],
    limit_ms: f64,
) -> Vec<f64> {
    let dims = slab_nodes(dx_mm);
    let grid = VoxelGrid::block(dims, dx_mm, Vec3::X);
    let mut solver = Monodomain::uniform(
        grid,
        Tissue {
            model,
            d_long: d.0,
            d_trans: d.1,
        },
        dt_ms,
    );
    let tolerance = dx_mm * 1e-3;
    let stim = solver.voxels_in_box(
        Vec3::splat(-tolerance),
        Vec3::splat(STIM_EDGE_MM + tolerance),
        None,
    );
    solver.stimulate(&stim, 0.0, STIM_MS, 1.0);
    let voxels: Vec<usize> = points
        .iter()
        .map(|p| {
            solver
                .grid()
                .nearest(Vec3::from(p.position_mm), dx_mm, None)
                .map_or(usize::MAX, |v| v as usize)
        })
        .collect();
    let mut t = 0.0;
    while t < limit_ms {
        t += 1.0;
        solver.advance_to(t);
        if voxels
            .iter()
            .all(|v| solver.activation_ms().get(*v).is_some_and(|a| a.is_finite()))
        {
            break;
        }
    }
    voxels
        .iter()
        .map(|v| solver.activation_ms().get(*v).map_or(f64::NAN, |a| f64::from(*a)))
        .collect()
}

fn point<'a>(points: &'a [RefPoint], name: &str) -> Result<&'a RefPoint> {
    points
        .iter()
        .find(|p| p.name == name)
        .ok_or_else(|| anyhow::anyhow!("the reference table has no point {name}"))
}

/// One calibrated variant of the benchmark, as reported in `niederer_metrics.json`.
#[derive(Debug, Clone, Serialize)]
pub struct Variant {
    pub dx_mm: f32,
    pub dt_ms: f32,
    pub voxels: usize,
    pub cv_long_target_m_s: f64,
    pub cv_trans_target_m_s: f64,
    pub cv_long_achieved_m_s: f64,
    pub cv_trans_achieved_m_s: f64,
    pub excitation: f32,
    pub d_long_mm2_ms: f32,
    pub d_trans_mm2_ms: f32,
    /// Dials tried, with RMS at the fit points; `None` when the targets were out of reach.
    pub dials_tried: Vec<(f32, Option<f64>)>,
    pub activation_ms: Vec<f64>,
    pub all: Agreement,
    pub fit: Agreement,
    pub held_out: Agreement,
    /// Benchmark diffusivities, unit excitation.
    pub uncalibrated_d_long_mm2_ms: f32,
    pub uncalibrated_d_trans_mm2_ms: f32,
    pub uncalibrated_activation_ms: Vec<f64>,
    pub uncalibrated_limit_ms: f64,
    pub uncalibrated: Agreement,
}

fn subset(points: &[RefPoint], sim: &[f64], role: Option<Role>) -> Agreement {
    let (mut a, mut b) = (Vec::new(), Vec::new());
    for (p, s) in points.iter().zip(sim) {
        if role.is_none_or(|r| r == p.role) {
            a.push(p.activation_ms);
            b.push(*s);
        }
    }
    agreement(&a, &b)
}

/// Run one spatial step.
pub fn run_variant(dx_mm: f32, points: &[RefPoint]) -> Result<Variant> {
    let dt_ms = dt_for(dx_mm);
    let (p1, p2, p5) = (point(points, "P1")?, point(points, "P2")?, point(points, "P5")?);
    // speeds over the distance from the edge of the stimulated cube
    let cv_long = f64::from(p2.position_mm[0] - STIM_EDGE_MM) / (p2.activation_ms - p1.activation_ms);
    let cv_trans = f64::from(p5.position_mm[1] - STIM_EDGE_MM) / (p5.activation_ms - p1.activation_ms);
    let base = AlievPanfilov::default();
    let mut best: Option<(f64, Variant)> = None;
    let mut tried = Vec::new();
    for s in EXCITATION_DIALS {
        let model = base.scaled_excitation(s);
        if dt_ms > 0.5 * model.max_dt_ms() {
            tried.push((s, None));
            continue;
        }
        let budget = trace_budget(&model, dx_mm, dt_ms, STABILITY);
        let (d_long, got_long) = solve_d(&model, cv_long as f32, dx_mm, dt_ms, budget * 0.9);
        let (d_trans, got_trans) = solve_d(&model, cv_trans as f32, dx_mm, dt_ms, budget * 0.9);
        let reached = |got: f32, want: f64| (f64::from(got) - want).abs() < 0.02 * want;
        if !reached(got_long, cv_long) || !reached(got_trans, cv_trans) || d_long + 2.0 * d_trans > budget {
            tried.push((s, None));
            continue;
        }
        let sim = run_slab(
            dx_mm,
            dt_ms,
            model,
            (d_long, d_trans),
            points,
            CALIBRATED_LIMIT_MS,
        );
        let fit = subset(points, &sim, Some(Role::Fit));
        // a run missing a fit point cannot be chosen, whatever its RMS over the rest
        let score = if fit.missing > 0 { f64::INFINITY } else { fit.rms };
        tried.push((s, score.is_finite().then_some(score)));
        if best.as_ref().is_none_or(|(b, _)| score < *b) {
            best = Some((
                score,
                Variant {
                    dx_mm,
                    dt_ms,
                    voxels: slab_nodes(dx_mm).iter().product(),
                    cv_long_target_m_s: cv_long,
                    cv_trans_target_m_s: cv_trans,
                    cv_long_achieved_m_s: f64::from(got_long),
                    cv_trans_achieved_m_s: f64::from(got_trans),
                    excitation: s,
                    d_long_mm2_ms: d_long,
                    d_trans_mm2_ms: d_trans,
                    dials_tried: Vec::new(),
                    all: subset(points, &sim, None),
                    fit,
                    held_out: subset(points, &sim, Some(Role::HeldOut)),
                    activation_ms: sim,
                    uncalibrated_d_long_mm2_ms: 0.0,
                    uncalibrated_d_trans_mm2_ms: 0.0,
                    uncalibrated_activation_ms: Vec::new(),
                    uncalibrated_limit_ms: UNCALIBRATED_LIMIT_MS,
                    uncalibrated: agreement(&[], &[]),
                },
            ));
        }
    }
    let Some((_, mut v)) = best.filter(|(score, _)| score.is_finite()) else {
        bail!("no excitation dial reached CV {cv_long:.3}/{cv_trans:.3} m/s at dx {dx_mm} mm: {tried:?}");
    };
    v.dials_tried = tried;

    let d = |g_i, g_e| (monodomain_sigma(g_i, g_e) / SIGMA_PER_D) as f32;
    v.uncalibrated_d_long_mm2_ms = d(super::export::G_IL, super::export::G_EL);
    v.uncalibrated_d_trans_mm2_ms = d(super::export::G_IT, super::export::G_ET);
    v.uncalibrated_activation_ms = run_slab(
        dx_mm,
        dt_ms,
        base,
        (v.uncalibrated_d_long_mm2_ms, v.uncalibrated_d_trans_mm2_ms),
        points,
        UNCALIBRATED_LIMIT_MS,
    );
    v.uncalibrated = subset(points, &v.uncalibrated_activation_ms, None);
    Ok(v)
}

/// An external solver's activation at each reference point.
#[derive(Debug, Clone, Serialize)]
pub struct External {
    pub directory: String,
    pub activation_ms: Vec<f64>,
    pub against_reference: Agreement,
}

fn external_activation(dir: &Path, mesh: Option<PathBuf>, points: &[RefPoint]) -> Result<External> {
    let options = SourceOptions {
        mesh,
        vm_array: None,
        lat_array: None,
        frame_dt_ms: None,
        length_unit: LengthUnit::Auto,
        lat_threshold: None,
    };
    let diagonal = f64::from(Vec3::from(SLAB_MM).length());
    let source = load_source(dir, &options, diagonal)?;
    let (sample_points, lat) = source
        .lat
        .ok_or_else(|| anyhow::anyhow!("{} holds no activation times", dir.display()))?;
    let targets: Vec<Vec3> = points.iter().map(|p| Vec3::from(p.position_mm)).collect();
    let activation_ms: Vec<f64> = nearest_map(&sample_points, &targets)
        .iter()
        .map(|m| {
            if m.distance_mm <= EXTERNAL_MATCH_MM {
                f64::from(lat[m.source as usize])
            } else {
                f64::NAN
            }
        })
        .collect();
    Ok(External {
        directory: dir.display().to_string(),
        against_reference: subset(points, &activation_ms, None),
        activation_ms,
    })
}

/// Settings of `niederer`.
#[derive(Debug)]
pub struct NiedererArgs {
    pub dx_mm: Vec<f32>,
    pub out: PathBuf,
    pub reference: Option<PathBuf>,
    pub external: Option<PathBuf>,
    pub mesh: Option<PathBuf>,
}

/// `niederer_metrics.json`.
#[derive(Debug, Serialize)]
struct Metrics<'a> {
    reference: &'a [RefPoint],
    variants: &'a [Variant],
    external: Option<&'a External>,
}

fn opt(v: f64) -> Option<f64> {
    v.is_finite().then_some(v)
}

fn report(points: &[RefPoint], variants: &[Variant], external: Option<&External>) -> String {
    let mut s = String::from("# Niederer slab: Tier 2 against the reference activation times\n\n");
    s.push_str(
        "**Read this first.** Tier 2 here is a *calibrated phenomenological* model, not TT06. The benchmark's own diffusivities \
are shown below for comparison. The agreement is a statement about front geometry after two conduction velocities and one \
excitation dial were fitted, and is **not** a verification of the monodomain equations with TT06. The held-out points are the \
measure to look at; they had no part in the calibration.\n\n\
The reference is **not** the benchmark's electronic supplementary material, which could not be retrieved. See \
`assets/validation/niederer_slab_reference.csv` and `docs/validation-bridge.md`. It is whole milliseconds, from one FEM solver at 0.1 mm.\n\n",
    );
    s.push_str("## Summary\n\n| dx mm | dt ms | RMS all ms | RMS fit ms | RMS held-out ms | max err ms | dial | D long mm²/ms | D trans mm²/ms | uncalibrated RMS ms | uncalibrated points missing |\n|---|---|---|---|---|---|---|---|---|---|---|\n");
    for v in variants {
        let _ = writeln!(
            s,
            "| {} | {} | {:.2} | {:.2} | {:.2} | {:.2} | {} | {:.3} | {:.3} | {} | {} |",
            v.dx_mm,
            v.dt_ms,
            v.all.rms,
            v.fit.rms,
            v.held_out.rms,
            v.all.max_abs,
            v.excitation,
            v.d_long_mm2_ms,
            v.d_trans_mm2_ms,
            cell(opt(v.uncalibrated.rms), 1),
            v.uncalibrated.missing
        );
    }
    for v in variants {
        let _ = writeln!(s, "\n## dx = {} mm, dt = {} ms\n", v.dx_mm, v.dt_ms);
        let _ = writeln!(
            s,
            "Targets from the reference's P1, P2, P5: CV long {:.3} m/s, trans {:.3} m/s; reached on the solver's own cable: {:.3} and {:.3} m/s. \
{} voxels. ![scatter](activation_dx{}.png)\n",
            v.cv_long_target_m_s,
            v.cv_trans_target_m_s,
            v.cv_long_achieved_m_s,
            v.cv_trans_achieved_m_s,
            v.voxels,
            v.dx_mm
        );
        s.push_str(
            "| point | x | y | z | role | reference ms | Tier 2 ms | error ms | benchmark D, unit dial ms |",
        );
        if external.is_some() {
            s.push_str(" external ms |");
        }
        s.push_str("\n|---|---|---|---|---|---|---|---|---|");
        if external.is_some() {
            s.push_str("---|");
        }
        s.push('\n');
        for (i, p) in points.iter().enumerate() {
            let sim = v.activation_ms[i];
            let _ = write!(
                s,
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                p.name,
                p.position_mm[0],
                p.position_mm[1],
                p.position_mm[2],
                if p.role == Role::Fit { "fit" } else { "held out" },
                p.activation_ms,
                cell(opt(sim), 1),
                cell(opt(sim - p.activation_ms), 1),
                cell(opt(v.uncalibrated_activation_ms[i]), 1)
            );
            if let Some(e) = external {
                let _ = write!(s, " {} |", cell(opt(e.activation_ms[i]), 1));
            }
            s.push('\n');
        }
        let _ = writeln!(
            s,
            "\nDials tried, with RMS at the fit points in ms (`-`: target out of reach or a fit point not activated): {}.",
            v.dials_tried
                .iter()
                .map(|(d, r)| format!("{d}: {}", cell(*r, 2)))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let _ = writeln!(
            s,
            "\nWith the benchmark's diffusivities ({:.4} and {:.4} mm²/ms) and unit excitation, a blank above means not activated within {} ms.",
            v.uncalibrated_d_long_mm2_ms, v.uncalibrated_d_trans_mm2_ms, v.uncalibrated_limit_ms
        );
    }
    if let Some(e) = external {
        let _ = writeln!(
            s,
            "\n## External solver\n\nActivation read from `{}`: RMS {} ms against the reference over {} points, {} missing.",
            e.directory,
            cell(opt(e.against_reference.rms), 2),
            e.against_reference.n,
            e.against_reference.missing
        );
    } else {
        s.push_str("\n## External solver\n\nNot compared: no solver results were given to this run (`--external`).\n");
    }
    s.push_str(
        "\n## Deviations from the benchmark\n\n\
- **Cell model.** Aliev-Panfilov, not TT06. The action potential has no TT06 ionic currents or APD behaviour.\n\
- **Diffusivities.** Calibrated to the reference's velocities on the solver's own grid, not the benchmark's σ/(βC_m). The excitation dial speeds the upstroke beyond the published model.\n\
- **Numerics.** Tier 2's explicit 18-point finite-difference stencil with Godunov splitting, not FEM. Each node's control volume reaches half a step past the slab, so the no-flux boundary sits h/2 further out than in a mesh.\n\
- **Activation threshold.** u = 0.5 of a normalised variable; the benchmark's 0 mV crossing is about 0.71 of the TT06 swing. With a fast upstroke this is a small constant shift, not corrected here.\n\
- **Stimulus.** A conductance pulling u to 1 for 2 ms, not the benchmark's 50 000 µA/cm³ current, so the corner activates earlier than the reference's 1 ms.\n\
- **Points.** Nine (corners and centre), not the benchmark's diagonal line.\n\
- **Reference.** A secondary table in whole milliseconds, computed with somewhat different diffusivities, with coordinates read from a published figure. See its file header.\n",
    );
    s
}

/// Run `niederer`.
pub fn run(a: &NiedererArgs) -> Result<()> {
    std::fs::create_dir_all(&a.out).with_context(|| format!("creating {}", a.out.display()))?;
    let text = match &a.reference {
        Some(p) => std::fs::read_to_string(p).with_context(|| format!("reading {}", p.display()))?,
        None => REFERENCE_CSV.to_string(),
    };
    let points = parse_reference(&text)?;
    let external = a
        .external
        .as_deref()
        .map(|d| external_activation(d, a.mesh.clone(), &points))
        .transpose()?;
    let mut variants = Vec::new();
    for dx in &a.dx_mm {
        ensure!(*dx > 0.0 && *dx <= 1.0, "--dx-mm {dx} is outside (0, 1] mm");
        let v = run_variant(*dx, &points)?;
        let pairs: Vec<(f64, f64)> = points
            .iter()
            .map(|p| p.activation_ms)
            .zip(v.activation_ms.iter().copied())
            .collect();
        activation_scatter(
            &a.out.join(format!("activation_dx{dx}.png")),
            &format!("Niederer slab, dx {dx} mm: Tier 2 (calibrated) against reference"),
            "reference activation, ms",
            "Tier 2 activation, ms",
            &pairs,
        )?;
        let mut csv = String::from("point,x_mm,y_mm,z_mm,role,reference_ms,tier2_ms,uncalibrated_ms\n");
        for (i, p) in points.iter().enumerate() {
            let _ = writeln!(
                csv,
                "{},{},{},{},{},{},{},{}",
                p.name,
                p.position_mm[0],
                p.position_mm[1],
                p.position_mm[2],
                if p.role == Role::Fit { "fit" } else { "held_out" },
                p.activation_ms,
                cell(opt(v.activation_ms[i]), 3),
                cell(opt(v.uncalibrated_activation_ms[i]), 3)
            );
        }
        std::fs::write(a.out.join(format!("activation_dx{dx}.csv")), csv)
            .context("writing activation CSV")?;
        eprintln!(
            "dx {dx} mm: RMS all {:.2} ms, fit {:.2} ms, held-out {:.2} ms (dial {}); benchmark D uncalibrated: RMS {} ms, {} points not activated by {} ms",
            v.all.rms,
            v.fit.rms,
            v.held_out.rms,
            v.excitation,
            cell(opt(v.uncalibrated.rms), 1),
            v.uncalibrated.missing,
            v.uncalibrated_limit_ms
        );
        variants.push(v);
    }
    std::fs::write(
        a.out.join("niederer_report.md"),
        report(&points, &variants, external.as_ref()),
    )
    .context("writing the report")?;
    let metrics = Metrics {
        reference: &points,
        variants: &variants,
        external: external.as_ref(),
    };
    std::fs::write(
        a.out.join("niederer_metrics.json"),
        serde_json::to_string_pretty(&metrics)?,
    )
    .context("writing the metrics")?;
    eprintln!("wrote {}", a.out.join("niederer_report.md").display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stored_reference_parses_and_keeps_points_out_of_the_fit() {
        let p = parse_reference(REFERENCE_CSV).unwrap();
        assert_eq!(p.len(), 9);
        assert_eq!(p.iter().filter(|x| x.role == Role::HeldOut).count(), 5);
        // the corner opposite the stimulus is last, and is never fitted
        let last = p
            .iter()
            .max_by(|a, b| a.activation_ms.total_cmp(&b.activation_ms))
            .unwrap();
        assert_eq!((last.name.as_str(), last.role), ("P8", Role::HeldOut));
    }

    #[test]
    fn slab_grids_and_steps() {
        assert_eq!(slab_nodes(0.5), [41, 15, 7]);
        assert_eq!(slab_nodes(0.2), [101, 36, 16]);
        assert_eq!(dt_for(0.5), 0.05);
        assert_eq!(dt_for(0.2), 0.005);
    }

    #[test]
    fn benchmark_diffusivity_is_sigma_over_beta_cm() {
        use crate::bridge::export::{G_EL, G_IL};
        let d = monodomain_sigma(G_IL, G_EL) / SIGMA_PER_D;
        assert!((d - 0.0953).abs() < 1e-3, "{d}");
    }

    #[test]
    fn bad_references_are_errors() {
        assert!(parse_reference("point,x_mm,y_mm,z_mm,activation_ms,role\nP1,0,0,0,1,maybe\n").is_err());
        assert!(parse_reference("point,x_mm,y_mm,z_mm,activation_ms,role\n").is_err());
    }
}
