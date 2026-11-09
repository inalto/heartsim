//! In-app update: the dialog that downloads this platform's installer of a newer release, checks it and
//! starts it.
//!
//! Opened from the update notice ([`crate::update_check::UpdateCheck`]); nothing happens until the user presses
//! "Download and install". Then a thread downloads the installer from the release's GitHub download address
//! (HTTPS only, nowhere else), shows its progress, and checks the file's size and SHA-256 against what the
//! releases API published for that asset. Only a file that matches is started:
//!
//! * Windows: the NSIS `-setup.exe` (or the MSI through `msiexec`); heartsim closes so the files can be replaced.
//! * macOS: the `.dmg` is opened in Finder for the usual drag to Applications; heartsim closes.
//! * Linux AppImage (`$APPIMAGE` set): the new AppImage replaces the running file and is started.
//! * Linux deb: `pkexec apt-get install` with the downloaded package (the desktop's password prompt), then
//!   heartsim restarts.
//!
//! Any other system (another CPU, a Linux without dpkg that is not running the AppImage) gets the link to the
//! release page instead. The download is kept in the temporary directory under `heartsim-update/`.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use eframe::egui;
use heartsim_ui::t;
use sha2::{Digest, Sha256};

use crate::update_check::{CURRENT_VERSION, Release, ReleaseAsset};

/// The only place installers are downloaded from: this project's GitHub release downloads (which redirect
/// to GitHub's object store, still HTTPS).
pub const DOWNLOAD_PREFIX: &str = "https://github.com/inalto/heartsim/releases/download/";
/// Upper bound of an installer download; the real ones are 15–30 MB.
const MAX_INSTALLER_BYTES: u64 = 512 * 1024 * 1024;
/// A stalled connection gives up after this long without data.
const READ_TIMEOUT: Duration = Duration::from_secs(30);

/// How this copy of heartsim was installed, which decides the installer it takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// Windows on x86-64: the NSIS setup (MSI as a fallback).
    Windows,
    /// macOS (the release's universal `.dmg`).
    MacOs,
    /// Linux x86-64 running from an AppImage (`$APPIMAGE` names the file).
    LinuxAppImage,
    /// Linux x86-64 with dpkg: the `.deb`.
    LinuxDeb,
    /// Anything the release has no installer for: the user downloads by hand.
    Unsupported,
}

impl Platform {
    /// The platform of the running program.
    #[must_use]
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
            Self::Windows
        } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            if std::env::var_os("APPIMAGE").is_some() {
                Self::LinuxAppImage
            } else if Path::new("/usr/bin/dpkg").exists() {
                Self::LinuxDeb
            } else {
                Self::Unsupported
            }
        } else {
            Self::Unsupported
        }
    }
}

/// The installer of `assets` for `platform`, by the file names cargo-packager and the release workflow give
/// them. Only assets downloadable from [`DOWNLOAD_PREFIX`] with a plain file name are considered.
#[must_use]
pub fn pick_asset(assets: &[ReleaseAsset], platform: Platform) -> Option<&ReleaseAsset> {
    let by_suffix = |suffix: &str| {
        assets.iter().find(|a| {
            a.name.ends_with(suffix) && a.url.starts_with(DOWNLOAD_PREFIX) && is_plain_file_name(&a.name)
        })
    };
    match platform {
        Platform::Windows => by_suffix("-setup.exe").or_else(|| by_suffix(".msi")),
        Platform::MacOs => by_suffix(".dmg"),
        Platform::LinuxAppImage => by_suffix(".AppImage"),
        Platform::LinuxDeb => by_suffix(".deb"),
        Platform::Unsupported => None,
    }
}

/// A name that cannot leave the download directory: no separators, no `..`, not empty.
fn is_plain_file_name(name: &str) -> bool {
    !name.is_empty() && !name.contains(['/', '\\', ':']) && name != "." && name != ".."
}

/// Lower-case hex of a digest.
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// Checks a finished download against the asset: the byte count always, the SHA-256 when GitHub listed one.
pub fn verify(asset: &ReleaseAsset, bytes: u64, sha256_hex: &str) -> Result<(), String> {
    if bytes != asset.size {
        return Err(format!(
            "downloaded {bytes} bytes, the release lists {}",
            asset.size
        ));
    }
    if let Some(expected) = &asset.sha256
        && !expected.eq_ignore_ascii_case(sha256_hex)
    {
        return Err(format!(
            "SHA-256 mismatch: got {sha256_hex}, the release lists {expected}"
        ));
    }
    Ok(())
}

/// What the worker thread reports to the dialog.
#[derive(Debug)]
enum Msg {
    Progress {
        done: u64,
        total: u64,
    },
    Installing,
    /// The installer is running (or the update is installed and the new version started): close heartsim.
    Finished,
    Failed(String),
}

/// Where the dialog is.
#[derive(Debug, Clone, PartialEq)]
enum State {
    Confirm,
    Downloading { done: u64, total: u64 },
    Installing,
    Closing,
    Failed(String),
}

/// The update dialog and its worker. Lives while the dialog is open.
#[derive(Debug)]
pub struct Updater {
    platform: Platform,
    state: State,
    rx: Option<Receiver<Msg>>,
    cancel: Arc<AtomicBool>,
}

impl Default for Updater {
    fn default() -> Self {
        Self::new()
    }
}

impl Updater {
    /// A dialog asking for confirmation; nothing is downloaded yet.
    #[must_use]
    pub fn new() -> Self {
        Self {
            platform: Platform::current(),
            state: State::Confirm,
            rx: None,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    fn start(&mut self, ctx: &egui::Context, asset: ReleaseAsset) {
        let (tx, rx) = mpsc::channel();
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = Arc::clone(&self.cancel);
        let platform = self.platform;
        let repaint = ctx.clone();
        let spawned = std::thread::Builder::new()
            .name("update-download".into())
            .spawn(move || {
                let send = |m: Msg| {
                    let _ = tx.send(m);
                    repaint.request_repaint();
                };
                let result = download(&asset, &cancel, &tx, &repaint).and_then(|file| {
                    send(Msg::Installing);
                    install(platform, &file)
                });
                send(match result {
                    Ok(()) => Msg::Finished,
                    Err(e) => Msg::Failed(e),
                });
            });
        match spawned {
            Ok(_) => {
                self.rx = Some(rx);
                self.state = State::Downloading { done: 0, total: 0 };
            }
            Err(e) => self.state = State::Failed(format!("could not start the download thread: {e}")),
        }
    }

    fn poll(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.rx else { return };
        loop {
            match rx.try_recv() {
                Ok(Msg::Progress { done, total }) => self.state = State::Downloading { done, total },
                Ok(Msg::Installing) => self.state = State::Installing,
                Ok(Msg::Finished) => {
                    self.state = State::Closing;
                    self.rx = None;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    return;
                }
                Ok(Msg::Failed(e)) => {
                    tracing::warn!("update failed: {e}");
                    self.state = State::Failed(e);
                    self.rx = None;
                    return;
                }
                Err(mpsc::TryRecvError::Empty) => return,
                Err(mpsc::TryRecvError::Disconnected) => {
                    if self.cancel.load(Ordering::Relaxed) {
                        self.state = State::Confirm;
                    } else if matches!(self.state, State::Downloading { .. } | State::Installing) {
                        self.state = State::Failed("the download thread stopped".into());
                    }
                    self.rx = None;
                    return;
                }
            }
        }
    }

    /// Draws the dialog for `release`. Returns `false` once it is closed (Cancel, Close, Escape or a click
    /// outside while nothing is running).
    pub fn show(&mut self, ctx: &egui::Context, release: &Release) -> bool {
        self.poll(ctx);
        let asset = pick_asset(&release.assets, self.platform).cloned();
        let busy = matches!(
            self.state,
            State::Downloading { .. } | State::Installing | State::Closing
        );
        let mut keep_open = true;
        let modal = egui::Modal::new(egui::Id::unique("update_dialog")).show(ctx, |ui| {
            ui.set_width(440.0);
            ui.heading(t!("update-modal-title", version = release.version.as_str()));
            ui.add_space(6.0);
            match (&self.state, &asset) {
                (State::Confirm, Some(asset)) => {
                    ui.label(t!(
                        "update-modal-file",
                        file = asset.name.as_str(),
                        size = megabytes(asset.size)
                    ));
                    ui.weak(t!("update-modal-note"));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button(t!("update-modal-start")).clicked() {
                            self.start(ctx, asset.clone());
                        }
                        if ui.button(t!("update-modal-cancel")).clicked() {
                            keep_open = false;
                        }
                    });
                }
                (State::Confirm, None) => {
                    ui.label(t!("update-modal-no-installer"));
                    ui.add_space(8.0);
                    if ui.button(t!("update-modal-close")).clicked() {
                        keep_open = false;
                    }
                }
                (State::Downloading { done, total }, _) => {
                    let fraction = if *total > 0 {
                        *done as f32 / *total as f32
                    } else {
                        0.0
                    };
                    ui.add(egui::ProgressBar::new(fraction).show_percentage().animate(true));
                    ui.label(t!(
                        "update-modal-progress",
                        done = megabytes(*done),
                        total = megabytes(*total)
                    ));
                    ui.add_space(8.0);
                    if ui.button(t!("update-modal-cancel")).clicked() {
                        self.cancel.store(true, Ordering::Relaxed);
                    }
                }
                (State::Installing, _) => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(t!("update-modal-installing"));
                    });
                }
                (State::Closing, _) => {
                    ui.label(t!("update-modal-closing"));
                }
                (State::Failed(error), _) => {
                    ui.colored_label(
                        ui.visuals().error_fg_color,
                        t!("update-modal-failed", error = error.as_str()),
                    );
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if let Some(asset) = &asset
                            && ui.button(t!("update-modal-retry")).clicked()
                        {
                            self.start(ctx, asset.clone());
                        }
                        if ui.button(t!("update-modal-close")).clicked() {
                            keep_open = false;
                        }
                    });
                }
            }
            ui.add_space(4.0);
            ui.hyperlink_to(t!("update-download"), &release.html_url);
        });
        if modal.should_close() && !busy {
            keep_open = false;
        }
        keep_open
    }
}

/// Bytes as megabytes with one decimal, for the dialog.
fn megabytes(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / 1_000_000.0)
}

/// Downloads `asset` into the temporary directory, reporting progress, and returns the verified file.
fn download(
    asset: &ReleaseAsset,
    cancel: &AtomicBool,
    tx: &Sender<Msg>,
    repaint: &egui::Context,
) -> Result<PathBuf, String> {
    if !asset.url.starts_with(DOWNLOAD_PREFIX) || !is_plain_file_name(&asset.name) {
        return Err(format!("refusing to download {}", asset.url));
    }
    if asset.size > MAX_INSTALLER_BYTES {
        return Err(format!("the installer is too large ({} bytes)", asset.size));
    }
    let dir = std::env::temp_dir().join("heartsim-update");
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let target = dir.join(&asset.name);
    let partial = dir.join(format!("{}.part", asset.name));

    let agent: ureq::Agent = ureq::Agent::config_builder()
        .https_only(true)
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_per_read(Some(READ_TIMEOUT))
        .user_agent(format!("heartsim/{CURRENT_VERSION}"))
        .build()
        .into();
    let response = agent
        .get(&asset.url)
        .header("Accept", "application/octet-stream")
        .call()
        .map_err(|e| format!("download failed: {e}"))?;
    let total = response.body().content_length().unwrap_or(asset.size);
    let mut reader = response
        .into_body()
        .into_with_config()
        .limit(MAX_INSTALLER_BYTES)
        .reader();

    let mut file = File::create(&partial).map_err(|e| format!("cannot write {}: {e}", partial.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0_u8; 64 * 1024];
    let mut done = 0_u64;
    loop {
        if cancel.load(Ordering::Relaxed) {
            drop(file);
            let _ = std::fs::remove_file(&partial);
            return Err("cancelled".into());
        }
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("download failed: {e}"))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])
            .map_err(|e| format!("cannot write {}: {e}", partial.display()))?;
        hasher.update(&buf[..n]);
        done += n as u64;
        let _ = tx.send(Msg::Progress { done, total });
        repaint.request_repaint();
    }
    file.sync_all()
        .map_err(|e| format!("cannot write {}: {e}", partial.display()))?;
    drop(file);

    if let Err(e) = verify(asset, done, &hex(&hasher.finalize())) {
        let _ = std::fs::remove_file(&partial);
        return Err(e);
    }
    std::fs::rename(&partial, &target).map_err(|e| format!("cannot move the download into place: {e}"))?;
    tracing::info!("update: downloaded and verified {}", target.display());
    Ok(target)
}

/// Starts the verified installer the way `platform` needs (see the module documentation).
fn install(platform: Platform, file: &Path) -> Result<(), String> {
    let spawn = |cmd: &mut Command| {
        cmd.spawn()
            .map(drop)
            .map_err(|e| format!("cannot start {}: {e}", file.display()))
    };
    match platform {
        Platform::Windows => {
            if file.extension().is_some_and(|e| e.eq_ignore_ascii_case("msi")) {
                spawn(Command::new("msiexec").arg("/i").arg(file))
            } else {
                spawn(&mut Command::new(file))
            }
        }
        Platform::MacOs => {
            let status = Command::new("open")
                .arg(file)
                .status()
                .map_err(|e| format!("cannot open {}: {e}", file.display()))?;
            status
                .success()
                .then_some(())
                .ok_or_else(|| format!("`open` failed with {status}"))
        }
        Platform::LinuxAppImage => replace_app_image(file),
        Platform::LinuxDeb => install_deb(file),
        Platform::Unsupported => Err("there is no installer for this system".into()),
    }
}

/// Puts the new AppImage where `$APPIMAGE` points (copy beside it, then an atomic rename) and starts it.
#[cfg(unix)]
fn replace_app_image(file: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let target = std::env::var_os("APPIMAGE")
        .map(PathBuf::from)
        .ok_or("APPIMAGE is not set")?;
    let staged = target.with_extension("AppImage.new");
    std::fs::copy(file, &staged).map_err(|e| format!("cannot write {}: {e}", staged.display()))?;
    std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| format!("cannot make {} executable: {e}", staged.display()))?;
    std::fs::rename(&staged, &target).map_err(|e| {
        let _ = std::fs::remove_file(&staged);
        format!("cannot replace {}: {e}", target.display())
    })?;
    Command::new(&target)
        .spawn()
        .map(drop)
        .map_err(|e| format!("installed, but cannot start {}: {e}", target.display()))
}

#[cfg(not(unix))]
fn replace_app_image(_file: &Path) -> Result<(), String> {
    Err("AppImages exist only on Linux".into())
}

/// Installs the `.deb` through `pkexec apt-get install` (the desktop asks for the password) and starts the
/// installed program again.
fn install_deb(file: &Path) -> Result<(), String> {
    // read before apt replaces the file: afterwards /proc/self/exe names the deleted inode
    let exe = std::env::current_exe().map_err(|e| format!("cannot find the running program: {e}"))?;
    let status = Command::new("pkexec")
        .args(["apt-get", "install", "-y", "--allow-downgrades"])
        .arg(file)
        .status()
        .map_err(|e| format!("cannot run pkexec ({e}); install {} by hand", file.display()))?;
    if !status.success() {
        return Err(format!(
            "the package manager stopped ({status}); the package is at {}",
            file.display()
        ));
    }
    Command::new(&exe)
        .spawn()
        .map(drop)
        .map_err(|e| format!("installed, but cannot start {}: {e}", exe.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(name: &str) -> ReleaseAsset {
        ReleaseAsset {
            name: name.into(),
            url: format!("{DOWNLOAD_PREFIX}v9.9.9/{name}"),
            size: 3,
            sha256: None,
        }
    }

    fn release_assets() -> Vec<ReleaseAsset> {
        [
            "heartsim_9.9.9_amd64.deb",
            "HeartSim_9.9.9_universal.dmg",
            "heartsim_9.9.9_x64_en-US.msi",
            "heartsim_9.9.9_x64-setup.exe",
            "heartsim_9.9.9_x86_64.AppImage",
            "SHA256SUMS",
        ]
        .into_iter()
        .map(asset)
        .collect()
    }

    #[test]
    fn each_platform_takes_its_own_installer() {
        let assets = release_assets();
        let name = |p| pick_asset(&assets, p).map(|a| a.name.as_str());
        assert_eq!(name(Platform::Windows), Some("heartsim_9.9.9_x64-setup.exe"));
        assert_eq!(name(Platform::MacOs), Some("HeartSim_9.9.9_universal.dmg"));
        assert_eq!(
            name(Platform::LinuxAppImage),
            Some("heartsim_9.9.9_x86_64.AppImage")
        );
        assert_eq!(name(Platform::LinuxDeb), Some("heartsim_9.9.9_amd64.deb"));
        assert_eq!(name(Platform::Unsupported), None);
    }

    #[test]
    fn windows_falls_back_to_the_msi() {
        let assets: Vec<_> = release_assets()
            .into_iter()
            .filter(|a| !a.name.ends_with("-setup.exe"))
            .collect();
        assert_eq!(
            pick_asset(&assets, Platform::Windows).map(|a| a.name.as_str()),
            Some("heartsim_9.9.9_x64_en-US.msi")
        );
    }

    #[test]
    fn only_this_projects_release_downloads_are_taken() {
        let mut elsewhere = asset("heartsim_9.9.9_amd64.deb");
        elsewhere.url = "https://example.com/heartsim_9.9.9_amd64.deb".into();
        assert!(pick_asset(&[elsewhere], Platform::LinuxDeb).is_none());
        let mut http = asset("heartsim_9.9.9_amd64.deb");
        http.url = http.url.replacen("https", "http", 1);
        assert!(pick_asset(&[http], Platform::LinuxDeb).is_none());
        let sneaky = ReleaseAsset {
            name: "../../evil.deb".into(),
            ..asset("x.deb")
        };
        assert!(pick_asset(&[sneaky], Platform::LinuxDeb).is_none());
    }

    #[test]
    fn a_download_must_match_the_listed_size_and_digest() {
        let digest = hex(&Sha256::digest(b"abc"));
        assert_eq!(
            digest,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let mut a = asset("x.deb");
        assert!(
            verify(&a, 3, &digest).is_ok(),
            "no digest listed: the size is enough"
        );
        assert!(verify(&a, 2, &digest).is_err(), "a short download is rejected");
        a.sha256 = Some(digest.to_uppercase());
        assert!(verify(&a, 3, &digest).is_ok());
        a.sha256 = Some("0".repeat(64));
        assert!(verify(&a, 3, &digest).is_err(), "a different file is rejected");
    }

    #[test]
    fn megabytes_have_one_decimal() {
        assert_eq!(megabytes(19_312_272), "19.3");
        assert_eq!(megabytes(0), "0.0");
    }
}
