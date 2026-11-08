use super::StreamFacts;
use super::cards::cover_uv;
use super::icons::{EMOJI_ICONS, Icon, addon_text, is_emoji, regional_letter};
use crate::theme;
use cineo_core::addon::Stream;
use cineo_core::app::StreamSort;
use eframe::egui::{self, Rect, Vec2};

#[test]
fn cover_crops_a_wide_image_at_the_sides() {
    let uv = cover_uv(Vec2::new(200.0, 100.0), Vec2::new(100.0, 100.0), 0.5);
    assert_eq!(
        uv,
        Rect::from_min_max(egui::pos2(0.25, 0.0), egui::pos2(0.75, 1.0))
    );
}

#[test]
fn cover_crops_a_tall_image_around_the_focus() {
    let uv = cover_uv(Vec2::new(100.0, 200.0), Vec2::new(100.0, 100.0), 0.0);
    assert_eq!(
        uv,
        Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 0.5))
    );
    let uv = cover_uv(Vec2::new(100.0, 200.0), Vec2::new(100.0, 100.0), 1.0);
    assert_eq!(
        uv,
        Rect::from_min_max(egui::pos2(0.0, 0.5), egui::pos2(1.0, 1.0))
    );
}

#[test]
fn cover_shows_everything_for_an_unknown_size() {
    let uv = cover_uv(Vec2::ZERO, Vec2::new(100.0, 100.0), 0.5);
    assert_eq!(
        uv,
        Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0))
    );
}

#[test]
fn every_icon_exists_in_the_tabler_set() {
    for icon in Icon::ALL {
        assert!(icon.glyph().is_some(), "{icon:?} ({})", icon.name());
    }
}

#[test]
fn every_emoji_stand_in_exists_in_the_tabler_set() {
    for (emoji, name) in EMOJI_ICONS {
        assert!(Icon::Named(name).glyph().is_some(), "{emoji} → {name}");
    }
}

#[test]
fn addon_emoji_become_tabler_icons_flags_and_nothing_else() {
    let job = addon_text(
        "👤 12 💾 1.2 GB ⚙️ YTS\nMulti / 🇬🇧 / 🇮🇹 😀✨",
        &theme::body(),
        theme::TEXT,
        theme::TEXT,
    );
    let text = &job.text;
    assert!(!text.chars().any(is_emoji), "{text:?}");
    assert!(
        !text.chars().any(|c| regional_letter(c).is_some()),
        "{text:?}"
    );
    for kept in ["12", "1.2 GB", "YTS", "Multi / GB / IT"] {
        assert!(text.contains(kept), "{kept} in {text:?}");
    }
    let tabler_sections = job
        .sections
        .iter()
        .filter(|s| matches!(&s.format.font_id.family, egui::FontFamily::Name(f) if f.starts_with("Tabler")))
        .count();
    assert_eq!(tabler_sections, 3, "user, floppy and gear");
}

#[test]
fn plain_addon_text_is_unchanged() {
    let text = "Example HTTP stream — 1080p → 720p ▲";
    let job = addon_text(text, &theme::body(), theme::TEXT, theme::TEXT);
    assert_eq!(job.text, text);
}

fn stream(name: Option<&str>, description: Option<&str>) -> Stream {
    let field = |key: &str, value: Option<&str>| {
        value.map_or(String::new(), |v| {
            format!(r#","{key}":"{}""#, v.replace('\n', "\\n"))
        })
    };
    let json = format!(
        r#"{{"streams":[{{"infoHash":"0123456789abcdef0123456789abcdef01234567"{}{}}}]}}"#,
        field("name", name),
        field("description", description),
    );
    cineo_core::addon::parse_stream_response(json.as_bytes())
        .unwrap()
        .value
        .remove(0)
}

#[test]
fn stream_facts_are_kept_until_the_streams_or_the_sort_change() {
    let first = [
        stream(Some("720p"), Some("👤 5")),
        stream(Some("1080p"), Some("👤 39")),
    ];
    let mut facts = StreamFacts::default();
    assert_eq!(facts.group(1, &first).sorted(StreamSort::Seeders).0, [1, 0]);
    assert_eq!(
        facts.group(1, &first).sorted(StreamSort::AddonOrder).0,
        [0, 1]
    );
    let group = facts.group(1, &first);
    assert_eq!(group.streams[0].quality.map(|q| q.label()), Some("720p"));
    group.streams[0].seeders = Some(100);
    assert_eq!(
        facts.group(1, &first).sorted(StreamSort::Seeders).0,
        [0, 1],
        "the same streams reuse their facts"
    );
    let second = [stream(Some("4k"), None)];
    let (order, fresh) = facts.group(1, &second).sorted(StreamSort::Seeders);
    assert_eq!(order, [0]);
    assert_eq!(fresh[0].quality.map(|q| q.label()), Some("4K"));
}

#[test]
fn stream_facts_notice_new_streams_in_the_same_memory() {
    let mut streams = [stream(Some("720p"), None)];
    let mut facts = StreamFacts::default();
    let quality = |facts: &mut StreamFacts, streams: &[Stream]| {
        facts.group(0, streams).streams[0]
            .quality
            .map(|q| q.label())
    };
    assert_eq!(quality(&mut facts, &streams), Some("720p"));
    streams[0] = stream(Some("4k"), None);
    assert_eq!(quality(&mut facts, &streams), Some("4K"));
}
