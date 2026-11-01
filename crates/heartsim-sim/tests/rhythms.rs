//! Acceptance tests for brief B01 (timing-engine extensions). Each rhythm runs 30 s at 1 ms and
//! is checked on event statistics only — no ECG involved.

use heartsim_core::scenario::{Action, StimSite};
use heartsim_core::{Millis, Region};
use heartsim_sim::timing::{
    AccessoryPathway, Focus, FocusMode, FocusRegion, PacerConfig, PacerMode, Side, TimingParams,
};
use heartsim_sim::{Chamber, Engine, Event, EventKind, Origin, TimingEngine};

const SECONDS: f64 = 30.0;

/// Run `seconds` at 1 ms, applying each `(at_ms, action)` when the clock reaches it.
fn run_with(params: TimingParams, seconds: f64, actions: &[(f64, Action)]) -> Vec<Event> {
    let mut e = TimingEngine::new(params);
    let mut all = Vec::new();
    let mut next_action = 0;
    let steps = (seconds * 1000.0) as usize;
    for _ in 0..steps {
        while let Some((at, action)) = actions.get(next_action)
            && *at <= e.now().0
        {
            e.apply(action, Millis(*at)).expect("action accepted");
            next_action += 1;
        }
        e.step(Millis(1.0));
        all.extend(e.drain_events());
    }
    all
}

fn run(params: TimingParams, seconds: f64) -> Vec<Event> {
    run_with(params, seconds, &[])
}

fn active(e: &Event) -> bool {
    e.kind != EventKind::Blocked
}

/// Atrial activations (P waves).
fn p_times(evs: &[Event]) -> Vec<f64> {
    evs.iter()
        .filter(|e| e.chamber == Chamber::Atrium && active(e))
        .map(|e| e.at.0)
        .collect()
}

/// Ventricular myocardium activations (QRS onsets), fusion duplicates removed.
fn v_times(evs: &[Event]) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    for e in evs
        .iter()
        .filter(|e| e.chamber == Chamber::Ventricle && active(e))
    {
        if out.last().is_none_or(|l| e.at.0 - l > 150.0) {
            out.push(e.at.0);
        }
    }
    out
}

fn intervals(t: &[f64]) -> Vec<f64> {
    t.windows(2).map(|w| w[1] - w[0]).collect()
}

fn mean(x: &[f64]) -> f64 {
    x.iter().sum::<f64>() / x.len() as f64
}

fn cv(x: &[f64]) -> f64 {
    let m = mean(x);
    let var = x.iter().map(|v| (v - m).powi(2)).sum::<f64>() / x.len() as f64;
    var.sqrt() / m
}

fn count(evs: &[Event], pred: impl Fn(&Event) -> bool) -> usize {
    evs.iter().filter(|e| pred(e)).count()
}

/// Stimulate the right atrium at `at_ms`.
fn pac_at(at_ms: f64) -> (f64, Action) {
    (
        at_ms,
        Action::Stimulate {
            site: StimSite::Landmark("ra_appendage".into()),
            strength: 1.0,
            duration_ms: 2.0,
        },
    )
}

// ---------------------------------------------------------------------------------------------

#[test]
fn sinus_70_is_regular_with_normal_pr() {
    let evs = run(TimingParams::default(), SECONDS);
    let p = p_times(&evs);
    let v = v_times(&evs);
    assert!((34..=36).contains(&p.len()), "{} P waves", p.len());
    assert!((34..=36).contains(&v.len()), "{} QRS", v.len());
    for (p, v) in p.iter().zip(&v) {
        let pr = v - p;
        assert!((120.0..=200.0).contains(&pr), "PR {pr}");
    }
    assert!(cv(&intervals(&v)) < 0.01, "RR CV {}", cv(&intervals(&v)));
}

#[test]
fn sinus_arrhythmia_follows_respiration() {
    let mut p = TimingParams::default();
    p.sinus.respiration_depth = 0.1;
    p.sinus.respiration_rate_bpm = 12.0;
    let evs = run(p, SECONDS);
    let pt = p_times(&evs);
    let rr = intervals(&pt);
    let max = rr.iter().copied().fold(f64::MIN, f64::max);
    let min = rr.iter().copied().fold(f64::MAX, f64::min);
    assert!((1.15..=1.25).contains(&(max / min)), "RR max/min {}", max / min);
    // Synchronous with respiration: RR(t) = CL·(1 − a·sin φ(t)) ⇒ RR correlates negatively with sin φ.
    let phase: Vec<f64> = pt[..rr.len()]
        .iter()
        .map(|t| (t / 1000.0 * 12.0 / 60.0 * std::f64::consts::TAU).sin())
        .collect();
    let (mr, mp) = (mean(&rr), mean(&phase));
    let cov: f64 = rr.iter().zip(&phase).map(|(r, s)| (r - mr) * (s - mp)).sum();
    let sr: f64 = rr.iter().map(|r| (r - mr).powi(2)).sum::<f64>().sqrt();
    let sp: f64 = phase.iter().map(|s| (s - mp).powi(2)).sum::<f64>().sqrt();
    let corr = cov / (sr * sp);
    assert!(corr < -0.8, "RR vs respiration correlation {corr}");
}

#[test]
fn pac_every_fifth_beat_resets_sinus() {
    let mut p = TimingParams::default();
    p.foci.push(Focus::pac(500.0, 5));
    let evs = run(p, SECONDS);
    let pacs: Vec<f64> = evs
        .iter()
        .filter(|e| {
            e.chamber == Chamber::Atrium && e.kind == EventKind::Automatic && e.origin == Origin::Focus(0)
        })
        .map(|e| e.at.0)
        .collect();
    assert_eq!(pacs.len(), 6, "PACs: {pacs:?}");
    let sinus: Vec<f64> = evs
        .iter()
        .filter(|e| e.chamber == Chamber::Atrium && active(e) && e.origin == Origin::Sinus)
        .map(|e| e.at.0)
        .collect();
    let cl = 60_000.0 / 70.0;
    for pac in &pacs {
        let before = sinus
            .iter()
            .filter(|t| **t < *pac)
            .copied()
            .fold(f64::MIN, f64::max);
        let after = sinus
            .iter()
            .filter(|t| **t > *pac)
            .copied()
            .fold(f64::MAX, f64::min);
        // non-compensatory pause: the sinus clock restarts from the PAC
        assert!(
            (after - pac - cl).abs() < 2.0,
            "sinus after PAC at {}",
            after - pac
        );
        assert!(after - before > cl + 100.0 && after - before < 2.0 * cl - 100.0);
    }
    // every PAC conducts
    assert_eq!(v_times(&evs).len(), p_times(&evs).len());
}

#[test]
fn pvc_bigeminy_doubles_ventricular_count_with_compensatory_pause() {
    let mut p = TimingParams::default();
    p.foci.push(Focus::pvc(620.0, 1));
    let evs = run(p, SECONDS);
    let conducted_p = count(&evs, |e| e.chamber == Chamber::Atrium && active(e))
        - count(&evs, |e| {
            e.region == Region::AvNodeCompact && e.kind == EventKind::Blocked
        });
    let pvcs = count(&evs, |e| {
        e.origin == Origin::Focus(0) && e.chamber == Chamber::Ventricle && active(e)
    });
    let v = v_times(&evs);
    assert!(pvcs >= 10, "{pvcs} PVCs");
    assert!(
        v.len().abs_diff(2 * conducted_p) <= 2,
        "V={} conducted P={conducted_p}",
        v.len()
    );
    // compensatory pause: conducted QRS → PVC → conducted QRS spans two sinus cycles
    let cl = 60_000.0 / 70.0;
    let sinus_v: Vec<f64> = evs
        .iter()
        .filter(|e| e.chamber == Chamber::Ventricle && active(e) && e.origin == Origin::Sinus)
        .map(|e| e.at.0)
        .collect();
    let gaps = intervals(&sinus_v);
    assert!(gaps.len() >= 10);
    for g in &gaps[1..] {
        assert!(
            (g - 2.0 * cl).abs() < 5.0,
            "sinus QRS gap {g}, expected {}",
            2.0 * cl
        );
    }
    assert!(
        count(&evs, |e| e.region == Region::AvNodeCompact
            && e.kind == EventKind::Blocked)
            >= 10
    );
}

#[test]
fn wenckebach_prolongs_pr_then_drops() {
    let mut p = TimingParams::default();
    p.sinus.rate_bpm = 110.0;
    p.av.fast.min_ms = 110.0;
    p.av.fast.recovery_amplitude_ms = 200.0;
    p.av.fast.recovery_tau_ms = 150.0;
    p.av.fast.erp_ms = 300.0;
    let evs = run(p, SECONDS);
    let p_n = p_times(&evs).len();
    let v_n = v_times(&evs).len();
    let ratio = p_n as f64 / v_n as f64;
    assert!((1.25..=1.5).contains(&ratio), "P:V = {p_n}:{v_n} ({ratio})");
    // PR lengthens inside each group and is shortest right after the dropped beat.
    let mut groups: Vec<Vec<f64>> = vec![Vec::new()];
    let mut last_p = None;
    for e in &evs {
        match (e.chamber, e.kind) {
            (Chamber::Atrium, k) if k != EventKind::Blocked => last_p = Some(e.at.0),
            (Chamber::Ventricle, k) if k != EventKind::Blocked => {
                if let Some(p) = last_p.take() {
                    groups.last_mut().unwrap().push(e.at.0 - p);
                }
            }
            _ if e.region == Region::AvNodeCompact && e.kind == EventKind::Blocked => groups.push(Vec::new()),
            _ => {}
        }
    }
    let full: Vec<&Vec<f64>> = groups.iter().filter(|g| g.len() >= 2).collect();
    assert!(full.len() >= 5, "groups: {groups:?}");
    for g in &full[1..] {
        assert!(
            g.windows(2).all(|w| w[1] > w[0] + 5.0),
            "PR not progressive: {g:?}"
        );
        assert!(g[0] < 200.0 && *g.last().unwrap() > 250.0, "PR range {g:?}");
    }
}

#[test]
fn mobitz2_and_complete_block_with_escapes() {
    // Mobitz II: all-or-none drop below the node, PR constant.
    let mut p = TimingParams::default();
    p.av.mobitz2_ratio = 3;
    let evs = run(p, SECONDS);
    let (pn, vn) = (p_times(&evs).len(), v_times(&evs).len());
    assert!((pn as f64 / vn as f64 - 1.5).abs() < 0.1, "P:V {pn}:{vn}");
    assert!(
        count(&evs, |e| e.region == Region::HisBundle
            && e.kind == EventKind::Blocked)
            >= 10
    );

    // Complete block: junctional 45 beats ventricular 30.
    let mut p = TimingParams::default();
    p.av.complete_block = true;
    p.escape.junctional_bpm = 45.0;
    p.escape.ventricular_bpm = 30.0;
    let evs = run(p, SECONDS);
    let v = v_times(&evs);
    assert!((21..=23).contains(&v.len()), "{} escape beats", v.len());
    assert!(
        evs.iter()
            .filter(|e| e.chamber == Chamber::Ventricle)
            .all(|e| e.origin == Origin::Escape)
    );
    assert_eq!(
        count(&evs, |e| e.region == Region::AvNodeCompact
            && e.kind == EventKind::Automatic),
        v.len()
    );
    assert_eq!(
        count(&evs, |e| e.chamber == Chamber::Ventricle
            && e.kind == EventKind::Automatic),
        0
    );
    assert!(
        (34..=36).contains(&p_times(&evs).len()),
        "atria keep going independently"
    );

    // Without a junctional pacemaker the ventricular focus takes over at 30.
    let mut p = TimingParams::default();
    p.av.complete_block = true;
    p.escape.junctional_bpm = 0.0;
    let evs = run(p, SECONDS);
    let v = v_times(&evs);
    assert!((14..=16).contains(&v.len()), "{} idioventricular beats", v.len());
    assert!(
        evs.iter()
            .filter(|e| e.chamber == Chamber::Ventricle)
            .all(|e| e.kind == EventKind::Automatic)
    );
}

#[test]
fn avnrt_starts_with_a_pac_and_stops_with_adenosine() {
    let mut p = TimingParams::default();
    p.av.dual_pathways = true;
    p.av.retrograde_fast = true;
    p.av.fast.erp_ms = 250.0; // long-ERP fast pathway (the AVNRT substrate)
    // sinus beats at 200 + k·857 ms; PAC 300 ms after the 4th
    let pac_t = 200.0 + 3.0 * 60_000.0 / 70.0 + 300.0;
    let adenosine_t = 15_000.0;
    let evs = run_with(
        p,
        SECONDS,
        &[
            pac_at(pac_t),
            (
                adenosine_t,
                Action::Drug {
                    name: "adenosine".into(),
                    dose: 1.0,
                },
            ),
        ],
    );
    let v = v_times(&evs);
    let tachy: Vec<f64> = v
        .iter()
        .copied()
        .filter(|t| *t > pac_t + 500.0 && *t < adenosine_t)
        .collect();
    let rr = intervals(&tachy);
    assert!(
        tachy.last().unwrap() - tachy[0] >= 10_000.0,
        "tachycardia sustained < 10 s"
    );
    let rate = 60_000.0 / mean(&rr);
    assert!((150.0..=250.0).contains(&rate), "AVNRT rate {rate}");
    assert!(cv(&rr) < 0.02, "AVNRT must be regular, CV {}", cv(&rr));
    let echoes = count(&evs, |e| {
        e.chamber == Chamber::Atrium && active(e) && e.origin == Origin::Reentry
    });
    assert!(echoes >= 25, "{echoes} retrograde P waves");
    // every tachycardia beat went down the slow pathway
    assert!(
        count(&evs, |e| e.region == Region::AvNodeSlowPathway
            && e.kind == EventKind::Conducted)
            >= 25
    );
    // adenosine: loop broken, sinus rhythm back within 3 s
    let after: Vec<f64> = v.iter().copied().filter(|t| *t > adenosine_t + 3000.0).collect();
    let rr_after = intervals(&after);
    assert!(
        rr_after.iter().all(|r| *r > 700.0),
        "post-adenosine RR {rr_after:?}"
    );
    assert_eq!(
        count(&evs, |e| e.at.0 > adenosine_t + 3000.0
            && e.origin == Origin::Reentry),
        0
    );
    assert!(
        count(&evs, |e| e.at.0 > adenosine_t + 3000.0
            && e.chamber == Chamber::Atrium
            && e.origin == Origin::Sinus)
            >= 10
    );
}

#[test]
fn wpw_pre_excitation_precedes_his_conduction() {
    let p = TimingParams {
        accessory: Some(AccessoryPathway {
            delay_ms: 30.0,
            erp_ms: 300.0,
            antegrade: true,
            retrograde: true,
            ventricular_side: Side::Left,
            atrial_side: Side::Left,
        }),
        ..TimingParams::default()
    };
    let expected = p.av.fast.min_ms + p.his_purkinje.total_ms() - 30.0;
    let evs = run(p, SECONDS);
    let pre: Vec<f64> = evs
        .iter()
        .filter(|e| e.chamber == Chamber::Ventricle && active(e) && e.origin == Origin::AccessoryPathway)
        .map(|e| e.at.0)
        .collect();
    let his: Vec<f64> = evs
        .iter()
        .filter(|e| e.chamber == Chamber::Ventricle && active(e) && e.origin == Origin::Sinus)
        .map(|e| e.at.0)
        .collect();
    assert!((34..=36).contains(&pre.len()));
    assert_eq!(pre.len(), his.len(), "every beat is a fusion of both wavefronts");
    for (a, b) in pre.iter().zip(&his) {
        assert!(
            (b - a - expected).abs() < 1.0,
            "pre-excitation lead {} expected {expected}",
            b - a
        );
    }
    // no echo in sinus rhythm: the pathway is refractory when the His front arrives
    assert_eq!(
        count(&evs, |e| e.chamber == Chamber::Atrium
            && e.origin == Origin::AccessoryPathway
            && active(e)),
        0
    );
}

#[test]
fn orthodromic_avrt_after_a_pac() {
    let p = TimingParams {
        atrial_erp_ms: 200.0,
        accessory: Some(AccessoryPathway {
            delay_ms: 30.0,
            erp_ms: 250.0,
            antegrade: true,
            retrograde: true,
            ventricular_side: Side::Left,
            atrial_side: Side::Left,
        }),
        ..TimingParams::default()
    };
    let pac_t = 200.0 + 3.0 * 60_000.0 / 70.0 + 230.0;
    let evs = run_with(p, SECONDS, &[pac_at(pac_t)]);
    let v = v_times(&evs);
    let tachy: Vec<f64> = v.iter().copied().filter(|t| *t > pac_t + 1000.0).collect();
    let rr = intervals(&tachy);
    assert!(
        tachy.len() > 40,
        "tachycardia not sustained: {} beats",
        tachy.len()
    );
    let rate = 60_000.0 / mean(&rr);
    assert!((150.0..=220.0).contains(&rate), "AVRT rate {rate}");
    // orthodromic: antegrade over the node (narrow QRS), retrograde over the pathway
    let late = |e: &&Event| e.at.0 > pac_t + 1000.0;
    assert!(
        evs.iter()
            .filter(late)
            .filter(|e| e.chamber == Chamber::Ventricle && active(e))
            .all(|e| e.origin == Origin::Reentry)
    );
    assert!(
        evs.iter()
            .filter(late)
            .filter(|e| e.chamber == Chamber::Atrium && active(e))
            .all(|e| e.origin == Origin::AccessoryPathway)
    );
    // retrograde P after the QRS (RP = pathway delay)
    let a_times: Vec<f64> = evs
        .iter()
        .filter(late)
        .filter(|e| e.chamber == Chamber::Atrium && active(e))
        .map(|e| e.at.0)
        .collect();
    for a in &a_times {
        let prev_v = tachy.iter().copied().filter(|t| *t < *a).fold(f64::MIN, f64::max);
        assert!((a - prev_v - 30.0).abs() < 1.0, "RP {}", a - prev_v);
    }
}

#[test]
fn vvi_fills_gaps_and_is_inhibited_by_faster_sinus() {
    let mut p = TimingParams::default();
    p.sinus.rate_bpm = 40.0;
    p.pacer = Some(PacerConfig::new(PacerMode::Vvi, 60.0));
    let evs = run(p, SECONDS);
    let paced = count(&evs, |e| {
        e.origin == Origin::Pacer(1) && e.kind == EventKind::Stimulus
    });
    assert!(paced >= 10, "{paced} ventricular paces");
    let v = v_times(&evs);
    assert!(v.len() >= 30, "ventricular rate held at ≥ 60: {} beats", v.len());
    assert!(
        intervals(&v).iter().all(|r| *r <= 1001.0),
        "no RR longer than the LRI"
    );
    assert_eq!(
        count(&evs, |e| e.origin == Origin::Pacer(0)),
        0,
        "VVI never paces the atrium"
    );

    let mut p = TimingParams::default();
    p.sinus.rate_bpm = 80.0;
    p.pacer = Some(PacerConfig::new(PacerMode::Vvi, 60.0));
    let evs = run(p, SECONDS);
    assert_eq!(
        count(&evs, |e| matches!(e.origin, Origin::Pacer(_))),
        0,
        "inhibited"
    );
}

#[test]
fn ddd_tracks_atria_up_to_the_upper_rate() {
    let mut p = TimingParams::default();
    p.sinus.rate_bpm = 100.0;
    p.av.complete_block = true;
    p.pacer = Some(PacerConfig::new(PacerMode::Ddd, 60.0));
    let evs = run(p, SECONDS);
    let pn = p_times(&evs).len();
    let paced: Vec<f64> = evs
        .iter()
        .filter(|e| e.origin == Origin::Pacer(1) && e.kind == EventKind::Stimulus)
        .map(|e| e.at.0)
        .collect();
    assert!(paced.len().abs_diff(pn) <= 2, "P={pn} paced V={}", paced.len());
    let pt = p_times(&evs);
    for v in &paced[1..] {
        let prev_p = pt.iter().copied().filter(|t| *t < *v).fold(f64::MIN, f64::max);
        assert!((v - prev_p - 150.0).abs() < 1.5, "AV delay {}", v - prev_p);
    }
    assert_eq!(
        count(&evs, |e| e.origin == Origin::Pacer(0)),
        0,
        "sinus faster than the lower rate"
    );

    // Above the upper rate the ventricle is paced at the URI (pacemaker Wenckebach).
    let mut p = TimingParams::default();
    p.sinus.rate_bpm = 150.0;
    p.av.complete_block = true;
    p.pacer = Some(PacerConfig::new(PacerMode::Ddd, 60.0));
    let evs = run(p, SECONDS);
    let paced: Vec<f64> = evs
        .iter()
        .filter(|e| e.origin == Origin::Pacer(1) && e.kind == EventKind::Stimulus)
        .map(|e| e.at.0)
        .collect();
    // Pacemaker Wenckebach / 2:1: the ventricular rate stays between half the atrial rate and
    // the upper rate (37–60 paces in 30 s), never faster than the URI.
    assert!(
        (37..=61).contains(&paced.len()),
        "{} paces at 150 bpm sinus, upper rate 120",
        paced.len()
    );
    assert!(
        intervals(&paced).iter().all(|r| *r >= 499.0),
        "never faster than the URI"
    );
    let mean_rr = mean(&intervals(&paced));
    assert!(mean_rr > 500.0 && mean_rr < 800.0, "mean paced RR {mean_rr}");
}

#[test]
fn pvarp_prevents_pacemaker_mediated_tachycardia() {
    // Classic PMT trigger: a PVC conducts retrogradely to the atria; if the retrograde P is
    // sensed (short PVARP) the device tracks it, paces the ventricle, which conducts
    // retrogradely again … an endless loop at the upper rate.
    let run_pvarp = |pvarp_ms: f64| {
        let mut p = TimingParams::default();
        p.av.retrograde_fast = true;
        p.av.retrograde_delay_ms = 150.0;
        p.foci.push(Focus::pvc(500.0, 5));
        p.pacer = Some(PacerConfig {
            pvarp_ms,
            av_delay_ms: 200.0,
            ..PacerConfig::new(PacerMode::Ddd, 60.0)
        });
        let evs = run(p, SECONDS);
        let retro = count(&evs, |e| {
            e.chamber == Chamber::Atrium && active(e) && e.origin == Origin::Focus(0)
        });
        let paced_v = count(&evs, |e| {
            e.origin == Origin::Pacer(1) && e.kind == EventKind::Stimulus
        });
        (retro, paced_v)
    };
    let (retro_long, v_long) = run_pvarp(300.0);
    assert!(
        retro_long >= 4,
        "PVCs conduct retrogradely to the atria: {retro_long}"
    );
    assert!(
        v_long <= 10,
        "PVARP 300: retrograde P not tracked, only lower-rate paces after the pause: {v_long}"
    );
    // (the coupled PVC focus keeps firing every 5th ventricular beat, replacing a pace)
    let (_, v_short) = run_pvarp(100.0);
    assert!(
        v_short >= 40,
        "PVARP 100: retrograde P tracked → PMT at the upper rate: {v_short} paces"
    );
}

#[test]
fn parasystole_and_runs() {
    // Parasystolic focus with entrance block keeps its own clock regardless of sinus beats.
    let mut p = TimingParams::default();
    p.foci.push(Focus::parasystole(50.0));
    let evs = run(p, SECONDS);
    let fires: Vec<f64> = evs
        .iter()
        .filter(|e| e.origin == Origin::Focus(0) && e.chamber == Chamber::Ventricle)
        .map(|e| e.at.0)
        .collect();
    assert!(
        (24..=26).contains(&fires.len()),
        "{} parasystolic discharges (manifest + blocked)",
        fires.len()
    );
    for r in intervals(&fires) {
        assert!((r - 1200.0).abs() < 0.5, "inter-ectopic interval {r}");
    }
    assert!(
        count(&evs, |e| e.origin == Origin::Focus(0)
            && e.kind == EventKind::Blocked)
            > 0,
        "some fall in refractory ventricle"
    );

    // Ventricular couplets every third beat.
    let mut p = TimingParams::default();
    p.foci.push(Focus {
        region: FocusRegion::RightVentricle,
        mode: FocusMode::Coupled {
            coupling_ms: 500.0,
            every_n_beats: 2,
            count: 2,
            run_cycle_ms: 400.0,
        },
        max_fires: 0,
    });
    let evs = run(p, SECONDS);
    let pvc: Vec<f64> = evs
        .iter()
        .filter(|e| e.origin == Origin::Focus(0) && active(e) && e.region == Region::RightVentricle)
        .map(|e| e.at.0)
        .collect();
    assert!(
        pvc.len() >= 10 && pvc.len().is_multiple_of(2),
        "{} PVCs in couplets",
        pvc.len()
    );
    assert!(pvc.chunks(2).all(|c| (c[1] - c[0] - 400.0).abs() < 0.5));
}

#[test]
fn sinus_pauses_sick_sinus_and_exit_block() {
    let mut p = TimingParams::default();
    p.sinus.pause_every_n = 5;
    p.sinus.pause_ms = 3000.0;
    let evs = run(p, SECONDS);
    let rr = intervals(&p_times(&evs));
    let pauses = rr.iter().filter(|r| (**r - 3000.0).abs() < 1.0).count();
    assert!(pauses >= 4, "{pauses} sinus pauses: {rr:?}");

    let mut p = TimingParams::default();
    p.sinus.exit_block_ratio = 4;
    let evs = run(p, SECONDS);
    let rr = intervals(&p_times(&evs));
    let cl = 60_000.0 / 70.0;
    assert!(
        rr.iter().filter(|r| (**r - 2.0 * cl).abs() < 1.0).count() >= 6,
        "exit block doubles the PP: {rr:?}"
    );
    assert!(
        count(&evs, |e| e.region == Region::SinoatrialNode
            && e.kind == EventKind::Blocked)
            >= 6
    );

    let mut p = TimingParams::default();
    p.sinus.tachy_brady_period_s = 10.0;
    p.sinus.tachy_rate_bpm = 120.0;
    p.sinus.brady_rate_bpm = 40.0;
    let evs = run(p, SECONDS);
    let pt = p_times(&evs);
    let early = pt.iter().filter(|t| **t < 10_000.0).count();
    let mid = pt.iter().filter(|t| **t >= 10_000.0 && **t < 20_000.0).count();
    assert!(early >= 19 && mid <= 8, "tachy {early} / brady {mid}");
}

#[test]
fn bundle_branch_block_edges_and_rate_dependent_block() {
    let mut p = TimingParams::default();
    p.his_purkinje.lbb.blocked = true;
    let evs = run(p, SECONDS);
    assert!(
        count(&evs, |e| e.region == Region::LeftBundleBranch
            && e.kind == EventKind::Blocked)
            >= 34
    );
    assert_eq!(
        count(&evs, |e| e.region == Region::LeftAnteriorFascicle),
        0,
        "fascicles never reached"
    );
    assert!(
        evs.iter()
            .filter(|e| e.chamber == Chamber::Ventricle && active(e))
            .all(|e| e.region == Region::RightVentricle)
    );
    assert!(
        count(&evs, |e| e.region == Region::RightBundleBranch
            && e.kind == EventKind::Conducted)
            >= 34
    );

    // Rate-dependent RBBB: normal at 70, blocked at 130.
    for (rate, blocked_expected) in [(70.0, false), (130.0, true)] {
        let mut p = TimingParams::default();
        p.sinus.rate_bpm = rate;
        p.his_purkinje.rbb.block_above_rate_bpm = 100.0;
        let evs = run(p, SECONDS);
        let blocked = count(&evs, |e| {
            e.region == Region::RightBundleBranch && e.kind == EventKind::Blocked
        });
        assert_eq!(
            blocked > 10,
            blocked_expected,
            "rate {rate}: {blocked} RBB blocks"
        );
    }
}

#[test]
fn junctional_rhythm_with_retrograde_p() {
    let mut p = TimingParams::default();
    p.sinus.rate_bpm = 30.0; // slow sinus, junction at 45 takes over
    p.av.retrograde_fast = true;
    p.av.retrograde_delay_ms = 120.0;
    let evs = run(p, SECONDS);
    let junctional = count(&evs, |e| {
        e.region == Region::AvNodeCompact && e.kind == EventKind::Automatic
    });
    assert!(junctional >= 15, "{junctional} junctional beats");
    let retro_p: Vec<&Event> = evs
        .iter()
        .filter(|e| e.chamber == Chamber::Atrium && active(e) && e.origin == Origin::Escape)
        .collect();
    assert!(retro_p.len() >= 15, "{} retrograde P waves", retro_p.len());
    let his: Vec<f64> = evs
        .iter()
        .filter(|e| e.region == Region::HisBundle && e.origin == Origin::Escape)
        .map(|e| e.at.0)
        .collect();
    for rp in &retro_p {
        let h = his
            .iter()
            .copied()
            .filter(|t| *t <= rp.at.0)
            .fold(f64::MIN, f64::max);
        assert!((rp.at.0 - h - 120.0).abs() < 0.5, "retro delay {}", rp.at.0 - h);
    }
}

#[test]
fn ladder_samples_cover_all_rows() {
    use heartsim_sim::timing::LadderDirection;
    let mut p = TimingParams::default();
    p.av.retrograde_fast = true;
    p.foci.push(Focus::pvc(620.0, 2));
    let mut e = TimingEngine::new(p);
    let mut rows = std::collections::BTreeSet::new();
    let mut retro = 0;
    let mut blocked = 0;
    for _ in 0..10_000 {
        e.step(Millis(1.0));
        for s in e.drain_ladder() {
            rows.insert(s.chamber);
            assert!(s.exit_ms >= s.enter_ms);
            retro += usize::from(s.direction == LadderDirection::Retrograde);
            blocked += usize::from(s.blocked);
        }
    }
    assert_eq!(rows.len(), 3);
    assert!(retro > 0 && blocked > 0);
}

#[test]
fn deterministic_and_ron_roundtrip() {
    let mut p = TimingParams::default();
    p.sinus.respiration_depth = 0.05;
    p.av.dual_pathways = true;
    p.av.retrograde_fast = true;
    p.foci.push(Focus::pac(500.0, 5));
    p.foci.push(Focus::parasystole(40.0));
    p.accessory = Some(AccessoryPathway::default());
    p.pacer = Some(PacerConfig::new(PacerMode::Ddi, 50.0));
    p.his_purkinje.rbb.block_above_rate_bpm = 140.0;
    let text = ron::ser::to_string_pretty(&p, ron::ser::PrettyConfig::default().struct_names(true)).unwrap();
    let back: TimingParams = ron::from_str(&text).unwrap();
    assert_eq!(p, back);
    // missing optional sections fall back to defaults
    let minimal: TimingParams = ron::from_str("(sinus: (rate_bpm: 55.0))").unwrap();
    assert_eq!(minimal.sinus.rate_bpm, 55.0);
    assert_eq!(minimal.av, TimingParams::default().av);

    let a = run(p.clone(), 10.0);
    let b = run(back, 10.0);
    assert_eq!(a, b);
}
