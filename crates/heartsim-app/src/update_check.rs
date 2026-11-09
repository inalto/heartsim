//! Update check: asks GitHub once per start whether a newer heartsim release exists and, if so, shows a
//! notice with a link to the release page and an "Update now" button.
//!
//! The check itself downloads nothing. Only when the user presses "Update now" and confirms in the dialog does
//! [`crate::update_install`] download this platform's installer, check it against the SHA-256 GitHub publishes
//! for the asset, and start it.
//!
//! Privacy: a single GET of the public releases API with a `User-Agent: heartsim/<version>` header and no
//! other identifier; nothing is stored. `Settings::check_for_updates` (on by default) turns it off, and then no
//! request is made at all. Every failure (offline, rate limit, bad JSON) is logged at debug level and is
//! otherwise silent: the app must start the same with or without a network.
//!
//! The network part exists only with the `update-check` cargo feature (desktop builds); without it, and in
//! the browser build, this module carries only the [`Release`] notice type the About window takes.

/// A release that is newer than the running version, as found on GitHub.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    /// The release version without the tag's leading `v`, as shown to the user (`0.7.0`).
    pub version: String,
    /// The release page, where the installers are attached.
    pub html_url: String,
    /// The files attached to the release (installers, checksums); empty when GitHub listed none.
    pub assets: Vec<ReleaseAsset>,
}

/// One file attached to a release, as the releases API lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseAsset {
    /// The file name (`heartsim_0.8.0_amd64.deb`); the installer is picked by its suffix.
    pub name: String,
    /// `browser_download_url`: the HTTPS address the file is downloaded from.
    pub url: String,
    /// The size in bytes GitHub reports; the download must have exactly this many.
    pub size: u64,
    /// The lower-case hex SHA-256 of the file from GitHub's `digest` field (`sha256:<hex>`), when listed.
    pub sha256: Option<String>,
}

#[cfg(feature = "update-check")]
pub use native::*;

#[cfg(feature = "update-check")]
mod native {
    use std::sync::mpsc::{self, Receiver};
    use std::time::Duration;

    use eframe::egui;
    use heartsim_ui::t;

    use super::{Release, ReleaseAsset};
    use crate::update_install::Updater;

    /// The GitHub releases API: the latest non-draft, non-pre-release release of heartsim.
    pub const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/inalto/heartsim/releases/latest";
    /// Whole-request timeout: a slow or unreachable network must not keep the thread around for long.
    pub const TIMEOUT: Duration = Duration::from_secs(5);
    /// Upper bound of the response body read: the release JSON is a few kilobytes plus the release notes.
    const MAX_BODY_BYTES: u64 = 1024 * 1024;
    /// The version this build is: what the GitHub tag is compared against.
    pub(crate) const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

    /// The running check: a thread that does the one request, a channel for its answer, and the notice
    /// state. Created once per start; the UI polls it each frame and nothing ever blocks.
    #[derive(Debug)]
    pub struct UpdateCheck {
        rx: Option<Receiver<Option<Release>>>,
        newer: Option<Release>,
        dismissed: bool,
        /// The update dialog, while it is open.
        updater: Option<Updater>,
    }

    impl UpdateCheck {
        /// Starts the check: spawns the request thread and returns at once.
        #[must_use]
        pub fn start() -> Self {
            let (tx, rx) = mpsc::channel();
            let spawned = std::thread::Builder::new()
                .name("update-check".into())
                .spawn(move || {
                    let result = fetch_latest();
                    // the receiver is gone when the app closed before the answer: nothing to do
                    let _ = tx.send(result);
                });
            if let Err(e) = &spawned {
                tracing::debug!("update check: could not spawn the request thread: {e}");
            }
            Self {
                rx: spawned.is_ok().then_some(rx),
                newer: None,
                dismissed: false,
                updater: None,
            }
        }

        /// Takes the thread's answer when it has arrived. Call once per frame; never blocks.
        pub fn poll(&mut self) {
            let Some(rx) = &self.rx else { return };
            match rx.try_recv() {
                Ok(found) => {
                    if let Some(r) = &found {
                        tracing::info!(
                            "update check: heartsim {} is available at {}",
                            r.version,
                            r.html_url
                        );
                    }
                    self.newer = found;
                    self.rx = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => self.rx = None,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }

        /// The update dialog is open (see [`crate::update_install::Updater`]).
        #[must_use]
        pub fn dialog_open(&self) -> bool {
            self.updater.is_some()
        }

        /// The newer release, when one was found (whether or not the notice bar was dismissed).
        #[must_use]
        pub fn newer(&self) -> Option<&Release> {
            self.newer.as_ref()
        }

        /// The thin bar under the tab bar: "heartsim <new> is available", an "Update now" button that opens
        /// the update dialog, the link to the release page and a Dismiss button that hides the bar for this
        /// run. Draws nothing until an answer with a newer release has arrived.
        pub fn notice_bar(&mut self, ui: &mut egui::Ui) {
            self.poll();
            let Some(release) = &self.newer else { return };
            if let Some(updater) = &mut self.updater
                && !updater.show(ui.ctx(), release)
            {
                self.updater = None;
            }
            if self.dismissed {
                return;
            }
            egui::Panel::top("update_notice").show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(t!("update-available", version = release.version.as_str()));
                    if ui.button(t!("update-install")).clicked() {
                        self.updater = Some(Updater::new());
                    }
                    ui.hyperlink_to(t!("update-download"), &release.html_url);
                    if ui.small_button(t!("update-dismiss")).clicked() {
                        self.dismissed = true;
                    }
                });
            });
        }
    }

    /// The one request: GET the latest release and keep it when it is newer than this build.
    /// `None` on any failure (logged at debug level) and when the latest release is not newer.
    fn fetch_latest() -> Option<Release> {
        let body = match fetch_body(LATEST_RELEASE_URL) {
            Ok(body) => body,
            Err(e) => {
                tracing::debug!("update check: request failed: {e}");
                return None;
            }
        };
        let Some(release) = parse_release(&body) else {
            tracing::debug!(
                "update check: unexpected response: {}",
                body.chars().take(200).collect::<String>()
            );
            return None;
        };
        is_newer(&release.version, CURRENT_VERSION).then_some(release)
    }

    /// The request itself: HTTPS only, the `heartsim/<version>` user agent, the timeout, a bounded body.
    fn fetch_body(url: &str) -> Result<String, ureq::Error> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(TIMEOUT))
            .https_only(true)
            .user_agent(format!("heartsim/{CURRENT_VERSION}"))
            .build()
            .into();
        agent
            .get(url)
            .header("Accept", "application/vnd.github+json")
            .call()?
            .body_mut()
            .with_config()
            .limit(MAX_BODY_BYTES)
            .read_to_string()
    }

    /// Reads `tag_name`, `html_url` and the `assets` from a GitHub release object; the tag's leading `v` is
    /// dropped. `None` when the text is not JSON or lacks either string field; an asset without a name, a
    /// download address or a size is left out.
    #[must_use]
    pub fn parse_release(json: &str) -> Option<Release> {
        let value: serde_json::Value = serde_json::from_str(json).ok()?;
        let tag = value.get("tag_name")?.as_str()?;
        let html_url = value.get("html_url")?.as_str()?;
        let assets = value
            .get("assets")
            .and_then(serde_json::Value::as_array)
            .map(|list| list.iter().filter_map(parse_asset).collect())
            .unwrap_or_default();
        Some(Release {
            version: strip_tag(tag).to_string(),
            html_url: html_url.to_string(),
            assets,
        })
    }

    /// One entry of a release's `assets`. The `digest` is kept only when it is a well-formed `sha256:<64 hex>`.
    fn parse_asset(value: &serde_json::Value) -> Option<ReleaseAsset> {
        let sha256 = value
            .get("digest")
            .and_then(serde_json::Value::as_str)
            .and_then(|d| d.strip_prefix("sha256:"))
            .filter(|hex| hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
            .map(str::to_ascii_lowercase);
        Some(ReleaseAsset {
            name: value.get("name")?.as_str()?.to_string(),
            url: value.get("browser_download_url")?.as_str()?.to_string(),
            size: value.get("size")?.as_u64()?,
            sha256,
        })
    }

    /// `v0.7.0` → `0.7.0`; surrounding whitespace is dropped too.
    fn strip_tag(tag: &str) -> &str {
        tag.trim().trim_start_matches('v')
    }

    /// True when `latest` (a release tag, with or without a leading `v`) is a release strictly newer than
    /// `current` by semver. A pre-release (`0.6.1-rc1`) never counts as newer: the user is on a release and
    /// should not be pointed at a candidate. Anything that is not a version is ignored (`false`).
    #[must_use]
    pub fn is_newer(latest: &str, current: &str) -> bool {
        let (Ok(latest), Ok(current)) = (
            semver::Version::parse(strip_tag(latest)),
            semver::Version::parse(strip_tag(current)),
        ) else {
            return false;
        };
        latest.pre.is_empty() && latest > current
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn a_later_release_tag_is_newer() {
            assert!(is_newer("v0.7.0", "0.6.0"));
            assert!(is_newer("0.7.0", "0.6.0"));
            assert!(is_newer("v1.0.0", "0.6.0"));
            assert!(is_newer("v0.6.1", "0.6.0"));
        }

        #[test]
        fn the_same_or_an_older_release_is_not_newer() {
            assert!(!is_newer("v0.6.0", "0.6.0"));
            assert!(!is_newer("0.6.0", "0.6.0"));
            assert!(!is_newer("v0.5.9", "0.6.0"));
            assert!(!is_newer("v0.6.0", "0.6.1"));
        }

        #[test]
        fn a_pre_release_never_counts_as_newer() {
            assert!(!is_newer("v0.6.1-rc1", "0.6.0"));
            assert!(!is_newer("v1.0.0-beta.2", "0.6.0"));
            // but a release is newer than the pre-release build the user runs
            assert!(is_newer("v0.7.0", "0.7.0-dev"));
        }

        #[test]
        fn garbage_is_ignored() {
            assert!(!is_newer("", "0.6.0"));
            assert!(!is_newer("latest", "0.6.0"));
            assert!(!is_newer("v0.7", "0.6.0"));
            assert!(!is_newer("v0.7.0", "not a version"));
        }

        #[test]
        fn the_parser_reads_tag_and_page_from_a_github_release() {
            // trimmed from a real `releases/latest` answer
            let sample = r#"{
              "url": "https://api.github.com/repos/inalto/heartsim/releases/1",
              "html_url": "https://github.com/inalto/heartsim/releases/tag/v0.7.0",
              "id": 1,
              "tag_name": "v0.7.0",
              "name": "heartsim 0.7.0",
              "draft": false,
              "prerelease": false,
              "assets": [
                {"name": "heartsim_0.7.0_amd64.deb", "size": 19312272,
                 "browser_download_url": "https://github.com/inalto/heartsim/releases/download/v0.7.0/heartsim_0.7.0_amd64.deb",
                 "digest": "sha256:D41F78066BE6BBEFE2F078EB8424A8646851AAA5A081EEB7E892E6BB82E14E4B"},
                {"name": "heartsim_0.7.0_x64-setup.exe", "size": 14721834,
                 "browser_download_url": "https://github.com/inalto/heartsim/releases/download/v0.7.0/heartsim_0.7.0_x64-setup.exe",
                 "digest": "md5:abc"},
                {"name": "no-url", "size": 1}
              ],
              "body": "Sixth tagged release."
            }"#;
            let release = parse_release(sample).expect("valid release JSON");
            assert_eq!(release.version, "0.7.0");
            assert_eq!(
                release.html_url,
                "https://github.com/inalto/heartsim/releases/tag/v0.7.0"
            );
            assert_eq!(
                release.assets.len(),
                2,
                "the asset without a download address is left out"
            );
            assert_eq!(release.assets[0].size, 19_312_272);
            assert_eq!(
                release.assets[0].sha256.as_deref(),
                Some("d41f78066be6bbefe2f078eb8424a8646851aaa5a081eeb7e892e6bb82e14e4b")
            );
            assert_eq!(release.assets[1].sha256, None, "only a sha256 digest is kept");
        }

        #[test]
        fn the_parser_rejects_bad_or_incomplete_json() {
            assert!(parse_release("").is_none());
            assert!(parse_release("<html>rate limited</html>").is_none());
            assert!(parse_release(r#"{"message": "Not Found"}"#).is_none());
            assert!(parse_release(r#"{"tag_name": "v0.7.0"}"#).is_none());
            assert!(parse_release(r#"{"tag_name": 7, "html_url": "x"}"#).is_none());
        }

        #[test]
        fn a_check_with_no_answer_yet_shows_nothing() {
            let check = UpdateCheck {
                rx: None,
                newer: None,
                dismissed: false,
                updater: None,
            };
            assert!(check.newer().is_none());
        }
    }
}
