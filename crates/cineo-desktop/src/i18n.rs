//! Fluent message catalogs (ADR-0017).

use std::cell::Cell;

use cineo_core::app::UiLanguage;
use fluent_bundle::{FluentArgs, FluentBundle, FluentResource};
use unic_langid::{LanguageIdentifier, langid};

/// A language of Cineo's own text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Locale {
    #[default]
    English,
    Spanish,
}

impl Locale {
    pub const ALL: [Self; 2] = [Self::English, Self::Spanish];

    pub fn from_tag(tag: &str) -> Self {
        let language = tag.split(['-', '_', '.', '@']).next().unwrap_or_default();
        if language.eq_ignore_ascii_case("es") {
            Self::Spanish
        } else {
            Self::English
        }
    }

    pub fn system() -> Self {
        sys_locale::get_locale().map_or(Self::English, |tag| Self::from_tag(&tag))
    }

    pub fn resolve(setting: UiLanguage, system: Self) -> Self {
        match setting {
            UiLanguage::System => system,
            UiLanguage::English => Self::English,
            UiLanguage::Spanish => Self::Spanish,
        }
    }

    pub fn endonym(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Spanish => "Español",
        }
    }

    fn id(self) -> LanguageIdentifier {
        match self {
            Self::English => langid!("en"),
            Self::Spanish => langid!("es"),
        }
    }

    fn source(self) -> &'static str {
        match self {
            Self::English => include_str!("../assets/locales/en.ftl"),
            Self::Spanish => include_str!("../assets/locales/es.ftl"),
        }
    }
}

type Bundle = FluentBundle<FluentResource>;

thread_local! {
    static CURRENT: Cell<Locale> = const { Cell::new(Locale::English) };
    static BUNDLES: [Bundle; 2] = Locale::ALL.map(bundle);
}

fn bundle(locale: Locale) -> Bundle {
    let mut bundle = FluentBundle::new(vec![locale.id()]);
    bundle.set_use_isolating(false);
    let resource =
        FluentResource::try_new(locale.source().to_owned()).unwrap_or_else(|(partial, _)| partial);
    if let Err(errors) = bundle.add_resource(resource) {
        tracing::warn!(?locale, ?errors, "duplicate messages");
    }
    bundle
}

pub fn set(locale: Locale) {
    CURRENT.set(locale);
}

pub fn text(id: &str, args: Option<&FluentArgs<'_>>) -> String {
    BUNDLES.with(|bundles| {
        format(&bundles[CURRENT.get() as usize], id, args)
            .or_else(|| format(&bundles[0], id, args))
            .unwrap_or_else(|| id.to_owned())
    })
}

fn format(bundle: &Bundle, id: &str, args: Option<&FluentArgs<'_>>) -> Option<String> {
    let pattern = bundle.get_message(id)?.value()?;
    let mut errors = Vec::new();
    let text = bundle.format_pattern(pattern, args, &mut errors);
    if !errors.is_empty() {
        tracing::debug!(id, ?errors, "message formatting failed");
    }
    Some(text.into_owned())
}

macro_rules! t {
    ($id:expr) => {
        $crate::i18n::text($id, None)
    };
    ($id:expr, $($name:ident = $value:expr),+ $(,)?) => {{
        let mut args = fluent_bundle::FluentArgs::new();
        $(args.set(stringify!($name), $value);)+
        $crate::i18n::text($id, Some(&args))
    }};
}
pub(crate) use t;

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use cineo_core::app::Language;

    use super::*;

    fn messages(locale: Locale) -> BTreeMap<String, BTreeSet<String>> {
        if let Err((_, errors)) = FluentResource::try_new(locale.source().to_owned()) {
            panic!("{locale:?} does not parse: {errors:?}");
        }
        let mut messages: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut current = None;
        for line in locale.source().lines() {
            if line.starts_with(|c: char| c.is_ascii_lowercase())
                && let Some((id, _)) = line.split_once(" =")
            {
                current = Some(id.to_owned());
                messages.entry(id.to_owned()).or_default();
            } else if !line.starts_with(' ') {
                current = None;
            }
            if let Some(id) = &current {
                for (_, rest) in line.match_indices('$').map(|(i, _)| line.split_at(i + 1)) {
                    let name: String = rest
                        .chars()
                        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                        .collect();
                    messages.entry(id.clone()).or_default().insert(name);
                }
            }
        }
        messages
    }

    fn ids(locale: Locale) -> BTreeSet<String> {
        messages(locale).into_keys().collect()
    }

    #[test]
    fn every_locale_parses_and_has_the_same_messages_and_arguments() {
        let english = messages(Locale::English);
        assert!(english.len() > 100, "found only {}", english.len());
        for locale in Locale::ALL {
            assert_eq!(messages(locale), english, "{locale:?}");
        }
    }

    #[test]
    fn every_message_the_code_asks_for_exists() {
        let english = ids(Locale::English);
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
        let mut used = BTreeSet::new();
        for file in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{e}")) {
            let path = file.unwrap_or_else(|e| panic!("{e}")).path();
            if path.ends_with("i18n.rs") {
                continue;
            }
            let code = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{e}"));
            for (i, _) in code.match_indices("t!(\"") {
                if code[..i].ends_with(|c: char| c.is_alphanumeric() || c == '_') {
                    continue;
                }
                let id = code[i + 4..].split('"').next().unwrap_or_default();
                used.insert(id.to_owned());
            }
        }
        assert!(used.len() > 100, "found only {} messages", used.len());
        let missing: Vec<_> = used.difference(&english).collect();
        assert!(missing.is_empty(), "{missing:?}");
    }

    #[test]
    fn every_audio_and_subtitle_language_has_a_name() {
        let english = ids(Locale::English);
        for language in Language::all() {
            assert!(
                english.contains(&format!("language-{}", language.code())),
                "{language:?}"
            );
        }
    }

    #[test]
    fn messages_follow_the_locale_with_plurals_and_fall_back_to_english() {
        set(Locale::Spanish);
        assert_eq!(t!("page-library"), "Biblioteca");
        assert_eq!(t!("count-titles", count = 1), "1 título");
        assert_eq!(t!("count-titles", count = 3), "3 títulos");
        assert_eq!(t!("no-such-message"), "no-such-message");
        set(Locale::English);
        assert_eq!(t!("count-titles", count = 1), "1 title");
        assert_eq!(
            t!("installed-addon", name = "Torrentio"),
            "Installed Torrentio",
            "no isolation marks around arguments"
        );
    }

    #[test]
    fn the_system_tag_picks_a_locale() {
        assert_eq!(Locale::from_tag("es-MX"), Locale::Spanish);
        assert_eq!(Locale::from_tag("es_ES.UTF-8"), Locale::Spanish);
        assert_eq!(Locale::from_tag("en-US"), Locale::English);
        assert_eq!(Locale::from_tag("fr-FR"), Locale::English);
        assert_eq!(Locale::from_tag("C"), Locale::English);
        assert_eq!(
            Locale::resolve(UiLanguage::System, Locale::Spanish),
            Locale::Spanish
        );
        assert_eq!(
            Locale::resolve(UiLanguage::English, Locale::Spanish),
            Locale::English
        );
    }
}
