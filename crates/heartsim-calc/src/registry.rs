//! The list of every calculator. Ids are stable: favourites and the command line refer to them.

use crate::calcs::{
    as_pathway, diastolic, filling, pericardium, physics, prosthetic, prosthetic_screen, regurgitation,
    right_heart, shunts, stenosis, ventricle,
};
use crate::model::{Calculator, Category};

/// Every calculator, grouped by category in page order.
pub static ALL: &[&Calculator] = &[
    &stenosis::AORTIC_VALVE_AREA,
    &stenosis::MITRAL_AREA_PHT,
    &stenosis::MITRAL_AREA_CONTINUITY,
    &stenosis::MITRAL_AREA_PISA,
    &stenosis::PULMONIC_STENOSIS,
    &stenosis::TRICUSPID_STENOSIS,
    &as_pathway::PATHWAY,
    &regurgitation::MR_PISA,
    &regurgitation::AR_PISA,
    &regurgitation::TR_PISA,
    &regurgitation::MR_VOLUMETRIC,
    &regurgitation::AR_VOLUMETRIC,
    &regurgitation::PR_VOLUMETRIC,
    &filling::E_E_PRIME,
    &filling::LAP_FROM_MR,
    &filling::LV_PRESSURE_FROM_JETS,
    &filling::LA_VOLUME_INDEX,
    &diastolic::SINUS_RHYTHM,
    &diastolic::ATRIAL_FIBRILLATION,
    &diastolic::MAC,
    &diastolic::PULMONARY_HYPERTENSION,
    &diastolic::HEART_TRANSPLANT,
    &diastolic::EXERCISE,
    &diastolic::MITRAL_STENOSIS,
    &diastolic::MITRAL_REGURGITATION,
    &diastolic::HCM,
    &diastolic::RESTRICTIVE,
    &diastolic::SINUS_TACHYCARDIA,
    &diastolic::LVAD,
    &ventricle::FRACTIONAL_SHORTENING,
    &ventricle::LV_DIAMETER_INDEX,
    &ventricle::LV_MASS,
    &ventricle::LV_VOLUMES_INDEXED,
    &ventricle::EJECTION_FRACTION,
    &ventricle::GLS_CHANGE,
    &ventricle::LVOT_STROKE_VOLUME,
    &ventricle::AORTIC_DIAMETER_INDEX,
    &ventricle::WALL_MOTION_SCORE,
    &right_heart::RAP_FROM_IVC,
    &right_heart::RVSP_PASP,
    &right_heart::PA_PRESSURES_FROM_PR,
    &right_heart::MPAP_FROM_ACCT,
    &right_heart::PVR_DOPPLER,
    &right_heart::RV_SYSTOLIC_FUNCTION,
    &right_heart::TAPSE_PASP,
    &right_heart::RV_MPI,
    &right_heart::RIGHT_ATRIAL_SIZE,
    &right_heart::RIGHT_VENTRICULAR_SIZE,
    &right_heart::BODY_SURFACE_AREA,
    &pericardium::EFFUSION_SIZE,
    &pericardium::MITRAL_RESPIRATORY,
    &pericardium::TRICUSPID_RESPIRATORY,
    &pericardium::LVOT_RESPIRATORY,
    &pericardium::HEPATIC_REVERSAL,
    &pericardium::SEPTAL_LATERAL_E_PRIME,
    &shunts::QP_QS,
    &shunts::SHUNT_FLOW,
    &shunts::VSD_RV_PRESSURE,
    &shunts::PDA_PA_PRESSURE,
    &shunts::COARCTATION_GRADIENT,
    &shunts::MEAN_GRADIENT_TRACE,
    &prosthetic::AORTIC_PROSTHESIS,
    &prosthetic::MITRAL_PROSTHESIS,
    &prosthetic_screen::AORTIC_SCREENING,
    &prosthetic_screen::AORTIC_SERIAL,
    &prosthetic_screen::MITRAL_SCREENING,
    &physics::WAVE,
    &physics::PRF_NYQUIST,
    &physics::PULSE_LENGTH,
    &physics::DOPPLER,
    &physics::IMPEDANCE,
    &physics::REFLECTION,
    &physics::INTENSITY,
    &physics::POISEUILLE,
];

/// The calculator with id `id`.
#[must_use]
pub fn by_id(id: &str) -> Option<&'static Calculator> {
    ALL.iter().copied().find(|c| c.id == id)
}

/// The calculators of one category, in list order.
pub fn in_category(category: Category) -> impl Iterator<Item = &'static Calculator> {
    ALL.iter().copied().filter(move |c| c.category == category)
}

/// The calculators that match `query` in `lang` (see [`Calculator::matches`]); all of them for an
/// empty query.
#[must_use]
pub fn search(query: &str, lang: &str) -> Vec<&'static Calculator> {
    ALL.iter().copied().filter(|c| c.matches(query, lang)).collect()
}
