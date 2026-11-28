//! The network part of online case libraries (brief B43): one GET at a time, with a byte cap,
//! started by a click and polled each frame. The Cases page ([`crate::case_ui`]) talks only to the
//! [`CaseFetch`] trait, so it has no `cfg`, like [`crate::case_io`].
//!
//! * [`UreqFetch`] (desktop, cargo feature `online-cases`): `ureq` on a short-lived thread, as
//!   [`crate::update_check`] does: HTTPS only (redirects too) except for an `http://localhost` or
//!   `http://127.0.0.1` address, which follows no redirect at all; a 5 s connect and 20 s total
//!   timeout; `User-Agent: heartsim/<version>` and no other header, no cookies.
//! * The browser build injects its own (`fetch` with an `AbortController`, in `heartsim-web`) with
//!   [`crate::HeartSimApp::set_case_fetch`].
//! * [`NoFetch`]: no network in this build (the page says so and its button is disabled).
//! * [`MemoryFetch`]: answers from memory and counts the requests; every test uses it, so no test
//!   touches the network.
//!
//! Nothing here runs at start: a request is made only when the user presses "Browse the online
//! library" or "Import".

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

/// Why a request failed; the Cases page puts it into words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchError {
    /// The server answered with a status other than 2xx (a redirect that was not followed too).
    Status(u16),
    /// The body is longer than the cap the request was started with.
    TooLarge { cap: usize },
    /// The browser refused to hand over the answer: the server cannot be reached, or (for another
    /// site than heartsim's own) it does not allow the web version to read it (CORS). JavaScript
    /// cannot tell the two apart. `cross_origin` says whether CORS can be the reason.
    Blocked { cross_origin: bool },
    /// The address was refused before any request (not https, an unsafe redirect).
    Refused(String),
    /// Anything else the transport reports (no connection, a timeout, a TLS error), in its words.
    Network(String),
}

/// What [`CaseFetch::poll`] found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchPoll {
    /// No request is running and no answer is waiting.
    Idle,
    /// The request is running.
    Pending,
    /// The body, at most the cap it was started with.
    Done(Vec<u8>),
    Failed(FetchError),
}

/// One GET at a time, started by a click, finished later.
pub trait CaseFetch {
    /// `false` when this build has no network: the page disables its buttons and says why.
    fn available(&self) -> bool {
        true
    }
    /// Start a GET of `url` whose body may have at most `max_bytes` bytes (one more is a
    /// [`FetchError::TooLarge`]). A request already running is cancelled first.
    fn start(&mut self, url: &str, max_bytes: usize);
    /// The state of the request; `Done` and `Failed` are returned once, then `Idle`.
    fn poll(&mut self) -> FetchPoll;
    /// Abandon the running request: its answer, if one still comes, is dropped.
    fn cancel(&mut self);
}

/// No network in this build.
#[derive(Debug, Default)]
pub struct NoFetch;

impl CaseFetch for NoFetch {
    fn available(&self) -> bool {
        false
    }
    fn start(&mut self, _url: &str, _max_bytes: usize) {}
    fn poll(&mut self) -> FetchPoll {
        FetchPoll::Idle
    }
    fn cancel(&mut self) {}
}

/// Answers from memory, for tests: a map from URL to an answer, a log of the requests made, and a
/// `hold` switch that keeps requests pending until [`Self::release`] (to test Cancel).
#[derive(Debug, Clone, Default)]
pub struct MemoryFetch(Arc<Mutex<Memory>>);

#[derive(Debug, Default)]
struct Memory {
    answers: BTreeMap<String, Result<Vec<u8>, FetchError>>,
    requests: Vec<String>,
    hold: bool,
    running: Option<(String, usize)>,
}

impl MemoryFetch {
    /// Answer `url` with `body`.
    pub fn serve(&self, url: &str, body: Vec<u8>) {
        if let Ok(mut m) = self.0.lock() {
            m.answers.insert(url.to_owned(), Ok(body));
        }
    }

    /// Answer `url` with `error`.
    pub fn fail(&self, url: &str, error: FetchError) {
        if let Ok(mut m) = self.0.lock() {
            m.answers.insert(url.to_owned(), Err(error));
        }
    }

    /// Keep requests pending (`true`) until [`Self::release`].
    pub fn hold(&self, on: bool) {
        if let Ok(mut m) = self.0.lock() {
            m.hold = on;
        }
    }

    /// Stop holding: the pending request is answered at the next poll.
    pub fn release(&self) {
        self.hold(false);
    }

    /// The URLs requested so far, in order.
    #[must_use]
    pub fn requests(&self) -> Vec<String> {
        self.0.lock().map(|m| m.requests.clone()).unwrap_or_default()
    }
}

impl CaseFetch for MemoryFetch {
    fn start(&mut self, url: &str, max_bytes: usize) {
        if let Ok(mut m) = self.0.lock() {
            m.requests.push(url.to_owned());
            m.running = Some((url.to_owned(), max_bytes));
        }
    }

    fn poll(&mut self) -> FetchPoll {
        let Ok(mut m) = self.0.lock() else {
            return FetchPoll::Idle;
        };
        let Some((url, cap)) = m.running.clone() else {
            return FetchPoll::Idle;
        };
        if m.hold {
            return FetchPoll::Pending;
        }
        m.running = None;
        match m.answers.get(&url) {
            Some(Ok(body)) if body.len() > cap => FetchPoll::Failed(FetchError::TooLarge { cap }),
            Some(Ok(body)) => FetchPoll::Done(body.clone()),
            Some(Err(e)) => FetchPoll::Failed(e.clone()),
            None => FetchPoll::Failed(FetchError::Status(404)),
        }
    }

    fn cancel(&mut self) {
        if let Ok(mut m) = self.0.lock() {
            m.running = None;
        }
    }
}

#[cfg(feature = "online-cases")]
pub use native::UreqFetch;

#[cfg(feature = "online-cases")]
mod native {
    use super::{CaseFetch, FetchError, FetchPoll};
    use std::io::Read as _;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::{self, Receiver, TryRecvError};
    use std::time::Duration;

    /// Time allowed to open the connection.
    pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
    /// Time allowed for the whole request, body included.
    pub const TOTAL_TIMEOUT: Duration = Duration::from_secs(20);
    /// Redirects followed for an https address (each hop must be https too).
    pub const MAX_REDIRECTS: u32 = 5;

    /// The desktop transport: one `ureq` request on a short-lived thread.
    #[derive(Debug, Default)]
    pub struct UreqFetch {
        job: Option<Job>,
    }

    #[derive(Debug)]
    struct Job {
        rx: Receiver<Result<Vec<u8>, FetchError>>,
        cancel: Arc<AtomicBool>,
    }

    impl CaseFetch for UreqFetch {
        fn start(&mut self, url: &str, max_bytes: usize) {
            self.cancel();
            let (tx, rx) = mpsc::channel();
            let cancel = Arc::new(AtomicBool::new(false));
            let flag = Arc::clone(&cancel);
            let url = url.to_owned();
            let spawned = std::thread::Builder::new()
                .name("case-library".into())
                .spawn(move || {
                    let result = get(&url, max_bytes, &flag);
                    // the receiver is gone after Cancel: nothing to do
                    let _ = tx.send(result);
                });
            match spawned {
                Ok(_) => self.job = Some(Job { rx, cancel }),
                Err(e) => {
                    let (tx, rx) = mpsc::channel();
                    let _ = tx.send(Err(FetchError::Network(e.to_string())));
                    self.job = Some(Job {
                        rx,
                        cancel: Arc::new(AtomicBool::new(false)),
                    });
                }
            }
        }

        fn poll(&mut self) -> FetchPoll {
            let Some(job) = &self.job else {
                return FetchPoll::Idle;
            };
            match job.rx.try_recv() {
                Ok(result) => {
                    self.job = None;
                    match result {
                        Ok(b) => FetchPoll::Done(b),
                        Err(e) => FetchPoll::Failed(e),
                    }
                }
                Err(TryRecvError::Empty) => FetchPoll::Pending,
                Err(TryRecvError::Disconnected) => {
                    self.job = None;
                    FetchPoll::Failed(FetchError::Network("the request thread ended".into()))
                }
            }
        }

        fn cancel(&mut self) {
            if let Some(job) = self.job.take() {
                job.cancel.store(true, Ordering::Relaxed);
            }
        }
    }

    /// The request: https only (with its redirects) unless the address is plain http, which is
    /// accepted only for localhost by the page and then follows no redirect.
    fn get(url: &str, max_bytes: usize, cancel: &AtomicBool) -> Result<Vec<u8>, FetchError> {
        let https = url.starts_with("https://");
        let local = url.strip_prefix("http://").is_some_and(|rest| {
            let authority = rest.split('/').next().unwrap_or_default();
            let host = authority.rsplit_once(':').map_or(authority, |(h, _)| h);
            host == "localhost" || host == "127.0.0.1"
        });
        if !https && !local {
            return Err(FetchError::Refused(url.chars().take(200).collect()));
        }
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .https_only(https)
            .max_redirects(if https { MAX_REDIRECTS } else { 0 })
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .timeout_global(Some(TOTAL_TIMEOUT))
            .user_agent(format!("heartsim/{}", env!("CARGO_PKG_VERSION")))
            .http_status_as_error(false)
            .build()
            .into();
        let response = agent.get(url).call().map_err(|e| match e {
            ureq::Error::RequireHttpsOnly(u) => FetchError::Refused(u.chars().take(200).collect()),
            ureq::Error::StatusCode(c) => FetchError::Status(c),
            other => FetchError::Network(other.to_string()),
        })?;
        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(FetchError::Status(status));
        }
        let mut reader = response.into_body().into_reader().take(max_bytes as u64 + 1);
        let mut bytes = Vec::new();
        let mut buf = vec![0_u8; 16 * 1024];
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(FetchError::Network("cancelled".into()));
            }
            let n = reader
                .read(&mut buf)
                .map_err(|e| FetchError::Network(e.to_string()))?;
            if n == 0 {
                break;
            }
            bytes.extend_from_slice(&buf[..n]);
            if bytes.len() > max_bytes {
                return Err(FetchError::TooLarge { cap: max_bytes });
            }
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_memory_transport_counts_caps_and_holds() {
        let mut m = MemoryFetch::default();
        assert_eq!(m.poll(), FetchPoll::Idle);
        m.serve("https://x/a", vec![1, 2, 3]);
        m.start("https://x/a", 3);
        assert_eq!(m.poll(), FetchPoll::Done(vec![1, 2, 3]));
        assert_eq!(m.poll(), FetchPoll::Idle);
        m.start("https://x/a", 2);
        assert_eq!(m.poll(), FetchPoll::Failed(FetchError::TooLarge { cap: 2 }));
        m.hold(true);
        m.start("https://x/b", 9);
        assert_eq!(m.poll(), FetchPoll::Pending);
        m.cancel();
        m.release();
        assert_eq!(m.poll(), FetchPoll::Idle);
        assert_eq!(m.requests().len(), 3);
        assert!(!NoFetch.available());
    }
}
