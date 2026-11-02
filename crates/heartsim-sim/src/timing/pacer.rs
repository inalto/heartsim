//! Artificial pacemaker timing cycles (NBG modes, Bernstein et al. 2002).
//!
//! The device is a pair of timers that *sense* chamber activations reported by the timing engine
//! and *request* stimuli. Capture (does the stimulus activate tissue?) is decided by the engine,
//! which knows the chambers' refractory state and the lead thresholds; the device records its
//! own paces regardless, as a real generator does.
//!
//! Cycles implemented:
//! * LRI — lower rate interval, measured from the last ventricular (or atrial, in AAI/AOO) event;
//! * AEI = LRI − AVD — atrial escape interval of dual-chamber modes;
//! * AVD — AV delay started by a paced or (DDD) sensed atrial event;
//! * URI — upper rate interval: a tracked ventricular pace is never delivered earlier than URI
//!   after the previous ventricular event (pacemaker Wenckebach above the upper rate);
//! * PVARP — atrial events within this window after a ventricular event are not tracked
//!   (extended once by the PMT intervention);
//! * VRP — ventricular signals within this window after a ventricular event are not counted;
//! * blanking — nothing is sensed for `blanking_ms` after any stimulus.
//!
//! Sensing is amplitude-gated: an intrinsic event counts only if the lead's electrogram
//! amplitude reaches the programmed sensitivity (undersensing otherwise), and the T wave of
//! every ventricular beat is offered to the ventricular channel ([`T_WAVE_DELAY_MS`] later), so
//! a sensitivity below the T-wave amplitude oversenses it.
//!
//! Every sensed or paced event is logged as a [`PacerMarker`] — the device's marker channel and
//! the source of the strip's spikes and the panel's LEDs.

use super::params::{PMT_DETECT_CYCLES, PacerChannel, PacerConfig};
use crate::engine::{Chamber, Event, EventKind, Origin};
use heartsim_core::Millis;
use serde::{Deserialize, Serialize};

/// Delay of the T wave on a ventricular electrogram after the ventricular event, ms (the
/// intracardiac T peaks ≈ 250–350 ms after the R; Ellenbogen, *Clinical Cardiac Pacing*).
pub const T_WAVE_DELAY_MS: f64 = 300.0;

/// Stimuli the device wants delivered now.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PaceRequest {
    pub atrium: bool,
    /// The (right) ventricular lead.
    pub ventricle: bool,
    /// The LV lead of a biventricular device.
    pub lv: bool,
}

/// What the device did, as on its marker channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MarkerKind {
    /// Stimulus delivered (AP / VP); `captured` = the tissue at the lead depolarised.
    Pace { captured: bool },
    /// Intrinsic (or oversensed) signal counted (AS / VS): inhibits or triggers.
    Sense,
    /// Signal seen inside a refractory period (AR / VR): logged, not acted upon.
    RefractorySense,
}

/// One marker-channel entry. Pace markers are the stimulus sites of the engine state: the
/// strip draws a spike at every one, the heart view flashes the lead.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PacerMarker {
    pub at: Millis,
    pub channel: PacerChannel,
    pub kind: MarkerKind,
}

impl PacerMarker {
    /// Conventional two-letter label: `AP`, `VS`, `AR`, …
    #[must_use]
    pub fn label(&self) -> String {
        let k = match self.kind {
            MarkerKind::Pace { .. } => 'P',
            MarkerKind::Sense => 'S',
            MarkerKind::RefractorySense => 'R',
        };
        format!("{}{k}", self.channel.letter())
    }

    /// `true` for a stimulus (captured or not).
    #[must_use]
    pub fn is_pace(&self) -> bool {
        matches!(self.kind, MarkerKind::Pace { .. })
    }
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
    /// T wave of the last captured/sensed ventricular beat, offered to the V channel.
    t_wave_at: Option<f64>,
    /// Second ventricular lead still to pace (biventricular VV offset): time and channel.
    second_v: Option<(f64, PacerChannel)>,
    /// Consecutive tracked ventricular paces at the upper rate (PMT detection).
    upper_rate_run: u32,
    /// PVARP after the ventricular event at this time is the PMT-intervention PVARP.
    extended_pvarp_after: Option<f64>,
}

impl Pacer {
    /// Reset the timers so the first pace falls one interval after `t`.
    pub fn reset(&mut self, t: f64) {
        *self = Self {
            last_a: t,
            last_v: t,
            blanking_until: f64::NEG_INFINITY,
            ..Self::default()
        };
    }

    /// The engine delivered a stimulus on `channel` at `t` and it did (not) capture: a captured
    /// ventricular pace has a T wave.
    pub fn paced(&mut self, channel: PacerChannel, t: f64, captured: bool) {
        if captured && channel != PacerChannel::Atrial {
            self.t_wave_at = Some(t + T_WAVE_DELAY_MS);
        }
    }

    /// Feed the events of the current step and decide whether to pace at `now`.
    /// `amplitude_mv[channel]` is the electrogram amplitude each lead sees for an intrinsic
    /// event; sense markers are appended to `markers` (pace markers are the engine's, which
    /// knows about capture).
    pub fn update(
        &mut self,
        cfg: &PacerConfig,
        now: f64,
        events: &[Event],
        amplitude_mv: [f64; 3],
        markers: &mut Vec<PacerMarker>,
    ) -> PaceRequest {
        let mode = cfg.effective_mode();
        let a_amp = amplitude_mv[PacerChannel::Atrial as usize];
        let v_amp = amplitude_mv[PacerChannel::Ventricular as usize];
        let v_sens = cfg.ventricular_sensitivity_mv;

        // T-wave of the previous ventricular beat, offered to the ventricular channel first.
        if let Some(tw) = self.t_wave_at
            && tw <= now
        {
            self.t_wave_at = None;
            let t_amp = cfg.ventricular_lead.t_wave();
            if mode.senses_ventricle() && t_amp >= v_sens && tw >= self.blanking_until {
                self.sense_ventricle(cfg, tw, false, markers);
            }
        }

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
            let t = ev.at.0;
            match ev.chamber {
                Chamber::Atrium if mode.senses_atrium() => {
                    if a_amp < cfg.atrial_sensitivity_mv {
                        continue; // undersensed
                    }
                    let pvarp = if self.extended_pvarp_after == Some(self.last_v) {
                        cfg.pmt_pvarp_ms
                    } else {
                        cfg.pvarp_ms
                    };
                    if t - self.last_v < pvarp {
                        // inside PVARP: logged, not tracked (retrograde P protection)
                        push(markers, t, PacerChannel::Atrial, MarkerKind::RefractorySense);
                        continue;
                    }
                    push(markers, t, PacerChannel::Atrial, MarkerKind::Sense);
                    self.last_a = t;
                    self.atrial_since_v = true;
                    if mode.tracks_atrium() && self.av_deadline.is_none() {
                        self.av_deadline = Some((t + cfg.av_delay_ms, true));
                    }
                }
                Chamber::Ventricle if mode.senses_ventricle() => {
                    if v_amp < v_sens {
                        continue; // undersensed
                    }
                    self.sense_ventricle(cfg, t, true, markers);
                }
                _ => {}
            }
        }

        let mut req = PaceRequest::default();
        if cfg.paused {
            return req;
        }
        let lri = 60_000.0 / cfg.lower_rate_bpm.max(1.0);
        let uri = 60_000.0 / cfg.upper_rate_bpm.max(1.0);

        // --- second ventricular lead (biventricular VV offset) ------------------------------
        if let Some((due, ch)) = self.second_v
            && now >= due
        {
            self.second_v = None;
            set(&mut req, ch);
        }

        // --- ventricular channel ------------------------------------------------------------
        if mode.paces_ventricle() {
            let due = match self.av_deadline {
                Some((deadline, tracked)) => now >= deadline && (!tracked || now - self.last_v >= uri),
                None => now - self.last_v >= lri,
            };
            if due {
                let tracked_at_upper_rate = matches!(self.av_deadline, Some((d, true)) if now - d > 0.5);
                if tracked_at_upper_rate {
                    self.upper_rate_run += 1;
                } else {
                    self.upper_rate_run = 0;
                }
                self.last_v = now;
                if cfg.pmt_intervention && self.upper_rate_run >= PMT_DETECT_CYCLES {
                    self.extended_pvarp_after = Some(now);
                    self.upper_rate_run = 0;
                }
                self.av_deadline = None;
                self.atrial_since_v = false;
                self.t_wave_at = None;
                self.blanking_until = now + cfg.blanking_ms;
                self.pace_ventricles(cfg, now, &mut req);
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

    /// A ventricular-channel signal at `t` (intrinsic beat or oversensed T wave).
    fn sense_ventricle(&mut self, cfg: &PacerConfig, t: f64, beat: bool, markers: &mut Vec<PacerMarker>) {
        if t - self.last_v < cfg.vrp_ms {
            push(markers, t, PacerChannel::Ventricular, MarkerKind::RefractorySense);
            return;
        }
        push(markers, t, PacerChannel::Ventricular, MarkerKind::Sense);
        self.last_v = t;
        self.av_deadline = None;
        self.second_v = None;
        self.atrial_since_v = false;
        self.upper_rate_run = 0;
        self.t_wave_at = beat.then_some(t + T_WAVE_DELAY_MS);
    }

    /// Ventricular pace now: the RV lead, and the LV lead now or after the VV offset.
    fn pace_ventricles(&mut self, cfg: &PacerConfig, now: f64, req: &mut PaceRequest) {
        if cfg.lv_lead.is_none() {
            req.ventricle = true;
            return;
        }
        let vv = cfg.vv_delay_ms;
        let (first, second) = if vv < 0.0 {
            (PacerChannel::LeftVentricular, PacerChannel::Ventricular)
        } else {
            (PacerChannel::Ventricular, PacerChannel::LeftVentricular)
        };
        set(req, first);
        if vv.abs() < 0.5 {
            set(req, second);
        } else {
            self.second_v = Some((now + vv.abs(), second));
        }
    }
}

fn set(req: &mut PaceRequest, ch: PacerChannel) {
    match ch {
        PacerChannel::Atrial => req.atrium = true,
        PacerChannel::Ventricular => req.ventricle = true,
        PacerChannel::LeftVentricular => req.lv = true,
    }
}

fn push(markers: &mut Vec<PacerMarker>, t: f64, channel: PacerChannel, kind: MarkerKind) {
    markers.push(PacerMarker {
        at: Millis(t),
        channel,
        kind,
    });
}
