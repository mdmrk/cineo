//! Subtitle languages the user can prefer, and matching them against the
//! language tags addons and media files use (ISO 639-2 B/T, ISO 639-1,
//! optionally with a region such as `pt-BR`).

/// A language from [`LANGUAGES`]. `code` is ISO 639-2 and is what gets
/// saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Language {
    pub code: &'static str,
    pub name: &'static str,
    aliases: &'static [&'static str],
}

const fn lang(
    code: &'static str,
    name: &'static str,
    aliases: &'static [&'static str],
) -> Language {
    Language {
        code,
        name,
        aliases,
    }
}

/// The languages offered in Settings, by English name.
pub const LANGUAGES: &[Language] = &[
    lang("ara", "Arabic", &["ar"]),
    lang("bul", "Bulgarian", &["bg"]),
    lang("cat", "Catalan", &["ca"]),
    lang("chi", "Chinese", &["zho", "zh"]),
    lang("hrv", "Croatian", &["hr"]),
    lang("cze", "Czech", &["ces", "cs"]),
    lang("dan", "Danish", &["da"]),
    lang("dut", "Dutch", &["nld", "nl"]),
    lang("eng", "English", &["en"]),
    lang("fin", "Finnish", &["fi"]),
    lang("fre", "French", &["fra", "fr"]),
    lang("ger", "German", &["deu", "de"]),
    lang("gre", "Greek", &["ell", "el"]),
    lang("heb", "Hebrew", &["he"]),
    lang("hin", "Hindi", &["hi"]),
    lang("hun", "Hungarian", &["hu"]),
    lang("ind", "Indonesian", &["id"]),
    lang("ita", "Italian", &["it"]),
    lang("jpn", "Japanese", &["ja"]),
    lang("kor", "Korean", &["ko"]),
    lang("nor", "Norwegian", &["nob", "no", "nb"]),
    lang("per", "Persian", &["fas", "fa"]),
    lang("pol", "Polish", &["pl"]),
    lang("por", "Portuguese", &["pt"]),
    lang("rum", "Romanian", &["ron", "ro"]),
    lang("rus", "Russian", &["ru"]),
    lang("srp", "Serbian", &["sr"]),
    lang("spa", "Spanish", &["es"]),
    lang("swe", "Swedish", &["sv"]),
    lang("tha", "Thai", &["th"]),
    lang("tur", "Turkish", &["tr"]),
    lang("ukr", "Ukrainian", &["uk"]),
    lang("vie", "Vietnamese", &["vi"]),
];

impl Language {
    /// The language saved as `code`, if Cineo offers it.
    pub fn from_code(code: &str) -> Option<Self> {
        LANGUAGES.iter().copied().find(|l| l.code == code)
    }

    /// Whether a track or addon language tag means this language.
    pub fn matches(&self, tag: &str) -> bool {
        let tag = tag.trim().to_ascii_lowercase();
        let base = tag.split(['-', '_']).next().unwrap_or_default();
        self.codes().any(|c| c == tag || c == base)
    }

    /// Every tag this language goes by, its code first.
    pub fn codes(&self) -> impl Iterator<Item = &'static str> {
        std::iter::once(self.code).chain(self.aliases.iter().copied())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_match_in_every_common_form() {
        let french = Language::from_code("fre").unwrap_or_else(|| panic!("listed"));
        for tag in ["fre", "fra", "fr", "FR", "fr-CA", " fre "] {
            assert!(french.matches(tag), "{tag}");
        }
        for tag in ["", "eng", "fry", "f"] {
            assert!(!french.matches(tag), "{tag}");
        }
        assert!(Language::from_code("por").is_some_and(|p| p.matches("pt-BR")));
    }

    #[test]
    fn only_listed_codes_are_languages() {
        assert_eq!(Language::from_code("xx"), None);
        assert_eq!(Language::from_code("fr"), None, "saved as ISO 639-2");
        let mut codes: Vec<_> = LANGUAGES.iter().flat_map(Language::codes).collect();
        let all = codes.len();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), all, "a tag belongs to one language");
    }
}
