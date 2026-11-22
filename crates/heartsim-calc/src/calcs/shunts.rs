//! Shunts and congenital lesions: Qp/Qs, shunt flow, RV pressure from a VSD jet, PA pressure from a
//! PDA jet, the coarctation gradient with the proximal velocity, and the mean gradient of a traced
//! velocity curve.

use super::{check, cite, n, num, o, opt, outcome, q, v};
use crate::blocks::{bernoulli_mmhg, bernoulli_proximal_mmhg, stroke_volume_ml};
use crate::model::ExampleValue::Number as N;
use crate::model::{Calculator, Category, InputError, InputSpec, Inputs, Outcome, Review, SimLink, Unit};
use crate::refs::{AS_2017, ASD_2015, GORCSAN_1991};
use crate::tx;

const RVOT_D: InputSpec = num(
    "rvot_d_cm",
    tx!(
        "RVOT diameter at the pulsed sample",
        "Diametro RVOT al campione pulsato"
    ),
    Unit::Cm,
    1.0,
    4.5,
    2,
);
const RVOT_VTI: InputSpec = num("rvot_vti_cm", tx!("RVOT VTI", "VTI RVOT"), Unit::Cm, 4.0, 60.0, 1);
const LVOT_D: InputSpec = num(
    "lvot_d_cm",
    tx!(
        "LVOT diameter at the pulsed sample",
        "Diametro LVOT al campione pulsato"
    ),
    Unit::Cm,
    1.0,
    4.0,
    2,
);
const LVOT_VTI: InputSpec = num("lvot_vti_cm", tx!("LVOT VTI", "VTI LVOT"), Unit::Cm, 3.0, 60.0, 1);
const SBP: InputSpec = num(
    "sbp_mmhg",
    tx!("Systolic blood pressure", "Pressione arteriosa sistolica"),
    Unit::MmHg,
    60.0,
    250.0,
    0,
);

fn flows(i: &Inputs, out: &mut Outcome) -> (f64, f64) {
    let (rd, rv, ld, lv) = (
        i.n("rvot_d_cm"),
        i.n("rvot_vti_cm"),
        i.n("lvot_d_cm"),
        i.n("lvot_vti_cm"),
    );
    let qp = stroke_volume_ml(rd, rv);
    let qs = stroke_volume_ml(ld, lv);
    out.push(
        q(
            "qp_ml",
            tx!("Pulmonary stroke volume (Qp)", "Volume sistolico polmonare (Qp)"),
            qp,
            Unit::Ml,
            0,
        ),
        "Qp = π × (d/2)² × VTI(RVOT)",
        format!("π × ({} / 2)² × {}", v(rd, 2, Unit::Cm), v(rv, 1, Unit::Cm)),
    );
    out.push(
        q(
            "qs_ml",
            tx!("Systemic stroke volume (Qs)", "Volume sistolico sistemico (Qs)"),
            qs,
            Unit::Ml,
            0,
        ),
        "Qs = π × (d/2)² × VTI(LVOT)",
        format!("π × ({} / 2)² × {}", v(ld, 2, Unit::Cm), v(lv, 1, Unit::Cm)),
    );
    out.push(
        q("qp_qs", tx!("Qp/Qs", "Qp/Qs"), qp / qs, Unit::None, 2),
        "Qp/Qs = Qp ÷ Qs",
        format!("{} ÷ {}", n(qp, 0), n(qs, 0)),
    );
    (qp, qs)
}

/// Pulmonary-to-systemic flow ratio from the two outflow tracts.
pub static QP_QS: Calculator = Calculator {
    id: "pulmonary-systemic-flow-ratio",
    category: Category::Shunts,
    name: tx!(
        "Pulmonary-to-systemic flow ratio (Qp/Qs)",
        "Rapporto tra portata polmonare e sistemica (Qp/Qs)"
    ),
    synonyms: tx!(
        "Qp/Qs, Qp:Qs, shunt ratio, ASD, VSD, PDA, left-to-right shunt",
        "Qp/Qs, rapporto di shunt, DIA, DIV, PDA, shunt sinistro-destro"
    ),
    summary: tx!(
        "RVOT stroke volume divided by LVOT stroke volume, each from its maximal systolic diameter and the pulsed VTI at the same site. Both diameters are squared, so small errors change the ratio a lot.",
        "Volume sistolico nel RVOT diviso per quello nel LVOT, ciascuno dal diametro sistolico massimo e dalla VTI pulsata nella stessa sede. Entrambi i diametri sono al quadrato: piccoli errori cambiano molto il rapporto."
    ),
    inputs: &[RVOT_D, RVOT_VTI, LVOT_D, LVOT_VTI],
    compute: |i| {
        let mut out = outcome();
        flows(i, &mut out);
        Ok(out)
    },
    references: &[cite(&ASD_2015, "Pulsed Doppler Qp/Qs, p. 929")],
    review: Review::FormulaOnly,
    example: &[
        ("rvot_d_cm", N(2.6)),
        ("rvot_vti_cm", N(24.0)),
        ("lvot_d_cm", N(2.0)),
        ("lvot_vti_cm", N(20.0)),
    ],
    sim_link: None,
};

/// Shunt volume per beat and per minute, and the shunt fraction of pulmonary flow.
pub static SHUNT_FLOW: Calculator = Calculator {
    id: "shunt-flow",
    category: Category::Shunts,
    name: tx!("Left-to-right shunt flow", "Portata dello shunt sinistro-destro"),
    synonyms: tx!(
        "shunt volume, shunt fraction, ASD flow, Qp − Qs, L/min, atrial septal defect",
        "volume di shunt, frazione di shunt, flusso DIA, Qp − Qs, L/min, difetto interatriale"
    ),
    summary: tx!(
        "The extra volume the right heart pumps with a pre-tricuspid or ventricular left-to-right shunt: Qp − Qs per beat, times heart rate per minute, and as a fraction of the pulmonary flow.",
        "Il volume in più pompato dal cuore destro con uno shunt sinistro-destro pre-tricuspidale o ventricolare: Qp − Qs per battito, per la frequenza al minuto, e come frazione della portata polmonare."
    ),
    inputs: &[
        RVOT_D,
        RVOT_VTI,
        LVOT_D,
        LVOT_VTI,
        opt(
            "hr_bpm",
            tx!("Heart rate", "Frequenza cardiaca"),
            Unit::Bpm,
            25.0,
            220.0,
            0,
        ),
    ],
    compute: |i| {
        let mut out = outcome();
        let (qp, qs) = flows(i, &mut out);
        check(
            qp <= qs,
            "rvot_vti_cm",
            tx!(
                "Pulmonary flow does not exceed systemic flow: no left-to-right shunt to quantify",
                "La portata polmonare non supera quella sistemica: nessuno shunt sinistro-destro da quantificare"
            ),
        )?;
        let shunt = qp - qs;
        out.push(
            q(
                "shunt_ml",
                tx!("Shunt volume per beat", "Volume di shunt per battito"),
                shunt,
                Unit::Ml,
                0,
            ),
            "Qp − Qs",
            format!("{} − {}", n(qp, 0), n(qs, 0)),
        );
        out.push(
            q(
                "shunt_fraction_percent",
                tx!("Shunt fraction of Qp", "Frazione di shunt su Qp"),
                shunt / qp * 100.0,
                Unit::Percent,
                0,
            ),
            "(Qp − Qs) ÷ Qp × 100",
            format!("{} ÷ {} × 100", n(shunt, 0), n(qp, 0)),
        );
        if let Some(hr) = o(i, "hr_bpm") {
            out.push(
                q(
                    "shunt_l_min",
                    tx!("Shunt flow", "Portata dello shunt"),
                    shunt * hr / 1000.0,
                    Unit::LMin,
                    2,
                ),
                "(Qp − Qs) × HR ÷ 1000",
                format!("{} × {} ÷ 1000", n(shunt, 0), v(hr, 0, Unit::Bpm)),
            );
        }
        Ok(out)
    },
    references: &[cite(&ASD_2015, "Pulsed Doppler Qp/Qs, p. 929")],
    review: Review::FormulaOnly,
    example: &[
        ("rvot_d_cm", N(2.7)),
        ("rvot_vti_cm", N(22.0)),
        ("lvot_d_cm", N(2.0)),
        ("lvot_vti_cm", N(19.0)),
        ("hr_bpm", N(75.0)),
    ],
    sim_link: Some(SimLink::HeartRate("hr_bpm")),
};

/// RV systolic pressure as systolic blood pressure minus the VSD jet gradient.
pub static VSD_RV_PRESSURE: Calculator = Calculator {
    id: "vsd-rv-pressure",
    category: Category::Shunts,
    name: tx!(
        "RV systolic pressure from a VSD jet",
        "Pressione sistolica del VD dal getto del DIV"
    ),
    synonyms: tx!(
        "VSD, ventricular septal defect, restrictive VSD, LV–RV gradient, RVSP",
        "DIV, difetto interventricolare, DIV restrittivo, gradiente VS–VD, PSVD"
    ),
    summary: tx!(
        "Without LV outflow obstruction the systolic blood pressure stands in for LV systolic pressure; the peak left-to-right VSD gradient (4V²) subtracted from it leaves the RV systolic pressure. A high velocity means a restrictive defect.",
        "In assenza di ostruzione all'efflusso del VS la pressione sistolica sostituisce quella del VS; sottraendo il gradiente sinistro-destro di picco del DIV (4V²) resta la pressione sistolica del VD. Una velocità alta indica un difetto restrittivo."
    ),
    inputs: &[
        SBP,
        num(
            "vsd_vmax_m_s",
            tx!("Peak VSD jet velocity", "Velocità di picco del getto del DIV"),
            Unit::MS,
            0.5,
            6.5,
            2,
        ),
    ],
    compute: |i| {
        let (sbp, vel) = (i.n("sbp_mmhg"), i.n("vsd_vmax_m_s"));
        let g = bernoulli_mmhg(vel);
        check(
            g >= sbp,
            "vsd_vmax_m_s",
            tx!(
                "The VSD gradient exceeds the systolic pressure",
                "Il gradiente del DIV supera la pressione sistolica"
            ),
        )?;
        let mut out = outcome();
        out.push(
            q(
                "lv_rv_gradient_mmhg",
                tx!("Peak LV–RV gradient", "Gradiente VS–VD di picco"),
                g,
                Unit::MmHg,
                0,
            ),
            "ΔP = 4 × V²",
            format!("4 × {}²", n(vel, 2)),
        );
        out.push(
            q(
                "rvsp_mmhg",
                tx!("RV systolic pressure", "Pressione sistolica del VD"),
                sbp - g,
                Unit::MmHg,
                0,
            ),
            "RVSP = SBP − ΔP",
            format!("{} − {}", n(sbp, 0), n(g, 0)),
        );
        Ok(out)
    },
    references: &[
        cite(&AS_2017, "Simplified Bernoulli, p. 378"),
        cite(&GORCSAN_1991, "SBP as LV systolic pressure (abstract)"),
    ],
    review: Review::FormulaOnly,
    example: &[("sbp_mmhg", N(118.0)), ("vsd_vmax_m_s", N(4.8))],
    sim_link: None,
};

/// PA systolic pressure as systolic blood pressure minus the PDA jet gradient.
pub static PDA_PA_PRESSURE: Calculator = Calculator {
    id: "pda-pa-pressure",
    category: Category::Shunts,
    name: tx!(
        "PA systolic pressure from a PDA jet",
        "Pressione sistolica polmonare dal getto del dotto"
    ),
    synonyms: tx!(
        "PDA, patent ductus arteriosus, ductal flow, aorta–pulmonary gradient, PASP",
        "PDA, dotto arterioso pervio, flusso duttale, gradiente aorta–polmonare, PAPs"
    ),
    summary: tx!(
        "With left-to-right ductal flow, the peak systolic aorto-pulmonary gradient (4V²) subtracted from the systolic blood pressure estimates PA systolic pressure. Not applicable to bidirectional or right-to-left flow.",
        "Con flusso duttale sinistro-destro, il gradiente sistolico aorto-polmonare di picco (4V²) sottratto alla pressione sistolica stima la pressione sistolica polmonare. Non applicabile a flusso bidirezionale o destro-sinistro."
    ),
    inputs: &[
        SBP,
        num(
            "pda_vmax_m_s",
            tx!(
                "Peak systolic PDA velocity",
                "Velocità sistolica di picco nel dotto"
            ),
            Unit::MS,
            0.5,
            6.5,
            2,
        ),
    ],
    compute: |i| {
        let (sbp, vel) = (i.n("sbp_mmhg"), i.n("pda_vmax_m_s"));
        let g = bernoulli_mmhg(vel);
        check(
            g >= sbp,
            "pda_vmax_m_s",
            tx!(
                "The ductal gradient exceeds the systolic pressure",
                "Il gradiente duttale supera la pressione sistolica"
            ),
        )?;
        let mut out = outcome();
        out.push(
            q(
                "ao_pa_gradient_mmhg",
                tx!("Peak aorta–PA gradient", "Gradiente aorta–AP di picco"),
                g,
                Unit::MmHg,
                0,
            ),
            "ΔP = 4 × V²",
            format!("4 × {}²", n(vel, 2)),
        );
        out.push(
            q(
                "pasp_mmhg",
                tx!("PA systolic pressure", "Pressione sistolica polmonare"),
                sbp - g,
                Unit::MmHg,
                0,
            ),
            "PASP = SBP − ΔP",
            format!("{} − {}", n(sbp, 0), n(g, 0)),
        );
        Ok(out)
    },
    references: &[cite(&AS_2017, "Simplified Bernoulli, p. 378")],
    review: Review::FormulaOnly,
    example: &[("sbp_mmhg", N(112.0)), ("pda_vmax_m_s", N(4.5))],
    sim_link: None,
};

/// Coarctation peak gradient, simplified and with the proximal velocity kept.
pub static COARCTATION_GRADIENT: Calculator = Calculator {
    id: "coarctation-gradient",
    category: Category::Shunts,
    name: tx!(
        "Coarctation peak gradient",
        "Gradiente di picco della coartazione"
    ),
    synonyms: tx!(
        "coarctation of the aorta, CoA, descending aorta, proximal velocity, modified Bernoulli",
        "coartazione aortica, aorta discendente, velocità prossimale, Bernoulli modificata"
    ),
    summary: tx!(
        "Peak gradient across the narrowing from the continuous-wave velocity V₂; the proximal velocity V₁ in the arch is often above 1 m/s and is then kept: ΔP = 4(V₂² − V₁²). A diastolic tail on the CW trace is part of the assessment.",
        "Gradiente di picco attraverso il restringimento dalla velocità al Doppler continuo V₂; la velocità prossimale V₁ nell'arco supera spesso 1 m/s e si conserva: ΔP = 4(V₂² − V₁²). Una coda diastolica alla traccia CW fa parte della valutazione."
    ),
    inputs: &[
        num(
            "v2_m_s",
            tx!(
                "Peak velocity across the coarctation (V₂)",
                "Velocità di picco attraverso la coartazione (V₂)"
            ),
            Unit::MS,
            1.0,
            6.5,
            2,
        ),
        opt(
            "v1_m_s",
            tx!("Proximal velocity (V₁)", "Velocità prossimale (V₁)"),
            Unit::MS,
            0.3,
            3.0,
            2,
        ),
    ],
    compute: |i| {
        let v2 = i.n("v2_m_s");
        let mut out = outcome();
        out.push(
            q(
                "peak_gradient_mmhg",
                tx!("Peak gradient (simplified)", "Gradiente di picco (semplificato)"),
                bernoulli_mmhg(v2),
                Unit::MmHg,
                0,
            ),
            "ΔP = 4 × V₂²",
            format!("4 × {}²", n(v2, 2)),
        );
        if let Some(v1) = o(i, "v1_m_s") {
            check(
                v1 >= v2,
                "v1_m_s",
                tx!(
                    "The proximal velocity must be lower than the jet velocity",
                    "La velocità prossimale deve essere inferiore a quella del getto"
                ),
            )?;
            out.push(
                q(
                    "corrected_gradient_mmhg",
                    tx!(
                        "Peak gradient (proximal velocity kept)",
                        "Gradiente di picco (con velocità prossimale)"
                    ),
                    bernoulli_proximal_mmhg(v2, v1),
                    Unit::MmHg,
                    0,
                ),
                "ΔP = 4 × (V₂² − V₁²)",
                format!("4 × ({}² − {}²)", n(v2, 2), n(v1, 2)),
            );
        }
        Ok(out)
    },
    references: &[cite(&AS_2017, "Bernoulli with proximal velocity, p. 378")],
    review: Review::FormulaOnly,
    example: &[("v2_m_s", N(3.6)), ("v1_m_s", N(1.4))],
    sim_link: None,
};

const SAMPLE_IDS: [&str; 8] = [
    "v1_m_s", "v2_m_s", "v3_m_s", "v4_m_s", "v5_m_s", "v6_m_s", "v7_m_s", "v8_m_s",
];

macro_rules! sample {
    ($id:expr, $k:expr, $opt:expr) => {
        InputSpec {
            id: $id,
            label: tx!(concat!("Velocity ", $k), concat!("Velocità ", $k)),
            kind: crate::model::InputKind::Number {
                unit: Unit::MS,
                min: 0.0,
                max: 7.0,
                decimals: 2,
            },
            optional: $opt,
            when: None,
        }
    };
}

/// Mean gradient as the average of the instantaneous gradients of a traced curve.
pub static MEAN_GRADIENT_TRACE: Calculator = Calculator {
    id: "mean-gradient-from-trace",
    category: Category::Shunts,
    name: tx!(
        "Mean gradient from a traced velocity",
        "Gradiente medio da una velocità tracciata"
    ),
    synonyms: tx!(
        "mean gradient, instantaneous gradient, velocity trace, Fontan, conduit, baffle",
        "gradiente medio, gradiente istantaneo, traccia di velocità, Fontan, condotto, tunnel"
    ),
    summary: tx!(
        "Enter velocities read at equal time steps along the envelope. The mean gradient is the average of 4v² at each step, not 4 × (mean velocity)²; the page shows both so the difference can be seen.",
        "Inserire velocità lette a intervalli di tempo uguali lungo l'inviluppo. Il gradiente medio è la media di 4v² a ogni intervallo, non 4 × (velocità media)²; la pagina mostra entrambi per vederne la differenza."
    ),
    inputs: &[
        sample!("v1_m_s", "1", false),
        sample!("v2_m_s", "2", false),
        sample!("v3_m_s", "3", true),
        sample!("v4_m_s", "4", true),
        sample!("v5_m_s", "5", true),
        sample!("v6_m_s", "6", true),
        sample!("v7_m_s", "7", true),
        sample!("v8_m_s", "8", true),
    ],
    compute: mean_gradient,
    references: &[cite(
        &AS_2017,
        "Mean gradient from instantaneous gradients, p. 378",
    )],
    review: Review::FormulaOnly,
    example: &[
        ("v1_m_s", N(1.0)),
        ("v2_m_s", N(2.6)),
        ("v3_m_s", N(3.4)),
        ("v4_m_s", N(3.6)),
        ("v5_m_s", N(3.2)),
        ("v6_m_s", N(2.5)),
        ("v7_m_s", N(1.5)),
        ("v8_m_s", N(0.6)),
    ],
    sim_link: None,
};

fn mean_gradient(i: &Inputs) -> Result<Outcome, Vec<InputError>> {
    let vs: Vec<f64> = SAMPLE_IDS.iter().filter_map(|id| o(i, id)).collect();
    let k = vs.len() as f64;
    let mean_g = vs.iter().map(|x| bernoulli_mmhg(*x)).sum::<f64>() / k;
    let mean_v = vs.iter().sum::<f64>() / k;
    let mut out = outcome();
    let terms: Vec<String> = vs.iter().map(|x| format!("4×{}²", n(*x, 1))).collect();
    out.push(
        q(
            "mean_gradient_mmhg",
            tx!("Mean gradient", "Gradiente medio"),
            mean_g,
            Unit::MmHg,
            1,
        ),
        "ΔPmean = Σ 4v² ÷ n",
        format!("({}) ÷ {}", terms.join(" + "), vs.len()),
    );
    out.push(
        q(
            "peak_gradient_mmhg",
            tx!("Peak gradient", "Gradiente di picco"),
            bernoulli_mmhg(vs.iter().copied().fold(0.0, f64::max)),
            Unit::MmHg,
            1,
        ),
        "ΔPmax = 4 × max(v)²",
        format!("4 × {}²", n(vs.iter().copied().fold(0.0, f64::max), 2)),
    );
    out.push(
        q(
            "from_mean_velocity_mmhg",
            tx!(
                "4 × (mean velocity)² (not the mean gradient)",
                "4 × (velocità media)² (non è il gradiente medio)"
            ),
            bernoulli_mmhg(mean_v),
            Unit::MmHg,
            1,
        ),
        "4 × v(mean)²",
        format!("4 × {}²", n(mean_v, 2)),
    );
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::super::testing::{close, inconsistent, ok};
    use super::*;

    #[test]
    fn qp_qs_and_shunt_flow() {
        // Qp π × 1.25² × 20 = 98.17; Qs π × 1² × 20 = 62.83; ratio 1.5625 = (2.5/2.0)²
        let i = Inputs::new()
            .with("rvot_d_cm", 2.5)
            .with("rvot_vti_cm", 20.0)
            .with("lvot_d_cm", 2.0)
            .with("lvot_vti_cm", 20.0)
            .with("hr_bpm", 60.0);
        let o = ok(&QP_QS, &i);
        close(o.value("qp_qs"), 1.5625, 1e-9);
        assert_eq!(o.findings.len(), 0);
        let o = ok(&SHUNT_FLOW, &i);
        close(o.value("shunt_ml"), 35.34, 0.01);
        close(o.value("shunt_fraction_percent"), 36.0, 1e-6);
        close(o.value("shunt_l_min"), 2.120, 1e-3);
        inconsistent(&SHUNT_FLOW, &i.with("rvot_d_cm", 1.8), "rvot_vti_cm");
    }

    #[test]
    fn jets_and_coarctation() {
        // 120 − 4 × 4.5² = 39
        close(
            ok(
                &VSD_RV_PRESSURE,
                &Inputs::new().with("sbp_mmhg", 120.0).with("vsd_vmax_m_s", 4.5),
            )
            .value("rvsp_mmhg"),
            39.0,
            1e-9,
        );
        // 110 − 4 × 4² = 46
        close(
            ok(
                &PDA_PA_PRESSURE,
                &Inputs::new().with("sbp_mmhg", 110.0).with("pda_vmax_m_s", 4.0),
            )
            .value("pasp_mmhg"),
            46.0,
            1e-9,
        );
        inconsistent(
            &VSD_RV_PRESSURE,
            &Inputs::new().with("sbp_mmhg", 80.0).with("vsd_vmax_m_s", 5.0),
            "vsd_vmax_m_s",
        );
        // 4 × 3² = 36; 4 × (9 − 1.5²) = 27
        let o = ok(
            &COARCTATION_GRADIENT,
            &Inputs::new().with("v2_m_s", 3.0).with("v1_m_s", 1.5),
        );
        close(o.value("peak_gradient_mmhg"), 36.0, 1e-9);
        close(o.value("corrected_gradient_mmhg"), 27.0, 1e-9);
        inconsistent(
            &COARCTATION_GRADIENT,
            &Inputs::new().with("v2_m_s", 2.0).with("v1_m_s", 2.5),
            "v1_m_s",
        );
    }

    #[test]
    fn mean_gradient_is_not_from_the_mean_velocity() {
        // samples 0, 2, 4: mean of 0, 16, 64 = 26.67; 4 × 2² = 16; peak 64
        let o = ok(
            &MEAN_GRADIENT_TRACE,
            &Inputs::new()
                .with("v1_m_s", 0.0)
                .with("v2_m_s", 2.0)
                .with("v3_m_s", 4.0),
        );
        close(o.value("mean_gradient_mmhg"), 80.0 / 3.0, 1e-9);
        close(o.value("from_mean_velocity_mmhg"), 16.0, 1e-9);
        close(o.value("peak_gradient_mmhg"), 64.0, 1e-9);
    }
}
