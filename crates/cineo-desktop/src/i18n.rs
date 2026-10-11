//! Message catalogs (ADR-0021).

use std::cell::Cell;

use cineo_core::app::UiLanguage;

/// A language of Cineo's own text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Locale {
    #[default]
    English,
    Spanish,
    Russian,
    Italian,
}

impl Locale {
    pub const ALL: [Self; 4] = [Self::English, Self::Spanish, Self::Russian, Self::Italian];

    pub fn from_tag(tag: &str) -> Self {
        let language = tag.split(['-', '_', '.', '@']).next().unwrap_or_default();
        if language.eq_ignore_ascii_case("es") {
            Self::Spanish
        } else if language.eq_ignore_ascii_case("ru") {
            Self::Russian
        } else if language.eq_ignore_ascii_case("it") {
            Self::Italian
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
            UiLanguage::Russian => Self::Russian,
            UiLanguage::Italian => Self::Italian,
        }
    }

    pub fn endonym(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Spanish => "Español",
            Self::Russian => "Русский",
            Self::Italian => "Italiano",
        }
    }

    #[cfg(test)]
    pub(crate) fn source(self) -> &'static str {
        match self {
            Self::English => include_str!("../assets/locales/en.toml"),
            Self::Spanish => include_str!("../assets/locales/es.toml"),
            Self::Russian => include_str!("../assets/locales/ru.toml"),
            Self::Italian => include_str!("../assets/locales/it.toml"),
        }
    }

    pub(crate) fn tag(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Spanish => "es",
            Self::Russian => "ru",
            Self::Italian => "it",
        }
    }
}

thread_local! {
    static CURRENT: Cell<Locale> = const { Cell::new(Locale::English) };
}

pub fn set(locale: Locale) {
    CURRENT.set(locale);
}

pub(crate) fn tag() -> &'static str {
    CURRENT.get().tag()
}

macro_rules! t {
    ($id:expr $(, $name:ident = $value:expr)* $(,)?) => {
        match $id {
            id => {
                let id: &str = id;
                rust_i18n::t!(id, locale = $crate::i18n::tag() $(, $name = $value)*).into_owned()
            }
        }
    };
}
pub(crate) use t;

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use cineo_core::app::Language;

    use super::*;

    fn messages(locale: Locale) -> BTreeMap<String, BTreeSet<String>> {
        let mut messages: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for line in locale.source().lines() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((id, value)) = line.split_once(" = ") else {
                panic!("{locale:?}: unexpected line {line:?}");
            };
            let arguments = messages.entry(id.to_owned()).or_default();
            for (_, rest) in value
                .match_indices("%{")
                .map(|(i, _)| value.split_at(i + 2))
            {
                let name = rest.split('}').next().unwrap_or_default();
                arguments.insert(name.to_owned());
            }
        }
        messages
    }

    fn ids(locale: Locale) -> BTreeSet<String> {
        messages(locale).into_keys().collect()
    }

    #[test]
    fn every_locale_has_the_same_messages_and_arguments() {
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
    fn messages_follow_the_locale_and_fall_back_to_the_id() {
        set(Locale::Spanish);
        assert_eq!(t!("page-library"), "Biblioteca");
        assert_eq!(t!("count-titles", count = 3), "Títulos: 3");
        assert_eq!(t!("no-such-message"), "no-such-message");
        set(Locale::Russian);
        assert_eq!(t!("count-titles", count = 21), "Названий: 21");
        assert_eq!(t!(&format!("language-{}", "ara")), "Арабский");
        set(Locale::Italian);
        assert_eq!(t!("watched-percent", percent = 40), "40 % riprodotto");
        set(Locale::English);
        assert_eq!(
            t!("installed-addon", name = "Torrentio"),
            "Installed Torrentio"
        );
        assert_eq!(t!("seconds", count = 2.5), "2.5 s");
    }

    #[test]
    fn the_system_tag_picks_a_locale() {
        assert_eq!(Locale::from_tag("es-MX"), Locale::Spanish);
        assert_eq!(Locale::from_tag("es_ES.UTF-8"), Locale::Spanish);
        assert_eq!(Locale::from_tag("ru_RU.UTF-8"), Locale::Russian);
        assert_eq!(Locale::from_tag("it-IT"), Locale::Italian);
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
