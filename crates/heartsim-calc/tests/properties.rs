//! Property tests of brief B29: for every calculator and any combination of in-range inputs (each
//! optional input present or not, each choice any of its options), the evaluation never panics and
//! an accepted result holds only finite values; refusals are typed consistency errors. Values out of
//! range are always refused against their field. A decision tool (part B) always ends at a leaf.

use heartsim_calc::{ALL, Calculator, ErrorKind, InputKind, Inputs, Value};
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};

/// One random input set: a fraction in [0, 1] per field picks the value inside the range (or the
/// choice), and a flag leaves optional fields empty.
fn inputs_for(c: &Calculator, fractions: &[f64], present: &[bool]) -> Inputs {
    let mut i = Inputs::new();
    for (k, spec) in c.inputs.iter().enumerate() {
        if spec.optional && !present[k] {
            continue;
        }
        match spec.kind {
            InputKind::Number { min, max, .. } => {
                i.set(spec.id, Value::Number(min + (max - min) * fractions[k]));
            }
            InputKind::Choice(choices) => {
                let pick = ((fractions[k] * choices.len() as f64) as usize).min(choices.len() - 1);
                i.set(spec.id, Value::Choice(choices[pick].id.to_owned()));
            }
        }
    }
    i
}

#[test]
fn no_nan_or_infinity_for_any_in_range_input() {
    for c in ALL {
        let n = c.inputs.len();
        let mut runner = TestRunner::new(Config {
            cases: 400,
            ..Config::default()
        });
        let strategy = (
            proptest::collection::vec(prop_oneof![Just(0.0), Just(1.0), 0.0..=1.0f64], n),
            proptest::collection::vec(any::<bool>(), n),
        );
        runner
            .run(&strategy, |(fractions, present)| {
                let i = inputs_for(c, &fractions, &present);
                match c.evaluate(&i) {
                    Ok(out) => {
                        // a decision tool always ends at a leaf of its tree
                        if let Some(t) = heartsim_calc::tree::for_calculator(c.id) {
                            let last = out.findings.last();
                            prop_assert!(
                                last.is_some_and(|f| heartsim_calc::tree::reached(t, f).is_some()),
                                "{}: {:?}",
                                c.id,
                                last
                            );
                        }
                        for q in &out.values {
                            prop_assert!(q.value.is_finite(), "{}.{} = {}", c.id, q.id, q.value);
                        }
                        for s in &out.steps {
                            prop_assert!(
                                !s.result.contains("NaN") && !s.result.contains("inf"),
                                "{}: {}",
                                c.id,
                                s.result
                            );
                        }
                    }
                    Err(errors) => {
                        // only consistency errors (all fields were filled in range)
                        for e in &errors {
                            prop_assert!(matches!(e.kind, ErrorKind::Inconsistent(_)), "{}: {e:?}", c.id);
                        }
                    }
                }
                Ok(())
            })
            .unwrap_or_else(|e| panic!("{}: {e}", c.id));
    }
}

#[test]
fn out_of_range_and_missing_inputs_are_refused_against_their_field() {
    for c in ALL {
        let ex = c.example_inputs();
        for spec in c.inputs {
            if !Calculator::visible(spec, &ex) {
                continue;
            }
            if let InputKind::Number { min, max, .. } = spec.kind {
                for bad in [min - (max - min) - 1.0, max + (max - min) + 1.0, f64::NAN] {
                    let mut i = ex.clone();
                    i.set(spec.id, Value::Number(bad));
                    let errors = c.evaluate(&i).expect_err(spec.id);
                    assert!(
                        errors
                            .iter()
                            .any(|e| e.input == Some(spec.id)
                                && matches!(e.kind, ErrorKind::OutOfRange { .. })),
                        "{}.{}: {errors:?}",
                        c.id,
                        spec.id
                    );
                }
            }
            if !spec.optional {
                let mut i = ex.clone();
                i.remove(spec.id);
                let errors = c.evaluate(&i).expect_err(spec.id);
                let missing = errors
                    .iter()
                    .find(|e| e.input == Some(spec.id) && e.kind == ErrorKind::Missing)
                    .unwrap_or_else(|| panic!("{}.{}", c.id, spec.id));
                assert_eq!(missing.message("en"), "Required");
                assert_eq!(missing.message("it"), "Campo obbligatorio");
            }
        }
    }
}
