mod bridge;
mod content;

use anyhow::Result;
use clap::{Parser, Subcommand};
use heartsim_core::{Lead, Millis, Scenario};
use heartsim_ecg::{GaussianSynth, Tier0Scenario};
use heartsim_geometry::{AnatomyPack, K_NEIGHBOURS};
use heartsim_sim::{Engine, TimingEngine, TimingParams};
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(name = "heartsim-cli", about = "Headless heartsim tools")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Synthesise a Tier-0 12-lead ECG and write it as CSV (ms, I..V6 in mV).
    Synth {
        #[arg(long, default_value_t = 70.0)]
        bpm: f64,
        #[arg(long, default_value_t = 10.0)]
        seconds: f64,
        #[arg(long, default_value_t = 500.0)]
        sample_rate: f64,
        #[arg(long)]
        av_block: bool,
        #[arg(long, default_value_t = 0.0)]
        axis_shift: f64,
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Write the default scenario file (RON) so it can be edited.
    DefaultScenario {
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Load every `*.ron` scenario in a directory, check ids, teaching content and differentials,
    /// run each for 20 s on the timing engine + Tier-0 synthesiser and compare the B4 analysis of
    /// the last 10 s with the file's `expected` block. Fails if any scenario fails.
    ValidateScenarios {
        /// Scenario directory (e.g. `assets/scenarios`).
        dir: PathBuf,
    },
    /// Load a tutorial (a `tutorial.ron`, a directory of tutorials, an installed tutorial id, or `all`),
    /// check its content rules (every page changes the simulation, checks are well formed, references
    /// given) and execute every page action headlessly on the timing engine and synthesiser. Fails on
    /// any error: an action the engine refuses, an unknown rhythm or highlight, a wait that never ends.
    RunTutorial {
        /// Tutorial file, directory of tutorials, id, or `all` for the installed library.
        #[arg(default_value = "all")]
        tutorial: String,
        /// The `assets/` directory (default: `$HEARTSIM_ASSETS`, `./assets`, the source tree).
        #[arg(long)]
        assets: Option<PathBuf>,
    },
    /// Load every `*.ron` patient case in a directory (e.g. `assets/cases`), check ids, references,
    /// question counts and explanations, and execute every stage's actions headlessly. Fails if any
    /// case does.
    ValidateCases {
        /// Case directory (a sibling `scenarios/` folder holds the rhythms).
        dir: PathBuf,
    },
    /// Run a scenario headless and write the same exports as the app's Export window: 12-lead PDF,
    /// page and strip PNGs, CSV, WFDB (.hea + .dat), scenario RON and the share bundle zip.
    Run {
        /// Scenario file, or the id of a scenario in the installed library (e.g. `lbbb`).
        scenario: String,
        /// Simulated time to run, s.
        #[arg(long, default_value_t = 10.0)]
        seconds: f64,
        /// Length of the exported excerpt (the end of the run), s.
        #[arg(long, default_value_t = 10.0)]
        window: f64,
        /// Paper speed of the PDF and PNGs, mm/s.
        #[arg(long, default_value_t = 25.0)]
        speed: f32,
        /// Gain of the PDF and PNGs, mm/mV.
        #[arg(long, default_value_t = 10.0)]
        gain: f32,
        /// PDF paper: a4 or letter (landscape).
        #[arg(long, default_value = "a4")]
        paper: String,
        /// Date printed in the headers, YYYY-MM-DD (default: today, UTC).
        #[arg(long)]
        date: Option<String>,
        /// Cabrera limb-lead order on the PDF and page PNG.
        #[arg(long)]
        cabrera: bool,
        /// Lead of the strip PNG.
        #[arg(long, default_value = "II")]
        strip_lead: String,
        /// Comma-separated subset of pdf,png,csv,wfdb,ron,bundle.
        #[arg(long, default_value = "pdf,png,csv,wfdb,ron,bundle")]
        formats: String,
        /// Output directory (created if missing).
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Load an anatomy pack and print counts, bounds, per-region cells, k-NN statistics, landmarks
    /// and electrodes.
    AnatomyInfo {
        /// Pack directory (contains `manifest.ron`).
        pack: PathBuf,
    },
    /// Write a scenario as an openCARP experiment: mesh, fibres, region tags, stimuli, parameters and a
    /// carputils script. heartsim only writes the files; running openCARP is your step, under its licence.
    ExportOpencarp {
        /// Scenario file or library id, or `niederer-slab` for the benchmark slab.
        scenario: String,
        /// Output directory (created if missing).
        #[arg(short, long)]
        out: PathBuf,
        /// Anatomy pack directory (default: the procedural idealised heart).
        #[arg(long)]
        pack: Option<PathBuf>,
        /// Lattice spacing of the procedural heart, mm.
        #[arg(long, default_value_t = 2.3)]
        spacing_mm: f32,
        /// Cell model the solver runs.
        #[arg(long, value_enum, default_value = "ap")]
        ionic: bridge::export::Ionic,
        /// Simulated time, ms; timing-engine events up to here become stimuli.
        #[arg(long, default_value_t = 800.0)]
        tend_ms: f64,
        /// Radius of each seed cell's electrode, mm (default 1.5 × the spacing).
        #[arg(long)]
        stim_radius_mm: Option<f32>,
        /// APD band width of the element tags, ms.
        #[arg(long, default_value_t = 10.0)]
        apd_step_ms: f32,
        /// Mesh spacing of `niederer-slab`, mm.
        #[arg(long, default_value_t = 0.5)]
        dx_mm: f32,
    },
    /// Read solver results (VTU, legacy VTK, PVD series, IGB + activation .dat) onto the cell cloud by
    /// nearest point, writing cells_activation.csv, cells_vm.igb and import.json.
    ImportVtk {
        /// Directory of solver results.
        dir: PathBuf,
        /// Output directory (default: `<dir>/heartsim_import`).
        #[arg(short, long)]
        out: Option<PathBuf>,
        /// Anatomy pack directory (default: the procedural idealised heart).
        #[arg(long)]
        pack: Option<PathBuf>,
        /// Lattice spacing of the procedural heart, mm.
        #[arg(long, default_value_t = 2.3)]
        spacing_mm: f32,
        /// Mesh base name for .igb and .dat results (`<base>.pts`).
        #[arg(long)]
        mesh: Option<PathBuf>,
        /// Name of the V_m array in VTK files.
        #[arg(long)]
        vm_array: Option<String>,
        /// Name of the activation-time array in VTK files.
        #[arg(long)]
        lat_array: Option<String>,
        /// Time between the files of a series with no .pvd, ms.
        #[arg(long)]
        frame_dt_ms: Option<f64>,
        /// Unit of VTK point coordinates.
        #[arg(long, value_enum, default_value = "auto")]
        length_unit: bridge::import::LengthUnit,
        /// Threshold for timing activation from V_m when no activation array exists (default 0 mV, or 0.5 if normalised).
        #[arg(long)]
        lat_threshold: Option<f64>,
        /// A cell with no sample closer than this gets no value, mm (default 1.5 × the spacing).
        #[arg(long)]
        max_distance_mm: Option<f32>,
    },
    /// Compare heartsim's run of a scenario with an import: activation scatter, per-lead ECG overlay and a
    /// metrics table, as Markdown + PNG + JSON.
    ValidationReport {
        /// Scenario file or library id.
        scenario: String,
        /// Output directory of `import-vtk`.
        #[arg(long)]
        imported: PathBuf,
        /// Output directory (created if missing).
        #[arg(short, long)]
        out: PathBuf,
        /// Anatomy pack directory (default: the procedural idealised heart).
        #[arg(long)]
        pack: Option<PathBuf>,
        /// Lattice spacing of the procedural heart, mm.
        #[arg(long, default_value_t = 2.3)]
        spacing_mm: f32,
        /// heartsim engine to compare: 1 or 2.
        #[arg(long, value_enum, default_value = "1")]
        tier: bridge::report::Tier,
        /// Imported V_m at rest (default: the first frame's median).
        #[arg(long, allow_negative_numbers = true)]
        vm_rest: Option<f32>,
        /// Imported V_m at peak (default: the series maximum).
        #[arg(long, allow_negative_numbers = true)]
        vm_peak: Option<f32>,
    },
    /// Run the Niederer slab on the Tier-2 solver against the stored reference activation times and write
    /// a report. The model is calibrated phenomenological tissue, not TT06: read the report's first paragraph.
    Niederer {
        /// Spatial steps, mm.
        #[arg(long, value_delimiter = ',', default_value = "0.5")]
        dx_mm: Vec<f32>,
        /// Output directory (created if missing).
        #[arg(short, long)]
        out: PathBuf,
        /// A reference CSV to use instead of the stored one.
        #[arg(long)]
        reference: Option<PathBuf>,
        /// A solver's results for the same slab, to add to the table.
        #[arg(long)]
        external: Option<PathBuf>,
        /// Mesh base name for `--external` .igb and .dat results.
        #[arg(long)]
        mesh: Option<PathBuf>,
    },
}

fn validate_scenarios(dir: &Path) -> Result<()> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "ron"))
        .collect();
    files.sort();
    let mut ids = std::collections::BTreeSet::new();
    let mut failed = 0usize;
    for path in &files {
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let mut problems = Vec::new();
        let text = std::fs::read_to_string(path)?;
        match Tier0Scenario::from_ron(&text) {
            Err(e) => problems.push(e.to_string()),
            Ok(s) => {
                if s.scenario.id != stem {
                    problems.push(format!("id `{}` differs from the file name", s.scenario.id));
                }
                if !ids.insert(s.scenario.id.clone()) {
                    problems.push(format!("duplicate id `{}`", s.scenario.id));
                }
                problems.extend(s.content_problems());
                match s.validate_run() {
                    Err(e) => problems.push(e.to_string()),
                    Ok((a, v)) => {
                        let f = |x: Option<f64>| x.map_or_else(|| "-".to_string(), |x| format!("{x:.0}"));
                        println!(
                            "{stem:32} HR {:>4} PR {:>4} QRS {:>4} axis {:>4}",
                            f(a.hr_bpm),
                            f(a.pr_ms),
                            f(a.qrs_ms),
                            f(a.qrs_axis.map(|x| x.degrees))
                        );
                        problems.extend(v);
                    }
                }
            }
        }
        if !problems.is_empty() {
            failed += 1;
            for p in &problems {
                println!("  FAIL {stem}: {p}");
            }
        }
    }
    println!("{} scenarios, {failed} failed", files.len());
    if failed > 0 {
        anyhow::bail!("{failed} of {} scenarios failed validation", files.len());
    }
    Ok(())
}

/// Settings of `run`, as parsed from the command line.
struct RunArgs {
    scenario: String,
    seconds: f64,
    window: f64,
    speed: f32,
    gain: f32,
    paper: String,
    date: Option<String>,
    cabrera: bool,
    strip_lead: String,
    formats: String,
    out: PathBuf,
}

/// A scenario path, or an id looked up in `$HEARTSIM_ASSETS/scenarios`, `./assets/scenarios` and the source tree.
pub(crate) fn scenario_path(arg: &str) -> Result<PathBuf> {
    let p = PathBuf::from(arg);
    if p.is_file() {
        return Ok(p);
    }
    let mut roots: Vec<PathBuf> = std::env::var_os("HEARTSIM_ASSETS")
        .map(PathBuf::from)
        .into_iter()
        .collect();
    roots.push(PathBuf::from("assets"));
    roots.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets"));
    roots
        .into_iter()
        .map(|r| r.join("scenarios").join(format!("{arg}.ron")))
        .find(|p| p.is_file())
        .ok_or_else(|| anyhow::anyhow!("no scenario file or library id `{arg}`"))
}

fn parse_formats(list: &str) -> Result<heartsim_io::Formats> {
    let mut f = heartsim_io::Formats {
        pdf: false,
        png: false,
        csv: false,
        wfdb: false,
        ron: false,
        bundle: false,
    };
    for item in list.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        match item {
            "pdf" => f.pdf = true,
            "png" => f.png = true,
            "csv" => f.csv = true,
            "wfdb" => f.wfdb = true,
            "ron" => f.ron = true,
            "bundle" | "zip" => f.bundle = true,
            other => anyhow::bail!("unknown format `{other}` (use pdf,png,csv,wfdb,ron,bundle)"),
        }
    }
    Ok(f)
}

fn run(a: RunArgs) -> Result<()> {
    let path = scenario_path(&a.scenario)?;
    let s = Tier0Scenario::from_ron(&std::fs::read_to_string(&path)?)
        .map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
    let paper = match a.paper.to_ascii_lowercase().as_str() {
        "a4" => heartsim_io::Paper::A4,
        "letter" | "us-letter" => heartsim_io::Paper::Letter,
        other => anyhow::bail!("unknown paper `{other}` (a4 or letter)"),
    };
    let date = a.date.unwrap_or_else(heartsim_io::today);
    anyhow::ensure!(
        heartsim_io::unix_time_of_date(&date).is_some(),
        "--date must be YYYY-MM-DD, not `{date}`"
    );
    let strip_lead = Lead::ALL
        .into_iter()
        .find(|l| l.label().eq_ignore_ascii_case(&a.strip_lead))
        .ok_or_else(|| anyhow::anyhow!("unknown lead `{}`", a.strip_lead))?;
    let rec = s.record(a.seconds)?;
    let excerpt = heartsim_io::EcgRecord::from_recording(&rec, a.window, heartsim_io::EXPORT_RATE_HZ);
    let info = heartsim_io::ExportInfo {
        scenario_id: s.scenario.id.clone(),
        scenario_title: s.scenario.title.clone(),
        mm_per_s: a.speed,
        mm_per_mv: a.gain,
        date,
        paper,
    };
    let opts = heartsim_io::ExportOptions {
        formats: parse_formats(&a.formats)?,
        order: if a.cabrera {
            heartsim_ui::LeadOrder::Cabrera
        } else {
            heartsim_ui::LeadOrder::Standard
        },
        strip_lead,
    };
    let files = heartsim_io::export_files(&excerpt, &info, &s, &opts)?;
    for p in heartsim_io::write_files(&a.out, &files)? {
        eprintln!("wrote {}", p.display());
    }
    Ok(())
}

fn anatomy_info(dir: &Path) -> Result<()> {
    let t = std::time::Instant::now();
    let pack = AnatomyPack::load(dir)?;
    let load_time = t.elapsed();
    let m = &pack.manifest;
    println!(
        "pack      {} (format v{}) — {}",
        m.name,
        m.format_version,
        dir.display()
    );
    println!("source    {}", m.source);
    println!("licence   {}", m.licence);
    println!("frame     {}", m.frame);
    println!("loaded in {load_time:.2?}, {} files", m.files.len());

    let cells = &pack.cells;
    let (lo, hi) = cells.positions.iter().fold(
        (
            glam::Vec3::splat(f32::INFINITY),
            glam::Vec3::splat(f32::NEG_INFINITY),
        ),
        |(l, h), p| (l.min(*p), h.max(*p)),
    );
    println!("\ncells     {} (spacing {} mm)", cells.len(), m.cell_spacing_mm);
    println!(
        "bounds    x {:.1}..{:.1}  y {:.1}..{:.1}  z {:.1}..{:.1} mm",
        lo.x, hi.x, lo.y, hi.y, lo.z, hi.z
    );
    println!("\nregion                      cells   mean depth");
    for (r, n) in cells.region_counts() {
        let depth: f32 = cells
            .depth
            .iter()
            .zip(&cells.regions)
            .filter(|(_, x)| **x == r)
            .map(|(d, _)| *d)
            .sum::<f32>()
            / n as f32;
        println!("  {:<26} {:>6}   {:.2}", r.label(), n, depth);
    }
    let mut by_territory = std::collections::BTreeMap::new();
    for i in 0..cells.len() {
        *by_territory
            .entry(pack.territory(i as u32).label())
            .or_insert(0usize) += 1;
    }
    println!("\nterritory  {by_territory:?}");

    let n = cells.len().max(1) as f32;
    let nearest: f32 = cells.neighbours.iter().map(|nb| nb[0].distance_mm).sum::<f32>() / n;
    let farthest: f32 = cells
        .neighbours
        .iter()
        .map(|nb| nb[K_NEIGHBOURS - 1].distance_mm)
        .sum::<f32>()
        / n;
    let align: f32 = cells
        .neighbours
        .iter()
        .flatten()
        .map(|e| e.fibre_alignment)
        .sum::<f32>()
        / (n * K_NEIGHBOURS as f32);
    println!(
        "\nk-NN (k = {K_NEIGHBOURS})  mean nearest {nearest:.2} mm   mean k-th {farthest:.2} mm   mean |cos| fibre alignment {align:.3}"
    );

    println!("\nmeshes");
    for e in &m.meshes {
        let hoods = pack
            .hoods
            .get(&e.name)
            .map_or(String::from("-"), |h| h.len().to_string());
        println!(
            "  {:<18} {:?} {:>7} vertices {:>7} triangles  hoods {hoods}",
            e.name, e.kind, e.vertices, e.triangles
        );
    }
    println!(
        "  {:<18} torso    {:>7} vertices {:>7} triangles",
        "torso",
        pack.torso.vertex_count(),
        pack.torso.triangle_count()
    );

    println!("\nlandmarks");
    for (name, idx) in &pack.landmarks.points {
        let p = cells.positions[*idx as usize];
        println!(
            "  {:<14} cell {:>6}  ({:>6.1}, {:>6.1}, {:>6.1})  {}",
            name,
            idx,
            p.x,
            p.y,
            p.z,
            cells.regions[*idx as usize].label()
        );
    }
    println!("\ncell sets");
    for (name, set) in &pack.landmarks.sets {
        println!("  {:<14} {} cells", name, set.len());
    }
    println!("\nelectrodes (heart frame, mm)");
    for (name, p) in &pack.electrodes.sites {
        println!("  {:<4} ({:>7.1}, {:>7.1}, {:>7.1})", name, p[0], p[1], p[2]);
    }
    Ok(())
}

fn main() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::Synth {
            bpm,
            seconds,
            sample_rate,
            av_block,
            axis_shift,
            out,
        } => {
            let mut params = TimingParams::default();
            params.sinus.rate_bpm = bpm;
            params.av.complete_block = av_block;
            let mut eng = TimingEngine::new(params);
            let mut synth = GaussianSynth::textbook();
            synth.axis_shift_deg = axis_shift;
            let mut w = csv::Writer::from_path(&out)?;
            let mut header = vec!["t_ms".to_string()];
            header.extend(Lead::ALL.iter().map(|l| l.label().to_string()));
            w.write_record(&header)?;
            let dt = Millis(1.0);
            let period = 1000.0 / sample_rate;
            let mut next_sample = 0.0;
            let total = (seconds * 1000.0) as usize;
            for _ in 0..total {
                eng.step(dt);
                synth.on_events(&eng.drain_events());
                if eng.now().0 >= next_sample {
                    next_sample += period;
                    let s = synth.sample(eng.now());
                    let mut rec = vec![format!("{:.1}", eng.now().0)];
                    rec.extend(s.0.iter().map(|v| format!("{v:.4}")));
                    w.write_record(&rec)?;
                }
            }
            w.flush()?;
            eprintln!("wrote {}", out.display());
        }
        Cmd::DefaultScenario { out } => {
            std::fs::write(&out, Scenario::sinus_normal().to_ron()?)?;
            eprintln!("wrote {}", out.display());
        }
        Cmd::Run {
            scenario,
            seconds,
            window,
            speed,
            gain,
            paper,
            date,
            cabrera,
            strip_lead,
            formats,
            out,
        } => run(RunArgs {
            scenario,
            seconds,
            window,
            speed,
            gain,
            paper,
            date,
            cabrera,
            strip_lead,
            formats,
            out,
        })?,
        Cmd::AnatomyInfo { pack } => anatomy_info(&pack)?,
        Cmd::ValidateScenarios { dir } => validate_scenarios(&dir)?,
        Cmd::RunTutorial { tutorial, assets } => content::run_tutorials(&tutorial, assets.as_deref())?,
        Cmd::ValidateCases { dir } => content::validate_cases(&dir)?,
        Cmd::ExportOpencarp {
            scenario,
            out,
            pack,
            spacing_mm,
            ionic,
            tend_ms,
            stim_radius_mm,
            apd_step_ms,
            dx_mm,
        } => bridge::export::run(&bridge::export::ExportArgs {
            scenario,
            out,
            pack,
            spacing_mm,
            ionic,
            tend_ms,
            stim_radius_mm,
            apd_step_ms,
            dx_mm,
        })?,
        Cmd::ImportVtk {
            dir,
            out,
            pack,
            spacing_mm,
            mesh,
            vm_array,
            lat_array,
            frame_dt_ms,
            length_unit,
            lat_threshold,
            max_distance_mm,
        } => bridge::import::run(&bridge::import::ImportArgs {
            dir,
            out,
            pack,
            spacing_mm,
            max_distance_mm,
            source: bridge::import::SourceOptions {
                mesh,
                vm_array,
                lat_array,
                frame_dt_ms,
                length_unit,
                lat_threshold,
            },
        })?,
        Cmd::ValidationReport {
            scenario,
            imported,
            out,
            pack,
            spacing_mm,
            tier,
            vm_rest,
            vm_peak,
        } => bridge::report::run(&bridge::report::ReportArgs {
            scenario,
            imported,
            out,
            pack,
            spacing_mm,
            tier,
            vm_rest,
            vm_peak,
        })?,
        Cmd::Niederer {
            dx_mm,
            out,
            reference,
            external,
            mesh,
        } => bridge::niederer::run(&bridge::niederer::NiedererArgs {
            dx_mm,
            out,
            reference,
            external,
            mesh,
        })?,
    }
    Ok(())
}
