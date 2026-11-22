//! Native valve regurgitation: effective orifice and regurgitant volume by PISA, and volume and
//! fraction by the volumetric (two-site stroke volume) method.

use super::words::{MILD, MODERATE, SEVERE};
use super::{check, cite, n, num, o, opt, outcome, q, v};
use crate::blocks::{pisa_flow_ml_s, stroke_volume_ml};
use crate::grade::{Scale, band, criterion_met, lt};
use crate::model::ExampleValue::Number as N;
use crate::model::{
    Calculator, Category, Cite, InputError, InputSpec, Inputs, Outcome, Review, Severity, Text, Unit,
};
use crate::refs::VR_2017;
use crate::tx;

const MR_T8: Cite = cite(&VR_2017, "Table 8, p. 332");
const AR_T11: Cite = cite(&VR_2017, "Table 11, p. 340");
const TR_T14: Cite = cite(&VR_2017, "Table 14, p. 350");
const PR_T16: Cite = cite(&VR_2017, "Table 16, p. 358");
const PISA_EQ: Cite = cite(&VR_2017, "PISA equations, p. 312");
const VOL_EQ: Cite = cite(&VR_2017, "Volumetric method and Figure 4, p. 311");

const MILD_MOD: Text = tx!("moderate (mild–moderate)", "moderata (lieve–moderata)");
const MOD_SEV: Text = tx!("moderate (moderate–severe)", "moderata (moderata–grave)");

/// The regurgitation tables' four grades: < a mild, a to < b mild–moderate, b to < c
/// moderate–severe, ≥ c severe.
macro_rules! four_scale {
    ($name:ident, $what:expr, $sym:expr, $unit:expr, $dec:expr, $a:expr, $b:expr, $c:expr, $src:expr) => {
        static $name: Scale = Scale {
            what: $what,
            symbol: $sym,
            unit: $unit,
            decimals: $dec,
            bands: &[
                band($a, false, Severity::Mild, MILD),
                band($b, false, Severity::Moderate, MILD_MOD),
                band($c, false, Severity::Moderate, MOD_SEV),
            ],
            above: (Severity::Severe, SEVERE),
            source: $src,
        };
    };
}

const EROA_W: Text = tx!(
    "Effective regurgitant orifice area",
    "Area dell'orifizio rigurgitante effettivo"
);
const RVOL_W: Text = tx!("Regurgitant volume", "Volume rigurgitante");
const RF_W: Text = tx!("Regurgitant fraction", "Frazione rigurgitante");

four_scale!(MR_EROA, EROA_W, "EROA", Unit::Cm2, 2, 0.20, 0.30, 0.40, MR_T8);
four_scale!(MR_RVOL, RVOL_W, "RVol", Unit::Ml, 0, 30.0, 45.0, 60.0, MR_T8);
four_scale!(MR_RF, RF_W, "RF", Unit::Percent, 0, 30.0, 40.0, 50.0, MR_T8);
four_scale!(AR_EROA, EROA_W, "EROA", Unit::Cm2, 2, 0.10, 0.20, 0.30, AR_T11);
four_scale!(AR_RVOL, RVOL_W, "RVol", Unit::Ml, 0, 30.0, 45.0, 60.0, AR_T11);
four_scale!(AR_RF, RF_W, "RF", Unit::Percent, 0, 30.0, 40.0, 50.0, AR_T11);

static TR_EROA: Scale = Scale {
    what: EROA_W,
    symbol: "EROA",
    unit: Unit::Cm2,
    decimals: 2,
    bands: &[
        band(0.20, false, Severity::Mild, MILD),
        band(0.40, false, Severity::Moderate, MODERATE),
    ],
    above: (Severity::Severe, SEVERE),
    source: TR_T14,
};
static TR_RVOL: Scale = Scale {
    what: RVOL_W,
    symbol: "RVol",
    unit: Unit::Ml,
    decimals: 0,
    bands: &[
        band(30.0, false, Severity::Mild, MILD),
        band(45.0, false, Severity::Moderate, MODERATE),
    ],
    above: (Severity::Severe, SEVERE),
    source: TR_T14,
};
/// Table 14 gives < 0.3, 0.3–0.69 and ≥ 0.7 cm.
static TR_VC: Scale = Scale {
    what: tx!("Vena contracta width", "Larghezza della vena contracta"),
    symbol: "VCW",
    unit: Unit::Cm,
    decimals: 1,
    bands: &[
        band(0.3, false, Severity::Mild, MILD),
        band(0.7, false, Severity::Moderate, MODERATE),
    ],
    above: (Severity::Severe, SEVERE),
    source: TR_T14,
};
static PR_RF: Scale = Scale {
    what: RF_W,
    symbol: "RF",
    unit: Unit::Percent,
    decimals: 0,
    bands: &[
        band(20.0, false, Severity::Mild, MILD),
        band(40.0, true, Severity::Moderate, MODERATE),
    ],
    above: (Severity::Severe, SEVERE),
    source: PR_T16,
};

const R: InputSpec = num(
    "pisa_r_cm",
    tx!("PISA radius", "Raggio PISA"),
    Unit::Cm,
    0.2,
    1.5,
    2,
);
const VA: InputSpec = num(
    "alias_cm_s",
    tx!("Aliasing velocity", "Velocità di aliasing"),
    Unit::CmS,
    15.0,
    80.0,
    1,
);
const LVOT_D: InputSpec = num(
    "lvot_d_cm",
    tx!("LVOT diameter", "Diametro LVOT"),
    Unit::Cm,
    1.0,
    4.0,
    2,
);
const LVOT_VTI: InputSpec = num("lvot_vti_cm", tx!("LVOT VTI", "VTI LVOT"), Unit::Cm, 3.0, 60.0, 1);
const MA_D: InputSpec = num(
    "ma_d_cm",
    tx!("Mitral annulus diameter", "Diametro dell'anulus mitralico"),
    Unit::Cm,
    2.0,
    5.5,
    2,
);
const MA_VTI: InputSpec = num(
    "ma_vti_cm",
    tx!("Mitral annulus VTI", "VTI all'anulus mitralico"),
    Unit::Cm,
    3.0,
    40.0,
    1,
);

/// What the PISA calculators compute, shared by the three valves.
struct PisaPlan {
    vmax_id: &'static str,
    vti_id: &'static str,
    sv_id: Option<&'static str>,
    eroa: &'static Scale,
    rvol: &'static Scale,
    rf: Option<&'static Scale>,
}

fn pisa(i: &Inputs, plan: &PisaPlan) -> Result<Outcome, Vec<InputError>> {
    let (r, va, vmax, vti) = (
        i.n("pisa_r_cm"),
        i.n("alias_cm_s"),
        i.n(plan.vmax_id),
        i.n(plan.vti_id),
    );
    let mut out = outcome();
    let flow = pisa_flow_ml_s(r, va);
    out.push(
        q(
            "flow_ml_s",
            tx!("Regurgitant flow rate", "Portata rigurgitante"),
            flow,
            Unit::MlS,
            0,
        ),
        "Q = 2π × r² × Va",
        format!("2π × {}² × {}", n(r, 2), v(va, 1, Unit::CmS)),
    );
    let eroa = flow / (vmax * 100.0);
    out.push(
        q("eroa_cm2", EROA_W, eroa, Unit::Cm2, 2),
        "EROA = Q ÷ Vmax",
        format!("{} ÷ {} cm/s", n(flow, 0), n(vmax * 100.0, 0)),
    );
    let rvol = eroa * vti;
    out.push(
        q("rvol_ml", RVOL_W, rvol, Unit::Ml, 0),
        "RVol = EROA × VTI",
        format!("{} × {}", v(eroa, 2, Unit::Cm2), v(vti, 0, Unit::Cm)),
    );
    out.findings.push(plan.eroa.finding(eroa));
    out.findings.push(plan.rvol.finding(rvol));
    if let (Some(sv_id), Some(rf_scale)) = (plan.sv_id, plan.rf)
        && let Some(sv) = o(i, sv_id)
    {
        check(
            rvol >= sv,
            sv_id,
            tx!(
                "The regurgitant volume exceeds this stroke volume: check the inputs",
                "Il volume rigurgitante supera questo volume sistolico: controllare i dati"
            ),
        )?;
        let rf = rvol / sv * 100.0;
        out.push(
            q("rf_percent", RF_W, rf, Unit::Percent, 0),
            "RF = RVol ÷ SV × 100",
            format!("{} ÷ {} × 100", n(rvol, 0), v(sv, 0, Unit::Ml)),
        );
        out.findings.push(rf_scale.finding(rf));
    }
    Ok(out)
}

/// Mitral regurgitation by PISA.
pub static MR_PISA: Calculator = Calculator {
    id: "mitral-regurgitation-pisa",
    category: Category::Regurgitation,
    name: tx!("Mitral regurgitation (PISA)", "Insufficienza mitralica (PISA)"),
    synonyms: tx!(
        "MR, EROA, ERO, regurgitant volume, regurgitant fraction, flow convergence, proximal isovelocity",
        "IM, EROA, volume rigurgitante, frazione rigurgitante, convergenza di flusso, isovelocità prossimale"
    ),
    summary: tx!(
        "Flow converging on the ventricular side of the leak, taken as a hemisphere: EROA = 2πr² × Va ÷ peak MR velocity, and the regurgitant volume is EROA × MR VTI. Underestimates a non-circular orifice; for late-systolic MR the volume is the better index.",
        "Flusso che converge sul versante ventricolare del rigurgito, assunto emisferico: EROA = 2πr² × Va ÷ velocità di picco dell'IM, e il volume rigurgitante è EROA × VTI dell'IM. Sottostima un orifizio non circolare; nell'IM telesistolica conta di più il volume."
    ),
    inputs: &[
        R,
        VA,
        num(
            "mr_vmax_m_s",
            tx!("Peak MR velocity", "Velocità di picco dell'IM"),
            Unit::MS,
            3.0,
            7.0,
            2,
        ),
        num(
            "mr_vti_cm",
            tx!("MR jet VTI", "VTI del getto di IM"),
            Unit::Cm,
            50.0,
            300.0,
            0,
        ),
        opt(
            "lv_sv_ml",
            tx!(
                "Total LV stroke volume (for the fraction)",
                "Volume sistolico totale del VS (per la frazione)"
            ),
            Unit::Ml,
            20.0,
            300.0,
            0,
        ),
    ],
    compute: |i| {
        pisa(
            i,
            &PisaPlan {
                vmax_id: "mr_vmax_m_s",
                vti_id: "mr_vti_cm",
                sv_id: Some("lv_sv_ml"),
                eroa: &MR_EROA,
                rvol: &MR_RVOL,
                rf: Some(&MR_RF),
            },
        )
    },
    references: &[PISA_EQ, MR_T8],
    review: Review::Verified,
    example: &[
        ("pisa_r_cm", N(0.9)),
        ("alias_cm_s", N(38.0)),
        ("mr_vmax_m_s", N(5.0)),
        ("mr_vti_cm", N(150.0)),
        ("lv_sv_ml", N(130.0)),
    ],
    sim_link: None,
};

/// Aortic regurgitation by PISA.
pub static AR_PISA: Calculator = Calculator {
    id: "aortic-regurgitation-pisa",
    category: Category::Regurgitation,
    name: tx!("Aortic regurgitation (PISA)", "Insufficienza aortica (PISA)"),
    synonyms: tx!(
        "AR, AI, EROA, regurgitant volume, regurgitant fraction, flow convergence",
        "IA, EROA, volume rigurgitante, frazione rigurgitante, convergenza di flusso"
    ),
    summary: tx!(
        "The same hemispheric flow convergence, on the aortic side of the valve in diastole: EROA = 2πr² × Va ÷ peak AR velocity, regurgitant volume = EROA × AR VTI. Less used than in MR: the convergence zone is often hard to image.",
        "La stessa convergenza emisferica, sul versante aortico della valvola in diastole: EROA = 2πr² × Va ÷ velocità di picco dell'IA, volume rigurgitante = EROA × VTI dell'IA. Meno usata che nell'IM: la zona di convergenza è spesso difficile da visualizzare."
    ),
    inputs: &[
        R,
        VA,
        num(
            "ar_vmax_m_s",
            tx!("Peak AR velocity", "Velocità di picco dell'IA"),
            Unit::MS,
            2.0,
            6.5,
            2,
        ),
        num(
            "ar_vti_cm",
            tx!("AR jet VTI", "VTI del getto di IA"),
            Unit::Cm,
            50.0,
            300.0,
            0,
        ),
        opt(
            "lvot_sv_ml",
            tx!(
                "LVOT stroke volume (for the fraction)",
                "Volume sistolico LVOT (per la frazione)"
            ),
            Unit::Ml,
            20.0,
            300.0,
            0,
        ),
    ],
    compute: |i| {
        pisa(
            i,
            &PisaPlan {
                vmax_id: "ar_vmax_m_s",
                vti_id: "ar_vti_cm",
                sv_id: Some("lvot_sv_ml"),
                eroa: &AR_EROA,
                rvol: &AR_RVOL,
                rf: Some(&AR_RF),
            },
        )
    },
    references: &[PISA_EQ, AR_T11],
    review: Review::Verified,
    example: &[
        ("pisa_r_cm", N(0.6)),
        ("alias_cm_s", N(40.0)),
        ("ar_vmax_m_s", N(4.5)),
        ("ar_vti_cm", N(220.0)),
        ("lvot_sv_ml", N(140.0)),
    ],
    sim_link: None,
};

/// Tricuspid regurgitation by PISA, with the vena contracta width if measured.
pub static TR_PISA: Calculator = Calculator {
    id: "tricuspid-regurgitation-pisa",
    category: Category::Regurgitation,
    name: tx!(
        "Tricuspid regurgitation (PISA)",
        "Insufficienza tricuspidale (PISA)"
    ),
    synonyms: tx!(
        "TR, EROA, regurgitant volume, vena contracta, flow convergence",
        "IT, EROA, volume rigurgitante, vena contracta, convergenza di flusso"
    ),
    summary: tx!(
        "PISA applied to the tricuspid valve. TR velocities are lower than MR ones, so the aliasing velocity is lowered too; the guideline has less validation than for MR and the 2D method underestimates a crescent-shaped orifice.",
        "PISA applicata alla tricuspide. Le velocità dell'IT sono più basse di quelle dell'IM e si abbassa anche la velocità di aliasing; la validazione è minore che per l'IM e il metodo 2D sottostima un orifizio a mezzaluna."
    ),
    inputs: &[
        R,
        VA,
        num(
            "tr_vmax_m_s",
            tx!("Peak TR velocity", "Velocità di picco dell'IT"),
            Unit::MS,
            1.5,
            5.5,
            2,
        ),
        num(
            "tr_vti_cm",
            tx!("TR jet VTI", "VTI del getto di IT"),
            Unit::Cm,
            20.0,
            200.0,
            0,
        ),
        opt(
            "vc_width_cm",
            tx!("Vena contracta width", "Larghezza della vena contracta"),
            Unit::Cm,
            0.05,
            2.5,
            2,
        ),
    ],
    compute: |i| {
        let mut out = pisa(
            i,
            &PisaPlan {
                vmax_id: "tr_vmax_m_s",
                vti_id: "tr_vti_cm",
                sv_id: None,
                eroa: &TR_EROA,
                rvol: &TR_RVOL,
                rf: None,
            },
        )?;
        if let Some(vc) = o(i, "vc_width_cm") {
            out.findings.push(TR_VC.finding(vc));
        }
        Ok(out)
    },
    references: &[PISA_EQ, TR_T14],
    review: Review::Verified,
    example: &[
        ("pisa_r_cm", N(0.8)),
        ("alias_cm_s", N(28.0)),
        ("tr_vmax_m_s", N(2.8)),
        ("tr_vti_cm", N(75.0)),
        ("vc_width_cm", N(0.6)),
    ],
    sim_link: None,
};

/// Which valve leaks, for the volumetric method.
#[derive(Clone, Copy)]
enum Leak {
    Mitral,
    Aortic,
    Pulmonic,
}

fn volumetric(i: &Inputs, leak: Leak) -> Result<Outcome, Vec<InputError>> {
    let lvot = stroke_volume_ml(i.n("lvot_d_cm"), i.n("lvot_vti_cm"));
    let mut out = outcome();
    let (reg_id, reg_label, reg_d, reg_vti, comp_label) = match leak {
        Leak::Mitral => (
            "mv_sv_ml",
            tx!(
                "Mitral annulus stroke volume",
                "Volume sistolico all'anulus mitralico"
            ),
            "ma_d_cm",
            "ma_vti_cm",
            "SV(LVOT)",
        ),
        Leak::Aortic => (
            "lvot_sv_ml",
            tx!("LVOT stroke volume", "Volume sistolico LVOT"),
            "lvot_d_cm",
            "lvot_vti_cm",
            "SV(MV)",
        ),
        Leak::Pulmonic => (
            "rvot_sv_ml",
            tx!("RVOT stroke volume", "Volume sistolico RVOT"),
            "rvot_d_cm",
            "rvot_vti_cm",
            "SV(LVOT)",
        ),
    };
    let (d, vti) = (i.n(reg_d), i.n(reg_vti));
    let reg = stroke_volume_ml(d, vti);
    out.push(
        q(reg_id, reg_label, reg, Unit::Ml, 0),
        "SV = π × (d/2)² × VTI",
        format!("π × ({} / 2)² × {}", v(d, 2, Unit::Cm), v(vti, 1, Unit::Cm)),
    );
    let comp = match leak {
        Leak::Aortic => {
            let (d, vti) = (i.n("ma_d_cm"), i.n("ma_vti_cm"));
            let sv = stroke_volume_ml(d, vti);
            out.push(
                q(
                    "mv_sv_ml",
                    tx!(
                        "Mitral annulus stroke volume",
                        "Volume sistolico all'anulus mitralico"
                    ),
                    sv,
                    Unit::Ml,
                    0,
                ),
                "SV = π × (d/2)² × VTI",
                format!("π × ({} / 2)² × {}", v(d, 2, Unit::Cm), v(vti, 1, Unit::Cm)),
            );
            sv
        }
        Leak::Mitral | Leak::Pulmonic => {
            let (d, vti) = (i.n("lvot_d_cm"), i.n("lvot_vti_cm"));
            out.push(
                q(
                    "lvot_sv_ml",
                    tx!("LVOT stroke volume", "Volume sistolico LVOT"),
                    lvot,
                    Unit::Ml,
                    0,
                ),
                "SV = π × (d/2)² × VTI",
                format!("π × ({} / 2)² × {}", v(d, 2, Unit::Cm), v(vti, 1, Unit::Cm)),
            );
            lvot
        }
    };
    check(
        reg <= comp,
        reg_d,
        tx!(
            "The stroke volume at the leaking valve must exceed the one at the competent site",
            "Il volume sistolico alla valvola insufficiente deve superare quello della sede continente"
        ),
    )?;
    let rvol = reg - comp;
    out.push(
        q("rvol_ml", RVOL_W, rvol, Unit::Ml, 0),
        &format!("RVol = SV(regurgitant) − {comp_label}"),
        format!("{} − {}", n(reg, 0), v(comp, 0, Unit::Ml)),
    );
    let rf = rvol / reg * 100.0;
    out.push(
        q("rf_percent", RF_W, rf, Unit::Percent, 0),
        "RF = RVol ÷ SV(regurgitant) × 100",
        format!("{} ÷ {} × 100", n(rvol, 0), n(reg, 0)),
    );
    let jet_vti = match leak {
        Leak::Mitral => o(i, "mr_vti_cm"),
        Leak::Aortic => o(i, "ar_vti_cm"),
        Leak::Pulmonic => None,
    };
    match leak {
        Leak::Mitral => {
            out.findings.push(MR_RVOL.finding(rvol));
            out.findings.push(MR_RF.finding(rf));
        }
        Leak::Aortic => {
            out.findings.push(AR_RVOL.finding(rvol));
            out.findings.push(AR_RF.finding(rf));
        }
        Leak::Pulmonic => {
            out.findings.push(PR_RF.finding(rf));
            if let Some(pht) = o(i, "pr_pht_ms")
                && lt(pht, 100.0)
            {
                out.findings.push(criterion_met(
                    Severity::Info,
                    tx!(
                        "The PR pressure half-time is in the range the table lists for severe PR (steep deceleration is not specific)",
                        "Il tempo di dimezzamento dell'IP è nell'intervallo che la tabella elenca per IP grave (una decelerazione ripida non è specifica)"
                    ),
                    "PHT < 100 ms",
                    PR_T16,
                ));
            }
        }
    }
    if let Some(jvti) = jet_vti {
        let eroa = rvol / jvti;
        out.push(
            q("eroa_cm2", EROA_W, eroa, Unit::Cm2, 2),
            "EROA = RVol ÷ VTI(jet)",
            format!("{} ÷ {}", n(rvol, 0), v(jvti, 0, Unit::Cm)),
        );
        out.findings.push(if matches!(leak, Leak::Mitral) {
            MR_EROA.finding(eroa)
        } else {
            AR_EROA.finding(eroa)
        });
    }
    Ok(out)
}

/// Mitral regurgitation by the volumetric method.
pub static MR_VOLUMETRIC: Calculator = Calculator {
    id: "mitral-regurgitation-volumetric",
    category: Category::Regurgitation,
    name: tx!(
        "Mitral regurgitation (volumetric)",
        "Insufficienza mitralica (metodo volumetrico)"
    ),
    synonyms: tx!(
        "MR, regurgitant volume, regurgitant fraction, stroke volume difference, mitral annulus, quantitative Doppler",
        "IM, volume rigurgitante, frazione rigurgitante, differenza di volumi sistolici, anulus mitralico, Doppler quantitativo"
    ),
    summary: tx!(
        "Total stroke volume through the mitral annulus minus the forward volume through the LVOT is the mitral regurgitant volume. Valid only without significant aortic regurgitation; errors in either diameter are squared.",
        "Il volume sistolico totale all'anulus mitralico meno il volume anterogrado nel LVOT è il volume rigurgitante mitralico. Valido solo senza insufficienza aortica significativa; gli errori sui diametri sono elevati al quadrato."
    ),
    inputs: &[
        LVOT_D,
        LVOT_VTI,
        MA_D,
        MA_VTI,
        opt(
            "mr_vti_cm",
            tx!("MR jet VTI (for EROA)", "VTI del getto di IM (per l'EROA)"),
            Unit::Cm,
            50.0,
            300.0,
            0,
        ),
    ],
    compute: |i| volumetric(i, Leak::Mitral),
    references: &[VOL_EQ, MR_T8],
    review: Review::Verified,
    example: &[
        ("lvot_d_cm", N(2.2)),
        ("lvot_vti_cm", N(17.0)),
        ("ma_d_cm", N(3.4)),
        ("ma_vti_cm", N(17.0)),
        ("mr_vti_cm", N(140.0)),
    ],
    sim_link: None,
};

/// Aortic regurgitation by the volumetric method.
pub static AR_VOLUMETRIC: Calculator = Calculator {
    id: "aortic-regurgitation-volumetric",
    category: Category::Regurgitation,
    name: tx!(
        "Aortic regurgitation (volumetric)",
        "Insufficienza aortica (metodo volumetrico)"
    ),
    synonyms: tx!(
        "AR, AI, regurgitant volume, regurgitant fraction, stroke volume difference",
        "IA, volume rigurgitante, frazione rigurgitante, differenza di volumi sistolici"
    ),
    summary: tx!(
        "Stroke volume through the LVOT (forward plus regurgitant) minus the forward volume through the mitral annulus. Valid only without significant mitral regurgitation.",
        "Volume sistolico nel LVOT (anterogrado più rigurgitante) meno il volume anterogrado all'anulus mitralico. Valido solo senza insufficienza mitralica significativa."
    ),
    inputs: &[
        LVOT_D,
        LVOT_VTI,
        MA_D,
        MA_VTI,
        opt(
            "ar_vti_cm",
            tx!("AR jet VTI (for EROA)", "VTI del getto di IA (per l'EROA)"),
            Unit::Cm,
            50.0,
            300.0,
            0,
        ),
    ],
    compute: |i| volumetric(i, Leak::Aortic),
    references: &[VOL_EQ, AR_T11],
    review: Review::Verified,
    example: &[
        ("lvot_d_cm", N(2.3)),
        ("lvot_vti_cm", N(28.0)),
        ("ma_d_cm", N(3.2)),
        ("ma_vti_cm", N(12.0)),
        ("ar_vti_cm", N(210.0)),
    ],
    sim_link: None,
};

/// Pulmonic regurgitation by the volumetric method.
pub static PR_VOLUMETRIC: Calculator = Calculator {
    id: "pulmonic-regurgitation-volumetric",
    category: Category::Regurgitation,
    name: tx!(
        "Pulmonic regurgitation (volumetric)",
        "Insufficienza polmonare (metodo volumetrico)"
    ),
    synonyms: tx!(
        "PR, PI, regurgitant fraction, RVOT stroke volume, pulmonary valve",
        "IP, frazione rigurgitante, volume sistolico RVOT, valvola polmonare"
    ),
    summary: tx!(
        "RVOT stroke volume (forward plus regurgitant) minus LVOT stroke volume. The fraction cut-offs come mainly from CMR and are not well validated for echocardiography.",
        "Volume sistolico nel RVOT (anterogrado più rigurgitante) meno il volume sistolico nel LVOT. Le soglie della frazione derivano soprattutto dalla RM e sono poco validate per l'ecocardiografia."
    ),
    inputs: &[
        num(
            "rvot_d_cm",
            tx!(
                "RVOT (pulmonary annulus) diameter",
                "Diametro RVOT (anulus polmonare)"
            ),
            Unit::Cm,
            1.5,
            4.5,
            2,
        ),
        num("rvot_vti_cm", tx!("RVOT VTI", "VTI RVOT"), Unit::Cm, 5.0, 50.0, 1),
        LVOT_D,
        LVOT_VTI,
        opt(
            "pr_pht_ms",
            tx!("PR pressure half-time", "Tempo di dimezzamento dell'IP"),
            Unit::Ms,
            30.0,
            600.0,
            0,
        ),
    ],
    compute: |i| volumetric(i, Leak::Pulmonic),
    references: &[VOL_EQ, PR_T16],
    review: Review::Verified,
    example: &[
        ("rvot_d_cm", N(2.6)),
        ("rvot_vti_cm", N(24.0)),
        ("lvot_d_cm", N(2.1)),
        ("lvot_vti_cm", N(21.0)),
        ("pr_pht_ms", N(140.0)),
    ],
    sim_link: None,
};

#[cfg(test)]
mod tests {
    use super::super::testing::{close, inconsistent, ok, severities};
    use super::*;

    #[test]
    fn mr_pisa_against_the_guideline_example_and_hand_case() {
        // Figure 11 caption, p. 321: r 1.0, Va 29.9, peak 527 cm/s, VTI 76 → 0.36 cm², 27 mL
        let i = Inputs::new()
            .with("pisa_r_cm", 1.0)
            .with("alias_cm_s", 29.9)
            .with("mr_vmax_m_s", 5.27)
            .with("mr_vti_cm", 76.0);
        let o = ok(&MR_PISA, &i);
        close(o.value("eroa_cm2"), 0.356, 0.001);
        close(o.value("rvol_ml"), 27.0, 0.5);
        // 2π × 0.8² × 40 = 160.8 mL/s ÷ 500 = 0.322 cm²; × 150 = 48.3 mL; ÷ 120 = 40 %
        let i = Inputs::new()
            .with("pisa_r_cm", 0.8)
            .with("alias_cm_s", 40.0)
            .with("mr_vmax_m_s", 5.0)
            .with("mr_vti_cm", 150.0)
            .with("lv_sv_ml", 120.0);
        let o = ok(&MR_PISA, &i);
        close(o.value("eroa_cm2"), 0.3217, 1e-3);
        close(o.value("rvol_ml"), 48.25, 0.05);
        close(o.value("rf_percent"), 40.2, 0.1);
        assert_eq!(
            severities(&o),
            [Severity::Moderate, Severity::Moderate, Severity::Moderate]
        );
        inconsistent(&MR_PISA, &i.with("lv_sv_ml", 40.0), "lv_sv_ml");
    }

    #[test]
    fn regurgitation_boundaries() {
        for (s, a, b, c) in [(&MR_EROA, 0.20, 0.30, 0.40), (&AR_EROA, 0.10, 0.20, 0.30)] {
            assert_eq!(s.class(a - 0.01).1, MILD);
            assert_eq!(s.class(a).1, MILD_MOD);
            assert_eq!(s.class(b - 0.01).1, MILD_MOD);
            assert_eq!(s.class(b).1, MOD_SEV);
            assert_eq!(s.class(c - 0.01).1, MOD_SEV);
            assert_eq!(s.class(c).0, Severity::Severe);
        }
        for s in [&MR_RVOL, &AR_RVOL] {
            assert_eq!(s.class(29.0).0, Severity::Mild);
            assert_eq!(s.class(30.0).1, MILD_MOD);
            assert_eq!(s.class(45.0).1, MOD_SEV);
            assert_eq!(s.class(59.0).1, MOD_SEV);
            assert_eq!(s.class(60.0).0, Severity::Severe);
        }
        for s in [&MR_RF, &AR_RF] {
            assert_eq!(s.class(29.0).0, Severity::Mild);
            assert_eq!(s.class(30.0).1, MILD_MOD);
            assert_eq!(s.class(49.0).1, MOD_SEV);
            assert_eq!(s.class(50.0).0, Severity::Severe);
        }
        assert_eq!(TR_EROA.class(0.19).0, Severity::Mild);
        assert_eq!(TR_EROA.class(0.20).0, Severity::Moderate);
        assert_eq!(TR_EROA.class(0.39).0, Severity::Moderate);
        assert_eq!(TR_EROA.class(0.40).0, Severity::Severe);
        assert_eq!(TR_RVOL.class(44.0).0, Severity::Moderate);
        assert_eq!(TR_RVOL.class(45.0).0, Severity::Severe);
        assert_eq!(TR_VC.class(0.69).0, Severity::Moderate);
        assert_eq!(TR_VC.class(0.7).0, Severity::Severe);
        assert_eq!(PR_RF.class(19.0).0, Severity::Mild);
        assert_eq!(PR_RF.class(20.0).0, Severity::Moderate);
        assert_eq!(PR_RF.class(40.0).0, Severity::Moderate);
        assert_eq!(PR_RF.class(41.0).0, Severity::Severe);
    }

    #[test]
    fn mr_volumetric_against_figure_4() {
        // Figure 4, p. 311: MV d 3.5 cm VTI 19 → 183 mL; LVOT d 2.3 VTI 14 → 58 mL; RVol 125 mL, RF 68 %
        let i = Inputs::new()
            .with("lvot_d_cm", 2.3)
            .with("lvot_vti_cm", 14.0)
            .with("ma_d_cm", 3.5)
            .with("ma_vti_cm", 19.0);
        let o = ok(&MR_VOLUMETRIC, &i);
        close(o.value("mv_sv_ml"), 182.8, 0.2);
        close(o.value("lvot_sv_ml"), 58.2, 0.2);
        close(o.value("rvol_ml"), 124.6, 0.3);
        close(o.value("rf_percent"), 68.2, 0.2);
        assert_eq!(severities(&o), [Severity::Severe, Severity::Severe]);
        // swapped sites: no regurgitant volume
        let bad = Inputs::new()
            .with("lvot_d_cm", 3.5)
            .with("lvot_vti_cm", 19.0)
            .with("ma_d_cm", 2.3)
            .with("ma_vti_cm", 14.0);
        inconsistent(&MR_VOLUMETRIC, &bad, "ma_d_cm");
    }

    #[test]
    fn ar_and_pr_volumetric_hand_cases() {
        // LVOT π × 1.0² × 30 = 94.2; MA π × 1.5² × 8 = 56.5; RVol 37.7, RF 40 %; EROA 37.7 ÷ 200 = 0.19
        let i = Inputs::new()
            .with("lvot_d_cm", 2.0)
            .with("lvot_vti_cm", 30.0)
            .with("ma_d_cm", 3.0)
            .with("ma_vti_cm", 8.0)
            .with("ar_vti_cm", 200.0);
        let o = ok(&AR_VOLUMETRIC, &i);
        close(o.value("rvol_ml"), 37.70, 0.01);
        close(o.value("rf_percent"), 40.0, 0.01);
        close(o.value("eroa_cm2"), 0.1885, 1e-3);
        // RVOT π × 1.25² × 20 = 98.2; LVOT π × 20 = 62.8; RF 36 % (moderate); PHT 90 < 100 noted
        let i = Inputs::new()
            .with("rvot_d_cm", 2.5)
            .with("rvot_vti_cm", 20.0)
            .with("lvot_d_cm", 2.0)
            .with("lvot_vti_cm", 20.0)
            .with("pr_pht_ms", 90.0);
        let o = ok(&PR_VOLUMETRIC, &i);
        close(o.value("rf_percent"), 36.0, 0.01);
        assert_eq!(severities(&o), [Severity::Moderate, Severity::Info]);
    }

    #[test]
    fn tr_pisa_hand_case() {
        // 2π × 0.9² × 30 = 152.7 mL/s ÷ 300 cm/s = 0.509 cm²; × 80 = 40.7 mL
        let i = Inputs::new()
            .with("pisa_r_cm", 0.9)
            .with("alias_cm_s", 30.0)
            .with("tr_vmax_m_s", 3.0)
            .with("tr_vti_cm", 80.0)
            .with("vc_width_cm", 0.7);
        let o = ok(&TR_PISA, &i);
        close(o.value("eroa_cm2"), 0.5089, 1e-3);
        close(o.value("rvol_ml"), 40.7, 0.1);
        assert_eq!(
            severities(&o),
            [Severity::Severe, Severity::Moderate, Severity::Severe]
        );
    }
}
