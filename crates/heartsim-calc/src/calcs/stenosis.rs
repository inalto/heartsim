//! Native valve stenosis: aortic valve area by continuity, mitral valve area by pressure half-time,
//! continuity and PISA, pulmonic peak gradient, tricuspid stenosis criteria.

use super::words::{MODERATE, SEVERE};
use super::{check, cite, n, need_one, num, o, opt, outcome, q, v};
use crate::blocks::{
    bernoulli_mmhg, circle_area_cm2, continuity_area_cm2, pisa_angle_corrected, pisa_flow_ml_s,
};
use crate::grade::{Scale, band, criterion_met, ge, le};
use crate::model::ExampleValue::Number as N;
use crate::model::{
    Calculator, Category, Cite, InputError, InputSpec, Inputs, Outcome, Review, Severity, Unit,
};
use crate::refs::{AS_2017, RHD_2023, VS_2009};
use crate::tx;

const AS_TABLE3: Cite = cite(&AS_2017, "Table 3, p. 383");
const MS_TABLE1: Cite = cite(&RHD_2023, "Table 1, p. 8");

const LVOT_D: InputSpec = num(
    "lvot_d_cm",
    tx!("LVOT diameter", "Diametro del tratto di efflusso (LVOT)"),
    Unit::Cm,
    1.0,
    4.0,
    2,
);
const LVOT_VTI: InputSpec = num(
    "lvot_vti_cm",
    tx!("LVOT VTI (pulsed wave)", "VTI LVOT (Doppler pulsato)"),
    Unit::Cm,
    3.0,
    60.0,
    1,
);

// ---------------------------------------------------------------------------------------------
// Aortic valve area

static AVA_SCALE: Scale = Scale {
    what: tx!("Aortic valve area", "Area valvolare aortica"),
    symbol: "AVA",
    unit: Unit::Cm2,
    decimals: 1,
    bands: &[
        band(1.0, false, Severity::Severe, SEVERE),
        band(1.5, true, Severity::Moderate, MODERATE),
    ],
    above: (Severity::Mild, tx!("mild", "lieve")),
    source: AS_TABLE3,
};
static AVAI_SCALE: Scale = Scale {
    what: tx!("Indexed aortic valve area", "Area valvolare aortica indicizzata"),
    symbol: "AVAi",
    unit: Unit::Cm2M2,
    decimals: 2,
    bands: &[
        band(0.60, false, Severity::Severe, SEVERE),
        band(0.85, true, Severity::Moderate, MODERATE),
    ],
    above: (Severity::Mild, tx!("mild", "lieve")),
    source: AS_TABLE3,
};
static VR_SCALE: Scale = Scale {
    what: tx!("Velocity ratio", "Rapporto delle velocità"),
    symbol: "VR",
    unit: Unit::None,
    decimals: 2,
    bands: &[
        band(0.25, false, Severity::Severe, SEVERE),
        band(0.50, true, Severity::Moderate, MODERATE),
    ],
    above: (Severity::Mild, tx!("mild", "lieve")),
    source: AS_TABLE3,
};
/// Table 3 lists ≤2.5 (sclerosis), 2.6–2.9, 3.0–4.0 and ≥4.0 m/s: values between 2.5 and 2.6 go to
/// mild, and 4.0 itself to severe (the text: "a peak velocity ≥4.0 m/s").
static AS_VMAX_SCALE: Scale = Scale {
    what: tx!("Peak aortic velocity", "Velocità aortica di picco"),
    symbol: "Vmax",
    unit: Unit::MS,
    decimals: 1,
    bands: &[
        band(
            2.5,
            true,
            Severity::Normal,
            tx!("aortic sclerosis", "sclerosi aortica"),
        ),
        band(3.0, false, Severity::Mild, tx!("mild", "lieve")),
        band(4.0, false, Severity::Moderate, MODERATE),
    ],
    above: (Severity::Severe, SEVERE),
    source: AS_TABLE3,
};
static AS_MEAN_SCALE: Scale = Scale {
    what: tx!("Mean aortic gradient", "Gradiente aortico medio"),
    symbol: "ΔPmean",
    unit: Unit::MmHg,
    decimals: 0,
    bands: &[
        band(20.0, false, Severity::Mild, tx!("mild", "lieve")),
        band(40.0, false, Severity::Moderate, MODERATE),
    ],
    above: (Severity::Severe, SEVERE),
    source: AS_TABLE3,
};

/// Aortic valve area by the continuity equation, with the velocity ratio and the indexed area.
pub static AORTIC_VALVE_AREA: Calculator = Calculator {
    id: "aortic-valve-area",
    category: Category::Stenosis,
    name: tx!(
        "Aortic valve area (continuity)",
        "Area valvolare aortica (equazione di continuità)"
    ),
    synonyms: tx!(
        "AVA, aortic stenosis, AS, continuity equation, velocity ratio, dimensionless index, AVAi, mean gradient, peak velocity",
        "AVA, stenosi aortica, equazione di continuità, rapporto delle velocità, indice adimensionale, AVAi, gradiente medio, velocità di picco"
    ),
    summary: tx!(
        "The stroke volume measured below the valve must pass through the stenotic orifice, so the effective orifice is the LVOT area times the LVOT VTI divided by the aortic jet VTI. Assumes a circular LVOT and the pulsed sample at the level of the diameter.",
        "Lo stesso volume sistolico misurato sotto la valvola attraversa l'orifizio stenotico: l'area effettiva è l'area del LVOT per la VTI del LVOT diviso la VTI del getto aortico. Presuppone un LVOT circolare e il campione pulsato allo stesso livello del diametro."
    ),
    inputs: &[
        LVOT_D,
        LVOT_VTI,
        num(
            "av_vti_cm",
            tx!(
                "Aortic jet VTI (continuous wave)",
                "VTI del getto aortico (Doppler continuo)"
            ),
            Unit::Cm,
            10.0,
            250.0,
            0,
        ),
        opt(
            "lvot_vmax_m_s",
            tx!("LVOT peak velocity", "Velocità di picco LVOT"),
            Unit::MS,
            0.3,
            2.5,
            2,
        ),
        opt(
            "av_vmax_m_s",
            tx!("Aortic jet peak velocity", "Velocità di picco del getto aortico"),
            Unit::MS,
            0.5,
            7.0,
            2,
        ),
        opt(
            "av_mean_mmhg",
            tx!(
                "Mean aortic gradient (traced)",
                "Gradiente aortico medio (tracciato)"
            ),
            Unit::MmHg,
            1.0,
            150.0,
            0,
        ),
        opt(
            "bsa_m2",
            tx!("Body surface area", "Superficie corporea"),
            Unit::M2,
            0.5,
            3.0,
            2,
        ),
    ],
    compute: aortic_valve_area,
    references: &[cite(
        &AS_2017,
        "Continuity equation p. 379; velocity ratio p. 382; Table 3 p. 383",
    )],
    review: Review::Verified,
    example: &[
        ("lvot_d_cm", N(2.1)),
        ("lvot_vti_cm", N(20.0)),
        ("av_vti_cm", N(95.0)),
        ("lvot_vmax_m_s", N(0.95)),
        ("av_vmax_m_s", N(4.3)),
        ("av_mean_mmhg", N(48.0)),
        ("bsa_m2", N(1.85)),
    ],
    sim_link: None,
};

fn aortic_valve_area(i: &Inputs) -> Result<Outcome, Vec<InputError>> {
    let (d, lvti, avti) = (i.n("lvot_d_cm"), i.n("lvot_vti_cm"), i.n("av_vti_cm"));
    let mut out = outcome();
    let csa = circle_area_cm2(d);
    out.push(
        q("lvot_area_cm2", tx!("LVOT area", "Area LVOT"), csa, Unit::Cm2, 2),
        "CSA = π × (d/2)²",
        format!("π × ({} / 2)²", v(d, 2, Unit::Cm)),
    );
    let ava = continuity_area_cm2(csa, lvti, avti);
    out.push(
        q(
            "ava_cm2",
            tx!("Aortic valve area", "Area valvolare aortica"),
            ava,
            Unit::Cm2,
            2,
        ),
        "AVA = CSA × VTI(LVOT) ÷ VTI(AV)",
        format!(
            "{} × {} ÷ {}",
            v(csa, 2, Unit::Cm2),
            v(lvti, 1, Unit::Cm),
            v(avti, 0, Unit::Cm)
        ),
    );
    out.push(
        q(
            "vti_ratio",
            tx!(
                "VTI ratio (dimensionless index)",
                "Rapporto delle VTI (indice adimensionale)"
            ),
            lvti / avti,
            Unit::None,
            2,
        ),
        "DI = VTI(LVOT) ÷ VTI(AV)",
        format!("{} ÷ {}", n(lvti, 1), n(avti, 0)),
    );
    out.findings.push(AVA_SCALE.finding(ava));
    if let Some(bsa) = o(i, "bsa_m2") {
        let avai = ava / bsa;
        out.push(
            q(
                "avai_cm2_m2",
                tx!("Indexed aortic valve area", "Area valvolare aortica indicizzata"),
                avai,
                Unit::Cm2M2,
                2,
            ),
            "AVAi = AVA ÷ BSA",
            format!("{} ÷ {}", v(ava, 2, Unit::Cm2), v(bsa, 2, Unit::M2)),
        );
        out.findings.push(AVAI_SCALE.finding(avai));
    }
    let av_vmax = o(i, "av_vmax_m_s");
    if let Some(vj) = av_vmax {
        out.push(
            q(
                "av_peak_gradient_mmhg",
                tx!("Peak aortic gradient", "Gradiente aortico di picco"),
                bernoulli_mmhg(vj),
                Unit::MmHg,
                0,
            ),
            "ΔPmax = 4 × Vmax²",
            format!("4 × {}²", n(vj, 2)),
        );
        out.findings.push(AS_VMAX_SCALE.finding(vj));
    }
    if let (Some(vl), Some(vj)) = (o(i, "lvot_vmax_m_s"), av_vmax) {
        check(
            vl >= vj,
            "lvot_vmax_m_s",
            tx!(
                "The LVOT velocity must be lower than the aortic jet velocity",
                "La velocità nel LVOT deve essere inferiore a quella del getto aortico"
            ),
        )?;
        let vr = vl / vj;
        out.push(
            q(
                "velocity_ratio",
                tx!("Velocity ratio", "Rapporto delle velocità"),
                vr,
                Unit::None,
                2,
            ),
            "VR = V(LVOT) ÷ V(AV)",
            format!("{} ÷ {}", n(vl, 2), n(vj, 2)),
        );
        out.findings.push(VR_SCALE.finding(vr));
    }
    if let Some(mg) = o(i, "av_mean_mmhg") {
        out.findings.push(AS_MEAN_SCALE.finding(mg));
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// Mitral valve area

static MVA_SCALE: Scale = Scale {
    what: tx!("Mitral valve area", "Area valvolare mitralica"),
    symbol: "MVA",
    unit: Unit::Cm2,
    decimals: 1,
    bands: &[
        band(1.5, true, Severity::Severe, SEVERE),
        band(
            2.5,
            true,
            Severity::Moderate,
            tx!("progressive (moderate)", "progressiva (moderata)"),
        ),
    ],
    above: (Severity::Mild, tx!("progressive (mild)", "progressiva (lieve)")),
    source: MS_TABLE1,
};
static PHT_SCALE: Scale = Scale {
    what: tx!(
        "Mitral pressure half-time",
        "Tempo di dimezzamento della pressione mitralica"
    ),
    symbol: "PHT",
    unit: Unit::Ms,
    decimals: 0,
    bands: &[
        band(
            100.0,
            false,
            Severity::Mild,
            tx!("progressive (mild)", "progressiva (lieve)"),
        ),
        band(
            150.0,
            false,
            Severity::Moderate,
            tx!("progressive (moderate)", "progressiva (moderata)"),
        ),
    ],
    above: (Severity::Severe, SEVERE),
    source: MS_TABLE1,
};
static MS_MEAN_SCALE: Scale = Scale {
    what: tx!(
        "Mean mitral gradient (at 60–80 bpm)",
        "Gradiente mitralico medio (a 60–80 bpm)"
    ),
    symbol: "ΔPmean",
    unit: Unit::MmHg,
    decimals: 0,
    bands: &[
        band(
            5.0,
            false,
            Severity::Mild,
            tx!("progressive (mild)", "progressiva (lieve)"),
        ),
        band(
            10.0,
            false,
            Severity::Moderate,
            tx!("progressive (moderate)", "progressiva (moderata)"),
        ),
    ],
    above: (Severity::Severe, SEVERE),
    source: MS_TABLE1,
};

/// Mitral valve area from the pressure half-time (the empirical constant 220).
pub static MITRAL_AREA_PHT: Calculator = Calculator {
    id: "mitral-area-pht",
    category: Category::Stenosis,
    name: tx!(
        "Mitral valve area (pressure half-time)",
        "Area valvolare mitralica (tempo di dimezzamento)"
    ),
    synonyms: tx!(
        "MVA, mitral stenosis, MS, PHT, pressure half-time, Hatle, 220, mean gradient",
        "MVA, stenosi mitralica, PHT, tempo di dimezzamento della pressione, Hatle, 220, gradiente medio"
    ),
    summary: tx!(
        "The slower the transmitral gradient falls, the smaller the orifice: MVA ≈ 220 ÷ PHT. Unreliable when atrial or ventricular compliance is abnormal, with significant aortic regurgitation, and just after balloon valvotomy.",
        "Quanto più lentamente cala il gradiente transmitralico, tanto più piccolo è l'orifizio: MVA ≈ 220 ÷ PHT. Poco affidabile con compliance atriale o ventricolare alterata, insufficienza aortica significativa e subito dopo valvuloplastica."
    ),
    inputs: &[
        num(
            "pht_ms",
            tx!("Pressure half-time", "Tempo di dimezzamento della pressione"),
            Unit::Ms,
            30.0,
            600.0,
            0,
        ),
        opt(
            "mean_gradient_mmhg",
            tx!("Mean transmitral gradient", "Gradiente transmitralico medio"),
            Unit::MmHg,
            0.5,
            40.0,
            1,
        ),
    ],
    compute: mitral_area_pht,
    references: &[cite(&RHD_2023, "PHT equation and Table 1, p. 8")],
    review: Review::Verified,
    example: &[("pht_ms", N(190.0)), ("mean_gradient_mmhg", N(9.0))],
    sim_link: None,
};

fn mitral_area_pht(i: &Inputs) -> Result<Outcome, Vec<InputError>> {
    let pht = i.n("pht_ms");
    let mva = 220.0 / pht;
    let mut out = outcome();
    out.push(
        q(
            "mva_cm2",
            tx!("Mitral valve area", "Area valvolare mitralica"),
            mva,
            Unit::Cm2,
            2,
        ),
        "MVA = 220 ÷ PHT",
        format!("220 ÷ {}", n(pht, 0)),
    );
    out.findings.push(MVA_SCALE.finding(mva));
    out.findings.push(PHT_SCALE.finding(pht));
    if let Some(mg) = o(i, "mean_gradient_mmhg") {
        out.findings.push(MS_MEAN_SCALE.finding(mg));
    }
    Ok(out)
}

/// Mitral valve area by the continuity equation (LVOT stroke volume ÷ mitral inflow VTI).
pub static MITRAL_AREA_CONTINUITY: Calculator = Calculator {
    id: "mitral-area-continuity",
    category: Category::Stenosis,
    name: tx!(
        "Mitral valve area (continuity)",
        "Area valvolare mitralica (equazione di continuità)"
    ),
    synonyms: tx!(
        "MVA, mitral stenosis, continuity equation, stroke volume, inflow VTI",
        "MVA, stenosi mitralica, equazione di continuità, volume sistolico, VTI di afflusso"
    ),
    summary: tx!(
        "The forward stroke volume through the LVOT also crosses the mitral orifice in diastole: MVA = LVOT area × LVOT VTI ÷ mitral inflow VTI. Not valid with significant aortic or mitral regurgitation.",
        "Il volume sistolico anterogrado del LVOT attraversa in diastole anche l'orifizio mitralico: MVA = area LVOT × VTI LVOT ÷ VTI dell'afflusso mitralico. Non valido con insufficienza aortica o mitralica significativa."
    ),
    inputs: &[
        LVOT_D,
        LVOT_VTI,
        num(
            "mv_vti_cm",
            tx!(
                "Mitral inflow VTI (continuous wave)",
                "VTI dell'afflusso mitralico (Doppler continuo)"
            ),
            Unit::Cm,
            10.0,
            200.0,
            0,
        ),
    ],
    compute: mitral_area_continuity,
    references: &[cite(&RHD_2023, "Continuity equation and Table 1, p. 8")],
    review: Review::Verified,
    example: &[
        ("lvot_d_cm", N(2.0)),
        ("lvot_vti_cm", N(18.0)),
        ("mv_vti_cm", N(55.0)),
    ],
    sim_link: None,
};

fn mitral_area_continuity(i: &Inputs) -> Result<Outcome, Vec<InputError>> {
    let (d, lvti, mvti) = (i.n("lvot_d_cm"), i.n("lvot_vti_cm"), i.n("mv_vti_cm"));
    let csa = circle_area_cm2(d);
    let mut out = outcome();
    out.push(
        q("lvot_area_cm2", tx!("LVOT area", "Area LVOT"), csa, Unit::Cm2, 2),
        "CSA = π × (d/2)²",
        format!("π × ({} / 2)²", v(d, 2, Unit::Cm)),
    );
    let mva = continuity_area_cm2(csa, lvti, mvti);
    out.push(
        q(
            "mva_cm2",
            tx!("Mitral valve area", "Area valvolare mitralica"),
            mva,
            Unit::Cm2,
            2,
        ),
        "MVA = CSA × VTI(LVOT) ÷ VTI(MV)",
        format!(
            "{} × {} ÷ {}",
            v(csa, 2, Unit::Cm2),
            v(lvti, 1, Unit::Cm),
            v(mvti, 0, Unit::Cm)
        ),
    );
    out.findings.push(MVA_SCALE.finding(mva));
    Ok(out)
}

/// Mitral valve area by PISA with the funnel-angle correction (formula only: the printed formula
/// of the source differs, see the research note).
pub static MITRAL_AREA_PISA: Calculator = Calculator {
    id: "mitral-area-pisa",
    category: Category::Stenosis,
    name: tx!("Mitral valve area (PISA)", "Area valvolare mitralica (PISA)"),
    synonyms: tx!(
        "MVA, mitral stenosis, PISA, flow convergence, proximal isovelocity, aliasing velocity, angle correction",
        "MVA, stenosi mitralica, PISA, convergenza di flusso, isovelocità prossimale, velocità di aliasing, correzione angolare"
    ),
    summary: tx!(
        "Diastolic flow converging on the atrial side of the funnel: a hemisphere of radius r at the aliasing velocity carries 2πr² × Va, dividing by the peak mitral velocity gives the orifice, and the funnel angle α corrects it by α/180°.",
        "Flusso diastolico che converge sul versante atriale dell'imbuto: una semisfera di raggio r alla velocità di aliasing porta 2πr² × Va; dividendo per la velocità mitralica di picco si ottiene l'orifizio, corretto per l'angolo dell'imbuto con α/180°."
    ),
    inputs: &[
        num(
            "pisa_r_cm",
            tx!("PISA radius", "Raggio PISA"),
            Unit::Cm,
            0.3,
            2.5,
            2,
        ),
        num(
            "alias_cm_s",
            tx!("Aliasing velocity", "Velocità di aliasing"),
            Unit::CmS,
            10.0,
            100.0,
            0,
        ),
        num(
            "mv_vmax_m_s",
            tx!(
                "Peak mitral inflow velocity",
                "Velocità di picco dell'afflusso mitralico"
            ),
            Unit::MS,
            0.8,
            4.0,
            2,
        ),
        num(
            "angle_deg",
            tx!("Funnel angle α", "Angolo dell'imbuto α"),
            Unit::Deg,
            30.0,
            180.0,
            0,
        ),
    ],
    compute: mitral_area_pisa,
    references: &[cite(&VS_2009, "§B.1.5 PISA method, p. 14")],
    review: Review::FormulaOnly,
    example: &[
        ("pisa_r_cm", N(1.0)),
        ("alias_cm_s", N(35.0)),
        ("mv_vmax_m_s", N(1.8)),
        ("angle_deg", N(120.0)),
    ],
    sim_link: None,
};

fn mitral_area_pisa(i: &Inputs) -> Result<Outcome, Vec<InputError>> {
    let (r, va, vmax, a) = (
        i.n("pisa_r_cm"),
        i.n("alias_cm_s"),
        i.n("mv_vmax_m_s"),
        i.n("angle_deg"),
    );
    let mut out = outcome();
    let flow = pisa_flow_ml_s(r, va);
    out.push(
        q(
            "flow_ml_s",
            tx!("Flow through the hemisphere", "Flusso attraverso la semisfera"),
            flow,
            Unit::MlS,
            0,
        ),
        "Q = 2π × r² × Va",
        format!("2π × {}² × {}", n(r, 2), v(va, 0, Unit::CmS)),
    );
    let mva = pisa_angle_corrected(flow / (vmax * 100.0), a);
    out.push(
        q(
            "mva_cm2",
            tx!("Mitral valve area", "Area valvolare mitralica"),
            mva,
            Unit::Cm2,
            2,
        ),
        "MVA = Q ÷ Vmax × α/180°",
        format!("{} ÷ {} cm/s × {}/180", n(flow, 0), n(vmax * 100.0, 0), n(a, 0)),
    );
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// Pulmonic and tricuspid stenosis

/// Table 11 of the 2009 recommendations (quoting the 2006 ACC/AHA definitions): mild < 3 m/s,
/// moderate 3–4 m/s, severe > 4 m/s.
static PS_SCALE: Scale = Scale {
    what: tx!("Peak pulmonic velocity", "Velocità polmonare di picco"),
    symbol: "Vmax",
    unit: Unit::MS,
    decimals: 1,
    bands: &[
        band(3.0, false, Severity::Mild, tx!("mild", "lieve")),
        band(4.0, true, Severity::Moderate, MODERATE),
    ],
    above: (Severity::Severe, SEVERE),
    source: cite(&VS_2009, "Table 11 and text, p. 20"),
};

/// Peak pulmonic gradient from the peak jet velocity, graded by velocity.
pub static PULMONIC_STENOSIS: Calculator = Calculator {
    id: "pulmonic-stenosis-gradient",
    category: Category::Stenosis,
    name: tx!(
        "Pulmonic stenosis peak gradient",
        "Gradiente di picco della stenosi polmonare"
    ),
    synonyms: tx!(
        "PS, pulmonary valve, Bernoulli, peak gradient, RVOT",
        "SP, valvola polmonare, Bernoulli, gradiente di picco, RVOT"
    ),
    summary: tx!(
        "Simplified Bernoulli on the peak continuous-wave velocity across the pulmonary valve. The proximal velocity is usually below 1 m/s and is neglected; with stenoses in series the gradients are not separable.",
        "Bernoulli semplificata sulla velocità di picco al Doppler continuo attraverso la valvola polmonare. La velocità prossimale è di norma sotto 1 m/s e si trascura; con stenosi in serie i gradienti non si separano."
    ),
    inputs: &[num(
        "pv_vmax_m_s",
        tx!(
            "Peak pulmonic jet velocity",
            "Velocità di picco del getto polmonare"
        ),
        Unit::MS,
        0.5,
        7.0,
        2,
    )],
    compute: pulmonic_stenosis,
    references: &[cite(&VS_2009, "§V.B.1.1 and Table 11, pp. 19–20")],
    review: Review::Verified,
    example: &[("pv_vmax_m_s", N(3.4))],
    sim_link: None,
};

fn pulmonic_stenosis(i: &Inputs) -> Result<Outcome, Vec<InputError>> {
    let vmax = i.n("pv_vmax_m_s");
    let mut out = outcome();
    out.push(
        q(
            "peak_gradient_mmhg",
            tx!("Peak pulmonic gradient", "Gradiente polmonare di picco"),
            bernoulli_mmhg(vmax),
            Unit::MmHg,
            0,
        ),
        "ΔPmax = 4 × Vmax²",
        format!("4 × {}²", n(vmax, 2)),
    );
    out.findings.push(PS_SCALE.finding(vmax));
    Ok(out)
}

const TS_CITE: Cite = cite(&RHD_2023, "Rheumatic tricuspid stenosis, p. 16");
static TVA_SCALE: Scale = Scale {
    what: tx!("Tricuspid valve area", "Area valvolare tricuspidale"),
    symbol: "TVA",
    unit: Unit::Cm2,
    decimals: 1,
    bands: &[
        band(1.0, true, Severity::Severe, SEVERE),
        band(1.5, true, Severity::Moderate, MODERATE),
    ],
    above: (Severity::Mild, tx!("mild", "lieve")),
    source: TS_CITE,
};

/// The four haemodynamic criteria of significant tricuspid stenosis, each checked if measured.
pub static TRICUSPID_STENOSIS: Calculator = Calculator {
    id: "tricuspid-stenosis-criteria",
    category: Category::Stenosis,
    name: tx!("Tricuspid stenosis criteria", "Criteri di stenosi tricuspidale"),
    synonyms: tx!(
        "TS, tricuspid valve area, TVA, mean gradient, pressure half-time, inflow VTI, rheumatic",
        "ST, area valvolare tricuspidale, TVA, gradiente medio, tempo di dimezzamento, VTI di afflusso, reumatica"
    ),
    summary: tx!(
        "Checks the measured tricuspid inflow values against the thresholds the rheumatic heart disease recommendation gives for significant stenosis. Average five or more beats: the inflow varies with respiration. An area from 190 ÷ PHT is shown but is less reliable than in the mitral valve.",
        "Confronta i valori misurati dell'afflusso tricuspidale con le soglie di stenosi significativa della raccomandazione sulla cardiopatia reumatica. Mediare cinque o più battiti: l'afflusso varia con il respiro. L'area da 190 ÷ PHT è mostrata ma è meno affidabile che per la mitrale."
    ),
    inputs: &[
        opt(
            "mean_gradient_mmhg",
            tx!("Mean tricuspid gradient", "Gradiente tricuspidale medio"),
            Unit::MmHg,
            0.5,
            30.0,
            1,
        ),
        opt(
            "pht_ms",
            tx!("Pressure half-time", "Tempo di dimezzamento della pressione"),
            Unit::Ms,
            30.0,
            600.0,
            0,
        ),
        opt(
            "inflow_vti_cm",
            tx!("Tricuspid inflow VTI", "VTI dell'afflusso tricuspidale"),
            Unit::Cm,
            5.0,
            150.0,
            0,
        ),
        opt(
            "tva_cm2",
            tx!(
                "Tricuspid valve area (measured)",
                "Area valvolare tricuspidale (misurata)"
            ),
            Unit::Cm2,
            0.3,
            8.0,
            2,
        ),
    ],
    compute: tricuspid_stenosis,
    references: &[TS_CITE],
    review: Review::Verified,
    example: &[
        ("mean_gradient_mmhg", N(6.0)),
        ("pht_ms", N(200.0)),
        ("inflow_vti_cm", N(64.0)),
        ("tva_cm2", N(0.9)),
    ],
    sim_link: None,
};

fn tricuspid_stenosis(i: &Inputs) -> Result<Outcome, Vec<InputError>> {
    let (mg, pht, vti, tva) = (
        o(i, "mean_gradient_mmhg"),
        o(i, "pht_ms"),
        o(i, "inflow_vti_cm"),
        o(i, "tva_cm2"),
    );
    if mg.is_none() && pht.is_none() && vti.is_none() && tva.is_none() {
        return Err(need_one("mean_gradient_mmhg"));
    }
    let mut out = outcome();
    let mut met = 0_u32;
    let mut counted = 0_u32;
    let mut criterion = |hit: bool,
                         text_met: crate::model::Text,
                         text_not: crate::model::Text,
                         crit: &str,
                         out: &mut Outcome| {
        counted += 1;
        if hit {
            met += 1;
            out.findings
                .push(criterion_met(Severity::Severe, text_met, crit, TS_CITE));
        } else {
            out.findings
                .push(criterion_met(Severity::Info, text_not, crit, TS_CITE));
        }
    };
    if let Some(x) = mg {
        criterion(
            ge(x, 5.0),
            tx!(
                "Mean gradient meets the criterion for significant stenosis",
                "Il gradiente medio soddisfa il criterio di stenosi significativa"
            ),
            tx!(
                "Mean gradient below the criterion for significant stenosis",
                "Gradiente medio sotto il criterio di stenosi significativa"
            ),
            "ΔPmean ≥ 5 mmHg",
            &mut out,
        );
    }
    if let Some(x) = vti {
        criterion(
            ge(x, 60.0),
            tx!(
                "Inflow VTI meets the criterion for significant stenosis",
                "La VTI di afflusso soddisfa il criterio di stenosi significativa"
            ),
            tx!(
                "Inflow VTI below the criterion for significant stenosis",
                "VTI di afflusso sotto il criterio di stenosi significativa"
            ),
            "VTI ≥ 60 cm",
            &mut out,
        );
    }
    if let Some(x) = pht {
        criterion(
            ge(x, 190.0),
            tx!(
                "Pressure half-time meets the criterion for significant stenosis",
                "Il tempo di dimezzamento soddisfa il criterio di stenosi significativa"
            ),
            tx!(
                "Pressure half-time below the criterion for significant stenosis",
                "Tempo di dimezzamento sotto il criterio di stenosi significativa"
            ),
            "PHT ≥ 190 ms",
            &mut out,
        );
    }
    if let Some(x) = tva {
        criterion(
            le(x, 1.0),
            tx!(
                "Valve area meets the criterion for significant stenosis",
                "L'area valvolare soddisfa il criterio di stenosi significativa"
            ),
            tx!(
                "Valve area above the criterion for significant stenosis",
                "Area valvolare sopra il criterio di stenosi significativa"
            ),
            "TVA ≤ 1.0 cm²",
            &mut out,
        );
    }
    out.push(
        q(
            "criteria_met",
            tx!("Criteria met", "Criteri soddisfatti"),
            f64::from(met),
            Unit::Score,
            0,
        ),
        "criteria met, of those measured",
        format!("{met} of {counted}"),
    );
    if let Some(p) = pht {
        out.push(
            q(
                "tva_pht_cm2",
                tx!("Area from PHT (less reliable)", "Area dal PHT (meno affidabile)"),
                190.0 / p,
                Unit::Cm2,
                2,
            ),
            "TVA = 190 ÷ PHT",
            format!("190 ÷ {}", n(p, 0)),
        );
    }
    if let Some(a) = tva {
        out.findings.push(TVA_SCALE.finding(a));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::super::testing::{close, inconsistent, ok, out_of_range, severities};
    use super::*;
    use crate::model::Inputs;

    #[test]
    fn aortic_valve_area_hand_case() {
        // d 2.0 cm → CSA π = 3.1416 cm²; × 20 ÷ 80 = 0.785 cm²; VTI ratio 0.25; AVAi 0.785 ÷ 1.8 = 0.436
        let i = Inputs::new()
            .with("lvot_d_cm", 2.0)
            .with("lvot_vti_cm", 20.0)
            .with("av_vti_cm", 80.0)
            .with("bsa_m2", 1.8)
            .with("lvot_vmax_m_s", 1.0)
            .with("av_vmax_m_s", 4.0)
            .with("av_mean_mmhg", 40.0);
        let o = ok(&AORTIC_VALVE_AREA, &i);
        close(o.value("ava_cm2"), std::f64::consts::FRAC_PI_4, 1e-9);
        close(o.value("vti_ratio"), 0.25, 1e-9);
        close(o.value("avai_cm2_m2"), 0.4363, 1e-3);
        // 4 × 4² = 64 mmHg; VR 1 ÷ 4 = 0.25 (moderate: Table 3 puts < 0.25 in severe)
        close(o.value("av_peak_gradient_mmhg"), 64.0, 1e-9);
        close(o.value("velocity_ratio"), 0.25, 1e-9);
        // AVA, AVAi severe; Vmax 4.0 and mean 40 severe (≥); VR 0.25 moderate
        assert_eq!(
            severities(&o),
            [
                Severity::Severe,
                Severity::Severe,
                Severity::Severe,
                Severity::Moderate,
                Severity::Severe
            ]
        );
        assert!(
            o.steps
                .iter()
                .any(|s| s.substituted.contains("3.14 cm² × 20.0 cm ÷ 80 cm"))
        );
    }

    #[test]
    fn aortic_grading_boundaries() {
        // AVA: 1.0 is moderate (< 1.0 severe), 1.5 moderate, above mild
        assert_eq!(AVA_SCALE.class(0.99).0, Severity::Severe);
        assert_eq!(AVA_SCALE.class(1.0).0, Severity::Moderate);
        assert_eq!(AVA_SCALE.class(1.5).0, Severity::Moderate);
        assert_eq!(AVA_SCALE.class(1.51).0, Severity::Mild);
        assert_eq!(AVAI_SCALE.class(0.59).0, Severity::Severe);
        assert_eq!(AVAI_SCALE.class(0.60).0, Severity::Moderate);
        assert_eq!(AVAI_SCALE.class(0.85).0, Severity::Moderate);
        assert_eq!(AVAI_SCALE.class(0.86).0, Severity::Mild);
        assert_eq!(VR_SCALE.class(0.24).0, Severity::Severe);
        assert_eq!(VR_SCALE.class(0.50).0, Severity::Moderate);
        assert_eq!(VR_SCALE.class(0.51).0, Severity::Mild);
        assert_eq!(AS_VMAX_SCALE.class(2.5).0, Severity::Normal);
        assert_eq!(AS_VMAX_SCALE.class(2.6).0, Severity::Mild);
        assert_eq!(AS_VMAX_SCALE.class(2.9).0, Severity::Mild);
        assert_eq!(AS_VMAX_SCALE.class(3.0).0, Severity::Moderate);
        assert_eq!(AS_VMAX_SCALE.class(3.99).0, Severity::Moderate);
        assert_eq!(AS_VMAX_SCALE.class(4.0).0, Severity::Severe);
        assert_eq!(AS_MEAN_SCALE.class(19.9).0, Severity::Mild);
        assert_eq!(AS_MEAN_SCALE.class(20.0).0, Severity::Moderate);
        assert_eq!(AS_MEAN_SCALE.class(39.9).0, Severity::Moderate);
        assert_eq!(AS_MEAN_SCALE.class(40.0).0, Severity::Severe);
        assert_eq!(AVA_SCALE.criterion(1), "1.0 ≤ AVA ≤ 1.5 cm²");
        assert_eq!(AVA_SCALE.criterion(0), "AVA < 1.0 cm²");
        assert_eq!(AVA_SCALE.criterion(2), "AVA > 1.5 cm²");
    }

    #[test]
    fn aortic_errors() {
        let base = AORTIC_VALVE_AREA.example_inputs();
        out_of_range(
            &AORTIC_VALVE_AREA,
            &base.clone().with("lvot_d_cm", 6.0),
            "lvot_d_cm",
        );
        inconsistent(
            &AORTIC_VALVE_AREA,
            &base.with("lvot_vmax_m_s", 2.4).with("av_vmax_m_s", 2.0),
            "lvot_vmax_m_s",
        );
    }

    #[test]
    fn mitral_pht_hand_case_and_boundaries() {
        // 220 ÷ 220 = 1.0 cm² (severe, ≤ 1.5); PHT 220 ≥ 150 severe; mean 9 moderate (5–9)
        let o = ok(
            &MITRAL_AREA_PHT,
            &Inputs::new()
                .with("pht_ms", 220.0)
                .with("mean_gradient_mmhg", 9.0),
        );
        close(o.value("mva_cm2"), 1.0, 1e-12);
        assert_eq!(
            severities(&o),
            [Severity::Severe, Severity::Severe, Severity::Moderate]
        );
        assert_eq!(MVA_SCALE.class(1.5).0, Severity::Severe);
        assert_eq!(MVA_SCALE.class(1.51).0, Severity::Moderate);
        assert_eq!(MVA_SCALE.class(2.5).0, Severity::Moderate);
        assert_eq!(MVA_SCALE.class(2.51).0, Severity::Mild);
        assert_eq!(PHT_SCALE.class(99.0).0, Severity::Mild);
        assert_eq!(PHT_SCALE.class(100.0).0, Severity::Moderate);
        assert_eq!(PHT_SCALE.class(149.0).0, Severity::Moderate);
        assert_eq!(PHT_SCALE.class(150.0).0, Severity::Severe);
        assert_eq!(MS_MEAN_SCALE.class(4.9).0, Severity::Mild);
        assert_eq!(MS_MEAN_SCALE.class(5.0).0, Severity::Moderate);
        assert_eq!(MS_MEAN_SCALE.class(10.0).0, Severity::Severe);
        out_of_range(&MITRAL_AREA_PHT, &Inputs::new().with("pht_ms", 10.0), "pht_ms");
    }

    #[test]
    fn mitral_continuity_and_pisa_hand_cases() {
        // π × 18 ÷ 56.55 = 1.0 cm²
        let o = ok(
            &MITRAL_AREA_CONTINUITY,
            &Inputs::new()
                .with("lvot_d_cm", 2.0)
                .with("lvot_vti_cm", 18.0)
                .with("mv_vti_cm", 18.0 * std::f64::consts::PI),
        );
        close(o.value("mva_cm2"), 1.0, 1e-9);
        // 2π × 1² × 30 = 188.5 mL/s; ÷ 150 cm/s = 1.257; × 120/180 = 0.838 cm²
        let o = ok(
            &MITRAL_AREA_PISA,
            &Inputs::new()
                .with("pisa_r_cm", 1.0)
                .with("alias_cm_s", 30.0)
                .with("mv_vmax_m_s", 1.5)
                .with("angle_deg", 120.0),
        );
        close(o.value("flow_ml_s"), 188.5, 0.1);
        close(o.value("mva_cm2"), 0.8378, 1e-3);
        assert_eq!(o.findings.len(), 0, "formula only");
    }

    #[test]
    fn pulmonic_hand_case_and_boundaries() {
        // 4 × 3.5² = 49 mmHg, moderate
        let o = ok(&PULMONIC_STENOSIS, &Inputs::new().with("pv_vmax_m_s", 3.5));
        close(o.value("peak_gradient_mmhg"), 49.0, 1e-9);
        assert_eq!(severities(&o), [Severity::Moderate]);
        assert_eq!(PS_SCALE.class(2.99).0, Severity::Mild);
        assert_eq!(PS_SCALE.class(3.0).0, Severity::Moderate);
        assert_eq!(PS_SCALE.class(4.0).0, Severity::Moderate);
        assert_eq!(PS_SCALE.class(4.01).0, Severity::Severe);
    }

    #[test]
    fn tricuspid_criteria() {
        // 190 ms exactly meets; 190 ÷ 190 = 1.0 cm²
        let o = ok(
            &TRICUSPID_STENOSIS,
            &Inputs::new()
                .with("mean_gradient_mmhg", 4.9)
                .with("pht_ms", 190.0)
                .with("inflow_vti_cm", 60.0)
                .with("tva_cm2", 1.0),
        );
        close(o.value("criteria_met"), 3.0, 0.0);
        close(o.value("tva_pht_cm2"), 1.0, 1e-12);
        assert_eq!(
            severities(&o),
            [
                Severity::Info,
                Severity::Severe,
                Severity::Severe,
                Severity::Severe,
                Severity::Severe
            ]
        );
        assert_eq!(TVA_SCALE.class(1.0).0, Severity::Severe);
        assert_eq!(TVA_SCALE.class(1.01).0, Severity::Moderate);
        assert_eq!(TVA_SCALE.class(1.5).0, Severity::Moderate);
        assert_eq!(TVA_SCALE.class(1.51).0, Severity::Mild);
        assert!(TRICUSPID_STENOSIS.evaluate(&Inputs::new()).is_err());
    }
}
