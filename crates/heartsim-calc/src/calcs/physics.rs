//! Ultrasound physics: period, wavelength and pulse duration; pulse repetition and the Nyquist limit;
//! spatial pulse length, axial resolution and near-field length; the Doppler equation both ways;
//! acoustic impedance, intensity reflection and intensity; Poiseuille flow.

use super::{check, choice, cite, n, num, o, opt, outcome, q, v, when};
use crate::model::ExampleValue::{Choice as C, Number as N};
use crate::model::{Calculator, Category, Choice, InputError, InputSpec, Inputs, Outcome, Review, Unit};
use crate::refs::{OPENSTAX_COLLEGE_2E, OPENSTAX_UNIVERSITY_1, TTE_2019};
use crate::tx;

/// Pascal per mmHg.
const PA_PER_MMHG: f64 = 133.322_387_415;

const F: InputSpec = num(
    "f_mhz",
    tx!("Transmit frequency", "Frequenza di trasmissione"),
    Unit::Mhz,
    0.5,
    20.0,
    1,
);
const C_TISSUE: InputSpec = num(
    "c_m_s",
    tx!(
        "Speed of sound (1540 in soft tissue)",
        "Velocità del suono (1540 nei tessuti molli)"
    ),
    Unit::MS,
    300.0,
    5000.0,
    0,
);
const CYCLES: InputSpec = num(
    "cycles",
    tx!("Cycles per pulse", "Cicli per impulso"),
    Unit::Cycles,
    1.0,
    20.0,
    0,
);

/// Period, wavelength and pulse duration.
pub static WAVE: Calculator = Calculator {
    id: "wave-period-wavelength",
    category: Category::Physics,
    name: tx!(
        "Period, wavelength and pulse duration",
        "Periodo, lunghezza d'onda e durata dell'impulso"
    ),
    synonyms: tx!(
        "period, wavelength, lambda, frequency, pulse duration, PD, speed of sound",
        "periodo, lunghezza d'onda, lambda, frequenza, durata dell'impulso, velocità del suono"
    ),
    summary: tx!(
        "A wave of frequency f travelling at c has period T = 1/f and wavelength λ = c/f. A pulse of n cycles lasts n × T. Higher frequency: shorter wavelength, finer detail, less penetration.",
        "Un'onda di frequenza f che viaggia a velocità c ha periodo T = 1/f e lunghezza d'onda λ = c/f. Un impulso di n cicli dura n × T. Frequenza più alta: lunghezza d'onda più corta, più dettaglio, meno penetrazione."
    ),
    inputs: &[
        F,
        C_TISSUE,
        opt(
            "cycles",
            tx!("Cycles per pulse", "Cicli per impulso"),
            Unit::Cycles,
            1.0,
            20.0,
            0,
        ),
    ],
    compute: |i| {
        let (f, c) = (i.n("f_mhz"), i.n("c_m_s"));
        let mut out = outcome();
        out.push(
            q("period_us", tx!("Period", "Periodo"), 1.0 / f, Unit::Us, 3),
            "T = 1 ÷ f",
            format!("1 ÷ {}", v(f, 1, Unit::Mhz)),
        );
        out.push(
            q(
                "wavelength_mm",
                tx!("Wavelength", "Lunghezza d'onda"),
                c / (f * 1000.0),
                Unit::Mm,
                3,
            ),
            "λ = c ÷ f",
            format!("{} ÷ {}", v(c, 0, Unit::MS), v(f, 1, Unit::Mhz)),
        );
        if let Some(k) = o(i, "cycles") {
            out.push(
                q(
                    "pulse_duration_us",
                    tx!("Pulse duration", "Durata dell'impulso"),
                    k / f,
                    Unit::Us,
                    3,
                ),
                "PD = n × T",
                format!("{} × {}", n(k, 0), v(1.0 / f, 3, Unit::Us)),
            );
        }
        Ok(out)
    },
    references: &[cite(&OPENSTAX_UNIVERSITY_1, "§16.1, v = fλ")],
    review: Review::FormulaOnly,
    example: &[("f_mhz", N(2.5)), ("c_m_s", N(1540.0)), ("cycles", N(3.0))],
    sim_link: None,
};

/// Round-trip time, maximum PRF, Nyquist limit and duty factor.
pub static PRF_NYQUIST: Calculator = Calculator {
    id: "prf-nyquist",
    category: Category::Physics,
    name: tx!(
        "Pulse repetition, Nyquist limit and duty factor",
        "Ripetizione degli impulsi, limite di Nyquist e fattore di servizio"
    ),
    synonyms: tx!(
        "PRF, PRP, pulse repetition frequency, Nyquist, aliasing, duty factor, sample depth, 13 µs per cm",
        "PRF, PRP, frequenza di ripetizione, Nyquist, aliasing, fattore di servizio, profondità del campione, 13 µs per cm"
    ),
    summary: tx!(
        "An echo from depth d returns after 2d/c (about 13 µs per cm in tissue); the next pulse cannot leave earlier, so PRF ≤ c/2d. Pulsed Doppler samples at the PRF, so shifts above PRF/2 alias. The duty factor is the fraction of time the transducer transmits.",
        "Un'eco dalla profondità d torna dopo 2d/c (circa 13 µs per cm nei tessuti); l'impulso successivo non può partire prima, quindi PRF ≤ c/2d. Il Doppler pulsato campiona alla PRF: le frequenze sopra PRF/2 vanno in aliasing. Il fattore di servizio è la frazione di tempo in cui il trasduttore trasmette."
    ),
    inputs: &[
        num(
            "depth_cm",
            tx!("Sample (imaging) depth", "Profondità del campione (immagine)"),
            Unit::Cm,
            0.5,
            30.0,
            1,
        ),
        C_TISSUE,
        opt(
            "prf_khz",
            tx!("PRF in use", "PRF in uso"),
            Unit::Khz,
            0.1,
            50.0,
            2,
        ),
        opt(
            "pd_us",
            tx!("Pulse duration", "Durata dell'impulso"),
            Unit::Us,
            0.1,
            20.0,
            2,
        ),
        opt(
            "f_mhz",
            tx!(
                "Transmit frequency (for the velocity limit)",
                "Frequenza di trasmissione (per il limite di velocità)"
            ),
            Unit::Mhz,
            0.5,
            20.0,
            1,
        ),
    ],
    compute: prf,
    references: &[
        cite(&OPENSTAX_COLLEGE_2E, "§17.7 echo timing, Example 17.8"),
        cite(&TTE_2019, "Spectral Doppler, p. 42"),
    ],
    review: Review::FormulaOnly,
    example: &[
        ("depth_cm", N(12.0)),
        ("c_m_s", N(1540.0)),
        ("prf_khz", N(5.0)),
        ("pd_us", N(1.2)),
        ("f_mhz", N(2.5)),
    ],
    sim_link: None,
};

fn prf(i: &Inputs) -> Result<Outcome, Vec<InputError>> {
    let (d, c) = (i.n("depth_cm"), i.n("c_m_s"));
    let prp_min = 2.0 * d / 100.0 / c * 1e6;
    let prf_max = 1000.0 / prp_min;
    let mut out = outcome();
    out.push(
        q(
            "prp_min_us",
            tx!(
                "Round-trip time (shortest PRP)",
                "Tempo di andata e ritorno (PRP minimo)"
            ),
            prp_min,
            Unit::Us,
            1,
        ),
        "PRP = 2d ÷ c",
        format!("2 × {} ÷ {}", v(d, 1, Unit::Cm), v(c, 0, Unit::MS)),
    );
    out.push(
        q(
            "prf_max_khz",
            tx!("Highest PRF at this depth", "PRF massima a questa profondità"),
            prf_max,
            Unit::Khz,
            2,
        ),
        "PRFmax = 1 ÷ PRP",
        format!("1 ÷ {}", v(prp_min, 1, Unit::Us)),
    );
    let prf = match o(i, "prf_khz") {
        Some(p) => {
            check(
                p > prf_max * (1.0 + 1e-9),
                "prf_khz",
                tx!(
                    "This PRF is higher than the depth allows: echoes would return after the next pulse (range ambiguity)",
                    "Questa PRF è più alta di quanto la profondità consenta: le eco tornerebbero dopo l'impulso successivo (ambiguità di distanza)"
                ),
            )?;
            p
        }
        None => prf_max,
    };
    let prp = 1000.0 / prf;
    let nyq = prf / 2.0;
    out.push(
        q(
            "nyquist_khz",
            tx!("Nyquist limit", "Limite di Nyquist"),
            nyq,
            Unit::Khz,
            2,
        ),
        "Nyquist = PRF ÷ 2",
        format!("{} ÷ 2", v(prf, 2, Unit::Khz)),
    );
    if let Some(pd) = o(i, "pd_us") {
        out.push(
            q(
                "duty_factor_percent",
                tx!("Duty factor", "Fattore di servizio"),
                pd / prp * 100.0,
                Unit::Percent,
                2,
            ),
            "DF = PD ÷ PRP × 100",
            format!("{} ÷ {} × 100", v(pd, 2, Unit::Us), v(prp, 1, Unit::Us)),
        );
    }
    if let Some(f) = o(i, "f_mhz") {
        let vmax = nyq * 1000.0 * c / (2.0 * f * 1e6);
        out.push(
            q(
                "v_nyquist_m_s",
                tx!(
                    "Highest velocity without aliasing (0°)",
                    "Velocità massima senza aliasing (0°)"
                ),
                vmax,
                Unit::MS,
                2,
            ),
            "v = Nyquist × c ÷ (2 f)",
            format!(
                "{} × {} ÷ (2 × {})",
                v(nyq, 2, Unit::Khz),
                v(c, 0, Unit::MS),
                v(f, 1, Unit::Mhz)
            ),
        );
    }
    Ok(out)
}

/// Spatial pulse length, axial resolution and near-field length.
pub static PULSE_LENGTH: Calculator = Calculator {
    id: "spatial-pulse-length",
    category: Category::Physics,
    name: tx!(
        "Spatial pulse length, axial resolution and near field",
        "Lunghezza spaziale dell'impulso, risoluzione assiale e campo vicino"
    ),
    synonyms: tx!(
        "SPL, axial resolution, near field, Fresnel zone, NZL, aperture, wavelength",
        "SPL, risoluzione assiale, campo vicino, zona di Fresnel, apertura, lunghezza d'onda"
    ),
    summary: tx!(
        "A pulse of n cycles occupies n × λ along the beam; two reflectors closer than half of that merge, so axial resolution ≈ SPL ÷ 2. A disc aperture of diameter D keeps the beam converging over a near field of about D² ÷ 4λ.",
        "Un impulso di n cicli occupa n × λ lungo il fascio; due riflettori più vicini della metà si fondono, quindi la risoluzione assiale ≈ SPL ÷ 2. Un'apertura circolare di diametro D mantiene il fascio convergente per un campo vicino di circa D² ÷ 4λ."
    ),
    inputs: &[
        F,
        C_TISSUE,
        CYCLES,
        opt(
            "aperture_mm",
            tx!("Aperture diameter", "Diametro dell'apertura"),
            Unit::Mm,
            1.0,
            50.0,
            1,
        ),
    ],
    compute: |i| {
        let (f, c, k) = (i.n("f_mhz"), i.n("c_m_s"), i.n("cycles"));
        let lambda = c / (f * 1000.0);
        let spl = k * lambda;
        let mut out = outcome();
        out.push(
            q(
                "wavelength_mm",
                tx!("Wavelength", "Lunghezza d'onda"),
                lambda,
                Unit::Mm,
                3,
            ),
            "λ = c ÷ f",
            format!("{} ÷ {}", v(c, 0, Unit::MS), v(f, 1, Unit::Mhz)),
        );
        out.push(
            q(
                "spl_mm",
                tx!("Spatial pulse length", "Lunghezza spaziale dell'impulso"),
                spl,
                Unit::Mm,
                3,
            ),
            "SPL = n × λ",
            format!("{} × {}", n(k, 0), v(lambda, 3, Unit::Mm)),
        );
        out.push(
            q(
                "axial_resolution_mm",
                tx!("Axial resolution", "Risoluzione assiale"),
                spl / 2.0,
                Unit::Mm,
                3,
            ),
            "axial ≈ SPL ÷ 2",
            format!("{} ÷ 2", v(spl, 3, Unit::Mm)),
        );
        if let Some(dm) = o(i, "aperture_mm") {
            out.push(
                q(
                    "near_field_mm",
                    tx!("Near-field length", "Lunghezza del campo vicino"),
                    dm * dm / (4.0 * lambda),
                    Unit::Mm,
                    1,
                ),
                "N = D² ÷ (4λ)",
                format!("{}² ÷ (4 × {})", n(dm, 1), v(lambda, 3, Unit::Mm)),
            );
        }
        Ok(out)
    },
    references: &[
        cite(&OPENSTAX_UNIVERSITY_1, "§16.1, v = fλ"),
        cite(&OPENSTAX_COLLEGE_2E, "§17.7 resolution and wavelength"),
    ],
    review: Review::FormulaOnly,
    example: &[
        ("f_mhz", N(3.0)),
        ("c_m_s", N(1540.0)),
        ("cycles", N(2.0)),
        ("aperture_mm", N(15.0)),
    ],
    sim_link: None,
};

const SOLVE: &[Choice] = &[
    Choice {
        id: "shift",
        label: tx!(
            "Doppler shift from velocity",
            "Spostamento Doppler dalla velocità"
        ),
    },
    Choice {
        id: "velocity",
        label: tx!(
            "Velocity from Doppler shift",
            "Velocità dallo spostamento Doppler"
        ),
    },
];

/// The Doppler equation either way, with the error of an uncorrected angle.
pub static DOPPLER: Calculator = Calculator {
    id: "doppler-shift-velocity",
    category: Category::Physics,
    name: tx!("Doppler equation", "Equazione Doppler"),
    synonyms: tx!(
        "Doppler shift, frequency shift, insonation angle, cos θ, angle error, blood velocity",
        "spostamento Doppler, frequenza Doppler, angolo di insonazione, cos θ, errore angolare, velocità del sangue"
    ),
    summary: tx!(
        "Δf = 2 × f₀ × v × cos θ ÷ c. Only the velocity component along the beam is measured: at 20° the reading is 6 % low, at 60° half. Echocardiography aligns the beam with flow rather than correcting the angle.",
        "Δf = 2 × f₀ × v × cos θ ÷ c. Si misura solo la componente della velocità lungo il fascio: a 20° la lettura è bassa del 6 %, a 60° della metà. In ecocardiografia si allinea il fascio al flusso invece di correggere l'angolo."
    ),
    inputs: &[
        choice("solve", tx!("Solve for", "Calcola"), SOLVE),
        F,
        C_TISSUE,
        num(
            "angle_deg",
            tx!("Beam-to-flow angle θ", "Angolo fascio-flusso θ"),
            Unit::Deg,
            0.0,
            80.0,
            0,
        ),
        when(
            num(
                "v_m_s",
                tx!("Blood velocity", "Velocità del sangue"),
                Unit::MS,
                0.01,
                7.0,
                2,
            ),
            "solve",
            &["shift"],
        ),
        when(
            num(
                "shift_khz",
                tx!("Doppler shift", "Spostamento Doppler"),
                Unit::Khz,
                0.01,
                50.0,
                2,
            ),
            "solve",
            &["velocity"],
        ),
    ],
    compute: |i| {
        let (f, c, a) = (i.n("f_mhz"), i.n("c_m_s"), i.n("angle_deg"));
        let cos = a.to_radians().cos();
        let mut out = outcome();
        if i.choice("solve") == Some("velocity") {
            let df = i.n("shift_khz");
            out.push(
                q(
                    "v_m_s",
                    tx!("Blood velocity", "Velocità del sangue"),
                    df * 1000.0 * c / (2.0 * f * 1e6 * cos),
                    Unit::MS,
                    2,
                ),
                "v = Δf × c ÷ (2 f₀ cos θ)",
                format!(
                    "{} × {} ÷ (2 × {} × cos {})",
                    v(df, 2, Unit::Khz),
                    v(c, 0, Unit::MS),
                    v(f, 1, Unit::Mhz),
                    v(a, 0, Unit::Deg)
                ),
            );
        } else {
            let vel = i.n("v_m_s");
            out.push(
                q(
                    "shift_khz",
                    tx!("Doppler shift", "Spostamento Doppler"),
                    2.0 * f * 1e6 * vel * cos / c / 1000.0,
                    Unit::Khz,
                    3,
                ),
                "Δf = 2 f₀ v cos θ ÷ c",
                format!(
                    "2 × {} × {} × cos {} ÷ {}",
                    v(f, 1, Unit::Mhz),
                    v(vel, 2, Unit::MS),
                    v(a, 0, Unit::Deg),
                    v(c, 0, Unit::MS)
                ),
            );
        }
        out.push(
            q(
                "angle_error_percent",
                tx!("Underestimate if θ is ignored", "Sottostima se θ è ignorato"),
                (1.0 - cos) * 100.0,
                Unit::Percent,
                1,
            ),
            "(1 − cos θ) × 100",
            format!("(1 − cos {}) × 100", v(a, 0, Unit::Deg)),
        );
        Ok(out)
    },
    references: &[cite(&TTE_2019, "Doppler angle, p. 42")],
    review: Review::FormulaOnly,
    example: &[
        ("solve", C("velocity")),
        ("f_mhz", N(2.0)),
        ("c_m_s", N(1540.0)),
        ("angle_deg", N(20.0)),
        ("shift_khz", N(3.9)),
    ],
    sim_link: None,
};

/// Acoustic impedance Z = ρc.
pub static IMPEDANCE: Calculator = Calculator {
    id: "acoustic-impedance",
    category: Category::Physics,
    name: tx!("Acoustic impedance", "Impedenza acustica"),
    synonyms: tx!(
        "impedance, Z, rayl, MRayl, density, speed of sound",
        "impedenza, Z, rayl, MRayl, densità, velocità del suono"
    ),
    summary: tx!(
        "Z = ρ × c. Echoes arise where impedance changes; soft tissues differ little from one another, bone and air very much.",
        "Z = ρ × c. Le eco nascono dove l'impedenza cambia; i tessuti molli differiscono poco tra loro, osso e aria moltissimo."
    ),
    inputs: &[
        num(
            "density_kg_m3",
            tx!("Density ρ", "Densità ρ"),
            Unit::KgM3,
            0.5,
            20000.0,
            1,
        ),
        C_TISSUE,
    ],
    compute: |i| {
        let (rho, c) = (i.n("density_kg_m3"), i.n("c_m_s"));
        let mut out = outcome();
        out.push(
            q(
                "z_mrayl",
                tx!("Acoustic impedance", "Impedenza acustica"),
                rho * c / 1e6,
                Unit::MRayl,
                4,
            ),
            "Z = ρ × c",
            format!("{} × {}", v(rho, 1, Unit::KgM3), v(c, 0, Unit::MS)),
        );
        Ok(out)
    },
    references: &[cite(&OPENSTAX_COLLEGE_2E, "§17.7 Eq. 17.38 and Table 17.5")],
    review: Review::FormulaOnly,
    example: &[("density_kg_m3", N(1060.0)), ("c_m_s", N(1570.0))],
    sim_link: None,
};

/// Intensity reflection coefficient at a boundary at normal incidence.
pub static REFLECTION: Calculator = Calculator {
    id: "reflection-coefficient",
    category: Category::Physics,
    name: tx!(
        "Intensity reflection coefficient",
        "Coefficiente di riflessione dell'intensità"
    ),
    synonyms: tx!(
        "reflection, transmission, boundary, interface, impedance mismatch, gel",
        "riflessione, trasmissione, interfaccia, disadattamento di impedenza, gel"
    ),
    summary: tx!(
        "At normal incidence the reflected fraction of intensity is ((Z₂ − Z₁) ÷ (Z₂ + Z₁))² and the rest is transmitted. Air against skin reflects almost everything, which is why gel is used.",
        "A incidenza normale la frazione di intensità riflessa è ((Z₂ − Z₁) ÷ (Z₂ + Z₁))² e il resto è trasmesso. L'aria contro la cute riflette quasi tutto: per questo si usa il gel."
    ),
    inputs: &[
        num(
            "z1_mrayl",
            tx!("Impedance of medium 1", "Impedenza del mezzo 1"),
            Unit::MRayl,
            0.0001,
            40.0,
            4,
        ),
        num(
            "z2_mrayl",
            tx!("Impedance of medium 2", "Impedenza del mezzo 2"),
            Unit::MRayl,
            0.0001,
            40.0,
            4,
        ),
    ],
    compute: |i| {
        let (z1, z2) = (i.n("z1_mrayl"), i.n("z2_mrayl"));
        let r = ((z2 - z1) / (z2 + z1)).powi(2);
        let mut out = outcome();
        out.push(
            q(
                "reflected_percent",
                tx!("Reflected intensity", "Intensità riflessa"),
                r * 100.0,
                Unit::Percent,
                2,
            ),
            "R = ((Z₂ − Z₁) ÷ (Z₂ + Z₁))² × 100",
            format!(
                "(({} − {}) ÷ ({} + {}))² × 100",
                n(z2, 4),
                n(z1, 4),
                n(z2, 4),
                n(z1, 4)
            ),
        );
        out.push(
            q(
                "transmitted_percent",
                tx!("Transmitted intensity", "Intensità trasmessa"),
                (1.0 - r) * 100.0,
                Unit::Percent,
                2,
            ),
            "T = 100 − R",
            format!("100 − {}", n(r * 100.0, 2)),
        );
        Ok(out)
    },
    references: &[cite(&OPENSTAX_COLLEGE_2E, "§17.7 Eq. 17.39")],
    review: Review::FormulaOnly,
    example: &[("z1_mrayl", N(1.34)), ("z2_mrayl", N(1.70))],
    sim_link: None,
};

/// Intensity as power over beam area.
pub static INTENSITY: Calculator = Calculator {
    id: "acoustic-intensity",
    category: Category::Physics,
    name: tx!("Acoustic intensity", "Intensità acustica"),
    synonyms: tx!(
        "intensity, power, beam area, W/cm², bioeffects",
        "intensità, potenza, area del fascio, W/cm², effetti biologici"
    ),
    summary: tx!(
        "Intensity is power per unit area, I = P ÷ A: focusing the same power onto a smaller area raises the intensity.",
        "L'intensità è la potenza per unità di area, I = P ÷ A: concentrare la stessa potenza su un'area minore aumenta l'intensità."
    ),
    inputs: &[
        num(
            "power_w",
            tx!("Acoustic power", "Potenza acustica"),
            Unit::W,
            0.0001,
            10.0,
            4,
        ),
        num(
            "area_cm2",
            tx!("Beam area", "Area del fascio"),
            Unit::Cm2,
            0.01,
            20.0,
            2,
        ),
    ],
    compute: |i| {
        let (p, a) = (i.n("power_w"), i.n("area_cm2"));
        let mut out = outcome();
        out.push(
            q(
                "intensity_w_cm2",
                tx!("Intensity", "Intensità"),
                p / a,
                Unit::WCm2,
                4,
            ),
            "I = P ÷ A",
            format!("{} ÷ {}", v(p, 4, Unit::W), v(a, 2, Unit::Cm2)),
        );
        Ok(out)
    },
    references: &[cite(&OPENSTAX_COLLEGE_2E, "§17.3 intensity (I = P ÷ A)")],
    review: Review::FormulaOnly,
    example: &[("power_w", N(0.05)), ("area_cm2", N(0.5))],
    sim_link: None,
};

/// Laminar flow through a rigid tube (Poiseuille).
pub static POISEUILLE: Calculator = Calculator {
    id: "poiseuille-flow",
    category: Category::Physics,
    name: tx!(
        "Poiseuille flow and resistance",
        "Flusso e resistenza di Poiseuille"
    ),
    synonyms: tx!(
        "Poiseuille, laminar flow, resistance, viscosity, radius to the fourth power, Hagen",
        "Poiseuille, flusso laminare, resistenza, viscosità, quarta potenza del raggio, Hagen"
    ),
    summary: tx!(
        "Steady laminar flow of a Newtonian fluid in a rigid tube: Q = π r⁴ ΔP ÷ (8 η L) and R = 8 η L ÷ (π r⁴). Halving the radius raises the resistance sixteen-fold. Blood vessels are not rigid and blood is not Newtonian: a teaching model.",
        "Flusso laminare stazionario di un fluido newtoniano in un tubo rigido: Q = π r⁴ ΔP ÷ (8 η L) e R = 8 η L ÷ (π r⁴). Dimezzare il raggio moltiplica la resistenza per sedici. I vasi non sono rigidi e il sangue non è newtoniano: è un modello didattico."
    ),
    inputs: &[
        num("radius_mm", tx!("Radius", "Raggio"), Unit::Mm, 0.05, 20.0, 2),
        num("length_cm", tx!("Length", "Lunghezza"), Unit::Cm, 0.1, 100.0, 1),
        num(
            "viscosity_mpa_s",
            tx!("Viscosity (blood about 3–4)", "Viscosità (sangue circa 3–4)"),
            Unit::MPaS,
            0.3,
            20.0,
            2,
        ),
        num(
            "dp_mmhg",
            tx!("Pressure drop", "Caduta di pressione"),
            Unit::MmHg,
            0.01,
            200.0,
            2,
        ),
    ],
    compute: |i| {
        let (r, l, eta, dp) = (
            i.n("radius_mm") / 1000.0,
            i.n("length_cm") / 100.0,
            i.n("viscosity_mpa_s") / 1000.0,
            i.n("dp_mmhg"),
        );
        let r_si = 8.0 * eta * l / (std::f64::consts::PI * r.powi(4));
        let q_si = dp * PA_PER_MMHG / r_si;
        let mut out = outcome();
        out.push(
            q(
                "resistance_mmhg_s_ml",
                tx!("Resistance", "Resistenza"),
                r_si / PA_PER_MMHG / 1e6,
                Unit::MmHgSPerMl,
                4,
            ),
            "R = 8 η L ÷ (π r⁴)",
            format!("8 × {} Pa·s × {} m ÷ (π × ({} m)⁴)", n(eta, 4), n(l, 3), n(r, 5)),
        );
        out.push(
            q("flow_ml_s", tx!("Flow", "Flusso"), q_si * 1e6, Unit::MlS, 3),
            "Q = ΔP ÷ R",
            format!(
                "{} ÷ {}",
                v(dp, 2, Unit::MmHg),
                v(r_si / PA_PER_MMHG / 1e6, 4, Unit::MmHgSPerMl)
            ),
        );
        Ok(out)
    },
    references: &[cite(&OPENSTAX_UNIVERSITY_1, "§14.7 Eqs. 14.18–14.19")],
    review: Review::FormulaOnly,
    example: &[
        ("radius_mm", N(2.0)),
        ("length_cm", N(10.0)),
        ("viscosity_mpa_s", N(3.5)),
        ("dp_mmhg", N(5.0)),
    ],
    sim_link: None,
};

#[cfg(test)]
mod tests {
    use super::super::testing::{close, inconsistent, ok};
    use super::*;

    #[test]
    fn wave_and_pulse() {
        // 2 MHz in tissue: T 0.5 µs, λ 1540 ÷ 2e6 = 0.77 mm; 3 cycles: PD 1.5 µs, SPL 2.31 mm, axial 1.155; N 10² ÷ 3.08 = 32.47 mm
        let o = ok(
            &WAVE,
            &Inputs::new()
                .with("f_mhz", 2.0)
                .with("c_m_s", 1540.0)
                .with("cycles", 3.0),
        );
        close(o.value("period_us"), 0.5, 1e-12);
        close(o.value("wavelength_mm"), 0.77, 1e-12);
        close(o.value("pulse_duration_us"), 1.5, 1e-12);
        let o = ok(
            &PULSE_LENGTH,
            &Inputs::new()
                .with("f_mhz", 2.0)
                .with("c_m_s", 1540.0)
                .with("cycles", 3.0)
                .with("aperture_mm", 10.0),
        );
        close(o.value("spl_mm"), 2.31, 1e-12);
        close(o.value("axial_resolution_mm"), 1.155, 1e-12);
        close(o.value("near_field_mm"), 32.468, 1e-3);
    }

    #[test]
    fn prf_and_nyquist() {
        // 10 cm: 2 × 0.1 ÷ 1540 = 129.87 µs (13 µs/cm); PRFmax 7.70 kHz; Nyquist 3.85; at 2.5 MHz 3850 × 1540 ÷ 5e6 = 1.186 m/s
        let o = ok(
            &PRF_NYQUIST,
            &Inputs::new()
                .with("depth_cm", 10.0)
                .with("c_m_s", 1540.0)
                .with("f_mhz", 2.5)
                .with("pd_us", 1.3),
        );
        close(o.value("prp_min_us"), 129.87, 0.01);
        close(o.value("prf_max_khz"), 7.70, 0.001);
        close(o.value("nyquist_khz"), 3.85, 0.001);
        close(o.value("v_nyquist_m_s"), 1.186, 1e-3);
        close(o.value("duty_factor_percent"), 1.001, 1e-3);
        inconsistent(
            &PRF_NYQUIST,
            &Inputs::new()
                .with("depth_cm", 10.0)
                .with("c_m_s", 1540.0)
                .with("prf_khz", 9.0),
            "prf_khz",
        );
    }

    #[test]
    fn doppler_both_ways_and_the_angle() {
        // 2 × 2e6 × 1 × 1 ÷ 1540 = 2597.4 Hz
        let o = ok(
            &DOPPLER,
            &Inputs::new()
                .with_choice("solve", "shift")
                .with("f_mhz", 2.0)
                .with("c_m_s", 1540.0)
                .with("angle_deg", 0.0)
                .with("v_m_s", 1.0),
        );
        close(o.value("shift_khz"), 2.5974, 1e-4);
        // back again at 60°: half the shift for the same velocity
        let o = ok(
            &DOPPLER,
            &Inputs::new()
                .with_choice("solve", "velocity")
                .with("f_mhz", 2.0)
                .with("c_m_s", 1540.0)
                .with("angle_deg", 60.0)
                .with("shift_khz", 1.2987),
        );
        close(o.value("v_m_s"), 1.0, 1e-4);
        close(o.value("angle_error_percent"), 50.0, 1e-9);
        // the TTE guideline's "20° → 6 %"
        let o = ok(
            &DOPPLER,
            &Inputs::new()
                .with_choice("solve", "shift")
                .with("f_mhz", 2.0)
                .with("c_m_s", 1540.0)
                .with("angle_deg", 20.0)
                .with("v_m_s", 1.0),
        );
        close(o.value("angle_error_percent"), 6.03, 0.01);
    }

    #[test]
    fn impedance_reflection_intensity_poiseuille() {
        // blood 1060 × 1570 = 1.664 MRayl (OpenStax Table 17.5: 1.66 × 10⁶)
        close(
            ok(
                &IMPEDANCE,
                &Inputs::new().with("density_kg_m3", 1060.0).with("c_m_s", 1570.0),
            )
            .value("z_mrayl"),
            1.6642,
            1e-4,
        );
        // fat 1.34 against muscle 1.70: (0.36 ÷ 3.04)² = 1.40 %
        let o = ok(
            &REFLECTION,
            &Inputs::new().with("z1_mrayl", 1.34).with("z2_mrayl", 1.70),
        );
        close(o.value("reflected_percent"), 1.4024, 1e-3);
        close(o.value("transmitted_percent"), 98.5976, 1e-3);
        close(
            ok(
                &INTENSITY,
                &Inputs::new().with("power_w", 0.1).with("area_cm2", 0.5),
            )
            .value("intensity_w_cm2"),
            0.2,
            1e-12,
        );
        // r 1 mm, L 10 cm, η 3 mPa·s, ΔP 10 mmHg: Q = π × 1e-12 × 1333.2 ÷ (8 × 0.003 × 0.1) = 1.745 mL/s
        let o = ok(
            &POISEUILLE,
            &Inputs::new()
                .with("radius_mm", 1.0)
                .with("length_cm", 10.0)
                .with("viscosity_mpa_s", 3.0)
                .with("dp_mmhg", 10.0),
        );
        close(o.value("flow_ml_s"), 1.7452, 1e-3);
        close(o.value("resistance_mmhg_s_ml"), 5.730, 1e-3);
        // half the radius: sixteen times the resistance
        let half = ok(
            &POISEUILLE,
            &Inputs::new()
                .with("radius_mm", 0.5)
                .with("length_cm", 10.0)
                .with("viscosity_mpa_s", 3.0)
                .with("dp_mmhg", 10.0),
        );
        close(half.value("resistance_mmhg_s_ml"), 16.0 * 5.730, 0.02);
    }
}
