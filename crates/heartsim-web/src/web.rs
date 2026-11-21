//! Browser glue: find the canvas, download the assets, start eframe's web runner, publish a small
//! status object for the smoke test (`window.heartsimStatus`).

use crate::archive::{ARCHIVE_NAME, unpack};
use crate::audio::{AudioStatus, WebAudio};
use heartsim_app::{AssetBundle, HeartSimApp};
use wasm_bindgen::{JsCast as _, JsValue};
use wasm_bindgen_futures::JsFuture;

/// `id` of the canvas in `index.html`.
const CANVAS_ID: &str = "heartsim_canvas";
/// `id` of the loading screen, removed once the first frame is up.
const LOADING_ID: &str = "loading";

/// Called from `main`: starts the app asynchronously and returns at once.
pub fn start() {
    console_error_panic_hook::set_once();
    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    wasm_bindgen_futures::spawn_local(async {
        if let Err(e) = run().await {
            let msg = e.as_string().unwrap_or_else(|| format!("{e:?}"));
            web_sys::console::error_1(&JsValue::from_str(&msg));
            show_loading_text(&format!(
                "heartsim could not start: {msg}. It needs a browser with WebGPU or WebGL2."
            ));
        }
    });
}

async fn run() -> Result<(), JsValue> {
    let document = web_sys::window()
        .and_then(|w| w.document())
        .ok_or_else(|| JsValue::from_str("no document"))?;
    let canvas = document
        .get_element_by_id(CANVAS_ID)
        .ok_or_else(|| JsValue::from_str("index.html has no #heartsim_canvas"))?
        .dyn_into::<web_sys::HtmlCanvasElement>()?;

    show_loading_text("Downloading anatomy and rhythm library…");
    let assets = match download_assets().await {
        Ok(a) => a,
        Err(e) => {
            // The simulator is still useful with the placeholder sphere and no rhythm library.
            web_sys::console::warn_1(&JsValue::from_str(&format!("assets unavailable: {e}")));
            AssetBundle::new()
        }
    };
    show_loading_text("Starting the renderer…");

    let status = Status::new()?;
    let audio = WebAudio::new();
    let sound_param = sound_requested(&web_sys::window().ok_or("no window")?);
    let app_audio = audio.clone();
    eframe::WebRunner::new()
        .start(
            canvas.clone(),
            eframe::WebOptions::default(),
            Box::new(move |cc| Ok(Box::new(WebApp::new(cc, &assets, status, app_audio, sound_param)))),
        )
        .await?;
    // after eframe's own canvas listeners: a click that switches the sound on has been handled by
    // the app when ours runs, so it starts the sound on that same click
    audio.install(&canvas)?;
    remove_loading_screen();
    Ok(())
}

async fn download_assets() -> Result<AssetBundle, String> {
    let window = web_sys::window().ok_or("no window")?;
    let response = JsFuture::from(window.fetch_with_str(ARCHIVE_NAME))
        .await
        .map_err(|e| format!("{e:?}"))?
        .dyn_into::<web_sys::Response>()
        .map_err(|_| "fetch did not return a Response".to_string())?;
    if !response.ok() {
        return Err(format!("{ARCHIVE_NAME}: HTTP {}", response.status()));
    }
    let buffer = JsFuture::from(response.array_buffer().map_err(|e| format!("{e:?}"))?)
        .await
        .map_err(|e| format!("{e:?}"))?;
    let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
    unpack(&bytes).map_err(|e| e.to_string())
}

/// `?sound=1` in the page URL switches the heart sounds on at start (the smoke test uses it, so it
/// does not have to find an egui widget).
fn sound_requested(window: &web_sys::Window) -> bool {
    window
        .location()
        .search()
        .is_ok_and(|q| q.trim_start_matches('?').split('&').any(|kv| kv == "sound=1"))
}

fn show_loading_text(text: &str) {
    if let Some(el) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id("loading_text"))
    {
        el.set_text_content(Some(text));
    }
}

fn remove_loading_screen() {
    if let Some(el) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id(LOADING_ID))
    {
        el.remove();
    }
}

/// `window.heartsimStatus`: plain properties a browser test can poll — `frames` (egui frames drawn),
/// `simTimeMs`, `stripSamples` (ECG samples pushed to the strip), `anatomy` (pack loaded) and
/// `audio` (B26: `state` "off" / "waiting" / "running" / "unavailable", `sampleRate`, `chunks`
/// scheduled, `queuedMs` ahead of the audio clock, `latencyMs` the views are drawn behind, `late`
/// chunks scheduled in the past); `backend` is left to the page's own `navigator.gpu` probe.
struct Status(js_sys::Object, js_sys::Object);

impl Status {
    fn new() -> Result<Self, JsValue> {
        let obj = js_sys::Object::new();
        let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
        js_sys::Reflect::set(&window, &JsValue::from_str("heartsimStatus"), &obj)?;
        let audio = js_sys::Object::new();
        js_sys::Reflect::set(&obj, &JsValue::from_str("audio"), &audio)?;
        Ok(Self(obj, audio))
    }

    fn publish_audio(&self, a: &AudioStatus) {
        let set = |k: &str, v: &JsValue| {
            js_sys::Reflect::set(&self.1, &JsValue::from_str(k), v).ok();
        };
        set("state", &JsValue::from_str(a.state));
        set("sampleRate", &JsValue::from_f64(a.sample_rate_hz));
        set("chunks", &JsValue::from_f64(a.chunks as f64));
        set("queuedMs", &JsValue::from_f64(a.queued_ms));
        set("latencyMs", &JsValue::from_f64(a.latency_ms));
        set("late", &JsValue::from_f64(a.late as f64));
    }

    fn publish(&self, frames: u64, s: &heartsim_app::AppStatus) {
        let set = |k: &str, v: f64| {
            js_sys::Reflect::set(&self.0, &JsValue::from_str(k), &JsValue::from_f64(v)).ok();
        };
        set("frames", frames as f64);
        set("simTimeMs", s.sim_time_ms);
        set("stripSamples", s.strip_samples as f64);
        set("anatomy", f64::from(u8::from(s.anatomy_loaded)));
    }
}

/// The desktop app plus the status publisher and the Web Audio output.
struct WebApp {
    inner: HeartSimApp,
    status: Status,
    audio: WebAudio,
    frames: u64,
}

impl WebApp {
    fn new(
        cc: &eframe::CreationContext<'_>,
        assets: &AssetBundle,
        status: Status,
        audio: WebAudio,
        sound: bool,
    ) -> Self {
        let mut inner = HeartSimApp::with_assets(cc, assets);
        inner.set_heart_sound_output(
            audio.factory(),
            heartsim_app::heart_sound_player::HeartSoundStatus::Unsupported,
        );
        if sound {
            inner.set_heart_sound_audio(true);
        }
        Self {
            inner,
            status,
            audio,
            frames: 0,
        }
    }
}

impl eframe::App for WebApp {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, frame: &mut eframe::Frame) {
        eframe::App::ui(&mut self.inner, ui, frame);
        self.audio.frame(self.inner.heart_sound_audio());
        self.frames += 1;
        self.status.publish(self.frames, &self.inner.status());
        self.status.publish_audio(&self.audio.status());
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::App::save(&mut self.inner, storage);
    }
}
