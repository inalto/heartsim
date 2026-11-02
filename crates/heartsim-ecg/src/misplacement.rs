//! Electrode misplacement detector (brief B10; research §3.6, Batchvarov et al. 2007).
//!
//! The simulator always knows what the same heart would have recorded with the standard
//! placement, so the detector compares the *derived leads* of the current placement with that
//! reference instead of guessing from population norms:
//!
//! 1. **Cable reversals** are permutations of the electrode potentials before the leads are
//!    formed. The reference electrode potentials are recovered from its leads (Wilson terminal as
//!    the zero, the two legs at the same potential) and every clinically relevant permutation is
//!    tried; one that reproduces the current limb leads while the identity does not is a reversal
//!    — e.g. lead I inverted, aVR ↔ aVL, aVF unchanged is RA/LA.
//! 2. **Precordial cable swaps** are found the same way on V1–V6, relative to the Wilson terminal
//!    of the limb permutation found in step 1.
//! 3. **Displacements** are what is left: limb leads that no permutation explains (limb electrodes
//!    on the torso), or individual chest leads that differ from the reference after removing the
//!    common Wilson-terminal shift (V1/V2 too high, V5/V6 too posterior).
//!
//! Errors are relative RMS differences over the whole window, so the detector needs a few
//! beats of both recordings sampled at the same instants. Limb hypotheses are judged on the
//! bipolar leads I, II and III: the augmented leads are linear combinations of them and their gain
//! differs between conventions (Goldberger vs. the unit hexaxial vectors of Tier 0).
//! Pure and deterministic.

use heartsim_core::{Electrode, FrontalAxis, LEAD_COUNT, Lead};

/// Thresholds of the detector, as relative RMS errors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DetectorConfig {
    /// A hypothesis "explains" the recording when its error is below this.
    pub match_tol: f64,
    /// The recording "differs" from the standard placement when the identity error exceeds this.
    pub deviate_tol: f64,
}

impl Default for DetectorConfig {
    fn default() -> Self {
        Self {
            match_tol: 0.15,
            deviate_tol: 0.35,
        }
    }
}

/// One misplacement the detector recognised.
#[derive(Debug, Clone, PartialEq)]
pub enum Finding {
    /// Two cables exchanged (pair in `Electrode` order).
    CableSwap(Electrode, Electrode),
    /// All three limb cables rotated; the value is the site the RA cable is on (LA or LL).
    LimbRotation(Electrode),
    /// Limb leads that no cable reversal explains: limb electrodes on the torso.
    LimbsOnTorso {
        /// Change of the frontal axis at the peak of the QRS, degrees (+ = rightwards/clockwise).
        axis_shift_deg: f64,
    },
    /// Chest electrodes whose leads differ from their standard sites.
    PrecordialDisplaced(Vec<Electrode>),
}

impl Finding {
    /// Short name of the finding.
    #[must_use]
    pub fn title(&self) -> String {
        match self {
            Finding::CableSwap(a, b) => format!("{}/{} cables reversed", a.label(), b.label()),
            Finding::LimbRotation(_) => "Limb cables rotated".to_string(),
            Finding::LimbsOnTorso { .. } => "Limb electrodes on the torso".to_string(),
            Finding::PrecordialDisplaced(v) => {
                let names: Vec<&str> = v.iter().map(|e| e.label()).collect();
                format!("{} misplaced", names.join("/"))
            }
        }
    }

    /// What the learner should have noticed, and why.
    #[must_use]
    pub fn explanation(&self) -> String {
        use Electrode::{LA, LL, RA, RL, V1, V2, V5, V6};
        match self {
            Finding::CableSwap(RA, LA) => "Lead I is inverted (negative P, QRS and T) and aVR is upright, while aVF and the chest leads are unchanged. II and III have exchanged places, as have aVR and aVL. True dextrocardia also inverts lead I, but there V4–V6 lose their R waves; here the chest leads are normal.".into(),
            Finding::CableSwap(RA, LL) => "Lead II is inverted, I and III have become -III and -I, and aVR and aVF have exchanged places (an upright aVR with a negative aVF and inferior P waves upside down); aVL is unchanged.".into(),
            Finding::CableSwap(LA, LL) => "Lead III is inverted, I and II have exchanged places, aVL and aVF have exchanged places; aVR is unchanged. Often missed: the clue is a P wave taller in I than in II, or a newly 'inferior' pattern.".into(),
            Finding::CableSwap(RA, RL) => "Lead II is almost a flat line: the RA cable is on the right leg, so lead II measures the potential difference between the two legs, which is about zero. The Wilson terminal moves too, so every chest lead changes slightly.".into(),
            Finding::CableSwap(LA, RL) => "Lead III is almost a flat line: the LA cable is on the right leg, so lead III measures leg to leg, which is about zero. The Wilson terminal moves too, so every chest lead changes slightly.".into(),
            Finding::CableSwap(a, b) if a.label().starts_with('V') && b.label().starts_with('V') => format!(
                "The R-wave progression is broken: {} looks like the expected {} and vice versa. The precordial R wave should grow steadily from V1 to V4/V5.",
                a.label(),
                b.label()
            ),
            Finding::CableSwap(a, b) => format!(
                "The {} and {} cables are exchanged: the limb leads are a permutation of the expected ones.",
                a.label(),
                b.label()
            ),
            Finding::LimbRotation(_) => "Every limb lead is replaced by another (I, II, III rotate; aVR, aVL, aVF rotate): all three limb cables have moved one position round Einthoven's triangle.".into(),
            Finding::LimbsOnTorso { axis_shift_deg } => format!(
                "No cable reversal explains the limb leads, yet they all differ from the standard: the limb electrodes are on the torso (Mason–Likar). The frontal axis moved {:+.0}° ({}) and the inferior leads changed amplitude. Acceptable for monitoring or exercise testing, not for a diagnostic 12-lead.",
                axis_shift_deg,
                if *axis_shift_deg >= 0.0 { "rightwards" } else { "leftwards" }
            ),
            Finding::PrecordialDisplaced(v) if !v.is_empty() && v.iter().all(|e| matches!(e, V1 | V2)) => "V1/V2 differ from their standard sites while V3–V6 are as expected: the commonest error is V1/V2 placed one or two spaces too high (2nd–3rd intercostal space). Look for a smaller r wave, an rSr' or a negative P wave in V1–V2, which can mimic incomplete RBBB, an old anterior infarction or a Brugada pattern.".into(),
            Finding::PrecordialDisplaced(v) if !v.is_empty() && v.iter().all(|e| matches!(e, V5 | V6)) => "V5/V6 differ from their standard sites while V1–V4 are as expected: they are probably too far back or too low (V5/V6 belong at the V4 level in the anterior and mid-axillary lines). Smaller R waves there can mimic poor R-wave progression or a lateral infarction.".into(),
            Finding::PrecordialDisplaced(v) => format!(
                "{} differ from their standard sites: check the intercostal space (count down from the sternal angle) and the vertical line of each chest electrode.",
                v.iter().map(|e| e.label()).collect::<Vec<_>>().join(", ")
            ),
        }
    }
}

/// Cable → site permutation of the four limb cables `[RA, LA, LL, RL]` (index into the same).
type LimbPerm = [usize; 4];

/// The limb permutations worth testing. LL ↔ RL is left out: both legs carry the same
/// potential, so that reversal changes nothing on the ECG.
const LIMB_PERMS: [(LimbPerm, Option<Finding>); 8] = [
    ([0, 1, 2, 3], None),
    (
        [1, 0, 2, 3],
        Some(Finding::CableSwap(Electrode::RA, Electrode::LA)),
    ),
    (
        [2, 1, 0, 3],
        Some(Finding::CableSwap(Electrode::RA, Electrode::LL)),
    ),
    (
        [0, 2, 1, 3],
        Some(Finding::CableSwap(Electrode::LA, Electrode::LL)),
    ),
    (
        [3, 1, 2, 0],
        Some(Finding::CableSwap(Electrode::RA, Electrode::RL)),
    ),
    (
        [0, 3, 2, 1],
        Some(Finding::CableSwap(Electrode::LA, Electrode::RL)),
    ),
    ([1, 2, 0, 3], Some(Finding::LimbRotation(Electrode::LA))),
    ([2, 0, 1, 3], Some(Finding::LimbRotation(Electrode::LL))),
];

const PRECORDIAL: [Electrode; 6] = crate::placement::PRECORDIAL;

/// Potentials `[RA, LA, LL, RL, V1..V6]` of one reference sample, Wilson terminal = 0.
fn electrode_potentials(leads: &[f64; LEAD_COUNT]) -> [f64; 10] {
    let (i, ii) = (leads[Lead::I as usize], leads[Lead::II as usize]);
    let ll = (2.0 * ii - i) / 3.0;
    let mut p = [
        -(i + ii) / 3.0,
        (2.0 * i - ii) / 3.0,
        ll,
        ll,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
    ];
    for k in 0..6 {
        p[4 + k] = leads[Lead::V1 as usize + k];
    }
    p
}

/// Bipolar limb leads `[I, II, III]` from RA, LA, LL potentials.
fn limb_leads(ra: f64, la: f64, ll: f64) -> [f64; 3] {
    [la - ra, ll - ra, ll - la]
}

/// Frontal axis of a sample from I and II, degrees (exact Einthoven: `atan2((2·II − I)/√3, I)`).
fn frontal_axis_deg(s: &[f64; LEAD_COUNT]) -> f64 {
    let (i, ii) = (s[Lead::I as usize], s[Lead::II as usize]);
    ((2.0 * ii - i) / 3f64.sqrt()).atan2(i).to_degrees()
}

/// Compare `current` with `reference` (twelve leads each, `Lead as usize` order, sampled at the
/// same instants) and return what is wrong with the placement, most important first. Empty when
/// the placement is standard.
///
/// # Panics
/// If either slice does not hold twelve leads.
#[must_use]
pub fn detect_misplacement(current: &[&[f32]], reference: &[&[f32]], cfg: &DetectorConfig) -> Vec<Finding> {
    assert_eq!(current.len(), LEAD_COUNT, "twelve current leads");
    assert_eq!(reference.len(), LEAD_COUNT, "twelve reference leads");
    let n = current
        .iter()
        .chain(reference)
        .map(|s| s.len())
        .min()
        .unwrap_or(0);
    if n == 0 {
        return Vec::new();
    }
    let sample =
        |src: &[&[f32]], t: usize| -> [f64; LEAD_COUNT] { std::array::from_fn(|l| f64::from(src[l][t])) };
    let cur: Vec<[f64; LEAD_COUNT]> = (0..n).map(|t| sample(current, t)).collect();
    let refs: Vec<[f64; LEAD_COUNT]> = (0..n).map(|t| sample(reference, t)).collect();
    let pots: Vec<[f64; 10]> = refs.iter().map(electrode_potentials).collect();

    // --- 1. limb cables
    let limb_norm = refs
        .iter()
        .map(|s| s[..3].iter().map(|v| v * v).sum::<f64>())
        .sum::<f64>()
        .sqrt()
        .max(1e-9);
    let limb_error = |perm: &LimbPerm| -> f64 {
        let sq: f64 = cur
            .iter()
            .zip(&pots)
            .map(|(c, p)| {
                let pred = limb_leads(p[perm[0]], p[perm[1]], p[perm[2]]);
                (0..3).map(|l| (c[l] - pred[l]).powi(2)).sum::<f64>()
            })
            .sum();
        sq.sqrt() / limb_norm
    };
    let errors: Vec<f64> = LIMB_PERMS.iter().map(|(p, _)| limb_error(p)).collect();
    let identity_error = errors[0];
    let (best, best_error) = errors
        .iter()
        .copied()
        .enumerate()
        .skip(1)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap_or((0, f64::INFINITY));
    let mut findings = Vec::new();
    let mut perm: LimbPerm = LIMB_PERMS[0].0;
    if identity_error > cfg.deviate_tol {
        if best_error < cfg.match_tol {
            perm = LIMB_PERMS[best].0;
            findings.extend(LIMB_PERMS[best].1.clone());
        } else {
            let peak = (0..n)
                .max_by(|&a, &b| {
                    let m = |s: &[f64; LEAD_COUNT]| s[0] * s[0] + s[1] * s[1];
                    m(&refs[a]).total_cmp(&m(&refs[b]))
                })
                .unwrap_or(0);
            let shift = FrontalAxis::normalise(frontal_axis_deg(&cur[peak]) - frontal_axis_deg(&refs[peak]));
            findings.push(Finding::LimbsOnTorso {
                axis_shift_deg: shift,
            });
        }
    }

    // --- 2./3. chest cables, relative to the Wilson terminal of `perm`
    let pred = |t: usize, site: usize| -> f64 {
        let p = &pots[t];
        p[4 + site] - (p[perm[0]] + p[perm[1]] + p[perm[2]]) / 3.0
    };
    // common Wilson-terminal shift the permutation does not explain (limbs on the torso): the
    // median over the six leads of current − predicted, per sample
    let common: Vec<f64> = (0..n)
        .map(|t| {
            let mut d: [f64; 6] = std::array::from_fn(|k| cur[t][Lead::V1 as usize + k] - pred(t, k));
            d.sort_by(f64::total_cmp);
            f64::midpoint(d[2], d[3])
        })
        .collect();
    let norms: [f64; 6] = std::array::from_fn(|k| {
        refs.iter()
            .map(|s| s[Lead::V1 as usize + k].powi(2))
            .sum::<f64>()
            .sqrt()
    });
    let floor = 0.25 * norms.iter().sum::<f64>() / 6.0;
    // relative error of cable `k` reading the site of cable `site`
    let residual = |k: usize, site: usize| -> f64 {
        let sq: f64 = (0..n)
            .map(|t| (cur[t][Lead::V1 as usize + k] - common[t] - pred(t, site)).powi(2))
            .sum();
        sq.sqrt() / norms[k].max(floor).max(1e-9)
    };
    let own: [f64; 6] = std::array::from_fn(|k| residual(k, k));
    let mut swapped = [false; 6];
    let mut pairs: Vec<(f64, usize, usize)> = Vec::new();
    for a in 0..6 {
        for b in a + 1..6 {
            if own[a] > cfg.deviate_tol && own[b] > cfg.deviate_tol {
                let (ra, rb) = (residual(a, b), residual(b, a));
                if ra < cfg.match_tol && rb < cfg.match_tol {
                    pairs.push((ra + rb, a, b));
                }
            }
        }
    }
    pairs.sort_by(|x, y| x.0.total_cmp(&y.0));
    for (_, a, b) in pairs {
        if !swapped[a] && !swapped[b] {
            swapped[a] = true;
            swapped[b] = true;
            findings.push(Finding::CableSwap(PRECORDIAL[a], PRECORDIAL[b]));
        }
    }
    let displaced: Vec<Electrode> = (0..6)
        .filter(|&k| !swapped[k] && own[k] > cfg.deviate_tol)
        .map(|k| PRECORDIAL[k])
        .collect();
    if !displaced.is_empty() {
        findings.push(Finding::PrecordialDisplaced(displaced));
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use heartsim_core::{ElectrodePotentials, TwelveLead};

    /// A synthetic "beat": electrode potentials from a rotating dipole seen by fixed vectors.
    fn record(perm: [Electrode; 10]) -> Vec<Vec<f32>> {
        let mut out = vec![Vec::new(); LEAD_COUNT];
        for t in 0..400 {
            let a = f64::from(t) * 0.03;
            let mut p = ElectrodePotentials::default();
            let phys = |e: Electrode| -> f64 {
                let k = f64::from(e as u8);
                match e {
                    // the legs share one potential
                    Electrode::RL | Electrode::LL => {
                        f64::midpoint(a.sin() * 0.3, a.cos() * 0.8) + 0.2 * (2.0 * a).sin()
                    }
                    _ => (a * (1.0 + 0.1 * k)).sin() * (0.5 + 0.1 * k) + 0.3 * (a + k).cos(),
                }
            };
            for e in Electrode::ALL {
                p.set(e, phys(perm[e as usize]));
            }
            let l: TwelveLead = p.to_leads();
            for (o, v) in out.iter_mut().zip(l.0) {
                o.push(v as f32);
            }
        }
        out
    }

    fn run(perm: [Electrode; 10]) -> Vec<Finding> {
        let reference = record(Electrode::ALL);
        let current = record(perm);
        let r: Vec<&[f32]> = reference.iter().map(Vec::as_slice).collect();
        let c: Vec<&[f32]> = current.iter().map(Vec::as_slice).collect();
        detect_misplacement(&c, &r, &DetectorConfig::default())
    }

    fn swapped(a: Electrode, b: Electrode) -> [Electrode; 10] {
        let mut p = Electrode::ALL;
        p.swap(a as usize, b as usize);
        p
    }

    #[test]
    fn identity_is_silent_and_swaps_are_named() {
        use Electrode::*;
        assert_eq!(run(Electrode::ALL), Vec::new());
        for (a, b) in [
            (RA, LA),
            (RA, LL),
            (LA, LL),
            (RA, RL),
            (LA, RL),
            (V1, V2),
            (V3, V4),
        ] {
            assert_eq!(run(swapped(a, b)), vec![Finding::CableSwap(a, b)], "{a:?}/{b:?}");
        }
    }
}
