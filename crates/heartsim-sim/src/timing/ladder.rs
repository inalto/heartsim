//! Ladder-diagram (laddergram) samples.
//!
//! A ladder diagram has three horizontal rows — atrium, AV junction, ventricle — and draws every
//! impulse as a line whose *slope* is its conduction time across the row. The timing engine emits
//! one [`LadderSample`] per row traversal; the UI maps `enter_ms` to one edge of the row and
//! `exit_ms` to the other (top → bottom for antegrade, bottom → top for retrograde) and ends a
//! blocked line with a bar at `exit_ms`.

use crate::engine::{Chamber, Origin};
use serde::{Deserialize, Serialize};

/// Direction of travel through the row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LadderDirection {
    /// Towards the ventricles (top → bottom of the diagram).
    Antegrade,
    /// Towards the atria (bottom → top).
    Retrograde,
}

/// One line segment of the ladder diagram.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LadderSample {
    /// Row of the diagram.
    pub chamber: Chamber,
    /// Time the impulse enters the row, ms.
    pub enter_ms: f64,
    /// Time it leaves the row (or, if `blocked`, where the block bar is drawn), ms.
    /// `exit_ms − enter_ms` is the conduction time, i.e. the slope.
    pub exit_ms: f64,
    pub direction: LadderDirection,
    /// The impulse died inside this row.
    pub blocked: bool,
    /// Source of the impulse (colour key).
    pub origin: Origin,
}

impl LadderSample {
    #[must_use]
    pub fn antegrade(chamber: Chamber, enter_ms: f64, exit_ms: f64, origin: Origin) -> Self {
        Self {
            chamber,
            enter_ms,
            exit_ms,
            direction: LadderDirection::Antegrade,
            blocked: false,
            origin,
        }
    }

    #[must_use]
    pub fn retrograde(chamber: Chamber, enter_ms: f64, exit_ms: f64, origin: Origin) -> Self {
        Self {
            chamber,
            enter_ms,
            exit_ms,
            direction: LadderDirection::Retrograde,
            blocked: false,
            origin,
        }
    }

    #[must_use]
    pub fn blocked(mut self) -> Self {
        self.blocked = true;
        self
    }

    /// Conduction time across the row, ms.
    #[must_use]
    pub fn duration_ms(&self) -> f64 {
        self.exit_ms - self.enter_ms
    }
}
