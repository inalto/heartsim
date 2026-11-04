//! WFDB records (PhysioNet): a `.hea` text header plus a `.dat` file of interleaved little-endian
//! 16-bit samples ("format 16"), 1000 ADC units per mV (1 µV per unit), baseline 0. This is the
//! layout of PTB-XL and most PhysioNet 12-lead databases, so the files open in `wfdb` (Python),
//! WAVE, LightWAVE and EDF converters.
//!
//! The reader accepts format-16 records whose signals are named by lead label (the format this
//! module writes, and the common PhysioNet 12-lead layout); other formats are rejected with a
//! message rather than misread.

use crate::ImportError;
use crate::csv::lead_of_label;
use crate::record::EcgRecord;
use heartsim_core::{LEAD_COUNT, Lead};
use std::fmt::Write as _;

/// ADC units per millivolt of the export.
pub const WFDB_GAIN_ADU_PER_MV: f64 = 1000.0;
/// WFDB's "no sample" value in format 16; exports clamp one above it.
const INVALID_SAMPLE: i16 = i16::MIN;

/// A written record: header text and sample bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WfdbRecord {
    /// Record name: the header and data files are `<name>.hea` and `<name>.dat`.
    pub name: String,
    pub header: String,
    pub data: Vec<u8>,
}

fn to_adu(mv: f32) -> i16 {
    let adu = (f64::from(mv) * WFDB_GAIN_ADU_PER_MV).round();
    adu.clamp(f64::from(INVALID_SAMPLE) + 1.0, f64::from(i16::MAX)) as i16
}

/// WFDB record `name` (letters, digits, `_` and `-` only) holding `rec`. `comments` become `#`
/// lines after the signal specifications.
#[must_use]
pub fn write_wfdb(rec: &EcgRecord, name: &str, comments: &[String]) -> WfdbRecord {
    let n = rec.len();
    let mut data = Vec::with_capacity(n * LEAD_COUNT * 2);
    let mut sums = [0i64; LEAD_COUNT];
    for i in 0..n {
        for (l, lead) in rec.leads.iter().enumerate() {
            let adu = to_adu(lead[i]);
            sums[l] += i64::from(adu);
            data.extend_from_slice(&adu.to_le_bytes());
        }
    }
    let fs = rec.sample_rate_hz;
    let fs_text = if (fs - fs.round()).abs() < 1e-9 {
        format!("{fs:.0}")
    } else {
        format!("{fs}")
    };
    let mut header = format!("{name} {LEAD_COUNT} {fs_text} {n}\n");
    for (l, lead) in Lead::ALL.iter().zip(&rec.leads) {
        let init = lead.first().map_or(0, |v| to_adu(*v));
        // 16-bit two's-complement sum of the samples, as the WFDB library computes it
        let checksum = (sums[*l as usize].rem_euclid(65_536) as u16).cast_signed();
        let _ = writeln!(
            header,
            "{name}.dat 16 {WFDB_GAIN_ADU_PER_MV:.0}/mV 16 0 {init} {checksum} 0 {}",
            l.label()
        );
    }
    for c in comments {
        for line in c.lines() {
            let _ = writeln!(header, "# {line}");
        }
    }
    WfdbRecord {
        name: name.to_string(),
        header,
        data,
    }
}

/// One signal specification line of a header.
#[derive(Debug, Clone)]
struct SignalSpec {
    file: String,
    gain_adu_per_unit: f64,
    baseline_adu: f64,
    /// Multiplier from the header's physical unit to mV.
    unit_to_mv: f64,
    description: String,
}

fn bad(msg: impl Into<String>) -> ImportError {
    ImportError::Format(msg.into())
}

fn parse_signal(line: &str) -> Result<SignalSpec, ImportError> {
    let f: Vec<&str> = line.split_whitespace().collect();
    let file = (*f.first().ok_or_else(|| bad("empty signal line"))?).to_string();
    let format = f.get(1).copied().unwrap_or("");
    // Exactly "16": samples-per-frame, skew and byte-offset modifiers would change the layout.
    if format != "16" {
        return Err(bad(format!("WFDB format {format} is not supported (only 16)")));
    }
    let adc_zero: f64 = f.get(4).and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let (mut gain, mut baseline, mut units) = (200.0, adc_zero, "mV".to_string());
    if let Some(g) = f.get(2) {
        let (num, rest) = g.split_once('/').map_or((*g, None), |(a, b)| (a, Some(b)));
        if let Some(u) = rest {
            units = u.to_string();
        }
        let (gain_text, base_text) = match num.split_once('(') {
            Some((a, b)) => (a, Some(b.trim_end_matches(')'))),
            None => (num, None),
        };
        let gain_val: f64 = gain_text.parse().map_err(|_| bad(format!("bad gain `{g}`")))?;
        if gain_val != 0.0 {
            gain = gain_val;
        }
        if let Some(b) = base_text {
            baseline = b.parse().map_err(|_| bad(format!("bad baseline `{g}`")))?;
        }
    }
    let unit_to_mv = match units.to_ascii_lowercase().as_str() {
        "mv" => 1.0,
        "uv" | "µv" => 1e-3,
        "v" => 1e3,
        other => return Err(bad(format!("unit `{other}` is not a voltage"))),
    };
    let description = f.get(8..).map(|d| d.join(" ")).unwrap_or_default();
    Ok(SignalSpec {
        file,
        gain_adu_per_unit: gain,
        baseline_adu: baseline,
        unit_to_mv,
        description,
    })
}

/// Comment lines of a header (`#` stripped), e.g. the scenario this module writes there.
#[must_use]
pub fn header_comments(header: &str) -> Vec<String> {
    header
        .lines()
        .filter_map(|l| l.trim_start().strip_prefix('#'))
        .map(|c| c.trim().to_string())
        .collect()
}

/// Data file names a header refers to, in order of first use (so the caller can load them).
#[must_use]
pub fn data_files(header: &str) -> Vec<String> {
    let mut files: Vec<String> = Vec::new();
    for line in header
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .skip(1)
    {
        if let Some(f) = line.split_whitespace().next()
            && !files.iter().any(|x| x == f)
        {
            files.push(f.to_string());
        }
    }
    files
}

/// Read a format-16 record. `data_for(file)` returns the bytes of each data file the header names
/// (see [`data_files`]). Signals are matched to leads by their description; all twelve leads must be
/// present (extra signals are ignored).
pub fn read_wfdb(
    header: &str,
    mut data_for: impl FnMut(&str) -> Result<Vec<u8>, ImportError>,
) -> Result<EcgRecord, ImportError> {
    let mut lines = header
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'));
    let record_line = lines.next().ok_or(ImportError::Empty)?;
    let r: Vec<&str> = record_line.split_whitespace().collect();
    if r.first().is_some_and(|n| n.contains('/')) {
        return Err(bad("multi-segment WFDB records are not supported"));
    }
    let nsig: usize = r
        .get(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| bad("record line has no signal count"))?;
    let fs: f64 = r
        .get(2)
        .copied()
        .map(|s| s.split(['/', '(']).next().unwrap_or(s))
        .map_or(Ok(250.0), str::parse::<f64>)
        .map_err(|_| bad("bad sampling frequency"))?;
    if !(fs > 0.0 && fs.is_finite()) {
        return Err(bad(format!("sampling frequency {fs} is not positive")));
    }
    let nsamp: Option<usize> = r.get(3).and_then(|s| s.parse().ok());
    let specs: Vec<SignalSpec> = lines.take(nsig).map(parse_signal).collect::<Result<_, _>>()?;
    if specs.len() != nsig {
        return Err(bad(format!(
            "header announces {nsig} signals but lists {}",
            specs.len()
        )));
    }
    // Signals sharing a file are interleaved frame by frame in header order.
    let mut leads: Vec<Option<Vec<f32>>> = vec![None; LEAD_COUNT];
    for file in data_files(header) {
        let members: Vec<usize> = (0..nsig).filter(|&s| specs[s].file == file).collect();
        let bytes = data_for(&file)?;
        let frame = members.len() * 2;
        let frames = nsamp.unwrap_or(bytes.len() / frame).min(bytes.len() / frame);
        for (k, &s) in members.iter().enumerate() {
            let Some(lead) = lead_of_label(&specs[s].description) else {
                continue;
            };
            let spec = &specs[s];
            let samples = (0..frames)
                .map(|i| {
                    let o = i * frame + k * 2;
                    let adu = i16::from_le_bytes([bytes[o], bytes[o + 1]]);
                    if adu == INVALID_SAMPLE {
                        0.0
                    } else {
                        ((f64::from(adu) - spec.baseline_adu) / spec.gain_adu_per_unit * spec.unit_to_mv)
                            as f32
                    }
                })
                .collect();
            leads[lead as usize].get_or_insert(samples);
        }
    }
    let leads: Vec<Vec<f32>> = Lead::ALL
        .into_iter()
        .zip(leads)
        .map(|(l, s)| s.ok_or_else(|| bad(format!("record has no {} signal", l.label()))))
        .collect::<Result<_, _>>()?;
    let n = leads.iter().map(Vec::len).min().unwrap_or(0);
    if n == 0 {
        return Err(ImportError::Empty);
    }
    let start_ms = header_comments(header)
        .iter()
        .find_map(|c| c.strip_prefix("start_ms:").and_then(|v| v.trim().parse().ok()))
        .unwrap_or(0.0);
    Ok(EcgRecord {
        sample_rate_hz: fs,
        leads: leads
            .into_iter()
            .map(|mut l| {
                l.truncate(n);
                l
            })
            .collect(),
        start_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_record() -> EcgRecord {
        let mut rec = EcgRecord::empty(500.0);
        for (l, lead) in rec.leads.iter_mut().enumerate() {
            *lead = (0..40)
                .map(|i| (i as f32 - 20.0) * 0.05 * (l as f32 + 1.0))
                .collect();
        }
        rec.leads[3][0] = 50.0; // clamps
        rec.start_ms = 1234.0;
        rec
    }

    #[test]
    fn header_follows_the_wfdb_layout() {
        let w = write_wfdb(&sample_record(), "rec_1", &["start_ms: 1234".into()]);
        let lines: Vec<&str> = w.header.lines().collect();
        assert_eq!(lines[0], "rec_1 12 500 40");
        assert!(
            lines[1].starts_with("rec_1.dat 16 1000/mV 16 0 -1000 "),
            "{}",
            lines[1]
        );
        assert!(lines[1].ends_with(" 0 I"));
        assert!(lines[12].ends_with(" V6"));
        assert_eq!(lines[13], "# start_ms: 1234");
        assert_eq!(w.data.len(), 40 * 12 * 2);
        // aVR's first sample is clamped to the largest valid value
        assert_eq!(i16::from_le_bytes([w.data[6], w.data[7]]), i16::MAX);
    }

    #[test]
    fn checksum_is_the_16_bit_sum() {
        let w = write_wfdb(&sample_record(), "r", &[]);
        let lead_i: i64 = (0..40)
            .map(|i| ((f64::from(i) - 20.0) * 50.0).round() as i64)
            .sum();
        let cs: i16 = w
            .header
            .lines()
            .nth(1)
            .unwrap()
            .split_whitespace()
            .nth(6)
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(
            i64::from(cs),
            lead_i.rem_euclid(65_536)
                - if lead_i.rem_euclid(65_536) > 32_767 {
                    65_536
                } else {
                    0
                }
        );
    }

    #[test]
    fn round_trip() {
        let rec = sample_record();
        let w = write_wfdb(&rec, "r", &["start_ms: 1234".into()]);
        assert_eq!(data_files(&w.header), ["r.dat"]);
        let back = read_wfdb(&w.header, |_| Ok(w.data.clone())).unwrap();
        assert_eq!(back.len(), 40);
        assert!((back.start_ms - 1234.0).abs() < 1e-9);
        for (l, (a, b)) in rec.leads.iter().zip(&back.leads).enumerate() {
            for (x, y) in a.iter().zip(b) {
                let want = x.clamp(-32.767, 32.767);
                assert!((want - y).abs() <= 0.000_5 + 1e-6, "lead {l}: {x} vs {y}");
            }
        }
    }

    #[test]
    fn microvolt_units_and_baselines_are_converted() {
        let mut header = String::from("x 12 250 2\n");
        for l in Lead::ALL {
            let _ = writeln!(header, "x.dat 16 2(10)/uV 16 0 0 0 0 {}", l.label());
        }
        let data: Vec<u8> = (0..24).flat_map(|_| 30i16.to_le_bytes()).collect();
        let rec = read_wfdb(&header, |_| Ok(data.clone())).unwrap();
        // (30 − 10) / 2 = 10 µV
        assert!((rec.lead(Lead::V3)[1] - 0.01).abs() < 1e-7);
        assert!((rec.sample_rate_hz - 250.0).abs() < 1e-12);
    }

    #[test]
    fn other_formats_and_rates_are_rejected_rather_than_misread() {
        // 16+24 is format 16 with a 24-byte header, 16x2 has two samples per frame
        for format in ["212", "16+24", "16x2", "16:1"] {
            let mut header = String::from("x 12 500 2\n");
            for l in Lead::ALL {
                let _ = writeln!(header, "x.dat {format} 200/mV 12 0 0 0 0 {}", l.label());
            }
            assert!(
                matches!(
                    read_wfdb(&header, |_| Ok(vec![0; 48])),
                    Err(ImportError::Format(_))
                ),
                "{format}"
            );
        }
        let mut header = String::from("x 12 0 2\n");
        for l in Lead::ALL {
            let _ = writeln!(header, "x.dat 16 200/mV 16 0 0 0 0 {}", l.label());
        }
        assert!(matches!(
            read_wfdb(&header, |_| Ok(vec![0; 48])),
            Err(ImportError::Format(_))
        ));
    }
}
