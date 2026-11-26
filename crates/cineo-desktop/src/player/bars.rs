use cineo_core::app::Settings;
use cineo_player::embedded::{PlayerCommand, Status, TrackKind};
use cineo_player::plain_text;
use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, CursorIcon, Label, Layout, Pos2, Rect, Response,
    RichText, Role, Sense, Ui, UiBuilder, WidgetInfo, pos2, vec2,
};

use super::stats::toggle_stats_shown;
use super::tracks::{speed_menu, style_covers_menu, style_panel, style_rect, track_menu};
use super::{
    Controls, Held, Menu, NextUp, Playback, STYLE_GAP, is_fullscreen, seek_icons, toggle_fullscreen,
};
use crate::i18n::t;
use crate::theme;
use crate::view::{Icon, gradient, paint_cover, paint_icon};

pub(super) const BAR_HEIGHT: f32 = 96.0;
const TOP_BAR_HEIGHT: f32 = 72.0;
const NEXT_CARD_MIN: egui::Vec2 = vec2(260.0, 170.0);
const BUTTON: f32 = 36.0;
const AUTO_NEXT_S: f64 = 10.0;
const NEXT_CARD: egui::Vec2 = vec2(384.0, 216.0);
const SEEK_REACH_UP: f32 = 16.0;
const SEEK_REACH_DOWN: f32 = 2.0;

pub(super) fn top_bar(ui: &mut Ui, rect: Rect, title: &str, out: &mut Vec<PlayerCommand>) {
    let bar = Rect::from_min_size(rect.min, vec2(rect.width(), TOP_BAR_HEIGHT));
    gradient(
        ui,
        bar,
        Color32::from_black_alpha(190),
        Color32::TRANSPARENT,
        false,
    );
    let back = Rect::from_center_size(
        pos2(bar.left() + 32.0, bar.top() + 30.0),
        egui::Vec2::splat(BUTTON),
    );
    if icon_button(ui, back, Icon::ArrowLeft, &t!("Back")).clicked() {
        out.push(PlayerCommand::Stop);
    }
    let text = Rect::from_min_max(
        pos2(back.right() + 10.0, back.top()),
        pos2(bar.right() - 24.0, back.bottom()),
    );
    left_label(
        ui,
        text,
        Label::new(
            RichText::new(title)
                .font(theme::strong())
                .color(theme::TEXT_BRIGHT),
        )
        .truncate(),
    );
}

pub(super) fn bottom_bar(
    ui: &mut Ui,
    rect: Rect,
    playback: &Playback<'_>,
    controls: &mut Controls,
    out: &mut Vec<PlayerCommand>,
) {
    let (status, addon_subtitles) = (playback.status, playback.addon_subtitles);
    let now = ui.input(|i| i.time);
    let step = playback.settings.seek_step;
    let (back_icon, forward_icon) = seek_icons(step);
    let seconds = step.seconds();
    let bar = Rect::from_min_max(pos2(rect.left(), rect.bottom() - BAR_HEIGHT), rect.max);
    gradient(
        ui,
        bar,
        Color32::TRANSPARENT,
        Color32::from_black_alpha(210),
        false,
    );
    let margin = 20.0;
    let seek = Rect::from_min_max(
        pos2(bar.left() + margin, bar.top() + 34.0),
        pos2(bar.right() - margin, bar.top() + 50.0),
    );
    let controls_area = bar.with_min_y(seek.top() - SEEK_REACH_UP);
    let _ = ui.interact(controls_area, ui.scope_id().with("bar"), Sense::click());
    seek_bar(ui, seek, status, &mut controls.seek, now, out);

    let y = bar.bottom() - 26.0;
    let position = clock(controls.seek.or(status.position_s), status.duration_s);
    let full_time = format!(
        "{position} / {}",
        clock(status.duration_s, status.duration_s)
    );
    let text_width = |text: &str| {
        ui.painter()
            .layout_no_wrap(text.to_owned(), theme::body(), theme::TEXT)
            .size()
            .x
    };
    let fit = BarFit::new(
        bar.width() - 2.0 * margin,
        playback.next.is_some(),
        text_width(&full_time),
        text_width(&position),
    );

    let mut left = Vec::with_capacity(4);
    left.push(LeftButton::Pause);
    if fit.next {
        left.push(LeftButton::Next);
    }
    if fit.seek {
        left.extend([LeftButton::Back, LeftButton::Forward]);
    }
    let mut x = bar.left() + margin + BUTTON / 2.0 - 6.0;
    for (i, button) in left.iter().enumerate() {
        let rect = Rect::from_center_size(pos2(x, y), egui::Vec2::splat(BUTTON));
        x += slot_step(i + 1 == left.len());
        match button {
            LeftButton::Pause => {
                let (icon, label) = if status.paused {
                    (Icon::Play, t!("Play"))
                } else {
                    (Icon::Pause, t!("Pause"))
                };
                if icon_button(ui, rect, icon, &label).clicked() {
                    out.push(PlayerCommand::TogglePause);
                }
            }
            LeftButton::Next => {
                if icon_button(
                    ui,
                    rect,
                    Icon::Named("player-skip-forward"),
                    &t!("Next episode"),
                )
                .clicked()
                {
                    controls.next_requested = true;
                }
            }
            LeftButton::Back => {
                if icon_button(
                    ui,
                    rect,
                    Icon::Named(back_icon),
                    &t!("Seek back {seconds} seconds", seconds = seconds),
                )
                .clicked()
                {
                    out.push(controls.seek_by(-seconds, status, now));
                }
            }
            LeftButton::Forward => {
                if icon_button(
                    ui,
                    rect,
                    Icon::Named(forward_icon),
                    &t!("Seek forward {seconds} seconds", seconds = seconds),
                )
                .clicked()
                {
                    out.push(controls.seek_by(seconds, status, now));
                }
            }
        }
    }
    let time = if fit.duration { full_time } else { position };
    left_label(
        ui,
        Rect::from_min_size(pos2(x - BUTTON / 2.0, y - 10.0), vec2(160.0, 20.0)),
        Label::new(RichText::new(time).font(theme::body()).color(theme::TEXT))
            .wrap_mode(egui::TextWrapMode::Extend),
    );

    let mut right = Vec::with_capacity(5);
    right.push(RightButton::Fullscreen);
    if fit.stats {
        right.push(RightButton::Stats);
    }
    if fit.speed {
        right.push(RightButton::Speed);
    }
    right.push(RightButton::Subtitles);
    if fit.audio {
        right.push(RightButton::Audio);
    }
    let mut x = bar.right() - margin - BUTTON / 2.0 + 6.0;
    let mut subtitles = Rect::NOTHING;
    let mut audio = None;
    let mut speed = None;
    for (i, button) in right.iter().enumerate() {
        let rect = Rect::from_center_size(pos2(x, y), egui::Vec2::splat(BUTTON));
        x -= slot_step(i + 1 == right.len());
        match button {
            RightButton::Fullscreen => {
                let (icon, label) = if is_fullscreen(ui.ctx()) {
                    ("minimize", t!("Exit fullscreen"))
                } else {
                    ("maximize", t!("Fullscreen"))
                };
                if icon_button(ui, rect, Icon::Named(icon), &label).clicked() {
                    toggle_fullscreen(ui);
                }
            }
            RightButton::Stats => {
                if icon_button(ui, rect, Icon::Named("activity"), &t!("Streaming stats")).clicked()
                {
                    toggle_stats_shown(ui.ctx());
                }
            }
            RightButton::Speed => {
                speed = Some(rect);
                if icon_button(ui, rect, Icon::Named("gauge"), &t!("Playback speed")).clicked() {
                    controls.menu = toggle(controls.menu, Menu::Speed);
                }
            }
            RightButton::Subtitles => {
                subtitles = rect;
                if icon_button(ui, rect, Icon::Named("badge-cc"), &t!("Subtitles")).clicked() {
                    controls.menu = toggle(controls.menu, Menu::Track(TrackKind::Subtitle));
                }
            }
            RightButton::Audio => {
                audio = Some(rect);
                if icon_button(ui, rect, Icon::Named("language"), &t!("Audio")).clicked() {
                    controls.menu = toggle(controls.menu, Menu::Track(TrackKind::Audio));
                }
            }
        }
    }
    let mute_center = if fit.volume {
        let volume = Rect::from_min_max(
            pos2(x - VOLUME_WIDTH + BUTTON / 2.0, y - 8.0),
            pos2(x + BUTTON / 2.0, y + 8.0),
        );
        volume_slider(ui, volume, status, &mut controls.volume, now, out);
        pos2(volume.left() - BUTTON / 2.0 - 4.0, y)
    } else {
        pos2(x, y)
    };
    let mute = Rect::from_center_size(mute_center, egui::Vec2::splat(BUTTON));
    let (icon, label) = if status.muted || status.volume <= 0.0 {
        ("volume-off", t!("Unmute"))
    } else {
        ("volume", t!("Mute"))
    };
    if icon_button(ui, mute, Icon::Named(icon), &label).clicked() {
        out.push(PlayerCommand::ToggleMute);
    }
    if (controls.menu == Some(Menu::Track(TrackKind::Audio)) && audio.is_none())
        || (controls.menu == Some(Menu::Speed) && speed.is_none())
    {
        controls.menu = None;
    }

    match (controls.menu, audio, speed) {
        (Some(Menu::Speed), _, Some(speed)) => speed_menu(ui, speed, controls, out),
        (Some(Menu::Track(TrackKind::Audio)), Some(audio), _) => {
            track_menu(
                ui,
                audio,
                status,
                &[],
                TrackKind::Audio,
                controls,
                out,
                true,
            );
        }
        (Some(Menu::Track(TrackKind::Subtitle)), _, _) => {
            let covers = style_covers_menu(ui, subtitles);
            let (menu, style_hovered) = track_menu(
                ui,
                subtitles,
                status,
                addon_subtitles,
                TrackKind::Subtitle,
                controls,
                out,
                !(covers && controls.style_open),
            );
            let panel = style_rect(ui, menu, covers);
            let ctx = ui.ctx();
            let keep = ctx.pointer_hover_pos().is_some_and(|p| {
                panel.expand(STYLE_GAP).contains(p) || menu.expand(STYLE_GAP).contains(p)
            }) || ctx.dragged_id().is_some()
                || egui::Popup::is_any_open(ctx);
            controls.style_open = style_hovered || (controls.style_open && keep);
            if controls.style_open {
                style_panel(ui, panel, playback.settings, controls, out);
            }
        }
        _ => {}
    }
    if controls.menu != Some(Menu::Track(TrackKind::Subtitle)) {
        controls.style_open = false;
    }
}

pub(super) fn next_popup(
    ui: &Ui,
    rect: Rect,
    status: &Status,
    settings: &Settings,
    next: &NextUp<'_>,
    controls: &mut Controls,
) {
    let remaining = status.duration_s - status.position_s;
    if !status.loaded
        || status.duration_s <= 0.0
        || remaining <= 0.0
        || remaining > settings.next_video_notice.seconds()
    {
        controls.next_shown_at = None;
        return;
    }
    if controls.next_dismissed {
        return;
    }
    let shown_at = controls
        .next_shown_at
        .filter(|at| *at <= status.position_s)
        .unwrap_or(status.position_s);
    controls.next_shown_at = Some(shown_at);
    let left = (AUTO_NEXT_S - (status.position_s - shown_at)).max(0.0);
    let auto = settings.binge_watching;
    if auto && left <= 0.0 {
        controls.next_requested = true;
    }
    let size = NEXT_CARD.min(vec2(
        rect.width() - 16.0,
        rect.height() - TOP_BAR_HEIGHT - BAR_HEIGHT - 16.0,
    ));
    if size.x < NEXT_CARD_MIN.x || size.y < NEXT_CARD_MIN.y {
        return;
    }
    let card = Rect::from_min_size(
        pos2(
            (rect.right() - size.x - 24.0).max(rect.left() + 8.0),
            rect.bottom() - BAR_HEIGHT - size.y - 16.0,
        ),
        size,
    );
    egui::Area::new(ui.scope_id().with("next-video"))
        .order(egui::Order::Foreground)
        .fixed_pos(card.min)
        .show(ui.ctx(), |ui| {
            let radius = CornerRadius::same(theme::RADIUS);
            ui.painter().rect_filled(card, radius, theme::PANEL);
            if let Some(image) = next.image {
                paint_cover(ui, image, card, radius, Color32::WHITE, 0.5, true);
            }
            ui.painter()
                .rect_filled(card, radius, Color32::from_black_alpha(150));
            let inner = card.shrink(radius.nw.into());
            gradient(
                ui,
                Rect::from_min_max(pos2(card.left(), inner.center().y), inner.right_bottom()),
                Color32::TRANSPARENT,
                Color32::from_black_alpha(90),
                false,
            );

            let c = card.center();
            let caption = if auto {
                t!("Next episode in {seconds} s", seconds = left.ceil())
            } else {
                t!("Next episode")
            };
            ui.painter().text(
                pos2(c.x, c.y - 66.0),
                Align2::CENTER_CENTER,
                caption,
                theme::caption(),
                theme::TEXT,
            );
            let mut job = egui::text::LayoutJob::simple(
                plain_text(next.title),
                theme::strong(),
                theme::TEXT_BRIGHT,
                (card.width() - 48.0).max(1.0),
            );
            job.halign = Align::Center;
            job.wrap.max_rows = 1;
            let galley = ui.painter().layout_job(job);
            ui.painter()
                .galley(pos2(c.x, c.y - 52.0), galley, theme::TEXT_BRIGHT);

            let play = Rect::from_center_size(pos2(c.x, c.y + 18.0), egui::Vec2::splat(56.0));
            let label = t!("Play");
            let response = ui
                .interact(play, ui.scope_id().with("next-play"), Sense::click())
                .on_hover_cursor(CursorIcon::PointingHand);
            response.widget_info(|| WidgetInfo::labeled(Role::Button, true, &label));
            let grow = ui
                .ctx()
                .animate_bool_with_time(response.id, response.hovered(), 0.12);
            ui.painter().circle_filled(
                play.center(),
                play.width() / 2.0 + 3.0 * grow,
                theme::ACCENT,
            );
            paint_icon(
                ui.painter(),
                Icon::Play,
                play.center() + vec2(2.0, 0.0),
                26.0,
                theme::TEXT_BRIGHT,
            );
            if response.clicked() {
                controls.next_requested = true;
            }

            let close = Rect::from_center_size(
                pos2(card.right() - 22.0, card.top() + 22.0),
                egui::Vec2::splat(32.0),
            );
            if icon_button(ui, close, Icon::Named("x"), &t!("Dismiss")).clicked() {
                controls.next_dismissed = true;
            }

            if auto {
                let track = Rect::from_min_max(
                    pos2(card.left() + 16.0, card.bottom() - 14.0),
                    pos2(card.right() - 16.0, card.bottom() - 10.0),
                );
                let bar = CornerRadius::same(2);
                ui.painter()
                    .rect_filled(track, bar, Color32::from_white_alpha(50));
                #[expect(clippy::cast_possible_truncation, reason = "a fraction in 0..=1")]
                let fraction = (1.0 - left / AUTO_NEXT_S) as f32;
                ui.painter().rect_filled(
                    Rect::from_min_size(track.min, vec2(track.width() * fraction, track.height())),
                    bar,
                    theme::ACCENT,
                );
            }
        });
}

const VOLUME_WIDTH: f32 = 80.0;
const BUTTON_GAP: f32 = 4.0;
const GROUP_GAP: f32 = 12.0;
const BARS_GAP: f32 = 16.0;

enum LeftButton {
    Pause,
    Next,
    Back,
    Forward,
}

enum RightButton {
    Fullscreen,
    Stats,
    Speed,
    Subtitles,
    Audio,
}

fn slot_step(last: bool) -> f32 {
    BUTTON + if last { GROUP_GAP } else { BUTTON_GAP }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BarFit {
    next: bool,
    seek: bool,
    duration: bool,
    audio: bool,
    stats: bool,
    speed: bool,
    volume: bool,
}

impl BarFit {
    fn new(width: f32, has_next: bool, full_time: f32, position: f32) -> Self {
        let mut fit = Self {
            next: has_next,
            seek: true,
            duration: true,
            audio: true,
            stats: true,
            speed: true,
            volume: true,
        };
        let drops: [fn(&mut Self); 7] = [
            |fit| fit.volume = false,
            |fit| fit.stats = false,
            |fit| fit.speed = false,
            |fit| fit.duration = false,
            |fit| fit.seek = false,
            |fit| fit.audio = false,
            |fit| fit.next = false,
        ];
        for drop in drops {
            if fit.width(full_time, position) <= width {
                break;
            }
            drop(&mut fit);
        }
        fit
    }

    fn width(self, full_time: f32, position: f32) -> f32 {
        let steps = |count: usize| {
            #[expect(clippy::cast_precision_loss, reason = "a handful of buttons")]
            let count = count as f32;
            count * (BUTTON + BUTTON_GAP) + GROUP_GAP - BUTTON_GAP
        };
        let left_buttons = 1 + usize::from(self.next) + 2 * usize::from(self.seek);
        let time = if self.duration { full_time } else { position };
        let left = steps(left_buttons) - 6.0 + time;
        let right_buttons =
            2 + usize::from(self.stats) + usize::from(self.speed) + usize::from(self.audio);
        let mute = if self.volume {
            VOLUME_WIDTH + BUTTON + BUTTON_GAP - 6.0
        } else {
            BUTTON / 2.0 + BUTTON / 2.0 - 6.0
        };
        left + BARS_GAP + steps(right_buttons) + mute
    }
}

fn left_label(ui: &mut Ui, rect: Rect, label: Label) {
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::left_to_right(Align::Center)),
        |ui| ui.add(label.selectable(false)),
    );
}

fn toggle(open: Option<Menu>, menu: Menu) -> Option<Menu> {
    (open != Some(menu)).then_some(menu)
}

fn seek_bar(
    ui: &Ui,
    rect: Rect,
    status: &Status,
    seek: &mut Held,
    now: f64,
    out: &mut Vec<PlayerCommand>,
) {
    let hit = Rect::from_min_max(
        pos2(rect.left(), rect.top() - SEEK_REACH_UP),
        pos2(rect.right(), rect.bottom() + SEEK_REACH_DOWN),
    );
    let response = ui.interact(hit, ui.scope_id().with("seek"), Sense::click_and_drag());
    let duration = status.duration_s;
    let seekable = duration > 0.0;
    let seconds_at =
        |pos: Pos2| f64::from(((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0)) * duration;
    if seekable && let Some(pos) = response.interact_pointer_pos() {
        let target = seconds_at(pos);
        if response.drag_stopped() || response.clicked() {
            seek.commit(target, now);
            out.push(PlayerCommand::SeekTo(target));
        } else if response.is_pointer_button_down_on() {
            seek.drag = Some(target);
        }
    }
    let position = seek.or(status.position_s);
    response.widget_info(|| WidgetInfo::slider(true, position, t!("Position")));
    let active = response.hovered() || seek.drag.is_some();
    let thickness = if active { 6.0 } else { 4.0 };
    let track = Rect::from_center_size(rect.center(), vec2(rect.width(), thickness));
    let radius = CornerRadius::same(3);
    let painter = ui.painter();
    painter.rect_filled(track, radius, Color32::from_white_alpha(60));
    for (start, end) in status.cached_spans() {
        let span = Rect::from_min_max(
            pos2(track.left() + track.width() * start, track.bottom() + 2.0),
            pos2(track.left() + track.width() * end, track.bottom() + 4.0),
        );
        painter.rect_filled(span, CornerRadius::same(1), Color32::from_white_alpha(150));
    }
    let played = if seekable {
        (position / duration).clamp(0.0, 1.0)
    } else {
        0.0
    };
    #[expect(clippy::cast_possible_truncation, reason = "a fraction in 0..=1")]
    let played = played as f32;
    let filled = Rect::from_min_max(
        track.min,
        pos2(track.left() + track.width() * played, track.bottom()),
    );
    painter.rect_filled(filled, radius, theme::ACCENT);
    if active && seekable {
        painter.circle_filled(pos2(filled.right(), track.center().y), 7.0, theme::ACCENT);
        if let Some(pos) = response.hover_pos().or(response.interact_pointer_pos()) {
            let x = pos.x.clamp(rect.left(), rect.right());
            painter.text(
                pos2(x, hit.top() - 4.0),
                Align2::CENTER_BOTTOM,
                clock(seconds_at(pos), duration),
                theme::caption(),
                theme::TEXT_BRIGHT,
            );
        }
        response.on_hover_cursor(CursorIcon::PointingHand);
    }
}

fn volume_slider(
    ui: &Ui,
    rect: Rect,
    status: &Status,
    volume: &mut Held,
    now: f64,
    out: &mut Vec<PlayerCommand>,
) {
    let response = ui.interact(rect, ui.scope_id().with("volume"), Sense::click_and_drag());
    if let Some(pos) = response.interact_pointer_pos()
        && (response.dragged() || response.clicked())
    {
        let percent =
            (f64::from(((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0)) * 100.0).round();
        if volume.value() != Some(percent) {
            out.push(PlayerCommand::SetVolume(percent));
        }
        volume.drag = Some(percent);
    }
    if !response.dragged()
        && let Some(percent) = volume.drag
    {
        volume.commit(percent, now);
    }
    let shown = volume.or(status.volume);
    response.widget_info(|| WidgetInfo::slider(true, shown, t!("Volume")));
    let level = if status.muted { 0.0 } else { shown / 100.0 };
    #[expect(clippy::cast_possible_truncation, reason = "a fraction in 0..=1")]
    let level = level.clamp(0.0, 1.0) as f32;
    let track = Rect::from_center_size(rect.center(), vec2(rect.width(), 4.0));
    let radius = CornerRadius::same(2);
    let painter = ui.painter();
    painter.rect_filled(track, radius, Color32::from_white_alpha(60));
    let filled = Rect::from_min_max(
        track.min,
        pos2(track.left() + track.width() * level, track.bottom()),
    );
    painter.rect_filled(filled, radius, theme::TEXT_BRIGHT);
    if response.hovered() {
        painter.circle_filled(
            pos2(filled.right(), track.center().y),
            6.0,
            theme::TEXT_BRIGHT,
        );
    }
}

fn icon_button(ui: &Ui, rect: Rect, icon: Icon, label: &str) -> Response {
    let response = ui
        .interact(rect, ui.scope_id().with(label), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, label));
    if response.hovered() {
        ui.painter().circle_filled(
            rect.center(),
            rect.width() / 2.0,
            Color32::from_white_alpha(30),
        );
    }
    paint_icon(ui.painter(), icon, rect.center(), 22.0, theme::TEXT_BRIGHT);
    response
}

pub(super) fn clock(seconds: f64, scale: f64) -> String {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to a non-negative, finite number of seconds"
    )]
    let total = if seconds.is_finite() {
        seconds.max(0.0) as u64
    } else {
        0
    };
    let (h, m, s) = (total / 3600, total / 60 % 60, total % 60);
    if scale >= 3600.0 || h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_shows_hours_only_for_long_media() {
        assert_eq!(clock(0.0, 0.0), "0:00");
        assert_eq!(clock(75.9, 600.0), "1:15");
        assert_eq!(clock(75.0, 7200.0), "0:01:15");
        assert_eq!(clock(3725.0, 7200.0), "1:02:05");
        assert_eq!(clock(f64::NAN, 10.0), "0:00");
        assert_eq!(clock(-5.0, 10.0), "0:00");
    }

    #[test]
    fn the_bottom_bar_drops_controls_in_order_until_it_fits() {
        let (full, short) = (90.0, 40.0);
        let wide = BarFit::new(2000.0, true, full, short);
        assert!(wide.next && wide.seek && wide.duration && wide.audio && wide.stats);
        assert!(wide.speed && wide.volume);

        let order = |fit: BarFit| {
            [
                fit.volume,
                fit.stats,
                fit.speed,
                fit.duration,
                fit.seek,
                fit.audio,
                fit.next,
            ]
        };
        let mut dropped_before = 0;
        for width in (200..=1000).rev().step_by(10) {
            #[expect(clippy::cast_precision_loss, reason = "small widths")]
            let width = width as f32;
            let fit = BarFit::new(width, true, full, short);
            let kept = order(fit);
            let dropped = kept.iter().take_while(|kept| !**kept).count();
            assert!(kept[dropped..].iter().all(|kept| *kept), "{fit:?}");
            assert!(dropped >= dropped_before, "{fit:?}");
            dropped_before = dropped;
            if dropped < kept.len() {
                assert!(fit.width(full, short) <= width, "{width} {fit:?}");
            }
        }

        let small = BarFit::new(360.0, true, full, short);
        assert!(!small.volume && !small.stats && !small.speed && !small.duration && !small.seek);
        assert!(small.audio && small.next);
        assert!(!BarFit::new(2000.0, false, full, short).next);
    }
}
