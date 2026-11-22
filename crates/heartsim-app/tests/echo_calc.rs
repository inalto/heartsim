//! Acceptance tests of brief B29 for the Echo calculators page (egui_kittest): the search finds a
//! calculator by name and by quantity in both languages; typed inputs show the result and the
//! substituted formula; an out-of-range input shows its message and no result; the worked example
//! fills and computes; Enter moves to the next field; favourites round-trip and typed values never
//! reach the saved string; the review badge is shown; the heart rate comes from the running rhythm
//! only where offered; the phone layout shows the list, then the calculator.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use heartsim_app::echo_calc::EchoCalcPage;
use heartsim_ui::i18n;

struct Model {
    page: EchoCalcPage,
    hr: Option<f64>,
}

fn harness_sized(lang: &str, width: f32, page: EchoCalcPage, hr: Option<f64>) -> Harness<'static, Model> {
    i18n::set_language(lang);
    Harness::builder()
        .with_size(egui::vec2(width, 3000.0))
        .build_ui_state(|ui, m: &mut Model| m.page.ui(ui, m.hr), Model { page, hr })
}

fn harness(lang: &str) -> Harness<'static, Model> {
    harness_sized(lang, 1280.0, EchoCalcPage::default(), None)
}

fn type_into(h: &mut Harness<'static, Model>, label: &str, text: &str) {
    h.get_by_label(label).click();
    h.run();
    h.get_by_label(label).type_text(text);
    h.run();
}

#[test]
fn search_finds_by_name_and_by_quantity_in_both_languages() {
    let mut h = harness("en");
    h.run();
    type_into(&mut h, "Search", "pressure half-time");
    assert!(
        h.query_by_label("Mitral valve area (pressure half-time)")
            .is_some()
    );
    assert!(
        h.query_by_label("Ejection fraction").is_none(),
        "the list is filtered"
    );
    h.state_mut().page.query = "regurgitant volume".into();
    h.run();
    assert!(h.query_by_label("Mitral regurgitation (PISA)").is_some());

    let mut h = harness("it");
    h.run();
    type_into(&mut h, "Cerca", "tempo di dimezzamento");
    assert!(
        h.query_by_label("Area valvolare mitralica (tempo di dimezzamento)")
            .is_some()
    );
    h.state_mut().page.query = "frazione di eiezione".into();
    h.run();
    h.get_by_label("Frazione di eiezione").click();
    h.run();
    assert_eq!(h.state().page.selected().id, "ejection-fraction");
    i18n::set_language("en");
}

#[test]
fn typed_inputs_show_the_result_and_the_substituted_formula() {
    let mut h = harness("en");
    h.run();
    assert_eq!(h.state().page.selected().id, "aortic-valve-area");
    assert!(
        h.query_by_label("Fill in the measurements to see the result.")
            .is_some()
    );
    type_into(&mut h, "LVOT diameter", "2");
    type_into(&mut h, "LVOT VTI (pulsed wave)", "20");
    type_into(&mut h, "Aortic jet VTI (continuous wave)", "80");
    let _ = h.get_by_label("0.79 cm²");
    let _ =
        h.get_by_label_contains("AVA = CSA × VTI(LVOT) ÷ VTI(AV)  = 3.14 cm² × 20.0 cm ÷ 80 cm  = 0.79 cm²");
    // the finding with its criterion and source
    let _ = h.get_by_label_contains("Aortic valve area in the range the source grades “severe”");
    let _ = h.get_by_label_contains("AVA < 1.0 cm² — J Am Soc Echocardiogr 2017;30:372–392, Table 3, p. 383");
    // the review badge
    let _ = h.get_by_label("Verified against the source");
    // typed values never reach what is saved
    let saved = h.state().page.storage_string();
    assert!(!saved.contains("80") && !saved.contains("20"), "{saved}");
    assert!(saved.contains("last=aortic-valve-area"));
}

#[test]
fn an_out_of_range_input_shows_its_message_and_no_result() {
    let mut h = harness("en");
    h.run();
    h.state_mut().page.load_example();
    h.run();
    assert!(h.query_by_label_contains("AVA = CSA").is_some());
    h.state_mut().page.set_field("lvot_d_cm", "7.5");
    h.run();
    let _ = h.get_by_label("Outside the plausible range 1–4");
    let _ = h.get_by_label("No result until the fields marked in red are corrected.");
    assert!(
        h.query_by_label_contains("AVA = CSA").is_none(),
        "no stale result"
    );
    // a consistency error names its field too
    h.state_mut().page.select("ejection-fraction");
    h.state_mut().page.set_field("edv_ml", "100");
    h.state_mut().page.set_field("esv_ml", "130");
    h.run();
    let _ = h.get_by_label("The end-systolic volume must be smaller than the end-diastolic one");
    // and text that is not a number
    h.state_mut().page.set_field("edv_ml", "1o0");
    h.run();
    let _ = h.get_by_label("Not a number");
}

#[test]
fn the_worked_example_fills_and_computes_with_the_language_s_decimal_comma() {
    let mut h = harness("it");
    h.run();
    h.get_by_label("Carica l'esempio svolto").click();
    h.run();
    assert_eq!(h.state().page.field("lvot_d_cm"), "2,1");
    // 2.1 cm, 20 cm, 95 cm: π × 1.05² × 20 ÷ 95 = 0.73 cm²
    let _ = h.get_by_label("0,73 cm²");
    let _ = h.get_by_label("Verificato sulla fonte");
    h.get_by_label("Cancella").click();
    h.run();
    assert_eq!(h.state().page.field("lvot_d_cm"), "");
    i18n::set_language("en");
}

#[test]
fn every_worked_example_computes_on_the_page() {
    let mut page = EchoCalcPage::default();
    for c in heartsim_calc::ALL {
        page.select(c.id);
        page.load_example();
        assert!(page.evaluate().is_ok(), "{}", c.id);
    }
}

#[test]
fn a_formula_only_calculator_says_so_and_shows_no_finding() {
    let mut page = EchoCalcPage::default();
    page.select("lap-from-mr-jet");
    page.load_example();
    let mut h = harness_sized("en", 1280.0, page, None);
    h.run();
    let _ = h.get_by_label("Formula only (no grading)");
    let _ = h.get_by_label(
        "Formula only: no grading is shown, because its criteria were not read on a primary source.",
    );
    let _ = h.get_by_label_contains("LAP = SBP − ΔP");
}

#[test]
fn enter_moves_to_the_next_field() {
    let mut h = harness("en");
    h.run();
    type_into(&mut h, "LVOT diameter", "2.1");
    h.key_press(egui::Key::Enter);
    h.run();
    h.run();
    let focused = h.ctx.memory(egui::Memory::focused);
    assert_eq!(
        focused,
        Some(egui::Id::unique(("calc", "aortic-valve-area", "lvot_vti_cm")))
    );
}

#[test]
fn favourites_and_the_last_calculator_round_trip() {
    let mut h = harness("en");
    h.run();
    h.state_mut().page.select("rvsp-pasp");
    h.run();
    h.get_by_label("Favourite").click();
    h.run();
    h.state_mut().page.toggle_favourite("pvr-doppler");
    h.state_mut().page.set_field("tr_vmax_m_s", "3.37");
    let saved = h.state().page.storage_string();
    assert_eq!(saved, "v1;last=rvsp-pasp;fav=rvsp-pasp,pvr-doppler");
    assert!(!saved.contains("3.37"));
    let mut back = EchoCalcPage::default();
    back.restore(&saved);
    assert_eq!(back.selected().id, "rvsp-pasp");
    assert_eq!(back.favourites(), ["rvsp-pasp", "pvr-doppler"]);
    assert_eq!(back.field("tr_vmax_m_s"), "", "typed values are not restored");
    // unknown ids from another version are dropped
    back.restore("v1;last=gone;fav=gone,pvr-doppler");
    assert_eq!(back.selected().id, "rvsp-pasp");
    assert_eq!(back.favourites(), ["pvr-doppler"]);
    // the favourites are listed first
    let mut h = harness_sized("en", 1280.0, back, None);
    h.run();
    let _ = h.get_by_label("Favourites");
}

#[test]
fn heart_rate_comes_from_the_running_rhythm_only_where_offered() {
    let mut page = EchoCalcPage::default();
    page.select("lvot-stroke-volume");
    let mut h = harness_sized("en", 1280.0, page, Some(72.4));
    h.run();
    h.get_by_label("Heart rate from the running rhythm (72 bpm)")
        .click();
    h.run();
    assert_eq!(h.state().page.field("hr_bpm"), "72");
    // no beats: the button says why and does nothing
    h.state_mut().hr = None;
    h.run();
    let _ = h.get_by_label("No heart rate: the simulated rhythm has no recent beats");
    // a calculator without a simulator input has no such button
    h.state_mut().page.select("aortic-valve-area");
    h.state_mut().hr = Some(70.0);
    h.run();
    assert!(
        h.query_by_label_contains("Heart rate from the running rhythm")
            .is_none()
    );
}

#[test]
fn at_phone_width_the_list_comes_first_then_the_calculator() {
    let mut h = harness_sized("en", 380.0, EchoCalcPage::default(), None);
    h.run();
    assert!(h.query_by_label("Load the worked example").is_none());
    h.get_by_label("Pericardial effusion size").click();
    h.run();
    let _ = h.get_by_label("Load the worked example");
    let back = h.get_by_label("Back to the list");
    assert!(back.rect().right() <= 380.0);
    back.click();
    h.run();
    let _ = h.get_by_label("Search");
}

#[test]
fn a_decision_tool_shows_its_path_and_an_indeterminate_result() {
    let mut page = EchoCalcPage::default();
    page.select("diastolic-sinus-rhythm");
    // all three step-1 criteria met, but no A velocity: grade 2 and grade 3 are both possible
    page.set_field("e_cm_s", "100");
    page.set_field("e_septal_cm_s", "5");
    page.set_field("tr_vmax_m_s", "3");
    let mut h = harness_sized("en", 1280.0, page, None);
    h.run();
    let _ = h.get_by_label("Path through the algorithm");
    let _ = h.get_by_label_contains(
        "Reduced e', increased E/e', increased TR velocity or PASP: all three abnormal",
    );
    // the step shows the values behind it and names its figure and page
    let _ = h.get_by_label_contains(
        "TR = 3.00 ≥ 2.8 m/s — J Am Soc Echocardiogr 2025;38:537–569, Figure 3, p. 554",
    );
    let _ = h.get_by_label("indeterminate");
    let _ = h.get_by_label_contains("LAP indeterminate with these measurements. Still possible:");
    let _ = h.get_by_label_contains("E/A = ?");
    // with the A velocity the path ends at a grade
    h.state_mut().page.set_field("a_cm_s", "45");
    h.run();
    let _ = h.get_by_label_contains("Result: meets the figure's criteria for grade 3 diastolic dysfunction");
    assert!(h.query_by_label("indeterminate").is_none());
    // in Italian
    i18n::set_language("it");
    h.run();
    let _ = h.get_by_label("Percorso nell'algoritmo");
    let _ = h.get_by_label_contains(
        "Risultato: soddisfa i criteri della figura per disfunzione diastolica di grado 3",
    );
    i18n::set_language("en");
}

#[test]
fn a_sex_specific_grading_changes_with_the_sex_choice() {
    let mut page = EchoCalcPage::default();
    page.select("ejection-fraction");
    // EF 53 %: normal for a man (52-72 %), mildly abnormal for a woman (54-74 %)
    page.set_field("edv_ml", "100");
    page.set_field("esv_ml", "47");
    let mut h = harness_sized("en", 1280.0, page, None);
    h.run();
    let _ = h.get_by_label_contains("choose the sex to see the grading");
    h.state_mut().page.set_field("sex", "male");
    h.run();
    let _ = h.get_by_label("Ejection fraction in the range the source grades “normal”");
    let _ = h.get_by_label_contains("52 ≤ EF ≤ 72%");
    h.state_mut().page.set_field("sex", "female");
    h.run();
    let _ = h.get_by_label("Ejection fraction in the range the source grades “mildly abnormal”");
    let _ = h.get_by_label("Verified against the source");
}

/// Every character a calculator can put on the page has a glyph in the app's fonts (the bundled egui
/// fonts): a missing one would be drawn as a box.
#[test]
fn every_calculator_text_has_glyphs_in_the_app_fonts() {
    use std::collections::BTreeSet;
    let ctx = egui::Context::default();
    ctx.run_pass(egui::RawInput::default(), |_| {})
        .textures_delta
        .clear();
    let mut missing = BTreeSet::new();
    let mut check = |s: &str, mono: bool| {
        let font = if mono {
            egui::FontId::monospace(13.0)
        } else {
            egui::FontId::proportional(13.0)
        };
        for ch in s.chars() {
            if !ctx.fonts_mut(|f| f.has_glyph(&font, ch)) {
                missing.insert((ch, if mono { "mono" } else { "text" }));
            }
        }
    };
    for c in heartsim_calc::ALL {
        for lang in ["en", "it"] {
            check(c.name.get(lang), false);
            check(c.summary.get(lang), false);
            check(c.category.name().get(lang), false);
            check(c.review.label().get(lang), false);
            for i in c.inputs {
                check(i.label.get(lang), false);
                if let heartsim_calc::InputKind::Choice(ch) = i.kind {
                    for x in ch {
                        check(x.label.get(lang), false);
                    }
                }
                if let heartsim_calc::InputKind::Number { unit, .. } = i.kind {
                    check(unit.symbol(), false);
                }
            }
            let out = c.evaluate(&c.example_inputs()).unwrap();
            for q in &out.values {
                check(q.label.get(lang), false);
                check(&q.display(), false);
            }
            for s in &out.steps {
                check(&format!("{}{}{}", s.formula, s.substituted, s.result), true);
            }
            for f in &out.findings {
                check(f.text.get(lang), false);
                check(&f.criterion, false);
                check(f.source.locator, false);
            }
            for e in c
                .evaluate(&heartsim_calc::Inputs::new())
                .err()
                .unwrap_or_default()
            {
                check(&e.message(lang), false);
            }
        }
        for r in c.references {
            check(r.reference.authors, false);
            check(r.reference.title, false);
            check(r.reference.journal, false);
            check(r.locator, false);
        }
    }
    assert!(missing.is_empty(), "no glyph for {missing:?}");
}
