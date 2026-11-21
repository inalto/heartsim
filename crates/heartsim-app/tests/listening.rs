//! B27 part B acceptance tests on the app side: the listening site and chest piece are persisted
//! (an older settings text gets the overview), the live heart sounds hear the chosen site (and the
//! overview leaves the ring as it was), the valve highlights come from the schedule the sound is
//! made from at the time the views are drawn, and the loudest site stays within full scale after the
//! B25 limiter. Headless, no GPU, no sound card.

use heartsim_app::heart_sounds::{HeartSounds, valve_display_time_ms};
use heartsim_app::settings::Settings;
use heartsim_app::sound_out::{HEADROOM_GAIN, LIMITER_KNEE, soft_limit};
use heartsim_core::Millis;
use heartsim_ecg::listening::{VALVE_FLASH_MS, Valve};
use heartsim_ecg::pcg::ComponentKind;
use heartsim_ecg::{ChestPiece, ListeningChoice, ListeningSite, SiteTable, Tier0Scenario};
use heartsim_sim::{Engine, TimingEngine};

fn scenario(stem: &str) -> Tier0Scenario {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../assets/scenarios/{stem}.ron"));
    Tier0Scenario::from_ron(&std::fs::read_to_string(path).unwrap()).unwrap()
}

const APEX_BELL: ListeningChoice = ListeningChoice {
    site: ListeningSite::Mitral,
    chest: ChestPiece::Bell,
};

#[test]
fn the_site_and_chest_piece_round_trip_and_old_settings_get_the_overview() {
    let s = Settings {
        listening_site: ListeningSite::Erb,
        chest_piece: ChestPiece::Bell,
        ..Settings::default()
    };
    let back: Settings = ron::from_str(&s.to_storage_string()).expect("valid RON");
    assert_eq!(back, s);
    assert_eq!(
        back.listening(),
        ListeningChoice {
            site: ListeningSite::Erb,
            chest: ChestPiece::Bell
        }
    );
    let old: Settings =
        ron::from_str("(language: Some(\"it\"), heart_sounds: true, heart_sound_audio: true)")
            .expect("0.18 settings");
    assert_eq!(old.listening_site, ListeningSite::Overview);
    assert_eq!(old.chest_piece, ChestPiece::Diaphragm);
    assert!(Settings::default().listening().is_overview());
}

/// The live loop in miniature, with `s`'s engine and section: the ring and the sounds.
fn live(s: &Tier0Scenario, seconds: f64, choice: ListeningChoice) -> HeartSounds {
    let mut sounds = HeartSounds::new(60.0);
    sounds.reset(
        s.scenario.seed,
        s.timing.sinus.respiration_rate_bpm,
        &s.scenario.auscultation,
    );
    sounds.set_listening(choice);
    let mut engine = TimingEngine::new(s.timing.clone());
    engine.set_shock_seed(s.scenario.seed);
    for k in 0..(seconds * 1000.0) as usize {
        engine.step(Millis(1.0));
        sounds.on_events(&engine.drain_events());
        if k % 2 == 1 {
            sounds.push_for_ecg_sample(engine.now());
        }
    }
    sounds
}

fn ring(sounds: &HeartSounds) -> Vec<f32> {
    sounds.ring().iter().collect()
}

#[test]
fn the_live_heart_sounds_hear_the_chosen_site() {
    let s = scenario("mitral_stenosis_sinus");
    let overview = live(&s, 6.0, ListeningChoice::default());
    // the overview ring is the scenario's own trace (the B24/B27 goldens' signal)
    let rec = s.record(6.0).unwrap();
    let ring_o = ring(&overview);
    let tail = &rec.pcg[rec.pcg.len() - ring_o.len()..];
    assert!(
        ring_o.iter().zip(tail).all(|(a, b)| a.to_bits() == b.to_bits()),
        "the overview changed the trace"
    );
    let apex = live(&s, 6.0, APEX_BELL);
    assert_eq!(apex.listening(), APEX_BELL);
    assert_ne!(ring(&apex), ring_o, "the site is heard");
    // the slow-motion synthesiser hears the same site
    let mut sounds = live(&s, 1.0, APEX_BELL);
    sounds.set_wall_active(true);
    assert!(sounds.wall_mut().is_some());
}

#[test]
fn valve_highlights_follow_the_heard_schedule_at_the_drawn_time() {
    let s = scenario("aortic_stenosis_severe");
    let sounds = live(&s, 8.0, ListeningChoice::default());
    let now = sounds.newest_ms().unwrap();
    // the same schedule, kept by an independent synthesiser fed the same events
    let mut synth = s.pcg_synth();
    synth.keep_beats(true);
    let mut engine = TimingEngine::new(s.timing.clone());
    engine.set_shock_seed(s.scenario.seed);
    for _ in 0..8000 {
        engine.step(Millis(1.0));
        synth.on_events(&engine.drain_events());
    }
    let m1s: Vec<f64> = synth
        .scheduled_sounds()
        .iter()
        .filter(|x| x.kind == ComponentKind::M1)
        .map(|x| x.onset_ms)
        .collect();
    let last_m1 = *m1s.iter().rev().find(|t| **t < now - 300.0).unwrap();
    // the views are drawn `delay` behind the newest sample: the mitral ring flashes when that
    // drawn time reaches M1, not when the engine does
    for delay in [0.0, 160.0, 220.0] {
        let drawn = valve_display_time_ms(now, delay);
        assert!((drawn - (now - delay)).abs() < 1e-9);
        let at_m1 = valve_display_time_ms(last_m1 + delay, delay);
        let g = sounds.valve_glow_at(at_m1);
        assert!((g[Valve::Mitral.index()] - 1.0).abs() < 1e-6, "{delay}: {g:?}");
        let expect = heartsim_ecg::valve_glow_at(synth.scheduled_sounds(), synth.scheduled_murmurs(), drawn);
        assert_eq!(sounds.valve_glow_at(drawn), expect, "{delay}");
    }
    // and goes dark a flash later
    let after = sounds.valve_glow_at(last_m1 + VALVE_FLASH_MS + 1.0);
    assert_eq!(after[Valve::Mitral.index()], 0.0);
    // the aortic valve glows through the stenotic murmur
    let m = synth
        .scheduled_murmurs()
        .iter()
        .rev()
        .find(|m| m.stop_ms() < now)
        .unwrap();
    let mid = f64::midpoint(m.start_ms, m.stop_ms());
    assert!(sounds.valve_glow_at(mid)[Valve::Aortic.index()] > 0.5);
    // a negative offset (none) is the newest time itself
    assert!((valve_display_time_ms(now, -5.0) - now).abs() < 1e-12);
}

#[test]
fn every_site_stays_within_full_scale_after_the_limiter() {
    let table = SiteTable::builtin();
    let mut worst = (String::new(), 0.0f32, 0.0f32);
    for stem in [
        "aortic_stenosis_severe",
        "mitral_regurgitation",
        "mitral_stenosis_sinus",
        "ventricular_septal_defect",
        "summation_gallop",
        "avb_3rd_junctional",
    ] {
        let s = scenario(stem);
        for site in ListeningSite::SITES {
            for chest in ChestPiece::ALL {
                let mut synth = s.pcg_synth();
                synth.set_listening(&table.listening(ListeningChoice { site, chest }));
                let mut engine = TimingEngine::new(s.timing.clone());
                engine.set_shock_seed(s.scenario.seed);
                let mut peak = 0.0f32;
                for _ in 0..10_000 {
                    engine.step(Millis(1.0));
                    synth.on_events(&engine.drain_events());
                    for i in (0..4).rev() {
                        let v = synth.sample(Millis(engine.now().0 - f64::from(i) * 0.25));
                        peak = peak.max(v.abs());
                    }
                }
                let out = soft_limit(HEADROOM_GAIN * peak).abs();
                assert!(out <= 1.0, "{stem} {site:?} {chest:?}: {out}");
                if peak > worst.1 {
                    worst = (
                        format!("{stem} {site:?} {chest:?}"),
                        peak,
                        HEADROOM_GAIN * peak / LIMITER_KNEE,
                    );
                }
            }
        }
    }
    eprintln!("loudest site: {worst:?} (peak, fraction of the knee)");
}
