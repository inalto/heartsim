//! One hint at a time for a view that is not found yet (brief B33, item 4): the pose error against
//! the view's reference pose, decomposed into the six movements of Mitchell et al. 2019, p5 (slide,
//! rotate, tilt, rock, angle, and the machine's depth), and the largest correctable one named with
//! its direction relative to the patient. Specific hints come first: lost contact, a rib, lung in the
//! way, the marker on the wrong side, a foreshortened apical view.
//!
//! The hint says what to do, not how far: the page shows the words, and the reference pose is never
//! drawn unless the learner presses *Show me*. All tolerances are in [`ViewTolerances`].

use crate::anatomy::EchoAnatomy;
use crate::probe::ProbePose;
use crate::views::{ViewId, ViewTargets, beam_occlusion};
use glam::Vec3;

/// The six movements a hint can name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Movement {
    /// Move the transducer on the skin (Mitchell p5).
    Slide,
    /// Turn the index marker, the transducer staying put.
    Rotate,
    /// Fan the beam to another plane, same axis (the plane moves out of itself).
    Tilt,
    /// Swing within the plane, toward or away from the marker.
    Rock,
    /// Keep the position, aim the beam at the structure (tilt and rock together).
    Angle,
    /// The machine's imaging depth.
    Depth,
}

impl Movement {
    /// Every movement.
    pub const ALL: [Self; 6] = [
        Self::Slide,
        Self::Rotate,
        Self::Tilt,
        Self::Rock,
        Self::Angle,
        Self::Depth,
    ];

    /// Stable key (the interface names it `echo-move-<key>`).
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Slide => "slide",
            Self::Rotate => "rotate",
            Self::Tilt => "tilt",
            Self::Rock => "rock",
            Self::Angle => "angle",
            Self::Depth => "depth",
        }
    }
}

/// A direction on the patient, eight ways, seen in the frame of the heart (+x the patient's left,
/// +y cranial).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PatientDir {
    Left,
    LeftShoulder,
    Head,
    RightShoulder,
    Right,
    RightHip,
    Foot,
    LeftHip,
}

impl PatientDir {
    /// The direction of the (x, y) components of `v`.
    #[must_use]
    pub fn of(v: Vec3) -> Self {
        let a = v.y.atan2(v.x).to_degrees();
        let i = ((a / 45.0).round() as i32).rem_euclid(8);
        match i {
            0 => Self::Left,
            1 => Self::LeftShoulder,
            2 => Self::Head,
            3 => Self::RightShoulder,
            4 => Self::Right,
            5 => Self::RightHip,
            6 => Self::Foot,
            _ => Self::LeftHip,
        }
    }

    /// Stable key (`echo-dir-<key>`).
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::LeftShoulder => "left-shoulder",
            Self::Head => "head",
            Self::RightShoulder => "right-shoulder",
            Self::Right => "right",
            Self::RightHip => "right-hip",
            Self::Foot => "foot",
            Self::LeftHip => "left-hip",
        }
    }
}

/// Which way to move.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Toward this side of the patient.
    Patient(PatientDir),
    /// Turn the marker clockwise (seen from outside, looking along the beam).
    Clockwise,
    /// ... counterclockwise.
    CounterClockwise,
    /// More depth.
    Deeper,
    /// Less depth.
    Shallower,
}

/// What kind of hint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HintKind {
    /// Nothing to correct: the view is found.
    Found,
    /// The beam is outside the contact limit; the image is empty.
    LostContact,
    /// A rib is under the transducer.
    OverRib,
    /// Aerated lung lies between the transducer and the heart.
    OverLung,
    /// The index marker points the other way.
    WrongMarkerSide,
    /// An apical view whose plane misses the true apex.
    Foreshortened,
    /// Make this movement ([`Hint::movement`]) in this direction.
    Move,
}

/// One hint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hint {
    pub kind: HintKind,
    /// The movement to make (for the specific kinds, the one that fixes them).
    pub movement: Movement,
    pub direction: Direction,
    /// How far is left: mm for a slide and depth, degrees otherwise.
    pub amount: f32,
}

impl Hint {
    /// Stable key (`echo-hint-<key>`) of the specific kinds, `None` for [`HintKind::Move`].
    #[must_use]
    pub const fn kind_key(&self) -> Option<&'static str> {
        match self.kind {
            HintKind::Found => Some("found"),
            HintKind::LostContact => Some("lost-contact"),
            HintKind::OverRib => Some("rib"),
            HintKind::OverLung => Some("lung"),
            HintKind::WrongMarkerSide => Some("marker"),
            HintKind::Foreshortened => Some("foreshortened"),
            HintKind::Move => None,
        }
    }
}

fn wrap_deg(d: f32) -> f32 {
    (d + 180.0).rem_euclid(360.0) - 180.0
}

/// The direction the beam tip (100 mm out) moves by when `to`'s angles replace `from`'s.
fn tip_shift(anatomy: &EchoAnatomy, from: &ProbePose, to: &ProbePose) -> Vec3 {
    let limit = anatomy.designed.contact_limit_deg;
    let a = from.frame(limit);
    let b = to.frame(limit);
    (b.apex_mm + b.beam * 100.0) - (a.apex_mm + a.beam * 100.0)
}

impl ViewTargets {
    /// The hint for `pose` in view `id`: [`HintKind::Found`] when the score is at least the found
    /// threshold; else the first of lost contact, rib, lung, wrong marker side, foreshortening, and
    /// otherwise the movement whose error against the reference pose is largest in units of its
    /// tolerance (a slide first when it is beyond its tolerance: find the window, then the angles).
    #[must_use]
    pub fn hint(&self, anatomy: &EchoAnatomy, id: ViewId, pose: &ProbePose) -> Hint {
        let tol = &self.tolerances;
        let pose = pose.clamped();
        let score = self.score(anatomy, id, &pose);
        let reference = self.reference(anatomy, id);
        let r = &reference.pose;
        let found = Hint {
            kind: HintKind::Found,
            movement: Movement::Slide,
            direction: Direction::Patient(PatientDir::Head),
            amount: 0.0,
        };
        if score.total >= tol.found_threshold && score.contact {
            return found;
        }
        // the errors against the reference pose
        let slide = r.skin_mm - pose.skin_mm;
        let rot = wrap_deg(r.rotation_deg - pose.rotation_deg);
        let dtilt = r.tilt_deg - pose.tilt_deg;
        let drock = r.rock_deg - pose.rock_deg;
        let ddepth = r.depth_mm - pose.depth_mm;
        let slide_hint = |kind| Hint {
            kind,
            movement: Movement::Slide,
            direction: Direction::Patient(PatientDir::of(slide)),
            amount: slide.length(),
        };
        let angle_dir = |p: &ProbePose| Direction::Patient(PatientDir::of(tip_shift(anatomy, &pose, p)));
        let angle_hint = |kind| {
            let both = ProbePose {
                tilt_deg: r.tilt_deg,
                rock_deg: r.rock_deg,
                ..pose
            };
            Hint {
                kind,
                movement: Movement::Angle,
                direction: angle_dir(&both),
                amount: dtilt.hypot(drock),
            }
        };
        if !score.contact {
            return angle_hint(HintKind::LostContact);
        }
        let frame = pose.frame(anatomy.designed.contact_limit_deg);
        let (bone, lung) = beam_occlusion(anatomy, frame.apex_mm, frame.beam, pose.depth_mm);
        // more than the reference pose's own line crosses (a window can lie partly behind a rib)
        let rf = r.frame(anatomy.designed.contact_limit_deg);
        let (ref_bone, ref_lung) = beam_occlusion(anatomy, rf.apex_mm, rf.beam, r.depth_mm);
        let (bone, lung) = (bone - ref_bone, lung - ref_lung);
        // (a line blocked while the skin point is already right is an angle error, not a rib)
        let moves = slide.length() >= 0.5 * tol.hint_slide_mm;
        if moves && bone >= tol.rib_bone_mm {
            // "slide up or down one space": the cranial or caudal part of the way to the reference
            let dir = if slide.y >= 0.0 {
                PatientDir::Head
            } else {
                PatientDir::Foot
            };
            return Hint {
                kind: HintKind::OverRib,
                movement: Movement::Slide,
                direction: Direction::Patient(dir),
                amount: slide.length(),
            };
        }
        if moves && lung >= tol.lung_mm {
            return slide_hint(HintKind::OverLung);
        }
        if score.marker_error_deg >= tol.wrong_marker_deg {
            return Hint {
                kind: HintKind::WrongMarkerSide,
                movement: Movement::Rotate,
                direction: if rot >= 0.0 {
                    Direction::Clockwise
                } else {
                    Direction::CounterClockwise
                },
                amount: rot.abs(),
            };
        }
        if moves
            && self.target(id).apical.is_some()
            && score.foreshortening < tol.foreshortened_below
            && score.plane > 0.5
            && score.marker > 0.5
        {
            return slide_hint(HintKind::Foreshortened);
        }
        // the largest correctable movement, in units of its tolerance
        let n_slide = slide.length() / tol.hint_slide_mm;
        if n_slide >= 1.0 {
            return slide_hint(HintKind::Move);
        }
        let n_rot = rot.abs() / tol.hint_rotate_deg;
        let n_tilt = dtilt.abs() / tol.hint_tilt_deg;
        let n_rock = drock.abs() / tol.hint_rock_deg;
        let n_depth = ddepth.abs() / tol.hint_depth_mm;
        let n_angle = if n_tilt >= 1.0 && n_rock >= 1.0 {
            n_tilt.hypot(n_rock)
        } else {
            0.0
        };
        let mut best = (n_slide, Movement::Slide);
        for (n, m) in [
            (n_rot, Movement::Rotate),
            (n_tilt, Movement::Tilt),
            (n_rock, Movement::Rock),
            (n_angle, Movement::Angle),
            (n_depth, Movement::Depth),
        ] {
            if n > best.0 + 1e-6 {
                best = (n, m);
            }
        }
        match best.1 {
            Movement::Slide => slide_hint(HintKind::Move),
            Movement::Rotate => Hint {
                kind: HintKind::Move,
                movement: Movement::Rotate,
                direction: if rot >= 0.0 {
                    Direction::Clockwise
                } else {
                    Direction::CounterClockwise
                },
                amount: rot.abs(),
            },
            Movement::Tilt => Hint {
                kind: HintKind::Move,
                movement: Movement::Tilt,
                direction: angle_dir(&ProbePose {
                    tilt_deg: r.tilt_deg,
                    ..pose
                }),
                amount: dtilt.abs(),
            },
            Movement::Rock => Hint {
                kind: HintKind::Move,
                movement: Movement::Rock,
                direction: angle_dir(&ProbePose {
                    rock_deg: r.rock_deg,
                    ..pose
                }),
                amount: drock.abs(),
            },
            Movement::Angle => {
                let mut h = angle_hint(HintKind::Move);
                h.movement = Movement::Angle;
                h
            }
            Movement::Depth => Hint {
                kind: HintKind::Move,
                movement: Movement::Depth,
                direction: if ddepth >= 0.0 {
                    Direction::Deeper
                } else {
                    Direction::Shallower
                },
                amount: ddepth.abs(),
            },
        }
    }
}
