use anyhow::Result;
use clap::{Parser, Subcommand};
use heartsim_core::{Lead, Millis, Scenario};
use heartsim_ecg::GaussianSynth;
use heartsim_sim::{Engine, TimingEngine, TimingParams};
use std::path::PathBuf;

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
    }
    Ok(())
}
