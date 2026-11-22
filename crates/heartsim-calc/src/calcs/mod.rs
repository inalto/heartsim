//! The calculators, one module per category (and one per guideline algorithm of brief B29 part B).
//! Each calculator is a `pub static` [`Calculator`] next to its compute function; the module's tests
//! hold the hand-computed cases.

use crate::fmt;
use crate::model::{
    Choice, Cite, InputError, InputKind, InputSpec, Inputs, Outcome, Quantity, Reference, Text, Unit,
};

// Every compute function has the signature of `Calculator::compute`, also those that cannot fail.
#[allow(clippy::unnecessary_wraps)]
pub mod as_pathway;
#[allow(clippy::unnecessary_wraps)]
pub mod diastolic;
#[allow(clippy::unnecessary_wraps)]
pub mod filling;
#[allow(clippy::unnecessary_wraps)]
pub mod pericardium;
#[allow(clippy::unnecessary_wraps)]
pub mod physics;
#[allow(clippy::unnecessary_wraps)]
pub mod prosthetic;
#[allow(clippy::unnecessary_wraps)]
pub mod prosthetic_screen;
#[allow(clippy::unnecessary_wraps)]
pub mod regurgitation;
#[allow(clippy::unnecessary_wraps)]
pub mod right_heart;
#[allow(clippy::unnecessary_wraps)]
pub mod shunts;
#[allow(clippy::unnecessary_wraps)]
pub mod stenosis;
#[allow(clippy::unnecessary_wraps)]
pub mod ventricle;

/// A required number input.
pub(crate) const fn num(
    id: &'static str,
    label: Text,
    unit: Unit,
    min: f64,
    max: f64,
    decimals: u8,
) -> InputSpec {
    InputSpec {
        id,
        label,
        kind: InputKind::Number {
            unit,
            min,
            max,
            decimals,
        },
        optional: false,
        when: None,
    }
}

/// An optional number input.
pub(crate) const fn opt(
    id: &'static str,
    label: Text,
    unit: Unit,
    min: f64,
    max: f64,
    decimals: u8,
) -> InputSpec {
    let mut s = num(id, label, unit, min, max, decimals);
    s.optional = true;
    s
}

/// Every decision tree (brief B29 part B), in registry order.
pub static TREES: &[&crate::tree::Tree] = &[
    &as_pathway::TREE,
    &diastolic::sinus::TREE,
    &diastolic::af::TREE,
    &diastolic::mac::TREE,
    &diastolic::ph::TREE,
    &diastolic::htx::TREE,
    &diastolic::exercise::TREE,
    &diastolic::ms::TREE,
    &diastolic::mr::TREE,
    &diastolic::hcm::TREE,
    &diastolic::restrictive::TREE,
    &diastolic::tachy::TREE,
    &diastolic::lvad::TREE,
    &prosthetic_screen::AORTIC_TREE,
    &prosthetic_screen::SERIAL_TREE,
    &prosthetic_screen::MITRAL_TREE,
];

/// An optional choice input.
pub(crate) const fn opt_choice(id: &'static str, label: Text, choices: &'static [Choice]) -> InputSpec {
    let mut s = choice(id, label, choices);
    s.optional = true;
    s
}

/// A required choice input.
pub(crate) const fn choice(id: &'static str, label: Text, choices: &'static [Choice]) -> InputSpec {
    InputSpec {
        id,
        label,
        kind: InputKind::Choice(choices),
        optional: false,
        when: None,
    }
}

/// `spec` shown only while `choice_id` holds one of `ids`.
pub(crate) const fn when(
    mut spec: InputSpec,
    choice_id: &'static str,
    ids: &'static [&'static str],
) -> InputSpec {
    spec.when = Some((choice_id, ids));
    spec
}

/// A reference with its locator.
pub(crate) const fn cite(reference: &'static Reference, locator: &'static str) -> Cite {
    Cite { reference, locator }
}

/// A computed value.
pub(crate) const fn q(id: &'static str, label: Text, value: f64, unit: Unit, decimals: u8) -> Quantity {
    Quantity {
        id,
        label,
        value,
        unit,
        decimals,
    }
}

/// A number with its unit, for the substituted formula.
pub(crate) fn v(x: f64, decimals: u8, unit: Unit) -> String {
    fmt::with_unit(x, decimals, unit)
}

/// A bare number, for the substituted formula.
pub(crate) fn n(x: f64, decimals: u8) -> String {
    fmt::num(x, decimals)
}

/// Fails with one error if `bad`.
pub(crate) fn check(bad: bool, input: &'static str, message: Text) -> Result<(), Vec<InputError>> {
    if bad {
        Err(vec![InputError::inconsistent(input, message)])
    } else {
        Ok(())
    }
}

/// The error for a calculator whose optional inputs are all empty: "at least one", shown on `input`.
pub(crate) fn need_one(input: &'static str) -> Vec<InputError> {
    vec![InputError::inconsistent(
        input,
        crate::tx!("Enter at least one measurement", "Inserire almeno una misura"),
    )]
}

/// An empty outcome, to fill.
pub(crate) fn outcome() -> Outcome {
    Outcome::default()
}

/// Shorthand for `inputs.num(id)`.
pub(crate) fn o(inputs: &Inputs, id: &str) -> Option<f64> {
    inputs.num(id)
}

/// The bands of the guideline tables that use the words normal, mildly, moderately and severely
/// abnormal.
pub(crate) mod words {
    use crate::model::Text;
    use crate::tx;
    pub const NORMAL: Text = tx!("normal", "normale");
    pub const MILD_ABN: Text = tx!("mildly abnormal", "lievemente anormale");
    pub const MOD_ABN: Text = tx!("moderately abnormal", "moderatamente anormale");
    pub const SEV_ABN: Text = tx!("severely abnormal", "gravemente anormale");
    pub const MILD: Text = tx!("mild", "lieve");
    pub const MODERATE: Text = tx!("moderate", "moderata");
    pub const SEVERE: Text = tx!("severe", "grave");
}

#[cfg(test)]
pub(crate) mod testing {
    use crate::model::{Calculator, ErrorKind, Inputs, Outcome, Severity};

    /// Evaluates and unwraps, with the errors in the panic message.
    pub fn ok(c: &Calculator, i: &Inputs) -> Outcome {
        c.evaluate(i).unwrap_or_else(|e| panic!("{}: {e:?}", c.id))
    }

    /// Asserts `got` is `want` within `tol`.
    #[track_caller]
    pub fn close(got: Option<f64>, want: f64, tol: f64) {
        let g = got.expect("value missing");
        assert!((g - want).abs() <= tol, "got {g}, want {want} ± {tol}");
    }

    /// The severities of the findings, in order.
    pub fn severities(o: &Outcome) -> Vec<Severity> {
        o.findings.iter().map(|f| f.severity).collect()
    }

    /// Asserts the evaluation fails with a consistency error on `input`.
    #[track_caller]
    pub fn inconsistent(c: &Calculator, i: &Inputs, input: &str) {
        let e = c.evaluate(i).expect_err("should be refused");
        assert!(
            e.iter()
                .any(|e| e.input == Some(input) && matches!(e.kind, ErrorKind::Inconsistent(_))),
            "{e:?}"
        );
    }

    /// Asserts the evaluation fails with an out-of-range error on `input`.
    #[track_caller]
    pub fn out_of_range(c: &Calculator, i: &Inputs, input: &str) {
        let e = c.evaluate(i).expect_err("should be refused");
        assert!(
            e.iter()
                .any(|e| e.input == Some(input) && matches!(e.kind, ErrorKind::OutOfRange { .. })),
            "{e:?}"
        );
    }
}
