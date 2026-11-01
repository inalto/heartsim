//! Conduction-system timing engine (Tier 0/1 backbone).
//!
//! Models the heart's rhythm as a small set of coupled oscillators and delay lines:
//! SA node → atria → AV node (decremental conduction + refractoriness) → His → ventricles,
//! with escape pacemakers in the AV junction and the ventricles. It emits [`Event`]s that
//! drive both the template ECG (Tier 0) and the spatial activation (Tier 1).
//!
//! AV conduction time follows the classic recovery curve
//! `AVCT(RT) = AVCT_min + A · exp(−(RT − ERP) / τ)` where RT is the recovery time since the
//! previous AV activation; impulses arriving before ERP are blocked. This single formula
//! produces 1st-degree block (large AVCT_min), Wenckebach (steep A/τ at high atrial rates) and
//! rate-dependent block; Mobitz II is modelled as an all-or-none His-level drop ratio.

use crate::engine::{CellState, Engine, Event, EventKind};
use heartsim_core::{Millis, Region};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimingParams {
    pub sinus_rate_bpm: f64,
    /// Fraction of RR modulated by respiration (0.1 = ±10 %), and breaths per minute.
    pub respiration_depth: f64,
    pub respiration_rate_bpm: f64,
    /// Atrial conduction time from SA node to AV node input, ms.
    pub atrial_delay_ms: f64,
    pub av_min_ms: f64,
    pub av_recovery_amplitude_ms: f64,
    pub av_recovery_tau_ms: f64,
    pub av_erp_ms: f64,
    /// Complete AV block.
    pub av_block: bool,
    /// Mobitz II: conduct `n − 1` of every `n` beats (0 or 1 = off).
    pub mobitz2_ratio: u32,
    /// His → ventricular myocardium delay, ms (His-Purkinje time).
    pub his_delay_ms: f64,
    /// Junctional and ventricular escape rates, bpm (0 = none).
    pub junctional_escape_bpm: f64,
    pub ventricular_escape_bpm: f64,
}

impl Default for TimingParams {
    fn default() -> Self {
        Self {
            sinus_rate_bpm: 70.0,
            respiration_depth: 0.0,
            respiration_rate_bpm: 12.0,
            atrial_delay_ms: 40.0,
            av_min_ms: 80.0,
            av_recovery_amplitude_ms: 120.0,
            av_recovery_tau_ms: 80.0,
            av_erp_ms: 300.0,
            av_block: false,
            mobitz2_ratio: 0,
            his_delay_ms: 40.0,
            junctional_escape_bpm: 45.0,
            ventricular_escape_bpm: 30.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingEngine {
    pub params: TimingParams,
    now: Millis,
    next_sinus: Millis,
    /// Pending scheduled events (small, kept sorted by time).
    queue: Vec<Event>,
    last_av_activation: Option<Millis>,
    last_ventricular: Option<Millis>,
    last_atrial: Option<Millis>,
    atrial_beats_since_drop: u32,
    events: Vec<Event>,
    cells: CellState,
}

impl TimingEngine {
    #[must_use]
    pub fn new(params: TimingParams) -> Self {
        let mut e = Self {
            params,
            now: Millis::ZERO,
            next_sinus: Millis::ZERO,
            queue: Vec::new(),
            last_av_activation: None,
            last_ventricular: None,
            last_atrial: None,
            atrial_beats_since_drop: 0,
            events: Vec::new(),
            cells: CellState::new(0),
        };
        e.reset();
        e
    }

    fn schedule(&mut self, ev: Event) {
        let pos = self.queue.partition_point(|e| e.at.0 <= ev.at.0);
        self.queue.insert(pos, ev);
    }

    fn sinus_cycle(&self) -> Millis {
        let rr = Millis::from_bpm(self.params.sinus_rate_bpm);
        if self.params.respiration_depth <= 0.0 {
            return rr;
        }
        let phase = self.now.seconds() * self.params.respiration_rate_bpm / 60.0 * std::f64::consts::TAU;
        rr * (1.0 - self.params.respiration_depth * phase.sin())
    }

    /// AV conduction time for an atrial impulse arriving now, or `None` if blocked.
    fn av_conduction(&self, arrival: Millis) -> Option<Millis> {
        let p = &self.params;
        if p.av_block {
            return None;
        }
        let Some(last) = self.last_av_activation else {
            return Some(Millis(p.av_min_ms));
        };
        let rt = arrival.0 - last.0;
        if rt < p.av_erp_ms {
            return None;
        }
        let avct =
            p.av_min_ms + p.av_recovery_amplitude_ms * (-(rt - p.av_erp_ms) / p.av_recovery_tau_ms).exp();
        Some(Millis(avct))
    }

    fn on_atrial(&mut self, at: Millis, kind: EventKind) {
        self.last_atrial = Some(at);
        self.events.push(Event {
            at,
            region: Region::RightAtrium,
            kind,
        });
        let arrival = at + Millis(self.params.atrial_delay_ms);
        // Mobitz II drop
        if self.params.mobitz2_ratio > 1 {
            self.atrial_beats_since_drop += 1;
            if self.atrial_beats_since_drop >= self.params.mobitz2_ratio {
                self.atrial_beats_since_drop = 0;
                self.events.push(Event {
                    at: arrival,
                    region: Region::HisBundle,
                    kind: EventKind::Blocked,
                });
                return;
            }
        }
        match self.av_conduction(arrival) {
            Some(delay) => {
                self.last_av_activation = Some(arrival);
                self.schedule(Event {
                    at: arrival + delay,
                    region: Region::HisBundle,
                    kind: EventKind::Conducted,
                });
            }
            None => self.events.push(Event {
                at: arrival,
                region: Region::AvNodeCompact,
                kind: EventKind::Blocked,
            }),
        }
    }

    fn on_his(&mut self, at: Millis, kind: EventKind) {
        self.events.push(Event {
            at,
            region: Region::HisBundle,
            kind,
        });
        self.schedule(Event {
            at: at + Millis(self.params.his_delay_ms),
            region: Region::LeftVentricle,
            kind: EventKind::Conducted,
        });
    }

    fn on_ventricular(&mut self, at: Millis, kind: EventKind) {
        self.last_ventricular = Some(at);
        self.events.push(Event {
            at,
            region: Region::LeftVentricle,
            kind,
        });
    }

    fn check_escape(&mut self) {
        let p = self.params.clone();
        let since_v = self.last_ventricular.map_or(f64::INFINITY, |t| self.now.0 - t.0);
        // junctional escape fires if no ventricular activation within its cycle
        if p.junctional_escape_bpm > 0.0 && since_v >= 60_000.0 / p.junctional_escape_bpm {
            let at = self.now;
            self.last_av_activation = Some(at);
            self.events.push(Event {
                at,
                region: Region::AvNodeCompact,
                kind: EventKind::Automatic,
            });
            self.on_his(at, EventKind::Conducted);
            // keep `last_ventricular` consistent until the scheduled event fires
            self.last_ventricular = Some(at + Millis(p.his_delay_ms));
        } else if p.ventricular_escape_bpm > 0.0 && since_v >= 60_000.0 / p.ventricular_escape_bpm {
            let at = self.now;
            self.on_ventricular(at, EventKind::Automatic);
        }
    }
}

impl Engine for TimingEngine {
    fn step(&mut self, dt: Millis) {
        self.now += dt;
        // sinus node
        while self.next_sinus.0 <= self.now.0 {
            let at = self.next_sinus;
            self.on_atrial(at, EventKind::Automatic);
            let cycle = self.sinus_cycle();
            self.next_sinus = at + cycle;
        }
        // scheduled conducted events
        while self.queue.first().is_some_and(|e| e.at.0 <= self.now.0) {
            let ev = self.queue.remove(0);
            match ev.region {
                Region::HisBundle => self.on_his(ev.at, ev.kind),
                Region::LeftVentricle => self.on_ventricular(ev.at, ev.kind),
                _ => self.events.push(ev),
            }
        }
        self.check_escape();
    }

    fn now(&self) -> Millis {
        self.now
    }

    fn cells(&self) -> &CellState {
        &self.cells
    }

    fn drain_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    fn reset(&mut self) {
        self.now = Millis::ZERO;
        self.next_sinus = Millis(200.0);
        self.queue.clear();
        self.last_av_activation = None;
        // Escape timers count from reset, so no escape beat precedes the first sinus beat.
        self.last_ventricular = Some(Millis::ZERO);
        self.last_atrial = None;
        self.atrial_beats_since_drop = 0;
        self.events.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(params: TimingParams, seconds: f64) -> Vec<Event> {
        let mut e = TimingEngine::new(params);
        let mut all = Vec::new();
        let steps = (seconds * 1000.0) as usize;
        for _ in 0..steps {
            e.step(Millis(1.0));
            all.extend(e.drain_events());
        }
        all
    }

    fn count(evs: &[Event], region: Region, kind: EventKind) -> usize {
        evs.iter()
            .filter(|e| e.region == region && e.kind == kind)
            .count()
    }

    #[test]
    fn sinus_rhythm_conducts_one_to_one() {
        let evs = run(TimingParams::default(), 10.0);
        let p = count(&evs, Region::RightAtrium, EventKind::Automatic);
        let v = count(&evs, Region::LeftVentricle, EventKind::Conducted);
        assert!((11..=12).contains(&p), "{p} P waves at 70 bpm in 10 s");
        assert_eq!(p, v);
        // PR interval = atrial delay + AVCT_min (+ recovery term ≈ 0 at 70 bpm) + His delay ≈ 160 ms
        let first_p = evs.iter().find(|e| e.region == Region::RightAtrium).unwrap().at;
        let first_v = evs.iter().find(|e| e.region == Region::LeftVentricle).unwrap().at;
        let pr = first_v.0 - first_p.0;
        assert!((150.0..=200.0).contains(&pr), "PR {pr}");
    }

    #[test]
    fn complete_block_gives_junctional_escape() {
        let evs = run(
            TimingParams {
                av_block: true,
                ..Default::default()
            },
            10.0,
        );
        assert_eq!(
            count(&evs, Region::LeftVentricle, EventKind::Conducted),
            7,
            "45 bpm escape ≈ 7 beats in 10 s"
        );
        assert!(count(&evs, Region::AvNodeCompact, EventKind::Blocked) >= 10);
    }

    #[test]
    fn fast_atrial_rate_produces_wenckebach_or_2to1() {
        // atrial tachycardia at 220 bpm (RR 273 ms) with ERP 300 → every second beat blocked
        let evs = run(
            TimingParams {
                sinus_rate_bpm: 220.0,
                ..Default::default()
            },
            10.0,
        );
        let p = count(&evs, Region::RightAtrium, EventKind::Automatic);
        let v = count(&evs, Region::LeftVentricle, EventKind::Conducted);
        assert!(v < p, "some atrial beats must be blocked: P={p} V={v}");
        assert!(count(&evs, Region::AvNodeCompact, EventKind::Blocked) > 0);
    }

    #[test]
    fn mobitz2_drops_every_third() {
        let evs = run(
            TimingParams {
                mobitz2_ratio: 3,
                ..Default::default()
            },
            30.0,
        );
        let p = count(&evs, Region::RightAtrium, EventKind::Automatic);
        let v = count(&evs, Region::LeftVentricle, EventKind::Conducted);
        assert!((v as f64 / p as f64 - 2.0 / 3.0).abs() < 0.05, "P={p} V={v}");
    }

    #[test]
    fn deterministic() {
        let a = run(TimingParams::default(), 5.0);
        let b = run(TimingParams::default(), 5.0);
        assert_eq!(a, b);
    }
}
