//! Graded tables as data: a scale of consecutive bands with the side each boundary falls on, read
//! from the guideline table it cites. One scale grades one quantity.

use crate::fmt;
use crate::model::{Cite, Finding, Message, Severity, Text, Unit};

/// One band of a scale: values up to `upto` (inclusive or not) that are not in an earlier band.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Band {
    /// Upper limit.
    pub upto: f64,
    /// Whether `upto` itself belongs to this band.
    pub inclusive: bool,
    /// Strength of the band.
    pub severity: Severity,
    /// The source's name for the band ("moderate").
    pub label: Text,
}

/// A graded quantity: ascending bands, then the band above the last limit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scale {
    /// The quantity in words, as the start of a sentence ("Aortic valve area").
    pub what: Text,
    /// Its symbol in the criterion ("AVA").
    pub symbol: &'static str,
    /// Unit of the limits.
    pub unit: Unit,
    /// Decimals of the limits in the criterion.
    pub decimals: u8,
    /// Bands in ascending order of `upto`.
    pub bands: &'static [Band],
    /// The band above the last limit: severity and label.
    pub above: (Severity, Text),
    /// Table and page.
    pub source: Cite,
}

/// Shorthand for a [`Band`].
#[must_use]
pub const fn band(upto: f64, inclusive: bool, severity: Severity, label: Text) -> Band {
    Band {
        upto,
        inclusive,
        severity,
        label,
    }
}

impl Scale {
    /// Index of the band `v` falls in (`bands.len()` for the top band).
    #[must_use]
    pub fn index(&self, v: f64) -> usize {
        self.bands
            .iter()
            .position(|b| if b.inclusive { le(v, b.upto) } else { lt(v, b.upto) })
            .unwrap_or(self.bands.len())
    }

    /// The criterion of band `i` written out ("1.0 ≤ AVA ≤ 1.5 cm²").
    #[must_use]
    pub fn criterion(&self, i: usize) -> String {
        let u = match self.unit.symbol() {
            "" => String::new(),
            "%" => "%".to_owned(),
            s => format!(" {s}"),
        };
        let lim = |x: f64| fmt::num(x, self.decimals);
        let lower = i.checked_sub(1).map(|j| self.bands[j]);
        let upper = self.bands.get(i);
        match (lower, upper) {
            (None, Some(b)) => format!(
                "{} {} {}{u}",
                self.symbol,
                if b.inclusive { "≤" } else { "<" },
                lim(b.upto)
            ),
            (Some(a), None) => format!(
                "{} {} {}{u}",
                self.symbol,
                if a.inclusive { ">" } else { "≥" },
                lim(a.upto)
            ),
            (Some(a), Some(b)) => format!(
                "{} {} {} {} {}{u}",
                lim(a.upto),
                if a.inclusive { "<" } else { "≤" },
                self.symbol,
                if b.inclusive { "≤" } else { "<" },
                lim(b.upto)
            ),
            (None, None) => self.symbol.to_owned(),
        }
    }

    /// Severity and label of the band `v` falls in.
    #[must_use]
    pub fn class(&self, v: f64) -> (Severity, Text) {
        let i = self.index(v);
        self.bands.get(i).map_or(self.above, |b| (b.severity, b.label))
    }

    /// The finding for `v`: the band it falls in, with its criterion and source.
    #[must_use]
    pub fn finding(&self, v: f64) -> Finding {
        let i = self.index(v);
        let (severity, label) = self.class(v);
        Finding {
            severity,
            text: Message {
                en: format!("{} in the range the source grades “{}”", self.what.en, label.en),
                it: format!(
                    "{} nell'intervallo che la fonte classifica “{}”",
                    self.what.it, label.it
                ),
            },
            criterion: self.criterion(i),
            source: self.source,
        }
    }
}

/// Tolerance of a boundary comparison, relative to the limit: a value computed as 2 × 1.05 ÷ 5
/// must count as the 0.42 the table prints, not as a hair above it.
fn eps(limit: f64) -> f64 {
    1e-9 * limit.abs().max(1.0)
}

/// `v < limit`, with `v` within rounding of `limit` counted as equal.
#[must_use]
pub fn lt(v: f64, limit: f64) -> bool {
    v < limit - eps(limit)
}

/// `v ≤ limit`, with `v` within rounding of `limit` counted as equal.
#[must_use]
pub fn le(v: f64, limit: f64) -> bool {
    v <= limit + eps(limit)
}

/// `v > limit`, with `v` within rounding of `limit` counted as equal.
#[must_use]
pub fn gt(v: f64, limit: f64) -> bool {
    !le(v, limit)
}

/// `v ≥ limit`, with `v` within rounding of `limit` counted as equal.
#[must_use]
pub fn ge(v: f64, limit: f64) -> bool {
    !lt(v, limit)
}

/// A finding for a single criterion that holds (not a band of a scale).
#[must_use]
pub fn criterion_met(severity: Severity, text: Text, criterion: &str, source: Cite) -> Finding {
    Finding {
        severity,
        text: text.into(),
        criterion: criterion.to_owned(),
        source,
    }
}
