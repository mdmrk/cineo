/// A language Cineo offers: one byte, an index into its table.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Language(u8);

struct Info {
    code: &'static str,
    aliases: &'static [&'static str],
}

const fn lang(code: &'static str, aliases: &'static [&'static str]) -> Info {
    Info { code, aliases }
}

const TABLE: &[Info] = &[
    lang("ara", &["ar"]),
    lang("bul", &["bg"]),
    lang("cat", &["ca"]),
    lang("chi", &["zho", "zh"]),
    lang("hrv", &["hr"]),
    lang("cze", &["ces", "cs"]),
    lang("dan", &["da"]),
    lang("dut", &["nld", "nl"]),
    lang("eng", &["en"]),
    lang("fin", &["fi"]),
    lang("fre", &["fra", "fr"]),
    lang("ger", &["deu", "de"]),
    lang("gre", &["ell", "el"]),
    lang("heb", &["he"]),
    lang("hin", &["hi"]),
    lang("hun", &["hu"]),
    lang("ind", &["id"]),
    lang("ita", &["it"]),
    lang("jpn", &["ja"]),
    lang("kor", &["ko"]),
    lang("nor", &["nob", "no", "nb"]),
    lang("per", &["fas", "fa"]),
    lang("pol", &["pl"]),
    lang("por", &["pt"]),
    lang("rum", &["ron", "ro"]),
    lang("rus", &["ru"]),
    lang("srp", &["sr"]),
    lang("spa", &["es"]),
    lang("swe", &["sv"]),
    lang("tha", &["th"]),
    lang("tur", &["tr"]),
    lang("ukr", &["uk"]),
    lang("vie", &["vi"]),
];

impl std::fmt::Debug for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Language").field(&self.code()).finish()
    }
}

impl Language {
    pub fn all() -> impl Iterator<Item = Self> {
        (0..TABLE.len()).filter_map(|i| u8::try_from(i).ok().map(Self))
    }

    /// The language saved as `code`, if Cineo offers it.
    pub fn from_code(code: &str) -> Option<Self> {
        Self::all().find(|l| l.code() == code)
    }

    pub fn code(self) -> &'static str {
        self.info().code
    }

    fn info(self) -> &'static Info {
        &TABLE[usize::from(self.0)]
    }

    /// Whether a track or addon language tag means this language.
    pub fn matches(&self, tag: &str) -> bool {
        let tag = tag.trim();
        let base = tag.split(['-', '_']).next().unwrap_or_default();
        self.codes()
            .any(|c| c.eq_ignore_ascii_case(tag) || c.eq_ignore_ascii_case(base))
    }

    /// Every tag this language goes by, its code first.
    pub fn codes(self) -> impl Iterator<Item = &'static str> {
        let info = self.info();
        std::iter::once(info.code).chain(info.aliases.iter().copied())
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
        let mut codes: Vec<_> = Language::all().flat_map(Language::codes).collect();
        let all = codes.len();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), all, "a tag belongs to one language");
    }
}
