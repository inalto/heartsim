//! Pericardium: effusion size, respiratory variation of mitral, tricuspid and LVOT flow, the
//! hepatic-vein expiratory reversal ratio and the septal-to-lateral e' ratio.

use super::{choice, cite, n, num, outcome, q, when};
use crate::grade::{Scale, band, criterion_met, ge, gt};
use crate::model::ExampleValue::{Choice as C, Number as N};
use crate::model::{Calculator, Category, Choice, Cite, Inputs, Outcome, Review, Severity, Unit};
use crate::refs::{DD_2025, PERI_2013};
use crate::tx;

const TAMPONADE: Cite = cite(&PERI_2013, "Tamponade key points, p. 991");
const CONSTRICTION: Cite = cite(&PERI_2013, "Constriction key points, p. 994");

static EFFUSION_SCALE: Scale = Scale {
    what: tx!("Effusion", "Versamento"),
    symbol: "separation",
    unit: Unit::Mm,
    decimals: 0,
    bands: &[
        band(10.0, false, Severity::Mild, tx!("small", "piccolo")),
        band(20.0, true, Severity::Moderate, tx!("moderate", "moderato")),
        band(25.0, true, Severity::Severe, tx!("large", "grande")),
    ],
    above: (Severity::Severe, tx!("very large", "molto grande")),
    source: cite(&PERI_2013, "Effusion size, p. 981"),
};

/// Semiquantitative effusion size from the end-diastolic echo-free space.
pub static EFFUSION_SIZE: Calculator = Calculator {
    id: "pericardial-effusion-size",
    category: Category::Pericardium,
    name: tx!("Pericardial effusion size", "Entità del versamento pericardico"),
    synonyms: tx!(
        "pericardial effusion, PEff, echo-free space, separation, tamponade",
        "versamento pericardico, spazio ecoprivo, separazione, tamponamento"
    ),
    summary: tx!(
        "The largest end-diastolic echo-free space between the pericardial layers. An effusion seen only in systole is trivial. Size alone does not say whether there is tamponade: that depends on how fast the fluid gathered.",
        "Il massimo spazio ecoprivo telediastolico tra i foglietti pericardici. Un versamento visibile solo in sistole è minimo. L'entità da sola non dice se c'è tamponamento: dipende dalla velocità di accumulo."
    ),
    inputs: &[num(
        "separation_mm",
        tx!(
            "Largest end-diastolic separation",
            "Massima separazione telediastolica"
        ),
        Unit::Mm,
        0.5,
        80.0,
        0,
    )],
    compute: |i| {
        let s = i.n("separation_mm");
        let mut out = outcome();
        out.push(
            q("separation_mm", tx!("Separation", "Separazione"), s, Unit::Mm, 0),
            "end-diastolic echo-free space",
            n(s, 0) + " mm",
        );
        out.findings.push(EFFUSION_SCALE.finding(s));
        Ok(out)
    },
    references: &[cite(&PERI_2013, "2D echocardiography of effusions, p. 981")],
    review: Review::Verified,
    example: &[("separation_mm", N(14.0))],
    sim_link: None,
};

/// (expiration − inspiration) ÷ expiration × 100, the writing group's convention.
fn variation(i: &Inputs, out: &mut Outcome, symbol: &str) -> f64 {
    let (e, insp) = (i.n("exp_cm_s"), i.n("insp_cm_s"));
    let pct = (e - insp) / e * 100.0;
    out.push(
        q(
            "variation_percent",
            tx!("Respiratory variation", "Variazione respiratoria"),
            pct,
            Unit::Percent,
            0,
        ),
        &format!("({symbol}exp − {symbol}insp) ÷ {symbol}exp × 100"),
        format!("({} − {}) ÷ {} × 100", n(e, 0), n(insp, 0), n(e, 0)),
    );
    pct
}

/// Inspiratory fall of mitral E.
pub static MITRAL_RESPIRATORY: Calculator = Calculator {
    id: "mitral-respiratory-variation",
    category: Category::Pericardium,
    name: tx!(
        "Mitral inflow respiratory variation",
        "Variazione respiratoria dell'afflusso mitralico"
    ),
    synonyms: tx!(
        "mitral E, respiratory variation, tamponade, constriction, pulsus paradoxus, ventricular interdependence",
        "E mitralica, variazione respiratoria, tamponamento, costrizione, polso paradosso, interdipendenza ventricolare"
    ),
    summary: tx!(
        "Peak mitral E on the first beat of expiration and of inspiration, with a respirometer trace. The guideline's convention is (expiration − inspiration) ÷ expiration. Not a stand-alone sign of tamponade.",
        "E mitralica di picco al primo battito dell'espirazione e dell'inspirazione, con traccia respiratoria. La convenzione della linea guida è (espirazione − inspirazione) ÷ espirazione. Non è un segno isolato di tamponamento."
    ),
    inputs: &[
        num(
            "exp_cm_s",
            tx!(
                "Peak E, first expiratory beat",
                "E di picco, primo battito espiratorio"
            ),
            Unit::CmS,
            10.0,
            250.0,
            0,
        ),
        num(
            "insp_cm_s",
            tx!(
                "Peak E, first inspiratory beat",
                "E di picco, primo battito inspiratorio"
            ),
            Unit::CmS,
            10.0,
            250.0,
            0,
        ),
    ],
    compute: |i| {
        let mut out = outcome();
        let pct = variation(i, &mut out, "E");
        if gt(pct, 30.0) {
            out.findings.push(criterion_met(
                Severity::Info,
                tx!(
                    "Above the 30 % the guideline describes in tamponade",
                    "Sopra il 30 % che la linea guida descrive nel tamponamento"
                ),
                "fall > 30 %",
                TAMPONADE,
            ));
        }
        if gt(pct, 25.0) {
            out.findings.push(criterion_met(
                Severity::Info,
                tx!(
                    "Above the 25 % the guideline describes in constriction",
                    "Sopra il 25 % che la linea guida descrive nella costrizione"
                ),
                "fall > 25 %",
                CONSTRICTION,
            ));
        } else {
            out.findings.push(criterion_met(
                Severity::Normal,
                tx!(
                    "Not above the 25 % described in constriction",
                    "Non oltre il 25 % descritto nella costrizione"
                ),
                "fall ≤ 25 %",
                CONSTRICTION,
            ));
        }
        Ok(out)
    },
    references: &[cite(
        &PERI_2013,
        "Tamponade p. 991; constriction p. 994; Figures A5–A6, p. 1012.e4",
    )],
    review: Review::Verified,
    example: &[("exp_cm_s", N(88.0)), ("insp_cm_s", N(61.0))],
    sim_link: None,
};

/// Inspiratory rise of tricuspid E (a negative variation by the guideline's convention).
pub static TRICUSPID_RESPIRATORY: Calculator = Calculator {
    id: "tricuspid-respiratory-variation",
    category: Category::Pericardium,
    name: tx!(
        "Tricuspid inflow respiratory variation",
        "Variazione respiratoria dell'afflusso tricuspidale"
    ),
    synonyms: tx!(
        "tricuspid E, respiratory variation, tamponade, constriction",
        "E tricuspidale, variazione respiratoria, tamponamento, costrizione"
    ),
    summary: tx!(
        "Peak tricuspid E rises with inspiration, so (expiration − inspiration) ÷ expiration comes out negative; its magnitude is compared with the guideline's figures.",
        "La E tricuspidale di picco aumenta con l'inspirazione, quindi (espirazione − inspirazione) ÷ espirazione risulta negativo; il valore assoluto si confronta con le cifre della linea guida."
    ),
    inputs: &[
        num(
            "exp_cm_s",
            tx!(
                "Peak E, first expiratory beat",
                "E di picco, primo battito espiratorio"
            ),
            Unit::CmS,
            10.0,
            200.0,
            0,
        ),
        num(
            "insp_cm_s",
            tx!(
                "Peak E, first inspiratory beat",
                "E di picco, primo battito inspiratorio"
            ),
            Unit::CmS,
            10.0,
            200.0,
            0,
        ),
    ],
    compute: |i| {
        let mut out = outcome();
        let rise = -variation(i, &mut out, "E");
        if gt(rise, 60.0) {
            out.findings.push(criterion_met(
                Severity::Info,
                tx!(
                    "An inspiratory rise above the 60 % the guideline describes in tamponade",
                    "Un aumento inspiratorio oltre il 60 % che la linea guida descrive nel tamponamento"
                ),
                "|variation| > 60 % (inspiratory rise)",
                TAMPONADE,
            ));
        }
        if gt(rise, 40.0) {
            out.findings.push(criterion_met(
                Severity::Info,
                tx!(
                    "An inspiratory rise above the 40 % the guideline describes in constriction",
                    "Un aumento inspiratorio oltre il 40 % che la linea guida descrive nella costrizione"
                ),
                "|variation| > 40 % (inspiratory rise)",
                CONSTRICTION,
            ));
        } else {
            out.findings.push(criterion_met(
                Severity::Normal,
                tx!(
                    "Not an inspiratory rise above the 40 % described in constriction",
                    "Non un aumento inspiratorio oltre il 40 % descritto nella costrizione"
                ),
                "inspiratory rise ≤ 40 %",
                CONSTRICTION,
            ));
        }
        Ok(out)
    },
    references: &[cite(
        &PERI_2013,
        "Tamponade p. 991; constriction p. 994; Figures A5–A6, p. 1012.e4",
    )],
    review: Review::Verified,
    example: &[("exp_cm_s", N(38.0)), ("insp_cm_s", N(57.0))],
    sim_link: None,
};

const LVOT_MEASURE: &[Choice] = &[
    Choice {
        id: "peak",
        label: tx!("Peak velocity", "Velocità di picco"),
    },
    Choice {
        id: "vti",
        label: tx!("VTI", "VTI"),
    },
];

/// Inspiratory fall of LVOT velocity or VTI (pulsus paradoxus on Doppler).
pub static LVOT_RESPIRATORY: Calculator = Calculator {
    id: "lvot-respiratory-variation",
    category: Category::Pericardium,
    name: tx!("LVOT respiratory variation", "Variazione respiratoria nel LVOT"),
    synonyms: tx!(
        "LVOT, pulsus paradoxus, tamponade, aortic outflow, respiratory variation",
        "LVOT, polso paradosso, tamponamento, efflusso aortico, variazione respiratoria"
    ),
    summary: tx!(
        "In tamponade the LVOT velocity and ejection time fall with inspiration (Doppler pulsus paradoxus). Computed like the inflow variation; the guideline gives no cut-off for it.",
        "Nel tamponamento velocità ed eiezione nel LVOT calano con l'inspirazione (polso paradosso al Doppler). Calcolata come la variazione degli afflussi; la linea guida non dà una soglia."
    ),
    inputs: &[
        choice("measure", tx!("Measurement", "Misura"), LVOT_MEASURE),
        when(
            num(
                "exp_cm_s",
                tx!("Peak velocity, expiration", "Velocità di picco, espirazione"),
                Unit::CmS,
                20.0,
                250.0,
                0,
            ),
            "measure",
            &["peak"],
        ),
        when(
            num(
                "insp_cm_s",
                tx!("Peak velocity, inspiration", "Velocità di picco, inspirazione"),
                Unit::CmS,
                20.0,
                250.0,
                0,
            ),
            "measure",
            &["peak"],
        ),
        when(
            num(
                "exp_vti_cm",
                tx!("VTI, expiration", "VTI, espirazione"),
                Unit::Cm,
                3.0,
                50.0,
                1,
            ),
            "measure",
            &["vti"],
        ),
        when(
            num(
                "insp_vti_cm",
                tx!("VTI, inspiration", "VTI, inspirazione"),
                Unit::Cm,
                3.0,
                50.0,
                1,
            ),
            "measure",
            &["vti"],
        ),
    ],
    compute: |i| {
        let mut out = outcome();
        let (e, insp, sym) = if i.choice("measure") == Some("vti") {
            (i.n("exp_vti_cm"), i.n("insp_vti_cm"), "VTI")
        } else {
            (i.n("exp_cm_s"), i.n("insp_cm_s"), "V")
        };
        out.push(
            q(
                "variation_percent",
                tx!("Inspiratory fall", "Calo inspiratorio"),
                (e - insp) / e * 100.0,
                Unit::Percent,
                0,
            ),
            &format!("({sym}exp − {sym}insp) ÷ {sym}exp × 100"),
            format!("({} − {}) ÷ {} × 100", n(e, 1), n(insp, 1), n(e, 1)),
        );
        Ok(out)
    },
    references: &[cite(
        &PERI_2013,
        "Figure 38 and Doppler in tamponade, pp. 988–990",
    )],
    review: Review::FormulaOnly,
    example: &[
        ("measure", C("vti")),
        ("exp_vti_cm", N(18.0)),
        ("insp_vti_cm", N(13.5)),
    ],
    sim_link: None,
};

/// Expiratory diastolic flow reversal in the hepatic veins relative to forward flow.
pub static HEPATIC_REVERSAL: Calculator = Calculator {
    id: "hepatic-vein-reversal-ratio",
    category: Category::Pericardium,
    name: tx!(
        "Hepatic vein expiratory reversal ratio",
        "Rapporto di inversione espiratoria nelle vene sovraepatiche"
    ),
    synonyms: tx!(
        "hepatic vein, diastolic reversal, expiratory reversal, constriction, restrictive",
        "vene sovraepatiche, inversione diastolica, inversione espiratoria, costrizione, restrizione"
    ),
    summary: tx!(
        "End-diastolic reversal velocity divided by forward diastolic velocity in the hepatic veins during expiration. Expiratory reversal points to constriction; inspiratory reversal to restriction.",
        "Velocità di inversione telediastolica divisa per la velocità diastolica anterograda nelle vene sovraepatiche in espirazione. L'inversione espiratoria orienta verso la costrizione, quella inspiratoria verso la restrizione."
    ),
    inputs: &[
        num(
            "reversal_cm_s",
            tx!(
                "Expiratory diastolic reversal velocity",
                "Velocità di inversione diastolica espiratoria"
            ),
            Unit::CmS,
            0.0,
            100.0,
            0,
        ),
        num(
            "forward_cm_s",
            tx!(
                "Expiratory diastolic forward velocity",
                "Velocità diastolica anterograda espiratoria"
            ),
            Unit::CmS,
            5.0,
            120.0,
            0,
        ),
    ],
    compute: |i| {
        let (r, f) = (i.n("reversal_cm_s"), i.n("forward_cm_s"));
        let ratio = r / f;
        let mut out = outcome();
        out.push(
            q(
                "reversal_ratio",
                tx!("Reversal ratio", "Rapporto di inversione"),
                ratio,
                Unit::None,
                2,
            ),
            "Dreversal ÷ Dforward",
            format!("{} ÷ {}", n(r, 0), n(f, 0)),
        );
        let src = cite(&DD_2025, "Pericardial constriction, p. 561");
        out.findings.push(if ge(ratio, 0.8) {
            criterion_met(
                Severity::Info,
                tx!(
                    "At or above the 0.8 the guideline lists among the findings of constriction",
                    "Pari o superiore allo 0,8 che la linea guida elenca tra i reperti di costrizione"
                ),
                "ratio ≥ 0.8",
                src,
            )
        } else {
            criterion_met(
                Severity::Normal,
                tx!(
                    "Below the 0.8 the guideline lists in constriction",
                    "Sotto lo 0,8 che la linea guida elenca nella costrizione"
                ),
                "ratio < 0.8",
                src,
            )
        });
        Ok(out)
    },
    references: &[cite(&DD_2025, "Pericardial constriction and key points, p. 561")],
    review: Review::Verified,
    example: &[("reversal_cm_s", N(26.0)), ("forward_cm_s", N(30.0))],
    sim_link: None,
};

/// Septal (medial) over lateral e': above 1 in "annulus reversus".
pub static SEPTAL_LATERAL_E_PRIME: Calculator = Calculator {
    id: "septal-lateral-e-prime-ratio",
    category: Category::Pericardium,
    name: tx!("Septal-to-lateral e' ratio", "Rapporto e' settale/laterale"),
    synonyms: tx!(
        "annulus reversus, medial e', lateral e', constriction, tethering",
        "annulus reversus, e' mediale, e' laterale, costrizione, ancoraggio"
    ),
    summary: tx!(
        "Normally lateral e' exceeds septal e'. In constriction the lateral wall is tethered and the ratio rises ('annulus reversus'). The guideline gives no numeric cut-off, so none is applied.",
        "Normalmente la e' laterale supera quella settale. Nella costrizione la parete laterale è ancorata e il rapporto aumenta ('annulus reversus'). La linea guida non dà una soglia numerica, quindi non se ne applica alcuna."
    ),
    inputs: &[
        num(
            "e_sep_cm_s",
            tx!("Septal (medial) e'", "e' settale (mediale)"),
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
    compute: |i| {
        let (s, l) = (i.n("e_sep_cm_s"), i.n("e_lat_cm_s"));
        let mut out = outcome();
        out.push(
            q(
                "ratio",
                tx!("Septal ÷ lateral e'", "e' settale ÷ laterale"),
                s / l,
                Unit::None,
                2,
            ),
            "e' septal ÷ e' lateral",
            format!("{} ÷ {}", n(s, 1), n(l, 1)),
        );
        Ok(out)
    },
    references: &[cite(&DD_2025, "Pericardial constriction, p. 561")],
    review: Review::FormulaOnly,
    example: &[("e_sep_cm_s", N(11.0)), ("e_lat_cm_s", N(8.5))],
    sim_link: None,
};

#[cfg(test)]
mod tests {
    use super::super::testing::{close, ok, severities};
    use super::*;
    use crate::model::Severity;

    #[test]
    fn effusion_bands() {
        assert_eq!(EFFUSION_SCALE.class(9.9).1.en, "small");
        assert_eq!(EFFUSION_SCALE.class(10.0).1.en, "moderate");
        assert_eq!(EFFUSION_SCALE.class(20.0).1.en, "moderate");
        assert_eq!(EFFUSION_SCALE.class(21.0).1.en, "large");
        assert_eq!(EFFUSION_SCALE.class(25.0).1.en, "large");
        assert_eq!(EFFUSION_SCALE.class(26.0).1.en, "very large");
    }

    #[test]
    fn respiratory_variation_against_the_appendix_example() {
        // Figure A6 caption, p. 1012.e4: tricuspid E 34 (expiration) and 64 (inspiration):
        // (34 − 64) ÷ 34 = −88 % (the caption prints 87 %), i.e. an inspiratory rise > 60 %
        let o = ok(
            &TRICUSPID_RESPIRATORY,
            &Inputs::new().with("exp_cm_s", 34.0).with("insp_cm_s", 64.0),
        );
        close(o.value("variation_percent"), -88.24, 0.01);
        assert_eq!(o.findings.len(), 2);
        // mitral, Figure A5 on the same page: (92 − 76) ÷ 92 = 17 %
        let o = ok(
            &MITRAL_RESPIRATORY,
            &Inputs::new().with("exp_cm_s", 92.0).with("insp_cm_s", 76.0),
        );
        close(o.value("variation_percent"), 17.39, 0.01);
        assert_eq!(severities(&o), [Severity::Normal]);
        // 25 % exactly does not exceed 25; 30 % exceeds 25 but not 30
        let o = ok(
            &MITRAL_RESPIRATORY,
            &Inputs::new().with("exp_cm_s", 100.0).with("insp_cm_s", 75.0),
        );
        assert_eq!(severities(&o), [Severity::Normal]);
        let o = ok(
            &MITRAL_RESPIRATORY,
            &Inputs::new().with("exp_cm_s", 100.0).with("insp_cm_s", 70.0),
        );
        assert_eq!(severities(&o), [Severity::Info]);
        let o = ok(
            &MITRAL_RESPIRATORY,
            &Inputs::new().with("exp_cm_s", 100.0).with("insp_cm_s", 69.0),
        );
        assert_eq!(severities(&o), [Severity::Info, Severity::Info]);
    }

    #[test]
    fn lvot_hepatic_and_e_prime_ratio() {
        // (20 − 15) ÷ 20 = 25 %
        let o = ok(
            &LVOT_RESPIRATORY,
            &Inputs::new()
                .with_choice("measure", "vti")
                .with("exp_vti_cm", 20.0)
                .with("insp_vti_cm", 15.0),
        );
        close(o.value("variation_percent"), 25.0, 1e-9);
        assert_eq!(o.findings.len(), 0);
        // 24 ÷ 30 = 0.8 → meets
        let o = ok(
            &HEPATIC_REVERSAL,
            &Inputs::new()
                .with("reversal_cm_s", 24.0)
                .with("forward_cm_s", 30.0),
        );
        close(o.value("reversal_ratio"), 0.8, 1e-12);
        assert_eq!(severities(&o), [Severity::Info]);
        let o = ok(
            &HEPATIC_REVERSAL,
            &Inputs::new()
                .with("reversal_cm_s", 23.0)
                .with("forward_cm_s", 30.0),
        );
        assert_eq!(severities(&o), [Severity::Normal]);
        let o = ok(
            &SEPTAL_LATERAL_E_PRIME,
            &Inputs::new().with("e_sep_cm_s", 12.0).with("e_lat_cm_s", 8.0),
        );
        close(o.value("ratio"), 1.5, 1e-12);
        assert_eq!(o.findings.len(), 0);
    }
}
