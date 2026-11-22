//! `heartsim-cli calc`: the echo calculators of `heartsim-calc` on the command line (brief B29).
//!
//! `calc list` prints the registry; `calc <id> key=value …` validates and computes exactly as the
//! app's page does and prints the outcome as text or, with `--json`, as JSON. Invalid input is an
//! error (non-zero exit) that names the field.

use anyhow::{Result, bail};
use heartsim_calc::{
    ALL, Calculator, ErrorKind, InputError, InputKind, Inputs, Outcome, Review, Severity, Value, by_id,
};
use serde_json::{Value as Json, json};
use std::fmt::Write as _;

/// The educational note printed under every outcome.
const NOTE_EN: &str = "For education only: not a medical device and not a diagnosis.";
const NOTE_IT: &str = "Solo per la didattica: non è un dispositivo medico né una diagnosi.";

/// Runs the sub-command and returns what to print on standard output.
pub fn run(args: &[String], json_out: bool, example: bool, lang: &str) -> Result<String> {
    let Some(first) = args.first() else {
        bail!("usage: heartsim-cli calc list | calc <id> key=value … [--json] [--example] [--lang it]");
    };
    if first == "list" {
        return Ok(list(json_out, lang));
    }
    let Some(c) = by_id(first) else {
        bail!("no calculator `{first}` (see `heartsim-cli calc list`)");
    };
    let mut inputs = if example {
        c.example_inputs()
    } else {
        Inputs::new()
    };
    let mut errors = Vec::new();
    for arg in &args[1..] {
        match parse_pair(c, arg) {
            Ok((id, v)) => inputs.set(id, v),
            Err(e) => errors.push(e),
        }
    }
    if errors.is_empty() {
        match c.evaluate(&inputs) {
            Ok(out) => {
                return Ok(if json_out {
                    serde_json::to_string_pretty(&outcome_json(c, &out, lang)).unwrap_or_default()
                } else {
                    outcome_text(c, &out, lang)
                });
            }
            Err(e) => errors = e,
        }
    }
    let lines: Vec<String> = errors
        .iter()
        .map(|e| match e.input {
            Some(id) => format!("{id}: {}", e.message(lang)),
            None => e.message(lang),
        })
        .collect();
    bail!("{}", lines.join("\n"))
}

/// `key=value` for one input: a number, or a choice id.
fn parse_pair(c: &Calculator, arg: &str) -> std::result::Result<(&'static str, Value), InputError> {
    let (key, value) = arg.split_once('=').unwrap_or((arg, ""));
    let Some(spec) = c.input(key.trim()) else {
        return Err(InputError {
            input: None,
            kind: ErrorKind::Inconsistent(heartsim_calc::Message {
                en: format!("unknown input `{key}` (inputs: {})", input_ids(c)),
                it: format!("campo sconosciuto `{key}` (campi: {})", input_ids(c)),
            }),
        });
    };
    let value = value.trim();
    match spec.kind {
        InputKind::Choice(_) => Ok((spec.id, Value::Choice(value.to_owned()))),
        InputKind::Number { .. } => value
            .replace(',', ".")
            .parse::<f64>()
            .map(|v| (spec.id, Value::Number(v)))
            .map_err(|_| InputError {
                input: Some(spec.id),
                kind: ErrorKind::NotANumber,
            }),
    }
}

fn input_ids(c: &Calculator) -> String {
    c.inputs.iter().map(|i| i.id).collect::<Vec<_>>().join(", ")
}

fn review_id(r: Review) -> &'static str {
    match r {
        Review::Verified => "verified",
        Review::FormulaOnly => "formula-only",
    }
}

fn severity_id(s: Severity) -> &'static str {
    match s {
        Severity::Normal => "normal",
        Severity::Mild => "mild",
        Severity::Moderate => "moderate",
        Severity::Severe => "severe",
        Severity::Info => "info",
        Severity::Abnormal => "abnormal",
        Severity::Indeterminate => "indeterminate",
    }
}

fn list(json_out: bool, lang: &str) -> String {
    if json_out {
        let rows: Vec<Json> = ALL
            .iter()
            .map(|c| {
                json!({
                    "id": c.id,
                    "category": c.category.name().get(lang),
                    "name": c.name.get(lang),
                    "review": review_id(c.review),
                })
            })
            .collect();
        return serde_json::to_string_pretty(&rows).unwrap_or_default();
    }
    let mut s = String::new();
    for c in ALL {
        let _ = writeln!(
            s,
            "{:<36} {:<13} {:<34} {}",
            c.id,
            review_id(c.review),
            c.category.name().get(lang),
            c.name.get(lang)
        );
    }
    s
}

/// The outcome as JSON: the values with full precision, the steps, the findings with their source.
pub fn outcome_json(c: &Calculator, out: &Outcome, lang: &str) -> Json {
    json!({
        "id": c.id,
        "name": c.name.get(lang),
        "review": review_id(c.review),
        "values": out.values.iter().map(|q| json!({
            "id": q.id,
            "label": q.label.get(lang),
            "value": q.value,
            "unit": q.unit.symbol(),
            "display": q.display(),
        })).collect::<Vec<_>>(),
        "steps": out.steps.iter().map(|s| json!({
            "formula": s.formula,
            "substituted": s.substituted,
            "result": s.result,
        })).collect::<Vec<_>>(),
        "findings": out.findings.iter().map(|f| json!({
            "severity": severity_id(f.severity),
            "text": f.text.get(lang),
            "criterion": f.criterion,
            "source": f.source.reference.journal,
            "locator": f.source.locator,
        })).collect::<Vec<_>>(),
        "references": c.references.iter().map(|r| json!({
            "authors": r.reference.authors,
            "title": r.reference.title,
            "journal": r.reference.journal,
            "doi": r.reference.doi,
            "locator": r.locator,
        })).collect::<Vec<_>>(),
        "note": if lang == "it" { NOTE_IT } else { NOTE_EN },
    })
}

fn outcome_text(c: &Calculator, out: &Outcome, lang: &str) -> String {
    let mut s = format!(
        "{}  [{}]  {}\n\n",
        c.name.get(lang),
        c.id,
        c.review.label().get(lang)
    );
    for (q, step) in out.values.iter().zip(&out.steps) {
        let _ = write!(
            s,
            "{}: {}\n    {}  {}  {}\n",
            q.label.get(lang),
            q.display(),
            step.formula,
            step.substituted,
            step.result
        );
    }
    if !out.findings.is_empty() {
        s.push('\n');
        for f in &out.findings {
            let _ = writeln!(
                s,
                "[{}] {} ({}) — {}, {}",
                severity_id(f.severity),
                f.text.get(lang),
                f.criterion,
                f.source.reference.journal,
                f.source.locator
            );
        }
    }
    s.push('\n');
    for r in c.references {
        let doi = r.reference.doi.map(|d| format!(" doi:{d}")).unwrap_or_default();
        let _ = writeln!(
            s,
            "{} {}. {}.{doi} — {}",
            r.reference.authors, r.reference.title, r.reference.journal, r.locator
        );
    }
    let _ = write!(s, "\n{}\n", if lang == "it" { NOTE_IT } else { NOTE_EN });
    s
}
