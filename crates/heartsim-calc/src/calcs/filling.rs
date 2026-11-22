//! Left-sided pressures from Doppler signals: E/e', left atrial pressure from the MR jet, LV
//! systolic and end-diastolic pressure from the MR and AR jets, and the left atrial volume index.

use super::{check, cite, n, num, outcome, q, v, when};
use crate::blocks::bernoulli_mmhg;
use crate::grade::{Scale, band, criterion_met, gt, lt};
use crate::model::ExampleValue::{Choice as C, Number as N};
use crate::model::{Calculator, Category, Choice, InputError, Inputs, Outcome, Review, Severity, Unit};
use crate::refs::{CQ_2015, DD_2025, GORCSAN_1991, NISHIMURA_1988, RH_2025};
use crate::tx;

/// Average E/e' and the pulmonary wedge pressure regression the right-heart guideline lists.
pub static E_E_PRIME: Calculator = Calculator {
    id: "mitral-e-to-e-prime",
    category: Category::FillingPressures,
    name: tx!("E/e' ratio", "Rapporto E/e'"),
    synonyms: tx!(
        "E/e', E over e prime, average E/e', septal, lateral, tissue Doppler, filling pressure, PCWP, wedge",
        "E/e', E su e primo, E/e' medio, settale, laterale, Doppler tissutale, pressione di riempimento, PCWP, pressione di incuneamento"
    ),
    summary: tx!(
        "Mitral E corrected for the speed of myocardial relaxation (annular e'): the average uses the mean of septal and lateral e'. Not accurate with heavy annular calcification, mitral prostheses, mitral disease or pericardial disease.",
        "La E mitralica corretta per la velocità di rilasciamento (e' anulare): la media usa e' settale e laterale. Non accurato con calcificazione anulare marcata, protesi mitraliche, valvulopatia mitralica o malattia pericardica."
    ),
    inputs: &[
        num(
            "e_cm_s",
            tx!("Mitral E velocity", "Velocità E mitralica"),
            Unit::CmS,
            20.0,
            250.0,
            0,
        ),
        num(
            "e_sep_cm_s",
            tx!("Septal e'", "e' settale"),
            Unit::CmS,
            2.0,
            25.0,
            1,
        ),
        num(
            "e_lat_cm_s",
            tx!("Lateral e'", "e' laterale"),
            Unit::CmS,
            2.0,
            30.0,
            1,
        ),
    ],
    compute: e_e_prime,
    references: &[
        cite(&DD_2025, "Table 3 (average E/e'), p. 541; Table 4, p. 548"),
        cite(&RH_2025, "Table 4 (PCWP = 1.24 × E/e' + 1.9), p. 163"),
    ],
    review: Review::Verified,
    example: &[
        ("e_cm_s", N(96.0)),
        ("e_sep_cm_s", N(5.5)),
        ("e_lat_cm_s", N(7.5)),
    ],
    sim_link: None,
};

fn e_e_prime(i: &Inputs) -> Result<Outcome, Vec<InputError>> {
    let (e, s, l) = (i.n("e_cm_s"), i.n("e_sep_cm_s"), i.n("e_lat_cm_s"));
    let mut out = outcome();
    let avg = f64::midpoint(s, l);
    out.push(
        q(
            "e_prime_avg_cm_s",
            tx!("Average e'", "e' media"),
            avg,
            Unit::CmS,
            1,
        ),
        "e' = (e' septal + e' lateral) ÷ 2",
        format!("({} + {}) ÷ 2", n(s, 1), n(l, 1)),
    );
    let ratio = e / avg;
    out.push(
        q(
            "e_e_prime_avg",
            tx!("Average E/e'", "E/e' medio"),
            ratio,
            Unit::None,
            1,
        ),
        "E/e' = E ÷ e'(average)",
        format!("{} ÷ {}", n(e, 0), n(avg, 1)),
    );
    out.push(
        q(
            "e_e_prime_sep",
            tx!("Septal E/e'", "E/e' settale"),
            e / s,
            Unit::None,
            1,
        ),
        "E/e'(septal) = E ÷ e' septal",
        format!("{} ÷ {}", n(e, 0), n(s, 1)),
    );
    out.push(
        q(
            "e_e_prime_lat",
            tx!("Lateral E/e'", "E/e' laterale"),
            e / l,
            Unit::None,
            1,
        ),
        "E/e'(lateral) = E ÷ e' lateral",
        format!("{} ÷ {}", n(e, 0), n(l, 1)),
    );
    let pcwp = 1.24 * ratio + 1.9;
    out.push(
        q(
            "pcwp_mmhg",
            tx!(
                "Wedge pressure (regression estimate)",
                "Pressione di incuneamento (stima per regressione)"
            ),
            pcwp,
            Unit::MmHg,
            0,
        ),
        "PCWP = 1.24 × E/e' + 1.9",
        format!("1.24 × {} + 1.9", n(ratio, 1)),
    );
    let src = cite(&DD_2025, "Table 4, p. 548");
    out.findings.push(if lt(ratio, 8.0) {
        criterion_met(Severity::Normal, tx!("Average E/e' below 8: the guideline says such values usually indicate normal filling pressures", "E/e' medio sotto 8: la linea guida indica che di norma corrisponde a pressioni di riempimento normali"), "E/e' < 8", src)
    } else if gt(ratio, 14.0) {
        criterion_met(Severity::Moderate, tx!("Average E/e' above 14: highly specific for increased filling pressures in the guideline", "E/e' medio sopra 14: molto specifico di pressioni di riempimento aumentate secondo la linea guida"), "E/e' > 14", src)
    } else {
        criterion_met(Severity::Info, tx!("Average E/e' in the 8–14 grey zone, where the guideline calls filling pressures indeterminate", "E/e' medio nella zona grigia 8–14, in cui la linea guida considera indeterminate le pressioni di riempimento"), "8 ≤ E/e' ≤ 14", src)
    });
    Ok(out)
}

/// Left atrial pressure as systolic blood pressure minus the LV–LA gradient of the MR jet.
pub static LAP_FROM_MR: Calculator = Calculator {
    id: "lap-from-mr-jet",
    category: Category::FillingPressures,
    name: tx!(
        "Left atrial pressure from the MR jet",
        "Pressione atriale sinistra dal getto di IM"
    ),
    synonyms: tx!(
        "LAP, mitral regurgitation, systolic blood pressure, Bernoulli, LV–LA gradient",
        "PAS sinistra, insufficienza mitralica, pressione arteriosa sistolica, Bernoulli, gradiente VS–AS"
    ),
    summary: tx!(
        "Without outflow obstruction the brachial systolic pressure stands in for LV systolic pressure; subtracting the peak LV–LA gradient (4 × MR Vmax²) leaves the left atrial pressure. Needs a complete MR envelope.",
        "In assenza di ostruzione all'efflusso la pressione sistolica brachiale sostituisce quella sistolica del VS; sottraendo il gradiente VS–AS di picco (4 × Vmax IM²) resta la pressione atriale sinistra. Serve un inviluppo completo dell'IM."
    ),
    inputs: &[
        num(
            "sbp_mmhg",
            tx!("Systolic blood pressure", "Pressione arteriosa sistolica"),
            Unit::MmHg,
            60.0,
            250.0,
            0,
        ),
        num(
            "mr_vmax_m_s",
            tx!("Peak MR velocity", "Velocità di picco dell'IM"),
            Unit::MS,
            2.0,
            7.0,
            2,
        ),
    ],
    compute: |i| {
        let (sbp, vmax) = (i.n("sbp_mmhg"), i.n("mr_vmax_m_s"));
        let g = bernoulli_mmhg(vmax);
        check(
            g >= sbp,
            "mr_vmax_m_s",
            tx!(
                "The MR gradient exceeds the systolic pressure",
                "Il gradiente dell'IM supera la pressione sistolica"
            ),
        )?;
        let mut out = outcome();
        out.push(
            q(
                "lv_la_gradient_mmhg",
                tx!("Peak LV–LA gradient", "Gradiente VS–AS di picco"),
                g,
                Unit::MmHg,
                0,
            ),
            "ΔP = 4 × Vmax²",
            format!("4 × {}²", n(vmax, 2)),
        );
        out.push(
            q(
                "lap_mmhg",
                tx!("Left atrial pressure", "Pressione atriale sinistra"),
                sbp - g,
                Unit::MmHg,
                0,
            ),
            "LAP = SBP − ΔP",
            format!("{} − {}", n(sbp, 0), v(g, 0, Unit::MmHg)),
        );
        Ok(out)
    },
    references: &[cite(
        &GORCSAN_1991,
        "Abstract: LAP = SBP − MR gradient, Am Heart J 121:858",
    )],
    review: Review::FormulaOnly,
    example: &[("sbp_mmhg", N(110.0)), ("mr_vmax_m_s", N(4.6))],
    sim_link: None,
};

const PRESSURE: &[Choice] = &[
    Choice {
        id: "systolic",
        label: tx!(
            "LV systolic pressure (MR jet)",
            "Pressione sistolica VS (getto di IM)"
        ),
    },
    Choice {
        id: "end_diastolic",
        label: tx!(
            "LV end-diastolic pressure (AR jet)",
            "Pressione telediastolica VS (getto di IA)"
        ),
    },
];

/// LV systolic pressure from the MR jet plus LAP, or LV end-diastolic pressure as diastolic blood
/// pressure minus the end-diastolic AR gradient.
pub static LV_PRESSURE_FROM_JETS: Calculator = Calculator {
    id: "lv-pressure-from-jets",
    category: Category::FillingPressures,
    name: tx!(
        "LV pressures from regurgitant jets",
        "Pressioni del VS dai getti rigurgitanti"
    ),
    synonyms: tx!(
        "LVSP, LVEDP, mitral regurgitation, aortic regurgitation, end-diastolic velocity, diastolic blood pressure",
        "PSVS, PTDVS, insufficienza mitralica, insufficienza aortica, velocità telediastolica, pressione diastolica"
    ),
    summary: tx!(
        "The regurgitant jets measure pressure differences: LV systolic = 4 × MR Vmax² + LAP; LV end-diastolic = diastolic blood pressure (≈ aortic end-diastolic) − 4 × AR end-diastolic velocity².",
        "I getti rigurgitanti misurano differenze di pressione: sistolica VS = 4 × Vmax IM² + PAS sinistra; telediastolica VS = pressione diastolica (≈ telediastolica aortica) − 4 × velocità telediastolica IA²."
    ),
    inputs: &[
        super::choice(
            "pressure",
            tx!("Pressure to estimate", "Pressione da stimare"),
            PRESSURE,
        ),
        when(
            num(
                "mr_vmax_m_s",
                tx!("Peak MR velocity", "Velocità di picco dell'IM"),
                Unit::MS,
                2.0,
                7.0,
                2,
            ),
            "pressure",
            &["systolic"],
        ),
        when(
            num(
                "lap_mmhg",
                tx!(
                    "Left atrial pressure (independent estimate)",
                    "Pressione atriale sinistra (stima indipendente)"
                ),
                Unit::MmHg,
                0.0,
                40.0,
                0,
            ),
            "pressure",
            &["systolic"],
        ),
        when(
            num(
                "dbp_mmhg",
                tx!("Diastolic blood pressure", "Pressione arteriosa diastolica"),
                Unit::MmHg,
                30.0,
                130.0,
                0,
            ),
            "pressure",
            &["end_diastolic"],
        ),
        when(
            num(
                "ar_ed_v_m_s",
                tx!("AR end-diastolic velocity", "Velocità telediastolica dell'IA"),
                Unit::MS,
                0.5,
                6.0,
                2,
            ),
            "pressure",
            &["end_diastolic"],
        ),
    ],
    compute: |i| {
        let mut out = outcome();
        if i.choice("pressure") == Some("systolic") {
            let (vmax, lap) = (i.n("mr_vmax_m_s"), i.n("lap_mmhg"));
            let g = bernoulli_mmhg(vmax);
            out.push(
                q(
                    "lvsp_mmhg",
                    tx!("LV systolic pressure", "Pressione sistolica del VS"),
                    g + lap,
                    Unit::MmHg,
                    0,
                ),
                "LVSP = 4 × Vmax(MR)² + LAP",
                format!("4 × {}² + {}", n(vmax, 2), n(lap, 0)),
            );
        } else {
            let (dbp, ved) = (i.n("dbp_mmhg"), i.n("ar_ed_v_m_s"));
            let g = bernoulli_mmhg(ved);
            check(
                g > dbp,
                "ar_ed_v_m_s",
                tx!(
                    "The AR gradient exceeds the diastolic pressure",
                    "Il gradiente dell'IA supera la pressione diastolica"
                ),
            )?;
            out.push(
                q(
                    "lvedp_mmhg",
                    tx!("LV end-diastolic pressure", "Pressione telediastolica del VS"),
                    dbp - g,
                    Unit::MmHg,
                    0,
                ),
                "LVEDP = DBP − 4 × V(AR, end-diastole)²",
                format!("{} − 4 × {}²", n(dbp, 0), n(ved, 2)),
            );
        }
        Ok(out)
    },
    references: &[cite(
        &NISHIMURA_1988,
        "Abstract: LV–LA and aorta–LV gradients, J Am Coll Cardiol 11:317",
    )],
    review: Review::FormulaOnly,
    example: &[
        ("pressure", C("end_diastolic")),
        ("dbp_mmhg", N(60.0)),
        ("ar_ed_v_m_s", N(3.4)),
    ],
    sim_link: None,
};

/// Table 4 of the 2015 chamber recommendation (same partitions for men and women): 16–34 normal,
/// 35–41 mild, 42–48 moderate, > 48 mL/m² severe; values between two whole numbers go up.
static LAVI_SCALE: Scale = Scale {
    what: tx!("LA volume index", "Volume atriale sinistro indicizzato"),
    symbol: "LAVi",
    unit: Unit::MlM2,
    decimals: 0,
    bands: &[
        band(
            16.0,
            false,
            Severity::Info,
            tx!(
                "below the listed normal range",
                "sotto l'intervallo normale indicato"
            ),
        ),
        band(34.0, true, Severity::Normal, tx!("normal", "normale")),
        band(
            41.0,
            true,
            Severity::Mild,
            tx!("mildly abnormal", "lievemente anormale"),
        ),
        band(
            48.0,
            true,
            Severity::Moderate,
            tx!("moderately abnormal", "moderatamente anormale"),
        ),
    ],
    above: (Severity::Severe, tx!("severely abnormal", "gravemente anormale")),
    source: cite(&CQ_2015, "Table 4, p. 10"),
};

/// Maximum left atrial volume (biplane) indexed to body surface area.
pub static LA_VOLUME_INDEX: Calculator = Calculator {
    id: "la-volume-index",
    category: Category::FillingPressures,
    name: tx!("Left atrial volume index", "Volume atriale sinistro indicizzato"),
    synonyms: tx!(
        "LAVi, LAVI, left atrium, biplane, disc summation, area-length",
        "LAVi, atrio sinistro, biplano, sommatoria dei dischi, area-lunghezza"
    ),
    summary: tx!(
        "Maximal LA volume at end-systole (biplane disc summation or area–length) divided by BSA. Reflects the cumulative effect of filling pressures over time.",
        "Volume massimo dell'atrio sinistro a fine sistole (sommatoria dei dischi biplana o area-lunghezza) diviso per la superficie corporea. Riflette l'effetto cumulativo delle pressioni di riempimento nel tempo."
    ),
    inputs: &[
        num(
            "la_volume_ml",
            tx!("Maximal LA volume (biplane)", "Volume massimo AS (biplano)"),
            Unit::Ml,
            10.0,
            300.0,
            0,
        ),
        num(
            "bsa_m2",
            tx!("Body surface area", "Superficie corporea"),
            Unit::M2,
            0.5,
            3.0,
            2,
        ),
    ],
    compute: |i| {
        let (vol, bsa) = (i.n("la_volume_ml"), i.n("bsa_m2"));
        let lavi = vol / bsa;
        let mut out = outcome();
        out.push(
            q(
                "lavi_ml_m2",
                tx!("LA volume index", "Volume AS indicizzato"),
                lavi,
                Unit::MlM2,
                0,
            ),
            "LAVi = LA volume ÷ BSA",
            format!("{} ÷ {}", v(vol, 0, Unit::Ml), v(bsa, 2, Unit::M2)),
        );
        out.findings.push(LAVI_SCALE.finding(lavi));
        Ok(out)
    },
    references: &[cite(&CQ_2015, "LA volume §9.3, p. 25; Table 4, p. 10")],
    review: Review::Verified,
    example: &[("la_volume_ml", N(74.0)), ("bsa_m2", N(1.9))],
    sim_link: None,
};

#[cfg(test)]
mod tests {
    use super::super::testing::{close, inconsistent, ok, severities};
    use super::*;

    #[test]
    fn e_e_prime_hand_case_and_grey_zone() {
        // e' (6 + 10) ÷ 2 = 8; 120 ÷ 8 = 15 (> 14); PCWP 1.24 × 15 + 1.9 = 20.5
        let o = ok(
            &E_E_PRIME,
            &Inputs::new()
                .with("e_cm_s", 120.0)
                .with("e_sep_cm_s", 6.0)
                .with("e_lat_cm_s", 10.0),
        );
        close(o.value("e_e_prime_avg"), 15.0, 1e-12);
        close(o.value("e_e_prime_sep"), 20.0, 1e-12);
        close(o.value("pcwp_mmhg"), 20.5, 1e-9);
        assert_eq!(severities(&o), [Severity::Moderate]);
        // boundaries: exactly 8 and 14 are in the grey zone; 7.9 normal, 14.1 increased
        for (e, sev) in [
            (79.0, Severity::Normal),
            (80.0, Severity::Info),
            (140.0, Severity::Info),
            (141.0, Severity::Moderate),
        ] {
            let o = ok(
                &E_E_PRIME,
                &Inputs::new()
                    .with("e_cm_s", e)
                    .with("e_sep_cm_s", 10.0)
                    .with("e_lat_cm_s", 10.0),
            );
            assert_eq!(severities(&o), [sev], "E {e}");
        }
    }

    #[test]
    fn lap_and_lv_pressures() {
        // 120 − 4 × 5² = 20 mmHg
        let o = ok(
            &LAP_FROM_MR,
            &Inputs::new().with("sbp_mmhg", 120.0).with("mr_vmax_m_s", 5.0),
        );
        close(o.value("lap_mmhg"), 20.0, 1e-12);
        assert_eq!(o.findings.len(), 0);
        inconsistent(
            &LAP_FROM_MR,
            &Inputs::new().with("sbp_mmhg", 90.0).with("mr_vmax_m_s", 5.0),
            "mr_vmax_m_s",
        );
        // 4 × 5² + 15 = 115; 70 − 4 × 4² = 6
        let o = ok(
            &LV_PRESSURE_FROM_JETS,
            &Inputs::new()
                .with_choice("pressure", "systolic")
                .with("mr_vmax_m_s", 5.0)
                .with("lap_mmhg", 15.0),
        );
        close(o.value("lvsp_mmhg"), 115.0, 1e-12);
        let o = ok(
            &LV_PRESSURE_FROM_JETS,
            &Inputs::new()
                .with_choice("pressure", "end_diastolic")
                .with("dbp_mmhg", 70.0)
                .with("ar_ed_v_m_s", 4.0),
        );
        close(o.value("lvedp_mmhg"), 6.0, 1e-12);
        // the inputs of the chosen pressure are required
        assert!(
            LV_PRESSURE_FROM_JETS
                .evaluate(
                    &Inputs::new()
                        .with_choice("pressure", "systolic")
                        .with("mr_vmax_m_s", 5.0)
                )
                .is_err()
        );
    }

    #[test]
    fn lavi_hand_case_and_boundaries() {
        // 68 ÷ 2.0 = 34 → normal (upper limit included)
        let o = ok(
            &LA_VOLUME_INDEX,
            &Inputs::new().with("la_volume_ml", 68.0).with("bsa_m2", 2.0),
        );
        close(o.value("lavi_ml_m2"), 34.0, 1e-12);
        assert_eq!(severities(&o), [Severity::Normal]);
        assert_eq!(LAVI_SCALE.class(34.5).0, Severity::Mild);
        assert_eq!(LAVI_SCALE.class(41.0).0, Severity::Mild);
        assert_eq!(LAVI_SCALE.class(42.0).0, Severity::Moderate);
        assert_eq!(LAVI_SCALE.class(48.0).0, Severity::Moderate);
        assert_eq!(LAVI_SCALE.class(48.5).0, Severity::Severe);
        assert_eq!(LAVI_SCALE.class(15.0).0, Severity::Info);
    }
}
