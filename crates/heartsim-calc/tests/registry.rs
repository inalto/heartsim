//! Acceptance tests of brief B29 for the registry: stable unique ids, every calculator referenced,
//! reviewed, named in both languages, with a worked example that validates and computes; units on
//! every output; formula-only calculators interpret nothing; the search finds by name and quantity.

use heartsim_calc::{ALL, Category, InputKind, Review, Unit, by_id, in_category, search};
use std::collections::HashSet;

/// The id list, in registry order. Ids are stable: favourites and command lines use them. A change
/// here is a deliberate, documented break.
const IDS: &[&str] = &[
    "aortic-valve-area",
    "mitral-area-pht",
    "mitral-area-continuity",
    "mitral-area-pisa",
    "pulmonic-stenosis-gradient",
    "tricuspid-stenosis-criteria",
    "aortic-stenosis-pathway",
    "mitral-regurgitation-pisa",
    "aortic-regurgitation-pisa",
    "tricuspid-regurgitation-pisa",
    "mitral-regurgitation-volumetric",
    "aortic-regurgitation-volumetric",
    "pulmonic-regurgitation-volumetric",
    "mitral-e-to-e-prime",
    "lap-from-mr-jet",
    "lv-pressure-from-jets",
    "la-volume-index",
    "diastolic-sinus-rhythm",
    "diastolic-atrial-fibrillation",
    "diastolic-mac",
    "diastolic-pulmonary-hypertension",
    "diastolic-heart-transplant",
    "diastolic-exercise",
    "diastolic-mitral-stenosis",
    "diastolic-mitral-regurgitation",
    "diastolic-hcm",
    "diastolic-restrictive",
    "diastolic-sinus-tachycardia",
    "diastolic-lvad",
    "lv-fractional-shortening",
    "lv-diameter-index",
    "lv-mass-linear",
    "lv-volumes-indexed",
    "ejection-fraction",
    "gls-relative-change",
    "lvot-stroke-volume",
    "aortic-diameter-index",
    "wall-motion-score-index",
    "rap-from-ivc",
    "rvsp-pasp",
    "pa-pressures-from-pr",
    "mpap-from-acceleration-time",
    "pvr-doppler",
    "rv-systolic-function",
    "tapse-pasp-coupling",
    "rv-mpi",
    "right-atrial-size",
    "right-ventricular-size",
    "body-surface-area",
    "pericardial-effusion-size",
    "mitral-respiratory-variation",
    "tricuspid-respiratory-variation",
    "lvot-respiratory-variation",
    "hepatic-vein-reversal-ratio",
    "septal-lateral-e-prime-ratio",
    "pulmonary-systemic-flow-ratio",
    "shunt-flow",
    "vsd-rv-pressure",
    "pda-pa-pressure",
    "coarctation-gradient",
    "mean-gradient-from-trace",
    "aortic-prosthesis-eoa",
    "mitral-prosthesis-eoa",
    "aortic-prosthesis-screening",
    "aortic-prosthesis-serial",
    "mitral-prosthesis-screening",
    "wave-period-wavelength",
    "prf-nyquist",
    "spatial-pulse-length",
    "doppler-shift-velocity",
    "acoustic-impedance",
    "reflection-coefficient",
    "acoustic-intensity",
    "poiseuille-flow",
];

#[test]
fn ids_are_unique_stable_and_well_formed() {
    let ids: Vec<&str> = ALL.iter().map(|c| c.id).collect();
    assert_eq!(ids, IDS);
    let set: HashSet<&str> = ids.iter().copied().collect();
    assert_eq!(set.len(), ids.len(), "duplicate id");
    for id in ids {
        assert!(
            id.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
            "{id}"
        );
        assert!(by_id(id).is_some());
    }
    assert!(by_id("no-such-calculator").is_none());
}

#[test]
fn every_calculator_is_complete() {
    for c in ALL {
        assert!(!c.name.en.is_empty() && !c.name.it.is_empty(), "{} name", c.id);
        assert!(
            !c.summary.en.is_empty() && !c.summary.it.is_empty(),
            "{} summary",
            c.id
        );
        assert!(
            !c.synonyms.en.is_empty() && !c.synonyms.it.is_empty(),
            "{} synonyms",
            c.id
        );
        assert!(!c.references.is_empty(), "{} has no reference", c.id);
        for r in c.references {
            assert!(!r.locator.is_empty(), "{} reference without locator", c.id);
            assert!(
                r.reference.doi.is_some() || r.reference.url.is_some(),
                "{} reference without a link",
                c.id
            );
        }
        let mut input_ids = HashSet::new();
        for i in c.inputs {
            assert!(input_ids.insert(i.id), "{} repeats input {}", c.id, i.id);
            assert!(
                !i.label.en.is_empty() && !i.label.it.is_empty(),
                "{}.{}",
                c.id,
                i.id
            );
            match i.kind {
                InputKind::Number { min, max, .. } => assert!(min < max, "{}.{} range", c.id, i.id),
                InputKind::Choice(ch) => assert_ne!(ch.len(), 0),
            }
            if let Some((sel, _)) = i.when {
                assert!(
                    matches!(c.input(sel).map(|s| s.kind), Some(InputKind::Choice(_))),
                    "{}.{} when",
                    c.id,
                    i.id
                );
            }
        }
        // the worked example validates and computes
        let ex = c.example_inputs();
        for (k, _) in ex.iter() {
            assert!(c.input(k).is_some(), "{} example sets unknown input {k}", c.id);
        }
        let out = c
            .evaluate(&ex)
            .unwrap_or_else(|e| panic!("{} example: {e:?}", c.id));
        assert_ne!(out.values.len(), 0, "{}", c.id);
        assert_eq!(out.values.len(), out.steps.len(), "{}: one step per value", c.id);
        let mut out_ids = HashSet::new();
        for q in &out.values {
            assert!(q.value.is_finite());
            assert!(out_ids.insert(q.id), "{} repeats output {}", c.id, q.id);
            assert!(!q.label.en.is_empty() && !q.label.it.is_empty());
        }
        if c.review == Review::FormulaOnly {
            assert_eq!(out.findings.len(), 0, "{} is formula only but interprets", c.id);
        } else {
            assert_ne!(
                out.findings.len(),
                0,
                "{}: a verified calculator's example should show its grading",
                c.id
            );
        }
        for f in &out.findings {
            assert!(!f.criterion.is_empty() && !f.source.locator.is_empty());
            // worded as criteria, never as a diagnosis
            assert!(!f.text.en.to_lowercase().contains("the patient has"), "{}", c.id);
        }
    }
}

#[test]
fn output_units_follow_their_ids() {
    // ids carry the unit: `_cm2` is cm², `_mmhg` mmHg, and so on
    let suffix_unit = [
        ("_mm_mmhg", Unit::MmPerMmHg),
        ("_mmhg_s_ml", Unit::MmHgSPerMl),
        ("_w_cm2", Unit::WCm2),
        ("_cm2_m2", Unit::Cm2M2),
        ("_ml_m2", Unit::MlM2),
        ("_cm_m2", Unit::CmM2),
        ("_g_m2", Unit::GM2),
        ("_l_min_m2", Unit::LMinM2),
        ("_cm2", Unit::Cm2),
        ("_mmhg", Unit::MmHg),
        ("_ml_s", Unit::MlS),
        ("_ml", Unit::Ml),
        ("_m_s", Unit::MS),
        ("_cm_s", Unit::CmS),
        ("_percent", Unit::Percent),
        ("_l_min", Unit::LMin),
        ("_us", Unit::Us),
        ("_khz", Unit::Khz),
        ("_wu", Unit::WoodUnits),
        ("_g", Unit::G),
    ];
    for c in ALL {
        let out = c.evaluate(&c.example_inputs()).unwrap();
        for q in &out.values {
            if let Some((_, u)) = suffix_unit.iter().find(|(s, _)| q.id.ends_with(s)) {
                assert_eq!(q.unit, *u, "{}.{}", c.id, q.id);
            }
        }
    }
}

#[test]
fn categories_cover_the_registry_and_status_counts() {
    let n: usize = Category::ALL.iter().map(|c| in_category(*c).count()).sum();
    assert_eq!(n, ALL.len());
    for cat in Category::ALL {
        assert!(in_category(cat).count() > 0, "{cat:?} is empty");
    }
    let verified = ALL.iter().filter(|c| c.review == Review::Verified).count();
    assert_eq!(ALL.len(), 74);
    assert_eq!(
        verified, 49,
        "change docs/research/echo-calculators.md with this count"
    );
}

#[test]
fn search_by_name_and_by_quantity_in_both_languages() {
    let ids = |q: &str, l: &str| search(q, l).iter().map(|c| c.id).collect::<Vec<_>>();
    assert!(ids("aortic valve area", "en").contains(&"aortic-valve-area"));
    assert!(ids("area valvolare aortica", "it").contains(&"aortic-valve-area"));
    // by a quantity typed into it
    assert!(ids("pressure half-time", "en").contains(&"mitral-area-pht"));
    assert!(ids("tempo di dimezzamento", "it").contains(&"mitral-area-pht"));
    // by a quantity it gives, and case-insensitively
    assert!(ids("eroa", "en").contains(&"mitral-regurgitation-pisa"));
    assert!(ids("TAPSE", "it").contains(&"tapse-pasp-coupling"));
    // a prime typed as an apostrophe finds e'
    assert!(ids("e/e'", "en").contains(&"mitral-e-to-e-prime"));
    // accents do not matter
    assert!(ids("velocita del suono", "it").contains(&"wave-period-wavelength"));
    assert_eq!(search("", "en").len(), ALL.len());
    assert!(search("zzzz", "en").is_empty());
}

#[test]
fn only_heart_rate_links_to_the_simulator() {
    for c in ALL {
        if let Some(heartsim_calc::SimLink::HeartRate(id)) = c.sim_link {
            let spec = c.input(id).expect("linked input exists");
            assert!(
                matches!(spec.kind, InputKind::Number { unit: Unit::Bpm, .. }),
                "{}",
                c.id
            );
        }
    }
    assert!(by_id("lvot-stroke-volume").unwrap().sim_link.is_some());
}
