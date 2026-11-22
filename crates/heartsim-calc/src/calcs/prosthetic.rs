//! Prosthetic valves: effective orifice area, Doppler velocity index and indexed area for aortic
//! and mitral prostheses (formulas only; the screening criteria are brief B29 part B).

use super::{check, cite, n, num, o, opt, outcome, q, v};
use crate::blocks::{circle_area_cm2, stroke_volume_ml};
use crate::model::ExampleValue::Number as N;
use crate::model::{Calculator, Category, InputError, InputSpec, Inputs, Outcome, Review, Unit};
use crate::refs::PV_2024;
use crate::tx;

const LVOT_D: InputSpec = num(
    "lvot_d_cm",
    tx!("LVOT diameter", "Diametro LVOT"),
    Unit::Cm,
    1.0,
    4.0,
    2,
);
const LVOT_VTI: InputSpec = num("lvot_vti_cm", tx!("LVOT VTI", "VTI LVOT"), Unit::Cm, 3.0, 60.0, 1);
const BSA: InputSpec = opt(
    "bsa_m2",
    tx!("Body surface area", "Superficie corporea"),
    Unit::M2,
    0.5,
    3.0,
    2,
);

fn eoa(i: &Inputs, mitral: bool) -> Result<Outcome, Vec<InputError>> {
    let (d, lvti, pvti) = (i.n("lvot_d_cm"), i.n("lvot_vti_cm"), i.n("prosthesis_vti_cm"));
    let mut out = outcome();
    let sv = stroke_volume_ml(d, lvti);
    out.push(
        q(
            "lvot_sv_ml",
            tx!("LVOT stroke volume", "Volume sistolico LVOT"),
            sv,
            Unit::Ml,
            0,
        ),
        "SV = π × (d/2)² × VTI(LVOT)",
        format!(
            "{} × {}",
            v(circle_area_cm2(d), 2, Unit::Cm2),
            v(lvti, 1, Unit::Cm)
        ),
    );
    let area = sv / pvti;
    out.push(
        q(
            "eoa_cm2",
            tx!("Effective orifice area", "Area effettiva dell'orifizio"),
            area,
            Unit::Cm2,
            2,
        ),
        "EOA = SV ÷ VTI(prosthesis)",
        format!("{} ÷ {}", n(sv, 0), v(pvti, 0, Unit::Cm)),
    );
    if mitral {
        out.push(
            q(
                "dvi",
                tx!(
                    "Doppler velocity index (VTI prosthesis ÷ VTI LVOT)",
                    "Indice di velocità Doppler (VTI protesi ÷ VTI LVOT)"
                ),
                pvti / lvti,
                Unit::None,
                2,
            ),
            "DVI = VTI(PrMV) ÷ VTI(LVOT)",
            format!("{} ÷ {}", n(pvti, 0), n(lvti, 1)),
        );
    } else {
        check(
            lvti > pvti,
            "lvot_vti_cm",
            tx!(
                "The LVOT VTI should not exceed the prosthesis VTI",
                "La VTI del LVOT non dovrebbe superare quella della protesi"
            ),
        )?;
        out.push(
            q(
                "dvi",
                tx!(
                    "Doppler velocity index (VTI LVOT ÷ VTI prosthesis)",
                    "Indice di velocità Doppler (VTI LVOT ÷ VTI protesi)"
                ),
                lvti / pvti,
                Unit::None,
                2,
            ),
            "DVI = VTI(LVOT) ÷ VTI(PrAV)",
            format!("{} ÷ {}", n(lvti, 1), n(pvti, 0)),
        );
    }
    if let Some(bsa) = o(i, "bsa_m2") {
        out.push(
            q(
                "ieoa_cm2_m2",
                tx!("Indexed EOA", "EOA indicizzata"),
                area / bsa,
                Unit::Cm2M2,
                2,
            ),
            "iEOA = EOA ÷ BSA",
            format!("{} ÷ {}", n(area, 2), v(bsa, 2, Unit::M2)),
        );
    }
    Ok(out)
}

/// Aortic prosthesis EOA by continuity, DVI and indexed EOA.
pub static AORTIC_PROSTHESIS: Calculator = Calculator {
    id: "aortic-prosthesis-eoa",
    category: Category::Prosthetic,
    name: tx!("Aortic prosthesis EOA and DVI", "EOA e DVI della protesi aortica"),
    synonyms: tx!(
        "prosthetic aortic valve, SAVR, TAVI, TAVR, EOA, iEOA, DVI, Doppler velocity index, patient–prosthesis mismatch",
        "protesi aortica, SAVR, TAVI, EOA, iEOA, DVI, indice di velocità Doppler, mismatch protesi-paziente"
    ),
    summary: tx!(
        "Continuity across the prosthesis: EOA = LVOT stroke volume ÷ prosthesis VTI; the DVI (LVOT VTI ÷ prosthesis VTI) needs no diameter. Use the largest LVOT diameter, measured just below the sewing ring or stent.",
        "Continuità attraverso la protesi: EOA = volume sistolico LVOT ÷ VTI della protesi; il DVI (VTI LVOT ÷ VTI protesi) non richiede diametri. Usare il diametro LVOT più grande, misurato appena sotto l'anello o lo stent."
    ),
    inputs: &[
        LVOT_D,
        LVOT_VTI,
        num(
            "prosthesis_vti_cm",
            tx!(
                "Prosthesis VTI (continuous wave)",
                "VTI della protesi (Doppler continuo)"
            ),
            Unit::Cm,
            8.0,
            200.0,
            0,
        ),
        BSA,
    ],
    compute: |i| eoa(i, false),
    references: &[cite(&PV_2024, "EOA p. 7; DVI p. 9")],
    review: Review::FormulaOnly,
    example: &[
        ("lvot_d_cm", N(2.1)),
        ("lvot_vti_cm", N(21.0)),
        ("prosthesis_vti_cm", N(45.0)),
        ("bsa_m2", N(1.9)),
    ],
    sim_link: None,
};

/// Mitral prosthesis EOA by continuity, the mitral DVI and indexed EOA.
pub static MITRAL_PROSTHESIS: Calculator = Calculator {
    id: "mitral-prosthesis-eoa",
    category: Category::Prosthetic,
    name: tx!(
        "Mitral prosthesis EOA and DVI",
        "EOA e DVI della protesi mitralica"
    ),
    synonyms: tx!(
        "prosthetic mitral valve, EOA, iEOA, DVI, Doppler velocity index",
        "protesi mitralica, EOA, iEOA, DVI, indice di velocità Doppler"
    ),
    summary: tx!(
        "EOA = LVOT stroke volume ÷ VTI through the mitral prosthesis, valid without significant aortic or mitral regurgitation; the mitral DVI is the inverse ratio, prosthesis VTI ÷ LVOT VTI.",
        "EOA = volume sistolico LVOT ÷ VTI attraverso la protesi mitralica, valido senza insufficienza aortica o mitralica significativa; il DVI mitralico è il rapporto inverso, VTI protesi ÷ VTI LVOT."
    ),
    inputs: &[
        LVOT_D,
        LVOT_VTI,
        num(
            "prosthesis_vti_cm",
            tx!(
                "Prosthesis VTI (continuous wave)",
                "VTI della protesi (Doppler continuo)"
            ),
            Unit::Cm,
            8.0,
            200.0,
            0,
        ),
        BSA,
    ],
    compute: |i| eoa(i, true),
    references: &[cite(&PV_2024, "DVI p. 9; mitral EOA and DVI p. 24")],
    review: Review::FormulaOnly,
    example: &[
        ("lvot_d_cm", N(2.0)),
        ("lvot_vti_cm", N(20.0)),
        ("prosthesis_vti_cm", N(34.0)),
        ("bsa_m2", N(1.75)),
    ],
    sim_link: None,
};

#[cfg(test)]
mod tests {
    use super::super::testing::{close, inconsistent, ok};
    use super::*;

    #[test]
    fn prosthesis_hand_cases() {
        // SV π × 1² × 20 = 62.83; ÷ 50 = 1.257 cm²; DVI 20 ÷ 50 = 0.40; iEOA 1.257 ÷ 2 = 0.628
        let i = Inputs::new()
            .with("lvot_d_cm", 2.0)
            .with("lvot_vti_cm", 20.0)
            .with("prosthesis_vti_cm", 50.0)
            .with("bsa_m2", 2.0);
        let o = ok(&AORTIC_PROSTHESIS, &i);
        close(o.value("eoa_cm2"), 1.2566, 1e-3);
        close(o.value("dvi"), 0.40, 1e-12);
        close(o.value("ieoa_cm2_m2"), 0.6283, 1e-3);
        assert_eq!(o.findings.len(), 0);
        // mitral DVI is the inverse: 50 ÷ 20 = 2.5
        close(ok(&MITRAL_PROSTHESIS, &i).value("dvi"), 2.5, 1e-12);
        inconsistent(&AORTIC_PROSTHESIS, &i.with("lvot_vti_cm", 55.0), "lvot_vti_cm");
    }
}
