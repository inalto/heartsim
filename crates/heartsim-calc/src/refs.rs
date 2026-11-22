//! The publications the calculators rest on. Every formula and threshold was read in these during
//! brief B29 (see `docs/research/echo-calculators.md` for what was read where).

use crate::model::Reference;

/// EACVI/ASE focused update on aortic stenosis (continuity equation, velocity ratio, Table 3).
pub static AS_2017: Reference = Reference {
    authors: "Baumgartner H, Hung J, Bermejo J, et al.",
    title: "Recommendations on the echocardiographic assessment of aortic valve stenosis: a focused update from the EACVI and the ASE",
    journal: "J Am Soc Echocardiogr 2017;30:372–392",
    year: 2017,
    doi: Some("10.1016/j.echo.2017.02.009"),
    url: Some("https://www.asecho.org/wp-content/uploads/2017/04/2017ValveStenosisGuideline.pdf"),
};

/// EAE/ASE recommendations on valve stenosis (mitral PISA, pulmonic stenosis Table 11).
pub static VS_2009: Reference = Reference {
    authors: "Baumgartner H, Hung J, Bermejo J, et al.",
    title: "Echocardiographic assessment of valve stenosis: EAE/ASE recommendations for clinical practice",
    journal: "J Am Soc Echocardiogr 2009;22:1–23",
    year: 2009,
    doi: Some("10.1016/j.echo.2008.11.029"),
    url: Some(
        "https://www.asecho.org/wp-content/uploads/2025/04/2009_Echo-Assessment-of-Valve-Stenosis_note_added.pdf",
    ),
};

/// ASE recommendations on rheumatic heart disease (mitral stenosis Table 1, tricuspid stenosis).
pub static RHD_2023: Reference = Reference {
    authors: "Pandian NG, Kim JK, Arias-Godinez JA, et al.",
    title: "Recommendations for the use of echocardiography in the evaluation of rheumatic heart disease",
    journal: "J Am Soc Echocardiogr 2023;36:3–28",
    year: 2023,
    doi: Some("10.1016/j.echo.2022.10.009"),
    url: Some("https://www.asecho.org/wp-content/uploads/2025/04/PIIS0894731722005211.pdf"),
};

/// ASE recommendations on native valvular regurgitation (PISA, volumetric method, Tables 8, 11, 14, 16).
pub static VR_2017: Reference = Reference {
    authors: "Zoghbi WA, Adams D, Bonow RO, et al.",
    title: "Recommendations for noninvasive evaluation of native valvular regurgitation",
    journal: "J Am Soc Echocardiogr 2017;30:303–371",
    year: 2017,
    doi: Some("10.1016/j.echo.2017.01.007"),
    url: Some("https://www.asecho.org/wp-content/uploads/2017/04/2017VavularRegurgitationGuideline.pdf"),
};

/// ASE 2025 diastolic function and HFpEF (E/e', constriction signs).
pub static DD_2025: Reference = Reference {
    authors: "Nagueh SF, Sanborn DY, Oh JK, et al.",
    title: "Recommendations for the evaluation of left ventricular diastolic function by echocardiography and for heart failure with preserved ejection fraction diagnosis",
    journal: "J Am Soc Echocardiogr 2025;38:537–569",
    year: 2025,
    doi: Some("10.1016/j.echo.2025.03.011"),
    url: Some("https://www.asecho.org/wp-content/uploads/2025/07/Left-Ventricular-Diastolic-Function.pdf"),
};

/// ASE/EACVI chamber quantification (LV mass, RWT, LA volume Table 4, wall-motion score).
pub static CQ_2015: Reference = Reference {
    authors: "Lang RM, Badano LP, Mor-Avi V, et al.",
    title: "Recommendations for cardiac chamber quantification by echocardiography in adults",
    journal: "J Am Soc Echocardiogr 2015;28:1–39.e14",
    year: 2015,
    doi: Some("10.1016/j.echo.2014.10.003"),
    url: Some("https://www.asecho.org/wp-content/uploads/2025/04/2015_ChamberQuantificationREV.pdf"),
};

/// ASE/EACVI 2025 strain consensus (relative change of GLS).
pub static STRAIN_2025: Reference = Reference {
    authors: "Thomas JD, Edvardsen T, Abraham T, et al.",
    title: "Clinical applications of strain echocardiography: a clinical consensus statement",
    journal: "J Am Soc Echocardiogr 2025;38:985–1020",
    year: 2025,
    doi: Some("10.1016/j.echo.2025.07.007"),
    url: Some("https://www.asecho.org/wp-content/uploads/2025/11/Strain-2025-Guideline-2.pdf"),
};

/// ASE comprehensive transthoracic examination (Doppler angle).
pub static TTE_2019: Reference = Reference {
    authors: "Mitchell C, Rahko PS, Blauwet LA, et al.",
    title: "Guidelines for performing a comprehensive transthoracic echocardiographic examination in adults",
    journal: "J Am Soc Echocardiogr 2019;32:1–64",
    year: 2019,
    doi: Some("10.1016/j.echo.2018.06.004"),
    url: Some("https://www.asecho.org/wp-content/uploads/2025/04/2019_Comprehensive-TTE.pdf"),
};

/// ASE 2025 right heart guideline (Table 1 partitions, Table 4 haemodynamic formulas).
pub static RH_2025: Reference = Reference {
    authors: "Mukherjee M, Rudski LG, Addetia K, et al.",
    title: "Guidelines for the echocardiographic assessment of the right heart in adults and special considerations in pulmonary hypertension",
    journal: "J Am Soc Echocardiogr 2025;38:141–186",
    year: 2025,
    doi: Some("10.1016/j.echo.2025.01.006"),
    url: Some("https://www.asecho.org/wp-content/uploads/2026/08/ASE-Right-Heart-Guidelines-May-2025.pdf"),
};

/// ASE multimodality imaging of pericardial disease (effusion size, respiratory variation).
pub static PERI_2013: Reference = Reference {
    authors: "Klein AL, Abbara S, Agler DA, et al.",
    title: "American Society of Echocardiography clinical recommendations for multimodality cardiovascular imaging of patients with pericardial disease",
    journal: "J Am Soc Echocardiogr 2013;26:965–1012",
    year: 2013,
    doi: Some("10.1016/j.echo.2013.06.023"),
    url: Some(
        "https://www.asecho.org/wp-content/uploads/2025/04/2013_Multimodality-CV-Imaging-for-Pericardial-Disease.pdf",
    ),
};

/// ASE/SCAI atrial septal defect and PFO (Qp/Qs).
pub static ASD_2015: Reference = Reference {
    authors: "Silvestry FE, Cohen MS, Armsby LB, et al.",
    title: "Guidelines for the echocardiographic assessment of atrial septal defect and patent foramen ovale",
    journal: "J Am Soc Echocardiogr 2015;28:910–958",
    year: 2015,
    doi: Some("10.1016/j.echo.2015.05.015"),
    url: Some("https://www.asecho.org/wp-content/uploads/2025/04/2015_ASD-PFO.pdf"),
};

/// ASE 2024 prosthetic valve guideline (EOA, DVI).
pub static PV_2024: Reference = Reference {
    authors: "Zoghbi WA, Jone PN, Chamsi-Pasha MA, et al.",
    title: "Guidelines for the evaluation of prosthetic valve function with cardiovascular imaging",
    journal: "J Am Soc Echocardiogr 2024;37:2–63",
    year: 2024,
    doi: Some("10.1016/j.echo.2023.10.004"),
    url: Some("https://www.asecho.org/wp-content/uploads/2024/01/PIIS0894731723005333.pdf"),
};

/// Left atrial pressure from the MR jet and the systolic blood pressure.
pub static GORCSAN_1991: Reference = Reference {
    authors: "Gorcsan J 3rd, Snow FR, Paulsen W, Nixon JV.",
    title: "Noninvasive estimation of left atrial pressure in patients with congestive heart failure and mitral regurgitation by Doppler echocardiography",
    journal: "Am Heart J 1991;121:858–863",
    year: 1991,
    doi: Some("10.1016/0002-8703(91)90199-r"),
    url: Some("https://pubmed.ncbi.nlm.nih.gov/2000753/"),
};

/// LV–LA and aorta–LV gradients from the MR and AR jets.
pub static NISHIMURA_1988: Reference = Reference {
    authors: "Nishimura RA, Tajik AJ.",
    title: "Determination of left-sided pressure gradients by utilizing Doppler aortic and mitral regurgitant signals: validation by simultaneous dual catheter and Doppler studies",
    journal: "J Am Coll Cardiol 1988;11:317–321",
    year: 1988,
    doi: Some("10.1016/0735-1097(88)90096-4"),
    url: Some("https://pubmed.ncbi.nlm.nih.gov/3339169/"),
};

/// Mosteller body surface area.
pub static MOSTELLER_1987: Reference = Reference {
    authors: "Mosteller RD.",
    title: "Simplified calculation of body-surface area",
    journal: "N Engl J Med 1987;317:1098",
    year: 1987,
    doi: Some("10.1056/NEJM198710223171717"),
    url: Some("https://pubmed.ncbi.nlm.nih.gov/3657876/"),
};

/// Du Bois body surface area (the height–weight form; exponents in the 1916 note).
pub static DU_BOIS_1916: Reference = Reference {
    authors: "Du Bois D, Du Bois EF.",
    title: "A height-weight formula to estimate the surface area of man",
    journal: "Proc Soc Exp Biol Med 1916;13:77–78",
    year: 1916,
    doi: Some("10.3181/00379727-13-45"),
    url: None,
};

/// Haycock body surface area.
pub static HAYCOCK_1978: Reference = Reference {
    authors: "Haycock GB, Schwartz GJ, Wisotsky DH.",
    title: "Geometric method for measuring body surface area: a height-weight formula validated in infants, children, and adults",
    journal: "J Pediatr 1978;93:62–66",
    year: 1978,
    doi: Some("10.1016/s0022-3476(78)80601-5"),
    url: Some("https://pubmed.ncbi.nlm.nih.gov/650346/"),
};

/// Open textbook: impedance, reflection, intensity (chapter 17).
pub static OPENSTAX_COLLEGE_2E: Reference = Reference {
    authors: "Urone PP, Hinrichs R.",
    title: "College Physics 2e, §17.7 Ultrasound",
    journal: "OpenStax, Rice University",
    year: 2022,
    doi: None,
    url: Some("https://openstax.org/books/college-physics-2e/pages/17-7-ultrasound"),
};

/// Open textbook: waves (chapter 16) and viscous flow (chapter 14).
pub static OPENSTAX_UNIVERSITY_1: Reference = Reference {
    authors: "Ling SJ, Sanny J, Moebs W.",
    title: "University Physics Volume 1, §14.7 Viscosity and turbulence; §16.1 Traveling waves",
    journal: "OpenStax, Rice University",
    year: 2016,
    doi: None,
    url: Some("https://openstax.org/books/university-physics-volume-1/pages/14-7-viscosity-and-turbulence"),
};
