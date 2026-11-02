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
    /// Load an anatomy pack and print counts, bounds, per-region cells, k-NN statistics, landmarks
    /// and electrodes.
    AnatomyInfo {
        /// Pack directory (contains `manifest.ron`).
        pack: PathBuf,
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
        Cmd::AnatomyInfo { pack } => anatomy_info(&pack)?,
        Cmd::ValidateScenarios { dir } => validate_scenarios(&dir)?,
    }
    Ok(())
}
