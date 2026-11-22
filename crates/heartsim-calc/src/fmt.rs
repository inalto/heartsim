//! Number formatting for results and worked formulas (a point as the decimal separator; the page
//! swaps it for the language's).

use crate::model::Unit;

/// `v` with `decimals` decimals.
#[must_use]
pub fn num(v: f64, decimals: u8) -> String {
    let d = usize::from(decimals);
    let s = format!("{v:.d$}");
    // "-0.0" reads as a sign error
    if s.starts_with('-') && s[1..].chars().all(|c| c == '0' || c == '.') {
        s[1..].to_owned()
    } else {
        s
    }
}

/// `v` without trailing zeros (for range limits: "0.5", "300").
#[must_use]
pub fn trim(v: f64) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_owned()
}

/// `v` with `decimals` decimals and the unit symbol after a space (none for a pure number).
#[must_use]
pub fn with_unit(v: f64, decimals: u8, unit: Unit) -> String {
    let n = num(v, decimals);
    match unit.symbol() {
        "" => n,
        "%" | "°" => format!("{n}{}", unit.symbol()),
        s => format!("{n} {s}"),
    }
}
