//! Heart-sound clips for the teaching pages (brief B28): recorded once per scenario, listening
//! choice and breath-hold, and kept with what the pages need to play and draw them.
//!
//! A clip is an offline Tier-0 recording ([`SoundClip::record`]), so it does not touch the running
//! simulation and the same request always gives the same clip.

use std::collections::HashMap;
use std::rc::Rc;

use heartsim_core::Lead;
use heartsim_ecg::clip::CLIP_ECG_RATE_HZ;
use heartsim_ecg::{ClipOptions, LeadRing, PCG_RATE_HZ, SoundClip};

use crate::scenarios::ScenarioLibrary;

/// A clip with its buffers in the shapes the player and the clip view take.
#[derive(Debug)]
pub struct ClipEntry {
    pub clip: SoundClip,
    /// The loop as played (the small-speaker version when asked for), shared with the player.
    pub pcg: Rc<[f32]>,
    /// The half-speed loop.
    pub half_speed: Rc<[f32]>,
    /// Lead II of the clip as a ring from index 0.
    pub ecg_ii: LeadRing,
    /// The loop as a ring from index 0 (the lane).
    pub pcg_ring: LeadRing,
}

impl ClipEntry {
    fn new(clip: SoundClip) -> Self {
        let ii = &clip.leads[Lead::II as usize];
        let mut ecg_ii = LeadRing::new(ii.len().max(1), CLIP_ECG_RATE_HZ);
        for v in ii {
            ecg_ii.push(*v);
        }
        let mut pcg_ring = LeadRing::new(clip.pcg.len().max(1), PCG_RATE_HZ);
        for v in &clip.pcg {
            pcg_ring.push(*v);
        }
        Self {
            pcg: clip.sound.clone().into(),
            half_speed: clip.half_speed.clone().into(),
            ecg_ii,
            pcg_ring,
            clip,
        }
    }

    /// The buffer to play: the clip, or its half-speed version.
    #[must_use]
    pub fn samples(&self, half_speed: bool) -> Rc<[f32]> {
        if half_speed {
            Rc::clone(&self.half_speed)
        } else {
            Rc::clone(&self.pcg)
        }
    }

    /// The index in the clip (the trace) that playback index `i` of [`Self::samples`] stands for.
    #[must_use]
    pub fn trace_index(&self, i: usize, half_speed: bool) -> usize {
        let len = self.pcg.len().max(1);
        if half_speed {
            (i * len / self.half_speed.len().max(1)).min(len - 1)
        } else {
            i.min(len - 1)
        }
    }

    /// The playback index of trace index `i` in [`Self::samples`].
    #[must_use]
    pub fn playback_index(&self, i: usize, half_speed: bool) -> usize {
        if half_speed {
            i * self.half_speed.len() / self.pcg.len().max(1)
        } else {
            i
        }
    }
}

/// Clips recorded so far, by scenario id and options.
#[derive(Debug, Default)]
pub struct ClipCache {
    clips: HashMap<(String, ClipOptions), Rc<ClipEntry>>,
}

impl ClipCache {
    /// The clip of scenario `id` with `options`, recorded on first request.
    ///
    /// # Errors
    /// The scenario is not in the library or cannot be run.
    pub fn get(
        &mut self,
        library: &ScenarioLibrary,
        id: &str,
        options: ClipOptions,
    ) -> Result<Rc<ClipEntry>, String> {
        let key = (id.to_owned(), options);
        if let Some(e) = self.clips.get(&key) {
            return Ok(Rc::clone(e));
        }
        let i = library
            .index_of(id)
            .ok_or_else(|| format!("no scenario `{id}`"))?;
        let clip = SoundClip::record(&library.entries[i], options).map_err(|e| e.to_string())?;
        let entry = Rc::new(ClipEntry::new(clip));
        self.clips.insert(key, Rc::clone(&entry));
        Ok(entry)
    }

    /// Clips held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.clips.len()
    }

    /// No clip recorded yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.clips.is_empty()
    }
}

/// Whether heart sounds can be played here at all: an output exists and has not failed. The
/// teaching pages show the trace, and say why, when not.
#[must_use]
pub fn sound_available(player: &crate::heart_sound_player::HeartSoundPlayer) -> bool {
    player.has_output() && player.available() != Some(false)
}
