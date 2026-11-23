//! Cardiac coordinates, computed once from the anatomy pack's landmarks, valve discs and tags.
//!
//! * **Ventricles.** Each has a long axis from its apex landmark (`lv_apex`, `rv_apex`) to the
//!   centre of its inflow valve disc (mitral, tricuspid). A point's *height* is its position
//!   along that axis (0 at the apex, 1 at the valve plane), its *radial direction* the unit
//!   vector from the axis to it, perpendicular to the axis, and its *radius* the distance to the
//!   axis. The transmural depth is the pack's (0 endocardium … 1 epicardium; the septum measured
//!   from the LV side).
//! * **Atria.** Each has a centre (the centroid of its cells) and an AV-plane normal, the unit
//!   vector from its AV valve's centre to that centroid; height runs from 0 at the valve plane to
//!   1 at the atrial cell furthest along the normal.
//!
//! Every cell belongs to exactly one [`MotionChamber`]: the working myocardium by its region, and
//! everything else (conduction system, septal and junctional tissue, scar) by the nearest working
//! myocardial cell.

use crate::valves::ValveDisc;
use glam::Vec3;
use heartsim_core::Region;
use heartsim_ecg::Valve;
use heartsim_geometry::{CellCloud, HashGrid, Landmarks};

/// The four chambers the motion model moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MotionChamber {
    LeftVentricle,
    RightVentricle,
    LeftAtrium,
    RightAtrium,
}

impl MotionChamber {
    /// Every chamber, in index order.
    pub const ALL: [MotionChamber; 4] = [
        MotionChamber::LeftVentricle,
        MotionChamber::RightVentricle,
        MotionChamber::LeftAtrium,
        MotionChamber::RightAtrium,
    ];

    /// Index in [`Self::ALL`].
    #[must_use]
    pub fn index(self) -> usize {
        self as usize
    }

    /// `true` for the two ventricles.
    #[must_use]
    pub fn is_ventricle(self) -> bool {
        matches!(self, MotionChamber::LeftVentricle | MotionChamber::RightVentricle)
    }

    /// The chamber a working-myocardium region belongs to (`None` for conduction tissue, the
    /// atrial septum, Bachmann's bundle, the coronary sinus and pathological tags, which take the
    /// chamber of their nearest working cell).
    #[must_use]
    pub fn of_region(region: Region) -> Option<Self> {
        match region {
            Region::LeftVentricle | Region::VentricularSeptum => Some(MotionChamber::LeftVentricle),
            Region::RightVentricle => Some(MotionChamber::RightVentricle),
            Region::LeftAtrium | Region::LeftAtrialAppendage | Region::PulmonaryVeinSleeves => {
                Some(MotionChamber::LeftAtrium)
            }
            Region::RightAtrium
            | Region::CristaTerminalis
            | Region::CavotricuspidIsthmus
            | Region::SinoatrialNode => Some(MotionChamber::RightAtrium),
            _ => None,
        }
    }
}

/// The axis of one chamber: `origin` (ventricle: apex; atrium: AV valve centre), the unit `axis`
/// from it (ventricle: toward the inflow valve; atrium: into the atrium) and the `length_mm` that
/// height 1 stands for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChamberAxis {
    pub origin_mm: Vec3,
    pub axis: Vec3,
    pub length_mm: f32,
    /// Ventricle: centre of the inflow valve; atrium: centroid of its cells.
    pub centre_mm: Vec3,
}

/// Coordinates of a point relative to a [`ChamberAxis`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxisCoords {
    /// Position along the axis over its length (not clamped).
    pub height: f32,
    /// Unit vector from the axis to the point, perpendicular to the axis (zero on the axis).
    pub radial: Vec3,
    /// Distance from the axis, mm.
    pub radius_mm: f32,
}

impl ChamberAxis {
    /// The coordinates of `p` about this axis.
    #[must_use]
    pub fn coords(&self, p: Vec3) -> AxisCoords {
        let d = p - self.origin_mm;
        let along = d.dot(self.axis);
        let perp = d - self.axis * along;
        let radius_mm = perp.length();
        AxisCoords {
            height: along / self.length_mm,
            radial: if radius_mm > 1e-6 {
                perp / radius_mm
            } else {
                Vec3::ZERO
            },
            radius_mm,
        }
    }

    /// The point of the axis at `height`.
    #[must_use]
    pub fn point_at(&self, height: f32) -> Vec3 {
        self.origin_mm + self.axis * (height * self.length_mm)
    }
}

/// The heart's cardiac frame: one axis per chamber and the four valve centres.
#[derive(Debug, Clone, PartialEq)]
pub struct CardiacFrame {
    /// Indexed by [`MotionChamber::index`].
    pub chambers: [ChamberAxis; 4],
    /// Valve centres, indexed by `Valve::index`.
    pub valve_centres_mm: [Vec3; 4],
}

impl CardiacFrame {
    /// The axis of `chamber`.
    #[must_use]
    pub fn axis(&self, chamber: MotionChamber) -> &ChamberAxis {
        &self.chambers[chamber.index()]
    }
}

/// Per-cell cardiac coordinates (see the module docs), one entry per cell of the cloud.
#[derive(Debug, Clone, PartialEq)]
pub struct CellCoords {
    pub chamber: Vec<MotionChamber>,
    /// Height in the cell's own chamber (ventricles 0 apex … 1 valve plane; atria 0 valve plane
    /// … 1 roof); not clamped.
    pub height: Vec<f32>,
    /// Unit radial direction in the cell's own chamber.
    pub radial: Vec<Vec3>,
    /// Distance from the chamber's axis, mm.
    pub radius_mm: Vec<f32>,
    /// Transmural depth, 0 endocardium … 1 epicardium (the pack's).
    pub depth: Vec<f32>,
}

/// Why the cardiac frame could not be built from a pack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameError {
    /// A landmark the frame needs is missing.
    MissingLandmark(&'static str),
    /// The pack has no `Valve` mesh with exactly four discs.
    NoValveDiscs,
    /// A chamber has no cells.
    EmptyChamber(MotionChamber),
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FrameError::MissingLandmark(n) => write!(f, "landmark `{n}` missing"),
            FrameError::NoValveDiscs => write!(f, "no valve-plane mesh with four discs"),
            FrameError::EmptyChamber(c) => write!(f, "chamber {c:?} has no cells"),
        }
    }
}

impl std::error::Error for FrameError {}

/// For every cell, the working-myocardium cell it moves with: itself, or for conduction tissue,
/// junctional tissue and pathological tags the nearest working cell (they have no contraction of
/// their own in this model).
#[must_use]
pub fn working_proxies(cells: &CellCloud) -> Vec<u32> {
    let own: Vec<bool> = cells
        .regions
        .iter()
        .map(|r| MotionChamber::of_region(*r).is_some())
        .collect();
    let pure_idx: Vec<u32> = (0..cells.len() as u32).filter(|&i| own[i as usize]).collect();
    let pure_pos: Vec<Vec3> = pure_idx.iter().map(|&i| cells.positions[i as usize]).collect();
    let grid = HashGrid::with_auto_cell(&pure_pos);
    (0..cells.len())
        .map(|i| {
            if own[i] {
                i as u32
            } else {
                grid.nearest(cells.positions[i])
                    .map_or(i as u32, |(_, j)| pure_idx[j as usize])
            }
        })
        .collect()
}

/// The chamber of every cell: its region's, else its nearest working cell's
/// ([`working_proxies`]).
#[must_use]
pub fn cell_chambers(cells: &CellCloud, proxies: &[u32]) -> Vec<MotionChamber> {
    proxies
        .iter()
        .map(|&j| MotionChamber::of_region(cells.regions[j as usize]).unwrap_or(MotionChamber::LeftVentricle))
        .collect()
}

/// Build the frame from the cells, their chambers, the landmarks and the valve discs.
pub fn cardiac_frame(
    cells: &CellCloud,
    chambers: &[MotionChamber],
    landmarks: &Landmarks,
    discs: &[ValveDisc],
) -> Result<CardiacFrame, FrameError> {
    let landmark = |name: &'static str| {
        landmarks
            .points
            .get(name)
            .and_then(|&i| cells.positions.get(i as usize).copied())
            .ok_or(FrameError::MissingLandmark(name))
    };
    let mut valve_centres_mm = [Vec3::ZERO; 4];
    for v in Valve::ALL {
        valve_centres_mm[v.index()] = discs
            .iter()
            .find(|d| d.valve == v)
            .ok_or(FrameError::NoValveDiscs)?
            .centre_mm;
    }
    let centroid = |ch: MotionChamber| {
        let (mut s, mut n) = (Vec3::ZERO, 0usize);
        for (p, c) in cells.positions.iter().zip(chambers) {
            if *c == ch {
                s += *p;
                n += 1;
            }
        }
        (n > 0).then(|| s / n as f32).ok_or(FrameError::EmptyChamber(ch))
    };
    let ventricle = |apex: Vec3, valve: Valve| {
        let base = valve_centres_mm[valve.index()];
        let d = base - apex;
        ChamberAxis {
            origin_mm: apex,
            axis: d.normalize_or_zero(),
            length_mm: d.length().max(1.0),
            centre_mm: base,
        }
    };
    let atrium = |ch: MotionChamber, valve: Valve| -> Result<ChamberAxis, FrameError> {
        let c = centroid(ch)?;
        let origin = valve_centres_mm[valve.index()];
        let axis = (c - origin).normalize_or_zero();
        let mut top = 1.0f32;
        for (p, k) in cells.positions.iter().zip(chambers) {
            if *k == ch {
                top = top.max((*p - origin).dot(axis));
            }
        }
        Ok(ChamberAxis {
            origin_mm: origin,
            axis,
            length_mm: top,
            centre_mm: c,
        })
    };
    Ok(CardiacFrame {
        chambers: [
            ventricle(landmark("lv_apex")?, Valve::Mitral),
            ventricle(landmark("rv_apex")?, Valve::Tricuspid),
            atrium(MotionChamber::LeftAtrium, Valve::Mitral)?,
            atrium(MotionChamber::RightAtrium, Valve::Tricuspid)?,
        ],
        valve_centres_mm,
    })
}

impl CellCoords {
    /// The coordinates of every cell in its own chamber.
    #[must_use]
    pub fn new(cells: &CellCloud, chambers: Vec<MotionChamber>, frame: &CardiacFrame) -> Self {
        let n = cells.len();
        let mut height = Vec::with_capacity(n);
        let mut radial = Vec::with_capacity(n);
        let mut radius_mm = Vec::with_capacity(n);
        for (p, ch) in cells.positions.iter().zip(&chambers) {
            let c = frame.axis(*ch).coords(*p);
            height.push(c.height);
            radial.push(c.radial);
            radius_mm.push(c.radius_mm);
        }
        Self {
            chamber: chambers,
            height,
            radial,
            radius_mm,
            depth: cells.depth.clone(),
        }
    }
}
