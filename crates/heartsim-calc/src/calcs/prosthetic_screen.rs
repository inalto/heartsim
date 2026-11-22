//! Prosthetic valves by the 2024 ASE guideline (Zoghbi et al.): screening of an aortic prosthesis
//! with a raised jet velocity (Figure 13, Table 5), the serial comparison with the post-implant
//! baseline (Table 5 TAVI rows, Table 6), and screening of a mitral prosthesis (Tables 11 and 12),
//! with patient-prosthesis mismatch by the indexed EOA (Table 7).
//!
//! Table values are copied row by row; a value in a gap between two printed limits goes to the
//! more abnormal column, as everywhere in this crate.

use super::{check, choice, cite, n, need_one, opt, opt_choice, outcome, q, v};
use crate::grade::{Scale, band, criterion_met};
use crate::model::ExampleValue::{Choice as C, Number as N};
use crate::model::{
    Calculator, Category, Choice, Cite, Finding, InputError, InputSpec, Inputs, Outcome, Review, Severity,
    Text, Unit,
};
use crate::refs::PV_2024;
use crate::tree::{self, Cmp, Fact, Leaf, LeafKind, Node, Tree, Walk, answers, test};
use crate::tx;

const fn pv(locator: &'static str) -> Cite {
    cite(&PV_2024, locator)
}

const fn node(id: &'static str, question: Text, locator: &'static str, uses: &'static [usize]) -> Node {
    Node {
        id,
        question,
        cite: pv(locator),
        uses,
    }
}

const fn leaf(
    id: &'static str,
    kind: LeafKind,
    severity: Severity,
    text: Text,
    rule: &'static str,
    locator: &'static str,
) -> Leaf {
    Leaf {
        id,
        kind,
        severity,
        text,
        rule,
        cite: pv(locator),
    }
}

const NORMAL: Text = tx!("normal", "normale");
const POSSIBLE: Text = tx!("possible stenosis", "possibile stenosi");
const SIGNIFICANT: Text = tx!(
    "suggests significant stenosis",
    "suggerisce stenosi significativa"
);
const T5: Cite = pv("Table 5, p. 19");
const T7: Cite = pv("Table 7, p. 20");
const T11: Cite = pv("Table 11, p. 27");

const BMI_CLASS: &[Choice] = &[
    Choice {
        id: "under-30",
        label: tx!("BMI below 30 kg/m²", "BMI sotto 30 kg/m²"),
    },
    Choice {
        id: "30-or-more",
        label: tx!("BMI 30 kg/m² or more", "BMI 30 kg/m² o più"),
    },
];

/// Table 7: patient-prosthesis mismatch by the indexed EOA, with the cut-offs for obesity.
fn ppm_scale(mitral: bool, obese: bool) -> Scale {
    Scale {
        what: tx!("Indexed EOA", "EOA indicizzata"),
        symbol: "iEOA",
        unit: Unit::Cm2M2,
        decimals: 2,
        bands: match (mitral, obese) {
            (false, false) => &PPM_BANDS_A,
            (false, true) => &PPM_BANDS_A_OBESE,
            (true, false) => &PPM_BANDS_M,
            (true, true) => &PPM_BANDS_M_OBESE,
        },
        above: (Severity::Normal, tx!("no mismatch", "nessun mismatch")),
        source: T7,
    }
}

const PPM_SEVERE: Text = tx!("severe mismatch", "mismatch grave");
const PPM_MODERATE: Text = tx!("moderate mismatch", "mismatch moderato");
static PPM_BANDS_A: [crate::grade::Band; 2] = [
    band(0.65, true, Severity::Severe, PPM_SEVERE),
    band(0.85, true, Severity::Moderate, PPM_MODERATE),
];
static PPM_BANDS_A_OBESE: [crate::grade::Band; 2] = [
    band(0.55, true, Severity::Severe, PPM_SEVERE),
    band(0.70, true, Severity::Moderate, PPM_MODERATE),
];
static PPM_BANDS_M: [crate::grade::Band; 2] = [
    band(0.90, true, Severity::Severe, PPM_SEVERE),
    band(1.2, true, Severity::Moderate, PPM_MODERATE),
];
static PPM_BANDS_M_OBESE: [crate::grade::Band; 2] = [
    band(0.75, true, Severity::Severe, PPM_SEVERE),
    band(1.0, true, Severity::Moderate, PPM_MODERATE),
];

/// The Table 7 finding, if the indexed EOA and the BMI class were given.
fn ppm(out: &mut Outcome, i: &Inputs, mitral: bool) {
    if let (Some(ieoa), Some(bmi)) = (i.num("ieoa_cm2_m2"), i.choice("bmi_class")) {
        out.findings
            .push(ppm_scale(mitral, bmi == "30-or-more").finding(ieoa));
    }
}

const IEOA: InputSpec = opt(
    "ieoa_cm2_m2",
    tx!("Indexed EOA", "EOA indicizzata"),
    Unit::Cm2M2,
    0.2,
    3.0,
    2,
);
const BMI: InputSpec = opt_choice(
    "bmi_class",
    tx!("Body mass index", "Indice di massa corporea"),
    BMI_CLASS,
);

// =============================================================================================
// Aortic prosthesis: Figure 13 and Table 5

static AT_SCALE: Scale = Scale {
    what: tx!("Acceleration time", "Tempo di accelerazione"),
    symbol: "AT",
    unit: Unit::Ms,
    decimals: 0,
    bands: &[
        band(80.0, false, Severity::Normal, NORMAL),
        band(100.0, true, Severity::Mild, POSSIBLE),
    ],
    above: (Severity::Severe, SIGNIFICANT),
    source: T5,
};
static AT_ET_SCALE: Scale = Scale {
    what: tx!("AT/ET ratio", "Rapporto AT/ET"),
    symbol: "AT/ET",
    unit: Unit::None,
    decimals: 2,
    bands: &[
        band(0.32, false, Severity::Normal, NORMAL),
        band(0.37, true, Severity::Mild, POSSIBLE),
    ],
    above: (Severity::Severe, SIGNIFICANT),
    source: T5,
};
static AV_VMAX_SCALE: Scale = Scale {
    what: tx!("Peak velocity", "Velocità massima"),
    symbol: "Vmax",
    unit: Unit::MS,
    decimals: 1,
    bands: &[
        band(3.0, false, Severity::Normal, NORMAL),
        band(4.0, false, Severity::Mild, POSSIBLE),
    ],
    above: (Severity::Severe, SIGNIFICANT),
    source: T5,
};
static SAVR_MEAN_SCALE: Scale = Scale {
    what: tx!(
        "Mean gradient (surgical valve)",
        "Gradiente medio (protesi chirurgica)"
    ),
    symbol: "mean gradient",
    unit: Unit::MmHg,
    decimals: 0,
    bands: &[
        band(20.0, false, Severity::Normal, NORMAL),
        band(34.0, true, Severity::Mild, POSSIBLE),
    ],
    above: (Severity::Severe, SIGNIFICANT),
    source: T5,
};
static SAVR_DVI_SCALE: Scale = Scale {
    what: tx!("DVI (surgical valve)", "DVI (protesi chirurgica)"),
    symbol: "DVI",
    unit: Unit::None,
    decimals: 2,
    bands: &[
        band(0.25, false, Severity::Severe, SIGNIFICANT),
        band(0.35, true, Severity::Mild, POSSIBLE),
    ],
    above: (Severity::Normal, NORMAL),
    source: T5,
};

const EOA_REFERENCE: &[Choice] = &[
    Choice {
        id: "within-1sd",
        label: tx!(
            "within 1 SD of the reference EOA",
            "entro 1 DS dall'EOA di riferimento"
        ),
    },
    Choice {
        id: "1sd-smaller",
        label: tx!(
            "1 SD smaller than the reference",
            "1 DS più piccola del riferimento"
        ),
    },
    Choice {
        id: "2sd-smaller",
        label: tx!(
            "2 SD smaller than the reference",
            "2 DS più piccola del riferimento"
        ),
    },
];

const IMPLANT: &[Choice] = &[
    Choice {
        id: "surgical",
        label: tx!("surgical (SAVR)", "chirurgica (SAVR)"),
    },
    Choice {
        id: "transcatheter",
        label: tx!("transcatheter (TAVI)", "transcatetere (TAVI)"),
    },
];

pub(crate) mod aortic {
    #[allow(clippy::wildcard_imports)]
    use super::*;

    const F13: &str = "Figure 13, p. 19";

    pub static VELOCITY: Node = node(
        "velocity",
        tx!(
            "Peak prosthetic jet velocity above 3 m/s",
            "Velocità massima del getto protesico sopra 3 m/s"
        ),
        F13,
        &[0],
    );
    pub static CONTOUR: Node = node(
        "contour",
        tx!("Jet contour by AT and AT/ET", "Profilo del getto da AT e AT/ET"),
        F13,
        &[1],
    );
    pub static DVI: Node = node("dvi", tx!("DVI", "DVI"), F13, &[2]);

    pub static NOT_ENTERED: Leaf = leaf(
        "not-entered",
        LeafKind::OutOfScope,
        Severity::Info,
        tx!(
            "the figure starts from a peak velocity above 3 m/s; see the Table 5 rows for this velocity",
            "la figura parte da una velocità massima sopra 3 m/s; vedere le righe della Tabella 5 per questa velocità"
        ),
        "Vmax ≤ 3 m/s",
        F13,
    );
    pub static DISCORDANT: Leaf = leaf(
        "discordant",
        LeafKind::Indeterminate,
        Severity::Indeterminate,
        tx!(
            "AT and AT/ET point to different jet contours, so the figure's branch cannot be chosen",
            "AT e AT/ET indicano profili del getto diversi, quindi il ramo della figura non si può scegliere"
        ),
        "early by one of AT and AT/ET, late by the other",
        F13,
    );
    pub static NORMAL_AVR: Leaf = leaf(
        "normal",
        LeafKind::Result,
        Severity::Normal,
        tx!(
            "meets the figure's criteria for a normal prosthesis; the figure then reads the indexed EOA: above 0.85 cm²/m² a high-flow state, below 0.85 patient-prosthesis mismatch",
            "soddisfa i criteri della figura per protesi normale; la figura legge poi l'EOA indicizzata: sopra 0,85 cm²/m² stato di alto flusso, sotto 0,85 mismatch protesi-paziente"
        ),
        "early peak, DVI ≥ 0.30",
        F13,
    );
    pub static POSSIBLE_STENOSIS: Leaf = leaf(
        "possible",
        LeafKind::Result,
        Severity::Mild,
        tx!(
            "meets the figure's criteria for possible stenosis (consider stenosis with a small LVOT, a misplaced LVOT sample, an underestimated jet velocity)",
            "soddisfa i criteri della figura per possibile stenosi (considerare stenosi con LVOT piccolo, campione LVOT mal posizionato, velocità del getto sottostimata)"
        ),
        "DVI 0.25-0.29",
        F13,
    );
    pub static STENOSIS: Leaf = leaf(
        "stenosis",
        LeafKind::Result,
        Severity::Severe,
        tx!(
            "meets the figure's criteria for stenosis (consider valvular stenosis including thrombosis, or subvalvular stenosis)",
            "soddisfa i criteri della figura per stenosi (considerare stenosi valvolare, trombosi compresa, o sottovalvolare)"
        ),
        "late peak, DVI < 0.25",
        F13,
    );
    pub static UNDRAWN: Leaf = leaf(
        "undrawn",
        LeafKind::OutOfScope,
        Severity::Indeterminate,
        tx!(
            "the figure draws no branch for this combination of jet contour and DVI",
            "la figura non disegna un ramo per questa combinazione di profilo del getto e DVI"
        ),
        "early peak with DVI < 0.25, or late peak with DVI ≥ 0.30",
        F13,
    );
    pub static OPEN: Leaf = leaf(
        "open",
        LeafKind::Indeterminate,
        Severity::Indeterminate,
        tx!(
            "indeterminate with these measurements",
            "indeterminato con queste misure"
        ),
        "measurements left empty change the result",
        F13,
    );

    pub fn walk(f: &[u8]) -> Walk {
        let mut w = Walk::new(&OPEN);
        if f[0] == 0 {
            w.ask(&VELOCITY, answers::NO);
            return w.end(&NOT_ENTERED);
        }
        w.ask(&VELOCITY, answers::YES);
        let late = match f[1] {
            0 => {
                w.ask(&CONTOUR, tx!("early peak", "picco precoce"));
                false
            }
            1 => {
                w.ask(&CONTOUR, tx!("late peak", "picco tardivo"));
                true
            }
            _ => {
                w.ask(&CONTOUR, tx!("AT and AT/ET disagree", "AT e AT/ET discordanti"));
                return w.end(&DISCORDANT);
            }
        };
        match (late, f[2]) {
            (false, 0) => {
                w.ask(&DVI, tx!("≥ 0.30", "≥ 0,30"));
                w.end(&NORMAL_AVR)
            }
            (_, 1) => {
                w.ask(&DVI, tx!("0.25-0.29", "0,25-0,29"));
                w.end(&POSSIBLE_STENOSIS)
            }
            (true, 2) => {
                w.ask(&DVI, tx!("< 0.25", "< 0,25"));
                w.end(&STENOSIS)
            }
            (true, _) => {
                w.ask(&DVI, tx!("≥ 0.30", "≥ 0,30"));
                w.end(&UNDRAWN)
            }
            (false, _) => {
                w.ask(&DVI, tx!("< 0.25", "< 0,25"));
                w.end(&UNDRAWN)
            }
        }
    }
}

/// Figure 13.
pub static AORTIC_TREE: Tree = Tree {
    calculator: "aortic-prosthesis-screening",
    nodes: &[&aortic::VELOCITY, &aortic::CONTOUR, &aortic::DVI],
    leaves: &[
        &aortic::NOT_ENTERED,
        &aortic::DISCORDANT,
        &aortic::NORMAL_AVR,
        &aortic::POSSIBLE_STENOSIS,
        &aortic::STENOSIS,
        &aortic::UNDRAWN,
    ],
    open: &aortic::OPEN,
    arities: &[2, 3, 3],
    walk: aortic::walk,
};

fn aortic_screening(i: &Inputs) -> Result<Outcome, Vec<InputError>> {
    let vmax = i.n("vmax_m_s");
    let (at, et) = (i.num("at_ms"), i.num("et_ms"));
    if let (Some(at), Some(et)) = (at, et) {
        check(
            at >= et,
            "at_ms",
            tx!(
                "The acceleration time must be shorter than the ejection time",
                "Il tempo di accelerazione deve essere più breve del tempo di eiezione"
            ),
        )?;
    }
    let mut out = outcome();
    out.push(
        q(
            "peak_gradient_mmhg",
            tx!("Peak gradient", "Gradiente massimo"),
            crate::blocks::bernoulli_mmhg(vmax),
            Unit::MmHg,
            0,
        ),
        "ΔP = 4v²",
        format!("4 × {}²", n(vmax, 2)),
    );
    let ratio = at.zip(et).map(|(a, e)| a / e);
    if let (Some(a), Some(e), Some(r)) = (at, et, ratio) {
        out.push(
            q("at_et_ratio", tx!("AT/ET", "AT/ET"), r, Unit::None, 2),
            "AT ÷ ET",
            format!("{} ÷ {}", v(a, 0, Unit::Ms), v(e, 0, Unit::Ms)),
        );
    }
    // Table 5, rows for every prosthetic aortic valve
    out.findings.push(AV_VMAX_SCALE.finding(vmax));
    if let Some(a) = at {
        out.findings.push(AT_SCALE.finding(a));
    }
    if let Some(r) = ratio {
        out.findings.push(AT_ET_SCALE.finding(r));
    }
    let dvi = i.num("dvi");
    // rows for surgical valves only
    if i.choice("implant") == Some("surgical") {
        if let Some(m) = i.num("mean_gradient_mmhg") {
            out.findings.push(SAVR_MEAN_SCALE.finding(m));
        }
        if let Some(d) = dvi {
            out.findings.push(SAVR_DVI_SCALE.finding(d));
        }
        if let Some(r) = i.choice("eoa_vs_reference") {
            let (sev, label, crit) = match r {
                "within-1sd" => (Severity::Normal, NORMAL, "EOA = reference ± 1 SD"),
                "1sd-smaller" => (Severity::Mild, POSSIBLE, "EOA 1 SD smaller than reference"),
                _ => (Severity::Severe, SIGNIFICANT, "EOA 2 SD smaller than reference"),
            };
            out.findings.push(Finding {
                severity: sev,
                text: crate::model::Message {
                    en: format!("EOA in the column the source labels “{}”", label.en),
                    it: format!("EOA nella colonna che la fonte chiama “{}”", label.it),
                },
                criterion: crit.to_owned(),
                source: T5,
            });
        }
    } else {
        out.findings.push(criterion_met(
            Severity::Info,
            tx!(
                "After TAVI, Table 5 grades the mean gradient, DVI and EOA as changes from the baseline study: see the serial comparison",
                "Dopo TAVI, la Tabella 5 gradua gradiente medio, DVI ed EOA come variazioni rispetto all'esame basale: vedere il confronto seriale"
            ),
            "TAVI: change from baseline",
            T5,
        ));
    }
    ppm(&mut out, i, false);
    // Figure 13
    let at_t = test("AT", Cmp::Ge, 100.0, Unit::Ms, 0);
    let ratio_t = test("AT/ET", Cmp::Ge, 0.37, Unit::None, 2);
    let late = [at_t.eval(at), ratio_t.eval(ratio)];
    let contour = match late {
        [None, None] => None,
        [Some(a), Some(b)] if a != b => Some(2),
        [Some(x), _] | [None, Some(x)] => Some(u8::from(x)),
    };
    let facts = [
        Fact::of(&test("Vmax", Cmp::Gt, 3.0, Unit::MS, 1), Some(vmax)),
        Fact::class(contour, 3, tree::show_all(&[(at_t, at), (ratio_t, ratio)])),
        {
            let shown = match dvi {
                Some(d) => format!("DVI = {}", n(d, 2)),
                None => "DVI = ?".to_owned(),
            };
            let class = dvi.map(|d| {
                if Cmp::Ge.holds(d, 0.30) {
                    0
                } else if Cmp::Ge.holds(d, 0.25) {
                    1
                } else {
                    2
                }
            });
            Fact::class(class, 3, shown)
        },
    ];
    out.findings.extend(tree::decide(&AORTIC_TREE, &facts).findings);
    Ok(out)
}

/// Screening of an aortic prosthesis with a raised jet velocity (Figure 13, Table 5, Table 7).
pub static AORTIC_SCREENING: Calculator = Calculator {
    id: "aortic-prosthesis-screening",
    category: Category::Prosthetic,
    name: tx!(
        "Aortic prosthesis: stenosis and mismatch screening",
        "Protesi aortica: screening di stenosi e mismatch"
    ),
    synonyms: tx!(
        "prosthetic aortic valve, SAVR, TAVI, obstruction, stenosis, DVI, acceleration time, AT/ET, jet contour, patient-prosthesis mismatch, PPM",
        "protesi aortica, SAVR, TAVI, ostruzione, stenosi, DVI, tempo di accelerazione, AT/ET, profilo del getto, mismatch protesi-paziente, PPM"
    ),
    summary: tx!(
        "Follows the 2024 ASE algorithm for a prosthetic aortic jet faster than 3 m/s (jet contour by acceleration time and AT/ET, then the DVI), shows the Table 5 column of each parameter (mean gradient, DVI and EOA rows for surgical valves only) and grades mismatch by the indexed EOA (Table 7). Significant stenosis should meet at least one flow-dependent and one flow-independent criterion.",
        "Segue l'algoritmo ASE 2024 per un getto protesico aortico oltre 3 m/s (profilo del getto da tempo di accelerazione e AT/ET, poi il DVI), mostra la colonna della Tabella 5 di ogni parametro (righe di gradiente medio, DVI ed EOA solo per protesi chirurgiche) e gradua il mismatch dall'EOA indicizzata (Tabella 7). Una stenosi significativa dovrebbe soddisfare almeno un criterio dipendente dal flusso e uno indipendente."
    ),
    inputs: &[
        choice("implant", tx!("Prosthesis", "Protesi"), IMPLANT),
        super::num(
            "vmax_m_s",
            tx!(
                "Peak prosthetic jet velocity",
                "Velocità massima del getto protesico"
            ),
            Unit::MS,
            0.5,
            7.0,
            1,
        ),
        opt(
            "at_ms",
            tx!("Acceleration time (AT)", "Tempo di accelerazione (AT)"),
            Unit::Ms,
            20.0,
            250.0,
            0,
        ),
        opt(
            "et_ms",
            tx!("LV ejection time (ET)", "Tempo di eiezione del VS (ET)"),
            Unit::Ms,
            100.0,
            500.0,
            0,
        ),
        opt(
            "dvi",
            tx!("DVI (VTI LVOT ÷ VTI prosthesis)", "DVI (VTI LVOT ÷ VTI protesi)"),
            Unit::None,
            0.05,
            1.5,
            2,
        ),
        opt(
            "mean_gradient_mmhg",
            tx!("Mean gradient", "Gradiente medio"),
            Unit::MmHg,
            1.0,
            150.0,
            0,
        ),
        opt_choice(
            "eoa_vs_reference",
            tx!(
                "EOA against the reference for this valve type and size",
                "EOA rispetto al riferimento per tipo e misura della protesi"
            ),
            EOA_REFERENCE,
        ),
        IEOA,
        BMI,
    ],
    compute: aortic_screening,
    references: &[
        cite(&PV_2024, "Figure 13 and Table 5, p. 19"),
        cite(&PV_2024, "Table 7 (mismatch), p. 20"),
    ],
    review: Review::Verified,
    example: &[
        ("implant", C("surgical")),
        ("vmax_m_s", N(3.6)),
        ("at_ms", N(115.0)),
        ("et_ms", N(290.0)),
        ("dvi", N(0.23)),
        ("mean_gradient_mmhg", N(38.0)),
    ],
    sim_link: None,
};

// =============================================================================================
// Serial comparison with the post-implant baseline: Table 5 (TAVI rows) and Table 6

/// A decrease from a baseline, graded by the larger of the absolute and the relative column.
fn decrease_class(base: f64, now: f64, abs: (f64, f64), rel: (f64, f64)) -> u8 {
    let d = base - now;
    let pct = d / base * 100.0;
    // the significant limit is above the possible one, so each limit passed adds a column
    let by = |x: f64, (possible, significant): (f64, f64)| {
        u8::from(Cmp::Ge.holds(x, possible)) + u8::from(Cmp::Ge.holds(x, significant))
    };
    by(d, abs).max(by(pct, rel))
}

const COLUMNS: [(Severity, Text); 3] = [
    (Severity::Normal, NORMAL),
    (Severity::Mild, POSSIBLE),
    (Severity::Severe, SIGNIFICANT),
];

fn column_finding(what: Text, class: u8, criterion: String) -> Finding {
    let (sev, label) = COLUMNS[usize::from(class.min(2))];
    Finding {
        severity: sev,
        text: crate::model::Message {
            en: format!("{} in the TAVI column the source labels “{}”", what.en, label.en),
            it: format!(
                "{} nella colonna TAVI che la fonte chiama “{}”",
                what.it, label.it
            ),
        },
        criterion,
        source: T5,
    }
}

const AR_CHANGE: &[Choice] = &[
    Choice {
        id: "none",
        label: tx!(
            "no new or increased AR of that size",
            "nessuna IA nuova o aumentata di tale entità"
        ),
    },
    Choice {
        id: "one-grade",
        label: tx!(
            "new, or at least one grade more, reaching moderate or more",
            "nuova, o almeno un grado in più, fino a moderata o più"
        ),
    },
    Choice {
        id: "two-grades",
        label: tx!(
            "at least two grades more, reaching moderate-to-severe or severe",
            "almeno due gradi in più, fino a moderata-grave o grave"
        ),
    },
];

pub(crate) mod serial {
    #[allow(clippy::wildcard_imports)]
    use super::*;

    const T6: &str = "Table 6, p. 20";

    pub static STABLE: Node = node(
        "stable",
        tx!(
            "LV function and blood pressure stable between the two studies",
            "Funzione del VS e pressione arteriosa stabili fra i due esami"
        ),
        T6,
        &[0],
    );
    pub static HAEMODYNAMIC: Node = node(
        "haemodynamic",
        tx!(
            "Rise in mean gradient with a concomitant fall in EOA and/or DVI",
            "Aumento del gradiente medio con concomitante calo di EOA e/o DVI"
        ),
        T6,
        &[1, 2],
    );
    pub static AR: Node = node(
        "ar",
        tx!(
            "New or increased intraprosthetic AR",
            "IA intraprotesica nuova o aumentata"
        ),
        T6,
        &[3],
    );
    pub static NONE: Leaf = leaf(
        "none",
        LeafKind::Result,
        Severity::Normal,
        tx!(
            "meets no criterion of the table for structural valve deterioration",
            "non soddisfa alcun criterio della tabella per deterioramento strutturale"
        ),
        "neither row met",
        T6,
    );
    pub static POSSIBLE_SVD: Leaf = leaf(
        "possible",
        LeafKind::Result,
        Severity::Mild,
        tx!(
            "meets the table's criteria for possible structural valve deterioration (morphological changes of the prosthesis should also be evident)",
            "soddisfa i criteri della tabella per possibile deterioramento strutturale (dovrebbero essere evidenti anche alterazioni morfologiche della protesi)"
        ),
        "mean gradient +10 to ≥ 20 mmHg with EOA −0.3 or −25% and/or DVI −0.1 or −20%; or AR up one grade to ≥ moderate",
        T6,
    );
    pub static SIGNIFICANT_SVD: Leaf = leaf(
        "significant",
        LeafKind::Result,
        Severity::Severe,
        tx!(
            "meets the table's criteria for significant structural valve deterioration (morphological changes of the prosthesis should also be evident)",
            "soddisfa i criteri della tabella per deterioramento strutturale significativo (dovrebbero essere evidenti anche alterazioni morfologiche della protesi)"
        ),
        "mean gradient +20 to ≥ 30 mmHg with EOA −0.6 or −50% and/or DVI −0.2 or −40%; or AR up two grades",
        T6,
    );
    pub static NOT_COMPARABLE: Leaf = leaf(
        "not-comparable",
        LeafKind::Indeterminate,
        Severity::Indeterminate,
        tx!(
            "the table's criteria assume stable LV function and blood pressure, so they cannot be applied",
            "i criteri della tabella presuppongono funzione del VS e pressione arteriosa stabili, quindi non si possono applicare"
        ),
        "conditions not stable",
        T6,
    );
    pub static OPEN: Leaf = leaf(
        "open",
        LeafKind::Indeterminate,
        Severity::Indeterminate,
        tx!(
            "indeterminate with these measurements",
            "indeterminato con queste misure"
        ),
        "measurements left empty change the result",
        T6,
    );

    pub fn walk(f: &[u8]) -> Walk {
        let mut w = Walk::new(&OPEN);
        if f[0] == 0 {
            w.ask(&STABLE, answers::NO);
            return w.end(&NOT_COMPARABLE);
        }
        w.ask(&STABLE, answers::YES);
        let haemo = f[1].min(f[2]);
        w.ask(
            &HAEMODYNAMIC,
            match haemo {
                0 => tx!("no criterion met", "nessun criterio soddisfatto"),
                1 => tx!(
                    "criteria for possible deterioration",
                    "criteri per possibile deterioramento"
                ),
                _ => tx!(
                    "criteria for significant deterioration",
                    "criteri per deterioramento significativo"
                ),
            },
        );
        w.ask(
            &AR,
            match f[3] {
                0 => tx!("no", "no"),
                1 => tx!(
                    "at least one grade, to moderate or more",
                    "almeno un grado, fino a moderata o più"
                ),
                _ => tx!("at least two grades", "almeno due gradi"),
            },
        );
        w.end(match haemo.max(f[3]) {
            0 => &NONE,
            1 => &POSSIBLE_SVD,
            _ => &SIGNIFICANT_SVD,
        })
    }
}

/// Table 6.
pub static SERIAL_TREE: Tree = Tree {
    calculator: "aortic-prosthesis-serial",
    nodes: &[&serial::STABLE, &serial::HAEMODYNAMIC, &serial::AR],
    leaves: &[
        &serial::NONE,
        &serial::POSSIBLE_SVD,
        &serial::SIGNIFICANT_SVD,
        &serial::NOT_COMPARABLE,
    ],
    open: &serial::OPEN,
    arities: &[2, 3, 3, 3],
    walk: serial::walk,
};

fn pair(i: &Inputs, base: &'static str, now: &'static str) -> Result<Option<(f64, f64)>, Vec<InputError>> {
    match (i.num(base), i.num(now)) {
        (Some(b), Some(c)) => Ok(Some((b, c))),
        (None, None) => Ok(None),
        (Some(_), None) => Err(vec![InputError::inconsistent(
            now,
            tx!(
                "Enter the baseline and the current value",
                "Inserire il valore basale e quello attuale"
            ),
        )]),
        (None, Some(_)) => Err(vec![InputError::inconsistent(
            base,
            tx!(
                "Enter the baseline and the current value",
                "Inserire il valore basale e quello attuale"
            ),
        )]),
    }
}

fn serial_comparison(i: &Inputs) -> Result<Outcome, Vec<InputError>> {
    let (m0, m1) = (i.n("mean_baseline_mmhg"), i.n("mean_current_mmhg"));
    let dvi = pair(i, "dvi_baseline", "dvi_current")?;
    let eoa = pair(i, "eoa_baseline_cm2", "eoa_current_cm2")?;
    let mut out = outcome();
    let rise = m1 - m0;
    out.push(
        q(
            "gradient_change_mmhg",
            tx!("Change in mean gradient", "Variazione del gradiente medio"),
            rise,
            Unit::MmHg,
            0,
        ),
        "Δ mean = current − baseline",
        format!("{} − {}", n(m1, 0), n(m0, 0)),
    );
    // EOA and/or DVI: the larger column of those measured
    let mut flow_independent: Option<u8> = None;
    let mut shown = Vec::new();
    if let Some((b, c)) = dvi {
        out.push(
            q(
                "dvi_fall_percent",
                tx!("Fall in DVI", "Calo del DVI"),
                (b - c) / b * 100.0,
                Unit::Percent,
                0,
            ),
            "(DVI₀ − DVI) ÷ DVI₀ × 100",
            format!("({} − {}) ÷ {} × 100", n(b, 2), n(c, 2), n(b, 2)),
        );
        let t5 = decrease_class(b, c, (0.1, 0.2), (20.0, 40.0));
        out.findings.push(column_finding(
            tx!("Fall in DVI", "Calo del DVI"),
            t5,
            format!("DVI baseline {}, now {}", n(b, 2), n(c, 2)),
        ));
        flow_independent = Some(flow_independent.map_or(t5, |c| c.max(t5)));
        shown.push(format!("DVI baseline {}, now {}", n(b, 2), n(c, 2)));
    }
    if let Some((b, c)) = eoa {
        out.push(
            q(
                "eoa_fall_percent",
                tx!("Fall in EOA", "Calo dell'EOA"),
                (b - c) / b * 100.0,
                Unit::Percent,
                0,
            ),
            "(EOA₀ − EOA) ÷ EOA₀ × 100",
            format!("({} − {}) ÷ {} × 100", n(b, 2), n(c, 2), n(b, 2)),
        );
        let t5 = decrease_class(b, c, (0.3, 0.6), (25.0, 50.0));
        out.findings.push(column_finding(
            tx!("Fall in EOA", "Calo dell'EOA"),
            t5,
            format!("EOA baseline {}, now {} cm²", n(b, 2), n(c, 2)),
        ));
        flow_independent = Some(flow_independent.map_or(t5, |c| c.max(t5)));
        shown.push(format!("EOA baseline {}, now {} cm²", n(b, 2), n(c, 2)));
    }
    // Table 5, TAVI: the rise in mean gradient
    let t5_gradient = u8::from(Cmp::Ge.holds(rise, 10.0)) + u8::from(Cmp::Ge.holds(rise, 20.0));
    out.findings.insert(
        0,
        column_finding(
            tx!("Rise in mean gradient", "Aumento del gradiente medio"),
            t5_gradient,
            format!("mean gradient baseline {}, now {} mmHg", n(m0, 0), n(m1, 0)),
        ),
    );
    // Table 6: the same changes, with the gradient they reach
    // the significant criterion implies the possible one
    let gradient_class = u8::from(Cmp::Ge.holds(rise, 10.0) && Cmp::Ge.holds(m1, 20.0))
        + u8::from(Cmp::Ge.holds(rise, 20.0) && Cmp::Ge.holds(m1, 30.0));
    let stable = i.choice("stable_conditions").map(|c| c == "yes");
    let ar = match i.choice("ar_change") {
        Some("two-grades") => 2,
        Some("one-grade") => 1,
        _ => 0,
    };
    let facts = [
        Fact::flag(
            stable,
            format!(
                "stable conditions = {}",
                i.choice("stable_conditions").unwrap_or("?")
            ),
        ),
        Fact::class(
            Some(gradient_class),
            3,
            format!(
                "mean gradient baseline {}, now {} mmHg (Δ {})",
                n(m0, 0),
                n(m1, 0),
                n(rise, 0)
            ),
        ),
        Fact::class(
            flow_independent,
            3,
            if shown.is_empty() {
                "EOA = ?; DVI = ?".to_owned()
            } else {
                shown.join("; ")
            },
        ),
        Fact::class(
            Some(ar),
            3,
            format!("AR = {}", i.choice("ar_change").unwrap_or("none")),
        ),
    ];
    out.findings.extend(tree::decide(&SERIAL_TREE, &facts).findings);
    Ok(out)
}

/// Serial comparison of an aortic bioprosthesis with its post-implant baseline (Tables 5 and 6).
pub static AORTIC_SERIAL: Calculator = Calculator {
    id: "aortic-prosthesis-serial",
    category: Category::Prosthetic,
    name: tx!(
        "Aortic bioprosthesis: change from baseline",
        "Bioprotesi aortica: variazione dal basale"
    ),
    synonyms: tx!(
        "structural valve deterioration, SVD, TAVI, bioprosthesis, serial, baseline, mean gradient, DVI, EOA",
        "deterioramento strutturale, SVD, TAVI, bioprotesi, seriale, basale, gradiente medio, DVI, EOA"
    ),
    summary: tx!(
        "Compares the current study with the baseline obtained 1-3 months after implantation under stable conditions: the TAVI columns of Table 5 for each change, and the Table 6 criteria for structural valve deterioration (a rise in mean gradient together with a fall in EOA and/or DVI, or new or increased intraprosthetic AR).",
        "Confronta l'esame attuale con il basale ottenuto 1-3 mesi dopo l'impianto in condizioni stabili: le colonne TAVI della Tabella 5 per ogni variazione e i criteri della Tabella 6 per il deterioramento strutturale (aumento del gradiente medio con calo di EOA e/o DVI, o IA intraprotesica nuova o aumentata)."
    ),
    inputs: &[
        choice(
            "stable_conditions",
            tx!(
                "LV function and blood pressure stable",
                "Funzione del VS e pressione arteriosa stabili"
            ),
            YES_NO,
        ),
        super::num(
            "mean_baseline_mmhg",
            tx!("Baseline mean gradient", "Gradiente medio basale"),
            Unit::MmHg,
            1.0,
            100.0,
            0,
        ),
        super::num(
            "mean_current_mmhg",
            tx!("Current mean gradient", "Gradiente medio attuale"),
            Unit::MmHg,
            1.0,
            150.0,
            0,
        ),
        opt(
            "dvi_baseline",
            tx!("Baseline DVI", "DVI basale"),
            Unit::None,
            0.05,
            1.5,
            2,
        ),
        opt(
            "dvi_current",
            tx!("Current DVI", "DVI attuale"),
            Unit::None,
            0.05,
            1.5,
            2,
        ),
        opt(
            "eoa_baseline_cm2",
            tx!("Baseline EOA", "EOA basale"),
            Unit::Cm2,
            0.2,
            5.0,
            2,
        ),
        opt(
            "eoa_current_cm2",
            tx!("Current EOA", "EOA attuale"),
            Unit::Cm2,
            0.2,
            5.0,
            2,
        ),
        choice(
            "ar_change",
            tx!(
                "Intraprosthetic AR since baseline",
                "IA intraprotesica dal basale"
            ),
            AR_CHANGE,
        ),
    ],
    compute: serial_comparison,
    references: &[
        cite(&PV_2024, "Table 5 (TAVI, change from baseline), p. 19"),
        cite(&PV_2024, "Table 6, p. 20"),
    ],
    review: Review::Verified,
    example: &[
        ("stable_conditions", C("yes")),
        ("mean_baseline_mmhg", N(9.0)),
        ("mean_current_mmhg", N(24.0)),
        ("dvi_baseline", N(0.55)),
        ("dvi_current", N(0.40)),
        ("ar_change", C("none")),
    ],
    sim_link: None,
};

const YES_NO: &[Choice] = &[
    Choice {
        id: "yes",
        label: tx!("yes", "sì"),
    },
    Choice {
        id: "no",
        label: tx!("no", "no"),
    },
];

// =============================================================================================
// Mitral prosthesis: Tables 11 and 12

static MV_VMAX_SCALE: Scale = Scale {
    what: tx!("Peak velocity", "Velocità massima"),
    symbol: "Vmax",
    unit: Unit::MS,
    decimals: 1,
    bands: &[
        band(1.9, false, Severity::Normal, NORMAL),
        band(2.5, false, Severity::Mild, POSSIBLE),
    ],
    above: (Severity::Severe, SIGNIFICANT),
    source: T11,
};
static MV_MEAN_SCALE: Scale = Scale {
    what: tx!("Mean gradient", "Gradiente medio"),
    symbol: "mean gradient",
    unit: Unit::MmHg,
    decimals: 0,
    bands: &[
        band(5.0, true, Severity::Normal, NORMAL),
        band(10.0, true, Severity::Mild, POSSIBLE),
    ],
    above: (Severity::Severe, SIGNIFICANT),
    source: T11,
};
static MV_DVI_SCALE: Scale = Scale {
    what: tx!(
        "VTI ratio (prosthesis ÷ LVOT)",
        "Rapporto delle VTI (protesi ÷ LVOT)"
    ),
    symbol: "VTI ratio",
    unit: Unit::None,
    decimals: 1,
    bands: &[
        band(2.2, false, Severity::Normal, NORMAL),
        band(2.5, true, Severity::Mild, POSSIBLE),
    ],
    above: (Severity::Severe, SIGNIFICANT),
    source: T11,
};
static MV_EOA_SCALE: Scale = Scale {
    what: tx!("EOA", "EOA"),
    symbol: "EOA",
    unit: Unit::Cm2,
    decimals: 1,
    bands: &[
        band(1.0, false, Severity::Severe, SIGNIFICANT),
        band(2.0, false, Severity::Mild, POSSIBLE),
    ],
    above: (Severity::Normal, NORMAL),
    source: T11,
};
static MV_PHT_SCALE: Scale = Scale {
    what: tx!("Pressure half-time", "Tempo di dimezzamento della pressione"),
    symbol: "PHT",
    unit: Unit::Ms,
    decimals: 0,
    bands: &[
        band(130.0, false, Severity::Normal, NORMAL),
        band(200.0, true, Severity::Mild, POSSIBLE),
    ],
    above: (Severity::Severe, SIGNIFICANT),
    source: T11,
};

const VALVE_TYPE: &[Choice] = &[
    Choice {
        id: "mechanical",
        label: tx!("mechanical", "meccanica"),
    },
    Choice {
        id: "biological",
        label: tx!("biological", "biologica"),
    },
];

/// The facts of the mitral tree: one Table 11 column per parameter, then the Table 12 pattern.
const MV_PARAMS: usize = 5;

pub(crate) mod mitral {
    #[allow(clippy::wildcard_imports)]
    use super::*;

    const T12: &str = "Table 12 and footnote, p. 28";
    const T11F: &str = "Table 11 and footnotes, p. 27";

    pub static MR_PATTERN: Node = node(
        "mr-pattern",
        tx!(
            "Mechanical valve: peak velocity ≥ 1.9 m/s and VTI ratio ≥ 2.5 with a normal PHT",
            "Protesi meccanica: velocità massima ≥ 1,9 m/s e rapporto delle VTI ≥ 2,5 con PHT normale"
        ),
        T12,
        &[0, 5, 4],
    );
    pub static MAJORITY: Node = node(
        "majority",
        tx!(
            "Column of Table 11 holding most (3 or more) of the five parameters",
            "Colonna della Tabella 11 che contiene la maggior parte (3 o più) dei cinque parametri"
        ),
        T11F,
        &[0, 1, 2, 3, 4],
    );
    pub static MR: Leaf = leaf(
        "mr",
        LeafKind::Result,
        Severity::Abnormal,
        tx!(
            "meets the findings the source gives as suggestive of significant prosthetic MR (both raised with a normal PHT: specificity close to 100%); TEE is often needed",
            "soddisfa i reperti che la fonte indica come suggestivi di IM protesica significativa (entrambi aumentati con PHT normale: specificità vicina al 100%); spesso serve l'ETE"
        ),
        "Vmax ≥ 1.9 m/s, VTI ratio ≥ 2.5, PHT < 130 ms",
        T12,
    );
    pub static NORMAL_COL: Leaf = leaf(
        "normal",
        LeafKind::Result,
        Severity::Normal,
        tx!(
            "most parameters in the table's normal column",
            "la maggior parte dei parametri nella colonna normale della tabella"
        ),
        "≥ 3 of 5 normal",
        T11F,
    );
    pub static SIGNIFICANT_COL: Leaf = leaf(
        "significant",
        LeafKind::Result,
        Severity::Severe,
        tx!(
            "most parameters in the column the table labels “suggests significant stenosis”",
            "la maggior parte dei parametri nella colonna che la tabella chiama “suggerisce stenosi significativa”"
        ),
        "≥ 3 of 5 in the significant column",
        T11F,
    );
    pub static ABNORMAL_COL: Leaf = leaf(
        "possible",
        LeafKind::Result,
        Severity::Mild,
        tx!(
            "most parameters abnormal, fewer than three in the significant column: values that, as the table says, should prompt a closer evaluation (also of high flow, heart rate, mismatch)",
            "la maggior parte dei parametri anormale, meno di tre nella colonna significativa: valori che, come dice la tabella, richiedono una valutazione più attenta (anche di alto flusso, frequenza, mismatch)"
        ),
        "≥ 3 of 5 abnormal, < 3 significant",
        T11F,
    );
    pub static OPEN: Leaf = leaf(
        "open",
        LeafKind::Indeterminate,
        Severity::Indeterminate,
        tx!(
            "indeterminate with these measurements",
            "indeterminato con queste misure"
        ),
        "measurements left empty change the result",
        T11F,
    );

    pub fn walk(f: &[u8]) -> Walk {
        let mut w = Walk::new(&OPEN);
        // fact 6: 1 for a mechanical valve (Table 12 is for mechanical valves)
        if f[6] == 1 {
            let pattern = f[0] >= 1 && f[5] == 1 && f[4] == 0;
            w.ask(&MR_PATTERN, if pattern { answers::YES } else { answers::NO });
            if pattern {
                return w.end(&MR);
            }
        }
        let count = |c: u8| f[..MV_PARAMS].iter().fold(0, |k, x| k + usize::from(*x == c));
        let (normal, significant) = (count(0), count(2));
        if normal >= 3 {
            w.ask(&MAJORITY, tx!("normal", "normale"));
            w.end(&NORMAL_COL)
        } else if significant >= 3 {
            w.ask(&MAJORITY, tx!("significant stenosis", "stenosi significativa"));
            w.end(&SIGNIFICANT_COL)
        } else {
            // with five parameters, fewer than three normal means three or more abnormal
            w.ask(
                &MAJORITY,
                tx!(
                    "abnormal (possible or significant)",
                    "anormale (possibile o significativa)"
                ),
            );
            w.end(&ABNORMAL_COL)
        }
    }
}

/// Tables 11 and 12.
pub static MITRAL_TREE: Tree = Tree {
    calculator: "mitral-prosthesis-screening",
    nodes: &[&mitral::MR_PATTERN, &mitral::MAJORITY],
    leaves: &[
        &mitral::MR,
        &mitral::NORMAL_COL,
        &mitral::SIGNIFICANT_COL,
        &mitral::ABNORMAL_COL,
    ],
    open: &mitral::OPEN,
    arities: &[3, 3, 3, 3, 3, 2, 2],
    walk: mitral::walk,
};

fn column(s: &Scale, v: Option<f64>) -> Fact {
    let class = v.map(|x| match s.class(x).0 {
        Severity::Normal => 0,
        Severity::Mild => 1,
        _ => 2,
    });
    let shown = match v {
        Some(x) => s
            .finding(x)
            .criterion
            .replace(s.symbol, &format!("{} = {}", s.symbol, n(x, s.decimals))),
        None => format!("{} = ?", s.symbol),
    };
    Fact::class(class, 3, shown)
}

fn mitral_screening(i: &Inputs) -> Result<Outcome, Vec<InputError>> {
    let ids = ["vmax_m_s", "mean_gradient_mmhg", "vti_ratio", "eoa_cm2", "pht_ms"];
    let values: Vec<Option<f64>> = ids.iter().map(|id| i.num(id)).collect();
    if values.iter().all(Option::is_none) {
        return Err(need_one("vmax_m_s"));
    }
    let scales = [
        &MV_VMAX_SCALE,
        &MV_MEAN_SCALE,
        &MV_DVI_SCALE,
        &MV_EOA_SCALE,
        &MV_PHT_SCALE,
    ];
    let mut out = outcome();
    let mut facts: Vec<Fact> = scales.iter().zip(&values).map(|(s, v)| column(s, *v)).collect();
    for c in 0..3u8 {
        let count = facts.iter().filter(|f| f.value == Some(c)).count();
        let (id, label) = match c {
            0 => (
                "parameters_normal",
                tx!(
                    "Parameters in the normal column",
                    "Parametri nella colonna normale"
                ),
            ),
            1 => (
                "parameters_possible",
                tx!(
                    "Parameters in the possible-stenosis column",
                    "Parametri nella colonna possibile stenosi"
                ),
            ),
            _ => (
                "parameters_significant",
                tx!(
                    "Parameters in the significant-stenosis column",
                    "Parametri nella colonna stenosi significativa"
                ),
            ),
        };
        out.push(
            q(
                id,
                label,
                f64::from(u32::try_from(count).unwrap_or(0)),
                Unit::Score,
                0,
            ),
            "count of the five Table 11 parameters",
            format!("{count} / {}", facts.iter().filter(|f| f.value.is_some()).count()),
        );
    }
    for (s, v) in scales.iter().zip(&values) {
        if let Some(x) = v {
            out.findings.push(s.finding(*x));
        }
    }
    ppm(&mut out, i, true);
    facts.push(Fact::of(
        &test("VTI ratio", Cmp::Ge, 2.5, Unit::None, 1),
        i.num("vti_ratio"),
    ));
    facts.push(Fact::flag(
        Some(i.choice("valve_type") == Some("mechanical")),
        format!("valve = {}", i.choice("valve_type").unwrap_or("?")),
    ));
    out.findings.extend(tree::decide(&MITRAL_TREE, &facts).findings);
    Ok(out)
}

/// Screening of a mitral prosthesis for stenosis, the MR pattern and mismatch (Tables 11, 12, 7).
pub static MITRAL_SCREENING: Calculator = Calculator {
    id: "mitral-prosthesis-screening",
    category: Category::Prosthetic,
    name: tx!(
        "Mitral prosthesis: stenosis, MR and mismatch screening",
        "Protesi mitralica: screening di stenosi, IM e mismatch"
    ),
    synonyms: tx!(
        "prosthetic mitral valve, MVR, obstruction, stenosis, PHT, VTI ratio, DVI, EOA, paravalvular, patient-prosthesis mismatch",
        "protesi mitralica, SVM, ostruzione, stenosi, PHT, rapporto delle VTI, DVI, EOA, paravalvolare, mismatch protesi-paziente"
    ),
    summary: tx!(
        "Places each Doppler parameter of a mitral prosthesis in its Table 11 column (at a normal heart rate), checks the pattern Table 12 gives for significant MR in a mechanical valve (raised velocity and VTI ratio with a normal PHT), and reports where most parameters fall: the table is most accurate when most are normal or most abnormal. Mismatch by the indexed EOA (Table 7).",
        "Colloca ogni parametro Doppler di una protesi mitralica nella sua colonna della Tabella 11 (a frequenza normale), verifica il quadro che la Tabella 12 indica per IM significativa in una protesi meccanica (velocità e rapporto delle VTI aumentati con PHT normale) e riferisce dove cade la maggior parte dei parametri: la tabella è più accurata quando la maggior parte è normale o anormale. Mismatch dall'EOA indicizzata (Tabella 7)."
    ),
    inputs: &[
        choice("valve_type", tx!("Prosthesis", "Protesi"), VALVE_TYPE),
        opt(
            "vmax_m_s",
            tx!("Peak early velocity", "Velocità protodiastolica massima"),
            Unit::MS,
            0.5,
            4.0,
            1,
        ),
        opt(
            "mean_gradient_mmhg",
            tx!("Mean gradient", "Gradiente medio"),
            Unit::MmHg,
            0.5,
            40.0,
            0,
        ),
        opt(
            "vti_ratio",
            tx!(
                "VTI ratio (prosthesis ÷ LVOT)",
                "Rapporto delle VTI (protesi ÷ LVOT)"
            ),
            Unit::None,
            0.5,
            8.0,
            1,
        ),
        opt(
            "eoa_cm2",
            tx!("EOA (continuity)", "EOA (continuità)"),
            Unit::Cm2,
            0.3,
            5.0,
            1,
        ),
        opt(
            "pht_ms",
            tx!("Pressure half-time", "Tempo di dimezzamento della pressione"),
            Unit::Ms,
            30.0,
            500.0,
            0,
        ),
        IEOA,
        BMI,
    ],
    compute: mitral_screening,
    references: &[
        cite(&PV_2024, "Table 11 and main criteria, pp. 24 and 27"),
        cite(&PV_2024, "Table 12, p. 28"),
        cite(&PV_2024, "Table 7 (mismatch), p. 20"),
    ],
    review: Review::Verified,
    example: &[
        ("valve_type", C("biological")),
        ("vmax_m_s", N(2.6)),
        ("mean_gradient_mmhg", N(12.0)),
        ("vti_ratio", N(2.7)),
        ("eoa_cm2", N(1.1)),
        ("pht_ms", N(210.0)),
    ],
    sim_link: None,
};

#[cfg(test)]
mod tests {
    use super::super::testing::{inconsistent, ok, severities};
    use super::*;

    fn leaf_id(c: &Calculator, i: &Inputs) -> &'static str {
        let t = tree::for_calculator(c.id).unwrap();
        let out = ok(c, i);
        tree::reached(t, out.findings.last().unwrap())
            .unwrap_or_else(|| panic!("{:?}", out.findings.last()))
            .id
    }

    #[test]
    fn figure_11_cases_reach_the_published_answer() {
        // Figure 11 (p. 17), stenotic valve: Vmax 4.1 m/s, AT 127 ms, AT/ET 0.4, DVI 0.24 → stenosis
        let stenotic = Inputs::new()
            .with_choice("implant", "surgical")
            .with("vmax_m_s", 4.1)
            .with("at_ms", 127.0)
            .with("et_ms", 127.0 / 0.4)
            .with("dvi", 0.24);
        assert_eq!(leaf_id(&AORTIC_SCREENING, &stenotic), "stenosis");
        // the normal valve: Vmax 1.8 m/s, AT 76 ms, AT/ET 0.24, DVI 0.6 → the figure is not entered
        let normal = Inputs::new()
            .with_choice("implant", "surgical")
            .with("vmax_m_s", 1.8)
            .with("at_ms", 76.0)
            .with("et_ms", 76.0 / 0.24)
            .with("dvi", 0.6);
        assert_eq!(leaf_id(&AORTIC_SCREENING, &normal), "not-entered");
        let out = ok(&AORTIC_SCREENING, &normal);
        // Table 5: velocity, AT, AT/ET normal, DVI normal
        assert_eq!(
            severities(&out)[..4],
            [
                Severity::Normal,
                Severity::Normal,
                Severity::Normal,
                Severity::Normal
            ]
        );
    }

    #[test]
    fn figure_13_branches_and_boundaries() {
        let a = |vmax: f64, at: f64, dvi: f64| {
            Inputs::new()
                .with_choice("implant", "transcatheter")
                .with("vmax_m_s", vmax)
                .with("at_ms", at)
                .with("dvi", dvi)
        };
        // 3.0 m/s does not enter (> 3), 3.1 does
        assert_eq!(leaf_id(&AORTIC_SCREENING, &a(3.0, 80.0, 0.4)), "not-entered");
        assert_eq!(leaf_id(&AORTIC_SCREENING, &a(3.1, 80.0, 0.4)), "normal");
        // DVI 0.30 normal, 0.29 and 0.25 possible, 0.24 undrawn after an early peak
        assert_eq!(leaf_id(&AORTIC_SCREENING, &a(3.5, 80.0, 0.30)), "normal");
        assert_eq!(leaf_id(&AORTIC_SCREENING, &a(3.5, 80.0, 0.29)), "possible");
        assert_eq!(leaf_id(&AORTIC_SCREENING, &a(3.5, 80.0, 0.25)), "possible");
        assert_eq!(leaf_id(&AORTIC_SCREENING, &a(3.5, 80.0, 0.24)), "undrawn");
        // AT 100 is late (the gap value goes to the abnormal side)
        assert_eq!(leaf_id(&AORTIC_SCREENING, &a(3.5, 100.0, 0.24)), "stenosis");
        assert_eq!(leaf_id(&AORTIC_SCREENING, &a(3.5, 99.0, 0.24)), "undrawn");
        assert_eq!(leaf_id(&AORTIC_SCREENING, &a(3.5, 120.0, 0.35)), "undrawn");
        // AT and AT/ET disagree
        let d = a(3.5, 120.0, 0.3).with("et_ms", 400.0);
        assert_eq!(leaf_id(&AORTIC_SCREENING, &d), "discordant");
        // no contour: open
        let o = Inputs::new()
            .with_choice("implant", "surgical")
            .with("vmax_m_s", 3.5)
            .with("dvi", 0.27);
        // possible on both drawn branches, but AT and AT/ET, once measured, could disagree
        assert_eq!(leaf_id(&AORTIC_SCREENING, &o), "open");
        inconsistent(
            &AORTIC_SCREENING,
            &a(3.5, 120.0, 0.3).with("et_ms", 110.0),
            "at_ms",
        );
    }

    #[test]
    fn table_5_and_table_7_bands() {
        let base = Inputs::new()
            .with_choice("implant", "surgical")
            .with("vmax_m_s", 2.0);
        let grade = |id: &str, x: f64| {
            let out = ok(&AORTIC_SCREENING, &base.clone().with(id, x));
            out.findings[1].clone()
        };
        // mean gradient: 19 normal, 20 possible, 34 possible, 34.5 (gap) significant, 35 significant
        assert_eq!(grade("mean_gradient_mmhg", 19.9).severity, Severity::Normal);
        assert_eq!(grade("mean_gradient_mmhg", 20.0).severity, Severity::Mild);
        assert_eq!(grade("mean_gradient_mmhg", 34.0).severity, Severity::Mild);
        assert_eq!(grade("mean_gradient_mmhg", 34.5).severity, Severity::Severe);
        // DVI: 0.36 normal, 0.35 possible, 0.25 possible, 0.24 significant
        assert_eq!(grade("dvi", 0.36).severity, Severity::Normal);
        assert_eq!(grade("dvi", 0.35).severity, Severity::Mild);
        assert_eq!(grade("dvi", 0.25).severity, Severity::Mild);
        assert_eq!(grade("dvi", 0.24).severity, Severity::Severe);
        // AT: 79 normal, 80 and 100 possible, 101 significant
        assert_eq!(grade("at_ms", 79.0).severity, Severity::Normal);
        assert_eq!(grade("at_ms", 80.0).severity, Severity::Mild);
        assert_eq!(grade("at_ms", 100.0).severity, Severity::Mild);
        assert_eq!(grade("at_ms", 101.0).severity, Severity::Severe);
        // velocity 2.99 normal, 3.0 possible, 4.0 significant
        let v = |x: f64| ok(&AORTIC_SCREENING, &base.clone().with("vmax_m_s", x)).findings[0].severity;
        assert_eq!(v(2.99), Severity::Normal);
        assert_eq!(v(3.0), Severity::Mild);
        assert_eq!(v(3.99), Severity::Mild);
        assert_eq!(v(4.0), Severity::Severe);
        // Table 7, aortic: 0.85 moderate, 0.86 none, 0.65 severe; with obesity 0.70 / 0.55
        let ppm = |x: f64, bmi: &str| ppm_scale(false, bmi == "30-or-more").finding(x).severity;
        assert_eq!(ppm(0.86, "under-30"), Severity::Normal);
        assert_eq!(ppm(0.85, "under-30"), Severity::Moderate);
        assert_eq!(ppm(0.66, "under-30"), Severity::Moderate);
        assert_eq!(ppm(0.65, "under-30"), Severity::Severe);
        assert_eq!(ppm(0.71, "30-or-more"), Severity::Normal);
        assert_eq!(ppm(0.70, "30-or-more"), Severity::Moderate);
        assert_eq!(ppm(0.55, "30-or-more"), Severity::Severe);
        // mitral: 1.2 moderate (table), 0.90 severe; obese 1.0 / 0.75
        assert_eq!(ppm_scale(true, false).finding(1.21).severity, Severity::Normal);
        assert_eq!(ppm_scale(true, false).finding(1.2).severity, Severity::Moderate);
        assert_eq!(ppm_scale(true, false).finding(0.91).severity, Severity::Moderate);
        assert_eq!(ppm_scale(true, false).finding(0.90).severity, Severity::Severe);
        assert_eq!(ppm_scale(true, true).finding(1.0).severity, Severity::Moderate);
        assert_eq!(ppm_scale(true, true).finding(0.75).severity, Severity::Severe);
        // and on the page through the inputs
        let out = ok(
            &AORTIC_SCREENING,
            &base
                .clone()
                .with("ieoa_cm2_m2", 0.6)
                .with_choice("bmi_class", "under-30"),
        );
        assert!(out.findings.iter().any(|f| f.criterion == "iEOA ≤ 0.65 cm²/m²"));
    }

    #[test]
    fn serial_comparison_tables_5_and_6() {
        let s = |m0: f64, m1: f64| {
            Inputs::new()
                .with_choice("stable_conditions", "yes")
                .with_choice("ar_change", "none")
                .with("mean_baseline_mmhg", m0)
                .with("mean_current_mmhg", m1)
        };
        // +10 to 20 mmHg with DVI 0.50 → 0.40 (−0.10, −20%): possible
        let i = s(10.0, 20.0).with("dvi_baseline", 0.5).with("dvi_current", 0.4);
        assert_eq!(leaf_id(&AORTIC_SERIAL, &i), "possible");
        // +9 only: none
        let i = s(10.0, 19.0).with("dvi_baseline", 0.5).with("dvi_current", 0.4);
        assert_eq!(leaf_id(&AORTIC_SERIAL, &i), "none");
        // +20 to 30 with EOA 1.6 → 0.8 (−50%): significant
        let i = s(10.0, 30.0)
            .with("eoa_baseline_cm2", 1.6)
            .with("eoa_current_cm2", 0.8);
        assert_eq!(leaf_id(&AORTIC_SERIAL, &i), "significant");
        // +20 to 30 with EOA −0.59 (1.8 → 1.21, −33%): possible only
        let i = s(10.0, 30.0)
            .with("eoa_baseline_cm2", 1.8)
            .with("eoa_current_cm2", 1.21);
        assert_eq!(leaf_id(&AORTIC_SERIAL, &i), "possible");
        // a gradient rise without EOA or DVI: open
        assert_eq!(leaf_id(&AORTIC_SERIAL, &s(10.0, 30.0)), "open");
        // no rise: none, whatever EOA and DVI
        assert_eq!(leaf_id(&AORTIC_SERIAL, &s(10.0, 12.0)), "none");
        // AR up two grades: significant alone
        let i = s(10.0, 12.0).with_choice("ar_change", "two-grades");
        assert_eq!(leaf_id(&AORTIC_SERIAL, &i), "significant");
        // unstable conditions
        let i = s(10.0, 30.0).with_choice("stable_conditions", "no");
        assert_eq!(leaf_id(&AORTIC_SERIAL, &i), "not-comparable");
        // Table 5 TAVI gradient column: +9 normal, +10 possible, +19 possible, +20 significant
        let col = |m1: f64| ok(&AORTIC_SERIAL, &s(10.0, m1)).findings[0].severity;
        assert_eq!(col(19.0), Severity::Normal);
        assert_eq!(col(20.0), Severity::Mild);
        assert_eq!(col(29.0), Severity::Mild);
        assert_eq!(col(30.0), Severity::Severe);
        // DVI fall: 0.09 and 19% normal; 0.1 possible; 40% significant
        assert_eq!(decrease_class(0.5, 0.41, (0.1, 0.2), (20.0, 40.0)), 0);
        assert_eq!(decrease_class(0.5, 0.40, (0.1, 0.2), (20.0, 40.0)), 1);
        assert_eq!(decrease_class(0.5, 0.30, (0.1, 0.2), (20.0, 40.0)), 2);
        assert_eq!(
            decrease_class(0.25, 0.15, (0.1, 0.2), (20.0, 40.0)),
            2,
            "−40% although −0.10"
        );
        // a pair needs both values
        inconsistent(
            &AORTIC_SERIAL,
            &s(10.0, 12.0).with("dvi_baseline", 0.5),
            "dvi_current",
        );
    }

    #[test]
    fn mitral_screening_tables_11_and_12() {
        let m = |v: f64, mean: f64, ratio: f64, eoa: f64, pht: f64| {
            Inputs::new()
                .with_choice("valve_type", "biological")
                .with("vmax_m_s", v)
                .with("mean_gradient_mmhg", mean)
                .with("vti_ratio", ratio)
                .with("eoa_cm2", eoa)
                .with("pht_ms", pht)
        };
        assert_eq!(
            leaf_id(&MITRAL_SCREENING, &m(1.6, 4.0, 1.9, 2.4, 100.0)),
            "normal"
        );
        assert_eq!(
            leaf_id(&MITRAL_SCREENING, &m(2.6, 12.0, 2.7, 0.9, 210.0)),
            "significant"
        );
        assert_eq!(
            leaf_id(&MITRAL_SCREENING, &m(2.0, 8.0, 2.3, 1.5, 150.0)),
            "possible"
        );
        assert_eq!(
            leaf_id(&MITRAL_SCREENING, &m(1.6, 4.0, 2.3, 1.5, 150.0)),
            "possible",
            "3 of 5 abnormal"
        );
        // two normal, two significant, one empty: either column could hold three
        let open = m(1.6, 4.0, 2.7, 0.9, 150.0);
        let mut open = open;
        open.remove("pht_ms");
        assert_eq!(leaf_id(&MITRAL_SCREENING, &open), "open");
        // boundaries of Table 11: 1.9 possible, 2.5 significant; mean 5 normal, 5.5 (gap) possible,
        // 10 possible, 10.5 significant; EOA 2.0 normal, 1.0 possible, 0.99 significant
        let col = |id: &str, x: f64| {
            let out = ok(
                &MITRAL_SCREENING,
                &Inputs::new().with_choice("valve_type", "biological").with(id, x),
            );
            out.findings[0].severity
        };
        assert_eq!(col("vmax_m_s", 1.89), Severity::Normal);
        assert_eq!(col("vmax_m_s", 1.9), Severity::Mild);
        assert_eq!(col("vmax_m_s", 2.5), Severity::Severe);
        assert_eq!(col("mean_gradient_mmhg", 5.0), Severity::Normal);
        assert_eq!(col("mean_gradient_mmhg", 5.5), Severity::Mild);
        assert_eq!(col("mean_gradient_mmhg", 10.0), Severity::Mild);
        assert_eq!(col("mean_gradient_mmhg", 10.5), Severity::Severe);
        assert_eq!(col("vti_ratio", 2.19), Severity::Normal);
        assert_eq!(col("vti_ratio", 2.2), Severity::Mild);
        assert_eq!(col("vti_ratio", 2.5), Severity::Mild);
        assert_eq!(col("vti_ratio", 2.51), Severity::Severe);
        assert_eq!(col("eoa_cm2", 2.0), Severity::Normal);
        assert_eq!(col("eoa_cm2", 1.99), Severity::Mild);
        assert_eq!(col("eoa_cm2", 1.0), Severity::Mild);
        assert_eq!(col("eoa_cm2", 0.99), Severity::Severe);
        assert_eq!(col("pht_ms", 129.0), Severity::Normal);
        assert_eq!(col("pht_ms", 130.0), Severity::Mild);
        assert_eq!(col("pht_ms", 200.0), Severity::Mild);
        assert_eq!(col("pht_ms", 201.0), Severity::Severe);
        // Table 12, mechanical valve: velocity 1.9, ratio 2.5, PHT 129 → MR pattern
        let mr = m(1.9, 6.0, 2.5, 1.8, 129.0).with_choice("valve_type", "mechanical");
        assert_eq!(leaf_id(&MITRAL_SCREENING, &mr), "mr");
        // PHT 130 is not normal: no MR pattern
        let not = m(1.9, 6.0, 2.5, 1.8, 130.0).with_choice("valve_type", "mechanical");
        assert_ne!(leaf_id(&MITRAL_SCREENING, &not), "mr");
        // a biological valve is not read with Table 12
        assert_ne!(leaf_id(&MITRAL_SCREENING, &m(1.9, 6.0, 2.5, 1.8, 129.0)), "mr");
        // Figure 19 (p. 28): a mean gradient of 29 mmHg is in the significant column
        assert_eq!(col("mean_gradient_mmhg", 29.0), Severity::Severe);
        assert!(
            MITRAL_SCREENING
                .evaluate(&Inputs::new().with_choice("valve_type", "mechanical"))
                .is_err()
        );
    }
}
