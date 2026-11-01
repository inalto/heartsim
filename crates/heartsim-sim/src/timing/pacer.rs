//! Artificial pacemaker timing cycles (NBG modes, Bernstein et al. 2002).
//!
//! The device is a pair of timers that *sense* chamber activations reported by the timing engine
//! and *request* stimuli. Capture (does the stimulus activate tissue?) is decided by the engine,
//! which knows the chambers' refractory state; the device records its own paces regardless, as a
//! real generator does.
//!
//! Cycles implemented:
//! * LRI — lower rate interval, measured from the last ventricular (or atrial, in AAI/AOO) event;
//! * AEI = LRI − AVD — atrial escape interval of dual-chamber modes;
//! * AVD — AV delay started by a paced or (DDD) sensed atrial event;
//! * URI — upper rate interval: a tracked ventricular pace is never delivered earlier than URI
//!   after the previous ventricular event (pacemaker Wenckebach above the upper rate);
//! * PVARP — atrial events within this window after a ventricular event are not sensed;
//! * blanking — nothing is sensed for `blanking_ms` after any stimulus.

use super::params::PacerConfig;
use crate::engine::{Chamber, Event, EventKind, Origin};

/// Stimuli the device wants delivered now.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PaceRequest {
    pub atrium: bool,
    pub ventricle: bool,
}

/// Device state. Times are absolute simulation milliseconds.
#[derive(Debug, Clone, Default)]
pub struct Pacer {
    /// Last atrial event (paced or sensed).
    last_a: f64,
    /// Last ventricular event (paced or sensed).
    last_v: f64,
    /// Running AV delay: deadline for the ventricular pace, and whether a sensed P started it
    /// (tracked paces obey the upper rate).
    av_deadline: Option<(f64, bool)>,
    /// Nothing is sensed before this time.
    blanking_until: f64,
    /// An atrial event has occurred since `last_v` (inhibits the atrial escape pace).
    atrial_since_v: bool,
}

impl Pacer {
    /// Reset the timers so the first pace falls one interval after `t`.
    pub fn reset(&mut self, t: f64) {
        self.last_a = t;
        self.last_v = t;
        self.av_deadline = None;
        self.blanking_until = f64::NEG_INFINITY;
        self.atrial_since_v = false;
    }

    /// Feed the events of the current step and decide whether to pace at `now`.
    pub fn update(&mut self, cfg: &PacerConfig, now: f64, events: &[Event]) -> PaceRequest {
        let mode = cfg.mode;
        for ev in events {
            // Own stimuli are already recorded as paces; conducted events they cause (e.g. a
            // retrograde P after a paced QRS) are genuine intrinsic activity and are sensed.
            let own_stimulus = ev.kind == EventKind::Stimulus && matches!(ev.origin, Origin::Pacer(_));
            if ev.kind == EventKind::Blocked || own_stimulus {
                continue;
            }
            if ev.at.0 < self.blanking_until {
                continue;
            }
            match ev.chamber {
                Chamber::Atrium if mode.senses_atrium() => {
                    if ev.at.0 - self.last_v < cfg.pvarp_ms {
                        continue; // inside PVARP: not sensed (retrograde P protection)
                    }
                    self.last_a = ev.at.0;
                    self.atrial_since_v = true;
                    if mode.tracks_atrium() && self.av_deadline.is_none() {
                        self.av_deadline = Some((ev.at.0 + cfg.av_delay_ms, true));
                    }
                }
                Chamber::Ventricle if mode.senses_ventricle() => {
                    self.last_v = ev.at.0;
                    self.av_deadline = None;
                    self.atrial_since_v = false;
                }
                _ => {}
            }
        }

        let lri = 60_000.0 / cfg.lower_rate_bpm;
        let uri = 60_000.0 / cfg.upper_rate_bpm;
        let mut req = PaceRequest::default();

        // --- ventricular channel ------------------------------------------------------------
        if mode.paces_ventricle() {
            let due = match self.av_deadline {
                Some((deadline, tracked)) => now >= deadline && (!tracked || now - self.last_v >= uri),
                None => now - self.last_v >= lri,
            };
            if due {
                req.ventricle = true;
                self.last_v = now;
                self.av_deadline = None;
                self.atrial_since_v = false;
                self.blanking_until = now + cfg.blanking_ms;
            }
        }

        // --- atrial channel -----------------------------------------------------------------
        if mode.paces_atrium() {
            let due = if mode.paces_ventricle() {
                // dual chamber: atrial escape interval from the last ventricular event
                !self.atrial_since_v
                    && self.av_deadline.is_none()
                    && now - self.last_v >= lri - cfg.av_delay_ms
            } else {
                now - self.last_a >= lri
            };
            if due {
                req.atrium = true;
                self.last_a = now;
                self.atrial_since_v = true;
                if mode.paces_ventricle() {
                    self.av_deadline = Some((now + cfg.av_delay_ms, false));
                }
                self.blanking_until = now + cfg.blanking_ms;
            }
        }
        req
    }
}
