use cineo_core::app::Settings;
use cineo_player::embedded::{PlayerCommand, Status, TrackKind};
use cineo_player::plain_text;
use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, CursorIcon, Label, Layout, Pos2, Rect, Response,
    RichText, Sense, Ui, UiBuilder, WidgetInfo, WidgetType, pos2, vec2,
};

use super::stats::toggle_stats_shown;
use super::tracks::{style_panel, style_rect, track_menu};
use super::{
    Controls, Held, NextUp, Playback, STYLE_GAP, is_fullscreen, seek_icons, toggle_fullscreen,
};
use crate::i18n::t;
use crate::theme;
use crate::view::{Icon, gradient, paint_cover, paint_icon};

const BAR_HEIGHT: f32 = 96.0;
const BUTTON: f32 = 36.0;
const AUTO_NEXT_S: f64 = 10.0;
const NEXT_CARD: egui::Vec2 = vec2(384.0, 216.0);
const SEEK_REACH_UP: f32 = 16.0;
const SEEK_REACH_DOWN: f32 = 2.0;

pub(super) fn top_bar(ui: &mut Ui, rect: Rect, title: &str, out: &mut Vec<PlayerCommand>) {
    let bar = Rect::from_min_size(rect.min, vec2(rect.width(), 72.0));
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
    let _ = ui.interact(controls_area, ui.id().with("bar"), Sense::click());
    seek_bar(ui, seek, status, &mut controls.seek, now, out);

    let y = bar.bottom() - 26.0;
    let mut x = bar.left() + margin + BUTTON / 2.0 - 6.0;
    let mut slot = |step: f32| {
        let c = pos2(x, y);
        x += step;
        Rect::from_center_size(c, egui::Vec2::splat(BUTTON))
    };
    let (icon, label) = if status.paused {
        (Icon::Play, t!("Play"))
    } else {
        (Icon::Pause, t!("Pause"))
    };
    if icon_button(ui, slot(BUTTON + 4.0), icon, &label).clicked() {
        out.push(PlayerCommand::TogglePause);
    }
    if playback.next.is_some()
        && icon_button(
            ui,
            slot(BUTTON + 4.0),
            Icon::Named("player-skip-forward"),
            &t!("Next episode"),
        )
        .clicked()
    {
        controls.next_requested = true;
    }
    if icon_button(
        ui,
        slot(BUTTON + 4.0),
        Icon::Named(back_icon),
        &t!("Seek back {seconds} seconds", seconds = seconds),
    )
    .clicked()
    {
        out.push(controls.seek_by(-seconds, status, now));
    }
    if icon_button(
        ui,
        slot(BUTTON + 12.0),
        Icon::Named(forward_icon),
        &t!("Seek forward {seconds} seconds", seconds = seconds),
    )
    .clicked()
    {
        out.push(controls.seek_by(seconds, status, now));
    }
    let time = format!(
        "{} / {}",
        clock(controls.seek.or(status.position_s), status.duration_s),
        clock(status.duration_s, status.duration_s)
    );
    left_label(
        ui,
        Rect::from_min_size(pos2(x - BUTTON / 2.0, y - 10.0), vec2(160.0, 20.0)),
        Label::new(RichText::new(time).font(theme::body()).color(theme::TEXT))
            .wrap_mode(egui::TextWrapMode::Extend),
    );

    let mut x = bar.right() - margin - BUTTON / 2.0 + 6.0;
    let mut slot = |step: f32| {
        let c = pos2(x, y);
        x -= step;
        Rect::from_center_size(c, egui::Vec2::splat(BUTTON))
    };
    let (icon, label) = if is_fullscreen(ui.ctx()) {
        ("minimize", t!("Exit fullscreen"))
    } else {
        ("maximize", t!("Fullscreen"))
    };
    if icon_button(ui, slot(BUTTON + 4.0), Icon::Named(icon), &label).clicked() {
        toggle_fullscreen(ui);
    }
    if icon_button(
        ui,
        slot(BUTTON + 4.0),
        Icon::Named("activity"),
        &t!("Streaming stats"),
    )
    .clicked()
    {
        toggle_stats_shown(ui.ctx());
    }
    let subtitles = slot(BUTTON + 4.0);
    if icon_button(ui, subtitles, Icon::Named("badge-cc"), &t!("Subtitles")).clicked() {
        controls.menu = toggle(controls.menu, TrackKind::Subtitle);
    }
    let audio = slot(BUTTON + 12.0);
    if icon_button(ui, audio, Icon::Named("language"), &t!("Audio")).clicked() {
        controls.menu = toggle(controls.menu, TrackKind::Audio);
    }
    let volume = Rect::from_min_max(
        pos2(x - 80.0 + BUTTON / 2.0, y - 8.0),
        pos2(x + BUTTON / 2.0, y + 8.0),
    );
    volume_slider(ui, volume, status, &mut controls.volume, now, out);
    let mute = Rect::from_center_size(
        pos2(volume.left() - BUTTON / 2.0 - 4.0, y),
        egui::Vec2::splat(BUTTON),
    );
    let (icon, label) = if status.muted || status.volume <= 0.0 {
        ("volume-off", t!("Unmute"))
    } else {
        ("volume", t!("Mute"))
    };
    if icon_button(ui, mute, Icon::Named(icon), &label).clicked() {
        out.push(PlayerCommand::ToggleMute);
    }

    match controls.menu {
        Some(TrackKind::Audio) => {
            track_menu(ui, audio, status, &[], TrackKind::Audio, controls, out);
        }
        Some(TrackKind::Subtitle) => {
            let (menu, style_hovered) = track_menu(
                ui,
                subtitles,
                status,
                addon_subtitles,
                TrackKind::Subtitle,
                controls,
                out,
            );
            let panel = style_rect(ui, menu);
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
    if controls.menu != Some(TrackKind::Subtitle) {
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
    let size = NEXT_CARD.min(vec2(rect.width() - 16.0, rect.height() * 0.5));
    let card = Rect::from_min_size(
        pos2(
            (rect.right() - size.x - 24.0).max(rect.left() + 8.0),
            rect.bottom() - BAR_HEIGHT - size.y - 16.0,
        ),
        size,
    );
    egui::Area::new(ui.id().with("next-video"))
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
                .interact(play, ui.id().with("next-play"), Sense::click())
                .on_hover_cursor(CursorIcon::PointingHand);
            response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &label));
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

fn left_label(ui: &mut Ui, rect: Rect, label: Label) {
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::left_to_right(Align::Center)),
        |ui| ui.add(label.selectable(false)),
    );
}

fn toggle(menu: Option<TrackKind>, kind: TrackKind) -> Option<TrackKind> {
    (menu != Some(kind)).then_some(kind)
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
    let response = ui.interact(hit, ui.id().with("seek"), Sense::click_and_drag());
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
    let response = ui.interact(rect, ui.id().with("volume"), Sense::click_and_drag());
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
        .interact(rect, ui.id().with(label), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, label));
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
}
