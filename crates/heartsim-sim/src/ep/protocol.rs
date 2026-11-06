//! Programmed electrical stimulation: what to deliver and when, and what came of it.
//!
//! A [`ProtocolRun`] is a pure state machine. The lab asks it for the stimuli due in the next step
//! and shows it the timing events and the activation time of the paced cell; it never touches the
//! engine, so a protocol is deterministic and can be tested without one.
//!
//! **ERP search** ([`Protocol::ErpSearch`]). Drive S1×n, then S2 at the current coupling; observe,
//! rest one drive cycle, repeat 10 ms shorter. The target's effective refractory period is the
//! longest coupling at which it fails to conduct (Josephson): the local chamber does not capture
//! ([`ErpTarget::Atrium`], [`ErpTarget::Ventricle`]), or the atrium does and the impulse goes no
//! further ([`ErpTarget::AvNode`]), leaves the fast pathway ([`ErpTarget::FastPathway`]) or does
//! not use the accessory pathway ([`ErpTarget::AccessoryPathway`]). The answer comes from the timing
//! events, which are what the strip and the mesh are driven by, so it is a measurement of the
//! engine and not a parameter read back.

use crate::engine::{Event, EventKind, Origin};
use heartsim_core::Region;
use serde::{Deserialize, Serialize};

/// How long after S2 its consequences are collected, ms. The longest route is a long-ERP fast
/// pathway blocking and the slow pathway conducting (≈ 400 ms A2–H2).
const OBSERVE_MS: f64 = 500.0;
/// Minimum time from a stimulus to the next activation counted as the return beat, ms: the paced
/// cell's own activation by that stimulus comes first.
const RETURN_BLANK_MS: f64 = 60.0;
/// Activations watched at the pacing site to measure the tachycardia cycle length.
const CYCLES_OBSERVED: usize = 4;
/// Give up waiting for a tachycardia or a return beat after this long, ms.
const WATCH_LIMIT_MS: f64 = 6000.0;
/// A jump of at least this in A2–H2 for one coupling step means dual AV-nodal pathways, ms.
pub const AH_JUMP_MS: f64 = 50.0;
/// PPI − TCL up to this means the pacing site is in the circuit, ms.
pub const IN_CIRCUIT_MS: f64 = 30.0;

/// What refractory period is being looked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErpTarget {
    Atrium,
    Ventricle,
    /// AV node as a whole: the atrium captures, the His bundle stays silent.
    AvNode,
    /// Fast pathway of a dual node: conduction jumps to the slow pathway (or stops).
    FastPathway,
    AccessoryPathway,
}

/// A stimulation protocol from one catheter pair.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Protocol {
    /// Pace until stopped.
    Continuous { cycle_ms: f64 },
    /// S1 × `count`, then optional S2 and S3 coupled to the previous stimulus.
    Train {
        s1_cycle_ms: f64,
        s1_count: u32,
        s2_ms: Option<f64>,
        s3_ms: Option<f64>,
    },
    /// Trains of `beats` stimuli whose cycle shortens by `step_ms` from `start_cycle_ms` down to
    /// `end_cycle_ms`: the Wenckebach point, atrial capture limit, induction by rate.
    Decremental {
        start_cycle_ms: f64,
        end_cycle_ms: f64,
        step_ms: f64,
        beats: u32,
    },
    /// See the module documentation.
    ErpSearch {
        target: ErpTarget,
        s1_cycle_ms: f64,
        s1_count: u32,
        s2_start_ms: f64,
        s2_step_ms: f64,
        s2_floor_ms: f64,
    },
    /// Measure the tachycardia cycle length at the pacing site, pace `offset_ms` faster for
    /// `count` stimuli and time the first return beat at the site.
    Entrainment { offset_ms: f64, count: u32 },
}

impl Protocol {
    /// S1×8 at 600 ms, S2 from 400 ms in 10 ms steps: the usual search.
    #[must_use]
    pub fn erp_search(target: ErpTarget) -> Self {
        Protocol::ErpSearch {
            target,
            s1_cycle_ms: 600.0,
            s1_count: 8,
            s2_start_ms: 400.0,
            s2_step_ms: 10.0,
            s2_floor_ms: 150.0,
        }
    }
}

/// What one S1–S2 coupling did.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CouplingPoint {
    /// S1–S2 interval, ms.
    pub s1s2_ms: f64,
    /// The chamber under the pair captured S2.
    pub captured: bool,
    /// The His bundle fired from the impulse.
    pub his: bool,
    /// Nodal-input to His time of the impulse (A2–H2), ms.
    pub a2h2_ms: Option<f64>,
    /// It travelled the slow pathway.
    pub slow_pathway: bool,
    /// The accessory pathway refused it.
    pub ap_blocked: bool,
}

/// Result of an ERP search.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErpResult {
    pub target: ErpTarget,
    /// Longest coupling that failed, ms. `None`: conduction held down to the floor, or the
    /// tissue around the pair went refractory first.
    pub erp_ms: Option<f64>,
    /// The stimulated chamber's own ERP ended the search before the target failed.
    pub limited_by_local_erp: bool,
    /// Coupling at which A2–H2 jumped by ≥ [`AH_JUMP_MS`], and the jump, ms.
    pub ah_jump: Option<(f64, f64)>,
    /// Every coupling tried, longest first.
    pub curve: Vec<CouplingPoint>,
}

/// Result of an entrainment.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EntrainmentResult {
    /// Tachycardia cycle length measured at the site before pacing, ms.
    pub tcl_ms: f64,
    pub pacing_cycle_ms: f64,
    /// Post-pacing interval: last stimulus to the next activation at the site, ms.
    pub ppi_ms: f64,
    /// PPI − TCL ≤ [`IN_CIRCUIT_MS`].
    pub in_circuit: bool,
}

/// How a protocol ended.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ProtocolOutcome {
    /// The stimuli were delivered.
    Delivered { stimuli: u32 },
    Erp(ErpResult),
    Entrainment(EntrainmentResult),
    /// Stimulus and return beats could not be timed: no rhythm to entrain, no return beat.
    NoReturn,
}

/// Progress, for the panel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProtocolStatus {
    /// Stimuli delivered so far and the current coupling or cycle, ms.
    Pacing { delivered: u32, interval_ms: f64 },
    /// Watching for the tachycardia cycle or the return beat.
    Watching,
    Finished,
}

#[derive(Debug, Clone, PartialEq)]
enum Phase {
    /// Planned stimuli, ascending; `next` is the first not delivered.
    Plan { times: Vec<f64>, next: usize },
    /// Continuous pacing: next stimulus time.
    Continuous { next_ms: f64, cycle_ms: f64 },
    /// One drive of an ERP search.
    Drive {
        times: Vec<f64>,
        next: usize,
        s2_ms: f64,
        s2_at: f64,
        point: CouplingPoint,
        his_seen: bool,
        input_at: Option<f64>,
    },
    /// Entrainment: collecting activations of the site.
    Measure { since_ms: f64, seen: Vec<f64> },
    /// Entrainment: pacing.
    EntrainPace {
        times: Vec<f64>,
        next: usize,
        tcl_ms: f64,
        cycle_ms: f64,
    },
    /// Entrainment: waiting for the return beat.
    Return {
        last_stim_ms: f64,
        tcl_ms: f64,
        cycle_ms: f64,
    },
    Done,
}

/// A protocol being delivered. See the module documentation.
#[derive(Debug, Clone, PartialEq)]
pub struct ProtocolRun {
    protocol: Protocol,
    phase: Phase,
    delivered: u32,
    /// ERP search: couplings so far, and the coupling of the next drive.
    curve: Vec<CouplingPoint>,
    /// Last activation time of the paced cell seen, ms.
    last_activation_ms: f64,
    outcome: Option<ProtocolOutcome>,
}

fn train_times(start: f64, cycle: f64, n: u32) -> Vec<f64> {
    (0..n).map(|i| start + f64::from(i) * cycle).collect()
}

impl ProtocolRun {
    /// Start `protocol` at `now_ms`; the first stimulus comes one step later at the earliest.
    #[must_use]
    pub fn new(protocol: Protocol, now_ms: f64) -> Self {
        let phase = match &protocol {
            Protocol::Continuous { cycle_ms } => Phase::Continuous {
                next_ms: now_ms,
                cycle_ms: *cycle_ms,
            },
            Protocol::Train {
                s1_cycle_ms,
                s1_count,
                s2_ms,
                s3_ms,
            } => {
                let mut times = train_times(now_ms, *s1_cycle_ms, *s1_count);
                let mut t = times.last().copied().unwrap_or(now_ms);
                for extra in [s2_ms, s3_ms].into_iter().map_while(|x| *x) {
                    t += extra;
                    times.push(t);
                }
                Phase::Plan { times, next: 0 }
            }
            Protocol::Decremental {
                start_cycle_ms,
                end_cycle_ms,
                step_ms,
                beats,
            } => {
                let mut times = Vec::new();
                let mut t = now_ms;
                let mut cycle = *start_cycle_ms;
                while cycle >= *end_cycle_ms - 1e-9 {
                    for _ in 0..*beats {
                        times.push(t);
                        t += cycle;
                    }
                    if *step_ms <= 0.0 {
                        break;
                    }
                    cycle -= step_ms;
                }
                Phase::Plan { times, next: 0 }
            }
            Protocol::ErpSearch {
                s1_cycle_ms,
                s1_count,
                s2_start_ms,
                ..
            } => Self::drive(now_ms, *s1_cycle_ms, *s1_count, *s2_start_ms),
            Protocol::Entrainment { .. } => Phase::Measure {
                since_ms: now_ms,
                seen: Vec::with_capacity(CYCLES_OBSERVED + 1),
            },
        };
        Self {
            protocol,
            phase,
            delivered: 0,
            curve: Vec::new(),
            last_activation_ms: f64::NEG_INFINITY,
            outcome: None,
        }
    }

    fn drive(start: f64, cycle: f64, count: u32, s2_ms: f64) -> Phase {
        let mut times = train_times(start, cycle, count);
        let s2_at = times.last().copied().unwrap_or(start) + s2_ms;
        times.push(s2_at);
        Phase::Drive {
            times,
            next: 0,
            s2_ms,
            s2_at,
            point: CouplingPoint {
                s1s2_ms: s2_ms,
                captured: false,
                his: false,
                a2h2_ms: None,
                slow_pathway: false,
                ap_blocked: false,
            },
            his_seen: false,
            input_at: None,
        }
    }

    /// The protocol.
    #[must_use]
    pub fn protocol(&self) -> &Protocol {
        &self.protocol
    }

    /// The result once finished.
    #[must_use]
    pub fn outcome(&self) -> Option<&ProtocolOutcome> {
        self.outcome.as_ref()
    }

    /// Stop now; a run stopped by hand reports what it delivered.
    pub fn stop(&mut self) {
        if self.outcome.is_none() {
            self.outcome = Some(match &self.protocol {
                Protocol::ErpSearch { .. } => ProtocolOutcome::Erp(self.erp_result(None, false)),
                _ => ProtocolOutcome::Delivered {
                    stimuli: self.delivered,
                },
            });
        }
        self.phase = Phase::Done;
    }

    /// Progress.
    #[must_use]
    pub fn status(&self) -> ProtocolStatus {
        match &self.phase {
            Phase::Plan { times, next } => ProtocolStatus::Pacing {
                delivered: self.delivered,
                interval_ms: match (next.checked_sub(1).and_then(|i| times.get(i)), times.get(*next)) {
                    (Some(a), Some(b)) => b - a,
                    _ => 0.0,
                },
            },
            Phase::Continuous { cycle_ms, .. } => ProtocolStatus::Pacing {
                delivered: self.delivered,
                interval_ms: *cycle_ms,
            },
            Phase::Drive { s2_ms, .. } => ProtocolStatus::Pacing {
                delivered: self.delivered,
                interval_ms: *s2_ms,
            },
            Phase::EntrainPace { cycle_ms, .. } => ProtocolStatus::Pacing {
                delivered: self.delivered,
                interval_ms: *cycle_ms,
            },
            Phase::Measure { .. } | Phase::Return { .. } => ProtocolStatus::Watching,
            Phase::Done => ProtocolStatus::Finished,
        }
    }

    /// Whether a stimulus is due by `until_ms`; call repeatedly until `None`, delivering each.
    pub fn next_stimulus(&mut self, until_ms: f64) -> Option<f64> {
        let delivered = match &mut self.phase {
            Phase::Plan { times, next }
            | Phase::Drive { times, next, .. }
            | Phase::EntrainPace { times, next, .. } => {
                let t = times.get(*next).copied().filter(|t| *t <= until_ms)?;
                *next += 1;
                t
            }
            Phase::Continuous { next_ms, cycle_ms } => {
                if *next_ms > until_ms {
                    return None;
                }
                let t = *next_ms;
                *next_ms += *cycle_ms;
                t
            }
            _ => return None,
        };
        self.delivered += 1;
        Some(delivered)
    }

    /// Feed the state after a step: `now_ms`, this step's timing events, and the last activation
    /// time of the paced cell.
    pub fn observe(&mut self, now_ms: f64, events: &[Event], site_activation_ms: f64) {
        let new_activation = site_activation_ms > self.last_activation_ms + 1.0;
        if new_activation {
            self.last_activation_ms = site_activation_ms;
        }
        match &mut self.phase {
            Phase::Plan { times, next } => {
                if *next >= times.len() {
                    self.outcome = Some(ProtocolOutcome::Delivered {
                        stimuli: self.delivered,
                    });
                    self.phase = Phase::Done;
                }
            }
            Phase::Drive { .. } => self.observe_drive(now_ms, events),
            Phase::Measure { since_ms, seen } => {
                if new_activation {
                    seen.push(site_activation_ms);
                }
                if seen.len() > CYCLES_OBSERVED {
                    let cycles = (seen[seen.len() - 1] - seen[0]) / (seen.len() - 1) as f64;
                    if let Protocol::Entrainment { offset_ms, count } = self.protocol {
                        let cycle_ms = (cycles - offset_ms).max(50.0);
                        // first stimulus one cycle after the last activation seen at the site
                        let start = (seen[seen.len() - 1] + cycle_ms).max(now_ms + 1.0);
                        self.phase = Phase::EntrainPace {
                            times: train_times(start, cycle_ms, count),
                            next: 0,
                            tcl_ms: cycles,
                            cycle_ms,
                        };
                    }
                } else if now_ms - *since_ms > WATCH_LIMIT_MS {
                    self.outcome = Some(ProtocolOutcome::NoReturn);
                    self.phase = Phase::Done;
                }
            }
            Phase::EntrainPace {
                times,
                next,
                tcl_ms,
                cycle_ms,
            } => {
                if *next >= times.len() {
                    self.phase = Phase::Return {
                        last_stim_ms: times[times.len() - 1],
                        tcl_ms: *tcl_ms,
                        cycle_ms: *cycle_ms,
                    };
                }
            }
            Phase::Return {
                last_stim_ms,
                tcl_ms,
                cycle_ms,
            } => {
                if new_activation && site_activation_ms > *last_stim_ms + RETURN_BLANK_MS {
                    let ppi_ms = site_activation_ms - *last_stim_ms;
                    self.outcome = Some(ProtocolOutcome::Entrainment(EntrainmentResult {
                        tcl_ms: *tcl_ms,
                        pacing_cycle_ms: *cycle_ms,
                        ppi_ms,
                        in_circuit: ppi_ms - *tcl_ms <= IN_CIRCUIT_MS,
                    }));
                    self.phase = Phase::Done;
                } else if now_ms - *last_stim_ms > WATCH_LIMIT_MS {
                    self.outcome = Some(ProtocolOutcome::NoReturn);
                    self.phase = Phase::Done;
                }
            }
            Phase::Continuous { .. } | Phase::Done => {}
        }
    }

    fn observe_drive(&mut self, now_ms: f64, events: &[Event]) {
        let Phase::Drive {
            s2_at,
            point,
            his_seen,
            input_at,
            ..
        } = &mut self.phase
        else {
            return;
        };
        for e in events {
            if e.at.0 < *s2_at - 1e-6 {
                continue;
            }
            let own = (e.at.0 - *s2_at).abs() < 1e-6 && e.origin == Origin::External;
            match (e.kind, e.region) {
                (EventKind::Stimulus, _) if own => point.captured = true,
                (EventKind::Conducted, Region::AvNodeFastPathway) if input_at.is_none() => {
                    *input_at = Some(e.at.0);
                }
                (EventKind::Conducted, Region::AvNodeSlowPathway) if input_at.is_none() => {
                    *input_at = Some(e.at.0);
                    point.slow_pathway = true;
                }
                (EventKind::Conducted, Region::HisBundle) if !*his_seen => {
                    *his_seen = true;
                    point.his = true;
                    // a His stimulus is captured by firing the bundle directly
                    point.captured = point.captured || own;
                    point.a2h2_ms = input_at.map(|t| e.at.0 - t);
                }
                (EventKind::Blocked, Region::AccessoryPathway) => point.ap_blocked = true,
                _ => {}
            }
        }
        if now_ms >= *s2_at + OBSERVE_MS {
            let finished = *point;
            self.end_drive(now_ms, finished);
        }
    }

    fn end_drive(&mut self, now_ms: f64, point: CouplingPoint) {
        let Protocol::ErpSearch {
            target,
            s1_cycle_ms,
            s1_count,
            s2_step_ms,
            s2_floor_ms,
            ..
        } = self.protocol
        else {
            return;
        };
        self.curve.push(point);
        let (failed, local) = match target {
            ErpTarget::Atrium | ErpTarget::Ventricle => (!point.captured, !point.captured),
            ErpTarget::AvNode => (point.captured && !point.his, !point.captured),
            ErpTarget::FastPathway => (point.captured && (point.slow_pathway || !point.his), !point.captured),
            ErpTarget::AccessoryPathway => (point.captured && point.ap_blocked, !point.captured),
        };
        let next_s2 = point.s1s2_ms - s2_step_ms;
        if failed || local || next_s2 < s2_floor_ms {
            let erp = failed.then_some(point.s1s2_ms);
            let limited = local
                && !matches!(target, ErpTarget::Atrium | ErpTarget::Ventricle);
            self.outcome = Some(ProtocolOutcome::Erp(self.erp_result(erp, limited)));
            self.phase = Phase::Done;
        } else {
            self.phase = Self::drive(now_ms + s1_cycle_ms, s1_cycle_ms, s1_count, next_s2);
        }
    }

    fn erp_result(&self, erp_ms: Option<f64>, limited_by_local_erp: bool) -> ErpResult {
        let target = match self.protocol {
            Protocol::ErpSearch { target, .. } => target,
            _ => ErpTarget::Atrium,
        };
        ErpResult {
            target,
            erp_ms,
            limited_by_local_erp,
            ah_jump: ah_jump(&self.curve),
            curve: self.curve.clone(),
        }
    }
}

/// First step of the A2–H2 curve of at least [`AH_JUMP_MS`], as `(coupling, jump)`.
fn ah_jump(curve: &[CouplingPoint]) -> Option<(f64, f64)> {
    curve.windows(2).find_map(|w| {
        let jump = w[1].a2h2_ms? - w[0].a2h2_ms?;
        (jump >= AH_JUMP_MS).then_some((w[1].s1s2_ms, jump))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Origin;
    use heartsim_core::Millis;

    fn stimuli_until(run: &mut ProtocolRun, until: f64) -> Vec<f64> {
        std::iter::from_fn(|| run.next_stimulus(until)).collect()
    }

    #[test]
    fn a_train_is_s1s_then_coupled_extrastimuli() {
        let mut run = ProtocolRun::new(
            Protocol::Train {
                s1_cycle_ms: 600.0,
                s1_count: 3,
                s2_ms: Some(300.0),
                s3_ms: Some(250.0),
            },
            1000.0,
        );
        assert_eq!(
            stimuli_until(&mut run, 10_000.0),
            vec![1000.0, 1600.0, 2200.0, 2500.0, 2750.0]
        );
        run.observe(10_000.0, &[], f64::NAN);
        assert_eq!(run.outcome(), Some(&ProtocolOutcome::Delivered { stimuli: 5 }));
    }

    #[test]
    fn a_decremental_ramp_shortens_the_cycle_in_steps() {
        let mut run = ProtocolRun::new(
            Protocol::Decremental {
                start_cycle_ms: 400.0,
                end_cycle_ms: 380.0,
                step_ms: 10.0,
                beats: 2,
            },
            0.0,
        );
        assert_eq!(
            stimuli_until(&mut run, 10_000.0),
            vec![0.0, 400.0, 800.0, 1190.0, 1580.0, 1960.0]
        );
    }

    #[test]
    fn continuous_pacing_runs_until_stopped() {
        let mut run = ProtocolRun::new(Protocol::Continuous { cycle_ms: 500.0 }, 0.0);
        assert_eq!(stimuli_until(&mut run, 1000.0).len(), 3);
        run.stop();
        assert_eq!(run.next_stimulus(10_000.0), None);
        assert_eq!(run.outcome(), Some(&ProtocolOutcome::Delivered { stimuli: 3 }));
    }

    #[test]
    fn an_atrial_erp_search_stops_at_the_first_coupling_that_fails_to_capture() {
        let protocol = Protocol::ErpSearch {
            target: ErpTarget::Atrium,
            s1_cycle_ms: 600.0,
            s1_count: 1,
            s2_start_ms: 300.0,
            s2_step_ms: 10.0,
            s2_floor_ms: 150.0,
        };
        let mut run = ProtocolRun::new(protocol, 0.0);
        let mut now = 0.0;
        // the tissue under the pair recovers at 285 ms in this script
        while run.outcome().is_none() && now < 20_000.0 {
            now += 1.0;
            let mut events = Vec::new();
            if let Some(t) = run.next_stimulus(now) {
                let coupling = match run.status() {
                    ProtocolStatus::Pacing { interval_ms, .. } => interval_ms,
                    _ => 0.0,
                };
                let is_s2 = matches!(run.phase, Phase::Drive { s2_at, .. } if (t - s2_at).abs() < 1e-6);
                let kind = if is_s2 && coupling < 285.0 {
                    EventKind::Blocked
                } else {
                    EventKind::Stimulus
                };
                events.push(Event::new(Millis(t), Region::RightAtrium, kind, Origin::External));
            }
            run.observe(now, &events, f64::NAN);
        }
        let Some(ProtocolOutcome::Erp(erp)) = run.outcome() else {
            panic!("no result: {:?}", run.outcome());
        };
        assert_eq!(erp.erp_ms, Some(280.0));
        assert_eq!(erp.curve.len(), 3);
        assert!(!erp.limited_by_local_erp);
    }

    #[test]
    fn the_ah_jump_is_the_first_step_of_at_least_50_ms() {
        let p = |s: f64, a: f64| CouplingPoint {
            s1s2_ms: s,
            captured: true,
            his: true,
            a2h2_ms: Some(a),
            slow_pathway: false,
            ap_blocked: false,
        };
        assert_eq!(ah_jump(&[p(400.0, 90.0), p(390.0, 95.0), p(380.0, 100.0)]), None);
        assert_eq!(
            ah_jump(&[p(400.0, 90.0), p(390.0, 95.0), p(380.0, 210.0), p(370.0, 300.0)]),
            Some((380.0, 115.0))
        );
    }
}
