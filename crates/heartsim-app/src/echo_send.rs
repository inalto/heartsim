//! Where a measurement of the Echo page can be sent in the calculators (brief B34): the stable
//! calculator and input ids each named measurement fills, and the unit conversion on the way.
//!
//! Only what the simulator really measured is ever sent, under the name the learner gave it; the
//! rest of a calculator's inputs stay for the learner to type. A measurement no calculator takes
//! (the left atrial diameter, MAPSE, a′, a mitral S′) has no targets and its *Send* is disabled.
//! The ids are those of `heartsim-calc`; `tests/echo_send.rs` fails if one disappears.

use heartsim_echo::{AnnulusSite, Named};

/// One input of one calculator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target {
    pub calculator: &'static str,
    pub input: &'static str,
}

const fn target(calculator: &'static str, input: &'static str) -> Target {
    Target { calculator, input }
}

/// The inputs `name` (measured at `site`, for the velocities) fills.
#[must_use]
pub fn targets(name: Named, site: Option<AnnulusSite>) -> Vec<Target> {
    match name {
        Named::Lvidd => vec![
            target("lv-fractional-shortening", "lvidd_cm"),
            target("lv-diameter-index", "lvidd_cm"),
            target("lv-mass-linear", "lvidd_cm"),
        ],
        Named::Lvids => vec![
            target("lv-fractional-shortening", "lvids_cm"),
            target("lv-diameter-index", "lvids_cm"),
        ],
        Named::Ivsd => vec![target("lv-mass-linear", "ivsd_cm")],
        Named::Lvpwd => vec![target("lv-mass-linear", "pwd_cm")],
        Named::AorticRoot => vec![target("aortic-diameter-index", "diameter_cm")],
        Named::Tapse => vec![
            target("rv-systolic-function", "tapse_mm"),
            target("tapse-pasp-coupling", "tapse_mm"),
        ],
        Named::SPrime => match site {
            Some(AnnulusSite::TricuspidLateral) => vec![target("rv-systolic-function", "s_prime_cm_s")],
            _ => Vec::new(),
        },
        Named::EPrime => match site {
            Some(AnnulusSite::MitralSeptal) => vec![
                target("mitral-e-to-e-prime", "e_sep_cm_s"),
                target("septal-lateral-e-prime-ratio", "e_sep_cm_s"),
            ],
            Some(AnnulusSite::MitralLateral) => vec![
                target("mitral-e-to-e-prime", "e_lat_cm_s"),
                target("septal-lateral-e-prime-ratio", "e_lat_cm_s"),
            ],
            _ => Vec::new(),
        },
        Named::LaDiameter | Named::Mapse | Named::APrime => Vec::new(),
    }
}

/// The value as the calculator's input takes it, text with the decimals the input is read to:
/// diameters and thicknesses in cm (the simulator measures mm), TAPSE in mm, velocities in cm/s.
#[must_use]
pub fn input_text(target: Target, value_mm_or_cm_s: f32) -> String {
    if target.input.ends_with("_cm") {
        format!("{:.2}", value_mm_or_cm_s / 10.0)
    } else {
        format!("{value_mm_or_cm_s:.1}")
    }
}

/// Other inputs of a target calculator the measurement fixes (the level of the aortic diameter).
#[must_use]
pub fn companion_inputs(name: Named) -> Vec<(Target, &'static str)> {
    match name {
        Named::AorticRoot => vec![(target("aortic-diameter-index", "level"), "sinuses")],
        _ => Vec::new(),
    }
}
