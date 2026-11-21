//! Acceptance tests of brief B27, part B (the pure side): listening sites, bell and diaphragm, and the
//! valve-highlight schedule.
//!
//! * the overview mix is today's signal, bit for bit, whatever chest piece is selected, in
//!   `PcgSynth` and in the wall-time synthesiser;
//! * at the aortic site A2 is louder than P2, and P2 is relatively louder at the pulmonic site;
//!   S1 is loudest at the apex; each murmur is loudest at its own site;
//! * the bell raises S3, S4 and the mitral rumble relative to A2 and the diaphragm lowers them;
//! * a site keeps the synthesiser deterministic and random-access (chunks, wall time at 1×);
//! * the valve highlights come from the schedule the sound is made from: which valve, when, for
//!   how long.

use heartsim_ecg::Tier0Scenario;
use heartsim_ecg::auscultation::{MurmurClass, SoundGains};
use heartsim_ecg::listening::{
    ChestPiece, Listening, ListeningChoice, ListeningSite, MURMUR_GLOW_LEVEL, SiteTable, VALVE_FLASH_MS,
    Valve, loudest_site, valve_glow_at, valve_highlights,
};
use heartsim_ecg::pcg::{ComponentKind, PCG_PERIOD_MS, PcgSynth};
use heartsim_ecg::scenario::STEP_MS;
use heartsim_sim::{Engine, TimingEngine};
use std::path::PathBuf;

fn scenario(stem: &str) -> Tier0Scenario {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../assets/scenarios/{stem}.ron"));
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{stem}: {e}"));
    Tier0Scenario::from_ron(&text).unwrap_or_else(|e| panic!("{stem}: {e}"))
}

fn choice(site: ListeningSite, chest: ChestPiece) -> ListeningChoice {
    ListeningChoice { site, chest }
}

/// Run `s` for `seconds` (timeline applied, as `Tier0Scenario::record`) with `listening`; the trace
/// and the synthesiser (every sound kept).
fn run(s: &Tier0Scenario, seconds: f64, listening: Option<&Listening>) -> (Vec<f32>, PcgSynth) {
    let mut eng = TimingEngine::new(s.timing.clone());
    eng.set_shock_seed(s.scenario.seed);
    let mut timeline = s.scenario.timeline.clone();
    timeline.sort_by(|a, b| a.at.0.total_cmp(&b.at.0));
    let mut next = 0;
    let mut synth = s.pcg_synth();
    synth.keep_beats(true);
    if let Some(l) = listening {
        synth.set_listening(l);
    }
    let mut out = Vec::new();
    for _ in 0..(seconds * 1000.0 / STEP_MS).round() as usize {
        while let Some(a) = timeline.get(next)
            && a.at.0 <= eng.now().0
        {
            eng.apply(&a.action, a.at).unwrap();
            next += 1;
        }
        eng.step(heartsim_core::Millis(STEP_MS));
        synth.on_events(&eng.drain_events());
        for i in (0..4).rev() {
            out.push(synth.sample(heartsim_core::Millis(eng.now().0 - f64::from(i) * PCG_PERIOD_MS)));
        }
    }
    (out, synth)
}

fn rms(v: &[f32]) -> f64 {
    (v.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>() / v.len().max(1) as f64).sqrt()
}

/// The listening of `c` with only the given components and murmur classes left audible (their site
/// gains and the chest piece kept).
fn isolated(c: ListeningChoice, kinds: &[ComponentKind], classes: &[MurmurClass]) -> Listening {
    let mut l = SiteTable::builtin().listening(c);
    for (i, k) in ComponentKind::ALL.iter().enumerate() {
        if !kinds.contains(k) {
            l.gains.components[i] = 0.0;
        }
    }
    for m in MurmurClass::ALL {
        if !classes.contains(&m) {
            l.gains.murmurs[m.index()] = 0.0;
        }
    }
    l
}

/// RMS of the trace of `stem` heard at `c` with only `kinds` and `classes` audible.
fn level(stem: &str, c: ListeningChoice, kinds: &[ComponentKind], classes: &[MurmurClass]) -> f64 {
    rms(&run(&scenario(stem), 6.0, Some(&isolated(c, kinds, classes))).0)
}

const OVERVIEW: ListeningChoice = ListeningChoice {
    site: ListeningSite::Overview,
    chest: ChestPiece::Diaphragm,
};

// --- the overview is today's signal ---------------------------------------------------------------

#[test]
fn the_overview_mix_equals_todays_signal_bit_for_bit() {
    let table = SiteTable::builtin();
    for chest in ChestPiece::ALL {
        let overview = table.listening(choice(ListeningSite::Overview, chest));
        assert_eq!(
            overview,
            Listening::overview(),
            "{chest:?}: no gain, no chest piece"
        );
    }
    for stem in [
        "sinus_normal",
        "aortic_stenosis_severe",
        "mitral_stenosis_sinus",
        "pericarditis",
    ] {
        let s = scenario(stem);
        let rec = s.record(6.0).unwrap();
        let (with, _) = run(
            &s,
            6.0,
            Some(&table.listening(choice(ListeningSite::Overview, ChestPiece::Bell))),
        );
        assert_eq!(rec.pcg.len(), with.len(), "{stem}");
        assert!(
            rec.pcg.iter().zip(&with).all(|(a, b)| a.to_bits() == b.to_bits()),
            "{stem}: the overview changed the trace"
        );
        // and a real site does change it
        let (apex, _) = run(
            &s,
            6.0,
            Some(&table.listening(choice(ListeningSite::Mitral, ChestPiece::Bell))),
        );
        assert_ne!(rec.pcg, apex, "{stem}");
    }
}

// --- sites ------------------------------------------------------------------------------------------

#[test]
fn a2_dominates_at_the_aortic_site_and_p2_is_relatively_louder_at_the_pulmonic_site() {
    use ComponentKind::{A2, P2};
    for chest in ChestPiece::ALL {
        let aortic = choice(ListeningSite::Aortic, chest);
        let pulmonic = choice(ListeningSite::Pulmonic, chest);
        let (a2_ao, p2_ao) = (
            level("sinus_normal", aortic, &[A2], &[]),
            level("sinus_normal", aortic, &[P2], &[]),
        );
        let (a2_pu, p2_pu) = (
            level("sinus_normal", pulmonic, &[A2], &[]),
            level("sinus_normal", pulmonic, &[P2], &[]),
        );
        assert!(
            a2_ao > 2.0 * p2_ao,
            "{chest:?}: A2 {a2_ao} vs P2 {p2_ao} at the aortic site"
        );
        assert!(
            p2_pu / a2_pu > 2.0 * (p2_ao / a2_ao),
            "{chest:?}: P2/A2 {} at the pulmonic site vs {} at the aortic site",
            p2_pu / a2_pu,
            p2_ao / a2_ao
        );
    }
}

#[test]
fn s1_is_loudest_at_the_apex() {
    use ComponentKind::{M1, T1};
    for chest in ChestPiece::ALL {
        let at = |site| level("sinus_normal", choice(site, chest), &[M1, T1], &[]);
        let apex = at(ListeningSite::Mitral);
        for site in ListeningSite::SITES {
            if site != ListeningSite::Mitral {
                assert!(apex > at(site), "{chest:?}: S1 at {site:?} ≥ at the apex");
            }
        }
    }
}

#[test]
fn each_murmur_is_loudest_at_its_own_site() {
    // the table itself, for every class (the rub and the presystolic accent included)
    let table = SiteTable::builtin();
    for class in MurmurClass::ALL {
        let home = loudest_site(class);
        let g = |site| table.gains(site).murmurs[class.index()];
        for site in ListeningSite::SITES {
            if site != home {
                assert!(g(home) > g(site), "{class:?}: {site:?} ≥ {home:?}");
            }
        }
    }
    // and as heard: each lesion scenario's murmur alone at each site, with either chest piece
    for (stem, class) in [
        ("aortic_stenosis_severe", MurmurClass::AorticStenosis),
        ("pulmonary_stenosis", MurmurClass::PulmonaryStenosis),
        ("mitral_regurgitation", MurmurClass::MitralRegurgitation),
        ("tricuspid_regurgitation", MurmurClass::TricuspidRegurgitation),
        ("ventricular_septal_defect", MurmurClass::VentricularSeptalDefect),
        ("mitral_valve_prolapse", MurmurClass::ProlapseLateSystolic),
        ("aortic_regurgitation", MurmurClass::AorticRegurgitation),
        ("mitral_stenosis_af", MurmurClass::MitralRumble),
        ("atrial_septal_defect", MurmurClass::AtrialSeptalFlow),
        ("innocent_murmur", MurmurClass::InnocentMurmur),
    ] {
        for chest in ChestPiece::ALL {
            let levels: Vec<(ListeningSite, f64)> = ListeningSite::SITES
                .iter()
                .map(|&site| (site, level(stem, choice(site, chest), &[], &[class])))
                .collect();
            let (best, _) = levels
                .iter()
                .copied()
                .fold((ListeningSite::Overview, f64::MIN), |acc, x| {
                    if x.1 > acc.1 { x } else { acc }
                });
            assert_eq!(best, loudest_site(class), "{stem} {chest:?}: {levels:?}");
            assert!(levels.iter().all(|(_, v)| *v > 0.0), "{stem}: heard everywhere");
        }
    }
}

// --- bell and diaphragm -----------------------------------------------------------------------------

#[test]
fn the_bell_raises_s3_s4_and_the_rumble_relative_to_a2_and_the_diaphragm_lowers_them() {
    use ComponentKind::{A2, S3, S4};
    let apex = |chest| choice(ListeningSite::Mitral, chest);
    let bell = apex(ChestPiece::Bell);
    let dia = apex(ChestPiece::Diaphragm);
    // the same site without a chest piece: the site's gains only
    let flat = |kinds: &[ComponentKind], classes: &[MurmurClass]| {
        let mut l = isolated(bell, kinds, classes);
        l.chest = None;
        l
    };
    let ratio = |stem: &str, low: (&[ComponentKind], &[MurmurClass]), l: Option<ListeningChoice>| {
        let num = match l {
            Some(c) => level(stem, c, low.0, low.1),
            None => rms(&run(&scenario(stem), 6.0, Some(&flat(low.0, low.1))).0),
        };
        let den = match l {
            Some(c) => level(stem, c, &[A2], &[]),
            None => rms(&run(&scenario(stem), 6.0, Some(&flat(&[A2], &[]))).0),
        };
        num / den
    };
    for (stem, low) in [
        ("heart_failure_s3", (&[S3][..], &[][..])),
        ("lvh_s4", (&[S4][..], &[][..])),
        ("mitral_stenosis_af", (&[][..], &[MurmurClass::MitralRumble][..])),
    ] {
        let (b, n, d) = (
            ratio(stem, low, Some(bell)),
            ratio(stem, low, None),
            ratio(stem, low, Some(dia)),
        );
        assert!(b > 1.2 * n, "{stem}: bell {b} vs no chest piece {n}");
        assert!(d < 0.75 * n, "{stem}: diaphragm {d} vs no chest piece {n}");
        assert!(
            b > 1.8 * d,
            "{stem}: bell {b} vs diaphragm {d}: clearly easier with the bell"
        );
    }
}

// --- determinism with a site ------------------------------------------------------------------------

#[test]
fn a_site_keeps_the_trace_random_access_and_the_wall_time_synth_equal_at_1x() {
    let table = SiteTable::builtin();
    for (stem, c) in [
        (
            "summation_gallop",
            choice(ListeningSite::Mitral, ChestPiece::Bell),
        ),
        (
            "aortic_stenosis_severe",
            choice(ListeningSite::Aortic, ChestPiece::Diaphragm),
        ),
        (
            "mitral_stenosis_sinus",
            choice(ListeningSite::Mitral, ChestPiece::Bell),
        ),
    ] {
        let s = scenario(stem);
        let l = table.listening(c);
        let (whole, _) = run(&s, 6.0, Some(&l));
        let mut eng = TimingEngine::new(s.timing.clone());
        eng.set_shock_seed(s.scenario.seed);
        let mut synth = s.pcg_synth();
        synth.set_listening(&l);
        let mut wall = s.wall_synth();
        wall.set_listening(&l);
        let (mut chunked, mut pending, mut sound) = (Vec::new(), Vec::new(), Vec::new());
        for step in 0..6000 {
            eng.step(heartsim_core::Millis(STEP_MS));
            let evs = eng.drain_events();
            synth.on_events(&evs);
            wall.on_events(&evs);
            for i in (0..4).rev() {
                pending.push(eng.now().0 - f64::from(i) * PCG_PERIOD_MS);
            }
            if step % 23 == 22 {
                chunked.extend(pending.drain(..).map(|t| synth.sample(heartsim_core::Millis(t))));
            }
            wall.render_until_sim(eng.now().0, &mut sound);
        }
        chunked.extend(pending.drain(..).map(|t| synth.sample(heartsim_core::Millis(t))));
        let same = |a: &[f32], b: &[f32]| {
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
        };
        assert!(same(&whole, &chunked), "{stem}: chunks differ");
        assert!(same(&whole, &sound), "{stem}: wall time at 1× differs");
    }
}

#[test]
fn a_site_change_takes_effect_from_the_next_sample_only() {
    let s = scenario("mitral_regurgitation");
    let table = SiteTable::builtin();
    let (overview, _) = run(&s, 4.0, None);
    let apex = table.listening(choice(ListeningSite::Mitral, ChestPiece::Diaphragm));
    let mut eng = TimingEngine::new(s.timing.clone());
    eng.set_shock_seed(s.scenario.seed);
    let mut synth = s.pcg_synth();
    let mut out = Vec::new();
    for step in 0..4000 {
        if step == 2000 {
            synth.set_listening(&apex);
        }
        eng.step(heartsim_core::Millis(STEP_MS));
        synth.on_events(&eng.drain_events());
        for i in (0..4).rev() {
            out.push(synth.sample(heartsim_core::Millis(eng.now().0 - f64::from(i) * PCG_PERIOD_MS)));
        }
    }
    assert!(
        out[..8000]
            .iter()
            .zip(&overview[..8000])
            .all(|(a, b)| a.to_bits() == b.to_bits()),
        "before the change: the overview"
    );
    assert_ne!(&out[8000..], &overview[8000..]);
}

// --- valve highlights -------------------------------------------------------------------------------

#[test]
fn valve_highlights_follow_the_schedule_which_valve_when_and_for_how_long() {
    let (_, synth) = run(&scenario("aortic_stenosis_severe"), 6.0, None);
    let sounds = synth.scheduled_sounds();
    let murmurs = synth.scheduled_murmurs();
    let hl = valve_highlights(sounds, murmurs);
    // one flash per valve closure, at its onset, for VALVE_FLASH_MS, on its own valve
    for (kind, valve) in [
        (ComponentKind::M1, Valve::Mitral),
        (ComponentKind::T1, Valve::Tricuspid),
        (ComponentKind::A2, Valve::Aortic),
        (ComponentKind::P2, Valve::Pulmonic),
    ] {
        let onsets: Vec<f64> = sounds
            .iter()
            .filter(|s| s.kind == kind)
            .map(|s| s.onset_ms)
            .collect();
        let flashes: Vec<_> = hl.iter().filter(|h| h.flash && h.valve == valve).collect();
        assert_eq!(onsets.len(), flashes.len(), "{kind:?}");
        assert!(onsets.len() >= 5, "{kind:?}: {} beats", onsets.len());
        for (t, h) in onsets.iter().zip(&flashes) {
            assert_eq!(h.start_ms.to_bits(), t.to_bits());
            assert!((h.end_ms - h.start_ms - VALVE_FLASH_MS).abs() < 1e-9);
        }
    }
    // the stenotic valve glows for the length of its murmur, and nothing else glows
    let glows: Vec<_> = hl.iter().filter(|h| !h.flash).collect();
    let as_murmurs: Vec<_> = murmurs
        .iter()
        .filter(|m| m.class == MurmurClass::AorticStenosis)
        .collect();
    assert_eq!(glows.len(), as_murmurs.len());
    for (g, m) in glows.iter().zip(&as_murmurs) {
        assert_eq!(g.valve, Valve::Aortic);
        assert_eq!((g.start_ms, g.end_ms), (m.start_ms, m.stop_ms()));
        let mid = f64::midpoint(m.start_ms, m.stop_ms());
        let glow = valve_glow_at(sounds, murmurs, mid);
        assert!((f64::from(glow[Valve::Aortic.index()]) - MURMUR_GLOW_LEVEL).abs() < 1e-6);
        assert_eq!(
            glow[Valve::Mitral.index()],
            0.0,
            "mid-systole: the mitral flash is over"
        );
    }
    // at M1 the mitral ring is fully lit, half a flash later half lit
    let m1 = sounds
        .iter()
        .find(|s| s.kind == ComponentKind::M1)
        .unwrap()
        .onset_ms;
    assert!((valve_glow_at(sounds, murmurs, m1)[0] - 1.0).abs() < 1e-6);
    assert!((valve_glow_at(sounds, murmurs, m1 + VALVE_FLASH_MS / 2.0)[0] - 0.5).abs() < 1e-6);
}

#[test]
fn a_rhythm_without_lesions_flashes_the_closures_only_and_forgetting_keeps_what_is_still_lit() {
    let (_, mut synth) = run(&scenario("sinus_normal"), 5.0, None);
    assert_eq!(synth.scheduled_murmurs(), []);
    let hl = valve_highlights(synth.scheduled_sounds(), synth.scheduled_murmurs());
    assert!(hl.iter().all(|h| h.flash));
    let last_a2 = synth
        .scheduled_sounds()
        .iter()
        .rev()
        .find(|s| s.kind == ComponentKind::A2)
        .unwrap()
        .onset_ms;
    let before = valve_glow_at(
        synth.scheduled_sounds(),
        synth.scheduled_murmurs(),
        last_a2 + 10.0,
    );
    synth.forget_before(last_a2);
    let after = valve_glow_at(
        synth.scheduled_sounds(),
        synth.scheduled_murmurs(),
        last_a2 + 10.0,
    );
    assert_eq!(before, after);
    assert!(after[Valve::Aortic.index()] > 0.9);
    assert!(
        synth
            .scheduled_sounds()
            .iter()
            .all(|s| s.onset_ms + VALVE_FLASH_MS >= last_a2),
        "older sounds dropped"
    );
}

#[test]
fn site_gains_are_the_ones_the_synth_applies() {
    // the overview's gains are the identity of part A's `SoundGains`
    assert_eq!(
        SiteTable::builtin().gains(ListeningSite::Overview),
        SoundGains::default()
    );
    assert!(OVERVIEW.is_overview());
}
