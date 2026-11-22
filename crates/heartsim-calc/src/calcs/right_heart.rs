//! Right heart and pulmonary circulation: right atrial pressure from the IVC, RVSP and PASP,
//! pulmonary diastolic and mean pressure, mean pressure from the acceleration time, Doppler PVR,
//! RV systolic function and RV–PA coupling, RV index of myocardial performance, RA and RV size,
//! and body surface area.
//!
//! The grading is Table 1 of the 2025 ASE right-heart guideline (p. 145), whose limits switch
//! between strict and inclusive from row to row; each scale below copies its row.

use super::words::{MILD_ABN, MOD_ABN, NORMAL, SEV_ABN};
use super::{check, choice, cite, n, need_one, num, o, opt, outcome, q, v, when};
use crate::blocks::{bernoulli_mmhg, bsa_du_bois_m2, bsa_haycock_m2, bsa_mosteller_m2, fraction_percent};
use crate::grade::{Band, Scale, band, criterion_met, ge, gt, le, lt};
use crate::model::ExampleValue::{Choice as C, Number as N};
use crate::model::{
    Calculator, Category, Choice, Cite, InputError, InputSpec, Inputs, Outcome, Review, Severity, Text, Unit,
};
use crate::refs::{DU_BOIS_1916, HAYCOCK_1978, MOSTELLER_1987, RH_2025};
use crate::tx;

const T1: Cite = cite(&RH_2025, "Table 1, p. 145");

/// Size rows of Table 1: normal < a, mild a to ≤ b, moderate > b to ≤ c, severe > c.
const fn size_bands(a: f64, b: f64, c: f64) -> [Band; 3] {
    [
        band(a, false, Severity::Normal, NORMAL),
        band(b, true, Severity::Mild, MILD_ABN),
        band(c, true, Severity::Moderate, MOD_ABN),
    ]
}

/// Function rows where low is abnormal: severe ≤ a, moderate ≤ b, mild ≤ c, normal > c.
const fn low_bands(a: f64, b: f64, c: f64) -> [Band; 3] {
    [
        band(a, true, Severity::Severe, SEV_ABN),
        band(b, true, Severity::Moderate, MOD_ABN),
        band(c, true, Severity::Mild, MILD_ABN),
    ]
}

macro_rules! scale {
    ($name:ident, $en:expr, $it:expr, $sym:expr, $unit:expr, $dec:expr, $bands:expr, $above:expr) => {
        static $name: Scale = Scale {
            what: tx!($en, $it),
            symbol: $sym,
            unit: $unit,
            decimals: $dec,
            bands: &$bands,
            above: $above,
            source: T1,
        };
    };
}

const SEV: (Severity, Text) = (Severity::Severe, SEV_ABN);
const NORM: (Severity, Text) = (Severity::Normal, NORMAL);

// --- haemodynamics
// TRV: < 2.8, ≥ 2.8 to ≤ 3.1, ≥ 3.2 to ≤ 3.5, ≥ 3.6 m/s (values in the 0.1 gaps go up).
scale!(
    TRV,
    "Peak TR velocity",
    "Velocità di picco dell'IT",
    "TRV",
    Unit::MS,
    1,
    [
        band(2.8, false, Severity::Normal, NORMAL),
        band(3.2, false, Severity::Mild, MILD_ABN),
        band(3.6, false, Severity::Moderate, MOD_ABN)
    ],
    SEV
);
// RVSP: ≤ 34, ≥ 35 to ≤ 49, ≥ 50 to ≤ 69, ≥ 70 mmHg (values in the 1 mmHg gaps go up).
scale!(
    RVSP,
    "RV systolic pressure",
    "Pressione sistolica del VD",
    "RVSP",
    Unit::MmHg,
    0,
    [
        band(35.0, false, Severity::Normal, NORMAL),
        band(50.0, false, Severity::Mild, MILD_ABN),
        band(70.0, false, Severity::Moderate, MOD_ABN)
    ],
    SEV
);
// AccT: > 105 normal, ≥ 80 to ≤ 105, ≥ 60 to < 80, < 60 ms.
scale!(
    ACCT,
    "RVOT acceleration time",
    "Tempo di accelerazione RVOT",
    "AccT",
    Unit::Ms,
    0,
    [
        band(60.0, false, Severity::Severe, SEV_ABN),
        band(80.0, false, Severity::Moderate, MOD_ABN),
        band(105.0, true, Severity::Mild, MILD_ABN)
    ],
    NORM
);

// --- RV systolic function
scale!(
    TAPSE,
    "TAPSE",
    "TAPSE",
    "TAPSE",
    Unit::Mm,
    0,
    low_bands(10.0, 13.0, 17.0),
    NORM
);
scale!(
    S_PRIME,
    "Tricuspid annular S'",
    "S' dell'anulus tricuspidale",
    "S'",
    Unit::CmS,
    1,
    low_bands(5.0, 7.2, 9.5),
    NORM
);
scale!(
    FAC,
    "Fractional area change",
    "Variazione frazionale di area",
    "FAC",
    Unit::Percent,
    0,
    low_bands(22.0, 29.0, 35.0),
    NORM
);
// 3D RVEF: > 45, ≤ 45 to > 39, ≤ 39 to ≥ 32, < 32 %.
scale!(
    RVEF,
    "3D RV ejection fraction",
    "Frazione di eiezione VD 3D",
    "RVEF",
    Unit::Percent,
    0,
    [
        band(32.0, false, Severity::Severe, SEV_ABN),
        band(39.0, true, Severity::Moderate, MOD_ABN),
        band(45.0, true, Severity::Mild, MILD_ABN)
    ],
    NORM
);
// Free-wall strain magnitude: > 20, ≤ 20 to ≥ 15, < 15 to ≥ 11, < 11 %.
scale!(
    FWS,
    "RV free-wall strain (magnitude)",
    "Strain della parete libera VD (valore assoluto)",
    "|FWLS|",
    Unit::Percent,
    0,
    [
        band(11.0, false, Severity::Severe, SEV_ABN),
        band(15.0, false, Severity::Moderate, MOD_ABN),
        band(20.0, true, Severity::Mild, MILD_ABN)
    ],
    NORM
);
// RV MPI: < a normal, ≥ a to < b, ≥ b to < c, ≥ c.
scale!(
    MPI_PW,
    "RV MPI (pulsed Doppler)",
    "MPI del VD (Doppler pulsato)",
    "MPI",
    Unit::None,
    2,
    [
        band(0.40, false, Severity::Normal, NORMAL),
        band(0.49, false, Severity::Mild, MILD_ABN),
        band(0.57, false, Severity::Moderate, MOD_ABN)
    ],
    SEV
);
scale!(
    MPI_TDI,
    "RV MPI (tissue Doppler)",
    "MPI del VD (Doppler tissutale)",
    "MPI",
    Unit::None,
    2,
    [
        band(0.55, false, Severity::Normal, NORMAL),
        band(0.62, false, Severity::Mild, MILD_ABN),
        band(0.70, false, Severity::Moderate, MOD_ABN)
    ],
    SEV
);

// --- RA and RV size (normal < a, mild a to ≤ b, moderate > b to ≤ c, severe > c)
scale!(
    RA_MAJOR,
    "RA major dimension",
    "Asse maggiore AD",
    "RA major",
    Unit::Cm,
    1,
    size_bands(5.4, 5.8, 6.3),
    SEV
);
scale!(
    RA_MINOR,
    "RA minor dimension",
    "Asse minore AD",
    "RA minor",
    Unit::Cm,
    1,
    size_bands(4.2, 4.7, 5.1),
    SEV
);
scale!(
    RA_AREA,
    "RA area",
    "Area AD",
    "RA area",
    Unit::Cm2,
    0,
    size_bands(19.0, 22.0, 24.0),
    SEV
);
scale!(
    RAVI_MOD,
    "RA volume index (disc method)",
    "Volume AD indicizzato (metodo dei dischi)",
    "RAVi",
    Unit::MlM2,
    0,
    size_bands(30.0, 36.0, 41.0),
    SEV
);
scale!(
    RAVI_AL,
    "RA volume index (area–length)",
    "Volume AD indicizzato (area-lunghezza)",
    "RAVi",
    Unit::MlM2,
    0,
    size_bands(33.0, 38.0, 44.0),
    SEV
);
scale!(
    RV_BASAL,
    "RV basal diameter",
    "Diametro basale VD",
    "RVD1",
    Unit::Cm,
    1,
    size_bands(4.1, 4.4, 4.9),
    SEV
);
scale!(
    RV_BASAL_I,
    "Indexed RV basal diameter",
    "Diametro basale VD indicizzato",
    "RVD1/BSA",
    Unit::CmM2,
    1,
    size_bands(2.4, 2.6, 2.9),
    SEV
);
scale!(
    RV_MID,
    "RV mid diameter",
    "Diametro medio VD",
    "RVD2",
    Unit::Cm,
    1,
    size_bands(3.5, 3.8, 4.2),
    SEV
);
scale!(
    RV_LONG,
    "RV longitudinal dimension",
    "Dimensione longitudinale VD",
    "RVD3",
    Unit::Cm,
    1,
    size_bands(8.2, 8.9, 9.6),
    SEV
);
scale!(
    RVOT_PLAX,
    "RVOT diameter (PLAX)",
    "Diametro RVOT (PLAX)",
    "RVOT PLAX",
    Unit::Cm,
    1,
    size_bands(3.3, 3.5, 3.9),
    SEV
);
scale!(
    RVOT_PROX,
    "Proximal RVOT diameter (PSAX)",
    "Diametro RVOT prossimale (PSAX)",
    "RVOT prox",
    Unit::Cm,
    1,
    size_bands(3.4, 3.8, 4.1),
    SEV
);
scale!(
    RVOT_DIST,
    "Distal RVOT diameter (PSAX)",
    "Diametro RVOT distale (PSAX)",
    "RVOT dist",
    Unit::Cm,
    1,
    size_bands(2.9, 3.0, 3.3),
    SEV
);
scale!(
    RVWT,
    "RV wall thickness",
    "Spessore parietale VD",
    "RVWT",
    Unit::Cm,
    1,
    size_bands(0.5, 0.7, 0.9),
    SEV
);
scale!(
    RV_EDA,
    "RV end-diastolic area",
    "Area telediastolica VD",
    "RVEDA",
    Unit::Cm2,
    0,
    size_bands(25.0, 28.0, 32.0),
    SEV
);
scale!(
    RV_EDA_I,
    "Indexed RV end-diastolic area",
    "Area telediastolica VD indicizzata",
    "RVEDAi",
    Unit::Cm2M2,
    0,
    size_bands(14.0, 15.0, 17.0),
    SEV
);
scale!(
    RV_ESA,
    "RV end-systolic area",
    "Area telesistolica VD",
    "RVESA",
    Unit::Cm2,
    0,
    size_bands(14.0, 16.0, 19.0),
    SEV
);
scale!(
    RV_ESA_I,
    "Indexed RV end-systolic area",
    "Area telesistolica VD indicizzata",
    "RVESAi",
    Unit::Cm2M2,
    0,
    size_bands(8.0, 9.0, 11.0),
    SEV
);
scale!(
    RV_EDV,
    "3D RV end-diastolic volume",
    "Volume telediastolico VD 3D",
    "RVEDV",
    Unit::Ml,
    0,
    size_bands(130.0, 150.0, 170.0),
    SEV
);
scale!(
    RV_EDV_I,
    "Indexed 3D RV end-diastolic volume",
    "Volume telediastolico VD 3D indicizzato",
    "RVEDVi",
    Unit::MlM2,
    0,
    size_bands(90.0, 103.0, 115.0),
    SEV
);
scale!(
    RV_ESV,
    "3D RV end-systolic volume",
    "Volume telesistolico VD 3D",
    "RVESV",
    Unit::Ml,
    0,
    size_bands(66.0, 77.0, 89.0),
    SEV
);
scale!(
    RV_ESV_I,
    "Indexed 3D RV end-systolic volume",
    "Volume telesistolico VD 3D indicizzato",
    "RVESVi",
    Unit::MlM2,
    0,
    size_bands(41.0, 48.0, 55.0),
    SEV
);

const RAP: InputSpec = num(
    "rap_mmhg",
    tx!(
        "Right atrial pressure (estimated)",
        "Pressione atriale destra (stimata)"
    ),
    Unit::MmHg,
    0.0,
    25.0,
    0,
);
const TR_VMAX: InputSpec = num(
    "tr_vmax_m_s",
    tx!("Peak TR velocity", "Velocità di picco dell'IT"),
    Unit::MS,
    1.0,
    6.0,
    2,
);
const BSA_OPT: InputSpec = opt(
    "bsa_m2",
    tx!("Body surface area", "Superficie corporea"),
    Unit::M2,
    0.5,
    3.0,
    2,
);

// ---------------------------------------------------------------------------------------------

const YES_NO: &[Choice] = &[
    Choice {
        id: "no",
        label: tx!("No", "No"),
    },
    Choice {
        id: "yes",
        label: tx!("Yes", "Sì"),
    },
];
const SECONDARY: &[Choice] = &[
    Choice {
        id: "unknown",
        label: tx!("Not assessed or uncertain", "Non valutati o incerti"),
    },
    Choice {
        id: "absent",
        label: tx!("None present", "Nessuno presente"),
    },
    Choice {
        id: "present",
        label: tx!("Present", "Presenti"),
    },
];

/// Right atrial pressure from the IVC diameter and its collapse with a sniff (Table 4 and text).
pub static RAP_FROM_IVC: Calculator = Calculator {
    id: "rap-from-ivc",
    category: Category::RightHeart,
    name: tx!(
        "Right atrial pressure from the IVC",
        "Pressione atriale destra dalla vena cava inferiore"
    ),
    synonyms: tx!(
        "RAP, CVP, IVC, inferior vena cava, sniff, collapsibility, central venous pressure",
        "PAD, PVC, VCI, vena cava inferiore, sniff, collassabilità, pressione venosa centrale"
    ),
    summary: tx!(
        "IVC measured at end-expiration 0.5–3 cm from the right atrium, then its smallest diameter with a sniff. A diameter cut-off of 2.1 cm and 50 % collapse assign 3, 8 or 15 mmHg; secondary signs move an indeterminate 8 to 3 or 15. Not usable during positive-pressure ventilation.",
        "VCI misurata a fine espirazione a 0,5–3 cm dall'atrio destro, poi il diametro minimo con lo sniff. Una soglia di 2,1 cm e il 50 % di collasso assegnano 3, 8 o 15 mmHg; i segni secondari portano un 8 indeterminato a 3 o a 15. Non utilizzabile in ventilazione a pressione positiva."
    ),
    inputs: &[
        num(
            "ivc_max_cm",
            tx!("IVC diameter, end-expiration", "Diametro VCI, fine espirazione"),
            Unit::Cm,
            0.3,
            4.5,
            2,
        ),
        num(
            "ivc_sniff_cm",
            tx!("IVC diameter with sniff", "Diametro VCI con sniff"),
            Unit::Cm,
            0.0,
            4.5,
            2,
        ),
        choice(
            "secondary",
            tx!("Secondary signs of raised RAP", "Segni secondari di PAD elevata"),
            SECONDARY,
        ),
        choice(
            "marked",
            tx!(
                "IVC > 2.5 cm with dilated hepatic veins and contrast or reflux",
                "VCI > 2,5 cm con vene sovraepatiche dilatate e contrasto o reflusso"
            ),
            YES_NO,
        ),
    ],
    compute: rap,
    references: &[cite(&RH_2025, "RAP section p. 160; Table 4, p. 161")],
    review: Review::Verified,
    example: &[
        ("ivc_max_cm", N(2.3)),
        ("ivc_sniff_cm", N(1.5)),
        ("secondary", C("absent")),
        ("marked", C("no")),
    ],
    sim_link: None,
};

fn rap(i: &Inputs) -> Result<Outcome, Vec<InputError>> {
    let (max, min) = (i.n("ivc_max_cm"), i.n("ivc_sniff_cm"));
    check(
        min > max,
        "ivc_sniff_cm",
        tx!(
            "The sniff diameter cannot exceed the expiratory one",
            "Il diametro con sniff non può superare quello espiratorio"
        ),
    )?;
    let collapse = fraction_percent(max, min);
    let mut out = outcome();
    out.push(
        q(
            "collapse_percent",
            tx!("Collapse with sniff", "Collasso con sniff"),
            collapse,
            Unit::Percent,
            0,
        ),
        "(Dmax − Dsniff) ÷ Dmax × 100",
        format!("({} − {}) ÷ {} × 100", n(max, 2), n(min, 2), n(max, 2)),
    );
    let small = le(max, 2.1);
    let collapses = ge(collapse, 50.0);
    let src = cite(&RH_2025, "Table 4, p. 161");
    let (rap, text, crit) = match (small, collapses) {
        (true, true) => (
            3.0,
            tx!(
                "Table 4 assigns 3 mmHg (range 0–5)",
                "La Tabella 4 assegna 3 mmHg (intervallo 0–5)"
            ),
            "IVC ≤ 2.1 cm and collapse ≥ 50 %",
        ),
        (false, false) if i.choice("marked") == Some("yes") && gt(max, 2.5) => (
            20.0,
            tx!(
                "Table 4 allows 20 mmHg for a markedly dilated, non-collapsing IVC with these signs",
                "La Tabella 4 ammette 20 mmHg per una VCI molto dilatata e non collassabile con questi segni"
            ),
            "IVC > 2.5 cm, collapse < 50 %, dilated hepatic veins, contrast or reflux",
        ),
        (false, false) => (
            15.0,
            tx!(
                "Table 4 assigns 15 mmHg (range 10–20)",
                "La Tabella 4 assegna 15 mmHg (intervallo 10–20)"
            ),
            "IVC > 2.1 cm and collapse < 50 %",
        ),
        _ => match i.choice("secondary") {
            Some("absent") => (
                3.0,
                tx!(
                    "Indeterminate pattern without secondary signs: the text downgrades it to 3 mmHg",
                    "Quadro indeterminato senza segni secondari: il testo lo riduce a 3 mmHg"
                ),
                "indeterminate IVC, no secondary signs",
            ),
            Some("present") => (
                15.0,
                tx!(
                    "Indeterminate pattern with secondary signs: the text upgrades it to 15 mmHg",
                    "Quadro indeterminato con segni secondari: il testo lo porta a 15 mmHg"
                ),
                "indeterminate IVC, secondary signs present",
            ),
            _ => (
                8.0,
                tx!(
                    "Indeterminate pattern: Table 4 assigns 8 mmHg (range 5–10)",
                    "Quadro indeterminato: la Tabella 4 assegna 8 mmHg (intervallo 5–10)"
                ),
                "IVC ≤ 2.1 cm with < 50 % collapse, or > 2.1 cm with ≥ 50 %",
            ),
        },
    };
    out.push(
        q(
            "rap_mmhg",
            tx!("Right atrial pressure", "Pressione atriale destra"),
            rap,
            Unit::MmHg,
            0,
        ),
        "RAP from IVC size and collapse",
        crit.to_owned(),
    );
    out.findings.push(criterion_met(Severity::Info, text, crit, src));
    Ok(out)
}

/// RV systolic pressure from the TR jet, and PASP when there is a pulmonic gradient to subtract.
pub static RVSP_PASP: Calculator = Calculator {
    id: "rvsp-pasp",
    category: Category::RightHeart,
    name: tx!(
        "RV systolic and pulmonary systolic pressure",
        "Pressione sistolica del VD e dell'arteria polmonare"
    ),
    synonyms: tx!(
        "RVSP, PASP, sPAP, TR velocity, TRV, pulmonary hypertension, Bernoulli",
        "PSVD, PAPs, velocità IT, ipertensione polmonare, Bernoulli"
    ),
    summary: tx!(
        "RVSP = 4 × TR Vmax² + RAP. Without RVOT, valve or proximal pulmonary-artery obstruction it equals the pulmonary systolic pressure; with a pulmonic gradient, PASP = RVSP − that gradient. Average at least three beats from the window with the highest dense envelope.",
        "PSVD = 4 × Vmax IT² + PAD. Senza ostruzione del RVOT, della valvola o dell'arteria polmonare prossimale è uguale alla pressione sistolica polmonare; con un gradiente polmonare, PAPs = PSVD − quel gradiente. Mediare almeno tre battiti dalla finestra con l'inviluppo denso più alto."
    ),
    inputs: &[
        TR_VMAX,
        RAP,
        opt(
            "ps_gradient_mmhg",
            tx!(
                "Peak RVOT or pulmonic gradient",
                "Gradiente di picco RVOT o polmonare"
            ),
            Unit::MmHg,
            0.0,
            150.0,
            0,
        ),
    ],
    compute: |i| {
        let (vmax, rap) = (i.n("tr_vmax_m_s"), i.n("rap_mmhg"));
        let rvsp = bernoulli_mmhg(vmax) + rap;
        let mut out = outcome();
        out.push(
            q(
                "rvsp_mmhg",
                tx!("RV systolic pressure", "Pressione sistolica del VD"),
                rvsp,
                Unit::MmHg,
                0,
            ),
            "RVSP = 4 × Vmax(TR)² + RAP",
            format!("4 × {}² + {}", n(vmax, 2), n(rap, 0)),
        );
        if let Some(g) = o(i, "ps_gradient_mmhg") {
            check(
                g >= rvsp,
                "ps_gradient_mmhg",
                tx!(
                    "The pulmonic gradient must be lower than the RV systolic pressure",
                    "Il gradiente polmonare deve essere inferiore alla pressione sistolica del VD"
                ),
            )?;
            out.push(
                q(
                    "pasp_mmhg",
                    tx!(
                        "Pulmonary artery systolic pressure",
                        "Pressione sistolica polmonare"
                    ),
                    rvsp - g,
                    Unit::MmHg,
                    0,
                ),
                "PASP = RVSP − ΔP(PS)",
                format!("{} − {}", n(rvsp, 0), n(g, 0)),
            );
        }
        out.findings.push(TRV.finding(vmax));
        out.findings.push(RVSP.finding(rvsp));
        Ok(out)
    },
    references: &[cite(&RH_2025, "RVSP p. 160; Table 4, p. 161"), T1],
    review: Review::Verified,
    example: &[("tr_vmax_m_s", N(3.1)), ("rap_mmhg", N(8.0))],
    sim_link: None,
};

/// Pulmonary end-diastolic and mean pressure from the PR jet (and from PASP and PAEDP).
pub static PA_PRESSURES_FROM_PR: Calculator = Calculator {
    id: "pa-pressures-from-pr",
    category: Category::RightHeart,
    name: tx!(
        "Pulmonary diastolic and mean pressure (PR jet)",
        "Pressione polmonare diastolica e media (getto di IP)"
    ),
    synonyms: tx!(
        "PAEDP, PADP, mPAP, mean pulmonary pressure, pulmonary regurgitation, end-diastolic velocity",
        "PAPd, PAPm, pressione polmonare media, insufficienza polmonare, velocità telediastolica"
    ),
    summary: tx!(
        "PAEDP = 4 × end-diastolic PR velocity² + RAP; mPAP = 4 × early-diastolic PR peak² + RAP, or ⅓ PASP + ⅔ PAEDP. Severe PR and restrictive RV physiology distort the PR signal.",
        "PAPd = 4 × velocità telediastolica IP² + PAD; PAPm = 4 × picco protodiastolico IP² + PAD, oppure ⅓ PAPs + ⅔ PAPd. Un'IP grave e una fisiologia restrittiva del VD alterano il segnale dell'IP."
    ),
    inputs: &[
        num(
            "pr_ed_v_m_s",
            tx!("PR end-diastolic velocity", "Velocità telediastolica dell'IP"),
            Unit::MS,
            0.3,
            4.0,
            2,
        ),
        opt(
            "pr_peak_v_m_s",
            tx!(
                "PR early-diastolic peak velocity",
                "Velocità protodiastolica di picco dell'IP"
            ),
            Unit::MS,
            0.5,
            5.0,
            2,
        ),
        RAP,
        opt(
            "pasp_mmhg",
            tx!(
                "PA systolic pressure (for ⅓ + ⅔)",
                "Pressione sistolica polmonare (per ⅓ + ⅔)"
            ),
            Unit::MmHg,
            15.0,
            150.0,
            0,
        ),
    ],
    compute: pa_pressures,
    references: &[cite(&RH_2025, "Table 4, p. 162; PAEDP and mPAP, p. 166")],
    review: Review::Verified,
    example: &[
        ("pr_ed_v_m_s", N(1.6)),
        ("pr_peak_v_m_s", N(2.4)),
        ("rap_mmhg", N(8.0)),
        ("pasp_mmhg", N(48.0)),
    ],
    sim_link: None,
};

fn pa_pressures(i: &Inputs) -> Result<Outcome, Vec<InputError>> {
    let (ved, rap) = (i.n("pr_ed_v_m_s"), i.n("rap_mmhg"));
    let paedp = bernoulli_mmhg(ved) + rap;
    let mut out = outcome();
    out.push(
        q(
            "paedp_mmhg",
            tx!("PA end-diastolic pressure", "Pressione telediastolica polmonare"),
            paedp,
            Unit::MmHg,
            0,
        ),
        "PAEDP = 4 × V(PR, end-diastole)² + RAP",
        format!("4 × {}² + {}", n(ved, 2), n(rap, 0)),
    );
    if let Some(vp) = o(i, "pr_peak_v_m_s") {
        check(
            vp < ved,
            "pr_peak_v_m_s",
            tx!(
                "The early PR peak should not be lower than the end-diastolic velocity",
                "Il picco protodiastolico dell'IP non dovrebbe essere inferiore alla velocità telediastolica"
            ),
        )?;
        out.push(
            q(
                "mpap_pr_mmhg",
                tx!(
                    "Mean PA pressure (PR peak)",
                    "Pressione polmonare media (picco IP)"
                ),
                bernoulli_mmhg(vp) + rap,
                Unit::MmHg,
                0,
            ),
            "mPAP = 4 × V(PR, early peak)² + RAP",
            format!("4 × {}² + {}", n(vp, 2), n(rap, 0)),
        );
    }
    if let Some(pasp) = o(i, "pasp_mmhg") {
        check(
            pasp <= paedp,
            "pasp_mmhg",
            tx!(
                "The systolic pressure must exceed the end-diastolic one",
                "La pressione sistolica deve superare quella telediastolica"
            ),
        )?;
        out.push(
            q(
                "mpap_formula_mmhg",
                tx!(
                    "Mean PA pressure (⅓ PASP + ⅔ PAEDP)",
                    "Pressione polmonare media (⅓ PAPs + ⅔ PAPd)"
                ),
                pasp / 3.0 + 2.0 * paedp / 3.0,
                Unit::MmHg,
                0,
            ),
            "mPAP = ⅓ × PASP + ⅔ × PAEDP",
            format!("{} ÷ 3 + 2 × {} ÷ 3", n(pasp, 0), n(paedp, 0)),
        );
    }
    let src = cite(&RH_2025, "PAEDP, p. 166");
    out.findings.push(if ge(ved, 2.2) {
        criterion_met(Severity::Moderate, tx!("End-diastolic PR velocity at or above the 2.2 m/s the guideline calls generally abnormal", "Velocità telediastolica dell'IP pari o superiore ai 2,2 m/s che la linea guida considera in genere anormali"), "V(PR, end-diastole) ≥ 2.2 m/s", src)
    } else {
        criterion_met(Severity::Normal, tx!("End-diastolic PR velocity below the 2.2 m/s the guideline calls generally abnormal", "Velocità telediastolica dell'IP sotto i 2,2 m/s che la linea guida considera in genere anormali"), "V(PR, end-diastole) < 2.2 m/s", src)
    });
    Ok(out)
}

/// Mean pulmonary pressure from the RVOT acceleration time (two regressions).
pub static MPAP_FROM_ACCT: Calculator = Calculator {
    id: "mpap-from-acceleration-time",
    category: Category::RightHeart,
    name: tx!(
        "Mean PA pressure from the RVOT acceleration time",
        "Pressione polmonare media dal tempo di accelerazione RVOT"
    ),
    synonyms: tx!(
        "AccT, AcT, PAAT, RVOT acceleration time, mPAP, Kitabatake, pulmonary hypertension",
        "AccT, tempo di accelerazione polmonare, PAPm, Kitabatake, ipertensione polmonare"
    ),
    summary: tx!(
        "The shorter the time from onset to peak of RVOT flow, the higher the pulmonary pressure: mPAP = 79 − 0.45 × AccT, and 90 − 0.62 × AccT when AccT < 120 ms. Indirect: the TR-based RVSP is preferred for reporting.",
        "Più breve è il tempo dall'inizio al picco del flusso RVOT, più alta è la pressione polmonare: PAPm = 79 − 0,45 × AccT, e 90 − 0,62 × AccT se AccT < 120 ms. Metodo indiretto: per il referto si preferisce la PSVD dall'IT."
    ),
    inputs: &[num(
        "acct_ms",
        tx!("RVOT acceleration time", "Tempo di accelerazione RVOT"),
        Unit::Ms,
        30.0,
        170.0,
        0,
    )],
    compute: |i| {
        let t = i.n("acct_ms");
        let mut out = outcome();
        out.push(
            q(
                "mpap_mmhg",
                tx!("Mean PA pressure", "Pressione polmonare media"),
                79.0 - 0.45 * t,
                Unit::MmHg,
                0,
            ),
            "mPAP = 79 − 0.45 × AccT",
            format!("79 − 0.45 × {}", n(t, 0)),
        );
        if lt(t, 120.0) {
            out.push(
                q(
                    "mpap_short_acct_mmhg",
                    tx!(
                        "Mean PA pressure (AccT < 120 ms)",
                        "Pressione polmonare media (AccT < 120 ms)"
                    ),
                    90.0 - 0.62 * t,
                    Unit::MmHg,
                    0,
                ),
                "mPAP = 90 − 0.62 × AccT",
                format!("90 − 0.62 × {}", n(t, 0)),
            );
        }
        out.findings.push(ACCT.finding(t));
        Ok(out)
    },
    references: &[cite(&RH_2025, "mPAP formulas, p. 166; AccT, p. 158"), T1],
    review: Review::Verified,
    example: &[("acct_ms", N(88.0))],
    sim_link: None,
};

/// Pulmonary vascular resistance from TR velocity and RVOT VTI (Abbas).
pub static PVR_DOPPLER: Calculator = Calculator {
    id: "pvr-doppler",
    category: Category::RightHeart,
    name: tx!(
        "Pulmonary vascular resistance (Doppler)",
        "Resistenze vascolari polmonari (Doppler)"
    ),
    synonyms: tx!(
        "PVR, Abbas, Wood units, TRV/VTI, pulmonary hypertension",
        "RVP, Abbas, unità Wood, TRV/VTI, ipertensione polmonare"
    ),
    summary: tx!(
        "Pressure rises with resistance while flow falls: PVR ≈ 10 × TR Vmax ÷ RVOT VTI + 0.16 Wood units. Unreliable when PVR is very high (above about 8 WU).",
        "La pressione sale con le resistenze mentre il flusso cala: RVP ≈ 10 × Vmax IT ÷ VTI RVOT + 0,16 unità Wood. Poco affidabile con resistenze molto alte (oltre circa 8 UW)."
    ),
    inputs: &[
        TR_VMAX,
        num("rvot_vti_cm", tx!("RVOT VTI", "VTI RVOT"), Unit::Cm, 4.0, 40.0, 1),
    ],
    compute: |i| {
        let (tr, vti) = (i.n("tr_vmax_m_s"), i.n("rvot_vti_cm"));
        let pvr = tr / vti * 10.0 + 0.16;
        let mut out = outcome();
        out.push(
            q(
                "pvr_wu",
                tx!("Pulmonary vascular resistance", "Resistenze vascolari polmonari"),
                pvr,
                Unit::WoodUnits,
                2,
            ),
            "PVR = 10 × TRV ÷ VTI(RVOT) + 0.16",
            format!("10 × {} ÷ {} + 0.16", n(tr, 2), n(vti, 1)),
        );
        let src = cite(&RH_2025, "PVR, p. 167");
        out.findings.push(if lt(pvr, 1.5) {
            criterion_met(Severity::Normal, tx!("Below the 1.5 WU the guideline calls normal", "Sotto le 1,5 UW che la linea guida considera normali"), "PVR < 1.5 WU", src)
        } else if gt(pvr, 2.0) {
            criterion_met(Severity::Moderate, tx!("Above the 2.0 WU the guideline calls abnormal and suggestive of pulmonary hypertension", "Sopra le 2,0 UW che la linea guida considera anormali e suggestive di ipertensione polmonare"), "PVR > 2.0 WU", src)
        } else {
            criterion_met(Severity::Info, tx!("Between the normal (< 1.5) and abnormal (> 2.0) limits", "Tra il limite normale (< 1,5) e quello anormale (> 2,0)"), "1.5 ≤ PVR ≤ 2.0 WU", src)
        });
        if gt(pvr, 8.0) {
            out.findings.push(criterion_met(
                Severity::Info,
                tx!(
                    "Above 8 WU, where the guideline calls the Abbas equation unreliable",
                    "Sopra 8 UW, dove la linea guida considera inaffidabile l'equazione di Abbas"
                ),
                "PVR > 8 WU",
                src,
            ));
        }
        Ok(out)
    },
    references: &[cite(&RH_2025, "PVR, p. 167; Table 4, p. 162")],
    review: Review::Verified,
    example: &[("tr_vmax_m_s", N(3.4)), ("rvot_vti_cm", N(14.0))],
    sim_link: None,
};

/// RV systolic function: TAPSE, S', FAC, 3D EF and free-wall strain, each graded if measured.
pub static RV_SYSTOLIC_FUNCTION: Calculator = Calculator {
    id: "rv-systolic-function",
    category: Category::RightHeart,
    name: tx!("RV systolic function", "Funzione sistolica del VD"),
    synonyms: tx!(
        "TAPSE, S', S prime, FAC, fractional area change, RVEF, free wall strain, RV function",
        "TAPSE, S', FAC, variazione frazionale di area, FEVD, strain di parete libera, funzione VD"
    ),
    summary: tx!(
        "Grades each RV function measurement entered against the guideline's partitions. FAC = (RVEDA − RVESA) ÷ RVEDA × 100 and the 3D EF are computed from the areas and volumes. Enter strain as a magnitude.",
        "Classifica ogni misura di funzione del VD inserita secondo le partizioni della linea guida. FAC = (AreaTD − AreaTS) ÷ AreaTD × 100 e la FE 3D sono calcolate da aree e volumi. Inserire lo strain come valore assoluto."
    ),
    inputs: &[
        opt("tapse_mm", tx!("TAPSE", "TAPSE"), Unit::Mm, 3.0, 40.0, 0),
        opt(
            "s_prime_cm_s",
            tx!("Tricuspid annular S'", "S' dell'anulus tricuspidale"),
            Unit::CmS,
            3.0,
            25.0,
            1,
        ),
        opt(
            "rv_eda_cm2",
            tx!("RV end-diastolic area", "Area telediastolica VD"),
            Unit::Cm2,
            5.0,
            60.0,
            1,
        ),
        opt(
            "rv_esa_cm2",
            tx!("RV end-systolic area", "Area telesistolica VD"),
            Unit::Cm2,
            2.0,
            50.0,
            1,
        ),
        opt(
            "rv_edv_ml",
            tx!("3D RV end-diastolic volume", "Volume telediastolico VD 3D"),
            Unit::Ml,
            40.0,
            400.0,
            0,
        ),
        opt(
            "rv_esv_ml",
            tx!("3D RV end-systolic volume", "Volume telesistolico VD 3D"),
            Unit::Ml,
            10.0,
            350.0,
            0,
        ),
        opt(
            "fwls_percent",
            tx!(
                "RV free-wall strain (magnitude)",
                "Strain parete libera VD (valore assoluto)"
            ),
            Unit::Percent,
            2.0,
            45.0,
            0,
        ),
    ],
    compute: rv_function,
    references: &[cite(&RH_2025, "FAC formula p. 157"), T1],
    review: Review::Verified,
    example: &[
        ("tapse_mm", N(15.0)),
        ("s_prime_cm_s", N(8.6)),
        ("rv_eda_cm2", N(24.0)),
        ("rv_esa_cm2", N(16.0)),
        ("fwls_percent", N(18.0)),
    ],
    sim_link: None,
};

fn rv_function(i: &Inputs) -> Result<Outcome, Vec<InputError>> {
    let mut out = outcome();
    let mut any = false;
    if let Some(t) = o(i, "tapse_mm") {
        any = true;
        out.findings.push(TAPSE.finding(t));
    }
    if let Some(s) = o(i, "s_prime_cm_s") {
        any = true;
        out.findings.push(S_PRIME.finding(s));
    }
    match (o(i, "rv_eda_cm2"), o(i, "rv_esa_cm2")) {
        (Some(ed), Some(es)) => {
            any = true;
            check(
                es >= ed,
                "rv_esa_cm2",
                tx!(
                    "The end-systolic area must be smaller than the end-diastolic one",
                    "L'area telesistolica deve essere minore di quella telediastolica"
                ),
            )?;
            let fac = fraction_percent(ed, es);
            out.push(
                q(
                    "fac_percent",
                    tx!("Fractional area change", "Variazione frazionale di area"),
                    fac,
                    Unit::Percent,
                    0,
                ),
                "FAC = (RVEDA − RVESA) ÷ RVEDA × 100",
                format!("({} − {}) ÷ {} × 100", n(ed, 1), n(es, 1), n(ed, 1)),
            );
            out.findings.push(FAC.finding(fac));
        }
        (None, None) => {}
        (None, Some(_)) => {
            return Err(vec![InputError::inconsistent(
                "rv_eda_cm2",
                tx!("Enter both areas", "Inserire entrambe le aree"),
            )]);
        }
        (Some(_), None) => {
            return Err(vec![InputError::inconsistent(
                "rv_esa_cm2",
                tx!("Enter both areas", "Inserire entrambe le aree"),
            )]);
        }
    }
    match (o(i, "rv_edv_ml"), o(i, "rv_esv_ml")) {
        (Some(ed), Some(es)) => {
            any = true;
            check(
                es >= ed,
                "rv_esv_ml",
                tx!(
                    "The end-systolic volume must be smaller than the end-diastolic one",
                    "Il volume telesistolico deve essere minore di quello telediastolico"
                ),
            )?;
            let ef = fraction_percent(ed, es);
            out.push(
                q(
                    "rvef_percent",
                    tx!("3D RV ejection fraction", "Frazione di eiezione VD 3D"),
                    ef,
                    Unit::Percent,
                    0,
                ),
                "RVEF = (EDV − ESV) ÷ EDV × 100",
                format!("({} − {}) ÷ {} × 100", n(ed, 0), n(es, 0), n(ed, 0)),
            );
            out.findings.push(RVEF.finding(ef));
        }
        (None, None) => {}
        (None, Some(_)) => {
            return Err(vec![InputError::inconsistent(
                "rv_edv_ml",
                tx!("Enter both volumes", "Inserire entrambi i volumi"),
            )]);
        }
        (Some(_), None) => {
            return Err(vec![InputError::inconsistent(
                "rv_esv_ml",
                tx!("Enter both volumes", "Inserire entrambi i volumi"),
            )]);
        }
    }
    if let Some(s) = o(i, "fwls_percent") {
        any = true;
        out.findings.push(FWS.finding(s));
    }
    if !any {
        return Err(need_one("tapse_mm"));
    }
    let graded = out.findings.len();
    out.push(
        q(
            "measures_graded",
            tx!("Measurements graded", "Misure classificate"),
            graded as f64,
            Unit::Score,
            0,
        ),
        "measurements entered",
        graded.to_string(),
    );
    Ok(out)
}

/// TAPSE ÷ PASP, an index of RV–pulmonary arterial coupling.
pub static TAPSE_PASP: Calculator = Calculator {
    id: "tapse-pasp-coupling",
    category: Category::RightHeart,
    name: tx!("TAPSE/PASP coupling", "Accoppiamento TAPSE/PAPs"),
    synonyms: tx!(
        "TAPSE/PASP, TAPSE/sPAP, RV-PA coupling, ventriculo-arterial coupling",
        "TAPSE/PAPs, accoppiamento VD-AP, accoppiamento ventricolo-arterioso"
    ),
    summary: tx!(
        "RV longitudinal shortening relative to the afterload it works against. The guideline reports 0.5–0.7 mm/mmHg in healthy people and associates 0.3–0.4 mm/mmHg with RV–PA uncoupling in pulmonary hypertension.",
        "Accorciamento longitudinale del VD rapportato al postcarico contro cui lavora. La linea guida riporta 0,5–0,7 mm/mmHg nei sani e associa 0,3–0,4 mm/mmHg al disaccoppiamento VD-AP nell'ipertensione polmonare."
    ),
    inputs: &[
        num("tapse_mm", tx!("TAPSE", "TAPSE"), Unit::Mm, 3.0, 40.0, 0),
        num(
            "pasp_mmhg",
            tx!("PA systolic pressure", "Pressione sistolica polmonare"),
            Unit::MmHg,
            10.0,
            150.0,
            0,
        ),
    ],
    compute: |i| {
        let (t, p) = (i.n("tapse_mm"), i.n("pasp_mmhg"));
        let r = t / p;
        let mut out = outcome();
        out.push(
            q(
                "tapse_pasp_mm_mmhg",
                tx!("TAPSE/PASP", "TAPSE/PAPs"),
                r,
                Unit::MmPerMmHg,
                2,
            ),
            "TAPSE ÷ PASP",
            format!("{} ÷ {}", v(t, 0, Unit::Mm), v(p, 0, Unit::MmHg)),
        );
        let src = cite(&RH_2025, "TAPSE/PASP, p. 152");
        if le(r, 0.4) {
            out.findings.push(criterion_met(Severity::Moderate, tx!("At or below the upper end of the 0.3–0.4 mm/mmHg range the guideline associates with RV–PA uncoupling in pulmonary hypertension", "Pari o inferiore all'estremo superiore dell'intervallo 0,3–0,4 mm/mmHg che la linea guida associa al disaccoppiamento VD-AP nell'ipertensione polmonare"), "TAPSE/PASP ≤ 0.4 mm/mmHg", src));
        } else if ge(r, 0.5) && le(r, 0.7) {
            out.findings.push(criterion_met(
                Severity::Normal,
                tx!(
                    "Within the 0.5–0.7 mm/mmHg the guideline reports in healthy individuals",
                    "Nell'intervallo 0,5–0,7 mm/mmHg che la linea guida riporta nei sani"
                ),
                "0.5 ≤ TAPSE/PASP ≤ 0.7 mm/mmHg",
                src,
            ));
        } else {
            out.findings.push(criterion_met(
                Severity::Info,
                tx!(
                    "Outside both ranges the guideline quotes",
                    "Fuori da entrambi gli intervalli citati dalla linea guida"
                ),
                "0.4 < TAPSE/PASP < 0.5 or > 0.7 mm/mmHg",
                src,
            ));
        }
        Ok(out)
    },
    references: &[cite(&RH_2025, "RV–PA coupling and key points, p. 152")],
    review: Review::Verified,
    example: &[("tapse_mm", N(17.0)), ("pasp_mmhg", N(52.0))],
    sim_link: None,
};

const MPI_METHOD: &[Choice] = &[
    Choice {
        id: "pw",
        label: tx!("Pulsed Doppler (TCO and ET)", "Doppler pulsato (TCO ed ET)"),
    },
    Choice {
        id: "tdi",
        label: tx!(
            "Tissue Doppler (IVCT, IVRT, ET)",
            "Doppler tissutale (TCI, TRI, ET)"
        ),
    },
];

/// RV index of myocardial performance (Tei index), pulsed or tissue Doppler.
pub static RV_MPI: Calculator = Calculator {
    id: "rv-mpi",
    category: Category::RightHeart,
    name: tx!(
        "RV myocardial performance index",
        "Indice di performance miocardica del VD"
    ),
    synonyms: tx!(
        "MPI, RIMP, Tei index, isovolumic times, ejection time, TCO",
        "MPI, RIMP, indice di Tei, tempi isovolumetrici, tempo di eiezione, TCO"
    ),
    summary: tx!(
        "The isovolumic times as a fraction of the ejection time: (TCO − ET) ÷ ET by pulsed Doppler, (IVCT + IVRT) ÷ ET by tissue Doppler. Load-dependent; measure on beats with similar RR intervals. The two methods have different normal limits.",
        "I tempi isovolumetrici come frazione del tempo di eiezione: (TCO − ET) ÷ ET con il Doppler pulsato, (TCI + TRI) ÷ ET con il Doppler tissutale. Dipende dal carico; misurare su battiti con RR simili. I due metodi hanno limiti normali diversi."
    ),
    inputs: &[
        choice("method", tx!("Method", "Metodo"), MPI_METHOD),
        when(
            num(
                "tco_ms",
                tx!(
                    "Tricuspid closure-to-opening time",
                    "Tempo chiusura-apertura tricuspidale"
                ),
                Unit::Ms,
                150.0,
                800.0,
                0,
            ),
            "method",
            &["pw"],
        ),
        when(
            num(
                "ivct_ms",
                tx!(
                    "Isovolumic contraction time",
                    "Tempo di contrazione isovolumetrica"
                ),
                Unit::Ms,
                10.0,
                200.0,
                0,
            ),
            "method",
            &["tdi"],
        ),
        when(
            num(
                "ivrt_ms",
                tx!(
                    "Isovolumic relaxation time",
                    "Tempo di rilasciamento isovolumetrico"
                ),
                Unit::Ms,
                10.0,
                250.0,
                0,
            ),
            "method",
            &["tdi"],
        ),
        num(
            "et_ms",
            tx!("RV ejection time", "Tempo di eiezione VD"),
            Unit::Ms,
            120.0,
            500.0,
            0,
        ),
    ],
    compute: |i| {
        let et = i.n("et_ms");
        let mut out = outcome();
        if i.choice("method") == Some("pw") {
            let tco = i.n("tco_ms");
            check(
                tco <= et,
                "tco_ms",
                tx!(
                    "The closure-to-opening time must exceed the ejection time",
                    "Il tempo chiusura-apertura deve superare il tempo di eiezione"
                ),
            )?;
            let mpi = (tco - et) / et;
            out.push(
                q("rv_mpi", tx!("RV MPI", "MPI del VD"), mpi, Unit::None, 2),
                "MPI = (TCO − ET) ÷ ET",
                format!("({} − {}) ÷ {}", n(tco, 0), n(et, 0), n(et, 0)),
            );
            out.findings.push(MPI_PW.finding(mpi));
        } else {
            let (c, r) = (i.n("ivct_ms"), i.n("ivrt_ms"));
            let mpi = (c + r) / et;
            out.push(
                q("rv_mpi", tx!("RV MPI", "MPI del VD"), mpi, Unit::None, 2),
                "MPI = (IVCT + IVRT) ÷ ET",
                format!("({} + {}) ÷ {}", n(c, 0), n(r, 0), n(et, 0)),
            );
            out.findings.push(MPI_TDI.finding(mpi));
        }
        Ok(out)
    },
    references: &[cite(&RH_2025, "RV MPI formulas, p. 158"), T1],
    review: Review::Verified,
    example: &[("method", C("pw")), ("tco_ms", N(410.0)), ("et_ms", N(290.0))],
    sim_link: None,
};

/// Right atrial size, each measurement graded if entered.
pub static RIGHT_ATRIAL_SIZE: Calculator = Calculator {
    id: "right-atrial-size",
    category: Category::RightHeart,
    name: tx!("Right atrial size", "Dimensioni dell'atrio destro"),
    synonyms: tx!(
        "RA, right atrium, RA area, RA volume index, RAVi, major axis, minor axis",
        "AD, atrio destro, area AD, volume AD indicizzato, asse maggiore, asse minore"
    ),
    summary: tx!(
        "Grades the RA measurements entered (apical four-chamber, end-systole) against the guideline's partitions; volumes are indexed to body surface area. Athletes and pregnant women may have larger atria without disease.",
        "Classifica le misure dell'atrio destro inserite (apicale quattro camere, telesistole) secondo le partizioni della linea guida; i volumi sono indicizzati alla superficie corporea. Atleti e gestanti possono avere atri più grandi senza malattia."
    ),
    inputs: &[
        opt(
            "ra_major_cm",
            tx!("RA major dimension", "Asse maggiore AD"),
            Unit::Cm,
            2.0,
            10.0,
            1,
        ),
        opt(
            "ra_minor_cm",
            tx!("RA minor dimension", "Asse minore AD"),
            Unit::Cm,
            2.0,
            9.0,
            1,
        ),
        opt(
            "ra_area_cm2",
            tx!("RA end-systolic area", "Area telesistolica AD"),
            Unit::Cm2,
            5.0,
            60.0,
            0,
        ),
        opt(
            "ra_vol_mod_ml",
            tx!("RA volume (disc method)", "Volume AD (metodo dei dischi)"),
            Unit::Ml,
            10.0,
            300.0,
            0,
        ),
        opt(
            "ra_vol_al_ml",
            tx!("RA volume (area–length)", "Volume AD (area-lunghezza)"),
            Unit::Ml,
            10.0,
            300.0,
            0,
        ),
        BSA_OPT,
    ],
    compute: |i| {
        let mut out = outcome();
        let bsa = o(i, "bsa_m2");
        for (id, scale) in [
            ("ra_major_cm", &RA_MAJOR),
            ("ra_minor_cm", &RA_MINOR),
            ("ra_area_cm2", &RA_AREA),
        ] {
            if let Some(x) = o(i, id) {
                out.findings.push(scale.finding(x));
            }
        }
        for (id, out_id, scale, label, formula) in [
            (
                "ra_vol_mod_ml",
                "ravi_mod_ml_m2",
                &RAVI_MOD,
                tx!("RA volume index (discs)", "Volume AD indicizzato (dischi)"),
                "RAVi = RA volume (discs) ÷ BSA",
            ),
            (
                "ra_vol_al_ml",
                "ravi_al_ml_m2",
                &RAVI_AL,
                tx!(
                    "RA volume index (area–length)",
                    "Volume AD indicizzato (area-lunghezza)"
                ),
                "RAVi = RA volume (area–length) ÷ BSA",
            ),
        ] {
            if let Some(vol) = o(i, id) {
                let Some(bsa) = bsa else {
                    return Err(vec![InputError::inconsistent(
                        "bsa_m2",
                        tx!(
                            "A volume needs the body surface area",
                            "Un volume richiede la superficie corporea"
                        ),
                    )]);
                };
                let x = vol / bsa;
                out.push(
                    q(out_id, label, x, Unit::MlM2, 0),
                    formula,
                    format!("{} ÷ {}", v(vol, 0, Unit::Ml), v(bsa, 2, Unit::M2)),
                );
                out.findings.push(scale.finding(x));
            }
        }
        if out.findings.is_empty() {
            return Err(need_one("ra_major_cm"));
        }
        let k = out.findings.len();
        out.push(
            q(
                "measures_graded",
                tx!("Measurements graded", "Misure classificate"),
                k as f64,
                Unit::Score,
                0,
            ),
            "measurements entered",
            k.to_string(),
        );
        Ok(out)
    },
    references: &[T1],
    review: Review::Verified,
    example: &[
        ("ra_major_cm", N(5.6)),
        ("ra_minor_cm", N(4.0)),
        ("ra_area_cm2", N(20.0)),
        ("ra_vol_mod_ml", N(58.0)),
        ("bsa_m2", N(1.8)),
    ],
    sim_link: None,
};

/// Right ventricular size, each measurement graded if entered (indices when BSA is given).
pub static RIGHT_VENTRICULAR_SIZE: Calculator = Calculator {
    id: "right-ventricular-size",
    category: Category::RightHeart,
    name: tx!("Right ventricular size", "Dimensioni del ventricolo destro"),
    synonyms: tx!(
        "RV, RVD1, RVD2, RVD3, basal diameter, RVOT, RV wall thickness, RV area, RV volume, dilatation",
        "VD, diametro basale, RVOT, spessore parietale VD, area VD, volume VD, dilatazione"
    ),
    summary: tx!(
        "Grades the RV dimensions, areas and 3D volumes entered (RV-focused apical view, end-diastole unless stated) against the guideline's partitions. With body surface area, the basal diameter, areas and volumes are also indexed and graded.",
        "Classifica diametri, aree e volumi 3D del VD inseriti (apicale focalizzata sul VD, telediastole salvo diversa indicazione) secondo le partizioni della linea guida. Con la superficie corporea, diametro basale, aree e volumi sono anche indicizzati e classificati."
    ),
    inputs: &[
        opt(
            "rvd1_cm",
            tx!("Basal diameter (RVD1)", "Diametro basale (RVD1)"),
            Unit::Cm,
            1.5,
            8.0,
            1,
        ),
        opt(
            "rvd2_cm",
            tx!("Mid diameter (RVD2)", "Diametro medio (RVD2)"),
            Unit::Cm,
            1.0,
            7.0,
            1,
        ),
        opt(
            "rvd3_cm",
            tx!("Longitudinal dimension (RVD3)", "Dimensione longitudinale (RVD3)"),
            Unit::Cm,
            4.0,
            13.0,
            1,
        ),
        opt(
            "rvot_plax_cm",
            tx!("RVOT diameter (PLAX)", "Diametro RVOT (PLAX)"),
            Unit::Cm,
            1.5,
            6.0,
            1,
        ),
        opt(
            "rvot_prox_cm",
            tx!("Proximal RVOT (PSAX)", "RVOT prossimale (PSAX)"),
            Unit::Cm,
            1.5,
            6.0,
            1,
        ),
        opt(
            "rvot_dist_cm",
            tx!("Distal RVOT (PSAX)", "RVOT distale (PSAX)"),
            Unit::Cm,
            1.0,
            5.0,
            1,
        ),
        opt(
            "rvwt_cm",
            tx!("RV wall thickness", "Spessore parietale VD"),
            Unit::Cm,
            0.1,
            1.5,
            1,
        ),
        opt(
            "rv_eda_cm2",
            tx!("RV end-diastolic area", "Area telediastolica VD"),
            Unit::Cm2,
            5.0,
            60.0,
            0,
        ),
        opt(
            "rv_esa_cm2",
            tx!("RV end-systolic area", "Area telesistolica VD"),
            Unit::Cm2,
            2.0,
            50.0,
            0,
        ),
        opt(
            "rv_edv_ml",
            tx!("3D RV end-diastolic volume", "Volume telediastolico VD 3D"),
            Unit::Ml,
            40.0,
            400.0,
            0,
        ),
        opt(
            "rv_esv_ml",
            tx!("3D RV end-systolic volume", "Volume telesistolico VD 3D"),
            Unit::Ml,
            10.0,
            350.0,
            0,
        ),
        BSA_OPT,
    ],
    compute: rv_size,
    references: &[T1],
    review: Review::Verified,
    example: &[
        ("rvd1_cm", N(4.3)),
        ("rvd2_cm", N(3.4)),
        ("rvd3_cm", N(8.0)),
        ("rv_eda_cm2", N(27.0)),
        ("rv_esa_cm2", N(15.0)),
        ("bsa_m2", N(1.9)),
    ],
    sim_link: None,
};

/// The indexed form of a size: output id, its scale, label and unit.
type Indexed = (&'static str, &'static Scale, Text, Unit);

fn rv_size(i: &Inputs) -> Result<Outcome, Vec<InputError>> {
    let mut out = outcome();
    let bsa = o(i, "bsa_m2");
    let direct: [(&str, &Scale, Option<Indexed>); 11] = [
        (
            "rvd1_cm",
            &RV_BASAL,
            Some((
                "rvd1_index_cm_m2",
                &RV_BASAL_I,
                tx!("Indexed basal diameter", "Diametro basale indicizzato"),
                Unit::CmM2,
            )),
        ),
        ("rvd2_cm", &RV_MID, None),
        ("rvd3_cm", &RV_LONG, None),
        ("rvot_plax_cm", &RVOT_PLAX, None),
        ("rvot_prox_cm", &RVOT_PROX, None),
        ("rvot_dist_cm", &RVOT_DIST, None),
        ("rvwt_cm", &RVWT, None),
        (
            "rv_eda_cm2",
            &RV_EDA,
            Some((
                "rv_eda_index_cm2_m2",
                &RV_EDA_I,
                tx!("Indexed end-diastolic area", "Area telediastolica indicizzata"),
                Unit::Cm2M2,
            )),
        ),
        (
            "rv_esa_cm2",
            &RV_ESA,
            Some((
                "rv_esa_index_cm2_m2",
                &RV_ESA_I,
                tx!("Indexed end-systolic area", "Area telesistolica indicizzata"),
                Unit::Cm2M2,
            )),
        ),
        (
            "rv_edv_ml",
            &RV_EDV,
            Some((
                "rv_edv_index_ml_m2",
                &RV_EDV_I,
                tx!(
                    "Indexed 3D end-diastolic volume",
                    "Volume telediastolico 3D indicizzato"
                ),
                Unit::MlM2,
            )),
        ),
        (
            "rv_esv_ml",
            &RV_ESV,
            Some((
                "rv_esv_index_ml_m2",
                &RV_ESV_I,
                tx!(
                    "Indexed 3D end-systolic volume",
                    "Volume telesistolico 3D indicizzato"
                ),
                Unit::MlM2,
            )),
        ),
    ];
    if let (Some(ed), Some(es)) = (o(i, "rv_eda_cm2"), o(i, "rv_esa_cm2")) {
        check(
            es >= ed,
            "rv_esa_cm2",
            tx!(
                "The end-systolic area must be smaller than the end-diastolic one",
                "L'area telesistolica deve essere minore di quella telediastolica"
            ),
        )?;
    }
    if let (Some(ed), Some(es)) = (o(i, "rv_edv_ml"), o(i, "rv_esv_ml")) {
        check(
            es >= ed,
            "rv_esv_ml",
            tx!(
                "The end-systolic volume must be smaller than the end-diastolic one",
                "Il volume telesistolico deve essere minore di quello telediastolico"
            ),
        )?;
    }
    for (id, scale, index) in direct {
        let Some(x) = o(i, id) else { continue };
        out.findings.push(scale.finding(x));
        if let (Some((out_id, iscale, label, unit)), Some(bsa)) = (index, bsa) {
            let xi = x / bsa;
            out.push(
                q(out_id, label, xi, unit, 1),
                &format!("{} ÷ BSA", scale.symbol),
                format!("{} ÷ {}", n(x, 1), v(bsa, 2, Unit::M2)),
            );
            out.findings.push(iscale.finding(xi));
        }
    }
    if out.findings.is_empty() {
        return Err(need_one("rvd1_cm"));
    }
    let k = out.findings.len();
    out.push(
        q(
            "measures_graded",
            tx!("Measurements graded", "Misure classificate"),
            k as f64,
            Unit::Score,
            0,
        ),
        "measurements graded",
        k.to_string(),
    );
    Ok(out)
}

const BSA_FORMULAS: &[Choice] = &[
    Choice {
        id: "mosteller",
        label: tx!("Mosteller", "Mosteller"),
    },
    Choice {
        id: "du_bois",
        label: tx!("Du Bois", "Du Bois"),
    },
    Choice {
        id: "haycock",
        label: tx!("Haycock", "Haycock"),
    },
];

/// Body surface area from height and weight.
pub static BODY_SURFACE_AREA: Calculator = Calculator {
    id: "body-surface-area",
    category: Category::RightHeart,
    name: tx!("Body surface area", "Superficie corporea"),
    synonyms: tx!(
        "BSA, Mosteller, Du Bois, DuBois, Haycock, height, weight, indexing",
        "BSA, SC, Mosteller, Du Bois, Haycock, altezza, peso, indicizzazione"
    ),
    summary: tx!(
        "Estimated skin area from height and weight, used to index chamber sizes and flows. Mosteller: √(H × W ÷ 3600); Du Bois: 0.007184 × W^0.425 × H^0.725; Haycock: 0.024265 × W^0.5378 × H^0.3964. They differ most at extremes of body size.",
        "Superficie cutanea stimata da altezza e peso, usata per indicizzare dimensioni e flussi. Mosteller: √(H × P ÷ 3600); Du Bois: 0,007184 × P^0,425 × H^0,725; Haycock: 0,024265 × P^0,5378 × H^0,3964. Le differenze sono maggiori agli estremi della corporatura."
    ),
    inputs: &[
        num("height_cm", tx!("Height", "Altezza"), Unit::Cm, 100.0, 230.0, 0),
        num("weight_kg", tx!("Weight", "Peso"), Unit::Kg, 25.0, 250.0, 1),
        choice("formula", tx!("Formula", "Formula"), BSA_FORMULAS),
    ],
    compute: |i| {
        let (h, w) = (i.n("height_cm"), i.n("weight_kg"));
        let mut out = outcome();
        let (bsa, formula, sub) = match i.choice("formula") {
            Some("du_bois") => (
                bsa_du_bois_m2(h, w),
                "BSA = 0.007184 × W^0.425 × H^0.725",
                format!("0.007184 × {}^0.425 × {}^0.725", n(w, 1), n(h, 0)),
            ),
            Some("haycock") => (
                bsa_haycock_m2(h, w),
                "BSA = 0.024265 × W^0.5378 × H^0.3964",
                format!("0.024265 × {}^0.5378 × {}^0.3964", n(w, 1), n(h, 0)),
            ),
            _ => (
                bsa_mosteller_m2(h, w),
                "BSA = √(H × W ÷ 3600)",
                format!("√({} × {} ÷ 3600)", n(h, 0), n(w, 1)),
            ),
        };
        out.push(
            q(
                "bsa_m2",
                tx!("Body surface area", "Superficie corporea"),
                bsa,
                Unit::M2,
                2,
            ),
            formula,
            sub,
        );
        Ok(out)
    },
    references: &[
        cite(&MOSTELLER_1987, "letter, p. 1098"),
        cite(&DU_BOIS_1916, "exponents 1/2.35 and 1/1.38, p. 77"),
        cite(&HAYCOCK_1978, "abstract, p. 62"),
    ],
    review: Review::FormulaOnly,
    example: &[
        ("height_cm", N(172.0)),
        ("weight_kg", N(74.0)),
        ("formula", C("mosteller")),
    ],
    sim_link: None,
};

#[cfg(test)]
mod tests {
    use super::super::testing::{close, inconsistent, ok, severities};
    use super::*;

    #[test]
    fn rap_categories_and_cut_offs() {
        let run = |max: f64, min: f64, sec: &str, marked: &str| {
            ok(
                &RAP_FROM_IVC,
                &Inputs::new()
                    .with("ivc_max_cm", max)
                    .with("ivc_sniff_cm", min)
                    .with_choice("secondary", sec)
                    .with_choice("marked", marked),
            )
            .value("rap_mmhg")
            .unwrap()
        };
        // 2.1 cm counts as normal size; 50 % collapse counts as collapsing
        assert!((run(2.1, 1.05, "unknown", "no") - 3.0).abs() < 1e-12);
        assert!((run(2.1, 1.06, "unknown", "no") - 8.0).abs() < 1e-12);
        assert!((run(2.2, 1.0, "unknown", "no") - 8.0).abs() < 1e-12);
        assert!((run(2.2, 1.2, "unknown", "no") - 15.0).abs() < 1e-12);
        assert!((run(2.6, 2.0, "unknown", "yes") - 20.0).abs() < 1e-12);
        assert!((run(2.5, 2.0, "unknown", "yes") - 15.0).abs() < 1e-12);
        assert!((run(2.0, 1.5, "absent", "no") - 3.0).abs() < 1e-12);
        assert!((run(2.0, 1.5, "present", "no") - 15.0).abs() < 1e-12);
        inconsistent(
            &RAP_FROM_IVC,
            &Inputs::new()
                .with("ivc_max_cm", 1.5)
                .with("ivc_sniff_cm", 2.0)
                .with_choice("secondary", "unknown")
                .with_choice("marked", "no"),
            "ivc_sniff_cm",
        );
    }

    #[test]
    fn rvsp_hand_case_and_bands() {
        // 4 × 3² + 8 = 44 mmHg (mild); PASP 44 − 10 = 34; TRV 3.0 mild
        let o = ok(
            &RVSP_PASP,
            &Inputs::new()
                .with("tr_vmax_m_s", 3.0)
                .with("rap_mmhg", 8.0)
                .with("ps_gradient_mmhg", 10.0),
        );
        close(o.value("rvsp_mmhg"), 44.0, 1e-9);
        close(o.value("pasp_mmhg"), 34.0, 1e-9);
        assert_eq!(severities(&o), [Severity::Mild, Severity::Mild]);
        assert_eq!(RVSP.class(34.0).0, Severity::Normal);
        assert_eq!(RVSP.class(35.0).0, Severity::Mild);
        assert_eq!(RVSP.class(49.0).0, Severity::Mild);
        assert_eq!(RVSP.class(50.0).0, Severity::Moderate);
        assert_eq!(RVSP.class(70.0).0, Severity::Severe);
        assert_eq!(TRV.class(2.79).0, Severity::Normal);
        assert_eq!(TRV.class(2.8).0, Severity::Mild);
        assert_eq!(TRV.class(3.1).0, Severity::Mild);
        assert_eq!(TRV.class(3.2).0, Severity::Moderate);
        assert_eq!(TRV.class(3.6).0, Severity::Severe);
        inconsistent(
            &RVSP_PASP,
            &Inputs::new()
                .with("tr_vmax_m_s", 2.0)
                .with("rap_mmhg", 3.0)
                .with("ps_gradient_mmhg", 30.0),
            "ps_gradient_mmhg",
        );
    }

    #[test]
    fn pa_pressures_hand_case() {
        // PAEDP 4 × 2² + 5 = 21; mPAP(PR) 4 × 3² + 5 = 41; ⅓ × 60 + ⅔ × 21 = 34
        let o = ok(
            &PA_PRESSURES_FROM_PR,
            &Inputs::new()
                .with("pr_ed_v_m_s", 2.0)
                .with("pr_peak_v_m_s", 3.0)
                .with("rap_mmhg", 5.0)
                .with("pasp_mmhg", 60.0),
        );
        close(o.value("paedp_mmhg"), 21.0, 1e-9);
        close(o.value("mpap_pr_mmhg"), 41.0, 1e-9);
        close(o.value("mpap_formula_mmhg"), 34.0, 1e-9);
        assert_eq!(severities(&o), [Severity::Normal]);
        let o = ok(
            &PA_PRESSURES_FROM_PR,
            &Inputs::new().with("pr_ed_v_m_s", 2.2).with("rap_mmhg", 5.0),
        );
        assert_eq!(severities(&o), [Severity::Moderate]);
    }

    #[test]
    fn acct_and_pvr() {
        // 79 − 0.45 × 100 = 34; 90 − 0.62 × 100 = 28; AccT 100 mild (80–105)
        let o = ok(&MPAP_FROM_ACCT, &Inputs::new().with("acct_ms", 100.0));
        close(o.value("mpap_mmhg"), 34.0, 1e-9);
        close(o.value("mpap_short_acct_mmhg"), 28.0, 1e-9);
        assert_eq!(severities(&o), [Severity::Mild]);
        let o = ok(&MPAP_FROM_ACCT, &Inputs::new().with("acct_ms", 120.0));
        assert!(o.value("mpap_short_acct_mmhg").is_none());
        assert_eq!(ACCT.class(105.0).0, Severity::Mild);
        assert_eq!(ACCT.class(106.0).0, Severity::Normal);
        assert_eq!(ACCT.class(80.0).0, Severity::Mild);
        assert_eq!(ACCT.class(79.0).0, Severity::Moderate);
        assert_eq!(ACCT.class(60.0).0, Severity::Moderate);
        assert_eq!(ACCT.class(59.0).0, Severity::Severe);
        // 10 × 3.0 ÷ 15 + 0.16 = 2.16 WU (abnormal)
        let o = ok(
            &PVR_DOPPLER,
            &Inputs::new().with("tr_vmax_m_s", 3.0).with("rvot_vti_cm", 15.0),
        );
        close(o.value("pvr_wu"), 2.16, 1e-9);
        assert_eq!(severities(&o), [Severity::Moderate]);
        // 10 × 2.0 ÷ 20 + 0.16 = 1.16 normal; 1.5 exactly is between
        assert_eq!(
            severities(&ok(
                &PVR_DOPPLER,
                &Inputs::new().with("tr_vmax_m_s", 2.0).with("rvot_vti_cm", 20.0)
            )),
            [Severity::Normal]
        );
        assert_eq!(
            severities(&ok(
                &PVR_DOPPLER,
                &Inputs::new().with("tr_vmax_m_s", 1.34).with("rvot_vti_cm", 10.0)
            )),
            [Severity::Info]
        );
    }

    #[test]
    fn rv_function_bands_and_fac() {
        // FAC (20 − 14) ÷ 20 = 30 % → mild (≤ 35 to > 29)
        let o = ok(
            &RV_SYSTOLIC_FUNCTION,
            &Inputs::new()
                .with("rv_eda_cm2", 20.0)
                .with("rv_esa_cm2", 14.0)
                .with("tapse_mm", 17.0),
        );
        close(o.value("fac_percent"), 30.0, 1e-9);
        assert_eq!(severities(&o), [Severity::Mild, Severity::Mild]);
        assert_eq!(TAPSE.class(17.0).0, Severity::Mild);
        assert_eq!(TAPSE.class(18.0).0, Severity::Normal);
        assert_eq!(TAPSE.class(13.0).0, Severity::Moderate);
        assert_eq!(TAPSE.class(10.0).0, Severity::Severe);
        assert_eq!(FAC.class(35.0).0, Severity::Mild);
        assert_eq!(FAC.class(36.0).0, Severity::Normal);
        assert_eq!(FAC.class(22.0).0, Severity::Severe);
        assert_eq!(RVEF.class(32.0).0, Severity::Moderate);
        assert_eq!(RVEF.class(31.9).0, Severity::Severe);
        assert_eq!(FWS.class(15.0).0, Severity::Mild);
        assert_eq!(FWS.class(14.9).0, Severity::Moderate);
        assert_eq!(FWS.class(20.0).0, Severity::Mild);
        assert_eq!(FWS.class(20.1).0, Severity::Normal);
        inconsistent(
            &RV_SYSTOLIC_FUNCTION,
            &Inputs::new().with("rv_eda_cm2", 20.0),
            "rv_esa_cm2",
        );
        assert!(RV_SYSTOLIC_FUNCTION.evaluate(&Inputs::new()).is_err());
    }

    #[test]
    fn coupling_mpi_and_sizes() {
        // 20 ÷ 50 = 0.40 → at the uncoupling limit
        let o = ok(
            &TAPSE_PASP,
            &Inputs::new().with("tapse_mm", 20.0).with("pasp_mmhg", 50.0),
        );
        close(o.value("tapse_pasp_mm_mmhg"), 0.4, 1e-12);
        assert_eq!(severities(&o), [Severity::Moderate]);
        assert_eq!(
            severities(&ok(
                &TAPSE_PASP,
                &Inputs::new().with("tapse_mm", 21.0).with("pasp_mmhg", 35.0)
            )),
            [Severity::Normal]
        );
        // (400 − 280) ÷ 280 = 0.43 (PW: mild); TDI (60 + 80) ÷ 280 = 0.50 (normal)
        let o = ok(
            &RV_MPI,
            &Inputs::new()
                .with_choice("method", "pw")
                .with("tco_ms", 400.0)
                .with("et_ms", 280.0),
        );
        close(o.value("rv_mpi"), 0.4286, 1e-3);
        assert_eq!(severities(&o), [Severity::Mild]);
        let o = ok(
            &RV_MPI,
            &Inputs::new()
                .with_choice("method", "tdi")
                .with("ivct_ms", 60.0)
                .with("ivrt_ms", 80.0)
                .with("et_ms", 280.0),
        );
        close(o.value("rv_mpi"), 0.5, 1e-12);
        assert_eq!(severities(&o), [Severity::Normal]);
        assert_eq!(MPI_PW.class(0.40).0, Severity::Mild);
        assert_eq!(MPI_PW.class(0.57).0, Severity::Severe);
        assert_eq!(MPI_TDI.class(0.55).0, Severity::Mild);
        assert_eq!(MPI_TDI.class(0.70).0, Severity::Severe);
        // RA major 5.4 mild, 5.8 mild, 5.81 moderate; RAVi 60 ÷ 2 = 30 → mild
        assert_eq!(RA_MAJOR.class(5.39).0, Severity::Normal);
        assert_eq!(RA_MAJOR.class(5.4).0, Severity::Mild);
        assert_eq!(RA_MAJOR.class(5.8).0, Severity::Mild);
        assert_eq!(RA_MAJOR.class(5.81).0, Severity::Moderate);
        assert_eq!(RA_MAJOR.class(6.31).0, Severity::Severe);
        let o = ok(
            &RIGHT_ATRIAL_SIZE,
            &Inputs::new().with("ra_vol_mod_ml", 60.0).with("bsa_m2", 2.0),
        );
        close(o.value("ravi_mod_ml_m2"), 30.0, 1e-12);
        assert_eq!(severities(&o), [Severity::Mild]);
        inconsistent(
            &RIGHT_ATRIAL_SIZE,
            &Inputs::new().with("ra_vol_mod_ml", 60.0),
            "bsa_m2",
        );
        // RVD1 4.4 ÷ 2.0 = 2.2 (index normal), 4.4 mild
        let o = ok(
            &RIGHT_VENTRICULAR_SIZE,
            &Inputs::new().with("rvd1_cm", 4.4).with("bsa_m2", 2.0),
        );
        close(o.value("rvd1_index_cm_m2"), 2.2, 1e-12);
        assert_eq!(severities(&o), [Severity::Mild, Severity::Normal]);
    }

    #[test]
    fn bsa_formulas() {
        let run = |f: &str| {
            ok(
                &BODY_SURFACE_AREA,
                &Inputs::new()
                    .with("height_cm", 180.0)
                    .with("weight_kg", 80.0)
                    .with_choice("formula", f),
            )
            .value("bsa_m2")
            .unwrap()
        };
        // √(180 × 80 ÷ 3600) = 2.000
        assert!((run("mosteller") - 2.0).abs() < 1e-12);
        // 0.007184 × 80^0.425 × 180^0.725 = 0.007184 × 6.4389 × 43.1591 = 1.9964
        assert!((run("du_bois") - 1.9964).abs() < 1e-4, "{}", run("du_bois"));
        // 0.024265 × 80^0.5378 × 180^0.3964 = 0.024265 × 10.5556 × 7.8341 = 2.0066
        assert!((run("haycock") - 2.0066).abs() < 1e-4, "{}", run("haycock"));
    }
}
