//! Update check: asks GitHub once per start whether a newer heartsim release exists and, if so, shows a
//! notice with a link to the release page.
//!
//! This is a check and a link only. Nothing is downloaded or replaced: the installers (deb, AppImage,
//! MSI, NSIS, dmg) are unsigned and platform-specific, so installing one is the user's step in the browser
//! and the package manager, not the running program's.
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
}

#[cfg(feature = "update-check")]
pub use native::*;

#[cfg(feature = "update-check")]
mod native {
    use std::sync::mpsc::{self, Receiver};
    use std::time::Duration;

    use eframe::egui;
    use heartsim_ui::t;

    use super::Release;

    /// The GitHub releases API: the latest non-draft, non-pre-release release of heartsim.
    pub const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/inalto/heartsim/releases/latest";
    /// Whole-request timeout: a slow or unreachable network must not keep the thread around for long.
    pub const TIMEOUT: Duration = Duration::from_secs(5);
    /// Upper bound of the response body read: the release JSON is a few kilobytes.
    const MAX_BODY_BYTES: u64 = 256 * 1024;
    /// The version this build is: what the GitHub tag is compared against.
    const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

    /// The running check: a thread that does the one request, a channel for its answer, and the notice
    /// state. Created once per start; the UI polls it each frame and nothing ever blocks.
    #[derive(Debug)]
    pub struct UpdateCheck {
        rx: Option<Receiver<Option<Release>>>,
        newer: Option<Release>,
        dismissed: bool,
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

        /// The newer release, when one was found (whether or not the notice bar was dismissed).
        #[must_use]
        pub fn newer(&self) -> Option<&Release> {
            self.newer.as_ref()
        }

        /// The thin bar under the tab bar: "heartsim <new> is available", the link to the release page and a
        /// Dismiss button that hides the bar for this run. Draws nothing until an answer with a newer
        /// release has arrived.
        pub fn notice_bar(&mut self, ui: &mut egui::Ui) {
            self.poll();
            if self.dismissed {
                return;
            }
            let Some(release) = &self.newer else { return };
            egui::Panel::top("update_notice").show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(t!("update-available", version = release.version.as_str()));
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

    /// Reads `tag_name` and `html_url` from a GitHub release object; the tag's leading `v` is dropped.
    /// `None` when the text is not JSON or lacks either string field.
    #[must_use]
    pub fn parse_release(json: &str) -> Option<Release> {
        let value: serde_json::Value = serde_json::from_str(json).ok()?;
        let tag = value.get("tag_name")?.as_str()?;
        let html_url = value.get("html_url")?.as_str()?;
        Some(Release {
            version: strip_tag(tag).to_string(),
            html_url: html_url.to_string(),
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
              "assets": [{"name": "heartsim_0.7.0_amd64.deb", "size": 1}],
              "body": "Sixth tagged release."
            }"#;
            let release = parse_release(sample).expect("valid release JSON");
            assert_eq!(release.version, "0.7.0");
            assert_eq!(
                release.html_url,
                "https://github.com/inalto/heartsim/releases/tag/v0.7.0"
            );
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
            };
            assert!(check.newer().is_none());
        }
    }
}
