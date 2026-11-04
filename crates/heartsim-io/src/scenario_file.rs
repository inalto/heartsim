//! The running scenario as a scenario file (`assets/scenarios/*.ron` format): the common
//! [`Scenario`](heartsim_core::Scenario) fields plus the Tier-0 engine sections `timing` and
//! `morphology` in the same struct, so `Tier0Scenario::from_ron` reads it back unchanged.

use heartsim_ecg::Tier0Scenario;
use ron::ser::PrettyConfig;

/// RON text of `s`.
pub fn scenario_ron(s: &Tier0Scenario) -> Result<String, ron::Error> {
    let cfg = PrettyConfig::default().struct_names(true);
    let base = ron::ser::to_string_pretty(&s.scenario, cfg.clone())?;
    let timing = ron::ser::to_string_pretty(&s.timing, cfg.clone())?;
    let morphology = ron::ser::to_string_pretty(&s.morphology, cfg)?;
    // `Scenario( … )`: splice the two sections in before the closing parenthesis, indented one level.
    let body = base.trim_end();
    let body = body
        .strip_suffix(')')
        .ok_or_else(|| ron::Error::Message("scenario did not serialise as a struct".into()))?;
    let indent = |text: &str| text.replace('\n', "\n    ");
    let mut out = String::with_capacity(base.len() + timing.len() + morphology.len() + 64);
    out.push_str(body.trim_end());
    if !out.ends_with(',') && !out.ends_with('(') {
        out.push(',');
    }
    out.push_str("\n    timing: ");
    out.push_str(&indent(&timing));
    out.push_str(",\n    morphology: ");
    out.push_str(&indent(&morphology));
    out.push_str(",\n)\n");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn every_library_scenario_round_trips() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/scenarios");
        let mut n = 0;
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.extension().is_none_or(|x| x != "ron") {
                continue;
            }
            let s = Tier0Scenario::from_ron(&std::fs::read_to_string(&p).unwrap()).unwrap();
            let text = scenario_ron(&s).unwrap();
            let back =
                Tier0Scenario::from_ron(&text).unwrap_or_else(|e| panic!("{}: {e}\n{text}", p.display()));
            assert_eq!(back.scenario, s.scenario, "{}", p.display());
            assert_eq!(back.timing, s.timing, "{}", p.display());
            assert_eq!(back.morphology, s.morphology, "{}", p.display());
            n += 1;
        }
        assert!(n >= 27);
    }
}
