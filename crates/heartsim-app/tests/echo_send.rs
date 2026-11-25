//! B34 acceptance: sending a measurement of the Echo page to the calculators. Every named
//! measurement maps to input ids that exist (a test fails if one disappears); the calculator
//! shows the value with its origin until it is edited; nothing is written to the saved state.

use heartsim_app::echo_calc::EchoCalcPage;
use heartsim_app::echo_send::{Target, companion_inputs, input_text, targets};
use heartsim_calc::{InputKind, by_id};
use heartsim_echo::{AnnulusSite, Named};

fn sites() -> Vec<Option<AnnulusSite>> {
    let mut v = vec![None];
    v.extend(AnnulusSite::ALL.map(Some));
    v
}

#[test]
fn every_target_is_a_calculator_input_that_exists() {
    let mut any = 0;
    for name in Named::ALL {
        for site in sites() {
            for t in targets(name, site) {
                let calc =
                    by_id(t.calculator).unwrap_or_else(|| panic!("{name:?}: no calculator {}", t.calculator));
                let spec = calc
                    .input(t.input)
                    .unwrap_or_else(|| panic!("{name:?}: {} has no input {}", t.calculator, t.input));
                assert!(
                    matches!(spec.kind, InputKind::Number { .. }),
                    "{name:?} fills a number"
                );
                any += 1;
            }
        }
        for (t, choice) in companion_inputs(name) {
            let spec = by_id(t.calculator)
                .and_then(|c| c.input(t.input))
                .expect("companion input");
            let InputKind::Choice(choices) = spec.kind else {
                panic!("a choice")
            };
            assert!(
                choices.iter().any(|c| c.id == choice),
                "{choice} is a choice of {}",
                t.input
            );
        }
    }
    assert!(any >= 15, "{any} targets");
}

#[test]
fn the_names_and_what_they_fill() {
    let ids = |n, s| -> Vec<(&str, &str)> {
        targets(n, s)
            .iter()
            .map(|t: &Target| (t.calculator, t.input))
            .collect()
    };
    assert!(ids(Named::Lvidd, None).contains(&("lv-fractional-shortening", "lvidd_cm")));
    assert!(ids(Named::Lvidd, None).contains(&("lv-mass-linear", "lvidd_cm")));
    assert!(ids(Named::Lvids, None).contains(&("lv-fractional-shortening", "lvids_cm")));
    assert_eq!(ids(Named::Ivsd, None), vec![("lv-mass-linear", "ivsd_cm")]);
    assert_eq!(ids(Named::Lvpwd, None), vec![("lv-mass-linear", "pwd_cm")]);
    assert!(ids(Named::Tapse, None).contains(&("rv-systolic-function", "tapse_mm")));
    assert!(ids(Named::Tapse, None).contains(&("tapse-pasp-coupling", "tapse_mm")));
    // the velocities depend on the annulus
    assert!(
        ids(Named::EPrime, Some(AnnulusSite::MitralLateral)).contains(&("mitral-e-to-e-prime", "e_lat_cm_s"))
    );
    assert!(
        ids(Named::EPrime, Some(AnnulusSite::MitralSeptal)).contains(&("mitral-e-to-e-prime", "e_sep_cm_s"))
    );
    assert_eq!(ids(Named::EPrime, Some(AnnulusSite::TricuspidLateral)).len(), 0);
    assert_eq!(
        ids(Named::SPrime, Some(AnnulusSite::TricuspidLateral)),
        vec![("rv-systolic-function", "s_prime_cm_s")]
    );
    assert_eq!(ids(Named::SPrime, Some(AnnulusSite::MitralLateral)).len(), 0);
    // no calculator takes these, so nothing is invented for them
    for n in [Named::LaDiameter, Named::Mapse, Named::APrime] {
        assert!(sites().into_iter().all(|s| targets(n, s).is_empty()), "{n:?}");
    }
}

#[test]
fn values_go_in_the_units_the_inputs_are_read_in() {
    let t = |c, i| Target {
        calculator: c,
        input: i,
    };
    assert_eq!(
        input_text(t("lv-fractional-shortening", "lvidd_cm"), 45.3),
        "4.53"
    );
    assert_eq!(input_text(t("rv-systolic-function", "tapse_mm"), 23.04), "23.0");
    assert_eq!(input_text(t("mitral-e-to-e-prime", "e_lat_cm_s"), 8.75), "8.8");
}

#[test]
fn the_calculator_shows_the_value_with_its_origin_and_nothing_is_stored() {
    let mut page = EchoCalcPage::default();
    let stored = page.storage_string();
    page.receive_from_echo("lv-fractional-shortening", "lvidd_cm", "4.53");
    page.receive_from_echo("lv-fractional-shortening", "lvids_cm", "3.10");
    page.select("lv-fractional-shortening");
    assert_eq!(page.field("lvidd_cm"), "4.53");
    assert!(page.is_from_echo("lvidd_cm") && page.is_from_echo("lvids_cm"));
    // the calculator works from the two values, as if typed
    let out = page.evaluate().expect("a result");
    assert!(
        out.values.iter().any(|q| q.display().contains("32")),
        "FS = (4.53-3.10)/4.53 = 31.6 %"
    );
    // an edit takes the mark away; the other field keeps it
    page.set_field("lvidd_cm", "4.6");
    assert!(!page.is_from_echo("lvidd_cm"));
    assert!(page.is_from_echo("lvids_cm"));
    // nothing but favourites and the last calculator is stored, never a value
    let after = page.storage_string();
    assert!(after.starts_with("v1;last=lv-fractional-shortening;"));
    assert!(!after.contains("4.5") && !after.contains("3.1"));
    assert_ne!(stored, "");
    // unknown ids are ignored
    page.receive_from_echo("no-such", "x", "1");
    page.receive_from_echo("lv-fractional-shortening", "no-such", "1");
}
