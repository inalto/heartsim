//! Plain-text 12-lead CSV: `t_ms,I,II,III,aVR,aVL,aVF,V1,…,V6`, time from the start of the excerpt,
//! values in mV with 1 µV resolution (the WFDB export's resolution, so both files carry the same
//! numbers). Negative zero is printed as zero so the text is identical on every platform.

use crate::ImportError;
use crate::record::EcgRecord;
use heartsim_core::{LEAD_COUNT, Lead};
use std::fmt::Write as _;

/// Decimal places of the voltage columns (1 µV).
pub const CSV_DECIMALS: usize = 3;

fn push_value(out: &mut String, v: f32) {
    let s = format!("{v:.CSV_DECIMALS$}");
    let zero = s.trim_start_matches('-').chars().all(|c| c == '0' || c == '.');
    out.push_str(if zero { s.trim_start_matches('-') } else { &s });
}

/// The CSV text of `rec`.
#[must_use]
pub fn write_csv(rec: &EcgRecord) -> String {
    write_csv_noted(rec, None)
}

/// The CSV text of `rec`, led by a `# trace: <note>` comment line when the trace is not the clean
/// engine signal (the display filter and artefacts it went through). [`read_csv`] skips `#` lines.
#[must_use]
pub fn write_csv_noted(rec: &EcgRecord, trace_note: Option<&str>) -> String {
    let period_ms = 1000.0 / rec.sample_rate_hz;
    let whole_ms = (period_ms - period_ms.round()).abs() < 1e-9;
    let mut out = String::with_capacity(rec.len() * 12 * 8 + 64);
    if let Some(note) = trace_note {
        out.push_str("# trace: ");
        out.push_str(&note.replace(['\n', '\r'], " "));
        out.push('\n');
    }
    out.push_str("t_ms");
    for l in Lead::ALL {
        out.push(',');
        out.push_str(l.label());
    }
    out.push('\n');
    for i in 0..rec.len() {
        let t = i as f64 * period_ms;
        if whole_ms {
            let _ = write!(out, "{t:.0}");
        } else {
            let _ = write!(out, "{t:.3}");
        }
        for lead in &rec.leads {
            out.push(',');
            push_value(&mut out, lead[i]);
        }
        out.push('\n');
    }
    out
}

/// Which lead a column header names (case-insensitive, so `AVR` and `v1` are accepted).
pub(crate) fn lead_of_label(label: &str) -> Option<Lead> {
    let label = label.trim().trim_matches('"');
    Lead::ALL
        .into_iter()
        .find(|l| l.label().eq_ignore_ascii_case(label))
}

/// Read a 12-lead CSV: a time column first (`t_ms`, or `t_s` / `time` in seconds) and one column
/// per lead, named by its label in any order and case. All twelve leads must be present; the
/// sampling rate is taken from the time column.
pub fn read_csv(text: &str) -> Result<EcgRecord, ImportError> {
    let mut lines = text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'));
    let header = lines.next().ok_or(ImportError::Empty)?;
    let names: Vec<&str> = header.split(',').map(str::trim).collect();
    let time_scale_ms = match names.first().map(|n| n.trim_matches('"').to_ascii_lowercase()) {
        Some(n) if n == "t_ms" || n == "time_ms" => 1.0,
        Some(n) if n == "t_s" || n == "time" || n == "time_s" => 1000.0,
        _ => {
            return Err(ImportError::Format(
                "the first CSV column must be t_ms or t_s".into(),
            ));
        }
    };
    let mut column_of = [usize::MAX; LEAD_COUNT];
    for (c, n) in names.iter().enumerate().skip(1) {
        if let Some(l) = lead_of_label(n) {
            column_of[l as usize] = c;
        }
    }
    if let Some(missing) = Lead::ALL
        .into_iter()
        .find(|l| column_of[*l as usize] == usize::MAX)
    {
        return Err(ImportError::Format(format!(
            "CSV has no {} column",
            missing.label()
        )));
    }
    let mut times = Vec::new();
    let mut leads = vec![Vec::new(); LEAD_COUNT];
    for (row, line) in lines.enumerate() {
        let cells: Vec<&str> = line.split(',').map(str::trim).collect();
        let num = |c: usize| -> Result<f64, ImportError> {
            cells.get(c).and_then(|s| s.parse::<f64>().ok()).ok_or_else(|| {
                ImportError::Format(format!("row {}: column {} is not a number", row + 2, c + 1))
            })
        };
        times.push(num(0)? * time_scale_ms);
        for (l, lead) in leads.iter_mut().enumerate() {
            lead.push(num(column_of[l])? as f32);
        }
    }
    if times.len() < 2 {
        return Err(ImportError::Empty);
    }
    let period_ms = (times[times.len() - 1] - times[0]) / (times.len() - 1) as f64;
    if !(period_ms > 0.0 && period_ms.is_finite()) {
        return Err(ImportError::Format("the time column does not increase".into()));
    }
    // Round to a whole rate in Hz: the time column is printed with limited precision.
    let sample_rate_hz = (1000.0 / period_ms).round();
    if sample_rate_hz < 1.0 {
        return Err(ImportError::Format(format!(
            "a sample every {period_ms:.0} ms is not an ECG (is the time column in the unit its name says?)"
        )));
    }
    Ok(EcgRecord {
        sample_rate_hz,
        leads,
        start_ms: 0.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_round_trips_at_microvolt_resolution() {
        let mut rec = EcgRecord::empty(500.0);
        for (l, lead) in rec.leads.iter_mut().enumerate() {
            *lead = (0..50)
                .map(|i| ((i * 7 + l * 3) % 23) as f32 * 0.0137 - 0.15)
                .collect();
        }
        let text = write_csv(&rec);
        assert!(text.starts_with("t_ms,I,II,III,aVR,aVL,aVF,V1,V2,V3,V4,V5,V6\n0,"));
        assert!(!text.contains("-0.000"));
        let back = read_csv(&text).unwrap();
        assert!((back.sample_rate_hz - 500.0).abs() < 1e-9);
        for (a, b) in rec.leads.iter().zip(&back.leads) {
            assert_eq!(a.len(), b.len());
            assert!(a.iter().zip(b).all(|(x, y)| (x - y).abs() <= 0.000_5 + 1e-6));
        }
    }

    #[test]
    fn columns_are_matched_by_label_in_any_order_and_case() {
        let labels = [
            "v6", "V5", "v4", "v3", "v2", "v1", "AVF", "avl", "avr", "iii", "ii", "i",
        ];
        let mut text = format!("t_s,{}\n", labels.join(","));
        for i in 0..3 {
            let row: Vec<String> = (0..12).map(|c| format!("{}", c as f32 * 0.1)).collect();
            let _ = writeln!(text, "{},{}", f64::from(i) * 0.004, row.join(","));
        }
        let rec = read_csv(&text).unwrap();
        assert!((rec.sample_rate_hz - 250.0).abs() < 1e-9);
        assert!((rec.lead(Lead::I)[0] - 1.1).abs() < 1e-6);
        assert!((rec.lead(Lead::V6)[0]).abs() < 1e-6);
    }

    #[test]
    fn implausible_sampling_rate_is_an_error() {
        let labels: Vec<&str> = Lead::ALL.iter().map(|l| l.label()).collect();
        // minutes written under a `t_s` header: one sample every 3 "seconds"
        let zeros = ["0"; 12].join(",");
        let text = format!("t_s,{}\n0,{zeros}\n3,{zeros}\n", labels.join(","));
        assert!(matches!(read_csv(&text), Err(ImportError::Format(_))));
    }

    #[test]
    fn missing_lead_is_an_error() {
        let text = "t_ms,I,II\n0,0,0\n2,0,0\n";
        assert!(matches!(read_csv(text), Err(ImportError::Format(_))));
    }
}
