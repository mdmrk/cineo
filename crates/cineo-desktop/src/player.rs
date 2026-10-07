//! The playback screen: the video fills the window under Cineo's own
//! controls (ADR-0014). [`show`] is pure rendering over a player
//! [`Status`]; it returns the [`PlayerCommand`]s the user triggered and
//! performs no IO. Track names come from the media file and are shown as
//! plain text only.

use cineo_player::embedded::{PlayerCommand, Status, Track, TrackKind};
use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, CursorIcon, Key, Label, Layout, Pos2, Rect,
    Response, RichText, Sense, Ui, UiBuilder, ViewportCommand, WidgetInfo, WidgetType, pos2, vec2,
};

use crate::theme;
use crate::view::{IMAGE_FILTER, Icon, gradient, paint_icon, paint_spinner};

const HIDE_AFTER: f64 = 2.5;
const SEEK_STEP: f64 = 10.0;
const VOLUME_STEP: f64 = 5.0;
const BAR_HEIGHT: f32 = 96.0;
const BUTTON: f32 = 36.0;
const LOGO_MAX: egui::Vec2 = vec2(560.0, 200.0);
const PULSE_PERIOD: f64 = 1.6;
const MENU_MAX_HEIGHT: f32 = 320.0;

/// Presentation-only state of the playback screen.
#[derive(Debug, Clone, Default)]
pub struct Controls {
    last_activity: f64,
    last_pointer: Option<Pos2>,
    menu: Option<TrackKind>,
    seek_drag: Option<f64>,
    volume_drag: Option<f64>,
}

/// Draws the controls over `rect` (the video) and returns the commands to
/// send to the player.
pub fn show(
    ui: &mut Ui,
    rect: Rect,
    status: &Status,
    title: &str,
    logo: Option<&str>,
    controls: &mut Controls,
) -> Vec<PlayerCommand> {
    let mut out = Vec::new();
    if theme::ensure(ui.ctx()) {
        ui.ctx().request_discard("theme applied");
        return out;
    }
    let now = ui.input(|i| i.time);
    track_activity(ui, controls, now);
    shortcuts(ui, status, &mut out);

    let video = ui.interact(rect, ui.id().with("video"), Sense::click());
    if video.double_clicked() {
        toggle_fullscreen(ui);
    } else if video.clicked() && controls.menu.take().is_none() {
        out.push(PlayerCommand::TogglePause);
    }

    if !status.loaded && (logo.is_some() || !title.trim().is_empty()) {
        loading_art(ui, rect, title, logo);
    } else if !status.loaded || status.buffering {
        paint_spinner(ui, rect.center(), 44.0, theme::TEXT_BRIGHT);
    }

    let idle = now - controls.last_activity;
    let visible = status.paused
        || !status.loaded
        || controls.menu.is_some()
        || controls.seek_drag.is_some()
        || controls.volume_drag.is_some()
        || idle < HIDE_AFTER;
    if !visible {
        ui.ctx().set_cursor_icon(CursorIcon::None);
        return out;
    }
    if !status.paused {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs_f64(
                (HIDE_AFTER - idle).max(0.05),
            ));
    }

    top_bar(ui, rect, title, &mut out);
    if status.loaded {
        bottom_bar(ui, rect, status, controls, &mut out);
    }
    out
}

fn track_activity(ui: &Ui, controls: &mut Controls, now: f64) {
    let (pointer, active) = ui.input(|i| {
        (
            i.pointer.latest_pos(),
            i.pointer.any_down()
                || !i.events.is_empty()
                    && i.events.iter().any(|e| {
                        matches!(e, egui::Event::Key { .. } | egui::Event::MouseWheel { .. })
                    }),
        )
    });
    let moved = pointer.is_some() && pointer != controls.last_pointer;
    controls.last_pointer = pointer;
    if moved || active || controls.last_activity == 0.0 {
        controls.last_activity = now;
    }
}

fn shortcuts(ui: &Ui, status: &Status, out: &mut Vec<PlayerCommand>) {
    let fullscreen = is_fullscreen(ui);
    let mut toggle_full = false;
    ui.input_mut(|i| {
        let mut key = |k| i.consume_key(egui::Modifiers::NONE, k);
        if key(Key::Space) || key(Key::K) {
            out.push(PlayerCommand::TogglePause);
        }
        if key(Key::ArrowLeft) || key(Key::J) {
            out.push(PlayerCommand::SeekBy(-SEEK_STEP));
        }
        if key(Key::ArrowRight) || key(Key::L) {
            out.push(PlayerCommand::SeekBy(SEEK_STEP));
        }
        if key(Key::ArrowUp) {
            out.push(PlayerCommand::SetVolume(status.volume + VOLUME_STEP));
        }
        if key(Key::ArrowDown) {
            out.push(PlayerCommand::SetVolume(status.volume - VOLUME_STEP));
        }
        if key(Key::M) {
            out.push(PlayerCommand::ToggleMute);
        }
        if key(Key::F) || key(Key::F11) {
            toggle_full = true;
        }
        if key(Key::Escape) {
            if fullscreen {
                toggle_full = true;
            } else {
                out.push(PlayerCommand::Stop);
            }
        }
    });
    if toggle_full {
        toggle_fullscreen(ui);
    }
}

fn is_fullscreen(ui: &Ui) -> bool {
    ui.input(|i| i.viewport().fullscreen.unwrap_or(false))
}

fn toggle_fullscreen(ui: &Ui) {
    let on = !is_fullscreen(ui);
    ui.ctx().send_viewport_cmd(ViewportCommand::Fullscreen(on));
}

/// Leaves fullscreen; called when playback ends.
pub fn leave_fullscreen(ctx: &egui::Context) {
    if ctx.input(|i| i.viewport().fullscreen.unwrap_or(false)) {
        ctx.send_viewport_cmd(ViewportCommand::Fullscreen(false));
    }
}

fn loading_art(ui: &Ui, rect: Rect, title: &str, logo: Option<&str>) {
    let opacity = pulse_opacity(ui.input(|i| i.time));
    ui.ctx().request_repaint();
    if let Some(logo) = logo {
        let image = egui::Image::new(logo)
            .texture_options(IMAGE_FILTER)
            .show_loading_spinner(false);
        let max = vec2(
            (rect.width() * 0.5).min(LOGO_MAX.x),
            (rect.height() * 0.3).min(LOGO_MAX.y),
        );
        match image.load_for_size(ui.ctx(), max) {
            Ok(poll) => {
                if let Some(size) = poll.size() {
                    let scale = (max.x / size.x).min(max.y / size.y);
                    image
                        .tint(Color32::WHITE.gamma_multiply(opacity))
                        .paint_at(ui, Rect::from_center_size(rect.center(), size * scale));
                }
                return;
            }
            Err(_) if title.trim().is_empty() => return,
            Err(_) => {}
        }
    }
    let color = theme::TEXT_BRIGHT.gamma_multiply(opacity);
    let font = egui::FontId::new(44.0, theme::heading().family);
    let mut job =
        egui::text::LayoutJob::simple(title.to_owned(), font, color, (rect.width() * 0.7).max(1.0));
    job.halign = Align::Center;
    job.wrap.max_rows = 3;
    let galley = ui.painter().layout_job(job);
    let pos = pos2(rect.center().x, rect.center().y - galley.size().y / 2.0);
    ui.painter().galley(pos, galley, color);
}

fn pulse_opacity(time: f64) -> f32 {
    let phase = 0.5 - 0.5 * (time * std::f64::consts::TAU / PULSE_PERIOD).cos();
    #[expect(clippy::cast_possible_truncation, reason = "a factor in 0..=1")]
    let phase = phase as f32;
    0.25 + 0.75 * phase
}

fn top_bar(ui: &mut Ui, rect: Rect, title: &str, out: &mut Vec<PlayerCommand>) {
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
    if icon_button(ui, back, Icon::ArrowLeft, "Back").clicked() {
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

fn bottom_bar(
    ui: &mut Ui,
    rect: Rect,
    status: &Status,
    controls: &mut Controls,
    out: &mut Vec<PlayerCommand>,
) {
    let bar = Rect::from_min_max(pos2(rect.left(), rect.bottom() - BAR_HEIGHT), rect.max);
    gradient(
        ui,
        bar,
        Color32::TRANSPARENT,
        Color32::from_black_alpha(210),
        false,
    );
    let _ = ui.interact(bar, ui.id().with("bar"), Sense::click());

    let margin = 20.0;
    let seek = Rect::from_min_max(
        pos2(bar.left() + margin, bar.top() + 34.0),
        pos2(bar.right() - margin, bar.top() + 50.0),
    );
    seek_bar(ui, seek, status, controls, out);

    let y = bar.bottom() - 26.0;
    let mut x = bar.left() + margin + BUTTON / 2.0 - 6.0;
    let mut slot = |step: f32| {
        let c = pos2(x, y);
        x += step;
        Rect::from_center_size(c, egui::Vec2::splat(BUTTON))
    };
    let (icon, label) = if status.paused {
        (Icon::Play, "Play")
    } else {
        (Icon::Pause, "Pause")
    };
    if icon_button(ui, slot(BUTTON + 4.0), icon, label).clicked() {
        out.push(PlayerCommand::TogglePause);
    }
    if icon_button(
        ui,
        slot(BUTTON + 4.0),
        Icon::Named("rewind-backward-10"),
        "Seek back 10 seconds",
    )
    .clicked()
    {
        out.push(PlayerCommand::SeekBy(-SEEK_STEP));
    }
    if icon_button(
        ui,
        slot(BUTTON + 12.0),
        Icon::Named("rewind-forward-10"),
        "Seek forward 10 seconds",
    )
    .clicked()
    {
        out.push(PlayerCommand::SeekBy(SEEK_STEP));
    }
    let shown = controls
        .seek_drag
        .map_or(status.position_s, |f| f * status.duration_s);
    let time = format!(
        "{} / {}",
        clock(shown, status.duration_s),
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
    let (icon, label) = if is_fullscreen(ui) {
        ("minimize", "Exit fullscreen")
    } else {
        ("maximize", "Fullscreen")
    };
    if icon_button(ui, slot(BUTTON + 4.0), Icon::Named(icon), label).clicked() {
        toggle_fullscreen(ui);
    }
    let subtitles = slot(BUTTON + 4.0);
    if icon_button(ui, subtitles, Icon::Named("badge-cc"), "Subtitles").clicked() {
        controls.menu = toggle(controls.menu, TrackKind::Subtitle);
    }
    let audio = slot(BUTTON + 12.0);
    if icon_button(ui, audio, Icon::Named("language"), "Audio").clicked() {
        controls.menu = toggle(controls.menu, TrackKind::Audio);
    }
    let volume = Rect::from_min_max(
        pos2(x - 80.0 + BUTTON / 2.0, y - 8.0),
        pos2(x + BUTTON / 2.0, y + 8.0),
    );
    volume_slider(ui, volume, status, controls, out);
    let mute = Rect::from_center_size(
        pos2(volume.left() - BUTTON / 2.0 - 4.0, y),
        egui::Vec2::splat(BUTTON),
    );
    let (icon, label) = if status.muted || status.volume <= 0.0 {
        ("volume-off", "Unmute")
    } else {
        ("volume", "Mute")
    };
    if icon_button(ui, mute, Icon::Named(icon), label).clicked() {
        out.push(PlayerCommand::ToggleMute);
    }

    match controls.menu {
        Some(TrackKind::Audio) => track_menu(ui, audio, status, TrackKind::Audio, controls, out),
        Some(TrackKind::Subtitle) => {
            track_menu(ui, subtitles, status, TrackKind::Subtitle, controls, out);
        }
        _ => {}
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

fn toggle(menu: Option<TrackKind>, kind: TrackKind) -> Option<TrackKind> {
    (menu != Some(kind)).then_some(kind)
}

fn seek_bar(
    ui: &mut Ui,
    rect: Rect,
    status: &Status,
    controls: &mut Controls,
    out: &mut Vec<PlayerCommand>,
) {
    let response = ui.interact(rect, ui.id().with("seek"), Sense::click_and_drag());
    response.widget_info(|| WidgetInfo::slider(true, status.position_s, "Position"));
    let seekable = status.duration_s > 0.0;
    let fraction_at = |pos: Pos2| f64::from(((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0));
    if seekable {
        if let Some(pos) = response.interact_pointer_pos()
            && (response.dragged() || response.is_pointer_button_down_on())
        {
            controls.seek_drag = Some(fraction_at(pos));
        }
        if (response.drag_stopped() || response.clicked())
            && let Some(fraction) = controls.seek_drag.take()
        {
            out.push(PlayerCommand::SeekTo(fraction * status.duration_s));
        }
    }
    let hovered = response.hovered() || controls.seek_drag.is_some();
    let thickness = if hovered { 6.0 } else { 4.0 };
    let track = Rect::from_center_size(rect.center(), vec2(rect.width(), thickness));
    let radius = CornerRadius::same(3);
    ui.painter()
        .rect_filled(track, radius, Color32::from_white_alpha(60));
    let played = controls.seek_drag.unwrap_or(if seekable {
        (status.position_s / status.duration_s).clamp(0.0, 1.0)
    } else {
        0.0
    });
    #[expect(clippy::cast_possible_truncation, reason = "a fraction in 0..=1")]
    let played = played as f32;
    let filled = Rect::from_min_max(
        track.min,
        pos2(track.left() + track.width() * played, track.bottom()),
    );
    ui.painter().rect_filled(filled, radius, theme::ACCENT);
    if hovered && seekable {
        ui.painter()
            .circle_filled(pos2(filled.right(), track.center().y), 7.0, theme::ACCENT);
        if let Some(pos) = response.hover_pos() {
            let at = fraction_at(pos) * status.duration_s;
            ui.painter().text(
                pos2(pos.x, rect.top() - 10.0),
                Align2::CENTER_BOTTOM,
                clock(at, status.duration_s),
                theme::caption(),
                theme::TEXT_BRIGHT,
            );
        }
        response.on_hover_cursor(CursorIcon::PointingHand);
    }
}

fn volume_slider(
    ui: &mut Ui,
    rect: Rect,
    status: &Status,
    controls: &mut Controls,
    out: &mut Vec<PlayerCommand>,
) {
    let response = ui.interact(rect, ui.id().with("volume"), Sense::click_and_drag());
    response.widget_info(|| WidgetInfo::slider(true, status.volume, "Volume"));
    if let Some(pos) = response.interact_pointer_pos()
        && (response.dragged() || response.clicked())
    {
        let percent = f64::from(((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0)) * 100.0;
        if controls.volume_drag != Some(percent) {
            controls.volume_drag = Some(percent);
            out.push(PlayerCommand::SetVolume(percent));
        }
    }
    if response.drag_stopped() || response.clicked() || !response.dragged() {
        controls.volume_drag = None;
    }
    let level = if status.muted {
        0.0
    } else {
        controls.volume_drag.unwrap_or(status.volume) / 100.0
    };
    #[expect(clippy::cast_possible_truncation, reason = "a fraction in 0..=1")]
    let level = level.clamp(0.0, 1.0) as f32;
    let track = Rect::from_center_size(rect.center(), vec2(rect.width(), 4.0));
    let radius = CornerRadius::same(2);
    ui.painter()
        .rect_filled(track, radius, Color32::from_white_alpha(60));
    let filled = Rect::from_min_max(
        track.min,
        pos2(track.left() + track.width() * level, track.bottom()),
    );
    ui.painter().rect_filled(filled, radius, theme::TEXT_BRIGHT);
    if response.hovered() {
        ui.painter().circle_filled(
            pos2(filled.right(), track.center().y),
            6.0,
            theme::TEXT_BRIGHT,
        );
    }
}

fn track_menu(
    ui: &mut Ui,
    anchor: Rect,
    status: &Status,
    kind: TrackKind,
    controls: &mut Controls,
    out: &mut Vec<PlayerCommand>,
) {
    let tracks: Vec<&Track> = status.tracks.iter().filter(|t| t.kind == kind).collect();
    let mut entries: Vec<(Option<i64>, String, bool)> = Vec::new();
    if kind == TrackKind::Subtitle {
        entries.push((None, "Off".into(), !tracks.iter().any(|t| t.selected)));
    }
    entries.extend(
        tracks
            .iter()
            .enumerate()
            .map(|(i, t)| (Some(t.id), track_label(t, i + 1), t.selected)),
    );
    if entries.is_empty() {
        entries.push((None, "No audio tracks".into(), false));
    }
    let row = 30.0;
    let width = 240.0;
    #[expect(clippy::cast_precision_loss, reason = "a short list")]
    let content = entries.len() as f32 * row;
    let room = (anchor.top() - ui.max_rect().top() - 24.0).max(row);
    let height = content.min(MENU_MAX_HEIGHT).min(room) + 8.0;
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
                    .max_height(height - 8.0)
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
                            painter.text(
                                pos2(item.left() + 30.0, item.center().y),
                                Align2::LEFT_CENTER,
                                &label,
                                theme::body(),
                                if selected {
                                    theme::TEXT_BRIGHT
                                } else {
                                    theme::TEXT
                                },
                            );
                            if response.clicked() && (id.is_some() || kind == TrackKind::Subtitle) {
                                out.push(match kind {
                                    TrackKind::Subtitle => PlayerCommand::SetSubtitle(id),
                                    _ => PlayerCommand::SetAudio(id),
                                });
                                controls.menu = None;
                            }
                        }
                    });
            });
        });
}

fn track_label(track: &Track, number: usize) -> String {
    let parts: Vec<&str> = [track.lang.as_deref(), track.title.as_deref()]
        .into_iter()
        .flatten()
        .collect();
    if parts.is_empty() {
        format!("Track {number}")
    } else {
        let label = parts.join(" · ").replace(['\n', '\r'], " ");
        label.chars().take(60).collect()
    }
}

fn icon_button(ui: &mut Ui, rect: Rect, icon: Icon, label: &str) -> Response {
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

fn clock(seconds: f64, scale: f64) -> String {
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
    fn the_loading_art_pulses_between_faint_and_opaque() {
        assert!((pulse_opacity(0.0) - 0.25).abs() < 1e-6);
        assert!((pulse_opacity(PULSE_PERIOD / 2.0) - 1.0).abs() < 1e-6);
        assert!((pulse_opacity(PULSE_PERIOD) - 0.25).abs() < 1e-6);
        for step in 0..100 {
            let opacity = pulse_opacity(f64::from(step) * 0.037);
            assert!((0.25..=1.0).contains(&opacity), "{opacity}");
        }
    }

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
    fn track_labels_use_language_and_title() {
        let track = |lang: Option<&str>, title: Option<&str>| Track {
            id: 1,
            kind: TrackKind::Audio,
            title: title.map(str::to_owned),
            lang: lang.map(str::to_owned),
            selected: false,
        };
        assert_eq!(
            track_label(&track(Some("eng"), Some("Commentary")), 1),
            "eng · Commentary"
        );
        assert_eq!(track_label(&track(None, None), 2), "Track 2");
        assert_eq!(track_label(&track(Some("a\nb"), None), 1), "a b");
    }

    #[test]
    fn every_player_icon_exists() {
        for name in [
            "rewind-backward-10",
            "rewind-forward-10",
            "maximize",
            "minimize",
            "badge-cc",
            "language",
            "volume",
            "volume-off",
            "check",
        ] {
            assert!(Icon::Named(name).glyph().is_some(), "{name}");
        }
    }
}
