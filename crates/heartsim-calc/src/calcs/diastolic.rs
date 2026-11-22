//! LV diastolic function and filling pressure by the 2025 ASE recommendation (Nagueh et al.): the
//! algorithm for sinus rhythm (Figure 3) and the special populations (Table 7, Figures 4, 5, 6 and
//! 8, diastolic exercise echocardiography). Each is a decision tree of [`crate::tree`]; every node
//! and every leaf cites the figure or table and the journal page it was read on.
//!
//! Boundary values are placed as the figure prints them; where a figure and the text or a key point
//! disagree, the figure is followed and the difference is listed in
//! `docs/research/echo-calculators.md`.

use super::{cite, n, need_one, opt, opt_choice, outcome, q, v, when};
use crate::model::ExampleValue::{Choice as C, Number as N};
use crate::model::{
    Calculator, Category, Choice, Cite, InputError, InputSpec, Inputs, Outcome, Review, Severity, Text, Unit,
};
use crate::refs::DD_2025;
use crate::tree::{self, Cmp, Fact, Leaf, LeafKind, Node, Tree, Walk, answers, test};
use crate::tx;

const fn dd(locator: &'static str) -> Cite {
    cite(&DD_2025, locator)
}

const fn node(id: &'static str, question: Text, locator: &'static str, uses: &'static [usize]) -> Node {
    Node {
        id,
        question,
        cite: dd(locator),
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
        cite: dd(locator),
    }
}

// ---------------------------------------------------------------------------------------------
// Inputs shared by the tools (all optional: a tree decides what an empty field could change)

const E: InputSpec = opt(
    "e_cm_s",
    tx!("Mitral E velocity", "Velocità E mitralica"),
    Unit::CmS,
    20.0,
    250.0,
    0,
);
const A: InputSpec = opt(
    "a_cm_s",
    tx!("Mitral A velocity", "Velocità A mitralica"),
    Unit::CmS,
    10.0,
    250.0,
    0,
);
const E_SEPTAL: InputSpec = opt(
    "e_septal_cm_s",
    tx!("Septal e' velocity", "Velocità e' settale"),
    Unit::CmS,
    1.0,
    30.0,
    1,
);
const E_LATERAL: InputSpec = opt(
    "e_lateral_cm_s",
    tx!("Lateral e' velocity", "Velocità e' laterale"),
    Unit::CmS,
    1.0,
    30.0,
    1,
);
const TR: InputSpec = opt(
    "tr_vmax_m_s",
    tx!("Peak TR velocity", "Velocità massima del rigurgito tricuspidale"),
    Unit::MS,
    0.5,
    6.0,
    2,
);
const PASP: InputSpec = opt(
    "pasp_mmhg",
    tx!(
        "PA systolic pressure (when RAP can be estimated)",
        "Pressione sistolica polmonare (se la PAD è stimabile)"
    ),
    Unit::MmHg,
    10.0,
    150.0,
    0,
);
const LARS: InputSpec = opt(
    "lars_percent",
    tx!("LA reservoir strain (LARS)", "Strain di serbatoio dell'AS (LARS)"),
    Unit::Percent,
    1.0,
    80.0,
    0,
);
const PV_SD: InputSpec = opt(
    "pv_sd_ratio",
    tx!(
        "Pulmonary vein S/D velocity ratio",
        "Rapporto S/D delle vene polmonari"
    ),
    Unit::None,
    0.1,
    5.0,
    2,
);
const LAVI: InputSpec = opt(
    "lavi_ml_m2",
    tx!("LA maximum volume index", "Volume massimo dell'AS indicizzato"),
    Unit::MlM2,
    10.0,
    200.0,
    0,
);
const IVRT: InputSpec = opt(
    "ivrt_ms",
    tx!(
        "Isovolumic relaxation time (IVRT)",
        "Tempo di rilasciamento isovolumetrico (IVRT)"
    ),
    Unit::Ms,
    20.0,
    200.0,
    0,
);
const DT: InputSpec = opt(
    "dt_ms",
    tx!("Mitral E deceleration time", "Tempo di decelerazione dell'onda E"),
    Unit::Ms,
    50.0,
    500.0,
    0,
);
const AR_DUR: InputSpec = opt(
    "ar_duration_ms",
    tx!(
        "Pulmonary vein Ar duration",
        "Durata dell'onda Ar delle vene polmonari"
    ),
    Unit::Ms,
    30.0,
    300.0,
    0,
);
const A_DUR: InputSpec = opt(
    "a_duration_ms",
    tx!("Mitral A duration", "Durata dell'onda A mitralica"),
    Unit::Ms,
    30.0,
    300.0,
    0,
);
const TE_E: InputSpec = opt(
    "te_e_ms",
    tx!(
        "TE-e' interval (onset of E to onset of e')",
        "Intervallo TE-e' (inizio di E - inizio di e')"
    ),
    Unit::Ms,
    1.0,
    100.0,
    0,
);

// ---------------------------------------------------------------------------------------------
// Derived values, pushed with their formula when the measurements are there

/// E/e' ratios and the average e' from what was measured.
#[derive(Debug, Clone, Copy, Default)]
struct Ratios {
    septal: Option<f64>,
    lateral: Option<f64>,
    average: Option<f64>,
}

fn ratios(out: &mut Outcome, i: &Inputs) -> Ratios {
    let e = i.num("e_cm_s");
    let (es, el) = (i.num("e_septal_cm_s"), i.num("e_lateral_cm_s"));
    let mut r = Ratios::default();
    if let (Some(es), Some(el)) = (es, el) {
        let avg = f64::midpoint(es, el);
        out.push(
            q(
                "e_prime_average_cm_s",
                tx!("Average e'", "e' media"),
                avg,
                Unit::CmS,
                1,
            ),
            "e'(avg) = (e'(sept) + e'(lat)) ÷ 2",
            format!("({} + {}) ÷ 2", n(es, 1), n(el, 1)),
        );
        if let Some(e) = e {
            r.average = Some(e / avg);
            out.push(
                q(
                    "e_e_average",
                    tx!("Average E/e'", "E/e' media"),
                    e / avg,
                    Unit::None,
                    1,
                ),
                "E/e'(avg) = E ÷ e'(avg)",
                format!("{} ÷ {}", n(e, 0), n(avg, 2)),
            );
        }
    }
    if let (Some(e), Some(es)) = (e, es) {
        r.septal = Some(e / es);
        out.push(
            q(
                "e_e_septal",
                tx!("Septal E/e'", "E/e' settale"),
                e / es,
                Unit::None,
                1,
            ),
            "E/e'(sept) = E ÷ e'(sept)",
            format!("{} ÷ {}", n(e, 0), n(es, 1)),
        );
    }
    if let (Some(e), Some(el)) = (e, el) {
        r.lateral = Some(e / el);
        out.push(
            q(
                "e_e_lateral",
                tx!("Lateral E/e'", "E/e' laterale"),
                e / el,
                Unit::None,
                1,
            ),
            "E/e'(lat) = E ÷ e'(lat)",
            format!("{} ÷ {}", n(e, 0), n(el, 1)),
        );
    }
    r
}

/// E/A from the two velocities, if both were measured.
fn e_over_a(out: &mut Outcome, i: &Inputs) -> Option<f64> {
    let (e, a) = (i.num("e_cm_s")?, i.num("a_cm_s")?);
    out.push(
        q(
            "e_a_ratio",
            tx!("E/A ratio", "Rapporto E/A"),
            e / a,
            Unit::None,
            2,
        ),
        "E/A = E ÷ A",
        format!("{} ÷ {}", n(e, 0), n(a, 0)),
    );
    Some(e / a)
}

/// Ar − A from the two durations, if both were measured.
fn ar_minus_a(out: &mut Outcome, i: &Inputs) -> Option<f64> {
    let (ar, a) = (i.num("ar_duration_ms")?, i.num("a_duration_ms")?);
    out.push(
        q(
            "ar_minus_a_ms",
            tx!("Ar − A duration", "Durata Ar − A"),
            ar - a,
            Unit::Ms,
            0,
        ),
        "Ar − A",
        format!("{} − {}", v(ar, 0, Unit::Ms), v(a, 0, Unit::Ms)),
    );
    Some(ar - a)
}

/// IVRT ÷ TE-e', if both were measured.
fn ivrt_over_te(out: &mut Outcome, i: &Inputs) -> Option<f64> {
    let (ivrt, te) = (i.num("ivrt_ms")?, i.num("te_e_ms")?);
    out.push(
        q(
            "ivrt_te_e_ratio",
            tx!("IVRT / TE-e'", "IVRT / TE-e'"),
            ivrt / te,
            Unit::None,
            1,
        ),
        "IVRT ÷ TE-e'",
        format!("{} ÷ {}", v(ivrt, 0, Unit::Ms), v(te, 0, Unit::Ms)),
    );
    Some(ivrt / te)
}

/// A category from consecutive classes: `uppers[k]` is the comparison with its limit that keeps a
/// value in class `k`; a value that passes none is in the last class. Shown with the value only.
fn classes(v: Option<f64>, symbol: &'static str, unit: Unit, decimals: u8, uppers: &[(Cmp, f64)]) -> Fact {
    let arity = u8::try_from(uppers.len() + 1).unwrap_or(u8::MAX);
    let class = v.map(|x| {
        let k = uppers
            .iter()
            .position(|(cmp, lim)| cmp.holds(x, *lim))
            .unwrap_or(uppers.len());
        u8::try_from(k).unwrap_or(0)
    });
    let shown = match v {
        None => format!("{symbol} = ?"),
        Some(x) => format!("{symbol} = {}{}", crate::fmt::num(x, decimals), suffix(unit)),
    };
    Fact::class(class, arity, shown)
}

fn suffix(unit: Unit) -> String {
    match unit.symbol() {
        "" => String::new(),
        "%" => "%".to_owned(),
        s => format!(" {s}"),
    }
}

/// The count of measured indicators met, as a value of the outcome.
fn push_count(out: &mut Outcome, facts: &[Fact]) {
    let met = facts.iter().filter(|f| f.value == Some(1)).count();
    let measured = facts.iter().filter(|f| f.value.is_some()).count();
    out.push(
        q(
            "indicators_met",
            tx!(
                "Indicators met (of those measured)",
                "Indicatori soddisfatti (fra quelli misurati)"
            ),
            f64::from(u32::try_from(met).unwrap_or(0)),
            Unit::Score,
            0,
        ),
        "indicators met, of those measured",
        format!("{met} of {measured}"),
    );
}

fn run(out: &mut Outcome, t: &Tree, facts: &[Fact]) {
    out.findings = tree::decide(t, facts).findings;
}

const AGE_CUTOFFS: &[Choice] = &[
    Choice {
        id: "figure",
        label: tx!(
            "Cut-offs of Figure 3 (any age)",
            "Soglie della Figura 3 (ogni età)"
        ),
    },
    Choice {
        id: "20-39",
        label: tx!("Table 6, age 20-39 years", "Tabella 6, età 20-39 anni"),
    },
    Choice {
        id: "40-65",
        label: tx!("Table 6, age 40-65 years", "Tabella 6, età 40-65 anni"),
    },
    Choice {
        id: "over-65",
        label: tx!("Table 6, age over 65 years", "Tabella 6, età oltre 65 anni"),
    },
];

// =============================================================================================
// Sinus rhythm: Figure 3

pub(crate) mod sinus {
    #[allow(clippy::wildcard_imports)]
    use super::*;

    pub const E_PRIME: usize = 0;
    pub const E_E: usize = 1;
    pub const TR_PASP: usize = 2;
    pub const EA: usize = 3;
    pub const STAGE2: usize = 4;
    pub const SUPPLEMENTAL: usize = 5;

    const F3: &str = "Figure 3, p. 554";

    pub static STEP1: Node = node(
        "step-1",
        tx!(
            "Reduced e', increased E/e', increased TR velocity or PASP",
            "e' ridotta, E/e' aumentato, velocità del RT o PAPs aumentata"
        ),
        F3,
        &[E_PRIME, E_E, TR_PASP],
    );
    pub static EA_LOW: Node = node(
        "e-a-0.8",
        tx!(
            "With reduced e' only, the E/A ratio",
            "Con sola e' ridotta, il rapporto E/A"
        ),
        F3,
        &[EA],
    );
    pub static MORE: Node = node(
        "additional",
        tx!(
            "Additional variables (PV S/D ≤ 0.67, LARS ≤ 18%, LAVi > 34 mL/m², or alternatively IVRT ≤ 70 ms)",
            "Variabili aggiuntive (S/D venoso ≤ 0,67, LARS ≤ 18%, LAVi > 34 mL/m², o in alternativa IVRT ≤ 70 ms)"
        ),
        "Figure 3 and text, pp. 554-555",
        &[STAGE2],
    );
    pub static SUPP: Node = node(
        "supplemental",
        tx!(
            "Supplemental methods (PR end-diastolic velocity, PA diastolic pressure, L wave, Ar − A, Valsalva)",
            "Metodi supplementari (velocità telediastolica del RP, pressione diastolica polmonare, onda L, Ar − A, Valsalva)"
        ),
        "Figure 3 legend, p. 554; Supplemental parameters a-e, p. 555",
        &[SUPPLEMENTAL],
    );
    pub static GRADE: Node = node(
        "grade",
        tx!(
            "With increased LAP, the E/A ratio that grades it",
            "Con PAS aumentata, il rapporto E/A che la gradua"
        ),
        F3,
        &[EA],
    );

    pub static NORMAL_DF: Leaf = leaf(
        "normal",
        LeafKind::Result,
        Severity::Normal,
        tx!(
            "meets the figure's criteria for normal diastolic function with normal LAP",
            "soddisfa i criteri della figura per funzione diastolica normale con PAS normale"
        ),
        "e', E/e' and TR velocity or PASP all normal",
        F3,
    );
    pub static GRADE1: Leaf = leaf(
        "grade-1",
        LeafKind::Result,
        Severity::Mild,
        tx!(
            "meets the figure's criteria for grade 1 diastolic dysfunction (normal LAP); if symptomatic, the source recommends diastolic exercise echocardiography",
            "soddisfa i criteri della figura per disfunzione diastolica di grado 1 (PAS normale); se sintomatico, la fonte raccomanda l'ecocardiografia diastolica da sforzo"
        ),
        "reduced e' only, E/A ≤ 0.8",
        F3,
    );
    pub static NORMAL_LAP: Leaf = leaf(
        "normal-lap",
        LeafKind::Result,
        Severity::Normal,
        tx!(
            "meets the figure's criteria for normal LAP (the figure gives no grade on this branch; if symptomatic, diastolic exercise echocardiography)",
            "soddisfa i criteri della figura per PAS normale (la figura non dà un grado su questo ramo; se sintomatico, ecocardiografia diastolica da sforzo)"
        ),
        "none of the additional (or supplemental) variables present",
        F3,
    );
    pub static GRADE2: Leaf = leaf(
        "grade-2",
        LeafKind::Result,
        Severity::Moderate,
        tx!(
            "meets the figure's criteria for grade 2 diastolic dysfunction (mild to moderately increased LAP)",
            "soddisfa i criteri della figura per disfunzione diastolica di grado 2 (PAS lievemente o moderatamente aumentata)"
        ),
        "increased LAP, E/A < 2",
        F3,
    );
    pub static GRADE3: Leaf = leaf(
        "grade-3",
        LeafKind::Result,
        Severity::Severe,
        tx!(
            "meets the figure's criteria for grade 3 diastolic dysfunction (markedly increased LAP)",
            "soddisfa i criteri della figura per disfunzione diastolica di grado 3 (PAS marcatamente aumentata)"
        ),
        "increased LAP, E/A ≥ 2",
        F3,
    );
    pub static OPEN: Leaf = leaf(
        "open",
        LeafKind::Indeterminate,
        Severity::Indeterminate,
        tx!(
            "LAP indeterminate with these measurements",
            "PAS indeterminata con queste misure"
        ),
        "measurements left empty change the result",
        F3,
    );

    fn grade(mut w: Walk, f: &[u8]) -> Walk {
        if f[EA] == 2 {
            w.ask(&GRADE, tx!("≥ 2", "≥ 2"));
            w.end(&GRADE3)
        } else {
            w.ask(&GRADE, tx!("< 2", "< 2"));
            w.end(&GRADE2)
        }
    }

    fn additional(mut w: Walk, f: &[u8]) -> Walk {
        match f[STAGE2] {
            0 => {
                w.ask(&MORE, tx!("none present", "nessuna presente"));
                w.end(&NORMAL_LAP)
            }
            1 => {
                w.ask(&MORE, tx!("at least one present", "almeno una presente"));
                grade(w, f)
            }
            _ => {
                w.ask(&MORE, tx!("none measured", "nessuna misurata"));
                if f[SUPPLEMENTAL] == 1 {
                    w.ask(&SUPP, tx!("at least one present", "almeno uno presente"));
                    grade(w, f)
                } else {
                    w.ask(&SUPP, tx!("none present", "nessuno presente"));
                    w.end(&NORMAL_LAP)
                }
            }
        }
    }

    pub fn walk(f: &[u8]) -> Walk {
        let mut w = Walk::new(&OPEN);
        let count = f[E_PRIME] + f[E_E] + f[TR_PASP];
        match count {
            0 => {
                w.ask(&STEP1, tx!("all normal", "tutti normali"));
                w.end(&NORMAL_DF)
            }
            3 => {
                w.ask(&STEP1, tx!("all three abnormal", "tutti e tre anormali"));
                grade(w, f)
            }
            1 if f[E_PRIME] == 1 => {
                w.ask(&STEP1, tx!("reduced e' only", "solo e' ridotta"));
                if f[EA] == 0 {
                    w.ask(&EA_LOW, tx!("≤ 0.8", "≤ 0,8"));
                    w.end(&GRADE1)
                } else {
                    w.ask(&EA_LOW, tx!("> 0.8", "> 0,8"));
                    additional(w, f)
                }
            }
            _ => {
                w.ask(
                    &STEP1,
                    tx!(
                        "increased E/e' only, increased TR/PASP only, or two of the three",
                        "solo E/e' aumentato, solo RT/PAPs aumentata, o due dei tre"
                    ),
                );
                additional(w, f)
            }
        }
    }

    pub static TREE: Tree = Tree {
        calculator: "diastolic-sinus-rhythm",
        nodes: &[&STEP1, &EA_LOW, &MORE, &SUPP, &GRADE],
        leaves: &[&NORMAL_DF, &GRADE1, &NORMAL_LAP, &GRADE2, &GRADE3],
        open: &OPEN,
        arities: &[2, 2, 2, 3, 3, 2],
        walk,
    };
}

/// The e' cut-offs: those of Figure 3, or the age-specific lower limits of Table 6.
fn e_prime_tests(choice: Option<&str>) -> [tree::Test; 3] {
    let u = Unit::CmS;
    match choice {
        Some("20-39") => [
            test("septal e'", Cmp::Lt, 7.0, u, 1),
            test("lateral e'", Cmp::Lt, 10.0, u, 1),
            test("average e'", Cmp::Lt, 9.0, u, 1),
        ],
        Some("40-65") => [
            test("septal e'", Cmp::Lt, 6.0, u, 1),
            test("lateral e'", Cmp::Lt, 8.0, u, 1),
            test("average e'", Cmp::Lt, 7.0, u, 1),
        ],
        Some("over-65") => [
            test("septal e'", Cmp::Lt, 6.0, u, 1),
            test("lateral e'", Cmp::Lt, 7.0, u, 1),
            test("average e'", Cmp::Lt, 6.5, u, 1),
        ],
        _ => [
            test("septal e'", Cmp::Le, 6.0, u, 1),
            test("lateral e'", Cmp::Le, 7.0, u, 1),
            test("average e'", Cmp::Le, 6.5, u, 1),
        ],
    }
}

fn sinus_rhythm(i: &Inputs) -> Result<Outcome, Vec<InputError>> {
    let mut out = outcome();
    let r = ratios(&mut out, i);
    let ea = e_over_a(&mut out, i);
    let ar_a = ar_minus_a(&mut out, i);
    let (es, el) = (i.num("e_septal_cm_s"), i.num("e_lateral_cm_s"));
    let avg = es.zip(el).map(|(a, b)| f64::midpoint(a, b));
    let [ts, tl, ta] = e_prime_tests(i.choice("e_prime_cutoffs"));
    let e_prime = Fact::any_measured(&[(ts, es), (tl, el), (ta, avg)]);
    let e_e = Fact::any_measured(&[
        (test("septal E/e'", Cmp::Ge, 15.0, Unit::None, 1), r.septal),
        (test("lateral E/e'", Cmp::Ge, 13.0, Unit::None, 1), r.lateral),
        (test("average E/e'", Cmp::Ge, 14.0, Unit::None, 1), r.average),
    ]);
    // PASP when the RAP could be estimated, otherwise the TR velocity (p. 554)
    let tr_pasp = match i.num("pasp_mmhg") {
        Some(p) => Fact::of(&test("PASP", Cmp::Ge, 35.0, Unit::MmHg, 0), Some(p)),
        None => Fact::of(&test("TR", Cmp::Ge, 2.8, Unit::MS, 2), i.num("tr_vmax_m_s")),
    };
    let ea_fact = classes(ea, "E/A", Unit::None, 2, &[(Cmp::Le, 0.8), (Cmp::Lt, 2.0)]);
    let stage2_tests = [
        (test("PV S/D", Cmp::Le, 0.67, Unit::None, 2), i.num("pv_sd_ratio")),
        (
            test("LARS", Cmp::Le, 18.0, Unit::Percent, 0),
            i.num("lars_percent"),
        ),
        (test("LAVi", Cmp::Gt, 34.0, Unit::MlM2, 0), i.num("lavi_ml_m2")),
        (test("IVRT", Cmp::Le, 70.0, Unit::Ms, 0), i.num("ivrt_ms")),
    ];
    let stage2 = {
        let f = Fact::any_measured(&stage2_tests);
        Fact::class(Some(f.value.unwrap_or(2)), 3, f.shown)
    };
    let supplemental = Fact::any_measured(&[
        (
            test("PR end-diastolic", Cmp::Ge, 2.0, Unit::MS, 2),
            i.num("pr_ed_m_s"),
        ),
        (
            test("PA diastolic", Cmp::Ge, 16.0, Unit::MmHg, 0),
            i.num("padp_mmhg"),
        ),
        (test("L wave", Cmp::Ge, 50.0, Unit::CmS, 0), i.num("l_wave_cm_s")),
        (test("Ar − A", Cmp::Gt, 30.0, Unit::Ms, 0), ar_a),
        (
            test("E/A fall with Valsalva", Cmp::Ge, 50.0, Unit::Percent, 0),
            i.num("valsalva_ea_fall_percent"),
        ),
    ]);
    run(
        &mut out,
        &sinus::TREE,
        &[e_prime, e_e, tr_pasp, ea_fact, stage2, supplemental],
    );
    Ok(out)
}

/// LV diastolic function and mean LAP in sinus rhythm (Figure 3).
pub static SINUS_RHYTHM: Calculator = Calculator {
    id: "diastolic-sinus-rhythm",
    category: Category::FillingPressures,
    name: tx!(
        "Diastolic function and LAP (sinus rhythm)",
        "Funzione diastolica e PAS (ritmo sinusale)"
    ),
    synonyms: tx!(
        "diastolic dysfunction, grade, LAP, filling pressure, e', E/e', TR, LARS, LAVi, IVRT, pseudonormal, algorithm",
        "disfunzione diastolica, grado, pressione atriale sinistra, pressioni di riempimento, e', E/e', RT, LARS, LAVi, IVRT, pseudonormale, algoritmo"
    ),
    summary: tx!(
        "The 2025 ASE algorithm for patients in sinus rhythm: e', E/e' and TR velocity (or PASP) first, then LARS, pulmonary vein S/D, LAVi or IVRT, then the supplemental methods; E/A grades an increased LAP. Not for atrial fibrillation, moderate or severe MAC, any mitral stenosis, severe primary MR, mitral repair or replacement, heart transplant, non-cardiac PH, constriction or LVAD. Fields may be left empty: the result says when they could change it.",
        "L'algoritmo ASE 2025 per pazienti in ritmo sinusale: prima e', E/e' e velocità del RT (o PAPs), poi LARS, S/D delle vene polmonari, LAVi o IVRT, poi i metodi supplementari; E/A gradua una PAS aumentata. Non per fibrillazione atriale, MAC moderata o grave, qualsiasi stenosi mitralica, IM primaria grave, riparazione o sostituzione mitralica, trapianto, IP non cardiaca, costrizione o LVAD. I campi possono restare vuoti: il risultato dice quando potrebbero cambiarlo."
    ),
    inputs: &[
        opt_choice("e_prime_cutoffs", tx!("e' cut-offs", "Soglie di e'"), AGE_CUTOFFS),
        E,
        A,
        E_SEPTAL,
        E_LATERAL,
        TR,
        PASP,
        LARS,
        PV_SD,
        LAVI,
        IVRT,
        opt(
            "pr_ed_m_s",
            tx!(
                "PR end-diastolic velocity (supplemental)",
                "Velocità telediastolica del RP (supplementare)"
            ),
            Unit::MS,
            0.2,
            5.0,
            2,
        ),
        opt(
            "padp_mmhg",
            tx!(
                "PA diastolic pressure (supplemental)",
                "Pressione diastolica polmonare (supplementare)"
            ),
            Unit::MmHg,
            0.0,
            80.0,
            0,
        ),
        opt(
            "l_wave_cm_s",
            tx!(
                "Mitral L-wave velocity (supplemental)",
                "Velocità dell'onda L mitralica (supplementare)"
            ),
            Unit::CmS,
            5.0,
            150.0,
            0,
        ),
        AR_DUR,
        A_DUR,
        opt(
            "valsalva_ea_fall_percent",
            tx!(
                "Fall of E/A with Valsalva (supplemental)",
                "Calo di E/A con Valsalva (supplementare)"
            ),
            Unit::Percent,
            0.0,
            100.0,
            0,
        ),
    ],
    compute: sinus_rhythm,
    references: &[
        cite(&DD_2025, "Figure 3 and text, pp. 554-555"),
        cite(&DD_2025, "Table 6 (age-specific e'), p. 553"),
    ],
    review: Review::Verified,
    example: &[
        ("e_cm_s", N(92.0)),
        ("a_cm_s", N(70.0)),
        ("e_septal_cm_s", N(5.5)),
        ("e_lateral_cm_s", N(7.5)),
        ("tr_vmax_m_s", N(2.6)),
        ("lars_percent", N(16.0)),
    ],
    sim_link: None,
};

// =============================================================================================
// Atrial fibrillation: Figure 8

pub(crate) mod af {
    #[allow(clippy::wildcard_imports)]
    use super::*;

    const F8: &str = "Figure 8, p. 563";

    pub static FIRST: Node = node(
        "first",
        tx!(
            "Of E ≥ 100 cm/s, septal E/e' > 11, TR > 2.8 m/s or PASP > 35 mmHg, DT ≤ 160 ms, the number met",
            "Di E ≥ 100 cm/s, E/e' settale > 11, RT > 2,8 m/s o PAPs > 35 mmHg, DT ≤ 160 ms, il numero soddisfatto"
        ),
        F8,
        &[0, 1, 2, 3],
    );
    pub static SECOND: Node = node(
        "second",
        tx!(
            "Of LARS < 18%, PV S/D < 1, BMI > 30 kg/m², the number met",
            "Di LARS < 18%, S/D venoso < 1, BMI > 30 kg/m², il numero soddisfatto"
        ),
        F8,
        &[4, 5, 6],
    );
    pub static NORMAL: Leaf = leaf(
        "normal",
        LeafKind::Result,
        Severity::Normal,
        tx!(
            "meets the figure's criteria for normal LAP",
            "soddisfa i criteri della figura per PAS normale"
        ),
        "none or 1 of the first four; or 2 and none of the second three",
        F8,
    );
    pub static ELEVATED: Leaf = leaf(
        "elevated",
        LeafKind::Result,
        Severity::Abnormal,
        tx!(
            "meets the figure's criteria for elevated LAP",
            "soddisfa i criteri della figura per PAS elevata"
        ),
        "3 or more of the first four; or 2 and 2 or 3 of the second three",
        F8,
    );
    pub static INDETERMINATE: Leaf = leaf(
        "indeterminate",
        LeafKind::Indeterminate,
        Severity::Indeterminate,
        tx!(
            "the figure's indeterminate LAP",
            "PAS indeterminata secondo la figura"
        ),
        "2 of the first four and only 1 of the second three (or none available)",
        F8,
    );
    pub static OPEN: Leaf = leaf(
        "open",
        LeafKind::Indeterminate,
        Severity::Indeterminate,
        tx!(
            "LAP indeterminate with these measurements",
            "PAS indeterminata con queste misure"
        ),
        "measurements left empty change the result",
        F8,
    );

    pub fn walk(f: &[u8]) -> Walk {
        let mut w = Walk::new(&OPEN);
        match f[0] + f[1] + f[2] + f[3] {
            0 | 1 => {
                w.ask(&FIRST, tx!("none or 1", "nessuno o 1"));
                w.end(&NORMAL)
            }
            2 => {
                w.ask(&FIRST, tx!("2", "2"));
                match f[4] + f[5] + f[6] {
                    0 => {
                        w.ask(&SECOND, tx!("none", "nessuno"));
                        w.end(&NORMAL)
                    }
                    1 => {
                        w.ask(&SECOND, tx!("1 only", "solo 1"));
                        w.end(&INDETERMINATE)
                    }
                    _ => {
                        w.ask(&SECOND, tx!("2 or 3", "2 o 3"));
                        w.end(&ELEVATED)
                    }
                }
            }
            _ => {
                w.ask(&FIRST, tx!("3 or more", "3 o più"));
                w.end(&ELEVATED)
            }
        }
    }

    pub static TREE: Tree = Tree {
        calculator: "diastolic-atrial-fibrillation",
        nodes: &[&FIRST, &SECOND],
        leaves: &[&NORMAL, &ELEVATED, &INDETERMINATE],
        open: &OPEN,
        arities: &[2, 2, 2, 2, 2, 2, 2],
        walk,
    };
}

fn atrial_fibrillation(i: &Inputs) -> Result<Outcome, Vec<InputError>> {
    let mut out = outcome();
    let r = ratios(&mut out, i);
    let tr = Fact::any_measured(&[
        (test("TR", Cmp::Gt, 2.8, Unit::MS, 2), i.num("tr_vmax_m_s")),
        (test("PASP", Cmp::Gt, 35.0, Unit::MmHg, 0), i.num("pasp_mmhg")),
    ]);
    let facts = [
        Fact::of(&test("E", Cmp::Ge, 100.0, Unit::CmS, 0), i.num("e_cm_s")),
        Fact::of(&test("septal E/e'", Cmp::Gt, 11.0, Unit::None, 1), r.septal),
        tr,
        Fact::of(&test("DT", Cmp::Le, 160.0, Unit::Ms, 0), i.num("dt_ms")),
        Fact::of(
            &test("LARS", Cmp::Lt, 18.0, Unit::Percent, 0),
            i.num("lars_percent"),
        ),
        Fact::of(&test("PV S/D", Cmp::Lt, 1.0, Unit::None, 2), i.num("pv_sd_ratio")),
        Fact::of(&test("BMI", Cmp::Gt, 30.0, Unit::KgM2, 1), i.num("bmi_kg_m2")),
    ];
    push_count(&mut out, &facts);
    run(&mut out, &af::TREE, &facts);
    Ok(out)
}

/// Mean LAP in atrial fibrillation (Figure 8).
pub static ATRIAL_FIBRILLATION: Calculator = Calculator {
    id: "diastolic-atrial-fibrillation",
    category: Category::FillingPressures,
    name: tx!(
        "Filling pressure in atrial fibrillation",
        "Pressioni di riempimento in fibrillazione atriale"
    ),
    synonyms: tx!(
        "AF, atrial fibrillation, LAP, E velocity, septal E/e', deceleration time, LARS, BMI, algorithm",
        "FA, fibrillazione atriale, pressione atriale sinistra, velocità E, E/e' settale, tempo di decelerazione, LARS, BMI, algoritmo"
    ),
    summary: tx!(
        "The 2025 ASE two-step algorithm for atrial fibrillation: four Doppler criteria, then LARS, pulmonary vein S/D and BMI when exactly two are met. Average several cycles that reflect the mean heart rate.",
        "L'algoritmo ASE 2025 in due passi per la fibrillazione atriale: quattro criteri Doppler, poi LARS, S/D delle vene polmonari e BMI quando ne sono soddisfatti esattamente due. Mediare più cicli rappresentativi della frequenza media."
    ),
    inputs: &[
        E,
        E_SEPTAL,
        TR,
        PASP,
        DT,
        LARS,
        PV_SD,
        opt(
            "bmi_kg_m2",
            tx!("Body mass index", "Indice di massa corporea"),
            Unit::KgM2,
            12.0,
            80.0,
            1,
        ),
    ],
    compute: atrial_fibrillation,
    references: &[cite(&DD_2025, "Figure 8, p. 563; Table 7 row 1, p. 557")],
    review: Review::Verified,
    example: &[
        ("e_cm_s", N(105.0)),
        ("e_septal_cm_s", N(8.0)),
        ("tr_vmax_m_s", N(2.5)),
        ("dt_ms", N(190.0)),
        ("lars_percent", N(14.0)),
        ("pv_sd_ratio", N(0.8)),
        ("bmi_kg_m2", N(27.0)),
    ],
    sim_link: None,
};

// =============================================================================================
// Moderate or severe mitral annular calcification: Figure 4

pub(crate) mod mac {
    #[allow(clippy::wildcard_imports)]
    use super::*;

    const F4: &str = "Figure 4, p. 558; Table 7 row 8, p. 557";

    pub static EA: Node = node("e-a", tx!("E/A ratio", "Rapporto E/A"), F4, &[0]);
    pub static IVRT: Node = node(
        "ivrt",
        tx!("With E/A 0.8-1.8, IVRT", "Con E/A 0,8-1,8, IVRT"),
        F4,
        &[1],
    );
    pub static NORMAL: Leaf = leaf(
        "normal",
        LeafKind::Result,
        Severity::Normal,
        tx!(
            "meets the figure's criteria for normal LAP",
            "soddisfa i criteri della figura per PAS normale"
        ),
        "E/A < 0.8, or E/A 0.8-1.8 with IVRT ≥ 80 ms",
        F4,
    );
    pub static ELEVATED: Leaf = leaf(
        "elevated",
        LeafKind::Result,
        Severity::Abnormal,
        tx!(
            "meets the figure's criteria for elevated LAP",
            "soddisfa i criteri della figura per PAS elevata"
        ),
        "E/A > 1.8, or E/A 0.8-1.8 with IVRT < 80 ms",
        F4,
    );
    pub static OPEN: Leaf = leaf(
        "open",
        LeafKind::Indeterminate,
        Severity::Indeterminate,
        tx!(
            "LAP indeterminate with these measurements",
            "PAS indeterminata con queste misure"
        ),
        "measurements left empty change the result",
        F4,
    );

    pub fn walk(f: &[u8]) -> Walk {
        let mut w = Walk::new(&OPEN);
        match f[0] {
            0 => {
                w.ask(&EA, tx!("< 0.8", "< 0,8"));
                w.end(&NORMAL)
            }
            2 => {
                w.ask(&EA, tx!("> 1.8", "> 1,8"));
                w.end(&ELEVATED)
            }
            _ => {
                w.ask(&EA, tx!("0.8-1.8", "0,8-1,8"));
                if f[1] == 1 {
                    w.ask(&IVRT, tx!("< 80 ms", "< 80 ms"));
                    w.end(&ELEVATED)
                } else {
                    w.ask(&IVRT, tx!("≥ 80 ms", "≥ 80 ms"));
                    w.end(&NORMAL)
                }
            }
        }
    }

    pub static TREE: Tree = Tree {
        calculator: "diastolic-mac",
        nodes: &[&EA, &IVRT],
        leaves: &[&NORMAL, &ELEVATED],
        open: &OPEN,
        arities: &[3, 2],
        walk,
    };
}

fn mitral_annular_calcification(i: &Inputs) -> Result<Outcome, Vec<InputError>> {
    let mut out = outcome();
    let ea = e_over_a(&mut out, i);
    let facts = [
        classes(ea, "E/A", Unit::None, 2, &[(Cmp::Lt, 0.8), (Cmp::Le, 1.8)]),
        Fact::of(&test("IVRT", Cmp::Lt, 80.0, Unit::Ms, 0), i.num("ivrt_ms")),
    ];
    run(&mut out, &mac::TREE, &facts);
    Ok(out)
}

/// Mean LAP with moderate or severe mitral annular calcification (Figure 4).
pub static MAC: Calculator = Calculator {
    id: "diastolic-mac",
    category: Category::FillingPressures,
    name: tx!(
        "Filling pressure with mitral annular calcification",
        "Pressioni di riempimento con calcificazione dell'anulus mitralico"
    ),
    synonyms: tx!(
        "MAC, mitral annular calcification, LAP, E/A, IVRT, algorithm",
        "MAC, calcificazione dell'anulus mitralico, pressione atriale sinistra, E/A, IVRT, algoritmo"
    ),
    summary: tx!(
        "With moderate or severe MAC the E/e' ratio is unreliable; the 2025 ASE algorithm uses the E/A ratio and, between 0.8 and 1.8, the IVRT.",
        "Con MAC moderata o grave il rapporto E/e' non è affidabile; l'algoritmo ASE 2025 usa il rapporto E/A e, fra 0,8 e 1,8, l'IVRT."
    ),
    inputs: &[E, A, IVRT],
    compute: mitral_annular_calcification,
    references: &[cite(&DD_2025, "Figure 4 and text, p. 558; Table 7 row 8, p. 557")],
    review: Review::Verified,
    example: &[("e_cm_s", N(110.0)), ("a_cm_s", N(90.0)), ("ivrt_ms", N(68.0))],
    sim_link: None,
};

// =============================================================================================
// Pulmonary hypertension: Figure 6

pub(crate) mod ph {
    #[allow(clippy::wildcard_imports)]
    use super::*;

    const F6: &str = "Figure 6, p. 560";

    pub static EA: Node = node(
        "e-a",
        tx!("E/A ratio and E velocity", "Rapporto E/A e velocità E"),
        F6,
        &[0, 1],
    );
    pub static E_PRIME: Node = node(
        "e-prime",
        tx!("With E/A ≥ 2, e' reduced", "Con E/A ≥ 2, e' ridotta"),
        F6,
        &[2],
    );
    pub static LARS: Node = node("lars", tx!("LARS", "LARS"), F6, &[3]);
    pub static LATERAL: Node = node(
        "lateral-e-e",
        tx!("Lateral E/e' ratio", "Rapporto E/e' laterale"),
        F6,
        &[4],
    );
    pub static NORMAL: Leaf = leaf(
        "normal",
        LeafKind::Result,
        Severity::Normal,
        tx!(
            "meets the figure's criteria for normal LAP",
            "soddisfa i criteri della figura per PAS normale"
        ),
        "E/A ≤ 0.8 with E ≤ 50 cm/s; or LARS > 18%; or lateral E/e' < 8",
        F6,
    );
    pub static ELEVATED: Leaf = leaf(
        "elevated",
        LeafKind::Result,
        Severity::Abnormal,
        tx!(
            "meets the figure's criteria for elevated LAP",
            "soddisfa i criteri della figura per PAS elevata"
        ),
        "E/A ≥ 2 with reduced e'; or LARS ≤ 18%; or lateral E/e' > 13",
        F6,
    );
    pub static INDETERMINATE: Leaf = leaf(
        "indeterminate",
        LeafKind::Indeterminate,
        Severity::Indeterminate,
        tx!(
            "the figure's indeterminate LAP (lateral E/e' between 8 and 13)",
            "PAS indeterminata secondo la figura (E/e' laterale fra 8 e 13)"
        ),
        "LARS not available, 8 ≤ lateral E/e' ≤ 13",
        F6,
    );
    pub static OUTSIDE: Leaf = leaf(
        "outside",
        LeafKind::OutOfScope,
        Severity::Indeterminate,
        tx!(
            "the figure has no branch for E/A ≥ 2 without reduced e'",
            "la figura non ha un ramo per E/A ≥ 2 senza e' ridotta"
        ),
        "E/A ≥ 2, e' not reduced",
        F6,
    );
    pub static OPEN: Leaf = leaf(
        "open",
        LeafKind::Indeterminate,
        Severity::Indeterminate,
        tx!(
            "LAP indeterminate with these measurements",
            "PAS indeterminata con queste misure"
        ),
        "measurements left empty change the result",
        F6,
    );

    pub fn walk(f: &[u8]) -> Walk {
        let mut w = Walk::new(&OPEN);
        let (ea, e_high) = (f[0], f[1] == 1);
        if ea == 0 && !e_high {
            w.ask(&EA, tx!("E/A ≤ 0.8 and E ≤ 50 cm/s", "E/A ≤ 0,8 ed E ≤ 50 cm/s"));
            return w.end(&NORMAL);
        }
        if ea == 2 {
            w.ask(&EA, tx!("E/A ≥ 2", "E/A ≥ 2"));
            return if f[2] == 1 {
                w.ask(&E_PRIME, answers::YES);
                w.end(&ELEVATED)
            } else {
                w.ask(&E_PRIME, answers::NO);
                w.end(&OUTSIDE)
            };
        }
        w.ask(
            &EA,
            tx!(
                "E/A ≤ 0.8 with E > 50 cm/s, or E/A between 0.8 and 2",
                "E/A ≤ 0,8 con E > 50 cm/s, o E/A fra 0,8 e 2"
            ),
        );
        match f[3] {
            0 => {
                w.ask(&LARS, tx!("> 18%", "> 18%"));
                w.end(&NORMAL)
            }
            1 => {
                w.ask(&LARS, tx!("≤ 18%", "≤ 18%"));
                w.end(&ELEVATED)
            }
            _ => {
                w.ask(&LARS, tx!("not available", "non disponibile"));
                match f[4] {
                    0 => {
                        w.ask(&LATERAL, tx!("< 8", "< 8"));
                        w.end(&NORMAL)
                    }
                    2 => {
                        w.ask(&LATERAL, tx!("> 13", "> 13"));
                        w.end(&ELEVATED)
                    }
                    _ => {
                        w.ask(&LATERAL, tx!("8-13", "8-13"));
                        w.end(&INDETERMINATE)
                    }
                }
            }
        }
    }

    pub static TREE: Tree = Tree {
        calculator: "diastolic-pulmonary-hypertension",
        nodes: &[&EA, &E_PRIME, &LARS, &LATERAL],
        leaves: &[&NORMAL, &ELEVATED, &INDETERMINATE, &OUTSIDE],
        open: &OPEN,
        arities: &[3, 2, 2, 3, 3],
        walk,
    };
}

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

fn pulmonary_hypertension(i: &Inputs) -> Result<Outcome, Vec<InputError>> {
    let mut out = outcome();
    let ea = e_over_a(&mut out, i);
    let r = ratios(&mut out, i);
    let lars = i.num("lars_percent");
    let lars_fact = {
        let t = test("LARS", Cmp::Le, 18.0, Unit::Percent, 0);
        Fact::class(Some(t.eval(lars).map_or(2, u8::from)), 3, t.show(lars))
    };
    let facts = [
        classes(ea, "E/A", Unit::None, 2, &[(Cmp::Le, 0.8), (Cmp::Lt, 2.0)]),
        Fact::of(&test("E", Cmp::Gt, 50.0, Unit::CmS, 0), i.num("e_cm_s")),
        Fact::flag(
            i.choice("e_prime_reduced").map(|c| c == "yes"),
            format!("e' reduced = {}", i.choice("e_prime_reduced").unwrap_or("?")),
        ),
        lars_fact,
        classes(
            r.lateral,
            "lateral E/e'",
            Unit::None,
            1,
            &[(Cmp::Lt, 8.0), (Cmp::Le, 13.0)],
        ),
    ];
    run(&mut out, &ph::TREE, &facts);
    Ok(out)
}

/// Mean LAP in pulmonary hypertension (Figure 6).
pub static PULMONARY_HYPERTENSION: Calculator = Calculator {
    id: "diastolic-pulmonary-hypertension",
    category: Category::FillingPressures,
    name: tx!(
        "Filling pressure in pulmonary hypertension",
        "Pressioni di riempimento nell'ipertensione polmonare"
    ),
    synonyms: tx!(
        "PH, pulmonary hypertension, pre-capillary, post-capillary, group II, LAP, lateral E/e', LARS, algorithm",
        "IP, ipertensione polmonare, pre-capillare, post-capillare, gruppo II, pressione atriale sinistra, E/e' laterale, LARS, algoritmo"
    ),
    summary: tx!(
        "In pulmonary hypertension septal motion makes the septal e' unreliable: the 2025 ASE algorithm starts from E/A and E, then uses LARS or, if not available, the lateral E/e'.",
        "Nell'ipertensione polmonare il movimento del setto rende inaffidabile la e' settale: l'algoritmo ASE 2025 parte da E/A ed E, poi usa il LARS o, se non disponibile, l'E/e' laterale."
    ),
    inputs: &[
        E,
        A,
        opt_choice(
            "e_prime_reduced",
            tx!(
                "e' reduced (cut-offs of Figure 3 or Table 6)",
                "e' ridotta (soglie della Figura 3 o della Tabella 6)"
            ),
            YES_NO,
        ),
        LARS,
        E_LATERAL,
    ],
    compute: pulmonary_hypertension,
    references: &[cite(&DD_2025, "Figure 6 and text, p. 560; Table 7 row 5, p. 557")],
    review: Review::Verified,
    example: &[
        ("e_cm_s", N(70.0)),
        ("a_cm_s", N(60.0)),
        ("e_lateral_cm_s", N(5.0)),
    ],
    sim_link: None,
};

// =============================================================================================
// Heart transplant: Figure 5

pub(crate) mod htx {
    #[allow(clippy::wildcard_imports)]
    use super::*;

    const F5: &str = "Figure 5 and text, p. 559; Table 7 row 10, p. 557";

    pub static AVERAGE: Node = node("average-e-e", tx!("Average E/e'", "E/e' media"), F5, &[0]);
    pub static SR: Node = node(
        "e-sr-ivr",
        tx!(
            "E/SRIVR (strain rate during isovolumic relaxation, three apical views)",
            "E/SRIVR (strain rate durante il rilasciamento isovolumetrico, tre viste apicali)"
        ),
        F5,
        &[1],
    );
    pub static TR: Node = node(
        "tr",
        tx!(
            "Without E/SRIVR, peak TR velocity",
            "Senza E/SRIVR, velocità massima del RT"
        ),
        F5,
        &[2],
    );
    pub static NORMAL: Leaf = leaf(
        "normal",
        LeafKind::Result,
        Severity::Normal,
        tx!(
            "meets the figure's criteria for normal LAP (< 15 mmHg)",
            "soddisfa i criteri della figura per PAS normale (< 15 mmHg)"
        ),
        "average E/e' < 7; or E/SRIVR ≤ 200 cm; or TR ≤ 2.8 m/s",
        F5,
    );
    pub static ELEVATED: Leaf = leaf(
        "elevated",
        LeafKind::Result,
        Severity::Abnormal,
        tx!(
            "meets the figure's criteria for elevated LAP (≥ 15 mmHg)",
            "soddisfa i criteri della figura per PAS elevata (≥ 15 mmHg)"
        ),
        "average E/e' > 14; or E/SRIVR > 200 cm; or TR > 2.8 m/s",
        F5,
    );
    pub static INDETERMINATE: Leaf = leaf(
        "indeterminate",
        LeafKind::Indeterminate,
        Severity::Indeterminate,
        tx!(
            "the figure's indeterminate LAP (TR jet absent or incomplete)",
            "PAS indeterminata secondo la figura (getto del RT assente o incompleto)"
        ),
        "E/SRIVR not measurable, TR absent or incomplete",
        F5,
    );
    pub static OPEN: Leaf = leaf(
        "open",
        LeafKind::Indeterminate,
        Severity::Indeterminate,
        tx!(
            "LAP indeterminate with these measurements",
            "PAS indeterminata con queste misure"
        ),
        "measurements left empty change the result",
        F5,
    );

    pub fn walk(f: &[u8]) -> Walk {
        let mut w = Walk::new(&OPEN);
        match f[0] {
            0 => {
                w.ask(&AVERAGE, tx!("< 7", "< 7"));
                return w.end(&NORMAL);
            }
            2 => {
                w.ask(&AVERAGE, tx!("> 14", "> 14"));
                return w.end(&ELEVATED);
            }
            _ => w.ask(&AVERAGE, tx!("7-14", "7-14")),
        }
        match f[1] {
            0 => {
                w.ask(&SR, tx!("≤ 200 cm", "≤ 200 cm"));
                return w.end(&NORMAL);
            }
            1 => {
                w.ask(&SR, tx!("> 200 cm", "> 200 cm"));
                return w.end(&ELEVATED);
            }
            _ => w.ask(&SR, tx!("cannot be measured", "non misurabile")),
        }
        match f[2] {
            0 => {
                w.ask(&TR, tx!("≤ 2.8 m/s", "≤ 2,8 m/s"));
                w.end(&NORMAL)
            }
            1 => {
                w.ask(&TR, tx!("> 2.8 m/s", "> 2,8 m/s"));
                w.end(&ELEVATED)
            }
            _ => {
                w.ask(&TR, tx!("absent or incomplete", "assente o incompleto"));
                w.end(&INDETERMINATE)
            }
        }
    }

    pub static TREE: Tree = Tree {
        calculator: "diastolic-heart-transplant",
        nodes: &[&AVERAGE, &SR, &TR],
        leaves: &[&NORMAL, &ELEVATED, &INDETERMINATE],
        open: &OPEN,
        arities: &[3, 3, 3],
        walk,
    };
}

fn heart_transplant(i: &Inputs) -> Result<Outcome, Vec<InputError>> {
    let mut out = outcome();
    let r = ratios(&mut out, i);
    let e_sr = match (i.num("e_cm_s"), i.num("sr_ivr_per_s")) {
        (Some(e), Some(sr)) => {
            out.push(
                q("e_sr_ivr_cm", tx!("E/SRIVR", "E/SRIVR"), e / sr, Unit::Cm, 0),
                "E ÷ SRIVR",
                format!("{} ÷ {}", v(e, 0, Unit::CmS), v(sr, 2, Unit::PerS)),
            );
            Some(e / sr)
        }
        _ => None,
    };
    let sr_t = test("E/SRIVR", Cmp::Gt, 200.0, Unit::Cm, 0);
    let tr = i.num("tr_vmax_m_s");
    let tr_t = test("TR", Cmp::Gt, 2.8, Unit::MS, 2);
    let facts = [
        classes(
            r.average,
            "average E/e'",
            Unit::None,
            1,
            &[(Cmp::Lt, 7.0), (Cmp::Le, 14.0)],
        ),
        Fact::class(Some(sr_t.eval(e_sr).map_or(2, u8::from)), 3, sr_t.show(e_sr)),
        Fact::class(Some(tr_t.eval(tr).map_or(2, u8::from)), 3, tr_t.show(tr)),
    ];
    run(&mut out, &htx::TREE, &facts);
    Ok(out)
}

/// Mean LAP in heart transplant recipients in sinus rhythm (Figure 5).
pub static HEART_TRANSPLANT: Calculator = Calculator {
    id: "diastolic-heart-transplant",
    category: Category::FillingPressures,
    name: tx!(
        "Filling pressure after heart transplant",
        "Pressioni di riempimento dopo trapianto cardiaco"
    ),
    synonyms: tx!(
        "HTX, heart transplant, LAP, average E/e', SRIVR, strain rate, TR, algorithm",
        "trapianto cardiaco, pressione atriale sinistra, E/e' media, SRIVR, strain rate, RT, algoritmo"
    ),
    summary: tx!(
        "The simplified 2025 ASE approach for transplant recipients in sinus rhythm: average E/e', then E over the isovolumic-relaxation strain rate, then the TR velocity. Leave SRIVR empty if it cannot be measured, and the TR velocity empty if the jet is absent or incomplete.",
        "L'approccio semplificato ASE 2025 per i trapiantati in ritmo sinusale: E/e' media, poi E sullo strain rate del rilasciamento isovolumetrico, poi la velocità del RT. Lasciare vuoto SRIVR se non misurabile e la velocità del RT se il getto è assente o incompleto."
    ),
    inputs: &[
        E,
        E_SEPTAL,
        E_LATERAL,
        opt(
            "sr_ivr_per_s",
            tx!(
                "LV strain rate in isovolumic relaxation (SRIVR)",
                "Strain rate del VS nel rilasciamento isovolumetrico (SRIVR)"
            ),
            Unit::PerS,
            0.05,
            3.0,
            2,
        ),
        TR,
    ],
    compute: heart_transplant,
    references: &[cite(
        &DD_2025,
        "Figure 5 and text, p. 559; Table 7 row 10, p. 557",
    )],
    review: Review::Verified,
    example: &[
        ("e_cm_s", N(85.0)),
        ("e_septal_cm_s", N(8.0)),
        ("e_lateral_cm_s", N(10.0)),
        ("sr_ivr_per_s", N(0.35)),
    ],
    sim_link: None,
};

// =============================================================================================
// Diastolic exercise echocardiography: section 13C

pub(crate) mod exercise {
    #[allow(clippy::wildcard_imports)]
    use super::*;

    const S13: &str = "Section 13C and key points, p. 556";

    pub static EE: Node = node(
        "e-e",
        tx!("E/e' during exercise", "E/e' durante lo sforzo"),
        S13,
        &[0],
    );
    pub static TR: Node = node(
        "tr",
        tx!(
            "Peak TR velocity during exercise",
            "Velocità massima del RT durante lo sforzo"
        ),
        S13,
        &[1],
    );
    pub static POSITIVE: Leaf = leaf(
        "positive",
        LeafKind::Result,
        Severity::Abnormal,
        tx!(
            "meets the source's criteria for increased LV filling pressure with exercise (a positive test)",
            "soddisfa i criteri della fonte per pressioni di riempimento aumentate da sforzo (test positivo)"
        ),
        "average E/e' ≥ 14 (or septal ≥ 15) and TR ≥ 3.2 m/s",
        S13,
    );
    pub static NORMAL: Leaf = leaf(
        "normal",
        LeafKind::Result,
        Severity::Normal,
        tx!(
            "meets the source's criteria for normal filling pressure with exercise",
            "soddisfa i criteri della fonte per pressioni di riempimento normali da sforzo"
        ),
        "E/e' < 10 and TR < 2.8 m/s",
        S13,
    );
    pub static LIKELY: Leaf = leaf(
        "likely",
        LeafKind::Result,
        Severity::Abnormal,
        tx!(
            "meets the pattern in which the source considers HFpEF likely",
            "soddisfa il quadro in cui la fonte considera probabile lo scompenso a FE preservata"
        ),
        "E/e' ≥ 14 and 2.8 ≤ TR < 3.2 m/s",
        S13,
    );
    pub static NOT_SUPPORTED: Leaf = leaf(
        "not-supported",
        LeafKind::Result,
        Severity::Info,
        tx!(
            "the source states that an E/e' below 14 does not support HFpEF, even with TR > 3.2 m/s",
            "la fonte afferma che un E/e' sotto 14 non sostiene lo scompenso a FE preservata, anche con RT > 3,2 m/s"
        ),
        "E/e' < 14 and TR ≥ 3.2 m/s",
        S13,
    );
    pub static INDETERMINATE: Leaf = leaf(
        "indeterminate",
        LeafKind::Indeterminate,
        Severity::Indeterminate,
        tx!(
            "neither the positive nor the normal criteria are met; the source suggests invasive testing when the clinical picture warrants",
            "non sono soddisfatti né i criteri positivi né quelli normali; la fonte suggerisce lo studio invasivo se il quadro clinico lo richiede"
        ),
        "other combinations",
        S13,
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
        S13,
    );

    pub fn walk(f: &[u8]) -> Walk {
        let mut w = Walk::new(&OPEN);
        let (ee, tr) = (f[0], f[1]);
        w.ask(
            &EE,
            match ee {
                0 => tx!("< 10", "< 10"),
                1 => tx!("10 to below the cut-off", "da 10 a sotto la soglia"),
                _ => tx!(
                    "at or above the cut-off (average 14, septal 15)",
                    "pari o sopra la soglia (media 14, settale 15)"
                ),
            },
        );
        w.ask(
            &TR,
            match tr {
                0 => tx!("< 2.8 m/s", "< 2,8 m/s"),
                1 => tx!("2.8 to below 3.2 m/s", "da 2,8 a sotto 3,2 m/s"),
                _ => tx!("≥ 3.2 m/s", "≥ 3,2 m/s"),
            },
        );
        w.end(match (ee, tr) {
            (2, 2) => &POSITIVE,
            (0, 0) => &NORMAL,
            (2, 1) => &LIKELY,
            (_, 2) => &NOT_SUPPORTED,
            _ => &INDETERMINATE,
        })
    }

    pub static TREE: Tree = Tree {
        calculator: "diastolic-exercise",
        nodes: &[&EE, &TR],
        leaves: &[&POSITIVE, &NORMAL, &LIKELY, &NOT_SUPPORTED, &INDETERMINATE],
        open: &OPEN,
        arities: &[3, 3],
        walk,
    };
}

fn exercise_echo(i: &Inputs) -> Result<Outcome, Vec<InputError>> {
    let mut out = outcome();
    // the average when given, otherwise the septal ratio with its own cut-off
    let (value, top, symbol) = match (i.num("e_e_average_peak"), i.num("e_e_septal_peak")) {
        (Some(a), _) => (Some(a), 14.0, "average E/e'"),
        (None, Some(s)) => (Some(s), 15.0, "septal E/e'"),
        (None, None) => (None, 14.0, "E/e'"),
    };
    if let Some(x) = value {
        out.push(
            q("e_e_exercise", tx!("E/e' used", "E/e' usato"), x, Unit::None, 1),
            "E/e' (average if given, else septal)",
            n(x, 1),
        );
    }
    if let Some(tr) = i.num("tr_vmax_peak_m_s") {
        let pg = crate::blocks::bernoulli_mmhg(tr);
        out.push(
            q(
                "tr_gradient_mmhg",
                tx!("Peak TR gradient", "Gradiente massimo del RT"),
                pg,
                Unit::MmHg,
                0,
            ),
            "ΔP = 4v²",
            format!("4 × {}²", n(tr, 2)),
        );
    }
    let facts = [
        classes(value, symbol, Unit::None, 1, &[(Cmp::Lt, 10.0), (Cmp::Lt, top)]),
        classes(
            i.num("tr_vmax_peak_m_s"),
            "TR",
            Unit::MS,
            2,
            &[(Cmp::Lt, 2.8), (Cmp::Lt, 3.2)],
        ),
    ];
    run(&mut out, &exercise::TREE, &facts);
    Ok(out)
}

/// Diastolic exercise echocardiography (section 13C).
pub static EXERCISE: Calculator = Calculator {
    id: "diastolic-exercise",
    category: Category::FillingPressures,
    name: tx!(
        "Diastolic exercise echocardiography",
        "Ecocardiografia diastolica da sforzo"
    ),
    synonyms: tx!(
        "diastolic stress test, exercise, supine bicycle, treadmill, E/e', TR velocity, HFpEF",
        "eco stress diastolico, sforzo, cicloergometro supino, treadmill, E/e', velocità del RT, scompenso a FE preservata"
    ),
    summary: tx!(
        "Interprets E/e' and the peak TR velocity recorded during exercise (or 1-2 minutes after, when E and A are not merged) by the criteria of the 2025 ASE recommendation. Enter the average E/e' if available, otherwise the septal one.",
        "Interpreta E/e' e la velocità massima del RT registrate durante lo sforzo (o 1-2 minuti dopo, quando E e A non sono fuse) secondo i criteri della raccomandazione ASE 2025. Inserire l'E/e' medio se disponibile, altrimenti quello settale."
    ),
    inputs: &[
        opt(
            "e_e_average_peak",
            tx!("Average E/e' during exercise", "E/e' medio durante lo sforzo"),
            Unit::None,
            2.0,
            50.0,
            1,
        ),
        opt(
            "e_e_septal_peak",
            tx!("Septal E/e' during exercise", "E/e' settale durante lo sforzo"),
            Unit::None,
            2.0,
            50.0,
            1,
        ),
        opt(
            "tr_vmax_peak_m_s",
            tx!(
                "Peak TR velocity during exercise",
                "Velocità massima del RT durante lo sforzo"
            ),
            Unit::MS,
            0.5,
            6.0,
            2,
        ),
    ],
    compute: exercise_echo,
    references: &[cite(&DD_2025, "Section 13C and key points, pp. 555-556")],
    review: Review::Verified,
    example: &[("e_e_average_peak", N(15.5)), ("tr_vmax_peak_m_s", N(3.3))],
    sim_link: None,
};

// =============================================================================================
// Table 7: lists of indicators (mitral stenosis, MR, HCM, restrictive cardiomyopathy, sinus
// tachycardia, LVAD). The table gives indicators of elevated pressure and no rule for normal
// pressure, so a tree of one question: is at least one indicator met?

/// A one-question tree over a list of indicators.
macro_rules! indicator_tree {
    ($modname:ident, $calc:expr, $locator:expr, $question:expr, $uses:expr, $arities:expr) => {
        pub(crate) mod $modname {
            #[allow(clippy::wildcard_imports)]
            use super::*;

            pub static ANY: Node = node(
                "indicators",
                $question,
                $locator,
                $uses,
            );
            pub static MET: Leaf = leaf(
                "met",
                LeafKind::Result,
                Severity::Abnormal,
                tx!(
                    "meets at least one of the source's indicators of elevated LV filling pressure in this population",
                    "soddisfa almeno uno degli indicatori di pressioni di riempimento elevate che la fonte dà per questa popolazione"
                ),
                "at least one indicator met",
                $locator,
            );
            pub static NONE: Leaf = leaf(
                "none",
                LeafKind::Result,
                Severity::Info,
                tx!(
                    "meets none of the source's indicators of elevated LV filling pressure (the table gives no criterion that establishes normal pressure)",
                    "non soddisfa nessuno degli indicatori di pressioni di riempimento elevate della fonte (la tabella non dà un criterio che stabilisca pressioni normali)"
                ),
                "no indicator met, all measured",
                $locator,
            );
            pub static OPEN: Leaf = leaf(
                "open",
                LeafKind::Indeterminate,
                Severity::Indeterminate,
                tx!(
                    "indeterminate: no indicator met among those measured",
                    "indeterminato: nessun indicatore soddisfatto fra quelli misurati"
                ),
                "measurements left empty could meet an indicator",
                $locator,
            );

            pub fn walk(f: &[u8]) -> Walk {
                let mut w = Walk::new(&OPEN);
                if f.iter().any(|x| *x == 1) {
                    w.ask(&ANY, tx!("at least one met", "almeno uno soddisfatto"));
                    w.end(&MET)
                } else {
                    w.ask(&ANY, tx!("none met", "nessuno soddisfatto"));
                    w.end(&NONE)
                }
            }

            pub static TREE: Tree = Tree {
                calculator: $calc,
                nodes: &[&ANY],
                leaves: &[&MET, &NONE],
                open: &OPEN,
                arities: $arities,
                walk,
            };
        }
    };
}

indicator_tree!(
    ms,
    "diastolic-mitral-stenosis",
    "Table 7 row 6, p. 557; text, pp. 557-558",
    tx!(
        "Indicators: IVRT < 60 ms (specific, not sensitive), mitral A > 1.5 m/s, IVRT/TE-e' < 4.2",
        "Indicatori: IVRT < 60 ms (specifico, non sensibile), A mitralica > 1,5 m/s, IVRT/TE-e' < 4,2"
    ),
    &[0, 1, 2],
    &[2, 2, 2]
);
indicator_tree!(
    mr,
    "diastolic-mitral-regurgitation",
    "Table 7 row 7, p. 557; text and key points, p. 558",
    tx!(
        "Indicators: IVRT < 60 ms (specific, not sensitive), Ar − A ≥ 30 ms, IVRT/TE-e' < 5.6, average E/e' > 14 with reduced EF",
        "Indicatori: IVRT < 60 ms (specifico, non sensibile), Ar − A ≥ 30 ms, IVRT/TE-e' < 5,6, E/e' medio > 14 con FE ridotta"
    ),
    &[0, 1, 2, 3],
    &[2, 2, 2, 2]
);
indicator_tree!(
    hcm,
    "diastolic-hcm",
    "Table 7 row 3, p. 557; HCM section, p. 562",
    tx!(
        "Indicators: average E/e' > 14, Ar − A ≥ 30 ms, TR > 2.8 m/s, LAVi > 34 mL/m²",
        "Indicatori: E/e' medio > 14, Ar − A ≥ 30 ms, RT > 2,8 m/s, LAVi > 34 mL/m²"
    ),
    &[0, 1, 2, 3],
    &[2, 2, 2, 2]
);
indicator_tree!(
    restrictive,
    "diastolic-restrictive",
    "Table 7 row 4, p. 557",
    tx!(
        "Indicators: average E/e' > 14, DT < 140 ms, E/A > 2.5, IVRT < 50 ms (the last three specific, not sensitive)",
        "Indicatori: E/e' medio > 14, DT < 140 ms, E/A > 2,5, IVRT < 50 ms (gli ultimi tre specifici, non sensibili)"
    ),
    &[0, 1, 2, 3],
    &[2, 2, 2, 2]
);
indicator_tree!(
    tachy,
    "diastolic-sinus-tachycardia",
    "Table 7 row 2, p. 557",
    tx!(
        "Indicators: predominant early filling with reduced EF, IVRT ≤ 70 ms, PV systolic filling fraction ≤ 40%, average E/e' > 14",
        "Indicatori: riempimento protodiastolico predominante con FE ridotta, IVRT ≤ 70 ms, frazione di riempimento sistolica venosa ≤ 40%, E/e' medio > 14"
    ),
    &[0, 1, 2, 3],
    &[2, 2, 2, 2]
);
indicator_tree!(
    lvad,
    "diastolic-lvad",
    "Table 7 row 9, p. 557",
    tx!(
        "Indicators: E/A > 2, RAP > 10 mmHg, PASP > 40 mmHg, average E/e' > 14 or septal E/e' ≥ 15, LAVi > 33 mL/m²",
        "Indicatori: E/A > 2, PAD > 10 mmHg, PAPs > 40 mmHg, E/e' medio > 14 o E/e' settale ≥ 15, LAVi > 33 mL/m²"
    ),
    &[0, 1, 2, 3, 4],
    &[2, 2, 2, 2, 2]
);

/// Runs an indicator tree; refuses an evaluation with nothing measured.
fn indicators(
    out: &mut Outcome,
    t: &Tree,
    facts: &[Fact],
    first: &'static str,
) -> Result<(), Vec<InputError>> {
    if facts.iter().all(|f| f.value.is_none()) {
        return Err(need_one(first));
    }
    push_count(out, facts);
    run(out, t, facts);
    Ok(())
}

fn mitral_stenosis(i: &Inputs) -> Result<Outcome, Vec<InputError>> {
    let mut out = outcome();
    let ratio = ivrt_over_te(&mut out, i);
    let facts = [
        Fact::of(&test("IVRT", Cmp::Lt, 60.0, Unit::Ms, 0), i.num("ivrt_ms")),
        Fact::of(&test("A", Cmp::Gt, 1.5, Unit::MS, 2), i.num("a_m_s")),
        Fact::of(&test("IVRT/TE-e'", Cmp::Lt, 4.2, Unit::None, 1), ratio),
    ];
    indicators(&mut out, &ms::TREE, &facts, "ivrt_ms")?;
    Ok(out)
}

/// Indicators of elevated filling pressure with mitral stenosis (Table 7 row 6).
pub static MITRAL_STENOSIS: Calculator = Calculator {
    id: "diastolic-mitral-stenosis",
    category: Category::FillingPressures,
    name: tx!(
        "Filling pressure with mitral stenosis",
        "Pressioni di riempimento con stenosi mitralica"
    ),
    synonyms: tx!(
        "MS, mitral stenosis, LAP, IVRT, TE-e', mitral A velocity",
        "SM, stenosi mitralica, pressione atriale sinistra, IVRT, TE-e', velocità A mitralica"
    ),
    summary: tx!(
        "With mitral stenosis E/e' is not useful; the 2025 ASE recommendation lists a short IVRT, a high mitral A velocity and a low IVRT/TE-e' ratio as indicators of elevated filling pressure.",
        "Con la stenosi mitralica E/e' non è utile; la raccomandazione ASE 2025 elenca un IVRT breve, una velocità A mitralica elevata e un basso rapporto IVRT/TE-e' come indicatori di pressioni di riempimento elevate."
    ),
    inputs: &[
        IVRT,
        opt(
            "a_m_s",
            tx!("Mitral A peak velocity", "Velocità massima dell'onda A mitralica"),
            Unit::MS,
            0.1,
            3.5,
            2,
        ),
        TE_E,
    ],
    compute: mitral_stenosis,
    references: &[cite(&DD_2025, "Table 7 row 6, p. 557; text, pp. 557-558")],
    review: Review::Verified,
    example: &[("ivrt_ms", N(55.0)), ("a_m_s", N(1.6)), ("te_e_ms", N(18.0))],
    sim_link: None,
};

const EF_CLASS: &[Choice] = &[
    Choice {
        id: "reduced",
        label: tx!("reduced", "ridotta"),
    },
    Choice {
        id: "normal",
        label: tx!("normal", "normale"),
    },
];

fn mitral_regurgitation(i: &Inputs) -> Result<Outcome, Vec<InputError>> {
    let mut out = outcome();
    let ratio = ivrt_over_te(&mut out, i);
    let ar_a = ar_minus_a(&mut out, i);
    let reduced = i.choice("ef") == Some("reduced");
    let r = if reduced {
        ratios(&mut out, i)
    } else {
        Ratios::default()
    };
    let ee = test("average E/e' (reduced EF)", Cmp::Gt, 14.0, Unit::None, 1);
    let ee_fact = if reduced {
        Fact::of(&ee, r.average)
    } else {
        // not an indicator with a normal EF: counts as not met
        Fact::flag(Some(false), "E/e': not used with a normal EF".to_owned())
    };
    let facts = [
        Fact::of(&test("IVRT", Cmp::Lt, 60.0, Unit::Ms, 0), i.num("ivrt_ms")),
        Fact::of(&test("Ar − A", Cmp::Ge, 30.0, Unit::Ms, 0), ar_a),
        Fact::of(&test("IVRT/TE-e'", Cmp::Lt, 5.6, Unit::None, 1), ratio),
        ee_fact,
    ];
    if facts[..3].iter().all(|f| f.value.is_none()) && (!reduced || r.average.is_none()) {
        return Err(need_one("ivrt_ms"));
    }
    push_count(&mut out, &facts);
    run(&mut out, &mr::TREE, &facts);
    Ok(out)
}

/// Indicators of elevated filling pressure with mitral regurgitation (Table 7 row 7).
pub static MITRAL_REGURGITATION: Calculator = Calculator {
    id: "diastolic-mitral-regurgitation",
    category: Category::FillingPressures,
    name: tx!(
        "Filling pressure with mitral regurgitation",
        "Pressioni di riempimento con insufficienza mitralica"
    ),
    synonyms: tx!(
        "MR, mitral regurgitation, LAP, IVRT, TE-e', Ar-A, E/e'",
        "IM, insufficienza mitralica, pressione atriale sinistra, IVRT, TE-e', Ar-A, E/e'"
    ),
    summary: tx!(
        "With significant MR the transmitral and venous patterns mimic advanced dysfunction. The 2025 ASE recommendation lists IVRT, Ar - A, IVRT/TE-e' and, only with a reduced EF, the average E/e' as indicators of elevated filling pressure.",
        "Con un'IM significativa i flussi transmitralico e venoso simulano una disfunzione avanzata. La raccomandazione ASE 2025 elenca IVRT, Ar - A, IVRT/TE-e' e, solo con FE ridotta, l'E/e' medio come indicatori di pressioni di riempimento elevate."
    ),
    inputs: &[
        super::choice(
            "ef",
            tx!("LV ejection fraction", "Frazione di eiezione del VS"),
            EF_CLASS,
        ),
        IVRT,
        TE_E,
        AR_DUR,
        A_DUR,
        when(E, "ef", &["reduced"]),
        when(E_SEPTAL, "ef", &["reduced"]),
        when(E_LATERAL, "ef", &["reduced"]),
    ],
    compute: mitral_regurgitation,
    references: &[cite(
        &DD_2025,
        "Table 7 row 7, p. 557; text and key points, p. 558",
    )],
    review: Review::Verified,
    example: &[
        ("ef", C("normal")),
        ("ivrt_ms", N(62.0)),
        ("te_e_ms", N(14.0)),
        ("ar_duration_ms", N(170.0)),
        ("a_duration_ms", N(130.0)),
    ],
    sim_link: None,
};

fn hcm(i: &Inputs) -> Result<Outcome, Vec<InputError>> {
    let mut out = outcome();
    let r = ratios(&mut out, i);
    let ar_a = ar_minus_a(&mut out, i);
    let facts = [
        Fact::of(&test("average E/e'", Cmp::Gt, 14.0, Unit::None, 1), r.average),
        Fact::of(&test("Ar − A", Cmp::Ge, 30.0, Unit::Ms, 0), ar_a),
        Fact::of(&test("TR", Cmp::Gt, 2.8, Unit::MS, 2), i.num("tr_vmax_m_s")),
        Fact::of(&test("LAVi", Cmp::Gt, 34.0, Unit::MlM2, 0), i.num("lavi_ml_m2")),
    ];
    indicators(&mut out, &hcm::TREE, &facts, "e_cm_s")?;
    Ok(out)
}

/// Indicators of elevated filling pressure in hypertrophic cardiomyopathy (Table 7 row 3).
pub static HCM: Calculator = Calculator {
    id: "diastolic-hcm",
    category: Category::FillingPressures,
    name: tx!(
        "Filling pressure in hypertrophic cardiomyopathy",
        "Pressioni di riempimento nella cardiomiopatia ipertrofica"
    ),
    synonyms: tx!(
        "HCM, hypertrophic cardiomyopathy, LAP, E/e', Ar-A, TR, LAVi",
        "CMI, cardiomiopatia ipertrofica, pressione atriale sinistra, E/e', Ar-A, RT, LAVi"
    ),
    summary: tx!(
        "The indicators of elevated filling pressure the 2025 ASE recommendation lists for HCM: average E/e', Ar - A, TR velocity and LA volume index. Significant MR raises the LAP, LA volume and TR velocity by itself.",
        "Gli indicatori di pressioni di riempimento elevate che la raccomandazione ASE 2025 elenca per la CMI: E/e' medio, Ar - A, velocità del RT e volume atriale indicizzato. Un'IM significativa aumenta da sola PAS, volume atriale e velocità del RT."
    ),
    inputs: &[E, E_SEPTAL, E_LATERAL, AR_DUR, A_DUR, TR, LAVI],
    compute: hcm,
    references: &[cite(&DD_2025, "Table 7 row 3, p. 557; HCM section, p. 562")],
    review: Review::Verified,
    example: &[
        ("e_cm_s", N(95.0)),
        ("e_septal_cm_s", N(5.0)),
        ("e_lateral_cm_s", N(7.0)),
        ("tr_vmax_m_s", N(2.6)),
        ("lavi_ml_m2", N(40.0)),
    ],
    sim_link: None,
};

fn restrictive(i: &Inputs) -> Result<Outcome, Vec<InputError>> {
    let mut out = outcome();
    let r = ratios(&mut out, i);
    let ea = e_over_a(&mut out, i);
    let facts = [
        Fact::of(&test("average E/e'", Cmp::Gt, 14.0, Unit::None, 1), r.average),
        Fact::of(&test("DT", Cmp::Lt, 140.0, Unit::Ms, 0), i.num("dt_ms")),
        Fact::of(&test("E/A", Cmp::Gt, 2.5, Unit::None, 2), ea),
        Fact::of(&test("IVRT", Cmp::Lt, 50.0, Unit::Ms, 0), i.num("ivrt_ms")),
    ];
    indicators(&mut out, &restrictive::TREE, &facts, "e_cm_s")?;
    Ok(out)
}

/// Indicators of elevated filling pressure in restrictive cardiomyopathy (Table 7 row 4).
pub static RESTRICTIVE: Calculator = Calculator {
    id: "diastolic-restrictive",
    category: Category::FillingPressures,
    name: tx!(
        "Filling pressure in restrictive cardiomyopathy",
        "Pressioni di riempimento nella cardiomiopatia restrittiva"
    ),
    synonyms: tx!(
        "restrictive cardiomyopathy, amyloidosis, LAP, E/e', deceleration time, E/A, IVRT",
        "cardiomiopatia restrittiva, amiloidosi, pressione atriale sinistra, E/e', tempo di decelerazione, E/A, IVRT"
    ),
    summary: tx!(
        "The indicators of elevated filling pressure the 2025 ASE recommendation lists for restrictive cardiomyopathy: average E/e', a short deceleration time, a high E/A and a short IVRT (the last three specific but not sensitive).",
        "Gli indicatori di pressioni di riempimento elevate che la raccomandazione ASE 2025 elenca per la cardiomiopatia restrittiva: E/e' medio, tempo di decelerazione breve, E/A elevato e IVRT breve (gli ultimi tre specifici ma non sensibili)."
    ),
    inputs: &[E, A, E_SEPTAL, E_LATERAL, DT, IVRT],
    compute: restrictive,
    references: &[cite(
        &DD_2025,
        "Table 7 row 4, p. 557; restrictive cardiomyopathy, p. 561",
    )],
    review: Review::Verified,
    example: &[
        ("e_cm_s", N(100.0)),
        ("a_cm_s", N(35.0)),
        ("e_septal_cm_s", N(4.0)),
        ("e_lateral_cm_s", N(5.0)),
        ("dt_ms", N(130.0)),
        ("ivrt_ms", N(48.0)),
    ],
    sim_link: None,
};

fn sinus_tachycardia(i: &Inputs) -> Result<Outcome, Vec<InputError>> {
    let mut out = outcome();
    let r = ratios(&mut out, i);
    let early = i.choice("early_filling_reduced_ef");
    let facts = [
        Fact::flag(
            early.map(|c| c == "yes"),
            format!("early filling with reduced EF = {}", early.unwrap_or("?")),
        ),
        Fact::of(&test("IVRT", Cmp::Le, 70.0, Unit::Ms, 0), i.num("ivrt_ms")),
        Fact::of(
            &test("PV systolic filling fraction", Cmp::Le, 40.0, Unit::Percent, 0),
            i.num("pv_sff_percent"),
        ),
        Fact::of(&test("average E/e'", Cmp::Gt, 14.0, Unit::None, 1), r.average),
    ];
    indicators(&mut out, &tachy::TREE, &facts, "ivrt_ms")?;
    Ok(out)
}

/// Indicators of elevated filling pressure in sinus tachycardia (Table 7 row 2).
pub static SINUS_TACHYCARDIA: Calculator = Calculator {
    id: "diastolic-sinus-tachycardia",
    category: Category::FillingPressures,
    name: tx!(
        "Filling pressure in sinus tachycardia",
        "Pressioni di riempimento in tachicardia sinusale"
    ),
    synonyms: tx!(
        "sinus tachycardia, fused E and A, LAP, IVRT, pulmonary vein systolic filling fraction, E/e'",
        "tachicardia sinusale, E e A fuse, pressione atriale sinistra, IVRT, frazione di riempimento sistolica venosa, E/e'"
    ),
    summary: tx!(
        "The indicators of elevated filling pressure the 2025 ASE recommendation lists for sinus tachycardia. When E and A are fused, the beat after a premature beat often separates them.",
        "Gli indicatori di pressioni di riempimento elevate che la raccomandazione ASE 2025 elenca per la tachicardia sinusale. Quando E e A sono fuse, il battito dopo un'extrasistole spesso le separa."
    ),
    inputs: &[
        opt_choice(
            "early_filling_reduced_ef",
            tx!(
                "Predominant early filling with reduced EF",
                "Riempimento protodiastolico predominante con FE ridotta"
            ),
            YES_NO,
        ),
        IVRT,
        opt(
            "pv_sff_percent",
            tx!(
                "Pulmonary vein systolic filling fraction",
                "Frazione di riempimento sistolica delle vene polmonari"
            ),
            Unit::Percent,
            0.0,
            100.0,
            0,
        ),
        E,
        E_SEPTAL,
        E_LATERAL,
    ],
    compute: sinus_tachycardia,
    references: &[cite(&DD_2025, "Table 7 row 2, p. 557")],
    review: Review::Verified,
    example: &[
        ("early_filling_reduced_ef", C("no")),
        ("ivrt_ms", N(65.0)),
        ("pv_sff_percent", N(45.0)),
    ],
    sim_link: None,
};

const SEPTUM: &[Choice] = &[
    Choice {
        id: "neutral",
        label: tx!("neutral", "neutro"),
    },
    Choice {
        id: "right",
        label: tx!("bulging to the right", "bombato verso destra"),
    },
    Choice {
        id: "left",
        label: tx!("bulging to the left", "bombato verso sinistra"),
    },
];

fn lvad(i: &Inputs) -> Result<Outcome, Vec<InputError>> {
    let mut out = outcome();
    let ea = e_over_a(&mut out, i);
    let r = ratios(&mut out, i);
    let rap = i.num("rap_mmhg");
    if let (Some(rap), Some(pos)) = (rap, i.choice("septum")) {
        let (shift, text) = match pos {
            "right" => (5.0, "RAP + 5"),
            "left" => (-5.0, "RAP − 5"),
            _ => (0.0, "RAP"),
        };
        out.push(
            q(
                "lap_estimate_mmhg",
                tx!(
                    "LAP from the RAP and the septal position",
                    "PAS dalla PAD e dalla posizione del setto"
                ),
                rap + shift,
                Unit::MmHg,
                0,
            ),
            "LAP = RAP (neutral), RAP + 5 (to the right), RAP − 5 (to the left)",
            format!("{text} = {}", n(rap, 0)),
        );
    }
    let ee = Fact::any_measured(&[
        (test("average E/e'", Cmp::Gt, 14.0, Unit::None, 1), r.average),
        (test("septal E/e'", Cmp::Ge, 15.0, Unit::None, 1), r.septal),
    ]);
    let facts = [
        Fact::of(&test("E/A", Cmp::Gt, 2.0, Unit::None, 2), ea),
        Fact::of(&test("RAP", Cmp::Gt, 10.0, Unit::MmHg, 0), rap),
        Fact::of(&test("PASP", Cmp::Gt, 40.0, Unit::MmHg, 0), i.num("pasp_mmhg")),
        ee,
        Fact::of(&test("LAVi", Cmp::Gt, 33.0, Unit::MlM2, 0), i.num("lavi_ml_m2")),
    ];
    indicators(&mut out, &lvad::TREE, &facts, "e_cm_s")?;
    Ok(out)
}

/// Indicators of elevated filling pressure with an LV assist device (Table 7 row 9).
pub static LVAD: Calculator = Calculator {
    id: "diastolic-lvad",
    category: Category::FillingPressures,
    name: tx!(
        "Filling pressure with an LV assist device",
        "Pressioni di riempimento con assistenza ventricolare sinistra"
    ),
    synonyms: tx!(
        "LVAD, assist device, LAP, E/A, RAP, PASP, E/e', LAVi, interatrial septum",
        "LVAD, assistenza ventricolare, pressione atriale sinistra, E/A, PAD, PAPs, E/e', LAVi, setto interatriale"
    ),
    summary: tx!(
        "The indicators of elevated filling pressure the 2025 ASE recommendation lists for LVAD patients, and the LAP estimate from the RAP and the position of the interatrial septum (equal with a neutral septum, 5 mmHg higher when it bulges to the right, 5 mmHg lower when it bulges to the left).",
        "Gli indicatori di pressioni di riempimento elevate che la raccomandazione ASE 2025 elenca per i pazienti con LVAD, e la stima della PAS dalla PAD e dalla posizione del setto interatriale (uguale con setto neutro, 5 mmHg in più se bombato verso destra, 5 mmHg in meno se verso sinistra)."
    ),
    inputs: &[
        E,
        A,
        opt(
            "rap_mmhg",
            tx!("Right atrial pressure", "Pressione atriale destra"),
            Unit::MmHg,
            0.0,
            30.0,
            0,
        ),
        PASP,
        E_SEPTAL,
        E_LATERAL,
        LAVI,
        opt_choice("septum", tx!("Interatrial septum", "Setto interatriale"), SEPTUM),
    ],
    compute: lvad,
    references: &[cite(&DD_2025, "Table 7 row 9 and footnote, p. 557")],
    review: Review::Verified,
    example: &[
        ("e_cm_s", N(80.0)),
        ("a_cm_s", N(50.0)),
        ("rap_mmhg", N(8.0)),
        ("pasp_mmhg", N(38.0)),
        ("lavi_ml_m2", N(30.0)),
        ("e_septal_cm_s", N(7.0)),
        ("e_lateral_cm_s", N(9.0)),
        ("septum", C("neutral")),
    ],
    sim_link: None,
};

#[cfg(test)]
mod tests {
    use super::super::testing::ok;
    use super::*;

    fn leaf_of(c: &Calculator, i: &Inputs) -> &'static str {
        let t = tree::for_calculator(c.id).expect("a tree");
        let out = ok(c, i);
        let last = out.findings.last().expect("a result");
        // the last finding is a leaf of the tree (or its open end)
        tree::reached(t, last)
            .unwrap_or_else(|| panic!("{}: {:?}", c.id, last.text))
            .id
    }

    fn sinus(e: f64, a: f64, es: f64, el: f64, tr: f64) -> Inputs {
        Inputs::new()
            .with("e_cm_s", e)
            .with("a_cm_s", a)
            .with("e_septal_cm_s", es)
            .with("e_lateral_cm_s", el)
            .with("tr_vmax_m_s", tr)
    }

    #[test]
    fn sinus_rhythm_reaches_each_end_of_figure_3() {
        // e' 10/12 normal, E/e' 80/11 = 7.3 normal, TR 2.3 normal: normal DF
        assert_eq!(
            leaf_of(&SINUS_RHYTHM, &sinus(80.0, 60.0, 10.0, 12.0, 2.3)),
            "normal"
        );
        // e' 5/6 reduced, E 50: E/e' 9.1 normal, TR 2.3, E/A 50/80 = 0.63: grade 1
        assert_eq!(
            leaf_of(&SINUS_RHYTHM, &sinus(50.0, 80.0, 5.0, 6.0, 2.3)),
            "grade-1"
        );
        // all three: e' 5/6, E 100 → E/e' 18.2, TR 3.0; E/A 100/80 = 1.25 → grade 2
        assert_eq!(
            leaf_of(&SINUS_RHYTHM, &sinus(100.0, 80.0, 5.0, 6.0, 3.0)),
            "grade-2"
        );
        // E/A 100/45 = 2.2 → grade 3
        assert_eq!(
            leaf_of(&SINUS_RHYTHM, &sinus(100.0, 45.0, 5.0, 6.0, 3.0)),
            "grade-3"
        );
        // reduced e' only, E/A 70/60 > 0.8: additional variables; LARS 16 ≤ 18 → increased → grade 2
        let i = sinus(70.0, 60.0, 5.0, 6.0, 2.3).with("lars_percent", 16.0);
        assert_eq!(leaf_of(&SINUS_RHYTHM, &i), "grade-2");
        // … LARS 25 > 18, no other: normal LAP
        let i = sinus(70.0, 60.0, 5.0, 6.0, 2.3).with("lars_percent", 25.0);
        assert_eq!(leaf_of(&SINUS_RHYTHM, &i), "normal-lap");
        // … nothing additional: the supplemental methods decide; PR end-diastolic 2.1 m/s
        let i = sinus(70.0, 60.0, 5.0, 6.0, 2.3).with("pr_ed_m_s", 2.1);
        assert_eq!(leaf_of(&SINUS_RHYTHM, &i), "grade-2");
        // … nothing at all: indeterminate, normal or increased still possible
        let i = sinus(70.0, 60.0, 5.0, 6.0, 2.3);
        assert_eq!(leaf_of(&SINUS_RHYTHM, &i), "open");
    }

    #[test]
    fn sinus_rhythm_boundaries_fall_as_figure_3_prints_them() {
        // septal e' 6.0 is reduced (≤ 6), 6.1 is not
        let e_prime = |es: f64| {
            leaf_of(
                &SINUS_RHYTHM,
                &Inputs::new()
                    .with("e_cm_s", 50.0)
                    .with("a_cm_s", 80.0)
                    .with("e_septal_cm_s", es)
                    .with("tr_vmax_m_s", 2.0),
            )
        };
        assert_eq!(e_prime(6.0), "grade-1");
        assert_eq!(e_prime(5.9), "grade-1");
        assert_eq!(e_prime(6.1), "normal");
        // TR 2.8 counts (≥ 2.8), 2.79 does not
        let tr = |v: f64| leaf_of(&SINUS_RHYTHM, &sinus(100.0, 80.0, 5.0, 6.0, v));
        assert_eq!(tr(2.8), "grade-2");
        assert_eq!(tr(2.81), "grade-2");
        assert_eq!(
            tr(2.79),
            "open",
            "two of three: the additional variables are empty"
        );
        // E/A 0.8 is grade 1 (≤ 0.8); 0.81 goes on
        let ea = |a: f64| {
            leaf_of(
                &SINUS_RHYTHM,
                &sinus(56.0, a, 5.0, 6.0, 2.0).with("lars_percent", 30.0),
            )
        };
        assert_eq!(ea(70.0), "grade-1");
        assert_eq!(ea(56.0 / 0.79), "grade-1");
        assert_eq!(ea(56.0 / 0.81), "normal-lap");
        // E/A 2 is grade 3 (≥ 2)
        let g = |a: f64| leaf_of(&SINUS_RHYTHM, &sinus(100.0, a, 5.0, 6.0, 3.0));
        assert_eq!(g(50.0), "grade-3");
        assert_eq!(g(100.0 / 1.99), "grade-2");
        // with PASP given (RAP estimated) the PASP decides, not the TR velocity
        let i = sinus(100.0, 80.0, 5.0, 6.0, 3.0)
            .with("pasp_mmhg", 34.0)
            .with("lars_percent", 30.0);
        assert_eq!(leaf_of(&SINUS_RHYTHM, &i), "normal-lap");
        // Table 6 age cut-offs replace Figure 3's: septal e' 6.5 is reduced at 20-39 (< 7)
        let i = Inputs::new()
            .with_choice("e_prime_cutoffs", "20-39")
            .with("e_cm_s", 50.0)
            .with("a_cm_s", 80.0)
            .with("e_septal_cm_s", 6.5)
            .with("tr_vmax_m_s", 2.0);
        assert_eq!(leaf_of(&SINUS_RHYTHM, &i), "grade-1");
    }

    #[test]
    fn an_empty_field_that_cannot_change_the_result_is_named_and_resolved() {
        // all three abnormal with E/A 2.2 except E/A empty: grade 2 or 3 → open
        let i = Inputs::new()
            .with("e_cm_s", 100.0)
            .with("e_septal_cm_s", 5.0)
            .with("tr_vmax_m_s", 3.0);
        let out = ok(&SINUS_RHYTHM, &i);
        let last = out.findings.last().unwrap();
        assert_eq!(last.severity, Severity::Indeterminate);
        assert!(
            last.text.en.contains("grade 2") && last.text.en.contains("grade 3"),
            "{}",
            last.text.en
        );
        // e' and E/e' normal, TR empty: normal diastolic function, or (TR raised, LARS 30) normal
        // LAP: two different ends, so the result is open
        let i = Inputs::new()
            .with("e_cm_s", 80.0)
            .with("a_cm_s", 60.0)
            .with("e_septal_cm_s", 10.0)
            .with("lars_percent", 30.0);
        let out = ok(&SINUS_RHYTHM, &i);
        let last = out.findings.last().unwrap();
        assert_eq!(last.severity, Severity::Indeterminate);
        assert!(
            last.text.en.contains("normal diastolic function")
                && last.text.en.contains("normal LAP (the figure"),
            "{}",
            last.text.en
        );
        // the measured value it looked at is shown, the empty one as "?"
        assert!(last.criterion.contains("TR = ?"), "{}", last.criterion);
    }

    #[test]
    fn atrial_fibrillation_follows_figure_8() {
        let af = |e: f64, es: f64, tr: f64, dt: f64| {
            Inputs::new()
                .with("e_cm_s", e)
                .with("e_septal_cm_s", es)
                .with("tr_vmax_m_s", tr)
                .with("dt_ms", dt)
        };
        // E 100 (≥ 100) only: normal
        assert_eq!(
            leaf_of(&ATRIAL_FIBRILLATION, &af(100.0, 12.0, 2.0, 200.0)),
            "normal"
        );
        // E 100, E/e' 100/9 = 11.1 > 11, TR 2.9 > 2.8: three → elevated
        assert_eq!(
            leaf_of(&ATRIAL_FIBRILLATION, &af(100.0, 9.0, 2.9, 200.0)),
            "elevated"
        );
        // two (E, DT 160 ≤ 160): second step
        let two = af(100.0, 12.0, 2.0, 160.0);
        assert_eq!(leaf_of(&ATRIAL_FIBRILLATION, &two), "open");
        let full = |lars: f64, sd: f64, bmi: f64| {
            two.clone()
                .with("lars_percent", lars)
                .with("pv_sd_ratio", sd)
                .with("bmi_kg_m2", bmi)
        };
        assert_eq!(leaf_of(&ATRIAL_FIBRILLATION, &full(25.0, 1.2, 25.0)), "normal");
        assert_eq!(
            leaf_of(&ATRIAL_FIBRILLATION, &full(15.0, 1.2, 25.0)),
            "indeterminate"
        );
        assert_eq!(leaf_of(&ATRIAL_FIBRILLATION, &full(15.0, 0.9, 25.0)), "elevated");
        // boundaries: LARS 18 is not < 18; BMI 30 is not > 30; S/D 1 is not < 1
        assert_eq!(leaf_of(&ATRIAL_FIBRILLATION, &full(18.0, 1.0, 30.0)), "normal");
        // two positive, one empty: elevated whatever the third
        let i = two.clone().with("lars_percent", 15.0).with("bmi_kg_m2", 31.0);
        assert_eq!(leaf_of(&ATRIAL_FIBRILLATION, &i), "elevated");
        // DT 161 is not ≤ 160: one criterion only
        assert_eq!(
            leaf_of(&ATRIAL_FIBRILLATION, &af(100.0, 12.0, 2.0, 161.0)),
            "normal"
        );
    }

    #[test]
    fn mac_follows_figure_4() {
        let m = |e: f64, a: f64| Inputs::new().with("e_cm_s", e).with("a_cm_s", a);
        assert_eq!(leaf_of(&MAC, &m(79.0, 100.0)), "normal");
        // 0.8 and 1.8 belong to the middle branch: IVRT decides
        assert_eq!(leaf_of(&MAC, &m(80.0, 100.0)), "open");
        assert_eq!(leaf_of(&MAC, &m(80.0, 100.0).with("ivrt_ms", 80.0)), "normal");
        assert_eq!(leaf_of(&MAC, &m(80.0, 100.0).with("ivrt_ms", 79.0)), "elevated");
        assert_eq!(leaf_of(&MAC, &m(180.0, 100.0).with("ivrt_ms", 90.0)), "normal");
        assert_eq!(leaf_of(&MAC, &m(181.0, 100.0)), "elevated");
    }

    #[test]
    fn pulmonary_hypertension_follows_figure_6() {
        let p = |e: f64, a: f64| Inputs::new().with("e_cm_s", e).with("a_cm_s", a);
        // E/A 40/50 = 0.8 with E 40 ≤ 50: normal
        assert_eq!(leaf_of(&PULMONARY_HYPERTENSION, &p(40.0, 50.0)), "normal");
        // E/A 0.8 with E 60 > 50: LARS
        assert_eq!(
            leaf_of(&PULMONARY_HYPERTENSION, &p(60.0, 75.0).with("lars_percent", 18.0)),
            "elevated"
        );
        assert_eq!(
            leaf_of(&PULMONARY_HYPERTENSION, &p(60.0, 75.0).with("lars_percent", 19.0)),
            "normal"
        );
        // LARS not available: lateral E/e' 60/8 = 7.5 < 8 normal; 60/4 = 15 > 13 elevated; 60/5 = 12 indeterminate
        let lat = |el: f64| leaf_of(&PULMONARY_HYPERTENSION, &p(60.0, 50.0).with("e_lateral_cm_s", el));
        assert_eq!(lat(8.0), "normal");
        assert_eq!(lat(4.0), "elevated");
        assert_eq!(lat(5.0), "indeterminate");
        assert_eq!(lat(7.5), "indeterminate", "8.0 exactly is not < 8");
        assert_eq!(leaf_of(&PULMONARY_HYPERTENSION, &p(60.0, 50.0)), "open");
        // E/A ≥ 2
        let hi = p(100.0, 50.0);
        assert_eq!(
            leaf_of(
                &PULMONARY_HYPERTENSION,
                &hi.clone().with_choice("e_prime_reduced", "yes")
            ),
            "elevated"
        );
        assert_eq!(
            leaf_of(&PULMONARY_HYPERTENSION, &hi.with_choice("e_prime_reduced", "no")),
            "outside"
        );
    }

    #[test]
    fn heart_transplant_follows_figure_5() {
        let h = |e: f64, es: f64, el: f64| {
            Inputs::new()
                .with("e_cm_s", e)
                .with("e_septal_cm_s", es)
                .with("e_lateral_cm_s", el)
        };
        // average e' 10: E/e' 6.9 < 7 normal; 7.0 middle; 14.1 elevated
        assert_eq!(leaf_of(&HEART_TRANSPLANT, &h(69.0, 10.0, 10.0)), "normal");
        assert_eq!(leaf_of(&HEART_TRANSPLANT, &h(141.0, 10.0, 10.0)), "elevated");
        // E/e' 7 (70/10) is in the 7-14 band: E/SRIVR 70/0.35 = 200 ≤ 200 normal; 70/0.34 > 200 elevated
        let mid = h(70.0, 10.0, 10.0);
        assert_eq!(
            leaf_of(&HEART_TRANSPLANT, &mid.clone().with("sr_ivr_per_s", 0.35)),
            "normal"
        );
        assert_eq!(
            leaf_of(&HEART_TRANSPLANT, &mid.clone().with("sr_ivr_per_s", 0.34)),
            "elevated"
        );
        // no SRIVR: TR 2.8 normal, 2.81 elevated, no TR: the figure's indeterminate
        assert_eq!(
            leaf_of(&HEART_TRANSPLANT, &mid.clone().with("tr_vmax_m_s", 2.8)),
            "normal"
        );
        assert_eq!(
            leaf_of(&HEART_TRANSPLANT, &mid.clone().with("tr_vmax_m_s", 2.81)),
            "elevated"
        );
        assert_eq!(leaf_of(&HEART_TRANSPLANT, &mid), "indeterminate");
        // the average needs both sites
        assert_eq!(
            leaf_of(&HEART_TRANSPLANT, &Inputs::new().with("e_cm_s", 70.0)),
            "open"
        );
    }

    #[test]
    fn exercise_follows_section_13() {
        let x = |ee: f64, tr: f64| {
            Inputs::new()
                .with("e_e_average_peak", ee)
                .with("tr_vmax_peak_m_s", tr)
        };
        assert_eq!(leaf_of(&EXERCISE, &x(14.0, 3.2)), "positive");
        assert_eq!(leaf_of(&EXERCISE, &x(13.9, 3.2)), "not-supported");
        assert_eq!(leaf_of(&EXERCISE, &x(14.0, 3.19)), "likely");
        assert_eq!(leaf_of(&EXERCISE, &x(9.9, 2.79)), "normal");
        assert_eq!(leaf_of(&EXERCISE, &x(10.0, 2.79)), "indeterminate");
        // septal ratio: its own cut-off of 15
        let s = |ee: f64| {
            Inputs::new()
                .with("e_e_septal_peak", ee)
                .with("tr_vmax_peak_m_s", 3.3)
        };
        assert_eq!(leaf_of(&EXERCISE, &s(15.0)), "positive");
        assert_eq!(leaf_of(&EXERCISE, &s(14.9)), "not-supported");
    }

    #[test]
    fn indicator_lists_of_table_7() {
        // mitral stenosis: IVRT 60 is not < 60; A 1.5 is not > 1.5; IVRT/TE-e' 4.2 is not < 4.2
        let i = Inputs::new()
            .with("ivrt_ms", 60.0)
            .with("a_m_s", 1.5)
            .with("te_e_ms", 60.0 / 4.2);
        assert_eq!(leaf_of(&MITRAL_STENOSIS, &i), "none");
        assert_eq!(leaf_of(&MITRAL_STENOSIS, &i.clone().with("ivrt_ms", 59.0)), "met");
        assert_eq!(
            leaf_of(&MITRAL_STENOSIS, &Inputs::new().with("a_m_s", 1.2)),
            "open"
        );
        assert!(MITRAL_STENOSIS.evaluate(&Inputs::new()).is_err());
        // MR: Ar − A 30 meets (≥ 30); E/e' used only with reduced EF
        let mr = Inputs::new()
            .with_choice("ef", "normal")
            .with("ivrt_ms", 70.0)
            .with("te_e_ms", 10.0)
            .with("ar_duration_ms", 160.0)
            .with("a_duration_ms", 131.0);
        assert_eq!(leaf_of(&MITRAL_REGURGITATION, &mr), "none");
        assert_eq!(
            leaf_of(&MITRAL_REGURGITATION, &mr.clone().with("a_duration_ms", 130.0)),
            "met"
        );
        let reduced = mr
            .with_choice("ef", "reduced")
            .with("e_cm_s", 150.0)
            .with("e_septal_cm_s", 10.0)
            .with("e_lateral_cm_s", 10.0);
        assert_eq!(leaf_of(&MITRAL_REGURGITATION, &reduced), "met");
        // HCM: LAVi 34 is not > 34
        let h = Inputs::new()
            .with("tr_vmax_m_s", 2.8)
            .with("lavi_ml_m2", 34.0)
            .with("e_cm_s", 70.0)
            .with("e_septal_cm_s", 5.0)
            .with("e_lateral_cm_s", 5.0)
            .with("ar_duration_ms", 120.0)
            .with("a_duration_ms", 120.0);
        assert_eq!(leaf_of(&HCM, &h), "none");
        assert_eq!(leaf_of(&HCM, &h.with("lavi_ml_m2", 35.0)), "met");
        // restrictive: E/A 2.5 is not > 2.5, DT 140 not < 140, IVRT 50 not < 50, E/e' 14 not > 14
        let r = Inputs::new()
            .with("e_cm_s", 70.0)
            .with("a_cm_s", 28.0)
            .with("e_septal_cm_s", 5.0)
            .with("e_lateral_cm_s", 5.0)
            .with("dt_ms", 140.0)
            .with("ivrt_ms", 50.0);
        assert_eq!(leaf_of(&RESTRICTIVE, &r), "none");
        assert_eq!(leaf_of(&RESTRICTIVE, &r.with("dt_ms", 139.0)), "met");
        // tachycardia: IVRT 70 meets (≤ 70)
        assert_eq!(
            leaf_of(&SINUS_TACHYCARDIA, &Inputs::new().with("ivrt_ms", 70.0)),
            "met"
        );
        // LVAD: LAP from RAP and septum; RAP 10 is not > 10
        let l = Inputs::new()
            .with("e_cm_s", 80.0)
            .with("a_cm_s", 40.0)
            .with("rap_mmhg", 10.0)
            .with("pasp_mmhg", 40.0)
            .with("lavi_ml_m2", 33.0)
            .with("e_septal_cm_s", 8.0)
            .with("e_lateral_cm_s", 8.0)
            .with_choice("septum", "right");
        let out = ok(&LVAD, &l);
        assert_eq!(out.value("lap_estimate_mmhg"), Some(15.0));
        assert_eq!(leaf_of(&LVAD, &l), "none");
    }
}
