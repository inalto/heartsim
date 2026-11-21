//! Heart sounds through Web Audio (brief B26), the thin layer that calls the browser. Everything
//! that decides something (start times, merging, re-anchoring, drift, gain, limiter, fades) is in
//! `heartsim_app::web_schedule`, tested natively; this module copies its chunks into
//! `AudioBuffer`s, starts them with `AudioBufferSourceNode.start(when)` through one `GainNode`
//! (used only for the fade on a clear), and runs the gesture gate and the page life cycle:
//!
//! * **Gesture gate.** Browsers start an `AudioContext` only after the learner interacted with the
//!   page (autoplay policy). The context is created, or `resume()`d, from real DOM events
//!   (`pointerdown`, `pointerup`, `mousedown`, `touchend` on the canvas, `keydown` on the window)
//!   whenever the sound setting is on and the context is not running; `suspended` and
//!   `interrupted` (iOS) are handled alike. eframe runs the app logic inside its own canvas
//!   listeners, which were installed first, so a click that switches the setting on is seen by ours
//!   afterwards and starts the sound on that same click.
//! * **Page life cycle.** When the tab is hidden, what is scheduled plays out and the context is
//!   then suspended; on return the old stream is dropped and the context resumed, and the player's
//!   next push starts a new stream (no burst of late audio).
//!
//! No threads, no `SharedArrayBuffer`, no AudioWorklet: the static site needs no special headers.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use heartsim_app::heart_sound_player::PcmSinkFactory;
use heartsim_app::sound_out::{FADE_MS, PcmSink, SinkReadiness};
use heartsim_app::web_schedule::{ScheduleStats, ScheduledChunk, WebScheduler};
use wasm_bindgen::JsCast as _;
use wasm_bindgen::prelude::*;
use web_sys::{AudioBufferSourceNode, AudioContext, AudioScheduledSourceNode, GainNode};

/// What `window.heartsimStatus.audio` shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioStatus {
    /// `"off"`, `"waiting"`, `"running"` or `"unavailable"`.
    pub state: &'static str,
    /// The context's sample rate, Hz (0 before it exists).
    pub sample_rate_hz: f64,
    /// Chunks scheduled since start.
    pub chunks: u64,
    /// Scheduled audio ahead of the context clock, ms.
    pub queued_ms: f64,
    /// The constant delay the sink reports to the player, ms.
    pub latency_ms: f64,
    /// Chunks scheduled with a start already past the clock (zero by construction).
    pub late: u64,
}

struct Node {
    id: u64,
    end_s: f64,
    source: AudioBufferSourceNode,
}

struct Context {
    ctx: AudioContext,
    gain: GainNode,
    scheduler: WebScheduler,
}

#[derive(Default)]
struct Shared {
    /// The sound setting is on.
    wanted: bool,
    hidden: bool,
    /// No Web Audio, or the browser refused the context.
    unavailable: bool,
    context: Option<Context>,
    nodes: VecDeque<Node>,
    volume_gain: f32,
}

impl Shared {
    fn state(&self) -> Option<String> {
        let c = self.context.as_ref()?;
        js_sys::Reflect::get(&c.ctx, &JsValue::from_str("state"))
            .ok()
            .and_then(|v| v.as_string())
    }

    fn running(&self) -> bool {
        !self.hidden && self.state().as_deref() == Some("running")
    }

    fn readiness(&self) -> SinkReadiness {
        if self.unavailable {
            SinkReadiness::Unavailable
        } else if self.running() {
            SinkReadiness::Running
        } else {
            SinkReadiness::WaitingForGesture
        }
    }

    fn latency_ms(&self) -> f32 {
        let get = |k: &str| {
            self.context
                .as_ref()
                .and_then(|c| js_sys::Reflect::get(&c.ctx, &JsValue::from_str(k)).ok())
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0)
        };
        // a browser that does not report outputLatency (or reports 0) adds nothing: the user's
        // audio offset covers it (known issues)
        WebScheduler::latency_ms(get("baseLatency"), get("outputLatency"))
    }

    fn now_s(&self) -> f64 {
        self.context.as_ref().map_or(0.0, |c| c.ctx.current_time())
    }

    /// From a DOM event: create the context, or resume it.
    fn ensure_running(&mut self) {
        if !self.wanted || self.hidden || self.unavailable {
            return;
        }
        if self.context.is_none() {
            match create_context() {
                Ok(c) => self.context = Some(c),
                Err(e) => {
                    log::warn!("heart sounds: the browser refused an AudioContext: {e:?}");
                    self.unavailable = true;
                    return;
                }
            }
        }
        if self.state().as_deref() != Some("running")
            && let Some(c) = &self.context
        {
            settle(c.ctx.resume());
        }
    }

    fn play(&mut self, chunk: &ScheduledChunk) {
        let Some(c) = &self.context else { return };
        if chunk.samples.is_empty() {
            return;
        }
        let rate = c.scheduler.out_rate_hz();
        let result = (|| -> Result<AudioBufferSourceNode, JsValue> {
            let len = u32::try_from(chunk.samples.len()).map_err(|_| JsValue::from_str("chunk too long"))?;
            let buffer = c.ctx.create_buffer(1, len, rate as f32)?;
            buffer.copy_to_channel(&chunk.samples, 0)?;
            let source = c.ctx.create_buffer_source()?;
            source.set_buffer(Some(&buffer));
            source.connect_with_audio_node(&c.gain)?;
            source.start_with_when(chunk.start_frame as f64 / rate)?;
            Ok(source)
        })();
        match result {
            Ok(source) => self.nodes.push_back(Node {
                id: chunk.id,
                end_s: chunk.end_frame() as f64 / rate,
                source,
            }),
            Err(e) => log::warn!("heart sounds: a chunk could not be scheduled: {e:?}"),
        }
    }

    fn push(&mut self, samples: &[f32]) -> usize {
        if self.readiness() != SinkReadiness::Running {
            // nothing is heard while waiting; the player stops pushing on the next frame
            return samples.len();
        }
        let now = self.now_s();
        let gain = self.volume_gain;
        let chunk = self.context.as_mut().and_then(|c| {
            c.scheduler.set_gain(gain);
            c.scheduler.push(now, samples)
        });
        if let Some(chunk) = chunk {
            self.play(&chunk);
        }
        self.prune(now);
        samples.len()
    }

    fn prune(&mut self, now: f64) {
        while self.nodes.front().is_some_and(|n| n.end_s <= now) {
            self.nodes.pop_front();
        }
    }

    /// Fade out what plays over [`FADE_MS`], stop what has not started, forget the stream.
    fn clear(&mut self) {
        let now = self.now_s();
        let Some(c) = self.context.as_mut() else { return };
        let live = c.scheduler.clear(now);
        let fade_s = f64::from(FADE_MS) / 1000.0;
        let g = c.gain.gain();
        let _ = g.cancel_scheduled_values(now);
        let _ = g.set_value_at_time(g.value(), now);
        let _ = g.linear_ramp_to_value_at_time(0.0, now + fade_s);
        // back to unity well before the next stream, which starts a look-ahead after now
        let _ = g.set_value_at_time(1.0, now + 2.0 * fade_s);
        for node in self.nodes.drain(..) {
            let started = live.iter().find(|l| l.id == node.id).is_none_or(|l| l.started);
            // through the base class: the `AudioBufferSourceNode` overloads are deprecated in web-sys
            let source: &AudioScheduledSourceNode = node.source.as_ref();
            let _ = if started {
                source.stop_with_when(now + fade_s)
            } else {
                source.stop()
            };
        }
    }
}

fn create_context() -> Result<Context, JsValue> {
    let ctx = AudioContext::new()?;
    let gain = ctx.create_gain()?;
    gain.connect_with_audio_node(&ctx.destination())?;
    let scheduler = WebScheduler::new(f64::from(ctx.sample_rate()));
    Ok(Context { ctx, gain, scheduler })
}

/// Await a promise and ignore its outcome (a refused `resume()` is not an error: the next click
/// tries again), so no unhandled rejection reaches the console.
fn settle(p: Result<js_sys::Promise, JsValue>) {
    if let Ok(p) = p {
        wasm_bindgen_futures::spawn_local(async move {
            let _ = wasm_bindgen_futures::JsFuture::from(p).await;
        });
    }
}

/// The browser's heart-sound output: shared state, the DOM listeners and the sink factory.
#[derive(Clone)]
pub struct WebAudio(Rc<RefCell<Shared>>);

impl WebAudio {
    /// No context yet: it is created on the learner's first click with the sound on.
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(Shared {
            volume_gain: 0.0,
            ..Shared::default()
        })))
    }

    /// The factory `HeartSimApp` calls the first time the sound is switched on. It fails only in a
    /// browser without Web Audio; otherwise the sink exists at once and waits for a click.
    pub fn factory(&self) -> PcmSinkFactory {
        let shared = self.0.clone();
        Box::new(move || {
            let window = web_sys::window().ok_or("no window")?;
            if !js_sys::Reflect::has(&window, &JsValue::from_str("AudioContext")).unwrap_or(false) {
                shared.borrow_mut().unavailable = true;
                return Err("Web Audio (AudioContext) is not available in this browser".to_string());
            }
            Ok(Box::new(WebSink(shared.clone())) as Box<dyn PcmSink>)
        })
    }

    /// Install the gesture and visibility listeners (after eframe installed its own on the canvas).
    pub fn install(&self, canvas: &web_sys::HtmlCanvasElement) -> Result<(), JsValue> {
        let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
        let document = window
            .document()
            .ok_or_else(|| JsValue::from_str("no document"))?;
        let gesture = {
            let shared = self.0.clone();
            Closure::<dyn FnMut(web_sys::Event)>::new(move |_e: web_sys::Event| {
                if let Ok(mut s) = shared.try_borrow_mut() {
                    s.ensure_running();
                }
            })
        };
        let f = gesture.as_ref().unchecked_ref();
        for kind in ["pointerdown", "pointerup", "mousedown", "touchend"] {
            canvas.add_event_listener_with_callback(kind, f)?;
        }
        // keys: capture on the window, so egui's handling cannot stop it
        window.add_event_listener_with_callback_and_bool("keydown", f, true)?;
        gesture.forget();

        let suspend = {
            let shared = self.0.clone();
            Closure::<dyn FnMut()>::new(move || {
                if let Ok(s) = shared.try_borrow()
                    && s.hidden
                    && let Some(c) = &s.context
                {
                    settle(c.ctx.suspend());
                }
            })
        };
        let visibility = {
            let shared = self.0.clone();
            let doc = document.clone();
            let window = window.clone();
            Closure::<dyn FnMut(web_sys::Event)>::new(move |_e: web_sys::Event| {
                let Ok(mut s) = shared.try_borrow_mut() else {
                    return;
                };
                s.hidden = doc.hidden();
                if s.hidden {
                    // let what is scheduled play out, then suspend
                    let lead_ms = f64::from(
                        s.context
                            .as_ref()
                            .map_or(0.0, |c| c.scheduler.queued_ms(c.ctx.current_time())),
                    );
                    let wait = (lead_ms + 2.0 * f64::from(FADE_MS)).ceil() as i32;
                    let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                        suspend.as_ref().unchecked_ref(),
                        wait,
                    );
                } else if s.context.is_some() {
                    // no burst of late audio: drop the old stream; the player re-anchors
                    s.clear();
                    if s.wanted
                        && let Some(c) = &s.context
                    {
                        settle(c.ctx.resume());
                    }
                }
            })
        };
        document.add_event_listener_with_callback("visibilitychange", visibility.as_ref().unchecked_ref())?;
        visibility.forget();
        Ok(())
    }

    /// Once per frame, after the app ran: the setting, the merge tail, and a context to suspend
    /// when the sound was switched off.
    pub fn frame(&self, wanted: bool) {
        let Ok(mut s) = self.0.try_borrow_mut() else {
            return;
        };
        s.wanted = wanted;
        if !wanted {
            if s.state().as_deref() == Some("running")
                && let Some(c) = &s.context
            {
                settle(c.ctx.suspend());
            }
            return;
        }
        if s.running() {
            let now = s.now_s();
            let chunk = s.context.as_mut().and_then(|c| c.scheduler.tick(now));
            if let Some(chunk) = chunk {
                s.play(&chunk);
            }
            s.prune(now);
        }
    }

    /// For `window.heartsimStatus.audio`.
    pub fn status(&self) -> AudioStatus {
        let Ok(s) = self.0.try_borrow() else {
            return AudioStatus {
                state: "off",
                sample_rate_hz: 0.0,
                chunks: 0,
                queued_ms: 0.0,
                latency_ms: 0.0,
                late: 0,
            };
        };
        let state = if !s.wanted {
            "off"
        } else if s.unavailable {
            "unavailable"
        } else if s.running() {
            "running"
        } else {
            "waiting"
        };
        let now = s.now_s();
        let (rate, stats, queued) = s
            .context
            .as_ref()
            .map_or((0.0, ScheduleStats::default(), 0.0), |c| {
                (
                    c.scheduler.out_rate_hz(),
                    c.scheduler.stats(),
                    f64::from(c.scheduler.queued_ms(now)),
                )
            });
        let ScheduleStats { chunks, late, .. } = stats;
        AudioStatus {
            state,
            sample_rate_hz: rate,
            chunks,
            queued_ms: queued,
            latency_ms: f64::from(s.latency_ms()),
            late,
        }
    }
}

/// The sink the player writes to; every call goes to the shared state.
struct WebSink(Rc<RefCell<Shared>>);

impl PcmSink for WebSink {
    fn push(&mut self, samples: &[f32]) -> usize {
        self.0.try_borrow_mut().map_or(0, |mut s| s.push(samples))
    }

    fn queued_ms(&self) -> f32 {
        self.0.try_borrow().map_or(0.0, |s| {
            let now = s.now_s();
            s.context
                .as_ref()
                .map_or(0.0, |c| c.scheduler.sink_queued_ms(now))
        })
    }

    fn clear(&mut self) {
        if let Ok(mut s) = self.0.try_borrow_mut() {
            s.clear();
        }
    }

    fn set_gain(&mut self, gain: f32) {
        if let Ok(mut s) = self.0.try_borrow_mut() {
            s.volume_gain = gain;
        }
    }

    fn latency_ms(&self) -> f32 {
        self.0
            .try_borrow()
            .map_or(heartsim_app::sound_out::QUEUE_TARGET_MS, |s| s.latency_ms())
    }

    fn readiness(&self) -> SinkReadiness {
        self.0
            .try_borrow()
            .map_or(SinkReadiness::WaitingForGesture, |s| s.readiness())
    }
}
