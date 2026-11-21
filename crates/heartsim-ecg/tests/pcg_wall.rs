//! Acceptance tests of brief B25 for the wall-time synthesiser (slow-motion listening):
//!
//! * at speed 1 it equals `PcgSynth` sample for sample (sinus, RBBB, biventricular pacing);
//! * at 0.25× the A2–P2 interval of an RBBB beat is four times the 1× interval, within one sample,
//!   while every atom keeps its pitch: its zero crossings over the first 20 ms fall where the
//!   chirp's own phase puts them.

use heartsim_core::Millis;
use heartsim_ecg::Tier0Scenario;
use heartsim_ecg::pcg::{AtomParams, ComponentKind, PCG_PERIOD_MS, PcgParams, WallStart, WallTimeSynth};
use heartsim_ecg::scenario::STEP_MS;
use heartsim_sim::{Engine, TimingEngine};
use std::path::PathBuf;

fn scenario(stem: &str) -> Tier0Scenario {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../assets/scenarios/{stem}.ron"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    Tier0Scenario::from_ron(&text).unwrap()
}

fn wall_synth(s: &Tier0Scenario) -> WallTimeSynth {
    WallTimeSynth::new(
        PcgParams::default(),
        s.scenario.seed,
        s.timing.sinus.respiration_rate_bpm,
    )
}

/// Run `s` for `seconds` of simulation time, feeding `PcgSynth` (sampled as the recording and the
/// app ring do) and the wall-time synthesiser at `speed` (rendered after every step). Returns both
/// outputs and the wall-time starts.
fn run(s: &Tier0Scenario, seconds: f64, speed: f64) -> (Vec<f32>, Vec<f32>, Vec<WallStart>) {
    let mut eng = TimingEngine::new(s.timing.clone());
    eng.set_shock_seed(s.scenario.seed);
    let mut timeline = s.scenario.timeline.clone();
    timeline.sort_by(|a, b| a.at.0.total_cmp(&b.at.0));
    let mut next = 0;
    let mut pcg = s.pcg_synth();
    let mut wall = wall_synth(s);
    wall.set_speed(speed);
    wall.record_starts(true);
    let per_step = (STEP_MS / PCG_PERIOD_MS).round() as usize;
    let (mut trace, mut sound, mut starts) = (Vec::new(), Vec::new(), Vec::new());
    for _ in 0..(seconds * 1000.0 / STEP_MS).round() as usize {
        while let Some(a) = timeline.get(next)
            && a.at.0 <= eng.now().0
        {
            eng.apply(&a.action, a.at).unwrap();
            next += 1;
        }
        eng.step(Millis(STEP_MS));
        let evs = eng.drain_events();
        pcg.on_events(&evs);
        wall.on_events(&evs);
        for i in (0..per_step).rev() {
            trace.push(pcg.sample(Millis(eng.now().0 - i as f64 * PCG_PERIOD_MS)));
        }
        wall.render_until_sim(eng.now().0, &mut sound);
        starts.extend(wall.take_starts());
    }
    (trace, sound, starts)
}

#[test]
fn at_speed_one_the_wall_time_synth_equals_the_trace_sample_for_sample() {
    for stem in ["sinus_normal", "rbbb", "paced_biv"] {
        let s = scenario(stem);
        let (trace, sound, starts) = run(&s, 12.0, 1.0);
        assert_eq!(trace.len(), sound.len(), "{stem}: as many samples");
        let first_diff = trace
            .iter()
            .zip(&sound)
            .position(|(a, b)| a.to_bits() != b.to_bits());
        assert_eq!(first_diff, None, "{stem}: differs at sample {first_diff:?}");
        assert!(trace.iter().any(|v| v.abs() > 0.5), "{stem}: there are sounds");
        // at 1× the wall onsets are the simulation onsets
        assert!(
            starts
                .iter()
                .all(|st| st.wall_ms.to_bits() == st.sim_ms.to_bits())
        );
    }
}

/// `(A2, P2)` wall onsets of every beat with both, matched by simulation onset.
fn splits(starts: &[WallStart]) -> Vec<(f64, f64, f64)> {
    let a2: Vec<&WallStart> = starts.iter().filter(|s| s.kind == ComponentKind::A2).collect();
    let p2: Vec<&WallStart> = starts.iter().filter(|s| s.kind == ComponentKind::P2).collect();
    a2.iter()
        .filter_map(|a| {
            // P2 of the same beat: the first P2 within 150 ms of simulation time
            let p = p2.iter().find(|p| (p.sim_ms - a.sim_ms).abs() < 150.0)?;
            Some((a.sim_ms, a.wall_ms, p.wall_ms))
        })
        .collect()
}

/// `f(τ) = a + b / √τ` of an atom (the definition in `pcg`), and the chirp phase in cycles.
fn phase_cycles(p: AtomParams, tau_ms: f64) -> f64 {
    let d = p.duration_ms.max(1.0 + 1e-6);
    let b = (p.f_start_hz - p.f_end_hz) / (1.0 - 1.0 / d.sqrt());
    let a = p.f_start_hz - b;
    (a * tau_ms + 2.0 * b * tau_ms.sqrt()) / 1000.0
}

/// Times (ms after onset) at which the atom's carrier crosses zero within `(0, until_ms)`.
fn analytic_crossings(p: AtomParams, until_ms: f64) -> Vec<f64> {
    let mut out = Vec::new();
    let mut n = 1.0;
    while phase_cycles(p, until_ms) * 2.0 > n {
        // phase is increasing: bisection for 2·phase = n
        let (mut lo, mut hi) = (0.0, until_ms);
        for _ in 0..60 {
            let mid = f64::midpoint(lo, hi);
            if phase_cycles(p, mid) * 2.0 < n {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        out.push(f64::midpoint(lo, hi));
        n += 1.0;
    }
    out
}

/// Zero crossings (linearly interpolated, ms after `onset_ms`) of `sound` within the window.
fn measured_crossings(sound: &[f32], onset_ms: f64, until_ms: f64) -> Vec<f64> {
    let t = |i: usize| (i + 1) as f64 * PCG_PERIOD_MS;
    let first = ((onset_ms / PCG_PERIOD_MS).ceil() as usize).saturating_sub(1);
    let mut out = Vec::new();
    for i in first..sound.len() - 1 {
        if t(i) - onset_ms > until_ms {
            break;
        }
        let (a, b) = (f64::from(sound[i]), f64::from(sound[i + 1]));
        if a != 0.0 && b != 0.0 && (a < 0.0) != (b < 0.0) {
            let at = t(i) + PCG_PERIOD_MS * a / (a - b);
            if at > onset_ms + 0.5 {
                out.push(at - onset_ms);
            }
        }
    }
    out
}

#[test]
fn slow_motion_stretches_the_split_and_keeps_the_pitch() {
    let s = scenario("rbbb");
    let (_, _, normal) = run(&s, 10.0, 1.0);
    let (_, slow_sound, slow) = run(&s, 3.0, 0.25);
    let normal = splits(&normal);
    let slow_splits = splits(&slow);
    assert!(
        slow_splits.len() >= 2,
        "{} split beats at 0.25×",
        slow_splits.len()
    );
    for (sim, a_wall, p_wall) in &slow_splits {
        let (_, a1, p1) = normal
            .iter()
            .find(|(s1, _, _)| (s1 - sim).abs() < 1e-9)
            .expect("the same beat at 1×");
        let at_one = p1 - a1;
        assert!(at_one > 30.0, "RBBB splits S2 ({at_one} ms)");
        let stretched = p_wall - a_wall;
        assert!(
            (stretched - 4.0 * at_one).abs() <= PCG_PERIOD_MS,
            "split {stretched} ms at 0.25×, {at_one} ms at 1×"
        );
    }
    // every atom keeps its natural carrier: the zero crossings of its first 20 ms are where the
    // chirp's own phase puts them (at 0.25× the components are far apart, so each is heard alone)
    let params = PcgParams::default();
    let mut checked = 0;
    for st in slow
        .iter()
        .filter(|st| st.wall_ms + 25.0 < slow_sound.len() as f64 * PCG_PERIOD_MS)
    {
        let atom = st.kind.atom(&params);
        let want = analytic_crossings(atom, 19.0);
        let got = measured_crossings(&slow_sound, st.wall_ms, 20.0);
        assert!(want.len() >= 2, "{:?}: {} crossings", st.kind, want.len());
        for w in &want {
            assert!(
                got.iter().any(|g| (g - w).abs() < 0.1),
                "{:?} at {:.1} ms: crossing at {w:.2} ms missing from {got:?}",
                st.kind,
                st.wall_ms
            );
        }
        // and nothing in between: as many crossings as the natural atom (± the window edge)
        assert!(got.len() <= want.len() + 1, "{:?}: {got:?} vs {want:?}", st.kind);
        checked += 1;
    }
    assert!(checked >= 8, "only {checked} atoms checked");
}

#[test]
fn a_speed_change_moves_only_the_components_not_yet_started() {
    let s = scenario("sinus_normal");
    let mut eng = TimingEngine::new(s.timing.clone());
    let mut wall = wall_synth(&s);
    wall.record_starts(true);
    let mut out = Vec::new();
    let mut starts = Vec::new();
    // run at 1× until M1 of the second beat has started, then slow down to 0.5×
    let mut switched = None;
    for _ in 0..3000 {
        eng.step(Millis(1.0));
        wall.on_events(&eng.drain_events());
        wall.render_until_sim(eng.now().0, &mut out);
        starts.extend(wall.take_starts());
        let m1s = starts.iter().filter(|s| s.kind == ComponentKind::M1).count();
        if switched.is_none() && m1s == 2 {
            switched = Some(eng.now().0);
            wall.set_speed(0.5);
        }
    }
    let t = switched.expect("two beats");
    let (before, after): (Vec<&WallStart>, Vec<&WallStart>) = starts.iter().partition(|s| s.sim_ms <= t);
    assert!(
        before.iter().all(|s| s.wall_ms.to_bits() == s.sim_ms.to_bits()),
        "kept"
    );
    assert!(!after.is_empty(), "components after the switch");
    for s in after {
        assert!(
            (s.wall_ms - (t + 2.0 * (s.sim_ms - t))).abs() < PCG_PERIOD_MS,
            "{s:?} after a switch at {t}"
        );
    }
}

#[test]
fn following_without_sound_keeps_the_sounds_in_progress() {
    // a synth that followed the simulation at 1× and then slows down sounds like one that rendered
    // all along: the switch is seamless
    let s = scenario("sinus_normal");
    let mut eng = TimingEngine::new(s.timing.clone());
    let (mut rendered, mut followed) = (wall_synth(&s), wall_synth(&s));
    let (mut a, mut b) = (Vec::new(), Vec::new());
    for k in 0..4000 {
        eng.step(Millis(1.0));
        let ev = eng.drain_events();
        rendered.on_events(&ev);
        followed.on_events(&ev);
        if k == 2000 {
            rendered.set_speed(0.5);
            followed.set_speed(0.5);
        }
        rendered.render_until_sim(eng.now().0, &mut a);
        if k < 2000 {
            a.clear();
            followed.follow(eng.now().0);
        } else {
            followed.render_until_sim(eng.now().0, &mut b);
        }
    }
    assert_eq!(a.len(), b.len());
    assert_eq!(a, b);
    assert!(followed.active_count() < 16, "finished components are retired");
}
