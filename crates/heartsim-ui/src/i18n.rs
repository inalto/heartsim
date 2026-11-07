//! Localisation with Project Fluent (B19).
//!
//! Message catalogues live in `assets/locales/<language>/*.ftl` and are embedded at compile time, so
//! the lookup needs no file I/O and the same code runs on the web build. English is the source
//! language: a key missing from the active language falls back to English, and a key missing from
//! both shows the key itself so the gap is visible on screen rather than silently blank.
//!
//! Use the [`t!`](crate::t) macro for every user-visible string:
//!
//! ```ignore
//! ui.button(t!("transport-pause"));
//! ui.label(t!("ecg-saved", path = file_name));
//! ```
//!
//! The active language is per thread (egui runs its UI on one thread); the app sets it once at
//! start-up and again whenever the user changes the language in Settings.

use std::cell::RefCell;

use fluent::{FluentArgs, FluentBundle, FluentResource, FluentValue};
use unic_langid::LanguageIdentifier;

/// The source language; every key must exist here.
pub const SOURCE_LANGUAGE: &str = "en";

/// A language heartsim ships a catalogue for, named in its own language for the Settings menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Language {
    /// BCP-47 primary subtag, also the directory name under `assets/locales/`.
    pub code: &'static str,
    /// The language's own name (endonym), so a user who cannot read the current language can still find theirs.
    pub name: &'static str,
}

/// Every shipped language. German, French and Spanish are stubs: they carry a few keys and fall
/// back to English for the rest.
pub const LANGUAGES: [Language; 5] = [
    Language {
        code: "en",
        name: "English",
    },
    Language {
        code: "it",
        name: "Italiano",
    },
    Language {
        code: "de",
        name: "Deutsch",
    },
    Language {
        code: "fr",
        name: "Français",
    },
    Language {
        code: "es",
        name: "Español",
    },
];

/// Embedded catalogues: `(language, file name, source)`. `tests/i18n.rs` checks this list against
/// the files on disk, so a new `.ftl` file cannot be forgotten here.
pub const CATALOGUES: &[(&str, &str, &str)] = &[
    (
        "en",
        "app.ftl",
        include_str!("../../../assets/locales/en/app.ftl"),
    ),
    ("en", "ui.ftl", include_str!("../../../assets/locales/en/ui.ftl")),
    ("en", "ep.ftl", include_str!("../../../assets/locales/en/ep.ftl")),
    (
        "it",
        "app.ftl",
        include_str!("../../../assets/locales/it/app.ftl"),
    ),
    ("it", "ui.ftl", include_str!("../../../assets/locales/it/ui.ftl")),
    ("it", "ep.ftl", include_str!("../../../assets/locales/it/ep.ftl")),
    (
        "de",
        "app.ftl",
        include_str!("../../../assets/locales/de/app.ftl"),
    ),
    ("de", "ui.ftl", include_str!("../../../assets/locales/de/ui.ftl")),
    (
        "fr",
        "app.ftl",
        include_str!("../../../assets/locales/fr/app.ftl"),
    ),
    ("fr", "ui.ftl", include_str!("../../../assets/locales/fr/ui.ftl")),
    (
        "es",
        "app.ftl",
        include_str!("../../../assets/locales/es/app.ftl"),
    ),
    ("es", "ui.ftl", include_str!("../../../assets/locales/es/ui.ftl")),
];

/// Picks the shipped language closest to a system locale string such as `it-IT`, `it_IT.UTF-8`
/// or `de`. Anything unknown or missing gives English.
#[must_use]
pub fn negotiate(system_locale: Option<&str>) -> &'static str {
    let Some(raw) = system_locale else {
        return SOURCE_LANGUAGE;
    };
    let primary = raw
        .split(['-', '_', '.', '@'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    LANGUAGES
        .iter()
        .find(|l| l.code == primary)
        .map_or(SOURCE_LANGUAGE, |l| l.code)
}

/// The language reported by the operating system, if it can be read.
#[must_use]
pub fn system_locale() -> Option<String> {
    sys_locale::get_locale()
}

/// The active language and its bundles: the chosen one plus English as the fallback.
struct Localizer {
    language: &'static str,
    primary: FluentBundle<FluentResource>,
    fallback: FluentBundle<FluentResource>,
}

fn bundle_for(code: &str) -> FluentBundle<FluentResource> {
    let langid: LanguageIdentifier = code
        .parse()
        .unwrap_or_else(|_| SOURCE_LANGUAGE.parse().expect("en is valid"));
    let mut bundle = FluentBundle::new(vec![langid]);
    // Fluent wraps placeables in invisible bidi isolation marks; egui text and tests want plain text.
    bundle.set_use_isolating(false);
    for (lang, name, src) in CATALOGUES.iter().filter(|(lang, _, _)| *lang == code) {
        match FluentResource::try_new((*src).to_string()) {
            Ok(res) => {
                if let Err(errors) = bundle.add_resource(res) {
                    tracing::warn!("{lang}/{name}: {} conflicting message id(s)", errors.len());
                }
            }
            Err((res, errors)) => {
                tracing::warn!("{lang}/{name}: {} syntax error(s)", errors.len());
                let _ = bundle.add_resource(res);
            }
        }
    }
    bundle
}

/// The shipped language a code resolves to; unknown codes give English.
fn resolve(code: &str) -> &'static str {
    LANGUAGES
        .iter()
        .find(|l| l.code == code)
        .map_or(SOURCE_LANGUAGE, |l| l.code)
}

impl Localizer {
    fn new(code: &str) -> Self {
        let language = resolve(code);
        Self {
            language,
            primary: bundle_for(language),
            fallback: bundle_for(SOURCE_LANGUAGE),
        }
    }

    fn format(&self, key: &str, args: Option<&FluentArgs<'_>>) -> Option<String> {
        [&self.primary, &self.fallback].into_iter().find_map(|bundle| {
            let message = bundle.get_message(key)?;
            let pattern = message.value()?;
            let mut errors = Vec::new();
            let text = bundle.format_pattern(pattern, args, &mut errors);
            if !errors.is_empty() {
                tracing::debug!("i18n key {key}: {} formatting error(s)", errors.len());
            }
            Some(text.into_owned())
        })
    }
}

thread_local! {
    static LOCALIZER: RefCell<Localizer> = RefCell::new(Localizer::new(SOURCE_LANGUAGE));
}

/// Switches the active language on this thread. Unknown codes select English.
pub fn set_language(code: &str) {
    let code = resolve(code);
    LOCALIZER.with(|l| {
        if l.borrow().language != code {
            *l.borrow_mut() = Localizer::new(code);
        }
    });
}

/// The code of the active language.
#[must_use]
pub fn language() -> &'static str {
    LOCALIZER.with(|l| l.borrow().language)
}

/// Formats a decimal number with the active language's decimal separator (`3.2` in English,
/// `3,2` in Italian and German). Use it for measured values inside translated sentences.
#[must_use]
pub fn number(value: f64, decimals: usize) -> String {
    let text = format!("{value:.decimals$}");
    match language() {
        "en" => text,
        _ => text.replace('.', ","),
    }
}

/// Looks up `key` in the active language, falling back to English, then to the key itself.
#[must_use]
pub fn tr(key: &str) -> String {
    tr_args(key, &[])
}

/// [`tr`] with Fluent variables, such as `{ $count }` in the message.
#[must_use]
pub fn tr_args(key: &str, args: &[(&str, FluentValue<'_>)]) -> String {
    let mut fluent_args = FluentArgs::new();
    for (name, value) in args {
        fluent_args.set(*name, value.clone());
    }
    let fluent_args = (!args.is_empty()).then_some(&fluent_args);
    LOCALIZER.with(|l| {
        l.borrow().format(key, fluent_args).unwrap_or_else(|| {
            tracing::debug!("missing i18n key {key}");
            key.to_string()
        })
    })
}

/// Translates a message key for the active language.
///
/// `t!("key")` looks the key up; `t!("key", name = value, …)` also fills Fluent variables. Values
/// may be anything convertible to [`FluentValue`](fluent::FluentValue): strings, integers, floats.
#[macro_export]
macro_rules! t {
    ($key:literal $(,)?) => {
        $crate::i18n::tr($key)
    };
    ($key:literal, $($name:ident = $value:expr),+ $(,)?) => {
        $crate::i18n::tr_args($key, &[$((stringify!($name), $crate::i18n::Value::from($value))),+])
    };
}

/// A Fluent variable value, re-exported so the [`t!`](crate::t) macro needs no `fluent` import.
pub use fluent::FluentValue as Value;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negotiation_maps_system_locales_to_shipped_languages() {
        assert_eq!(negotiate(Some("it-IT")), "it");
        assert_eq!(negotiate(Some("it_IT.UTF-8")), "it");
        assert_eq!(negotiate(Some("de")), "de");
        assert_eq!(negotiate(Some("ja-JP")), "en");
        assert_eq!(negotiate(None), "en");
    }

    #[test]
    fn unknown_language_falls_back_to_english() {
        set_language("xx");
        assert_eq!(language(), "en");
        set_language("en");
    }
}
