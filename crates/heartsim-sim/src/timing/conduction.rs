//! Per-structure handlers of the timing engine: what happens when a pending item comes due.
//!
//! Each handler emits [`Event`]s and [`LadderSample`]s at the item's own time `t` (≤ `now`) and
//! schedules the downstream consequences, so the queue stays chronological.

use super::ladder::LadderSample;
use super::pacer::{MarkerKind, PacerMarker};
use super::params::{AvPathway, FocusMode, FocusRegion, LeadSite, PacerChannel, PacerConfig};
use super::{AtrialVia, Edge, HisVia, Pending, StimTarget, TimingEngine, VentVia};
use crate::engine::{Chamber, EventKind, Origin};
use heartsim_core::{Millis, Region};

/// Shrier recovery curve: conduction time for recovery time `rt` (ms), `None` if blocked.
/// Drugs add `erp_bonus_ms` to the pathway's ERP and divide the conduction time by
/// `cv_scale` (nodal conduction velocity).
fn pathway_conduction(p: &AvPathway, rt: f64, erp_bonus_ms: f64, cv_scale: f64) -> Option<f64> {
    if p.blocked || rt < (p.erp_ms + erp_bonus_ms).max(0.0) {
        return None;
    }
    Some((p.min_ms + p.recovery_amplitude_ms * (-(rt - p.erp_ms) / p.recovery_tau_ms).exp()) / cv_scale)
}

fn since(last: Option<f64>, t: f64) -> f64 {
    last.map_or(f64::INFINITY, |l| t - l)
}

impl TimingEngine {
    pub(crate) fn dispatch(&mut self, t: f64, what: Pending) {
        match what {
            Pending::Sinus => self.on_sinus(t),
            Pending::FocusFire { id, remaining } => self.on_focus_fire(t, id, remaining),
            Pending::Atrial {
                region,
                kind,
                origin,
                via,
            } => {
                self.activate_atrium(t, region, kind, origin, via);
            }
            Pending::AvArrival { origin } => self.on_av_arrival(t, origin),
            Pending::His { origin, via } => self.on_his(t, origin, via),
            Pending::HisBlocked { origin } => self.emit(t, Region::HisBundle, EventKind::Blocked, origin),
            Pending::Branch {
                edge,
                origin,
                blocked,
            } => {
                let kind = if blocked {
                    EventKind::Blocked
                } else {
                    EventKind::Conducted
                };
                self.emit(t, edge.region(), kind, origin);
            }
            Pending::Ventricular {
                region,
                kind,
                origin,
                via,
            } => {
                self.activate_ventricle(t, region, kind, origin, via);
            }
            Pending::HisRetro { origin } => self.on_his_retro(t, origin),
            Pending::Shock { joules, synchronised } => self.deliver_shock(t, joules, synchronised),
            Pending::Stimulus { target } => match target {
                StimTarget::Atrium(r) => {
                    self.activate_atrium(t, r, EventKind::Stimulus, Origin::External, AtrialVia::External);
                }
                StimTarget::His => self.on_his(t, Origin::External, HisVia::Junctional),
                StimTarget::Ventricle(r) => {
                    self.activate_ventricle(t, r, EventKind::Stimulus, Origin::External, VentVia::External);
                }
            },
        }
    }

    // --- sinus node ---------------------------------------------------------------------------

    /// Sinus cycle length at time `t`, ms: sick-sinus phase, then respiratory modulation.
    pub(crate) fn sinus_cycle_ms(&self, t: f64) -> f64 {
        let s = &self.params.sinus;
        let rate = if s.tachy_brady_period_s > 0.0 {
            let phase = (t / (s.tachy_brady_period_s * 1000.0)).floor() as i64;
            if phase % 2 == 0 {
                s.tachy_rate_bpm
            } else {
                s.brady_rate_bpm
            }
        } else {
            s.rate_bpm
        };
        let rr = 60_000.0 / (rate * self.drug_effects().sa_rate_scale);
        if s.respiration_depth <= 0.0 {
            return rr;
        }
        let phase = t / 1000.0 * s.respiration_rate_bpm / 60.0 * std::f64::consts::TAU;
        rr * (1.0 - s.respiration_depth * phase.sin())
    }

    fn on_sinus(&mut self, t: f64) {
        self.sinus_beats += 1;
        let s = self.params.sinus.clone();
        let exit_blocked = s.exit_block_ratio > 1 && self.sinus_beats.is_multiple_of(s.exit_block_ratio);
        if exit_blocked {
            self.emit(t, Region::SinoatrialNode, EventKind::Blocked, Origin::Sinus);
        } else {
            self.activate_atrium(
                t,
                Region::RightAtrium,
                EventKind::Automatic,
                Origin::Sinus,
                AtrialVia::Sinus,
            );
        }
        let cycle = if s.pause_every_n > 0 && self.sinus_beats.is_multiple_of(s.pause_every_n) {
            s.pause_ms
        } else {
            self.sinus_cycle_ms(t) + self.sinus_suppression_ms
        };
        self.sinus_suppression_ms = 0.0;
        self.schedule(t + cycle, Pending::Sinus);
    }

    /// Overdrive suppression / phase resetting after a non-sinus atrial activation at `t`.
    fn sinus_overdriven(&mut self, t: f64) {
        let s = self.params.sinus.clone();
        if s.reset_on_ectopic {
            let next = t + self.sinus_cycle_ms(t) + s.suppression_ms;
            self.unschedule(|p| matches!(p, Pending::Sinus));
            self.schedule(next, Pending::Sinus);
        } else {
            self.sinus_suppression_ms = s.suppression_ms;
        }
    }

    // --- atria --------------------------------------------------------------------------------

    /// Attempt to activate the atria at `t`. Returns `true` on capture.
    pub(crate) fn activate_atrium(
        &mut self,
        t: f64,
        region: Region,
        kind: EventKind,
        origin: Origin,
        via: AtrialVia,
    ) -> bool {
        // A front sensed on the tissue model has already passed the tissue's own refractoriness:
        // the timing engine's atrial ERP (a Tier-0/1 stand-in for it) must not veto a fact.
        if via != AtrialVia::Tissue && since(self.last_atrial, t) < self.atrial_erp() {
            let r = if via == AtrialVia::Sinus {
                Region::SinoatrialNode
            } else {
                region
            };
            self.emit(t, r, EventKind::Blocked, origin);
            return false;
        }
        if let Some(last) = self.last_atrial {
            self.last_a_cycle_ms = Some(t - last);
        }
        self.last_atrial = Some(t);
        self.emit(t, region, kind, origin);
        // A front sensed on the tissue model is already at the nodal input.
        let atrial_delay = if via == AtrialVia::Tissue {
            0.0
        } else {
            self.atrial_delay()
        };
        let ladder = match via {
            AtrialVia::NodeRetro | AtrialVia::AccessoryRetro => {
                LadderSample::retrograde(Chamber::Atrium, t, t + atrial_delay, origin)
            }
            _ => LadderSample::antegrade(Chamber::Atrium, t, t + atrial_delay, origin),
        };
        self.ladder_push(ladder);

        if via != AtrialVia::Sinus {
            self.sinus_overdriven(t);
        }
        self.notify_foci(t, Chamber::Atrium, origin);

        // Towards the AV node. Echoes re-enter through the nodal turnaround; everything else
        // crosses the atria first. Impulses arriving over the accessory pathway are the AVRT
        // loop: tag them as re-entry from here on.
        let (delay, av_origin) = match via {
            AtrialVia::NodeRetro => (self.params.av.reentry_delay_ms, origin),
            AtrialVia::AccessoryRetro => (atrial_delay, Origin::Reentry),
            _ => (atrial_delay, origin),
        };
        self.schedule(t + delay, Pending::AvArrival { origin: av_origin });

        // Accessory pathway, antegrade (pre-excitation). The bundle is entered at the AV ring,
        // i.e. after atrial conduction, at the same moment the impulse reaches the nodal input.
        if via != AtrialVia::AccessoryRetro
            && let Some(ap) = self.params.accessory.clone()
            && ap.antegrade
        {
            let arrive = t + atrial_delay;
            if since(self.ap_last_use, arrive) >= ap.erp_ms + self.drug_effects().accessory_erp_add_ms {
                let v_t = arrive + ap.delay_ms;
                self.ap_last_use = Some(arrive);
                self.ladder_push(LadderSample::antegrade(
                    Chamber::AvJunction,
                    arrive,
                    v_t,
                    Origin::AccessoryPathway,
                ));
                self.schedule(
                    v_t,
                    Pending::Ventricular {
                        region: ap.ventricular_region(),
                        kind: EventKind::Conducted,
                        origin: Origin::AccessoryPathway,
                        via: VentVia::Accessory,
                    },
                );
            } else {
                self.emit(arrive, Region::AccessoryPathway, EventKind::Blocked, origin);
            }
        }
        true
    }

    // --- AV node ------------------------------------------------------------------------------

    fn on_av_arrival(&mut self, t: f64, origin: Origin) {
        let av = self.params.av.clone();
        if av.complete_block {
            self.emit(t, Region::AvNodeCompact, EventKind::Blocked, origin);
            let bar = t + 0.5 * av.fast.min_ms;
            self.ladder_push(LadderSample::antegrade(Chamber::AvJunction, t, bar, origin).blocked());
            return;
        }
        let (bonus, cv) = (self.av_erp_bonus(), self.drug_effects().av_conduction_scale);
        let rt = since(self.last_his_output, t);
        let (route, avct) = if let Some(d) = pathway_conduction(&av.fast, rt, bonus, cv) {
            (Some((HisVia::Fast, Region::AvNodeFastPathway)), d)
        } else if av.dual_pathways
            && let Some(d) = pathway_conduction(&av.slow, rt, bonus, cv)
        {
            (Some((HisVia::Slow, Region::AvNodeSlowPathway)), d)
        } else {
            (None, 0.0)
        };

        let Some((via, pathway_region)) = route else {
            // Blocked: concealed conduction advances the recovery clock part of the way.
            let c = av.concealment_fraction;
            if let Some(last) = self.last_his_output {
                self.last_his_output = Some(last + c * (t - last));
            }
            self.emit(t, Region::AvNodeCompact, EventKind::Blocked, origin);
            let bar = t + 0.5 * av.fast.min_ms;
            self.ladder_push(LadderSample::antegrade(Chamber::AvJunction, t, bar, origin).blocked());
            return;
        };

        match via {
            HisVia::Fast => self.fast_last_use = Some(t),
            _ => self.slow_last_use = Some(t),
        }
        self.emit(t, pathway_region, EventKind::Conducted, origin);
        let out = t + avct;
        self.last_his_output = Some(out);
        self.ladder_push(LadderSample::antegrade(Chamber::AvJunction, t, out, origin));

        // Mobitz II: infranodal all-or-none drop.
        let ratio = self.params.av.mobitz2_ratio;
        if ratio > 1 {
            let conducted = match self.params.av.mobitz2_conducted {
                0 => ratio - 1,
                n => n.min(ratio - 1),
            };
            self.his_beats_since_drop += 1;
            let position = self.his_beats_since_drop;
            if position >= ratio {
                self.his_beats_since_drop = 0;
            }
            if position > conducted {
                self.schedule(out, Pending::HisBlocked { origin });
                return;
            }
        }
        self.schedule(out, Pending::His { origin, via });
    }

    // --- His bundle ---------------------------------------------------------------------------

    pub(crate) fn on_his(&mut self, t: f64, origin: Origin, via: HisVia) {
        let his_interval = since(self.last_his_fired, t);
        self.last_his_fired = Some(t);
        if self.last_his_output.is_none_or(|o| o < t) {
            self.last_his_output = Some(t);
        }
        self.emit(t, Region::HisBundle, EventKind::Conducted, origin);
        self.notify_foci(t, Chamber::AvJunction, origin);

        // Retrograde over the fast pathway (VA conduction, AVNRT echo) — never back up the
        // pathway the impulse came down.
        let av = self.params.av.clone();
        if via != HisVia::Fast && av.retrograde_fast && !av.complete_block && !av.fast.blocked {
            let bonus = self.av_erp_bonus();
            if since(self.fast_last_use, t) >= (av.fast.erp_ms + bonus).max(0.0) {
                let delay = av.retrograde_delay_ms / self.drug_effects().av_conduction_scale;
                let echo_origin = if via == HisVia::Slow {
                    Origin::Reentry
                } else {
                    origin
                };
                self.fast_last_use = Some(t);
                // the node is busy until the impulse leaves it into the atria
                self.last_his_output = Some(t + delay);
                self.ladder_push(LadderSample::retrograde(
                    Chamber::AvJunction,
                    t,
                    t + delay,
                    echo_origin,
                ));
                self.schedule(
                    t + delay,
                    Pending::Atrial {
                        region: Region::RightAtrium,
                        kind: EventKind::Conducted,
                        origin: echo_origin,
                        via: AtrialVia::NodeRetro,
                    },
                );
            }
        }

        if via != HisVia::Retrograde {
            self.conduct_his_purkinje(t, origin, his_interval);
        }
    }

    /// His → bundle branches → earliest ventricular activation.
    fn conduct_his_purkinje(&mut self, t: f64, origin: Origin, his_interval: f64) {
        let hp = self.his_purkinje();
        let branch_t = t + hp.his_ms;
        let mut earliest: Option<(f64, Region)> = None;
        let mut consider = |time: f64, region: Region| {
            if earliest.is_none_or(|(e, _)| time < e) {
                earliest = Some((time, region));
            }
        };

        // Right bundle.
        let rbb_ok = hp.rbb.conducts(his_interval);
        let rbb_t = branch_t + hp.rbb.delay_ms;
        if rbb_ok {
            consider(rbb_t, Region::RightVentricle);
        }
        // Left bundle and fascicles.
        let lbb_ok = hp.lbb.conducts(his_interval);
        let lbb_t = branch_t + hp.lbb.delay_ms;
        let laf_ok = lbb_ok && hp.laf.conducts(his_interval);
        let lpf_ok = lbb_ok && hp.lpf.conducts(his_interval);
        if laf_ok {
            consider(lbb_t + hp.laf.delay_ms, Region::LeftVentricle);
        }
        if lpf_ok {
            consider(lbb_t + hp.lpf.delay_ms, Region::LeftVentricle);
        }

        let b = |edge: Edge, blocked: bool| Pending::Branch {
            edge,
            origin,
            blocked,
        };
        self.schedule(if rbb_ok { rbb_t } else { branch_t }, b(Edge::Rbb, !rbb_ok));
        self.schedule(if lbb_ok { lbb_t } else { branch_t }, b(Edge::Lbb, !lbb_ok));
        if lbb_ok {
            self.schedule(
                if laf_ok { lbb_t + hp.laf.delay_ms } else { lbb_t },
                b(Edge::Laf, !laf_ok),
            );
            self.schedule(
                if lpf_ok { lbb_t + hp.lpf.delay_ms } else { lbb_t },
                b(Edge::Lpf, !lpf_ok),
            );
        }

        if let Some((v_t, region)) = earliest {
            self.ladder_push(LadderSample::antegrade(Chamber::AvJunction, t, v_t, origin));
            self.schedule(
                v_t,
                Pending::Ventricular {
                    region,
                    kind: EventKind::Conducted,
                    origin,
                    via: VentVia::HisPurkinje,
                },
            );
        } else {
            self.ladder_push(LadderSample::antegrade(Chamber::AvJunction, t, branch_t, origin).blocked());
        }
    }

    /// Retrograde arrival at the His bundle from a ventricular impulse.
    fn on_his_retro(&mut self, t: f64, origin: Origin) {
        let his_erp = self.params.his_purkinje.erp_ms;
        if since(self.last_his_fired, t) < his_erp {
            return; // His bundle still refractory from its own antegrade activation
        }
        let total = self.his_purkinje().total_ms();
        self.ladder_push(LadderSample::retrograde(
            Chamber::AvJunction,
            t - total,
            t,
            origin,
        ));
        self.on_his(t, origin, HisVia::Retrograde);
    }

    // --- ventricles ---------------------------------------------------------------------------

    /// Attempt to activate the ventricular myocardium at `t`. Returns `true` on capture.
    pub(crate) fn activate_ventricle(
        &mut self,
        t: f64,
        region: Region,
        kind: EventKind,
        origin: Origin,
        via: VentVia,
    ) -> bool {
        let gap = since(self.last_ventricular, t);
        let fusion = gap < self.params.fusion_window_ms && self.last_vent_via != Some(via);
        // As for the atria: a ventricular front the tissue produced (VT, VF) is never vetoed by
        // the timing engine's ERP, which would turn VF (cycles of 150-200 ms) into "asystole".
        if !fusion && via != VentVia::Tissue && gap < self.ventricular_erp() {
            self.emit(t, region, EventKind::Blocked, origin);
            return false;
        }
        if !fusion {
            self.last_v_rr_ms = gap.is_finite().then_some(gap);
            self.last_ventricular = Some(t);
            self.last_vent_via = Some(via);
        }
        self.emit(t, region, kind, origin);
        let qrs = self.params.fusion_window_ms;
        self.ladder_push(LadderSample::antegrade(Chamber::Ventricle, t, t + qrs, origin));
        if fusion {
            return true;
        }
        self.notify_foci(t, Chamber::Ventricle, origin);

        // Retrograde into the His–Purkinje system (not for impulses that came down it).
        if via != VentVia::HisPurkinje {
            let total = self.his_purkinje().total_ms();
            self.schedule(t + total, Pending::HisRetro { origin });
        }
        // Retrograde over the accessory pathway (orthodromic AVRT, retrograde P).
        if via != VentVia::Accessory
            && let Some(ap) = self.params.accessory.clone()
            && ap.retrograde
        {
            if since(self.ap_last_use, t) >= ap.erp_ms + self.drug_effects().accessory_erp_add_ms {
                let (delay, a_region) = (ap.delay_ms, ap.atrial_region());
                self.ap_last_use = Some(t);
                self.ladder_push(LadderSample::retrograde(
                    Chamber::AvJunction,
                    t,
                    t + delay,
                    Origin::AccessoryPathway,
                ));
                self.schedule(
                    t + delay,
                    Pending::Atrial {
                        region: a_region,
                        kind: EventKind::Conducted,
                        origin: Origin::AccessoryPathway,
                        via: AtrialVia::AccessoryRetro,
                    },
                );
            } else {
                self.emit(t, Region::AccessoryPathway, EventKind::Blocked, origin);
            }
        }
        true
    }

    // --- escape pacemakers --------------------------------------------------------------------

    pub(crate) fn check_escape(&mut self) {
        let now = self.now;
        let mut esc = self.params.escape.clone();
        esc.junctional_bpm = self.automatic_rate(esc.junctional_bpm);
        esc.ventricular_bpm = self.automatic_rate(esc.ventricular_bpm);
        let hp_total = self.his_purkinje().total_ms();
        // Anchor: the last ventricular activation or the one already on its way from the node.
        let anchor = self
            .escape_anchor
            .max(self.last_ventricular.unwrap_or(f64::NEG_INFINITY))
            .max(self.last_his_output.map_or(f64::NEG_INFINITY, |h| h + hp_total));
        let silent = now - anchor;
        if esc.junctional_bpm > 0.0 && silent >= 60_000.0 / esc.junctional_bpm {
            self.emit(now, Region::AvNodeCompact, EventKind::Automatic, Origin::Escape);
            self.on_his(now, Origin::Escape, HisVia::Junctional);
        } else if esc.ventricular_bpm > 0.0 && silent >= 60_000.0 / esc.ventricular_bpm {
            self.activate_ventricle(
                now,
                esc.ventricular_region,
                EventKind::Automatic,
                Origin::Escape,
                VentVia::Escape,
            );
        }
    }

    // --- pacemaker ----------------------------------------------------------------------------

    pub(crate) fn run_pacer(&mut self) {
        let Some(cfg) = self.params.pacer.take() else {
            return;
        };
        let now = self.now;
        let amplitude = PacerChannel::ALL.map(|ch| self.lead_amplitude_mv(&cfg, ch));
        let req = self.pacer.update(
            &cfg,
            now,
            &self.events[self.step_start..],
            amplitude,
            &mut self.markers,
        );
        for (wanted, ch) in [
            (req.atrium, PacerChannel::Atrial),
            (req.ventricle, PacerChannel::Ventricular),
            (req.lv, PacerChannel::LeftVentricular),
        ] {
            if wanted {
                self.deliver_pace(&cfg, ch, now);
            }
        }
        self.params.pacer = Some(cfg);
    }

    /// One stimulus on `channel` at `t`: captures if the output reaches the lead's threshold and
    /// the tissue is excitable. Logs the pace marker either way.
    fn deliver_pace(&mut self, cfg: &PacerConfig, channel: PacerChannel, t: f64) {
        let Some(lead) = cfg.lead(channel) else {
            return;
        };
        let origin = Origin::Pacer(channel.index());
        let region = lead.site.region();
        let above_threshold = cfg.output_capture && cfg.output_ma(channel) >= lead.threshold_at(t);
        let captured = if !above_threshold {
            self.emit(t, region, EventKind::Blocked, origin);
            false
        } else if lead.site.is_atrial() {
            self.activate_atrium(t, region, EventKind::Stimulus, origin, AtrialVia::Pacer)
        } else if lead.site == LeadSite::HisBundle {
            // Selective His capture: the impulse enters the conduction system and reaches the
            // ventricles over the bundle branches (narrow QRS). The bundle must be excitable.
            if since(self.last_his_fired, t) < self.params.his_purkinje.erp_ms {
                self.emit(t, region, EventKind::Blocked, origin);
                false
            } else {
                self.emit(t, region, EventKind::Stimulus, origin);
                self.on_his(t, origin, HisVia::Junctional);
                true
            }
        } else {
            let via = if channel == PacerChannel::LeftVentricular {
                VentVia::PacerLv
            } else {
                VentVia::Pacer
            };
            self.activate_ventricle(t, region, EventKind::Stimulus, origin, via)
        };
        self.pacer.paced(channel, t, captured);
        self.markers.push(PacerMarker {
            at: Millis(t),
            channel,
            kind: MarkerKind::Pace { captured },
        });
    }

    /// Electrogram amplitude the lead of `channel` sees for an intrinsic event, mV: measured on
    /// the mesh when Tier 1 supplied it, the lead's parameter otherwise.
    fn lead_amplitude_mv(&self, cfg: &PacerConfig, channel: PacerChannel) -> f64 {
        self.measured_amplitude_mv[channel as usize]
            .or_else(|| cfg.lead(channel).map(super::params::LeadParams::intrinsic))
            .unwrap_or(0.0)
    }

    // --- ectopic foci -------------------------------------------------------------------------

    /// A chamber was activated by `origin` at `t`: advance coupled foci, reset automatic ones.
    fn notify_foci(&mut self, t: f64, chamber: Chamber, origin: Origin) {
        for id in 0..self.params.foci.len() {
            let f = &self.params.foci[id];
            let focus_chamber = match f.region {
                FocusRegion::RightAtrium | FocusRegion::LeftAtrium => Chamber::Atrium,
                FocusRegion::Junctional => Chamber::AvJunction,
                FocusRegion::RightVentricle | FocusRegion::LeftVentricle => Chamber::Ventricle,
            };
            if focus_chamber != chamber || origin == Origin::Focus(id as u8) {
                continue;
            }
            let st = &mut self.foci[id];
            if st.terminated || (f.max_fires > 0 && st.fires >= f.max_fires) {
                continue;
            }
            match f.mode {
                FocusMode::Coupled {
                    coupling_ms,
                    every_n_beats,
                    count,
                    ..
                } => {
                    st.beats += 1;
                    if st.beats >= every_n_beats.max(1) {
                        st.beats = 0;
                        let remaining = count.max(1);
                        self.schedule(
                            t + coupling_ms,
                            Pending::FocusFire {
                                id: id as u8,
                                remaining,
                            },
                        );
                    }
                }
                FocusMode::Automatic {
                    rate_bpm,
                    entrance_block,
                    ..
                } => {
                    if !entrance_block && rate_bpm > 0.0 {
                        self.unschedule(
                            |p| matches!(p, Pending::FocusFire { id: fid, .. } if *fid == id as u8),
                        );
                        self.schedule(
                            t + 60_000.0 / self.automatic_rate(rate_bpm),
                            Pending::FocusFire {
                                id: id as u8,
                                remaining: 1,
                            },
                        );
                    }
                }
            }
        }
    }

    fn on_focus_fire(&mut self, t: f64, id: u8, remaining: u32) {
        let Some(f) = self.params.foci.get(id as usize).cloned() else {
            return;
        };
        let st = &mut self.foci[id as usize];
        if st.terminated || (f.max_fires > 0 && st.fires >= f.max_fires) {
            return;
        }
        st.fires += 1;
        let fires = st.fires;
        let origin = Origin::Focus(id);
        let region = f.region.region();

        let exit_blocked = matches!(f.mode, FocusMode::Automatic { exit_block_ratio, .. }
            if exit_block_ratio > 1 && fires.is_multiple_of(exit_block_ratio));
        if exit_blocked {
            self.emit(t, region, EventKind::Blocked, origin);
        } else {
            match f.region {
                FocusRegion::RightAtrium | FocusRegion::LeftAtrium => {
                    self.activate_atrium(t, region, EventKind::Automatic, origin, AtrialVia::Focus);
                }
                FocusRegion::Junctional => {
                    self.emit(t, Region::AvNodeCompact, EventKind::Automatic, origin);
                    self.on_his(t, origin, HisVia::Junctional);
                }
                FocusRegion::RightVentricle | FocusRegion::LeftVentricle => {
                    self.activate_ventricle(t, region, EventKind::Automatic, origin, VentVia::Focus);
                }
            }
        }

        match f.mode {
            FocusMode::Coupled { run_cycle_ms, .. } if remaining > 1 => {
                self.schedule(
                    t + run_cycle_ms,
                    Pending::FocusFire {
                        id,
                        remaining: remaining - 1,
                    },
                );
            }
            FocusMode::Automatic { rate_bpm, .. } if rate_bpm > 0.0 => {
                self.schedule(
                    t + 60_000.0 / self.automatic_rate(rate_bpm),
                    Pending::FocusFire { id, remaining: 1 },
                );
            }
            _ => {}
        }
    }
}
