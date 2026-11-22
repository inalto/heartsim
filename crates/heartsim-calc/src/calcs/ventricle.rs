//! Left ventricle and aorta: fractional shortening, indexed diameters, linear LV mass with relative
//! wall thickness, volumes and ejection fraction, relative change of GLS, LVOT stroke volume and
//! cardiac output, indexed aortic diameters, wall-motion score index.

use super::{check, choice, cite, n, num, o, opt, opt_choice, outcome, q, v, when, words};
use crate::blocks::{circle_area_cm2, fraction_percent, stroke_volume_ml};
use crate::grade::{Band, Scale, band, criterion_met, ge, gt, lt};
use crate::model::ExampleValue::{Choice as C, Number as N};
use crate::model::{
    Calculator, Category, Choice, Cite, Finding, InputError, InputSpec, Inputs, Outcome, Review, Severity,
    SimLink, Text, Unit,
};
use crate::refs::{AS_2017, CQ_2015, RH_2025, STRAIN_2025, VR_2017};
use crate::tx;

const BSA: InputSpec = num(
    "bsa_m2",
    tx!("Body surface area", "Superficie corporea"),
    Unit::M2,
    0.5,
    3.0,
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
const LVIDD: InputSpec = num(
    "lvidd_cm",
    tx!(
        "LV internal diameter, end-diastole",
        "Diametro interno VS, telediastole"
    ),
    Unit::Cm,
    2.0,
    9.0,
    2,
);

// ---------------------------------------------------------------------------------------------
// Sex-specific normal ranges and abnormality partitions (Lang 2015, Supplemental Table 3 and
// Table 4). Each row is copied as printed; a value between two printed ranges goes to the more
// abnormal one, and a value under the normal range is reported as such (the table does not grade it).

const SEXES: &[Choice] = &[
    Choice {
        id: "male",
        label: tx!("male", "maschio"),
    },
    Choice {
        id: "female",
        label: tx!("female", "femmina"),
    },
];
const SEX: InputSpec = opt_choice(
    "sex",
    tx!(
        "Sex (for the reference ranges)",
        "Sesso (per gli intervalli di riferimento)"
    ),
    SEXES,
);
const BELOW: Text = tx!("below the normal range", "sotto l'intervallo normale");
const ABOVE: Text = tx!("above the normal range", "sopra l'intervallo normale");
const SUPP3: Cite = cite(&CQ_2015, "Supplemental Table 3, p. 39.e8");
const TABLE4: Cite = cite(&CQ_2015, "Table 4, p. 10; Supplemental Table 3, p. 39.e8");

/// A size row: under the normal range, normal, mildly, moderately, severely abnormal.
const fn size_row(low: f64, normal: f64, mild: f64, moderate: f64) -> [Band; 4] {
    [
        band(low, false, Severity::Info, BELOW),
        band(normal, true, Severity::Normal, words::NORMAL),
        band(mild, true, Severity::Mild, words::MILD_ABN),
        band(moderate, true, Severity::Moderate, words::MOD_ABN),
    ]
}

/// One parameter for men and women.
struct Row {
    what: Text,
    symbol: &'static str,
    unit: Unit,
    decimals: u8,
    male: [Band; 4],
    female: [Band; 4],
    above: (Severity, Text),
    source: Cite,
}

impl Row {
    fn scale(&'static self, male: bool) -> Scale {
        Scale {
            what: self.what,
            symbol: self.symbol,
            unit: self.unit,
            decimals: self.decimals,
            bands: if male { &self.male } else { &self.female },
            above: self.above,
            source: self.source,
        }
    }
}

const SEV: (Severity, Text) = (Severity::Severe, words::SEV_ABN);

static LVIDD_ROW: Row = Row {
    what: tx!("LV diastolic diameter", "Diametro diastolico del VS"),
    symbol: "LVIDd",
    unit: Unit::Cm,
    decimals: 1,
    male: size_row(4.2, 5.8, 6.3, 6.8),
    female: size_row(3.8, 5.2, 5.6, 6.1),
    above: SEV,
    source: SUPP3,
};
static LVIDD_I_ROW: Row = Row {
    what: tx!(
        "Indexed LV diastolic diameter",
        "Diametro diastolico del VS indicizzato"
    ),
    symbol: "LVIDd/BSA",
    unit: Unit::CmM2,
    decimals: 1,
    male: size_row(2.2, 3.0, 3.3, 3.6),
    female: size_row(2.3, 3.1, 3.4, 3.7),
    above: SEV,
    source: SUPP3,
};
static LVIDS_ROW: Row = Row {
    what: tx!("LV systolic diameter", "Diametro sistolico del VS"),
    symbol: "LVIDs",
    unit: Unit::Cm,
    decimals: 1,
    male: size_row(2.5, 4.0, 4.3, 4.5),
    female: size_row(2.2, 3.5, 3.8, 4.1),
    above: SEV,
    source: SUPP3,
};
static LVIDS_I_ROW: Row = Row {
    what: tx!(
        "Indexed LV systolic diameter",
        "Diametro sistolico del VS indicizzato"
    ),
    symbol: "LVIDs/BSA",
    unit: Unit::CmM2,
    decimals: 1,
    male: size_row(1.3, 2.1, 2.3, 2.5),
    female: size_row(1.3, 2.1, 2.3, 2.6),
    above: SEV,
    source: SUPP3,
};
static EDV_ROW: Row = Row {
    what: tx!("LV diastolic volume", "Volume diastolico del VS"),
    symbol: "EDV",
    unit: Unit::Ml,
    decimals: 0,
    male: size_row(62.0, 150.0, 174.0, 200.0),
    female: size_row(46.0, 106.0, 120.0, 130.0),
    above: SEV,
    source: SUPP3,
};
static EDVI_ROW: Row = Row {
    what: tx!(
        "Indexed LV diastolic volume",
        "Volume diastolico del VS indicizzato"
    ),
    symbol: "EDV/BSA",
    unit: Unit::MlM2,
    decimals: 0,
    male: size_row(34.0, 74.0, 89.0, 100.0),
    female: size_row(29.0, 61.0, 70.0, 80.0),
    above: SEV,
    source: SUPP3,
};
static ESV_ROW: Row = Row {
    what: tx!("LV systolic volume", "Volume sistolico del VS"),
    symbol: "ESV",
    unit: Unit::Ml,
    decimals: 0,
    male: size_row(21.0, 61.0, 73.0, 85.0),
    female: size_row(14.0, 42.0, 55.0, 67.0),
    above: SEV,
    source: SUPP3,
};
static ESVI_ROW: Row = Row {
    what: tx!(
        "Indexed LV systolic volume",
        "Volume sistolico del VS indicizzato"
    ),
    symbol: "ESV/BSA",
    unit: Unit::MlM2,
    decimals: 0,
    male: size_row(11.0, 31.0, 38.0, 45.0),
    female: size_row(8.0, 24.0, 32.0, 40.0),
    above: SEV,
    source: SUPP3,
};
static WALL_ROW: Row = Row {
    what: tx!("Septal wall thickness", "Spessore del setto"),
    symbol: "IVSd",
    unit: Unit::Cm,
    decimals: 1,
    male: size_row(0.6, 1.0, 1.3, 1.6),
    female: size_row(0.6, 0.9, 1.2, 1.5),
    above: SEV,
    source: SUPP3,
};
static PW_ROW: Row = Row {
    what: tx!("Posterior wall thickness", "Spessore della parete posteriore"),
    symbol: "PWd",
    unit: Unit::Cm,
    decimals: 1,
    male: size_row(0.6, 1.0, 1.3, 1.6),
    female: size_row(0.6, 0.9, 1.2, 1.5),
    above: SEV,
    source: SUPP3,
};
static MASS_ROW: Row = Row {
    what: tx!("LV mass (linear method)", "Massa del VS (metodo lineare)"),
    symbol: "LVM",
    unit: Unit::G,
    decimals: 0,
    male: size_row(88.0, 224.0, 258.0, 292.0),
    female: size_row(67.0, 162.0, 186.0, 210.0),
    above: SEV,
    source: SUPP3,
};
static MASS_I_ROW: Row = Row {
    what: tx!(
        "LV mass index (linear method)",
        "Indice di massa del VS (metodo lineare)"
    ),
    symbol: "LVM/BSA",
    unit: Unit::GM2,
    decimals: 0,
    male: size_row(49.0, 115.0, 131.0, 148.0),
    female: size_row(43.0, 95.0, 108.0, 121.0),
    above: SEV,
    source: SUPP3,
};
/// EF falls as it worsens: severely (< 30), moderately (30-40), mildly (41-51 men, 41-53 women),
/// normal (52-72 men, 54-74 women), then above the normal range.
static EF_ROW: Row = Row {
    what: tx!("Ejection fraction", "Frazione di eiezione"),
    symbol: "EF",
    unit: Unit::Percent,
    decimals: 0,
    male: [
        band(30.0, false, Severity::Severe, words::SEV_ABN),
        band(41.0, false, Severity::Moderate, words::MOD_ABN),
        band(52.0, false, Severity::Mild, words::MILD_ABN),
        band(72.0, true, Severity::Normal, words::NORMAL),
    ],
    female: [
        band(30.0, false, Severity::Severe, words::SEV_ABN),
        band(41.0, false, Severity::Moderate, words::MOD_ABN),
        band(54.0, false, Severity::Mild, words::MILD_ABN),
        band(74.0, true, Severity::Normal, words::NORMAL),
    ],
    above: (Severity::Info, ABOVE),
    source: TABLE4,
};

/// The sex chosen: `Some(true)` for male.
fn male(i: &Inputs) -> Option<bool> {
    i.choice("sex").map(|s| s == "male")
}

/// The finding asking for the sex, for a grading that depends on it.
fn needs_sex(source: Cite) -> Finding {
    criterion_met(
        Severity::Info,
        tx!(
            "The source's ranges differ for men and women: choose the sex to see the grading",
            "Gli intervalli della fonte differiscono per uomini e donne: scegliere il sesso per vedere la classificazione"
        ),
        "sex-specific ranges",
        source,
    )
}

/// The findings of `rows` for `values`, or the request for the sex.
fn graded(i: &Inputs, out: &mut Outcome, rows: &[(&'static Row, Option<f64>)]) {
    match male(i) {
        None => out.findings.push(needs_sex(rows[0].0.source)),
        Some(m) => {
            for (row, value) in rows {
                if let Some(x) = value {
                    out.findings.push(row.scale(m).finding(*x));
                }
            }
        }
    }
}

/// Fractional shortening of the LV minor axis.
pub static FRACTIONAL_SHORTENING: Calculator = Calculator {
    id: "lv-fractional-shortening",
    category: Category::VentricularFunction,
    name: tx!("Fractional shortening", "Frazione di accorciamento"),
    synonyms: tx!(
        "FS, LVIDd, LVIDs, M-mode, linear",
        "FS, FA, DTD, DTS, M-mode, lineare"
    ),
    summary: tx!(
        "Percentage shortening of the LV internal diameter from end-diastole to end-systole. A linear index: misleading with regional wall-motion abnormalities.",
        "Accorciamento percentuale del diametro interno del VS da telediastole a telesistole. Indice lineare: fuorviante con alterazioni regionali della cinesi."
    ),
    inputs: &[
        LVIDD,
        num(
            "lvids_cm",
            tx!(
                "LV internal diameter, end-systole",
                "Diametro interno VS, telesistole"
            ),
            Unit::Cm,
            1.0,
            8.0,
            2,
        ),
    ],
    compute: |i| {
        let (d, s) = (i.n("lvidd_cm"), i.n("lvids_cm"));
        check(
            s >= d,
            "lvids_cm",
            tx!(
                "The end-systolic diameter must be smaller than the end-diastolic one",
                "Il diametro telesistolico deve essere minore di quello telediastolico"
            ),
        )?;
        let mut out = outcome();
        out.push(
            q(
                "fs_percent",
                tx!("Fractional shortening", "Frazione di accorciamento"),
                fraction_percent(d, s),
                Unit::Percent,
                0,
            ),
            "FS = (LVIDd − LVIDs) ÷ LVIDd × 100",
            format!("({} − {}) ÷ {} × 100", n(d, 2), n(s, 2), n(d, 2)),
        );
        Ok(out)
    },
    references: &[cite(&CQ_2015, "§2.1 Fractional shortening, p. 6")],
    review: Review::FormulaOnly,
    example: &[("lvidd_cm", N(5.0)), ("lvids_cm", N(3.3))],
    sim_link: None,
};

/// LV diameters indexed to body surface area.
pub static LV_DIAMETER_INDEX: Calculator = Calculator {
    id: "lv-diameter-index",
    category: Category::VentricularFunction,
    name: tx!("Indexed LV diameters", "Diametri del VS indicizzati"),
    synonyms: tx!(
        "LVIDd/BSA, LVIDs/BSA, LV size, dilatation",
        "DTD/BSA, DTS/BSA, dimensioni VS, dilatazione"
    ),
    summary: tx!(
        "End-diastolic and end-systolic LV diameters divided by body surface area, each graded by the sex-specific normal ranges and abnormality partitions of the chamber quantification guideline.",
        "Diametri telediastolico e telesistolico del VS divisi per la superficie corporea, ciascuno classificato con gli intervalli normali e le partizioni di anormalità per sesso della linea guida sulla quantificazione delle camere."
    ),
    inputs: &[
        LVIDD,
        opt(
            "lvids_cm",
            tx!(
                "LV internal diameter, end-systole",
                "Diametro interno VS, telesistole"
            ),
            Unit::Cm,
            1.0,
            8.0,
            2,
        ),
        BSA,
        SEX,
    ],
    compute: |i| {
        let (d, bsa) = (i.n("lvidd_cm"), i.n("bsa_m2"));
        let mut out = outcome();
        out.push(
            q(
                "lvidd_index_cm_m2",
                tx!(
                    "Indexed end-diastolic diameter",
                    "Diametro telediastolico indicizzato"
                ),
                d / bsa,
                Unit::CmM2,
                2,
            ),
            "LVIDd ÷ BSA",
            format!("{} ÷ {}", v(d, 2, Unit::Cm), v(bsa, 2, Unit::M2)),
        );
        if let Some(s) = o(i, "lvids_cm") {
            check(
                s >= d,
                "lvids_cm",
                tx!(
                    "The end-systolic diameter must be smaller than the end-diastolic one",
                    "Il diametro telesistolico deve essere minore di quello telediastolico"
                ),
            )?;
            out.push(
                q(
                    "lvids_index_cm_m2",
                    tx!(
                        "Indexed end-systolic diameter",
                        "Diametro telesistolico indicizzato"
                    ),
                    s / bsa,
                    Unit::CmM2,
                    2,
                ),
                "LVIDs ÷ BSA",
                format!("{} ÷ {}", v(s, 2, Unit::Cm), v(bsa, 2, Unit::M2)),
            );
        }
        let s = o(i, "lvids_cm");
        graded(
            i,
            &mut out,
            &[
                (&LVIDD_ROW, Some(d)),
                (&LVIDD_I_ROW, Some(d / bsa)),
                (&LVIDS_ROW, s),
                (&LVIDS_I_ROW, s.map(|s| s / bsa)),
            ],
        );
        Ok(out)
    },
    references: &[cite(
        &CQ_2015,
        "§1.1 linear measurements; Supplemental Table 3, p. 39.e8",
    )],
    review: Review::Verified,
    example: &[
        ("lvidd_cm", N(5.4)),
        ("lvids_cm", N(3.6)),
        ("bsa_m2", N(1.95)),
        ("sex", C("female")),
    ],
    sim_link: None,
};

/// Linear (cube-formula) LV mass with the ASE correction, its index and relative wall thickness.
pub static LV_MASS: Calculator = Calculator {
    id: "lv-mass-linear",
    category: Category::VentricularFunction,
    name: tx!("LV mass (linear method)", "Massa del VS (metodo lineare)"),
    synonyms: tx!(
        "LVM, LV mass index, LVMI, RWT, relative wall thickness, cube formula, Devereux, hypertrophy, concentric, eccentric",
        "massa VS, indice di massa, RWT, spessore parietale relativo, formula cubica, Devereux, ipertrofia, concentrica, eccentrica"
    ),
    summary: tx!(
        "Myocardial shell volume from the cube of the outer and inner diameters, times myocardial density 1.04 g/mL, with the 0.8 correction and + 0.6 g. Relative wall thickness = 2 × posterior wall ÷ LVIDd separates concentric from eccentric patterns. Cubing magnifies measurement errors.",
        "Volume del guscio miocardico dal cubo dei diametri esterno e interno, per la densità miocardica 1,04 g/mL, con la correzione 0,8 e + 0,6 g. Lo spessore parietale relativo = 2 × parete posteriore ÷ DTD distingue i quadri concentrici da quelli eccentrici. Il cubo amplifica gli errori di misura."
    ),
    inputs: &[
        num(
            "ivsd_cm",
            tx!(
                "Septal thickness, end-diastole",
                "Spessore del setto, telediastole"
            ),
            Unit::Cm,
            0.4,
            2.5,
            2,
        ),
        LVIDD,
        num(
            "pwd_cm",
            tx!(
                "Posterior (inferolateral) wall, end-diastole",
                "Parete posteriore (inferolaterale), telediastole"
            ),
            Unit::Cm,
            0.4,
            2.5,
            2,
        ),
        BSA_OPT,
        SEX,
    ],
    compute: lv_mass,
    references: &[
        cite(&CQ_2015, "Table 5 (cube formula), p. 14; RWT, p. 16"),
        cite(
            &CQ_2015,
            "Supplemental Table 3, p. 39.e8; Table 6 and Figure 6, p. 17",
        ),
    ],
    review: Review::Verified,
    example: &[
        ("ivsd_cm", N(1.2)),
        ("lvidd_cm", N(4.8)),
        ("pwd_cm", N(1.1)),
        ("bsa_m2", N(1.9)),
        ("sex", C("male")),
    ],
    sim_link: None,
};

fn lv_mass(i: &Inputs) -> Result<Outcome, Vec<InputError>> {
    let (s, d, p) = (i.n("ivsd_cm"), i.n("lvidd_cm"), i.n("pwd_cm"));
    let mass = 0.8 * 1.04 * ((s + d + p).powi(3) - d.powi(3)) + 0.6;
    let mut out = outcome();
    out.push(
        q("lv_mass_g", tx!("LV mass", "Massa del VS"), mass, Unit::G, 0),
        "LVM = 0.8 × 1.04 × [(IVS + LVID + PW)³ − LVID³] + 0.6",
        format!(
            "0.8 × 1.04 × [({} + {} + {})³ − {}³] + 0.6",
            n(s, 2),
            n(d, 2),
            n(p, 2),
            n(d, 2)
        ),
    );
    if let Some(bsa) = o(i, "bsa_m2") {
        out.push(
            q(
                "lv_mass_index_g_m2",
                tx!("LV mass index", "Indice di massa del VS"),
                mass / bsa,
                Unit::GM2,
                0,
            ),
            "LVMi = LVM ÷ BSA",
            format!("{} ÷ {}", v(mass, 0, Unit::G), v(bsa, 2, Unit::M2)),
        );
    }
    let rwt = 2.0 * p / d;
    out.push(
        q(
            "rwt",
            tx!("Relative wall thickness", "Spessore parietale relativo"),
            rwt,
            Unit::None,
            2,
        ),
        "RWT = 2 × PW ÷ LVID",
        format!("2 × {} ÷ {}", n(p, 2), n(d, 2)),
    );
    let src = cite(&CQ_2015, "RWT, p. 16");
    out.findings.push(if gt(rwt, 0.42) {
        criterion_met(Severity::Info, tx!("RWT above 0.42: the cut-off the guideline uses for a concentric pattern", "RWT sopra 0,42: la soglia che la linea guida usa per un quadro concentrico"), "RWT > 0.42", src)
    } else {
        criterion_met(Severity::Info, tx!("RWT at or below 0.42: the guideline's side of the cut-off for an eccentric or normal pattern", "RWT pari o inferiore a 0,42: il lato della soglia che la linea guida usa per un quadro eccentrico o normale"), "RWT ≤ 0.42", src)
    });
    let index = o(i, "bsa_m2").map(|bsa| mass / bsa);
    graded(
        i,
        &mut out,
        &[
            (&MASS_ROW, Some(mass)),
            (&MASS_I_ROW, index),
            (&WALL_ROW, Some(s)),
            (&PW_ROW, Some(p)),
        ],
    );
    // the geometry of Figure 6: LV mass index against its upper normal limit, RWT against 0.42
    if let (Some(m), Some(index)) = (male(i), index) {
        let limit = if m { 115.0 } else { 95.0 };
        let increased = gt(index, limit);
        let concentric = gt(rwt, 0.42);
        let (text, crit) = match (increased, concentric) {
            (true, true) => (
                tx!(
                    "Increased mass with RWT above 0.42: the guideline's concentric hypertrophy",
                    "Massa aumentata con RWT sopra 0,42: l'ipertrofia concentrica della linea guida"
                ),
                "LVMi > ULN, RWT > 0.42",
            ),
            (true, false) => (
                tx!(
                    "Increased mass with RWT at or below 0.42: the guideline's eccentric hypertrophy",
                    "Massa aumentata con RWT pari o sotto 0,42: l'ipertrofia eccentrica della linea guida"
                ),
                "LVMi > ULN, RWT ≤ 0.42",
            ),
            (false, true) => (
                tx!(
                    "Normal mass with RWT above 0.42: the guideline's concentric remodelling",
                    "Massa normale con RWT sopra 0,42: il rimodellamento concentrico della linea guida"
                ),
                "LVMi ≤ ULN, RWT > 0.42",
            ),
            (false, false) => (
                tx!(
                    "Normal mass with RWT at or below 0.42: the guideline's normal geometry",
                    "Massa normale con RWT pari o sotto 0,42: la geometria normale della linea guida"
                ),
                "LVMi ≤ ULN, RWT ≤ 0.42",
            ),
        };
        out.findings.push(criterion_met(
            if increased || concentric {
                Severity::Info
            } else {
                Severity::Normal
            },
            text,
            &format!("{crit} (ULN {} g/m²)", n(limit, 0)),
            cite(&CQ_2015, "LV mass and RWT, p. 16; Figure 6, p. 17"),
        ));
    }
    Ok(out)
}

/// LV volumes indexed to body surface area.
pub static LV_VOLUMES_INDEXED: Calculator = Calculator {
    id: "lv-volumes-indexed",
    category: Category::VentricularFunction,
    name: tx!("Indexed LV volumes", "Volumi del VS indicizzati"),
    synonyms: tx!(
        "EDV, ESV, EDVi, ESVi, biplane, Simpson, LV size",
        "VTD, VTS, VTDi, VTSi, biplano, Simpson, dimensioni VS"
    ),
    summary: tx!(
        "Biplane (disc summation) end-diastolic and end-systolic volumes, absolute and divided by body surface area, graded by the sex-specific 2D ranges of the chamber quantification guideline (3D volumes have other reference values, not applied).",
        "Volumi telediastolico e telesistolico biplani (sommatoria dei dischi), assoluti e divisi per la superficie corporea, classificati con gli intervalli 2D per sesso della linea guida sulla quantificazione delle camere (i volumi 3D hanno altri valori di riferimento, non applicati)."
    ),
    inputs: &[
        num(
            "edv_ml",
            tx!("End-diastolic volume", "Volume telediastolico"),
            Unit::Ml,
            20.0,
            500.0,
            0,
        ),
        opt(
            "esv_ml",
            tx!("End-systolic volume", "Volume telesistolico"),
            Unit::Ml,
            5.0,
            450.0,
            0,
        ),
        BSA,
        SEX,
    ],
    compute: |i| {
        let (edv, bsa) = (i.n("edv_ml"), i.n("bsa_m2"));
        let mut out = outcome();
        out.push(
            q(
                "edvi_ml_m2",
                tx!(
                    "Indexed end-diastolic volume",
                    "Volume telediastolico indicizzato"
                ),
                edv / bsa,
                Unit::MlM2,
                0,
            ),
            "EDVi = EDV ÷ BSA",
            format!("{} ÷ {}", v(edv, 0, Unit::Ml), v(bsa, 2, Unit::M2)),
        );
        if let Some(esv) = o(i, "esv_ml") {
            check(
                esv >= edv,
                "esv_ml",
                tx!(
                    "The end-systolic volume must be smaller than the end-diastolic one",
                    "Il volume telesistolico deve essere minore di quello telediastolico"
                ),
            )?;
            out.push(
                q(
                    "esvi_ml_m2",
                    tx!("Indexed end-systolic volume", "Volume telesistolico indicizzato"),
                    esv / bsa,
                    Unit::MlM2,
                    0,
                ),
                "ESVi = ESV ÷ BSA",
                format!("{} ÷ {}", v(esv, 0, Unit::Ml), v(bsa, 2, Unit::M2)),
            );
        }
        let esv = o(i, "esv_ml");
        graded(
            i,
            &mut out,
            &[
                (&EDV_ROW, Some(edv)),
                (&EDVI_ROW, Some(edv / bsa)),
                (&ESV_ROW, esv),
                (&ESVI_ROW, esv.map(|e| e / bsa)),
            ],
        );
        Ok(out)
    },
    references: &[cite(&CQ_2015, "§1.2 volumes; Supplemental Table 3, p. 39.e8")],
    review: Review::Verified,
    example: &[
        ("edv_ml", N(118.0)),
        ("esv_ml", N(46.0)),
        ("bsa_m2", N(1.8)),
        ("sex", C("male")),
    ],
    sim_link: None,
};

/// Ejection fraction from end-diastolic and end-systolic volumes.
pub static EJECTION_FRACTION: Calculator = Calculator {
    id: "ejection-fraction",
    category: Category::VentricularFunction,
    name: tx!("Ejection fraction", "Frazione di eiezione"),
    synonyms: tx!(
        "EF, LVEF, biplane, Simpson, stroke volume, systolic function",
        "FE, FEVS, biplano, Simpson, volume sistolico, funzione sistolica"
    ),
    summary: tx!(
        "The fraction of the end-diastolic volume ejected: (EDV − ESV) ÷ EDV, from biplane volumes, graded by the sex-specific normal range and partitions of the chamber quantification guideline (below 52% in men and 54% in women is abnormal).",
        "La frazione del volume telediastolico espulsa: (VTD − VTS) ÷ VTD, dai volumi biplani, classificata con l'intervallo normale e le partizioni per sesso della linea guida sulla quantificazione delle camere (sotto 52% negli uomini e 54% nelle donne è anormale)."
    ),
    inputs: &[
        num(
            "edv_ml",
            tx!("End-diastolic volume", "Volume telediastolico"),
            Unit::Ml,
            20.0,
            500.0,
            0,
        ),
        num(
            "esv_ml",
            tx!("End-systolic volume", "Volume telesistolico"),
            Unit::Ml,
            5.0,
            450.0,
            0,
        ),
        SEX,
    ],
    compute: |i| {
        let (edv, esv) = (i.n("edv_ml"), i.n("esv_ml"));
        check(
            esv >= edv,
            "esv_ml",
            tx!(
                "The end-systolic volume must be smaller than the end-diastolic one",
                "Il volume telesistolico deve essere minore di quello telediastolico"
            ),
        )?;
        let mut out = outcome();
        out.push(
            q(
                "sv_ml",
                tx!("Stroke volume", "Volume sistolico"),
                edv - esv,
                Unit::Ml,
                0,
            ),
            "SV = EDV − ESV",
            format!("{} − {}", n(edv, 0), n(esv, 0)),
        );
        out.push(
            q(
                "ef_percent",
                tx!("Ejection fraction", "Frazione di eiezione"),
                fraction_percent(edv, esv),
                Unit::Percent,
                0,
            ),
            "EF = (EDV − ESV) ÷ EDV × 100",
            format!("({} − {}) ÷ {} × 100", n(edv, 0), n(esv, 0), n(edv, 0)),
        );
        graded(i, &mut out, &[(&EF_ROW, Some(fraction_percent(edv, esv)))]);
        Ok(out)
    },
    references: &[
        cite(&CQ_2015, "§2.2 ejection fraction, pp. 6–7"),
        cite(&CQ_2015, "Table 4, p. 10"),
    ],
    review: Review::Verified,
    example: &[("edv_ml", N(140.0)), ("esv_ml", N(84.0)), ("sex", C("female"))],
    sim_link: None,
};

/// Relative change of global longitudinal strain from a baseline.
pub static GLS_CHANGE: Calculator = Calculator {
    id: "gls-relative-change",
    category: Category::VentricularFunction,
    name: tx!("Relative change of GLS", "Variazione relativa del GLS"),
    synonyms: tx!(
        "GLS, global longitudinal strain, speckle tracking, cardio-oncology, cardiotoxicity, CTRCD, serial",
        "GLS, strain longitudinale globale, speckle tracking, cardio-oncologia, cardiotossicità, CTRCD, seriale"
    ),
    summary: tx!(
        "Compares the magnitude of the current GLS with a baseline on the same vendor and software: relative reduction = (|baseline| − |current|) ÷ |baseline| × 100. Strain is usually written negative; enter both with the same sign.",
        "Confronta il valore assoluto del GLS attuale con un basale ottenuto con stesso fornitore e software: riduzione relativa = (|basale| − |attuale|) ÷ |basale| × 100. Lo strain si scrive di solito negativo; inserire entrambi con lo stesso segno."
    ),
    inputs: &[
        num(
            "gls_baseline_percent",
            tx!("Baseline GLS", "GLS basale"),
            Unit::Percent,
            -35.0,
            35.0,
            1,
        ),
        num(
            "gls_current_percent",
            tx!("Current GLS", "GLS attuale"),
            Unit::Percent,
            -35.0,
            35.0,
            1,
        ),
    ],
    compute: gls_change,
    references: &[cite(&STRAIN_2025, "Key points p. 995; cardio-oncology p. 999")],
    review: Review::Verified,
    example: &[
        ("gls_baseline_percent", N(-20.5)),
        ("gls_current_percent", N(-17.0)),
    ],
    sim_link: None,
};

fn gls_change(i: &Inputs) -> Result<Outcome, Vec<InputError>> {
    let (b, c) = (i.n("gls_baseline_percent"), i.n("gls_current_percent"));
    check(
        b.abs() < 1.0,
        "gls_baseline_percent",
        tx!(
            "A baseline GLS this small cannot be compared",
            "Un GLS basale così piccolo non è confrontabile"
        ),
    )?;
    check(
        b * c < 0.0,
        "gls_current_percent",
        tx!(
            "Enter both strains with the same sign",
            "Inserire entrambi gli strain con lo stesso segno"
        ),
    )?;
    let (b, c) = (b.abs(), c.abs());
    let rel = (b - c) / b * 100.0;
    let mut out = outcome();
    out.push(
        q(
            "absolute_change_points",
            tx!(
                "Absolute fall (percentage points)",
                "Calo assoluto (punti percentuali)"
            ),
            b - c,
            Unit::None,
            1,
        ),
        "|GLS₀| − |GLS|",
        format!("{} − {}", n(b, 1), n(c, 1)),
    );
    out.push(
        q(
            "relative_reduction_percent",
            tx!("Relative reduction", "Riduzione relativa"),
            rel,
            Unit::Percent,
            1,
        ),
        "(|GLS₀| − |GLS|) ÷ |GLS₀| × 100",
        format!("({} − {}) ÷ {} × 100", n(b, 1), n(c, 1), n(b, 1)),
    );
    out.findings.push(if ge(rel, 15.0) {
        criterion_met(Severity::Moderate, tx!("A relative fall of 15 % or more: the consensus regards this as preclinical dysfunction during cardiotoxic therapy (ESC and IC-OS: mild CTRCD)", "Un calo relativo del 15 % o più: il consenso lo considera disfunzione preclinica durante terapia cardiotossica (ESC e IC-OS: CTRCD lieve)"), "relative fall ≥ 15 %", cite(&STRAIN_2025, "p. 999"))
    } else if ge(rel, 10.0) {
        criterion_met(Severity::Info, tx!("A relative fall in the 10–15 % range the consensus calls likely to be significant", "Un calo relativo nell'intervallo 10–15 % che il consenso ritiene probabilmente significativo"), "10 % ≤ relative fall < 15 %", cite(&STRAIN_2025, "Key points, p. 995"))
    } else {
        criterion_met(Severity::Normal, tx!("A relative change below the 10–15 % the consensus calls likely to be significant", "Una variazione relativa sotto il 10–15 % che il consenso ritiene probabilmente significativo"), "relative fall < 10 %", cite(&STRAIN_2025, "Key points, p. 995"))
    });
    Ok(out)
}

/// LVOT stroke volume, cardiac output and their indices.
pub static LVOT_STROKE_VOLUME: Calculator = Calculator {
    id: "lvot-stroke-volume",
    category: Category::VentricularFunction,
    name: tx!(
        "Stroke volume and cardiac output (LVOT)",
        "Volume sistolico e portata cardiaca (LVOT)"
    ),
    synonyms: tx!(
        "SV, CO, SVi, CI, cardiac index, LVOT VTI, Doppler stroke volume, low flow",
        "VS, GC, VSi, IC, indice cardiaco, VTI LVOT, volume sistolico Doppler, basso flusso"
    ),
    summary: tx!(
        "Doppler stroke volume is the LVOT cross-section times the pulsed-wave VTI at the same level; times heart rate it is the cardiac output. Diameter errors are squared.",
        "Il volume sistolico Doppler è la sezione del LVOT per la VTI in Doppler pulsato allo stesso livello; per la frequenza cardiaca è la portata. Gli errori sul diametro sono elevati al quadrato."
    ),
    inputs: &[
        num(
            "lvot_d_cm",
            tx!("LVOT diameter", "Diametro LVOT"),
            Unit::Cm,
            1.0,
            4.0,
            2,
        ),
        num(
            "lvot_vti_cm",
            tx!("LVOT VTI (pulsed wave)", "VTI LVOT (Doppler pulsato)"),
            Unit::Cm,
            3.0,
            60.0,
            1,
        ),
        opt(
            "hr_bpm",
            tx!("Heart rate", "Frequenza cardiaca"),
            Unit::Bpm,
            25.0,
            220.0,
            0,
        ),
        BSA_OPT,
    ],
    compute: lvot_flow,
    references: &[
        cite(&VR_2017, "Stroke volume at a site, p. 311"),
        cite(&RH_2025, "Table 4 (CO = πr² × VTI × HR), p. 162"),
        cite(&AS_2017, "Low flow, SVi < 35 mL/m², p. 383"),
    ],
    review: Review::Verified,
    example: &[
        ("lvot_d_cm", N(2.1)),
        ("lvot_vti_cm", N(19.0)),
        ("hr_bpm", N(68.0)),
        ("bsa_m2", N(1.85)),
    ],
    sim_link: Some(SimLink::HeartRate("hr_bpm")),
};

fn lvot_flow(i: &Inputs) -> Result<Outcome, Vec<InputError>> {
    let (d, vti) = (i.n("lvot_d_cm"), i.n("lvot_vti_cm"));
    let (hr, bsa) = (o(i, "hr_bpm"), o(i, "bsa_m2"));
    let csa = circle_area_cm2(d);
    let sv = stroke_volume_ml(d, vti);
    let mut out = outcome();
    out.push(
        q("lvot_area_cm2", tx!("LVOT area", "Area LVOT"), csa, Unit::Cm2, 2),
        "CSA = π × (d/2)²",
        format!("π × ({} / 2)²", v(d, 2, Unit::Cm)),
    );
    out.push(
        q("sv_ml", tx!("Stroke volume", "Volume sistolico"), sv, Unit::Ml, 0),
        "SV = CSA × VTI",
        format!("{} × {}", v(csa, 2, Unit::Cm2), v(vti, 1, Unit::Cm)),
    );
    if let Some(bsa) = bsa {
        let svi = sv / bsa;
        out.push(
            q(
                "svi_ml_m2",
                tx!("Stroke volume index", "Volume sistolico indicizzato"),
                svi,
                Unit::MlM2,
                0,
            ),
            "SVi = SV ÷ BSA",
            format!("{} ÷ {}", n(sv, 0), v(bsa, 2, Unit::M2)),
        );
        if lt(svi, 35.0) {
            out.findings.push(criterion_met(Severity::Info, tx!("SVi below 35 mL/m²: the example of low flow the aortic stenosis update gives", "VSi sotto 35 mL/m²: l'esempio di basso flusso indicato dall'aggiornamento sulla stenosi aortica"), "SVi < 35 mL/m²", cite(&AS_2017, "p. 383")));
        } else {
            out.findings.push(criterion_met(Severity::Normal, tx!("SVi at or above the 35 mL/m² the aortic stenosis update uses as a low-flow example", "VSi pari o superiore ai 35 mL/m² che l'aggiornamento sulla stenosi aortica usa come esempio di basso flusso"), "SVi ≥ 35 mL/m²", cite(&AS_2017, "p. 383")));
        }
    }
    if let Some(hr) = hr {
        let co = sv * hr / 1000.0;
        out.push(
            q(
                "co_l_min",
                tx!("Cardiac output", "Portata cardiaca"),
                co,
                Unit::LMin,
                2,
            ),
            "CO = SV × HR ÷ 1000",
            format!("{} × {} ÷ 1000", n(sv, 0), v(hr, 0, Unit::Bpm)),
        );
        if let Some(bsa) = bsa {
            out.push(
                q(
                    "ci_l_min_m2",
                    tx!("Cardiac index", "Indice cardiaco"),
                    co / bsa,
                    Unit::LMinM2,
                    2,
                ),
                "CI = CO ÷ BSA",
                format!("{} ÷ {}", n(co, 2), v(bsa, 2, Unit::M2)),
            );
        }
    }
    Ok(out)
}

const LEVELS: &[Choice] = &[
    Choice {
        id: "annulus",
        label: tx!("Annulus", "Anulus"),
    },
    Choice {
        id: "sinuses",
        label: tx!("Sinuses of Valsalva", "Seni di Valsalva"),
    },
    Choice {
        id: "stj",
        label: tx!("Sinotubular junction", "Giunzione sinotubulare"),
    },
    Choice {
        id: "ascending",
        label: tx!("Proximal ascending aorta", "Aorta ascendente prossimale"),
    },
];

/// An aortic diameter indexed to body surface area.
pub static AORTIC_DIAMETER_INDEX: Calculator = Calculator {
    id: "aortic-diameter-index",
    category: Category::VentricularFunction,
    name: tx!("Indexed aortic diameter", "Diametro aortico indicizzato"),
    synonyms: tx!(
        "aortic root, sinuses of Valsalva, ascending aorta, sinotubular junction, annulus, dilatation",
        "radice aortica, seni di Valsalva, aorta ascendente, giunzione sinotubulare, anulus, dilatazione"
    ),
    summary: tx!(
        "An aortic diameter divided by body surface area. The age- and sex-stratified reference values are not applied here.",
        "Un diametro aortico diviso per la superficie corporea. I valori di riferimento per età e sesso non sono applicati qui."
    ),
    inputs: &[
        choice("level", tx!("Level", "Livello"), LEVELS),
        num("diameter_cm", tx!("Diameter", "Diametro"), Unit::Cm, 1.0, 7.0, 2),
        BSA,
    ],
    compute: |i| {
        let (d, bsa) = (i.n("diameter_cm"), i.n("bsa_m2"));
        let mut out = outcome();
        out.push(
            q(
                "diameter_index_cm_m2",
                tx!("Indexed diameter", "Diametro indicizzato"),
                d / bsa,
                Unit::CmM2,
                2,
            ),
            "D ÷ BSA",
            format!("{} ÷ {}", v(d, 2, Unit::Cm), v(bsa, 2, Unit::M2)),
        );
        Ok(out)
    },
    references: &[cite(&CQ_2015, "§10 aorta and Table 14, p. 32")],
    review: Review::FormulaOnly,
    example: &[
        ("level", C("sinuses")),
        ("diameter_cm", N(3.6)),
        ("bsa_m2", N(1.9)),
    ],
    sim_link: None,
};

const SCORES: &[Choice] = &[
    Choice {
        id: "1",
        label: tx!("1 normal or hyperkinetic", "1 normale o ipercinetico"),
    },
    Choice {
        id: "2",
        label: tx!("2 hypokinetic", "2 ipocinetico"),
    },
    Choice {
        id: "3",
        label: tx!("3 akinetic", "3 acinetico"),
    },
    Choice {
        id: "4",
        label: tx!("4 dyskinetic", "4 discinetico"),
    },
];
const MODELS: &[Choice] = &[
    Choice {
        id: "16",
        label: tx!("16 segments", "16 segmenti"),
    },
    Choice {
        id: "17",
        label: tx!("17 segments (with apical cap)", "17 segmenti (con apice)"),
    },
];

macro_rules! segment {
    ($id:expr, $en:expr, $it:expr) => {
        InputSpec {
            id: $id,
            label: tx!($en, $it),
            kind: crate::model::InputKind::Choice(SCORES),
            optional: true,
            when: None,
        }
    };
}

/// Segment ids in the order of the 17-segment model.
pub const SEGMENTS: [&str; 17] = [
    "s01", "s02", "s03", "s04", "s05", "s06", "s07", "s08", "s09", "s10", "s11", "s12", "s13", "s14", "s15",
    "s16", "s17",
];

/// Wall-motion score index over the segments that were seen.
pub static WALL_MOTION_SCORE: Calculator = Calculator {
    id: "wall-motion-score-index",
    category: Category::VentricularFunction,
    name: tx!("Wall-motion score index", "Indice di cinesi parietale"),
    synonyms: tx!(
        "WMSI, WMS, regional wall motion, segments, hypokinesia, akinesia, dyskinesia, stress echo",
        "WMSI, cinesi regionale, segmenti, ipocinesia, acinesia, discinesia, eco stress"
    ),
    summary: tx!(
        "Each segment visualised is scored 1 (normal or hyperkinetic), 2 (hypokinetic), 3 (akinetic) or 4 (dyskinetic); the index is the mean of the scores of the segments seen. Leave a segment empty if it was not seen. The apical cap is scored in the 17-segment model only.",
        "Ogni segmento visualizzato riceve 1 (normale o ipercinetico), 2 (ipocinetico), 3 (acinetico) o 4 (discinetico); l'indice è la media dei punteggi dei segmenti visti. Lasciare vuoto un segmento non visto. L'apice è valutato solo nel modello a 17 segmenti."
    ),
    inputs: &[
        choice("model", tx!("Segment model", "Modello segmentario"), MODELS),
        segment!("s01", "1 Basal anterior", "1 Anteriore basale"),
        segment!("s02", "2 Basal anteroseptal", "2 Anterosettale basale"),
        segment!("s03", "3 Basal inferoseptal", "3 Inferosettale basale"),
        segment!("s04", "4 Basal inferior", "4 Inferiore basale"),
        segment!("s05", "5 Basal inferolateral", "5 Inferolaterale basale"),
        segment!("s06", "6 Basal anterolateral", "6 Anterolaterale basale"),
        segment!("s07", "7 Mid anterior", "7 Anteriore medio"),
        segment!("s08", "8 Mid anteroseptal", "8 Anterosettale medio"),
        segment!("s09", "9 Mid inferoseptal", "9 Inferosettale medio"),
        segment!("s10", "10 Mid inferior", "10 Inferiore medio"),
        segment!("s11", "11 Mid inferolateral", "11 Inferolaterale medio"),
        segment!("s12", "12 Mid anterolateral", "12 Anterolaterale medio"),
        segment!("s13", "13 Apical anterior", "13 Anteriore apicale"),
        segment!("s14", "14 Apical septal", "14 Settale apicale"),
        segment!("s15", "15 Apical inferior", "15 Inferiore apicale"),
        segment!("s16", "16 Apical lateral", "16 Laterale apicale"),
        when(
            segment!("s17", "17 Apex (apical cap)", "17 Apice"),
            "model",
            &["17"],
        ),
    ],
    compute: wmsi,
    references: &[cite(
        &CQ_2015,
        "Regional function and scoring, p. 11; Figures 3–4",
    )],
    review: Review::FormulaOnly,
    example: &[
        ("model", C("17")),
        ("s01", C("1")),
        ("s02", C("2")),
        ("s03", C("1")),
        ("s04", C("1")),
        ("s05", C("1")),
        ("s06", C("1")),
        ("s07", C("2")),
        ("s08", C("3")),
        ("s09", C("1")),
        ("s10", C("1")),
        ("s11", C("1")),
        ("s12", C("1")),
        ("s13", C("3")),
        ("s14", C("3")),
        ("s15", C("2")),
        ("s16", C("1")),
        ("s17", C("3")),
    ],
    sim_link: None,
};

fn wmsi(i: &Inputs) -> Result<Outcome, Vec<InputError>> {
    let count = if i.choice("model") == Some("17") { 17 } else { 16 };
    let scores: Vec<u32> = SEGMENTS[..count]
        .iter()
        .filter_map(|s| i.choice(s).and_then(|c| c.parse().ok()))
        .collect();
    if scores.is_empty() {
        return Err(vec![InputError::inconsistent(
            "s01",
            tx!(
                "Score at least one segment",
                "Assegnare un punteggio ad almeno un segmento"
            ),
        )]);
    }
    let sum: u32 = scores.iter().sum();
    let k = scores.len() as u32;
    let mut out = outcome();
    out.push(
        q(
            "segments_scored",
            tx!("Segments scored", "Segmenti valutati"),
            f64::from(k),
            Unit::Score,
            0,
        ),
        "segments seen",
        format!("{k} / {count}"),
    );
    out.push(
        q(
            "wmsi",
            tx!("Wall-motion score index", "Indice di cinesi parietale"),
            f64::from(sum) / f64::from(k),
            Unit::None,
            2,
        ),
        "WMSI = Σ scores ÷ segments scored",
        format!("{sum} ÷ {k}"),
    );
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::super::testing::{close, inconsistent, ok, severities};
    use super::*;

    #[test]
    fn fs_and_ef_hand_cases() {
        // (5.0 − 3.0) ÷ 5.0 = 40 %
        let o = ok(
            &FRACTIONAL_SHORTENING,
            &Inputs::new().with("lvidd_cm", 5.0).with("lvids_cm", 3.0),
        );
        close(o.value("fs_percent"), 40.0, 1e-9);
        inconsistent(
            &FRACTIONAL_SHORTENING,
            &Inputs::new().with("lvidd_cm", 4.0).with("lvids_cm", 4.5),
            "lvids_cm",
        );
        // (120 − 48) ÷ 120 = 60 %, SV 72
        let o = ok(
            &EJECTION_FRACTION,
            &Inputs::new().with("edv_ml", 120.0).with("esv_ml", 48.0),
        );
        close(o.value("ef_percent"), 60.0, 1e-9);
        close(o.value("sv_ml"), 72.0, 1e-9);
        // without the sex the grading only asks for it
        assert_eq!(severities(&o), [Severity::Info]);
        inconsistent(
            &EJECTION_FRACTION,
            &Inputs::new().with("edv_ml", 100.0).with("esv_ml", 120.0),
            "esv_ml",
        );
        inconsistent(
            &LV_VOLUMES_INDEXED,
            &Inputs::new()
                .with("edv_ml", 100.0)
                .with("esv_ml", 120.0)
                .with("bsa_m2", 2.0),
            "esv_ml",
        );
    }

    fn ef(ef: f64, sex: &str) -> Severity {
        // EDV 100 mL, so ESV = 100 − EF
        ok(
            &EJECTION_FRACTION,
            &Inputs::new()
                .with("edv_ml", 100.0)
                .with("esv_ml", 100.0 - ef)
                .with_choice("sex", sex),
        )
        .findings[0]
            .severity
    }

    #[test]
    fn ef_partitions_by_sex() {
        // Table 4: men 52-72 normal, 41-51 mild, 30-40 moderate, < 30 severe
        assert_eq!(ef(52.0, "male"), Severity::Normal);
        assert_eq!(
            ef(51.9, "male"),
            Severity::Mild,
            "in the 51-52 gap: the more abnormal band"
        );
        assert_eq!(ef(41.0, "male"), Severity::Mild);
        assert_eq!(ef(40.9, "male"), Severity::Moderate);
        assert_eq!(ef(30.0, "male"), Severity::Moderate);
        assert_eq!(ef(29.9, "male"), Severity::Severe);
        assert_eq!(ef(72.0, "male"), Severity::Normal);
        assert_eq!(ef(72.1, "male"), Severity::Info, "above the normal range");
        // women: 54-74 normal, 41-53 mild
        assert_eq!(ef(54.0, "female"), Severity::Normal);
        assert_eq!(ef(53.9, "female"), Severity::Mild);
        assert_eq!(ef(74.0, "female"), Severity::Normal);
        assert_eq!(ef(74.1, "female"), Severity::Info);
        // the same EF of 53 % is normal for a man, mildly abnormal for a woman
        assert_eq!(ef(53.0, "male"), Severity::Normal);
        assert_eq!(ef(53.0, "female"), Severity::Mild);
    }

    #[test]
    fn size_partitions_by_sex() {
        let d = |lvidd: f64, sex: &str| {
            ok(
                &LV_DIAMETER_INDEX,
                &Inputs::new()
                    .with("lvidd_cm", lvidd)
                    .with("bsa_m2", 2.0)
                    .with_choice("sex", sex),
            )
            .findings[0]
                .severity
        };
        // men: 4.2-5.8 normal, 5.9-6.3 mild, 6.4-6.8 moderate, > 6.8 severe
        assert_eq!(d(4.19, "male"), Severity::Info, "below the normal range");
        assert_eq!(d(4.2, "male"), Severity::Normal);
        assert_eq!(d(5.8, "male"), Severity::Normal);
        assert_eq!(d(5.85, "male"), Severity::Mild);
        assert_eq!(d(6.3, "male"), Severity::Mild);
        assert_eq!(d(6.31, "male"), Severity::Moderate);
        assert_eq!(d(6.8, "male"), Severity::Moderate);
        assert_eq!(d(6.81, "male"), Severity::Severe);
        // women: 3.8-5.2 normal, 5.3-5.6 mild, 5.7-6.1 moderate
        assert_eq!(d(5.2, "female"), Severity::Normal);
        assert_eq!(d(5.21, "female"), Severity::Mild);
        assert_eq!(d(6.1, "female"), Severity::Moderate);
        assert_eq!(d(6.11, "female"), Severity::Severe);
        // indexed: LVIDd 6.0 cm ÷ 2.0 m² = 3.0 cm/m²: normal for a man (2.2-3.0), normal for a
        // woman (2.3-3.1); 6.4 ÷ 2 = 3.2: mild for both (3.1-3.3, 3.2-3.4)
        let di = |lvidd: f64, sex: &str| {
            ok(
                &LV_DIAMETER_INDEX,
                &Inputs::new()
                    .with("lvidd_cm", lvidd)
                    .with("bsa_m2", 2.0)
                    .with_choice("sex", sex),
            )
            .findings[1]
                .severity
        };
        assert_eq!(di(6.0, "male"), Severity::Normal);
        assert_eq!(di(6.2, "male"), Severity::Mild);
        assert_eq!(di(6.2, "female"), Severity::Normal);
        assert_eq!(di(6.4, "female"), Severity::Mild);
        // volumes: EDV 107 mL is mildly abnormal for a woman (107-120), normal for a man
        let vol = |edv: f64, sex: &str| {
            ok(
                &LV_VOLUMES_INDEXED,
                &Inputs::new()
                    .with("edv_ml", edv)
                    .with("bsa_m2", 2.0)
                    .with_choice("sex", sex),
            )
            .findings[0]
                .severity
        };
        assert_eq!(vol(106.0, "female"), Severity::Normal);
        assert_eq!(vol(107.0, "female"), Severity::Mild);
        assert_eq!(vol(130.0, "female"), Severity::Moderate);
        assert_eq!(vol(131.0, "female"), Severity::Severe);
        assert_eq!(vol(150.0, "male"), Severity::Normal);
        assert_eq!(vol(151.0, "male"), Severity::Mild);
        assert_eq!(vol(200.0, "male"), Severity::Moderate);
        assert_eq!(vol(201.0, "male"), Severity::Severe);
    }

    #[test]
    fn lv_mass_partitions_and_geometry() {
        // 182 g, BSA 2.0 → 91 g/m², RWT 0.40: normal for a woman (≤ 95), so normal geometry; with
        // PW 1.2: 207 g → 104 g/m² and RWT 0.48, concentric hypertrophy
        let i = |pw: f64, sex: &str| {
            ok(
                &LV_MASS,
                &Inputs::new()
                    .with("ivsd_cm", 1.0)
                    .with("lvidd_cm", 5.0)
                    .with("pwd_cm", pw)
                    .with("bsa_m2", 2.0)
                    .with_choice("sex", sex),
            )
        };
        let o = i(1.0, "female");
        assert!(
            o.findings
                .last()
                .unwrap()
                .criterion
                .starts_with("LVMi ≤ ULN, RWT ≤ 0.42")
        );
        // mass 182 g: female 163-186 mild; male normal (88-224)
        assert_eq!(o.findings[1].severity, Severity::Mild);
        assert_eq!(i(1.0, "male").findings[1].severity, Severity::Normal);
        let o = i(1.2, "female");
        assert!(
            o.findings
                .last()
                .unwrap()
                .criterion
                .starts_with("LVMi > ULN, RWT > 0.42"),
            "{:?}",
            o.findings.last()
        );
        // the LV mass index boundary: 95 g/m² for women is normal, 96 mild
        let row = MASS_I_ROW.scale(false);
        assert_eq!(row.class(95.0).0, Severity::Normal);
        assert_eq!(row.class(95.5).0, Severity::Mild);
        assert_eq!(row.class(121.0).0, Severity::Moderate);
        assert_eq!(row.class(121.5).0, Severity::Severe);
        let row = MASS_I_ROW.scale(true);
        assert_eq!(row.class(115.0).0, Severity::Normal);
        assert_eq!(row.class(116.0).0, Severity::Mild);
        assert_eq!(row.class(148.0).0, Severity::Moderate);
        assert_eq!(row.class(149.0).0, Severity::Severe);
    }

    #[test]
    fn lv_mass_hand_case_and_rwt_cut_off() {
        // (1.0 + 5.0 + 1.0)³ − 5³ = 343 − 125 = 218; × 0.832 = 181.4; + 0.6 = 182.0 g; RWT 2 × 1 ÷ 5 = 0.40
        let o = ok(
            &LV_MASS,
            &Inputs::new()
                .with("ivsd_cm", 1.0)
                .with("lvidd_cm", 5.0)
                .with("pwd_cm", 1.0)
                .with("bsa_m2", 2.0),
        );
        close(o.value("lv_mass_g"), 181.976, 1e-3);
        close(o.value("lv_mass_index_g_m2"), 90.988, 1e-3);
        close(o.value("rwt"), 0.40, 1e-12);
        assert!(o.findings[0].criterion.contains("≤ 0.42"));
        // 0.42 exactly is on the eccentric side; 0.43 concentric
        let at = |pw: f64| {
            ok(
                &LV_MASS,
                &Inputs::new()
                    .with("ivsd_cm", 1.0)
                    .with("lvidd_cm", 5.0)
                    .with("pwd_cm", pw),
            )
            .findings[0]
                .criterion
                .clone()
        };
        assert_eq!(at(1.05), "RWT ≤ 0.42");
        assert_eq!(at(1.075), "RWT > 0.42");
    }

    #[test]
    fn gls_relative_change_boundaries() {
        // (20 − 17) ÷ 20 = 15 % → meets; signs ignored when equal
        let run = |b: f64, c: f64| {
            ok(
                &GLS_CHANGE,
                &Inputs::new()
                    .with("gls_baseline_percent", b)
                    .with("gls_current_percent", c),
            )
        };
        let o = run(-20.0, -17.0);
        close(o.value("relative_reduction_percent"), 15.0, 1e-9);
        assert_eq!(severities(&o), [Severity::Moderate]);
        assert_eq!(severities(&run(-20.0, -17.02)), [Severity::Info]);
        assert_eq!(severities(&run(20.0, 18.0)), [Severity::Info]);
        assert_eq!(severities(&run(20.0, 18.02)), [Severity::Normal]);
        inconsistent(
            &GLS_CHANGE,
            &Inputs::new()
                .with("gls_baseline_percent", -20.0)
                .with("gls_current_percent", 18.0),
            "gls_current_percent",
        );
    }

    #[test]
    fn lvot_flow_hand_case() {
        // π × 1² × 20 = 62.8 mL; × 70 = 4.40 L/min; ÷ 2 = 31.4 mL/m² (< 35), CI 2.20
        let i = Inputs::new()
            .with("lvot_d_cm", 2.0)
            .with("lvot_vti_cm", 20.0)
            .with("hr_bpm", 70.0)
            .with("bsa_m2", 2.0);
        let o = ok(&LVOT_STROKE_VOLUME, &i);
        close(o.value("sv_ml"), 62.83, 0.01);
        close(o.value("co_l_min"), 4.398, 1e-3);
        close(o.value("svi_ml_m2"), 31.42, 0.01);
        close(o.value("ci_l_min_m2"), 2.199, 1e-3);
        assert_eq!(severities(&o), [Severity::Info]);
        assert_eq!(LVOT_STROKE_VOLUME.sim_link, Some(SimLink::HeartRate("hr_bpm")));
        // SVi 35 exactly is not low flow
        let o = ok(
            &LVOT_STROKE_VOLUME,
            &Inputs::new()
                .with("lvot_d_cm", 2.0)
                .with("lvot_vti_cm", 70.0 / std::f64::consts::PI)
                .with("bsa_m2", 2.0),
        );
        assert_eq!(severities(&o), [Severity::Normal]);
    }

    #[test]
    fn wmsi_hand_case() {
        // 16 segments: 12 × 1 + 2 × 2 + 2 × 3 = 22 ÷ 16 = 1.375; s17 ignored in the 16-segment model
        let mut i = Inputs::new().with_choice("model", "16").with_choice("s17", "4");
        for (k, s) in SEGMENTS[..16].iter().enumerate() {
            let score = match k {
                0 | 1 => "2",
                2 | 3 => "3",
                _ => "1",
            };
            i = i.with_choice(s, score);
        }
        let o = ok(&WALL_MOTION_SCORE, &i);
        close(o.value("wmsi"), 1.375, 1e-12);
        close(o.value("segments_scored"), 16.0, 0.0);
        // unseen segments are left out of the mean
        let o = ok(
            &WALL_MOTION_SCORE,
            &Inputs::new()
                .with_choice("model", "17")
                .with_choice("s17", "3")
                .with_choice("s01", "1"),
        );
        close(o.value("wmsi"), 2.0, 1e-12);
        inconsistent(
            &WALL_MOTION_SCORE,
            &Inputs::new().with_choice("model", "16"),
            "s01",
        );
    }
}
