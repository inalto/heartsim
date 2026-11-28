//! The browser transport of online case libraries (brief B43): the page's `fetch`, one request at
//! a time, cancelled with an `AbortController`. Injected into the app with
//! `HeartSimApp::set_case_fetch`, like the Web Audio sink.
//!
//! Nothing is added to the request: no header of ours (the browser sends its own), and
//! `credentials: "omit"`, so no cookie goes along even to the page's own site. A foreign library
//! must answer with `Access-Control-Allow-Origin`; when it does not, `fetch` fails with a
//! `TypeError` that JavaScript cannot tell apart from "unreachable", so the error says both
//! ([`FetchError::Blocked`] with `cross_origin`). The body is read whole (`arrayBuffer`): the
//! `Content-Length` is checked against the cap before, and the length after.

use heartsim_app::case_fetch::{CaseFetch, FetchError, FetchPoll};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::{JsCast as _, JsValue};
use wasm_bindgen_futures::JsFuture;

#[derive(Default)]
struct Shared {
    /// Bumped by every start and cancel: an answer of an older request is dropped.
    generation: u64,
    running: bool,
    answer: Option<Result<Vec<u8>, FetchError>>,
}

/// `fetch` with an `AbortController`.
#[derive(Default)]
pub struct WebFetch {
    shared: Rc<RefCell<Shared>>,
    abort: Option<web_sys::AbortController>,
}

impl CaseFetch for WebFetch {
    fn start(&mut self, url: &str, max_bytes: usize) {
        self.cancel();
        let generation = {
            let mut s = self.shared.borrow_mut();
            s.running = true;
            s.answer = None;
            s.generation
        };
        let abort = web_sys::AbortController::new().ok();
        let signal = abort.as_ref().map(web_sys::AbortController::signal);
        self.abort = abort;
        let shared = Rc::clone(&self.shared);
        let url = url.to_owned();
        wasm_bindgen_futures::spawn_local(async move {
            let result = get(&url, max_bytes, signal.as_ref()).await;
            let mut s = shared.borrow_mut();
            if s.generation == generation {
                s.running = false;
                s.answer = Some(result);
            }
        });
    }

    fn poll(&mut self) -> FetchPoll {
        let mut s = self.shared.borrow_mut();
        match s.answer.take() {
            Some(Ok(b)) => FetchPoll::Done(b),
            Some(Err(e)) => FetchPoll::Failed(e),
            None if s.running => FetchPoll::Pending,
            None => FetchPoll::Idle,
        }
    }

    fn cancel(&mut self) {
        {
            let mut s = self.shared.borrow_mut();
            s.generation += 1;
            s.running = false;
            s.answer = None;
        }
        if let Some(a) = self.abort.take() {
            a.abort();
        }
    }
}

/// Whether `url` (relative ones resolved against the page) is on another origin than the page.
fn cross_origin(window: &web_sys::Window, url: &str) -> bool {
    let location = window.location();
    let (Ok(href), Ok(origin)) = (location.href(), location.origin()) else {
        return true;
    };
    !web_sys::Url::new_with_base(url, &href).is_ok_and(|u| u.origin() == origin)
}

async fn get(
    url: &str,
    max_bytes: usize,
    signal: Option<&web_sys::AbortSignal>,
) -> Result<Vec<u8>, FetchError> {
    let window = web_sys::window().ok_or_else(|| FetchError::Network("no window".into()))?;
    let init = web_sys::RequestInit::new();
    init.set_method("GET");
    init.set_mode(web_sys::RequestMode::Cors);
    init.set_credentials(web_sys::RequestCredentials::Omit);
    init.set_signal(signal);
    let js = |e: JsValue| e.as_string().unwrap_or_else(|| format!("{e:?}"));
    let response = JsFuture::from(window.fetch_with_str_and_init(url, &init))
        .await
        .map_err(|e| {
            if e.is_instance_of::<js_sys::TypeError>() {
                FetchError::Blocked {
                    cross_origin: cross_origin(&window, url),
                }
            } else {
                FetchError::Network(js(e))
            }
        })?
        .dyn_into::<web_sys::Response>()
        .map_err(|_| FetchError::Network("fetch did not return a Response".into()))?;
    if !response.ok() {
        return Err(FetchError::Status(response.status()));
    }
    let listed = response
        .headers()
        .get("content-length")
        .ok()
        .flatten()
        .and_then(|v| v.trim().parse::<u64>().ok());
    if listed.is_some_and(|n| n > max_bytes as u64) {
        return Err(FetchError::TooLarge { cap: max_bytes });
    }
    let buffer = JsFuture::from(response.array_buffer().map_err(|e| FetchError::Network(js(e)))?)
        .await
        .map_err(|e| FetchError::Network(js(e)))?;
    let array = js_sys::Uint8Array::new(&buffer);
    if array.length() as usize > max_bytes {
        return Err(FetchError::TooLarge { cap: max_bytes });
    }
    Ok(array.to_vec())
}
