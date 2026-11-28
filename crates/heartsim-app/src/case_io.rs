//! The platform part of case files (brief B41): asking the user for a `.hs` file to open, saving
//! one, and keeping the user's cases ("My cases") between runs. The Cases page
//! ([`crate::case_ui`]) talks only to the two traits here, so it has no `cfg`.
//!
//! * [`CaseFileIo`]: open and save. [`DialogIo`] uses `rfd` on both targets: the native file
//!   dialogs on the desktop (an `AsyncFileDialog` driven by `pollster` on a short-lived thread, so
//!   the UI never blocks and macOS gets its dialog on the main thread), and in the browser a file
//!   picker and a download (`rfd`'s web backend, driven by `wasm_bindgen_futures::spawn_local`).
//!   The result is handed to the UI through a shared queue polled each frame, the pattern of
//!   [`crate::update_check`]. [`NoIo`] does nothing (tests, the documentation tour).
//! * [`CaseStore`]: where My cases live. [`DirStore`] (desktop): one `<id>.hs` per case in
//!   `<user data dir>/cases/`, next to the saved scenarios. [`StorageStore`] (browser): every case
//!   base64-encoded in one eframe storage key (`localStorage`), at most [`STORAGE_CAP_BYTES`] in all;
//!   without storage it keeps them for the session only (tests use it that way).

use heartsim_casefile::{CaseFileError, MAX_FILE_BYTES, valid_id};
use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

/// What an open or save request came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IoEvent {
    /// The user chose a file: its name (for messages) and its bytes (not yet checked).
    Opened { name: String, bytes: Vec<u8> },
    /// The file was written (desktop: the path; browser: the download's name).
    Saved { name: String },
    /// The dialog was closed without a choice.
    Cancelled,
    /// Reading or writing failed; the text is shown to the user.
    Failed(String),
}

/// Open and save dialogs, started by a click and finished later (the result comes from
/// [`Self::poll`]).
pub trait CaseFileIo {
    /// Ask the user for a case file to open.
    fn open(&mut self);
    /// Offer `bytes` to be saved as `file_name` (a dialog on the desktop, a download in the browser).
    fn save(&mut self, file_name: &str, bytes: Vec<u8>);
    /// The next finished request, if any. Call once per frame.
    fn poll(&mut self) -> Option<IoEvent>;
    /// `true` while a dialog is open (the page disables its buttons meanwhile).
    fn busy(&self) -> bool;
}

/// No dialogs: requests are ignored. Tests drive the page through `open_bytes` instead.
#[derive(Debug, Default)]
pub struct NoIo;

impl CaseFileIo for NoIo {
    fn open(&mut self) {}
    fn save(&mut self, _file_name: &str, _bytes: Vec<u8>) {}
    fn poll(&mut self) -> Option<IoEvent> {
        None
    }
    fn busy(&self) -> bool {
        false
    }
}

/// Native file dialogs on the desktop, the file picker and a download in the browser (`rfd`).
#[derive(Debug, Default)]
pub struct DialogIo {
    shared: Arc<Mutex<Shared>>,
}

#[derive(Debug, Default)]
struct Shared {
    events: VecDeque<IoEvent>,
    running: bool,
}

impl DialogIo {
    fn start(&self) -> bool {
        let Ok(mut s) = self.shared.lock() else {
            return false;
        };
        if s.running {
            return false;
        }
        s.running = true;
        true
    }

    fn finish(shared: &Arc<Mutex<Shared>>, event: IoEvent) {
        if let Ok(mut s) = shared.lock() {
            s.events.push_back(event);
            s.running = false;
        }
    }
}

/// The `.hs` filter of the dialogs.
fn dialog() -> rfd::AsyncFileDialog {
    rfd::AsyncFileDialog::new().add_filter("heartsim case", &[heartsim_casefile::EXTENSION])
}

impl CaseFileIo for DialogIo {
    fn open(&mut self) {
        if !self.start() {
            return;
        }
        let shared = Arc::clone(&self.shared);
        spawn(async move {
            let event = match dialog().pick_file().await {
                None => IoEvent::Cancelled,
                Some(handle) => read_handle(&handle).await,
            };
            DialogIo::finish(&shared, event);
        });
    }

    fn save(&mut self, file_name: &str, bytes: Vec<u8>) {
        if !self.start() {
            return;
        }
        let shared = Arc::clone(&self.shared);
        let name = file_name.to_owned();
        spawn(async move {
            let event = match dialog().set_file_name(name.clone()).save_file().await {
                None => IoEvent::Cancelled,
                Some(handle) => match handle.write(&bytes).await {
                    Ok(()) => IoEvent::Saved {
                        name: saved_name(&handle, &name),
                    },
                    Err(e) => IoEvent::Failed(e.to_string()),
                },
            };
            DialogIo::finish(&shared, event);
        });
    }

    fn poll(&mut self) -> Option<IoEvent> {
        self.shared.lock().ok()?.events.pop_front()
    }

    fn busy(&self) -> bool {
        self.shared.lock().is_ok_and(|s| s.running)
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn(f: impl std::future::Future<Output = ()> + Send + 'static) {
    std::thread::spawn(move || pollster::block_on(f));
}

#[cfg(target_arch = "wasm32")]
fn spawn(f: impl std::future::Future<Output = ()> + 'static) {
    wasm_bindgen_futures::spawn_local(f);
}

/// The chosen file's bytes; on the desktop its size is checked before it is read.
#[cfg_attr(not(target_arch = "wasm32"), allow(clippy::unused_async))]
async fn read_handle(handle: &rfd::FileHandle) -> IoEvent {
    let name = handle.file_name();
    #[cfg(not(target_arch = "wasm32"))]
    {
        match read_path_capped(handle.path()) {
            Ok(bytes) => IoEvent::Opened { name, bytes },
            Err(e) => IoEvent::Failed(format!("{name}: {e}")),
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let bytes = handle.read().await;
        if bytes.len() > MAX_FILE_BYTES {
            return IoEvent::Failed(format!(
                "{name}: {}",
                CaseFileError::FileTooLarge { size: bytes.len() }
            ));
        }
        IoEvent::Opened { name, bytes }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn saved_name(handle: &rfd::FileHandle, _name: &str) -> String {
    handle.path().display().to_string()
}

#[cfg(target_arch = "wasm32")]
fn saved_name(_handle: &rfd::FileHandle, name: &str) -> String {
    name.to_owned()
}

/// A file of at most [`MAX_FILE_BYTES`], read without trusting its reported size.
///
/// # Errors
/// When it cannot be read or is too large.
pub fn read_path_capped(path: &std::path::Path) -> Result<Vec<u8>, String> {
    use std::io::Read as _;
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_FILE_BYTES {
        return Err(CaseFileError::FileTooLarge { size: bytes.len() }.to_string());
    }
    Ok(bytes)
}

/// Reads the `.hs` files dropped on the window (any page): at once from their path on the desktop
/// (with the size cap), asynchronously in the browser (`DroppedFile::bytes_async`, the size is
/// checked after reading). Results come from [`Self::poll`] as `(file name, bytes or error)`.
#[derive(Debug, Default)]
pub struct DropReader {
    queue: Arc<Mutex<VecDeque<Dropped>>>,
}

/// A dropped file: its name and its bytes, or why they could not be read.
pub type Dropped = (String, Result<Vec<u8>, String>);

impl DropReader {
    /// Take this frame's dropped case files; other files are left for the Import window.
    pub fn take(&self, ctx: &egui::Context) {
        let files: Vec<egui::DroppedFileHandle> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter(|f| f.path().to_str().is_some_and(is_case_file_name))
                .cloned()
                .collect()
        });
        for f in files {
            let name = f
                .path()
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            #[cfg(not(target_arch = "wasm32"))]
            if let Ok(mut q) = self.queue.lock() {
                q.push_back((name, read_path_capped(f.path())));
            }
            #[cfg(target_arch = "wasm32")]
            {
                let queue = Arc::clone(&self.queue);
                wasm_bindgen_futures::spawn_local(async move {
                    let bytes = f.bytes_async().await.and_then(|b| {
                        if b.len() > MAX_FILE_BYTES {
                            Err(CaseFileError::FileTooLarge { size: b.len() }.to_string())
                        } else {
                            Ok(b)
                        }
                    });
                    if let Ok(mut q) = queue.lock() {
                        q.push_back((name, bytes));
                    }
                });
            }
        }
    }

    /// The next dropped file that was read.
    pub fn poll(&self) -> Option<Dropped> {
        self.queue.lock().ok()?.pop_front()
    }
}

/// Whether a file name looks like a case file (`*.hs`, any case).
#[must_use]
pub fn is_case_file_name(name: &str) -> bool {
    std::path::Path::new(name)
        .extension()
        .is_some_and(|x| x.eq_ignore_ascii_case(heartsim_casefile::EXTENSION))
}

/// Where My cases are kept between runs. Keys are case ids (`[a-z0-9_]`, checked by the format).
pub trait CaseStore {
    /// Every stored case file as `(id, bytes)`, in id order.
    fn load(&mut self) -> Vec<(String, Vec<u8>)>;
    /// Keep `bytes` under `id`, replacing what was there.
    ///
    /// # Errors
    /// When it cannot be kept (the text is shown to the user; the case stays open for the session).
    fn put(&mut self, id: &str, bytes: &[u8]) -> Result<(), String>;
    /// Forget `id`.
    ///
    /// # Errors
    /// When the stored copy cannot be deleted.
    fn remove(&mut self, id: &str) -> Result<(), String>;
    /// Where the cases are kept, in words, for the page.
    fn describe(&self) -> String;
    /// The text the host must write into eframe storage under [`STORAGE_KEY`], when it changed
    /// (only [`StorageStore`] with persistence has any).
    fn take_storage_text(&mut self) -> Option<String> {
        None
    }
}

/// eframe storage key of My cases in the browser.
pub const STORAGE_KEY: &str = "my_cases";
/// The most My cases may take in browser storage, bytes of base64 text (1 MiB; `localStorage`
/// usually allows about 5 MiB per site, shared with the settings).
pub const STORAGE_CAP_BYTES: usize = 1024 * 1024;

/// My cases in memory, written into one eframe storage key as `id<TAB>base64` lines (browser), or
/// for the session only (`persistent == false`).
#[derive(Debug, Default)]
pub struct StorageStore {
    entries: BTreeMap<String, String>,
    persistent: bool,
    dirty: bool,
}

impl StorageStore {
    /// The store kept in eframe storage, starting from the text stored under [`STORAGE_KEY`].
    /// Lines that do not decode are dropped.
    #[must_use]
    pub fn persistent(text: Option<&str>) -> Self {
        let mut entries = BTreeMap::new();
        for line in text.unwrap_or_default().lines() {
            if let Some((id, b64)) = line.split_once('\t')
                && valid_id(id)
            {
                entries.insert(id.to_owned(), b64.trim().to_owned());
            }
        }
        Self {
            entries,
            persistent: true,
            dirty: false,
        }
    }

    /// A store that keeps the cases for this session only.
    #[must_use]
    pub fn session() -> Self {
        Self::default()
    }

    fn text(&self) -> String {
        let mut s = String::new();
        for (id, b64) in &self.entries {
            s.push_str(id);
            s.push('\t');
            s.push_str(b64);
            s.push('\n');
        }
        s
    }
}

impl CaseStore for StorageStore {
    fn load(&mut self) -> Vec<(String, Vec<u8>)> {
        use base64::Engine as _;
        self.entries
            .iter()
            .filter_map(|(id, b64)| {
                base64::engine::general_purpose::STANDARD
                    .decode(b64)
                    .ok()
                    .map(|b| (id.clone(), b))
            })
            .collect()
    }

    fn put(&mut self, id: &str, bytes: &[u8]) -> Result<(), String> {
        use base64::Engine as _;
        if !valid_id(id) {
            return Err(format!("`{id}` is not a valid case id"));
        }
        let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
        let others: usize = self
            .entries
            .iter()
            .filter(|(k, _)| k.as_str() != id)
            .map(|(k, v)| k.len() + v.len() + 2)
            .sum();
        if self.persistent && others + id.len() + b64.len() + 2 > STORAGE_CAP_BYTES {
            return Err(heartsim_ui::t!(
                "cases-storage-full",
                cap = (STORAGE_CAP_BYTES / 1024).to_string()
            ));
        }
        self.entries.insert(id.to_owned(), b64);
        self.dirty = true;
        Ok(())
    }

    fn remove(&mut self, id: &str) -> Result<(), String> {
        self.entries.remove(id);
        self.dirty = true;
        Ok(())
    }

    fn describe(&self) -> String {
        if self.persistent {
            heartsim_ui::t!("cases-kept-browser", cap = (STORAGE_CAP_BYTES / 1024).to_string())
        } else {
            heartsim_ui::t!("cases-kept-session")
        }
    }

    fn take_storage_text(&mut self) -> Option<String> {
        if self.persistent && self.dirty {
            self.dirty = false;
            Some(self.text())
        } else {
            None
        }
    }
}

/// My cases as `<id>.hs` files in a directory (desktop: `<user data dir>/cases/`).
#[cfg(feature = "native")]
#[derive(Debug)]
pub struct DirStore {
    pub dir: std::path::PathBuf,
}

#[cfg(feature = "native")]
impl DirStore {
    /// `<user data dir>/cases`, next to [`crate::scenario_file::user_scenario_dir`].
    #[must_use]
    pub fn user_dir() -> Option<Self> {
        directories::ProjectDirs::from("", "", "heartsim").map(|d| Self {
            dir: d.data_dir().join("cases"),
        })
    }

    fn path(&self, id: &str) -> Result<std::path::PathBuf, String> {
        if !valid_id(id) {
            return Err(format!("`{id}` is not a valid case id"));
        }
        Ok(self.dir.join(format!("{id}.{}", heartsim_casefile::EXTENSION)))
    }
}

#[cfg(feature = "native")]
impl CaseStore for DirStore {
    fn load(&mut self) -> Vec<(String, Vec<u8>)> {
        let Ok(read) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut out: Vec<(String, Vec<u8>)> = read
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_file() && p.to_str().is_some_and(is_case_file_name))
            .filter_map(|p| {
                let id = p.file_stem()?.to_string_lossy().into_owned();
                if !valid_id(&id) {
                    return None;
                }
                read_path_capped(&p)
                    .inspect_err(|e| tracing::warn!("{}: {e}", p.display()))
                    .ok()
                    .map(|b| (id, b))
            })
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    fn put(&mut self, id: &str, bytes: &[u8]) -> Result<(), String> {
        let path = self.path(id)?;
        std::fs::create_dir_all(&self.dir).map_err(|e| format!("{}: {e}", self.dir.display()))?;
        std::fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))
    }

    fn remove(&mut self, id: &str) -> Result<(), String> {
        let path = self.path(id)?;
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    fn describe(&self) -> String {
        heartsim_ui::t!("cases-kept-dir", dir = self.dir.display().to_string())
    }
}

/// Today's date as `YYYY-MM-DD` (UTC), for the manifest's informational `created` field.
#[must_use]
pub fn today_iso() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        let d = js_sys::Date::new_0();
        format!(
            "{:04}-{:02}-{:02}",
            d.get_utc_full_year(),
            d.get_utc_month() + 1,
            d.get_utc_date()
        )
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        civil_date((secs / 86_400).cast_signed())
    }
}

/// `YYYY-MM-DD` of a day count since 1970-01-01 (H. Hinnant's civil-from-days algorithm).
#[must_use]
pub fn civil_date(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates() {
        assert_eq!(civil_date(0), "1970-01-01");
        assert_eq!(civil_date(11_016), "2000-02-29");
        assert_eq!(civil_date(20_784), "2026-11-27");
        assert_eq!(today_iso().len(), 10);
    }

    #[test]
    fn case_file_names() {
        assert!(is_case_file_name("a.hs"));
        assert!(is_case_file_name("dir/A.HS"));
        assert!(!is_case_file_name("a.hs.zip"));
        assert!(!is_case_file_name("a.ron"));
    }

    #[test]
    fn the_browser_store_round_trips_and_refuses_beyond_its_cap() {
        let mut s = StorageStore::persistent(None);
        s.put("case_a", b"abc").unwrap();
        s.put("case_b", &[0, 1, 2, 255]).unwrap();
        let text = s.take_storage_text().expect("changed");
        assert!(s.take_storage_text().is_none(), "written once");
        let mut back = StorageStore::persistent(Some(&text));
        assert_eq!(
            back.load(),
            [
                ("case_a".to_owned(), b"abc".to_vec()),
                ("case_b".to_owned(), vec![0, 1, 2, 255])
            ]
        );
        // base64 grows 4/3: three quarters of the cap no longer fits next to the others
        let big = vec![7u8; STORAGE_CAP_BYTES * 3 / 4];
        assert!(back.put("case_c", &big).is_err());
        back.remove("case_a").unwrap();
        assert_eq!(back.load().len(), 1);
        assert!(back.put("Bad Id", b"x").is_err());
        // the session store has no cap and nothing to write
        let mut session = StorageStore::session();
        session.put("case_c", &big).unwrap();
        assert!(session.take_storage_text().is_none());
    }

    #[cfg(feature = "native")]
    #[test]
    fn the_directory_store_keeps_one_file_per_case() {
        let dir = std::env::temp_dir().join(format!("heartsim-b41-store-{}", std::process::id()));
        let mut s = DirStore { dir: dir.clone() };
        assert_eq!(s.load().len(), 0);
        s.put("case_a", b"one").unwrap();
        s.put("case_a", b"two").unwrap();
        std::fs::write(dir.join("notes.txt"), b"ignored").unwrap();
        assert_eq!(s.load(), [("case_a".to_owned(), b"two".to_vec())]);
        assert!(dir.join("case_a.hs").is_file());
        assert!(s.put("../escape", b"x").is_err());
        s.remove("case_a").unwrap();
        s.remove("case_a").unwrap();
        assert_eq!(s.load().len(), 0);
        let _ = std::fs::remove_dir_all(dir);
    }
}
