//! B23 acceptance, through the real binary: `export-opencarp` against openCARP's documented file formats,
//! `import-vtk` round trips of synthetic results, `validation-report`, and the Niederer slab.
//!
//! openCARP and `meshtool` are not run here: validating an export with them is the documented manual step
//! in `docs/validation-bridge.md`. These tests check the same properties with heartsim's own parsers and
//! `check_mesh`.

use glam::Vec3;
use heartsim_geometry::bridge::igb::{igb_bytes, parse_igb};
use heartsim_geometry::bridge::opencarp::{parse_elem, parse_lon, parse_pts, parse_vtx};
use heartsim_geometry::bridge::tetra::{TetMesh, check_mesh};
use heartsim_geometry::bridge::vtk::{parse_vtu, vtu_text};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// Lattice spacing of the procedural heart in these tests: a few thousand cells, as in `Resolution::COARSE`.
const SPACING: &str = "4";

fn cli(args: &[&str]) -> std::process::Output {
    let out = Command::new(env!("CARGO_BIN_EXE_heartsim-cli"))
        .args(args)
        .output()
        .expect("the binary runs");
    assert!(
        out.status.success(),
        "heartsim-cli {args:?} failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

fn fresh_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("heartsim_b23_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn text(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn json(path: &Path) -> serde_json::Value {
    serde_json::from_str(&text(path)).unwrap()
}

fn read_mesh(dir: &Path) -> TetMesh {
    let elems = parse_elem(&text(&dir.join("heart.elem"))).unwrap();
    assert!(elems.iter().all(|e| e.kind == "Tt"));
    TetMesh {
        points_mm: parse_pts(&text(&dir.join("heart.pts"))).unwrap(),
        tets: elems
            .iter()
            .map(|e| [e.nodes[0], e.nodes[1], e.nodes[2], e.nodes[3]])
            .collect(),
        tags: elems
            .iter()
            .map(|e| e.tag.expect("every element is tagged"))
            .collect(),
        fibres: parse_lon(&text(&dir.join("heart.lon"))).unwrap(),
    }
}

fn param<'a>(par: &'a str, key: &str) -> &'a str {
    par.lines()
        .find_map(|l| l.strip_prefix(key).and_then(|r| r.trim_start().strip_prefix('=')))
        .unwrap_or_else(|| panic!("no `{key}` in the parameter file"))
        .trim()
}

/// The idealised heart's sinus beat, exported once for all tests.
fn exported_heart() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = fresh_dir("heart");
        cli(&[
            "export-opencarp",
            "sinus_normal",
            "--out",
            dir.to_str().unwrap(),
            "--spacing-mm",
            SPACING,
            "--tend-ms",
            "500",
        ]);
        dir
    })
}

#[test]
fn export_of_the_idealised_heart_meets_the_documented_formats() {
    let dir = exported_heart();
    let mesh = read_mesh(dir);
    // one fibre per element: the manual requires it and readers fail otherwise
    assert_eq!(mesh.fibres.len(), mesh.tets.len());
    let check = check_mesh(&mesh);
    assert!(check.is_valid(), "{check:?}");
    assert!(check.elements > 10_000, "{check:?}");
    // the atria and ventricles are insulated, so there are at least two pieces
    assert!(check.components >= 2, "{check:?}");

    // the wall must be solid: nearly every tissue cell has its own cube, so the mesh has the cloud's volume
    let summary = json(&dir.join("mesh_summary.json"));
    let (own, merged) = (
        summary["cells_meshed"].as_u64().unwrap() as f64,
        summary["cells_merged"].as_u64().unwrap() as f64,
    );
    eprintln!("cells with their own cube {own}, merged {merged}");
    assert!(
        merged / (own + merged) < 0.02,
        "{merged} of {} cells lost their cube",
        own + merged
    );

    // coordinates are micrometres: a heart is about 100 mm across, so 100 000 in the file
    let raw = text(&dir.join("heart.pts"));
    let largest = raw
        .lines()
        .skip(1)
        .flat_map(str::split_whitespace)
        .map(|w| w.parse::<f64>().unwrap().abs())
        .fold(0.0, f64::max);
    assert!(
        (30_000.0..200_000.0).contains(&largest),
        "largest coordinate {largest}"
    );

    // every tag in the mesh has conductivities and a cell model, and no others are declared
    let par = text(&dir.join("parameters.par"));
    let regions: usize = param(&par, "num_gregions").parse().unwrap();
    assert_eq!(regions, check.tag_counts.len());
    assert_eq!(param(&par, "num_imp_regions"), param(&par, "num_gregions"));
    for (i, tag) in check.tag_counts.keys().enumerate() {
        assert_eq!(param(&par, &format!("gregion[{i}].ID")), tag.to_string());
        assert_eq!(param(&par, &format!("imp_region[{i}].ID")), tag.to_string());
        let g: f64 = param(&par, &format!("gregion[{i}].g_il")).parse().unwrap();
        assert!(g > 0.0 && g.is_finite(), "tag {tag}: g_il {g}");
        assert_eq!(param(&par, &format!("imp_region[{i}].im")), "AlievPanfilov");
    }
    assert_eq!(param(&par, "bidomain"), "0");
    assert_eq!(param(&par, "num_LATs"), "1");

    // tags.csv describes exactly the elements of the mesh
    let tags = text(&dir.join("tags.csv"));
    let total: usize = tags
        .lines()
        .skip(1)
        .map(|l| l.split(',').nth(2).unwrap().parse::<usize>().unwrap())
        .sum();
    assert_eq!(total, check.elements);
}

#[test]
fn stimuli_are_timing_engine_events_on_real_nodes() {
    let dir = exported_heart();
    let par = text(&dir.join("parameters.par"));
    let count: usize = param(&par, "num_stim").parse().unwrap();
    assert!(
        count > 1,
        "a sinus beat has atrial and ventricular stimuli, got {count}"
    );
    let nodes = parse_pts(&text(&dir.join("heart.pts"))).unwrap().len();
    let mut starts = Vec::new();
    for i in 0..count {
        assert_eq!(
            param(&par, &format!("stim[{i}].crct.type")),
            "0",
            "transmembrane current"
        );
        let strength: f64 = param(&par, &format!("stim[{i}].pulse.strength")).parse().unwrap();
        assert!(
            (strength - 35.71).abs() < 0.01,
            "the openCARP example's value, 50000/1400: {strength}"
        );
        let file = param(&par, &format!("stim[{i}].elec.vtx_file"));
        let vtx = parse_vtx(&text(&dir.join(format!("{file}.vtx")))).unwrap();
        assert!(!vtx.is_empty() && vtx.iter().all(|n| (*n as usize) < nodes));
        starts.push(
            param(&par, &format!("stim[{i}].ptcl.start"))
                .parse::<f64>()
                .unwrap(),
        );
    }
    assert!(starts.windows(2).all(|w| w[0] < w[1]), "{starts:?}");
    assert!(starts[0] < 500.0);
    // the ventricles fire after the AV delay, so the stimuli span more than one chamber's activation
    assert!(starts[starts.len() - 1] - starts[0] > 80.0, "{starts:?}");
}

#[test]
fn the_carputils_script_is_valid_python_and_calls_opencarp_as_documented() {
    let script = text(&exported_heart().join("run.py"));
    for needle in [
        "from carputils import tools",
        "@tools.carpexample(parser, jobID)",
        "tools.standard_parser()",
        "tools.carp_cmd(",
        "job.carp(cmd)",
        "-meshname",
    ] {
        assert!(script.contains(needle), "run.py lacks `{needle}`");
    }
    let Ok(status) = Command::new("python3").arg("--version").output() else {
        eprintln!("python3 not found: run.py syntax not checked");
        return;
    };
    assert!(status.status.success());
    let out = Command::new("python3")
        .args(["-I", "-c", "import ast,sys; ast.parse(open(sys.argv[1]).read())"])
        .arg(exported_heart().join("run.py"))
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
}

#[test]
fn export_writes_the_licence_note_and_the_manual_check() {
    let readme = text(&exported_heart().join("README.md"));
    assert!(readme.contains("meshtool query quality"));
    assert!(readme.contains("licence"));
    assert!(readme.contains("first guess"));
}

#[test]
fn the_benchmark_slab_exports_with_its_published_parameters() {
    let dir = fresh_dir("slab");
    cli(&[
        "export-opencarp",
        "niederer-slab",
        "--out",
        dir.to_str().unwrap(),
        "--dx-mm",
        "0.5",
    ]);
    let mesh = read_mesh(&dir);
    assert_eq!(mesh.points_mm.len(), 41 * 15 * 7);
    let check = check_mesh(&mesh);
    assert!(check.is_valid(), "{check:?}");
    assert!(
        (check.volume_mm3 - 20.0 * 7.0 * 3.0).abs() < 1e-2,
        "{}",
        check.volume_mm3
    );
    assert!(mesh.fibres.iter().all(|f| *f == Vec3::X));
    let par = text(&dir.join("parameters.par"));
    // the values of the openCARP example of this benchmark
    for (key, value) in [
        ("gregion[0].g_il", "0.17"),
        ("gregion[0].g_it", "0.019"),
        ("gregion[0].g_in", "0.019"),
        ("gregion[0].g_el", "0.62"),
        ("gregion[0].g_et", "0.24"),
        ("gregion[0].g_en", "0.24"),
        ("imp_region[0].im", "tenTusscherPanfilov"),
        ("imp_region[0].im_param", "\"flags=EPI\""),
        ("imp_region[0].cellSurfVolRatio", "0.14"),
        ("stim[0].pulse.strength", "35.71"),
        ("stim[0].ptcl.duration", "2"),
        ("stim[0].elec.geom_type", "2"),
        ("stim[0].elec.p1[0]", "1500"),
        ("lats[0].threshold", "0"),
        ("dt", "50"),
    ] {
        assert_eq!(param(&par, key), value, "{key}");
    }
}

/// The analytic field of the round trips: a plane wave over the heart, in ms.
fn plane_wave_ms(p: Vec3) -> f64 {
    100.0 + f64::from(p.x) * 0.5 + f64::from(p.y) * 0.25 - f64::from(p.z) * 0.125
}

/// Mesh nodes of the export in solver units (µm), with the plane wave, as a solver would write them.
fn solver_nodes() -> (Vec<[f64; 3]>, Vec<[u32; 4]>, Vec<f64>) {
    let mesh = read_mesh(exported_heart());
    let um = mesh
        .points_mm
        .iter()
        .map(|p| (p.as_dvec3() * 1000.0).to_array())
        .collect();
    let lat = mesh.points_mm.iter().map(|p| plane_wave_ms(*p)).collect();
    (um, mesh.tets, lat)
}

/// Check an imported activation map against the plane wave, within what nearest-point mapping can do.
fn check_plane_wave(import_dir: &Path) {
    let summary = json(&import_dir.join("import.json"));
    assert_eq!(summary["length_scale_to_mm"], 1e-3, "{summary}");
    let spacing: f64 = SPACING.parse().unwrap();
    // a cell centre is at most half a cube diagonal from a corner of its cube
    let limit = 3f64.sqrt() / 2.0 * spacing;
    let body = text(&import_dir.join("cells_activation.csv"));
    let (mut compared, mut worst) = (0, 0.0f64);
    for line in body.lines().skip(1) {
        let w: Vec<&str> = line.split(',').collect();
        let distance: f64 = w[7].parse().unwrap();
        if w[5].is_empty() || distance > limit + 1e-3 {
            continue;
        }
        let p = Vec3::new(
            w[1].parse().unwrap(),
            w[2].parse().unwrap(),
            w[3].parse().unwrap(),
        );
        let got: f64 = w[5].parse().unwrap();
        worst = worst.max((got - plane_wave_ms(p)).abs());
        compared += 1;
    }
    assert!(compared > 1000, "only {compared} cells compared");
    // |∇t| = √(0.5² + 0.25² + 0.125²) ms/mm, times the mapping distance
    let gradient = (0.25f64 + 0.0625 + 0.015_625).sqrt();
    assert!(worst <= gradient * limit + 1e-2, "worst error {worst} ms");
}

#[test]
fn a_synthetic_vtu_round_trips_onto_the_cell_cloud() {
    let dir = fresh_dir("vtu");
    let (points, tets, lat) = solver_nodes();
    std::fs::write(
        dir.join("result.vtu"),
        vtu_text(&points, &tets, &[("activation_time", &lat)], &[]),
    )
    .unwrap();
    // the file parses back to what was written
    let back = parse_vtu(text(&dir.join("result.vtu")).as_bytes()).unwrap();
    assert_eq!(back.points.len(), points.len());
    let out = dir.join("imported");
    cli(&[
        "import-vtk",
        dir.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--spacing-mm",
        SPACING,
    ]);
    check_plane_wave(&out);
}

#[test]
fn a_cell_data_vtu_maps_from_cell_centres() {
    // finite-volume solvers (MonoAlg3D) store fields per cell; the unit is told from the mesh size
    let dir = fresh_dir("cell_data");
    let (points, tets, _) = solver_nodes();
    let lat: Vec<f64> = tets
        .iter()
        .map(|t| {
            let c = t
                .iter()
                .map(|n| Vec3::from_array(points[*n as usize].map(|v| v as f32)))
                .sum::<Vec3>()
                / 4.0;
            plane_wave_ms(c / 1000.0)
        })
        .collect();
    std::fs::write(
        dir.join("result.vtu"),
        vtu_text(&points, &tets, &[], &[("LAT", &lat)]),
    )
    .unwrap();
    let out = dir.join("imported");
    cli(&[
        "import-vtk",
        dir.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--spacing-mm",
        SPACING,
    ]);
    check_plane_wave(&out);
}

#[test]
fn igb_and_activation_dat_round_trip_with_the_export_mesh() {
    let dir = fresh_dir("igb");
    let (_, _, lat) = solver_nodes();
    let nodes = lat.len();
    let mut dat = String::new();
    for t in &lat {
        let _ = writeln!(dat, "{t}");
    }
    std::fs::write(dir.join("vm_act-thresh.dat"), dat).unwrap();
    // three frames, each a constant: nodes follow the frames exactly
    let mut series = Vec::with_capacity(nodes * 3);
    for v in [-85.0f32, -20.0, 30.0] {
        series.extend(std::iter::repeat_n(v, nodes));
    }
    std::fs::write(dir.join("vm.igb"), igb_bytes(nodes, &series, 0.0, 10.0, "mV")).unwrap();
    let out = dir.join("imported");
    let mesh = exported_heart().join("heart");
    cli(&[
        "import-vtk",
        dir.to_str().unwrap(),
        "--mesh",
        mesh.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--spacing-mm",
        SPACING,
    ]);
    check_plane_wave(&out);
    let summary = json(&out.join("import.json"));
    assert_eq!(summary["frames"], 3);
    assert_eq!(summary["vm_units"], "mV");
    let cells = parse_igb(&std::fs::read(out.join("cells_vm.igb")).unwrap()).unwrap();
    assert_eq!(cells.frames, 3);
    assert_eq!(cells.dt_ms, 10.0);
    assert!(
        cells
            .frame(2)
            .iter()
            .filter(|v| v.is_finite())
            .all(|v| (*v - 30.0).abs() < 1e-6)
    );
}

#[test]
fn the_wrong_mesh_is_refused_rather_than_mapped() {
    let dir = fresh_dir("wrong_mesh");
    std::fs::write(dir.join("vm_act-thresh.dat"), "1\n2\n3\n").unwrap();
    let mesh = exported_heart().join("heart");
    let out = Command::new(env!("CARGO_BIN_EXE_heartsim-cli"))
        .args([
            "import-vtk",
            dir.to_str().unwrap(),
            "--mesh",
            mesh.to_str().unwrap(),
            "--spacing-mm",
            SPACING,
        ])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("nodes"));
}

/// Activation and V_m of a fake solver: a travelling step over the mesh, written as a `.pvd` VTU series.
fn write_series(dir: &Path, delay_ms: f64) {
    let (points, tets, _) = solver_nodes();
    let mesh = read_mesh(exported_heart());
    let mut index = String::from("<VTKFile type=\"Collection\"><Collection>\n");
    for k in 0..30 {
        let t = f64::from(k) * 20.0;
        let vm: Vec<f64> = mesh
            .points_mm
            .iter()
            .map(|p| {
                if t >= plane_wave_ms(*p) + delay_ms {
                    20.0
                } else {
                    -80.0
                }
            })
            .collect();
        let name = format!("frame_{k}.vtu");
        std::fs::write(dir.join(&name), vtu_text(&points, &tets, &[("Vm", &vm)], &[])).unwrap();
        let _ = writeln!(index, "<DataSet timestep=\"{t}\" file=\"{name}\"/>");
    }
    index.push_str("</Collection></VTKFile>\n");
    std::fs::write(dir.join("series.pvd"), index).unwrap();
}

#[test]
fn report_is_written_and_its_activation_error_is_the_error_put_in() {
    // two imports differing by a known delay: activation times computed from V_m must differ by that delay
    let (a_dir, b_dir) = (fresh_dir("series_a"), fresh_dir("series_b"));
    write_series(&a_dir, 0.0);
    write_series(&b_dir, 40.0);
    let import = |d: &Path| {
        let out = d.join("imported");
        cli(&[
            "import-vtk",
            d.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--spacing-mm",
            SPACING,
        ]);
        out
    };
    let (a, b) = (import(&a_dir), import(&b_dir));
    assert_eq!(json(&a.join("import.json"))["lat_from_vm"], true);

    let report = |imported: &Path, name: &str| {
        let out = fresh_dir(name);
        cli(&[
            "validation-report",
            "sinus_normal",
            "--imported",
            imported.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--spacing-mm",
            SPACING,
        ]);
        out
    };
    let (ra, rb) = (report(&a, "report_a"), report(&b, "report_b"));
    for dir in [&ra, &rb] {
        for f in [
            "report.md",
            "metrics.json",
            "activation_scatter.png",
            "ecg_overlay.png",
        ] {
            assert!(dir.join(f).is_file(), "{f} missing");
        }
        let png = std::fs::read(dir.join("ecg_overlay.png")).unwrap();
        assert!(png.starts_with(b"\x89PNG"));
    }
    let (ma, mb) = (json(&ra.join("metrics.json")), json(&rb.join("metrics.json")));
    assert_eq!(ma["leads"].as_array().unwrap().len(), 12);
    assert_eq!(ma["weight_stats"].as_array().unwrap().len(), 9);
    // a plane wave is not a sinus beat, so the error is large; a further delay shows up as bias
    let (ba, bb) = (
        ma["activation_ms"]["bias"].as_f64().unwrap(),
        mb["activation_ms"]["bias"].as_f64().unwrap(),
    );
    assert!(ma["activation_ms"]["n"].as_u64().unwrap() > 100);
    // the delay is two whole frames, so the interpolation is the same and only the frame number changes
    assert!((bb - ba - 40.0).abs() < 0.01, "bias {ba} then {bb}");

    let md = text(&ra.join("report.md"));
    for needle in [
        "Activation time",
        "RMSE mV",
        "QRS r",
        "T r",
        "ecg_overlay.png",
        "same forward model",
    ] {
        assert!(md.contains(needle), "report lacks `{needle}`");
    }
}

/// A "solver" that is Tier 1 itself: its V_m every 10 ms, written per cell, must score an ECG error of
/// zero. This pins the whole path: file format, nearest-point mapping, time alignment, normalisation and
/// the shared forward model.
#[test]
fn a_solver_that_is_tier1_itself_has_no_ecg_error() {
    use heartsim_core::Millis;
    use heartsim_ecg::Tier0Scenario;
    use heartsim_geometry::AnatomyPack;
    use heartsim_geometry::procedural::Resolution;
    use heartsim_sim::{EikonalEngine, EikonalParams, Engine, Tier1Engine};
    use std::sync::Arc;

    let spacing: f32 = SPACING.parse().unwrap();
    let pack = AnatomyPack::procedural(Resolution {
        mesh_voxel_mm: spacing,
        cell_spacing_mm: spacing,
    });
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/scenarios/sinus_normal.ron");
    let s = Tier0Scenario::from_ron(&text(&file)).unwrap();
    let eik = EikonalEngine::with_seed_sets(
        Arc::new(pack.cells.clone()),
        &s.scenario.parameters,
        EikonalParams::default(),
        &pack.landmarks.sets,
    );
    let mut t1 = Tier1Engine::new(s.timing.clone(), eik);

    let dir = fresh_dir("tier1_solver");
    let points: Vec<[f64; 3]> = pack
        .cells
        .positions
        .iter()
        .map(|p| p.as_dvec3().to_array())
        .collect();
    let mut index = String::from("<VTKFile type=\"Collection\"><Collection>\n");
    for k in 0..=60 {
        if k > 0 {
            for _ in 0..10 {
                t1.step(Millis(1.0));
            }
        }
        let vm: Vec<f64> = t1.cells().vm.iter().map(|v| f64::from(*v)).collect();
        let name = format!("frame_{k}.vtu");
        std::fs::write(dir.join(&name), vtu_text(&points, &[], &[("Vm", &vm)], &[])).unwrap();
        let _ = writeln!(index, "<DataSet timestep=\"{}\" file=\"{name}\"/>", k * 10);
    }
    index.push_str("</Collection></VTKFile>\n");
    std::fs::write(dir.join("series.pvd"), index).unwrap();

    let imported = dir.join("imported");
    cli(&[
        "import-vtk",
        dir.to_str().unwrap(),
        "--out",
        imported.to_str().unwrap(),
        "--spacing-mm",
        SPACING,
        "--length-unit",
        "mm",
    ]);
    let summary = json(&imported.join("import.json"));
    assert_eq!(summary["max_distance_mm"], 0.0, "{summary}");
    assert_eq!(summary["vm_units"], "1");

    let rep = fresh_dir("tier1_solver_report");
    cli(&[
        "validation-report",
        "sinus_normal",
        "--imported",
        imported.to_str().unwrap(),
        "--out",
        rep.to_str().unwrap(),
        "--spacing-mm",
        SPACING,
        "--vm-rest",
        "0",
        "--vm-peak",
        "1",
    ]);
    let m = json(&rep.join("metrics.json"));
    for lead in m["leads"].as_array().unwrap() {
        let rmse = number(&lead["rmse_mv"]);
        // single-precision rounding in the lead sums, which differs by platform: up to 0.8 µV on x86-64 Linux and
        // 1.3 µV on Apple silicon, against a QRS of about 1 mV
        assert!(rmse < 5e-3, "{lead}");
        // a lead whose QRS or T is flat in this coarse window has no correlation; a real one must be 1
        if let Some(r) = lead["qrs_r"].as_f64() {
            assert!(r > 0.999_99, "{lead}");
        }
        if let Some(r) = lead["t_r"].as_f64() {
            assert!(r > 0.999_99, "{lead}");
        }
    }
    assert!(m["leads"].as_array().unwrap().iter().any(|l| l["qrs_r"].is_f64()));
    // activation from 10 ms frames can only be good to about the frame interval
    let a = &m["activation_ms"];
    eprintln!("activation against own V_m crossing: {a}");
    assert!(number(&a["rms"]) < 10.0, "{a}");
    assert!(a["r"].as_f64().unwrap() > 0.99, "{a}");
}

#[test]
fn an_import_without_v_m_says_the_ecg_was_not_compared() {
    let dir = fresh_dir("self");
    let first = fresh_dir("self_first");
    let (points, tets, lat) = solver_nodes();
    std::fs::write(
        first.join("result.vtu"),
        vtu_text(&points, &tets, &[("lat", &lat)], &[]),
    )
    .unwrap();
    let imported = dir.join("imported");
    cli(&[
        "import-vtk",
        first.to_str().unwrap(),
        "--out",
        imported.to_str().unwrap(),
        "--spacing-mm",
        SPACING,
    ]);
    let rep = fresh_dir("self_report");
    cli(&[
        "validation-report",
        "sinus_normal",
        "--imported",
        imported.to_str().unwrap(),
        "--out",
        rep.to_str().unwrap(),
        "--spacing-mm",
        SPACING,
    ]);
    let md = text(&rep.join("report.md"));
    assert!(md.contains("Not compared: the import holds no V_m series"));
    assert!(!rep.join("ecg_overlay.png").exists());
}

fn number(v: &serde_json::Value) -> f64 {
    v.as_f64().unwrap_or(f64::NAN)
}

#[test]
fn niederer_slab_at_half_a_millimetre_is_within_3_ms_of_the_reference() {
    let dir = fresh_dir("niederer");
    cli(&["niederer", "--out", dir.to_str().unwrap(), "--dx-mm", "0.5"]);
    let m = json(&dir.join("niederer_metrics.json"));
    let v = &m["variants"][0];
    let (all, fit, held) = (
        number(&v["all"]["rms"]),
        number(&v["fit"]["rms"]),
        number(&v["held_out"]["rms"]),
    );
    eprintln!("dx 0.5: RMS all {all:.3}, fit {fit:.3}, held out {held:.3}");
    // the brief's criterion
    assert!(all < 3.0, "activation RMS {all} ms");
    // and the same bound where nothing was fitted, or the criterion would mean little
    assert!(held < 3.0, "held-out RMS {held} ms");
    assert_eq!(v["all"]["missing"], 0);
    assert_eq!(v["held_out"]["n"], 5);
    // the honest baseline: the benchmark's own diffusivities are nowhere near with this cell model
    let uncalibrated = number(&v["uncalibrated"]["rms"]);
    let not_reached = v["uncalibrated"]["missing"].as_u64().unwrap();
    assert!(
        not_reached > 0 || uncalibrated > 10.0,
        "uncalibrated RMS {uncalibrated}, {not_reached} missing"
    );

    assert!(dir.join("activation_dx0.5.png").is_file());
    assert!(dir.join("activation_dx0.5.csv").is_file());
    let md = text(&dir.join("niederer_report.md"));
    for needle in [
        "not** a verification",
        "not** the benchmark's electronic supplementary material",
        "Deviations from the benchmark",
        "Not compared",
        "held out",
    ] {
        assert!(md.contains(needle), "the report lacks `{needle}`");
    }
}

#[test]
fn niederer_report_is_deterministic() {
    let run = |name: &str| {
        let dir = fresh_dir(name);
        cli(&["niederer", "--out", dir.to_str().unwrap(), "--dx-mm", "0.5"]);
        dir
    };
    let (a, b) = (run("det_a"), run("det_b"));
    for f in [
        "niederer_report.md",
        "niederer_metrics.json",
        "activation_dx0.5.png",
        "activation_dx0.5.csv",
    ] {
        assert_eq!(
            std::fs::read(a.join(f)).unwrap(),
            std::fs::read(b.join(f)).unwrap(),
            "{f}"
        );
    }
}

#[test]
fn an_external_solver_is_added_to_the_slab_table() {
    // a fake solver: the reference times themselves at the eight corners of a coarse slab mesh
    let root = fresh_dir("external");
    let results = root.join("results");
    std::fs::create_dir_all(&results).unwrap();
    let reference: [([f64; 3], f64); 9] = [
        ([0.0, 0.0, 0.0], 1.0),
        ([20_000.0, 0.0, 0.0], 29.0),
        ([0.0, 0.0, 3000.0], 8.0),
        ([20_000.0, 0.0, 3000.0], 31.0),
        ([0.0, 7000.0, 0.0], 27.0),
        ([20_000.0, 7000.0, 0.0], 41.0),
        ([0.0, 7000.0, 3000.0], 29.0),
        ([20_000.0, 7000.0, 3000.0], 43.0),
        ([10_000.0, 3500.0, 1500.0], 20.0),
    ];
    let points: Vec<[f64; 3]> = reference.iter().map(|r| r.0).collect();
    let lat: Vec<f64> = reference.iter().map(|r| r.1).collect();
    std::fs::write(
        results.join("lat.vtu"),
        vtu_text(&points, &[], &[("LAT", &lat)], &[]),
    )
    .unwrap();
    let out = root.join("report");
    cli(&[
        "niederer",
        "--out",
        out.to_str().unwrap(),
        "--dx-mm",
        "0.5",
        "--external",
        results.to_str().unwrap(),
    ]);
    let m = json(&out.join("niederer_metrics.json"));
    assert_eq!(number(&m["external"]["against_reference"]["rms"]), 0.0);
    assert_eq!(m["external"]["against_reference"]["n"], 9);
    assert!(text(&out.join("niederer_report.md")).contains("External solver"));
}

/// Takes minutes in the dev profile: `cargo test -p heartsim-cli --release -- --ignored`.
#[test]
#[ignore = "fine slab; minutes of CPU"]
fn niederer_slab_at_a_fifth_of_a_millimetre() {
    let dir = fresh_dir("niederer_fine");
    cli(&["niederer", "--out", dir.to_str().unwrap(), "--dx-mm", "0.2"]);
    let m = json(&dir.join("niederer_metrics.json"));
    let v = &m["variants"][0];
    let all = number(&v["all"]["rms"]);
    eprintln!(
        "dx 0.2: RMS all {all:.3}, held out {:.3}",
        number(&v["held_out"]["rms"])
    );
    assert!(all < 3.0, "activation RMS {all} ms");
}
