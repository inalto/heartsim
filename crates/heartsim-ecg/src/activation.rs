//! Order of ventricular activation, decided once for the ECG and the heart sounds.
//!
//! The timing engine reports a blocked bundle branch or fascicle as a `Blocked` event between the
//! His activation and the first ventricular activation of the beat, and an ectopic, paced or
//! pre-excited beat by the region its first ventricular event names. The QRS morphology of the strip
//! ([`crate::synth::GaussianSynth`]) and the splitting of the heart sounds ([`crate::pcg`]) both
//! read the beat through the types here, so the strip can never show an RBBB while the
//! phonocardiogram splits S2 the LBBB way.

use heartsim_core::Region;
use serde::{Deserialize, Serialize};

/// Bundle-branch and fascicle block events seen since the last His activation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BranchBlocks(u8);

const BLOCK_RBB: u8 = 1;
const BLOCK_LBB: u8 = 2;
const BLOCK_LAF: u8 = 4;
const BLOCK_LPF: u8 = 8;

impl BranchBlocks {
    /// Record a `Blocked` event of `region` (regions other than the branches and fascicles are
    /// ignored).
    pub fn record_block(&mut self, region: Region) {
        self.0 |= match region {
            Region::RightBundleBranch => BLOCK_RBB,
            Region::LeftBundleBranch => BLOCK_LBB,
            Region::LeftAnteriorFascicle => BLOCK_LAF,
            Region::LeftPosteriorFascicle => BLOCK_LPF,
            _ => 0,
        };
    }

    /// Forget the blocks (a new His activation, or a beat that did not use the His–Purkinje system).
    pub fn clear(&mut self) {
        self.0 = 0;
    }

    /// The conduction pattern of the beat now reaching the ventricles over the His–Purkinje system,
    /// clearing the record for the next beat.
    pub fn take(&mut self) -> ConductionPattern {
        let b = std::mem::take(&mut self.0);
        ConductionPattern::from_bits(b)
    }
}

/// Conduction over the left bundle of one His–Purkinje beat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LeftConduction {
    /// Left bundle and both fascicles conducted.
    Intact,
    /// Left anterior fascicular block (the posterior fascicle conducted).
    AnteriorFascicleBlocked,
    /// Left posterior fascicular block (the anterior fascicle conducted).
    PosteriorFascicleBlocked,
    /// Left bundle-branch block, or both fascicles blocked (which is the same for the ventricles).
    Blocked,
}

/// What a His–Purkinje beat met on its way: the input of the QRS template choice and of the
/// interventricular delay of the heart sounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConductionPattern {
    /// The right bundle branch was blocked.
    pub right_blocked: bool,
    pub left: LeftConduction,
}

impl ConductionPattern {
    /// Both bundles conducted.
    pub const NORMAL: Self = Self {
        right_blocked: false,
        left: LeftConduction::Intact,
    };

    fn from_bits(b: u8) -> Self {
        let lbbb = b & BLOCK_LBB != 0 || b & (BLOCK_LAF | BLOCK_LPF) == BLOCK_LAF | BLOCK_LPF;
        let left = if lbbb {
            LeftConduction::Blocked
        } else if b & BLOCK_LAF != 0 {
            LeftConduction::AnteriorFascicleBlocked
        } else if b & BLOCK_LPF != 0 {
            LeftConduction::PosteriorFascicleBlocked
        } else {
            LeftConduction::Intact
        };
        Self {
            right_blocked: b & BLOCK_RBB != 0,
            left,
        }
    }

    /// The ventricle activated late, if a whole bundle is blocked: the left one in LBBB (the right
    /// ventricle is activated first and the left by slow spread through the septum), the right one in
    /// RBBB. Fascicular blocks delay only part of the left ventricle and count as simultaneous. When
    /// both bundles report a block the left one wins, as it does for the QRS template.
    #[must_use]
    pub fn late_ventricle(self) -> Option<Ventricle> {
        if self.left == LeftConduction::Blocked {
            Some(Ventricle::Left)
        } else if self.right_blocked {
            Some(Ventricle::Right)
        } else {
            None
        }
    }
}

/// One of the two ventricles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Ventricle {
    Left,
    Right,
}

impl Ventricle {
    /// The ventricle an ectopic, paced or pre-excited front starts in, from the region of its first
    /// ventricular event: the right ventricle for `RightVentricle`, the left one otherwise (the
    /// septum activates left to right).
    #[must_use]
    pub fn of_site(region: Region) -> Self {
        if region == Region::RightVentricle {
            Ventricle::Right
        } else {
            Ventricle::Left
        }
    }

    /// The other ventricle.
    #[must_use]
    pub fn other(self) -> Self {
        match self {
            Ventricle::Left => Ventricle::Right,
            Ventricle::Right => Ventricle::Left,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_left_bundle_wins_and_fascicles_are_simultaneous() {
        let pattern = |regions: &[Region]| {
            let mut b = BranchBlocks::default();
            for r in regions {
                b.record_block(*r);
            }
            b.take()
        };
        assert_eq!(pattern(&[]), ConductionPattern::NORMAL);
        assert_eq!(
            pattern(&[Region::RightBundleBranch]).late_ventricle(),
            Some(Ventricle::Right)
        );
        assert_eq!(
            pattern(&[Region::LeftBundleBranch]).late_ventricle(),
            Some(Ventricle::Left)
        );
        let both_fascicles = pattern(&[Region::LeftAnteriorFascicle, Region::LeftPosteriorFascicle]);
        assert_eq!(both_fascicles.left, LeftConduction::Blocked);
        let bifascicular = pattern(&[Region::RightBundleBranch, Region::LeftAnteriorFascicle]);
        assert_eq!(bifascicular.left, LeftConduction::AnteriorFascicleBlocked);
        assert_eq!(bifascicular.late_ventricle(), Some(Ventricle::Right));
        assert_eq!(pattern(&[Region::LeftPosteriorFascicle]).late_ventricle(), None);
        let mut b = BranchBlocks::default();
        b.record_block(Region::RightBundleBranch);
        let _ = b.take();
        assert_eq!(b.take(), ConductionPattern::NORMAL, "take clears");
    }
}
