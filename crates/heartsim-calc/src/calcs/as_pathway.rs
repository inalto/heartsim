//! Aortic stenosis: the integrated, stepwise approach of the 2017 EACVI/ASE focused update
//! (Baumgartner et al., Figure 8): high or low gradient, the high-flow check, valve area, flow
//! status, ejection fraction, low-dose dobutamine and the calcium score (Table 5).
//!
//! The figure is followed where it and the text differ (AVA ≤ 1.0 cm² and SVi ≤ 35 mL/m² in the
//! figure, AVA < 1.0 and SVi < 35 in the text); the differences are listed in
//! `docs/research/echo-calculators.md`.

use super::{cite, n, opt, opt_choice, outcome, q, v, when};
use crate::model::ExampleValue::{Choice as C, Number as N};
use crate::model::{
    Calculator, Category, Choice, Cite, InputError, Inputs, Outcome, Review, Severity, Text, Unit,
};
use crate::refs::AS_2017;
use crate::tree::{self, Cmp, Fact, Leaf, LeafKind, Node, Tree, Walk, answers, test};
use crate::tx;

const fn src(locator: &'static str) -> Cite {
    cite(&AS_2017, locator)
}

const fn node(id: &'static str, question: Text, locator: &'static str, uses: &'static [usize]) -> Node {
    Node {
        id,
        question,
        cite: src(locator),
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
        cite: src(locator),
    }
}

const GRADIENT: usize = 0;
const HIGH_FLOW: usize = 1;
const REVERSIBLE: usize = 2;
const AVA: usize = 3;
const CHECKED: usize = 4;
const LOW_FLOW: usize = 5;
const LOW_EF: usize = 6;
const RESERVE: usize = 7;
const DSE: usize = 8;
const CALCIUM: usize = 9;

const F8: &str = "Figure 8, p. 388";

static STEP1: Node = node(
    "step-1",
    tx!(
        "Step 1, velocity and mean gradient",
        "Passo 1, velocità e gradiente medio"
    ),
    "Figure 8, p. 388; text, p. 387",
    &[GRADIENT],
);
static HIGH_FLOW_Q: Node = node(
    "high-flow",
    tx!("High flow (SVi > 58 mL/m²)", "Alto flusso (SVi > 58 mL/m²)"),
    "Figure 8, p. 388; high gradient track, p. 387",
    &[HIGH_FLOW],
);
static REVERSIBLE_Q: Node = node(
    "reversible",
    tx!(
        "The high-flow state is reversible",
        "Lo stato di alto flusso è reversibile"
    ),
    "Figure 8, p. 388; high gradient track, p. 387",
    &[REVERSIBLE],
);
static STEP2: Node = node(
    "step-2",
    tx!("Step 2, aortic valve area", "Passo 2, area valvolare aortica"),
    F8,
    &[AVA],
);
static STEP3: Node = node(
    "step-3",
    tx!(
        "Step 3, measurement errors that underestimate gradient, flow or area excluded",
        "Passo 3, esclusi gli errori di misura che sottostimano gradiente, flusso o area"
    ),
    "Figure 8, p. 388; step 3, p. 387",
    &[CHECKED],
);
static STEP4: Node = node(
    "step-4",
    tx!("Step 4, flow status (SVi)", "Passo 4, stato del flusso (SVi)"),
    F8,
    &[LOW_FLOW],
);
static STEP5: Node = node(
    "step-5",
    tx!(
        "Step 5, LV ejection fraction",
        "Passo 5, frazione di eiezione del VS"
    ),
    F8,
    &[LOW_EF],
);
static STEP6: Node = node(
    "step-6",
    tx!(
        "Step 6, low-dose dobutamine: flow reserve (stroke volume rise of 20% or more)",
        "Passo 6, dobutamina a basse dosi: riserva di flusso (aumento della gittata del 20% o più)"
    ),
    "Figure 8, p. 388; flow reserve, pp. 384 and 387-388",
    &[RESERVE],
);
static STEP6B: Node = node(
    "step-6-result",
    tx!("Dobutamine result", "Risultato con dobutamina"),
    "Table 4, p. 384; findings recommended as reliable, p. 385",
    &[DSE],
);
static STEP7: Node = node(
    "step-7",
    tx!(
        "Step 7, calcium score by CT",
        "Passo 7, punteggio di calcio alla TC"
    ),
    "Figure 8, p. 388; Table 5, p. 385",
    &[CALCIUM],
);

static SEVERE_HIGH: Leaf = leaf(
    "severe-high-gradient",
    LeafKind::Result,
    Severity::Severe,
    tx!(
        "meets the figure's criteria for severe high-gradient aortic stenosis (flow and EF then define its subgroup)",
        "soddisfa i criteri della figura per stenosi aortica grave ad alto gradiente (flusso e FE ne definiscono poi il sottogruppo)"
    ),
    "Vmax ≥ 4 m/s or mean ≥ 40 mmHg, no reversible high flow",
    F8,
);
static REASSESS: Leaf = leaf(
    "reassess",
    LeafKind::Indeterminate,
    Severity::Indeterminate,
    tx!(
        "high gradient with a reversible high-flow state: the figure asks to re-assess once flow is normal",
        "alto gradiente con alto flusso reversibile: la figura chiede di rivalutare a flusso normalizzato"
    ),
    "high gradient, SVi > 58 mL/m², reversible",
    F8,
);
static MODERATE: Leaf = leaf(
    "moderate",
    LeafKind::Result,
    Severity::Moderate,
    tx!(
        "meets the figure's criteria for moderate aortic stenosis",
        "soddisfa i criteri della figura per stenosi aortica moderata"
    ),
    "low gradient, AVA > 1.0 cm²",
    F8,
);
static CHECK_FIRST: Leaf = leaf(
    "check-measurements",
    LeafKind::Indeterminate,
    Severity::Indeterminate,
    tx!(
        "the figure requires excluding measurement errors before going on",
        "la figura richiede di escludere gli errori di misura prima di procedere"
    ),
    "step 3 not done",
    "Figure 8, p. 388; step 3, p. 387",
);
static UNLIKELY: Leaf = leaf(
    "severe-unlikely",
    LeafKind::Result,
    Severity::Moderate,
    tx!(
        "normal flow with a low gradient: the figure calls severe aortic stenosis unlikely",
        "flusso normale con basso gradiente: la figura considera improbabile una stenosi aortica grave"
    ),
    "low gradient, AVA ≤ 1.0 cm², SVi > 35 mL/m²",
    "Figure 8, p. 388; step 4, p. 387",
);
static PSEUDO: Leaf = leaf(
    "pseudosevere",
    LeafKind::Result,
    Severity::Moderate,
    tx!(
        "meets the source's finding for pseudosevere stenosis (valve area above 1.0 cm² with dobutamine)",
        "soddisfa il reperto della fonte per stenosi pseudo-grave (area valvolare sopra 1,0 cm² con dobutamina)"
    ),
    "flow reserve, stress AVA > 1.0 cm²",
    "Figure 8, p. 388; p. 385",
);
static TRUE_SEVERE: Leaf = leaf(
    "true-severe",
    LeafKind::Result,
    Severity::Severe,
    tx!(
        "meets the source's finding for true severe stenosis (Vmax ≥ 4 m/s or mean gradient ≥ 40 mmHg with the area never above 1.0 cm²)",
        "soddisfa il reperto della fonte per stenosi realmente grave (Vmax ≥ 4 m/s o gradiente medio ≥ 40 mmHg con area mai sopra 1,0 cm²)"
    ),
    "flow reserve, stress Vmax ≥ 4 or mean ≥ 40, AVA ≤ 1.0 cm²",
    "Figure 8, p. 388; p. 385",
);
static CA_UNLIKELY: Leaf = leaf(
    "calcium-unlikely",
    LeafKind::Result,
    Severity::Moderate,
    tx!(
        "calcium score in the range Table 5 calls severe stenosis unlikely",
        "punteggio di calcio nell'intervallo in cui la Tabella 5 considera improbabile una stenosi grave"
    ),
    "men < 1600, women < 800 AU",
    "Table 5, p. 385",
);
static CA_GREY: Leaf = leaf(
    "calcium-grey-zone",
    LeafKind::Indeterminate,
    Severity::Indeterminate,
    tx!(
        "calcium score in the grey zone between Table 5's limits: it cannot decide by itself; the integrated approach of Table 5 applies",
        "punteggio di calcio nella zona grigia fra i limiti della Tabella 5: da solo non decide; si applica l'approccio integrato della Tabella 5"
    ),
    "men 1600-1999, women 800-1199 AU",
    "Table 5, p. 385; text, p. 385",
);
static CA_LIKELY: Leaf = leaf(
    "calcium-likely",
    LeafKind::Result,
    Severity::Severe,
    tx!(
        "calcium score in the range Table 5 calls severe stenosis likely",
        "punteggio di calcio nell'intervallo in cui la Tabella 5 considera probabile una stenosi grave"
    ),
    "men ≥ 2000, women ≥ 1200 AU",
    "Table 5, p. 385",
);
static CA_VERY: Leaf = leaf(
    "calcium-very-likely",
    LeafKind::Result,
    Severity::Severe,
    tx!(
        "calcium score in the range Table 5 calls severe stenosis very likely",
        "punteggio di calcio nell'intervallo in cui la Tabella 5 considera molto probabile una stenosi grave"
    ),
    "men ≥ 3000, women ≥ 1600 AU",
    "Table 5, p. 385",
);
static OPEN: Leaf = leaf(
    "open",
    LeafKind::Indeterminate,
    Severity::Indeterminate,
    tx!(
        "indeterminate with these measurements",
        "indeterminato con queste misure"
    ),
    "measurements left empty change the result",
    F8,
);

fn calcium(mut w: Walk, f: &[u8]) -> Walk {
    let (answer, end) = match f[CALCIUM] {
        0 => (tx!("severe unlikely", "grave improbabile"), &CA_UNLIKELY),
        1 => (tx!("grey zone", "zona grigia"), &CA_GREY),
        2 => (tx!("severe likely", "grave probabile"), &CA_LIKELY),
        _ => (tx!("severe very likely", "grave molto probabile"), &CA_VERY),
    };
    w.ask(&STEP7, answer);
    w.end(end)
}

fn walk(f: &[u8]) -> Walk {
    let mut w = Walk::new(&OPEN);
    if f[GRADIENT] == 1 {
        w.ask(&STEP1, tx!("high gradient", "alto gradiente"));
        if f[HIGH_FLOW] == 0 {
            w.ask(&HIGH_FLOW_Q, answers::NO);
            return w.end(&SEVERE_HIGH);
        }
        w.ask(&HIGH_FLOW_Q, answers::YES);
        return if f[REVERSIBLE] == 1 {
            w.ask(&REVERSIBLE_Q, answers::YES);
            w.end(&REASSESS)
        } else {
            w.ask(&REVERSIBLE_Q, answers::NO);
            w.end(&SEVERE_HIGH)
        };
    }
    w.ask(&STEP1, tx!("low gradient", "basso gradiente"));
    if f[AVA] == 0 {
        w.ask(&STEP2, tx!("> 1.0 cm²", "> 1,0 cm²"));
        return w.end(&MODERATE);
    }
    w.ask(&STEP2, tx!("≤ 1.0 cm²", "≤ 1,0 cm²"));
    if f[CHECKED] == 0 {
        w.ask(&STEP3, answers::NO);
        return w.end(&CHECK_FIRST);
    }
    w.ask(&STEP3, answers::YES);
    if f[LOW_FLOW] == 0 {
        w.ask(
            &STEP4,
            tx!("normal flow (SVi > 35 mL/m²)", "flusso normale (SVi > 35 mL/m²)"),
        );
        return w.end(&UNLIKELY);
    }
    w.ask(
        &STEP4,
        tx!("low flow (SVi ≤ 35 mL/m²)", "basso flusso (SVi ≤ 35 mL/m²)"),
    );
    if f[LOW_EF] == 0 {
        w.ask(
            &STEP5,
            tx!(
                "≥ 50%: integrated approach (Table 5)",
                "≥ 50%: approccio integrato (Tabella 5)"
            ),
        );
        return calcium(w, f);
    }
    w.ask(&STEP5, tx!("< 50%: dobutamine", "< 50%: dobutamina"));
    if f[RESERVE] == 0 {
        w.ask(&STEP6, answers::NO);
        return calcium(w, f);
    }
    w.ask(&STEP6, answers::YES);
    match f[DSE] {
        0 => {
            w.ask(&STEP6B, tx!("area above 1.0 cm²", "area sopra 1,0 cm²"));
            w.end(&PSEUDO)
        }
        1 => {
            w.ask(
                &STEP6B,
                tx!(
                    "Vmax ≥ 4 m/s or mean ≥ 40 mmHg, area ≤ 1.0 cm²",
                    "Vmax ≥ 4 m/s o media ≥ 40 mmHg, area ≤ 1,0 cm²"
                ),
            );
            w.end(&TRUE_SEVERE)
        }
        _ => {
            w.ask(
                &STEP6B,
                tx!(
                    "neither (mean gradient below 40 mmHg, or in the 30-40 mmHg range the text leaves open)",
                    "nessuno dei due (gradiente medio sotto 40 mmHg, o nell'intervallo 30-40 mmHg che il testo lascia aperto)"
                ),
            );
            calcium(w, f)
        }
    }
}

/// Figure 8.
pub static TREE: Tree = Tree {
    calculator: "aortic-stenosis-pathway",
    nodes: &[
        &STEP1,
        &HIGH_FLOW_Q,
        &REVERSIBLE_Q,
        &STEP2,
        &STEP3,
        &STEP4,
        &STEP5,
        &STEP6,
        &STEP6B,
        &STEP7,
    ],
    leaves: &[
        &SEVERE_HIGH,
        &REASSESS,
        &MODERATE,
        &CHECK_FIRST,
        &UNLIKELY,
        &PSEUDO,
        &TRUE_SEVERE,
        &CA_UNLIKELY,
        &CA_GREY,
        &CA_LIKELY,
        &CA_VERY,
    ],
    open: &OPEN,
    arities: &[2, 2, 2, 2, 2, 2, 2, 2, 3, 4],
    walk,
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
const SEX: &[Choice] = &[
    Choice {
        id: "male",
        label: tx!("male", "maschio"),
    },
    Choice {
        id: "female",
        label: tx!("female", "femmina"),
    },
];

fn yes_no(i: &Inputs, id: &str, label: &str) -> Fact {
    let c = i.choice(id);
    Fact::flag(c.map(|c| c == "yes"), format!("{label} = {}", c.unwrap_or("?")))
}

/// Table 5 calcium classes: 0 unlikely, 1 grey zone, 2 likely, 3 very likely.
fn calcium_class(score: f64, male: bool) -> u8 {
    let (unlikely, likely, very) = if male {
        (1600.0, 2000.0, 3000.0)
    } else {
        (800.0, 1200.0, 1600.0)
    };
    // the limits rise, so each one passed adds a class
    u8::from(Cmp::Ge.holds(score, unlikely))
        + u8::from(Cmp::Ge.holds(score, likely))
        + u8::from(Cmp::Ge.holds(score, very))
}

fn pathway(i: &Inputs) -> Result<Outcome, Vec<InputError>> {
    let (vmax, mean) = (i.n("vmax_m_s"), i.n("mean_gradient_mmhg"));
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
    let dse = i.choice("dse") == Some("yes");
    let sv = if dse {
        i.num("sv_rest_ml").zip(i.num("sv_stress_ml"))
    } else {
        None
    };
    let rise = sv.map(|(r, s)| (s - r) / r * 100.0);
    if let (Some((r, s)), Some(rise)) = (sv, rise) {
        out.push(
            q(
                "sv_rise_percent",
                tx!(
                    "Stroke volume rise with dobutamine",
                    "Aumento della gittata con dobutamina"
                ),
                rise,
                Unit::Percent,
                0,
            ),
            "(SV(stress) − SV(rest)) ÷ SV(rest) × 100",
            format!(
                "({} − {}) ÷ {} × 100",
                v(s, 0, Unit::Ml),
                v(r, 0, Unit::Ml),
                n(r, 0)
            ),
        );
    }
    let high = Cmp::Ge.holds(vmax, 4.0) || Cmp::Ge.holds(mean, 40.0);
    let svi = i.num("svi_ml_m2");
    let ava = i.num("ava_cm2");
    let dse_class = if dse {
        let (a, vs, ms) = (
            i.num("ava_stress_cm2"),
            i.num("vmax_stress_m_s"),
            i.num("mean_stress_mmhg"),
        );
        match a {
            Some(a) if Cmp::Gt.holds(a, 1.0) => Some(0),
            Some(_) => match (vs, ms) {
                (Some(x), _) if Cmp::Ge.holds(x, 4.0) => Some(1),
                (_, Some(m)) if Cmp::Ge.holds(m, 40.0) => Some(1),
                (Some(_), Some(_)) => Some(2),
                // one of the two missing and the other below its limit: it could still meet it
                _ => None,
            },
            None => None,
        }
    } else {
        None
    };
    let dse_shown = format!(
        "{}; {}; {}",
        test("AVA(stress)", Cmp::Gt, 1.0, Unit::Cm2, 2).show(if dse {
            i.num("ava_stress_cm2")
        } else {
            None
        }),
        test("Vmax(stress)", Cmp::Ge, 4.0, Unit::MS, 1).show(if dse {
            i.num("vmax_stress_m_s")
        } else {
            None
        }),
        test("mean(stress)", Cmp::Ge, 40.0, Unit::MmHg, 0).show(if dse {
            i.num("mean_stress_mmhg")
        } else {
            None
        }),
    );
    let calcium = i.num("calcium_score_au").zip(i.choice("sex"));
    let facts = [
        Fact::flag(
            Some(high),
            tree::show_all(&[
                (test("Vmax", Cmp::Ge, 4.0, Unit::MS, 1), Some(vmax)),
                (test("mean", Cmp::Ge, 40.0, Unit::MmHg, 0), Some(mean)),
            ]),
        ),
        Fact::of(&test("SVi", Cmp::Gt, 58.0, Unit::MlM2, 0), svi),
        yes_no(i, "high_flow_reversible", "reversible"),
        Fact::of(&test("AVA", Cmp::Le, 1.0, Unit::Cm2, 2), ava),
        yes_no(i, "errors_excluded", "errors excluded"),
        Fact::of(&test("SVi", Cmp::Le, 35.0, Unit::MlM2, 0), svi),
        Fact::of(
            &test("LVEF", Cmp::Lt, 50.0, Unit::Percent, 0),
            i.num("lvef_percent"),
        ),
        Fact::of(&test("SV rise", Cmp::Ge, 20.0, Unit::Percent, 0), rise),
        Fact::class(dse_class, 3, dse_shown),
        Fact::class(
            calcium.map(|(s, sex)| calcium_class(s, sex == "male")),
            4,
            match calcium {
                Some((s, sex)) => format!("calcium = {} AU ({sex})", n(s, 0)),
                None => "calcium = ?".to_owned(),
            },
        ),
    ];
    out.findings = tree::decide(&TREE, &facts).findings;
    Ok(out)
}

/// The integrated, stepwise grading of aortic stenosis (Figure 8).
pub static PATHWAY: Calculator = Calculator {
    id: "aortic-stenosis-pathway",
    category: Category::Stenosis,
    name: tx!(
        "Aortic stenosis: stepwise grading and low-flow pathway",
        "Stenosi aortica: graduazione per passi e percorso a basso flusso"
    ),
    synonyms: tx!(
        "AS, low flow low gradient, paradoxical, discordant grading, dobutamine stress, flow reserve, contractile reserve, pseudosevere, calcium score, SVi",
        "SA, basso flusso basso gradiente, paradossa, graduazione discordante, eco stress con dobutamina, riserva di flusso, riserva contrattile, pseudo-grave, punteggio di calcio, SVi"
    ),
    summary: tx!(
        "Follows the 2017 EACVI/ASE integrated approach: a high gradient (Vmax ≥ 4 m/s or mean ≥ 40 mmHg) unless a reversible high-flow state explains it; with a low gradient, the valve area, the exclusion of measurement errors, the flow (SVi), the ejection fraction, low-dose dobutamine and the CT calcium score (Table 5). The source stresses that the final judgement integrates clinical data.",
        "Segue l'approccio integrato EACVI/ASE 2017: alto gradiente (Vmax ≥ 4 m/s o media ≥ 40 mmHg) salvo uno stato di alto flusso reversibile; con basso gradiente, l'area valvolare, l'esclusione degli errori di misura, il flusso (SVi), la frazione di eiezione, la dobutamina a basse dosi e il punteggio di calcio alla TC (Tabella 5). La fonte sottolinea che il giudizio finale integra i dati clinici."
    ),
    inputs: &[
        super::num(
            "vmax_m_s",
            tx!("Peak aortic jet velocity", "Velocità massima del getto aortico"),
            Unit::MS,
            1.0,
            7.0,
            1,
        ),
        super::num(
            "mean_gradient_mmhg",
            tx!("Mean gradient", "Gradiente medio"),
            Unit::MmHg,
            2.0,
            150.0,
            0,
        ),
        opt(
            "ava_cm2",
            tx!(
                "Aortic valve area (continuity)",
                "Area valvolare aortica (continuità)"
            ),
            Unit::Cm2,
            0.2,
            4.0,
            2,
        ),
        opt(
            "svi_ml_m2",
            tx!("Stroke volume index", "Volume sistolico indicizzato"),
            Unit::MlM2,
            10.0,
            120.0,
            0,
        ),
        opt_choice(
            "high_flow_reversible",
            tx!(
                "High flow reversible (anaemia, fever, hyperthyroidism, AV shunt)",
                "Alto flusso reversibile (anemia, febbre, ipertiroidismo, shunt AV)"
            ),
            YES_NO,
        ),
        opt_choice(
            "errors_excluded",
            tx!(
                "Measurement errors excluded (LVOT, velocities)",
                "Errori di misura esclusi (LVOT, velocità)"
            ),
            YES_NO,
        ),
        opt(
            "lvef_percent",
            tx!("LV ejection fraction", "Frazione di eiezione del VS"),
            Unit::Percent,
            5.0,
            90.0,
            0,
        ),
        opt_choice(
            "dse",
            tx!("Low-dose dobutamine study", "Studio con dobutamina a basse dosi"),
            YES_NO,
        ),
        when(
            opt(
                "sv_rest_ml",
                tx!("Stroke volume at rest", "Gittata a riposo"),
                Unit::Ml,
                10.0,
                200.0,
                0,
            ),
            "dse",
            &["yes"],
        ),
        when(
            opt(
                "sv_stress_ml",
                tx!(
                    "Stroke volume at peak dobutamine",
                    "Gittata al picco di dobutamina"
                ),
                Unit::Ml,
                10.0,
                250.0,
                0,
            ),
            "dse",
            &["yes"],
        ),
        when(
            opt(
                "ava_stress_cm2",
                tx!(
                    "Largest valve area during dobutamine",
                    "Area valvolare massima durante dobutamina"
                ),
                Unit::Cm2,
                0.2,
                4.0,
                2,
            ),
            "dse",
            &["yes"],
        ),
        when(
            opt(
                "vmax_stress_m_s",
                tx!(
                    "Peak velocity during dobutamine",
                    "Velocità massima durante dobutamina"
                ),
                Unit::MS,
                1.0,
                7.0,
                1,
            ),
            "dse",
            &["yes"],
        ),
        when(
            opt(
                "mean_stress_mmhg",
                tx!(
                    "Mean gradient during dobutamine",
                    "Gradiente medio durante dobutamina"
                ),
                Unit::MmHg,
                2.0,
                150.0,
                0,
            ),
            "dse",
            &["yes"],
        ),
        opt_choice("sex", tx!("Sex", "Sesso"), SEX),
        opt(
            "calcium_score_au",
            tx!(
                "Aortic valve calcium score (Agatston units)",
                "Punteggio di calcio valvolare (unità Agatston)"
            ),
            Unit::None,
            0.0,
            15000.0,
            0,
        ),
    ],
    compute: pathway,
    references: &[
        cite(&AS_2017, "Figure 8, p. 388; integrated approach, pp. 386-388"),
        cite(&AS_2017, "Table 4 and dobutamine findings, pp. 384-385"),
        cite(&AS_2017, "Table 5 (calcium score), p. 385"),
    ],
    review: Review::Verified,
    example: &[
        ("vmax_m_s", N(3.3)),
        ("mean_gradient_mmhg", N(28.0)),
        ("ava_cm2", N(0.8)),
        ("svi_ml_m2", N(29.0)),
        ("errors_excluded", C("yes")),
        ("lvef_percent", N(35.0)),
        ("dse", C("yes")),
        ("sv_rest_ml", N(50.0)),
        ("sv_stress_ml", N(64.0)),
        ("ava_stress_cm2", N(0.9)),
        ("vmax_stress_m_s", N(4.2)),
        ("mean_stress_mmhg", N(44.0)),
    ],
    sim_link: None,
};

#[cfg(test)]
mod tests {
    use super::super::testing::ok;
    use super::*;

    fn leaf_id(i: &Inputs) -> &'static str {
        let out = ok(&PATHWAY, i);
        tree::reached(&TREE, out.findings.last().unwrap())
            .unwrap_or_else(|| panic!("{:?}", out.findings.last()))
            .id
    }

    fn low() -> Inputs {
        Inputs::new()
            .with("vmax_m_s", 3.5)
            .with("mean_gradient_mmhg", 30.0)
            .with("ava_cm2", 0.8)
            .with_choice("errors_excluded", "yes")
    }

    #[test]
    fn high_gradient_track() {
        let h = |vmax: f64, mean: f64| {
            Inputs::new()
                .with("vmax_m_s", vmax)
                .with("mean_gradient_mmhg", mean)
                .with("svi_ml_m2", 40.0)
        };
        // 4.0 m/s or 40 mmHg is high (≥), 3.9 and 39 low
        assert_eq!(leaf_id(&h(4.0, 30.0)), "severe-high-gradient");
        assert_eq!(leaf_id(&h(3.9, 40.0)), "severe-high-gradient");
        assert_ne!(leaf_id(&h(3.9, 39.0)), "severe-high-gradient");
        // SVi 58 is not high flow; 59 is, then reversibility decides
        let hf = h(4.5, 50.0).with("svi_ml_m2", 59.0);
        assert_eq!(leaf_id(&hf), "open");
        assert_eq!(
            leaf_id(&hf.clone().with_choice("high_flow_reversible", "yes")),
            "reassess"
        );
        assert_eq!(
            leaf_id(&hf.with_choice("high_flow_reversible", "no")),
            "severe-high-gradient"
        );
        assert_eq!(
            leaf_id(&h(4.5, 50.0).with("svi_ml_m2", 58.0)),
            "severe-high-gradient"
        );
    }

    #[test]
    fn low_gradient_track() {
        // AVA 1.0 goes on (≤ 1.0), 1.01 is moderate
        assert_eq!(leaf_id(&low().with("ava_cm2", 1.01)), "moderate");
        assert_eq!(
            leaf_id(&low().with("ava_cm2", 1.0).with("svi_ml_m2", 36.0)),
            "severe-unlikely"
        );
        // step 3 not done
        assert_eq!(
            leaf_id(&low().with_choice("errors_excluded", "no")),
            "check-measurements"
        );
        // SVi 35 is low flow (figure: ≤ 35)
        let lf = low().with("svi_ml_m2", 35.0);
        // preserved EF → calcium score
        let p = lf.clone().with("lvef_percent", 50.0).with_choice("sex", "female");
        assert_eq!(
            leaf_id(&p.clone().with("calcium_score_au", 799.0)),
            "calcium-unlikely"
        );
        assert_eq!(
            leaf_id(&p.clone().with("calcium_score_au", 800.0)),
            "calcium-grey-zone"
        );
        assert_eq!(
            leaf_id(&p.clone().with("calcium_score_au", 1200.0)),
            "calcium-likely"
        );
        assert_eq!(
            leaf_id(&p.clone().with("calcium_score_au", 1600.0)),
            "calcium-very-likely"
        );
        let m = lf.clone().with("lvef_percent", 60.0).with_choice("sex", "male");
        assert_eq!(
            leaf_id(&m.clone().with("calcium_score_au", 1599.0)),
            "calcium-unlikely"
        );
        assert_eq!(
            leaf_id(&m.clone().with("calcium_score_au", 1999.0)),
            "calcium-grey-zone"
        );
        assert_eq!(
            leaf_id(&m.clone().with("calcium_score_au", 2000.0)),
            "calcium-likely"
        );
        assert_eq!(
            leaf_id(&m.clone().with("calcium_score_au", 3000.0)),
            "calcium-very-likely"
        );
        // no calcium score: open
        assert_eq!(leaf_id(&m), "open");
    }

    #[test]
    fn dobutamine_step() {
        let r = low()
            .with("svi_ml_m2", 28.0)
            .with("lvef_percent", 49.0)
            .with_choice("dse", "yes")
            .with("sv_rest_ml", 50.0);
        // SV +20% is flow reserve (20% or more)
        let s = r.clone().with("sv_stress_ml", 60.0);
        assert_eq!(leaf_id(&s.clone().with("ava_stress_cm2", 1.1)), "pseudosevere");
        assert_eq!(
            leaf_id(
                &s.clone()
                    .with("ava_stress_cm2", 1.0)
                    .with("vmax_stress_m_s", 4.0)
                    .with("mean_stress_mmhg", 35.0)
            ),
            "true-severe"
        );
        // 35 mmHg and 3.8 m/s: in the range the text leaves open → calcium (empty) → open
        let grey = s
            .clone()
            .with("ava_stress_cm2", 0.9)
            .with("vmax_stress_m_s", 3.8)
            .with("mean_stress_mmhg", 35.0);
        assert_eq!(leaf_id(&grey), "open");
        assert_eq!(
            leaf_id(&grey.with_choice("sex", "male").with("calcium_score_au", 3100.0)),
            "calcium-very-likely"
        );
        // +19%: no flow reserve → calcium
        let no = r
            .clone()
            .with("sv_stress_ml", 59.5)
            .with_choice("sex", "male")
            .with("calcium_score_au", 1000.0);
        assert_eq!(leaf_id(&no), "calcium-unlikely");
        // the example: SV 50 → 64 (+28%), stress area 0.9, 4.2 m/s → true severe
        assert_eq!(leaf_id(&PATHWAY.example_inputs()), "true-severe");
    }
}
