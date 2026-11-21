//! B28 acceptance tests of the heart-sound WAV export: the file parses back to the same samples
//! within 16-bit rounding, at 44.1 kHz, with the right length and metadata, and it is deterministic.

use heartsim_ecg::resample::resample_block;
use heartsim_ecg::{ClipOptions, SoundClip, Tier0Scenario};
use heartsim_io::wav::{WAV_GAIN, WAV_RATE_HZ, wav_comment};
use heartsim_io::{WavInfo, heart_sound_file, read_wav, wav_bytes};
use std::path::Path;

fn scenario(id: &str) -> Tier0Scenario {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../assets/scenarios/{id}.ron"));
    Tier0Scenario::from_ron(&std::fs::read_to_string(p).unwrap()).unwrap()
}

#[test]
fn a_clip_exported_as_wav_reads_back_with_its_samples_rate_length_and_metadata() {
    let s = scenario("aortic_stenosis_severe");
    let clip = SoundClip::record(&s, ClipOptions::default()).unwrap();
    let info = WavInfo {
        title: s.scenario.title.clone(),
        comment: wav_comment(&s.scenario.id, "overview", None),
        date: "2026-11-21".into(),
    };
    let file = heart_sound_file("heartsim_aortic_stenosis_severe", &clip.pcg, 4000.0, &info);
    assert_eq!(file.name, "heartsim_aortic_stenosis_severe_heart_sounds.wav");
    let wav = read_wav(&file.bytes).unwrap();
    assert_eq!((wav.rate_hz, wav.channels, wav.bits), (WAV_RATE_HZ, 1, 16));
    let want_len = (clip.pcg.len() as f64 * f64::from(WAV_RATE_HZ) / 4000.0).round() as usize;
    assert_eq!(wav.samples.len(), want_len);
    let expected = resample_block(&clip.pcg, 4000.0, f64::from(WAV_RATE_HZ));
    let lsb = 1.0 / f32::from(i16::MAX);
    let worst = expected
        .iter()
        .zip(&wav.samples)
        .map(|(e, s)| ((e * WAV_GAIN).clamp(-1.0, 1.0) - f32::from(*s) * lsb).abs())
        .fold(0.0, f32::max);
    assert!(worst <= 0.5 * lsb + 1e-7, "rounding error {worst}");
    assert!(
        wav.samples.iter().any(|s| s.unsigned_abs() > 3000),
        "there is sound"
    );
    assert_eq!(wav.info("INAM"), Some(s.scenario.title.as_str()));
    let comment = wav.info("ICMT").unwrap();
    assert!(comment.contains("synthesised by heartsim"), "{comment}");
    assert!(comment.contains("aortic_stenosis_severe") && comment.contains("Site: overview"));
    assert!(wav.info("ISFT").unwrap().starts_with("heartsim "));
    assert_eq!(wav.info("ICRD"), Some("2026-11-21"));
    // deterministic
    assert_eq!(wav_bytes(&clip.pcg, 4000.0, &info), file.bytes);
}

#[test]
fn a_file_that_is_not_wav_is_refused() {
    assert!(read_wav(b"RIFF\0\0\0\0AVI ").is_err());
    assert!(read_wav(b"").is_err());
}
