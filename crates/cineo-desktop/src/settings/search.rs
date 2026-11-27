use eframe::egui::{Id, Ui};

use super::sort_key;
use crate::theme;
use crate::view::section;

#[derive(Debug, Clone, Default)]
struct Filter {
    words: Vec<String>,
    heading: Option<String>,
    shown: bool,
}

fn id() -> Id {
    Id::unique("settings-search")
}

pub(super) fn words(query: &str) -> Vec<String> {
    sort_key(query)
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

pub(super) fn matches(words: &[String], texts: &[&str]) -> bool {
    let haystack = sort_key(&texts.join(" "));
    words.iter().all(|word| haystack.contains(word.as_str()))
}

pub(super) fn begin(ui: &Ui, words: Vec<String>, heading: &str) {
    let filter = Filter {
        words,
        heading: Some(heading.to_owned()),
        shown: false,
    };
    ui.data_mut(|d| d.insert_temp(id(), filter));
}

pub(super) fn end(ui: &Ui) -> bool {
    ui.data_mut(|d| d.remove_temp::<Filter>(id()))
        .is_some_and(|filter| filter.shown)
}

pub(super) fn filtering(ui: &Ui) -> bool {
    ui.data(|d| d.get_temp::<Filter>(id())).is_some()
}

pub(super) fn shows(ui: &mut Ui, title: &str, help: &str) -> bool {
    let Some(mut filter) = ui.data(|d| d.get_temp::<Filter>(id())) else {
        return true;
    };
    if !matches(&filter.words, &[title, help]) {
        return false;
    }
    if let Some(heading) = filter.heading.take() {
        section(ui, &heading, |_| {});
        ui.add_space(theme::GAP / 2.0);
    }
    filter.shown = true;
    ui.data_mut(|d| d.insert_temp(id(), filter));
    true
}

#[cfg(test)]
mod tests {
    use super::{matches, words};

    #[test]
    fn every_word_must_appear_ignoring_case_and_accents() {
        let query = words("  SUBTÍTULO  idioma ");
        assert!(matches(&query, &["Idioma de los subtítulos", ""]));
        assert!(matches(&query, &["Idioma", "Activa un subtitulo"]));
        assert!(!matches(&query, &["Idioma del audio", ""]));
        assert!(matches(&words(""), &["anything"]));
    }
}
