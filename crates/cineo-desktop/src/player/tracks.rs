use cineo_core::app::Settings;
use cineo_player::embedded::{PlayerCommand, Status, Track, TrackKind};
use cineo_player::plain_text;
use cineo_player::subtitles::AddonSubtitle;
use eframe::egui::{
    self, Align2, Color32, CornerRadius, CursorIcon, Rect, RichText, Sense, Ui, UiBuilder,
    WidgetInfo, WidgetType, pos2, vec2,
};

use url::Url;

use crate::i18n::{Text, pt, t};
use crate::settings::subtitle_controls;
use crate::theme;
use crate::view::{Icon, dim, paint_icon};

use super::{Controls, STYLE_GAP};

const MENU_MAX_HEIGHT: f32 = 320.0;
const STYLE_WIDTH: f32 = 420.0;
const STYLE_MAX_HEIGHT: f32 = 520.0;

enum Entry {
    Track(Option<i64>),
    Addon(Url),
    Empty,
}

fn paint_elided(painter: &egui::Painter, rect: Rect, text: &str, color: Color32) -> bool {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), theme::body(), color);
    job.wrap.max_width = rect.width().max(1.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let galley = painter.layout_job(job);
    let elided = galley.elided;
    let y = rect.center().y - galley.size().y / 2.0;
    painter.galley(pos2(rect.left(), y), galley, color);
    elided
}

pub(super) fn track_menu(
    ui: &Ui,
    anchor: Rect,
    status: &Status,
    addon_subtitles: &[AddonSubtitle],
    kind: TrackKind,
    controls: &mut Controls,
    out: &mut Vec<PlayerCommand>,
) -> (Rect, bool) {
    let tracks: Vec<&Track> = status.tracks.iter().filter(|t| t.kind == kind).collect();
    let mut entries: Vec<(Entry, Text, bool)> = Vec::new();
    if kind == TrackKind::Subtitle {
        entries.push((
            Entry::Track(None),
            pt!("subtitles-off", "Off"),
            !tracks.iter().any(|t| t.selected),
        ));
    }
    entries.extend(
        tracks
            .iter()
            .filter(|t| t.external_file.is_none())
            .enumerate()
            .map(|(i, t)| (Entry::Track(Some(t.id)), track_label(t, i + 1), t.selected)),
    );
    entries.extend(addon_subtitles.iter().map(|s| {
        let source = s.label.as_deref().unwrap_or(&s.addon_name);
        (
            Entry::Addon(s.url.clone()),
            plain_text(&format!("{} · {source}", s.lang)).into(),
            s.selected,
        )
    }));
    if entries.is_empty() {
        entries.push((Entry::Empty, t!("No audio tracks"), false));
    }
    let row = 30.0;
    let width = 240.0;
    #[expect(clippy::cast_precision_loss, reason = "a short list")]
    let content = entries.len() as f32 * row;
    let room = (anchor.top() - ui.max_rect().top() - 24.0).max(row);
    let footer = if kind == TrackKind::Subtitle {
        row + 4.0
    } else {
        0.0
    };
    let list = content.min(MENU_MAX_HEIGHT).min(room);
    let height = list + footer + 8.0;
    let mut style_hovered = false;
    let menu = Rect::from_min_size(
        pos2(
            (anchor.right() - width).max(ui.max_rect().left() + 8.0),
            anchor.top() - 8.0 - height,
        ),
        vec2(width, height),
    );
    egui::Area::new(ui.id().with("track-menu"))
        .order(egui::Order::Foreground)
        .fixed_pos(menu.min)
        .show(ui.ctx(), |ui| {
            ui.painter()
                .rect_filled(menu, CornerRadius::same(theme::RADIUS), theme::PANEL);
            ui.scope_builder(UiBuilder::new().max_rect(menu.shrink(4.0)), |ui| {
                egui::ScrollArea::vertical()
                    .max_height(list)
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        for (id, label, selected) in entries {
                            let (item, response) = ui.allocate_exact_size(
                                vec2(ui.available_width(), row),
                                Sense::click(),
                            );
                            let response = response.on_hover_cursor(CursorIcon::PointingHand);
                            response.widget_info(|| {
                                WidgetInfo::selected(
                                    WidgetType::RadioButton,
                                    true,
                                    selected,
                                    &label,
                                )
                            });
                            let painter = ui.painter();
                            if response.hovered() {
                                painter.rect_filled(
                                    item,
                                    CornerRadius::same(theme::RADIUS),
                                    theme::SURFACE_HOVER,
                                );
                            }
                            if selected {
                                paint_icon(
                                    painter,
                                    Icon::Named("check"),
                                    pos2(item.left() + 14.0, item.center().y),
                                    16.0,
                                    theme::ACCENT,
                                );
                            }
                            let color = if selected {
                                theme::TEXT_BRIGHT
                            } else {
                                theme::TEXT
                            };
                            let text = item
                                .with_min_x(item.left() + 30.0)
                                .with_max_x(item.right() - 8.0);
                            let response = if paint_elided(painter, text, &label, color) {
                                response.on_hover_text(&*label)
                            } else {
                                response
                            };
                            if response.clicked() {
                                match id {
                                    Entry::Track(id) if kind == TrackKind::Subtitle => {
                                        out.push(PlayerCommand::SetSubtitle(id));
                                    }
                                    Entry::Track(id @ Some(_)) => {
                                        out.push(PlayerCommand::SetAudio(id));
                                    }
                                    Entry::Addon(url) => controls.addon_request = Some(url),
                                    Entry::Track(None) | Entry::Empty => continue,
                                }
                                controls.menu = None;
                            }
                        }
                    });
            });
            if footer > 0.0 {
                style_hovered = style_row(ui, menu, row, controls.style_open);
            }
        });
    (menu, style_hovered)
}

fn style_row(ui: &Ui, menu: Rect, row: f32, open: bool) -> bool {
    let item = Rect::from_min_size(
        pos2(menu.left() + 4.0, menu.bottom() - 4.0 - row),
        vec2(menu.width() - 8.0, row),
    );
    let label = t!("Style and delay");
    let response = ui.interact(item, ui.id().with("subtitle-style-row"), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &label));
    let painter = ui.painter();
    painter.hline(
        item.x_range().shrink(6.0),
        item.top() - 2.0,
        egui::Stroke::new(1.0, theme::RULE),
    );
    if open || response.hovered() {
        painter.rect_filled(
            item,
            CornerRadius::same(theme::RADIUS),
            theme::SURFACE_HOVER,
        );
    }
    paint_icon(
        painter,
        Icon::ChevronLeft,
        pos2(item.left() + 14.0, item.center().y),
        16.0,
        theme::TEXT,
    );
    painter.text(
        pos2(item.left() + 30.0, item.center().y),
        Align2::LEFT_CENTER,
        &label,
        theme::body(),
        theme::TEXT,
    );
    response.hovered()
}

pub(super) fn style_rect(ui: &Ui, menu: Rect) -> Rect {
    let top = (menu.bottom() - STYLE_MAX_HEIGHT).max(ui.max_rect().top() + 24.0);
    let left = (menu.left() - STYLE_GAP - STYLE_WIDTH).max(ui.max_rect().left() + 8.0);
    Rect::from_min_max(
        pos2(left, top),
        pos2(menu.left() - STYLE_GAP, menu.bottom()),
    )
}

pub(super) fn style_panel(
    ui: &Ui,
    panel: Rect,
    settings: &Settings,
    controls: &mut Controls,
    out: &mut Vec<PlayerCommand>,
) {
    egui::Area::new(ui.id().with("subtitle-style"))
        .order(egui::Order::Foreground)
        .fixed_pos(panel.min)
        .show(ui.ctx(), |ui| {
            ui.painter()
                .rect_filled(panel, CornerRadius::same(theme::RADIUS), theme::PANEL);
            ui.scope_builder(UiBuilder::new().max_rect(panel.shrink(16.0)), |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        delay(ui, controls, out);
                        subtitle_controls(ui, settings, &mut controls.settings);
                    });
            });
        });
}

fn delay(ui: &mut Ui, controls: &mut Controls, out: &mut Vec<PlayerCommand>) {
    let ms = controls.subtitle_delay_ms;
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(t!("Delay"))
                .font(theme::strong())
                .color(theme::TEXT_BRIGHT),
        );
        ui.label(dim(&t!("This video only.")).small());
    });
    ui.add_space(6.0);
    let cells = [
        (
            -1000,
            "−1 s",
            t!("Subtitles {amount} earlier", amount = "1 s"),
        ),
        (
            -100,
            "−0.1 s",
            t!("Subtitles {amount} earlier", amount = "0.1 s"),
        ),
        (0, "", Text::Borrowed("")),
        (
            100,
            "+0.1 s",
            t!("Subtitles {amount} later", amount = "0.1 s"),
        ),
        (1000, "+1 s", t!("Subtitles {amount} later", amount = "1 s")),
    ];
    let gap = 4.0;
    let (strip, _) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::hover());
    #[expect(clippy::cast_precision_loss, reason = "five cells")]
    let width = (strip.width() - gap * (cells.len() - 1) as f32) / cells.len() as f32;
    let mut next = ms;
    for (i, (step, text, label)) in cells.into_iter().enumerate() {
        #[expect(clippy::cast_precision_loss, reason = "five cells")]
        let left = strip.left() + i as f32 * (width + gap);
        let rect = Rect::from_min_size(pos2(left, strip.top()), vec2(width, strip.height()));
        let middle = step == 0;
        let enabled = !middle || ms != 0;
        let response = ui.interact(rect, ui.id().with(("delay", i)), Sense::click());
        response.widget_info(|| {
            let label = if middle {
                t!("Subtitle delay {delay}, reset", delay = delay_label(ms))
            } else {
                label.clone()
            };
            WidgetInfo::labeled(WidgetType::Button, enabled, label)
        });
        let hovered = enabled && response.hovered();
        if hovered {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        let fill = if hovered {
            theme::SURFACE_HOVER
        } else {
            theme::SURFACE
        };
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(theme::RADIUS), fill);
        let (text, font, color) = if middle {
            (delay_label(ms), theme::strong(), theme::TEXT_BRIGHT)
        } else {
            (text.to_owned(), theme::body(), theme::TEXT)
        };
        painter.text(rect.center(), Align2::CENTER_CENTER, text, font, color);
        if enabled && response.clicked() {
            next = if middle { 0 } else { ms + step };
        }
    }
    ui.add_space(theme::GAP);
    if next != ms {
        controls.subtitle_delay_ms = next;
        out.push(PlayerCommand::SetSubtitleDelay(f64::from(next) / 1000.0));
    }
}

fn delay_label(ms: i32) -> String {
    format!("{:+.1} s", f64::from(ms) / 1000.0)
}

fn track_label(track: &Track, number: usize) -> Text {
    track
        .label()
        .map_or_else(|| t!("Track {number}", number = number), Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_labels_use_language_and_title() {
        let track = |lang: Option<&str>, title: Option<&str>| Track {
            id: 1,
            kind: TrackKind::Audio,
            title: title.map(str::to_owned),
            lang: lang.map(str::to_owned),
            selected: false,
            external_file: None,
            codec: None,
        };
        assert_eq!(
            track_label(&track(Some("eng"), Some("Commentary")), 1),
            "eng · Commentary"
        );
        assert_eq!(track_label(&track(None, None), 2), "Track 2");
        assert_eq!(track_label(&track(Some("a\nb"), None), 1), "a b");
    }
}
