use cineo_core::app::{Action, SeekStep, Settings, TorrentStatus};
use cineo_player::embedded::{PlayerCommand, Status, TrackKind};
use cineo_player::subtitles::AddonSubtitle;
use eframe::egui::{
    self, Align, Color32, CornerRadius, CursorIcon, Key, Pos2, Rect, Sense, Ui, ViewportCommand,
    pos2, vec2,
};

use url::Url;

mod bars;
mod stats;
mod tracks;

use bars::{bottom_bar, next_popup, top_bar};
use stats::{stat_rows, stats_overlay, stats_shown, toggle_stats_shown};

use crate::theme;
use crate::view::{IMAGE_FILTER, paint_cover, paint_loading};

const VOLUME_STEP: f64 = 5.0;
const LOGO_MAX: egui::Vec2 = vec2(560.0, 200.0);
const LOADING_WIDTH: f32 = 176.0;
const LOADING_BACKDROP_TINT: Color32 = Color32::from_gray(70);
const PULSE_PERIOD: f64 = 1.6;
const STYLE_GAP: f32 = 8.0;
const SEEK_SETTLE_S: f64 = 1.0;
const VOLUME_SETTLE: f64 = 0.5;
const HOLD_TIMEOUT_S: f64 = 2.0;

#[derive(Debug, Clone, Copy, Default)]
struct Held {
    drag: Option<f64>,
    sent: Option<(f64, f64)>,
    busy_seen: bool,
}

impl Held {
    fn value(&self) -> Option<f64> {
        self.drag.or(self.sent.map(|(value, _)| value))
    }

    fn or(&self, actual: f64) -> f64 {
        self.value().unwrap_or(actual)
    }

    fn commit(&mut self, value: f64, now: f64) {
        self.drag = None;
        self.sent = Some((value, now));
        self.busy_seen = false;
    }

    fn settle(&mut self, actual: f64, tolerance: f64, busy: bool, now: f64) {
        let Some((value, at)) = self.sent else {
            return;
        };
        let finished = std::mem::replace(&mut self.busy_seen, busy) && !busy;
        if self.drag.is_none()
            && ((actual - value).abs() <= tolerance || finished || now - at > HOLD_TIMEOUT_S)
        {
            self.sent = None;
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Controls {
    last_activity: f64,
    last_pointer: Option<Pos2>,
    menu: Option<TrackKind>,
    style_open: bool,
    seek: Held,
    volume: Held,
    addon_request: Option<Url>,
    subtitle_delay_ms: i32,
    settings: Vec<Action>,
    next_dismissed: bool,
    next_requested: bool,
    next_shown_at: Option<f64>,
    started: bool,
    click_toggled_pause: bool,
}

impl Controls {
    fn seek_by(&mut self, delta: f64, status: &Status, now: f64) -> PlayerCommand {
        if status.duration_s > 0.0 {
            let target = (self.seek.or(status.position_s) + delta).clamp(0.0, status.duration_s);
            self.seek.commit(target, now);
        }
        PlayerCommand::SeekBy(delta)
    }

    pub fn take_addon_subtitle(&mut self) -> Option<Url> {
        self.addon_request.take()
    }

    pub fn take_settings(&mut self) -> Vec<Action> {
        std::mem::take(&mut self.settings)
    }

    pub fn take_next(&mut self) -> bool {
        std::mem::take(&mut self.next_requested)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Playback<'a> {
    pub status: &'a Status,
    pub title: &'a str,
    pub logo: Option<&'a str>,
    pub background: Option<&'a str>,
    pub addon_subtitles: &'a [AddonSubtitle],
    pub settings: &'a Settings,
    pub next: Option<NextUp<'a>>,
    pub torrent: Option<&'a TorrentStatus>,
}

#[derive(Debug, Clone, Copy)]
pub struct NextUp<'a> {
    pub title: &'a str,
    pub image: Option<&'a str>,
}

pub fn show(
    ui: &mut Ui,
    rect: Rect,
    playback: &Playback<'_>,
    controls: &mut Controls,
) -> Vec<PlayerCommand> {
    let &Playback {
        status,
        title,
        logo,
        background,
        settings,
        ..
    } = playback;
    let mut out = Vec::new();
    if theme::ensure(ui.ctx()) {
        ui.ctx().request_discard("theme applied");
        return out;
    }
    let now = ui.input(|i| i.time);
    controls
        .seek
        .settle(status.position_s, SEEK_SETTLE_S, status.seeking, now);
    controls
        .volume
        .settle(status.volume, VOLUME_SETTLE, false, now);
    track_activity(ui, controls, now);
    shortcuts(ui, status, settings, controls, now, &mut out);

    let video = ui.interact(rect, ui.id().with("video"), Sense::click());
    if video.double_clicked() {
        toggle_fullscreen(ui);
        if std::mem::take(&mut controls.click_toggled_pause) {
            out.push(PlayerCommand::TogglePause);
        }
    } else if video.clicked() {
        controls.click_toggled_pause = controls.menu.take().is_none();
        if controls.click_toggled_pause {
            out.push(PlayerCommand::TogglePause);
        }
    }

    let first_load = first_load(controls, status);
    if first_load && let Some(background) = background {
        paint_cover(
            ui,
            background,
            rect,
            CornerRadius::ZERO,
            LOADING_BACKDROP_TINT,
            0.5,
            false,
        );
    }
    if first_load && (logo.is_some() || !title.trim().is_empty()) {
        loading_art(ui, rect, title, logo);
    } else if first_load || status.is_waiting() {
        paint_loading(ui, rect.center(), LOADING_WIDTH, theme::TEXT_BRIGHT);
    }

    if stats_shown(ui.ctx()) {
        stats_overlay(ui, rect, &stat_rows(status, playback.torrent));
    }

    if let Some(next) = &playback.next {
        next_popup(ui, rect, status, settings, next, controls);
    }

    let idle = now - controls.last_activity;
    let visible = status.paused
        || !status.loaded
        || controls.menu.is_some()
        || controls.seek.drag.is_some()
        || controls.volume.drag.is_some()
        || idle < settings.hide_controls.seconds();
    if !visible {
        ui.ctx().set_cursor_icon(CursorIcon::None);
        return out;
    }
    if !status.paused {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs_f64(
                (settings.hide_controls.seconds() - idle).max(0.05),
            ));
    }

    top_bar(ui, rect, title, &mut out);
    if !first_load {
        bottom_bar(ui, rect, playback, controls, &mut out);
    }
    out
}

fn first_load(controls: &mut Controls, status: &Status) -> bool {
    controls.started |= status.loaded && !status.is_waiting();
    !controls.started
}

fn track_activity(ui: &Ui, controls: &mut Controls, now: f64) {
    let (pointer, active) = ui.input(|i| {
        (
            i.pointer.latest_pos(),
            i.pointer.any_down()
                || i.events
                    .iter()
                    .any(|e| matches!(e, egui::Event::Key { .. } | egui::Event::MouseWheel { .. })),
        )
    });
    let moved = pointer.is_some() && pointer != controls.last_pointer;
    controls.last_pointer = pointer;
    if moved || active || controls.last_activity == 0.0 {
        controls.last_activity = now;
    }
}

fn shortcuts(
    ui: &Ui,
    status: &Status,
    settings: &Settings,
    controls: &mut Controls,
    now: f64,
    out: &mut Vec<PlayerCommand>,
) {
    let fullscreen = is_fullscreen(ui.ctx());
    let mut toggle_full = false;
    let mut toggle_stats = false;
    let (step, short) = (
        settings.seek_step.seconds(),
        settings.short_seek_step.seconds(),
    );
    ui.input_mut(|i| {
        if i.consume_key(egui::Modifiers::SHIFT, Key::ArrowLeft) {
            out.push(controls.seek_by(-short, status, now));
        }
        if i.consume_key(egui::Modifiers::SHIFT, Key::ArrowRight) {
            out.push(controls.seek_by(short, status, now));
        }
        let mut key = |k| i.consume_key(egui::Modifiers::NONE, k);
        if key(Key::Space) || key(Key::K) {
            out.push(PlayerCommand::TogglePause);
        }
        if key(Key::ArrowLeft) || key(Key::J) {
            out.push(controls.seek_by(-step, status, now));
        }
        if key(Key::ArrowRight) || key(Key::L) {
            out.push(controls.seek_by(step, status, now));
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
        if key(Key::I) {
            toggle_stats = true;
        }
        if key(Key::F) || key(Key::F11) {
            toggle_full = true;
        }
        if key(Key::Escape) {
            if fullscreen && settings.escape_leaves_fullscreen {
                toggle_full = true;
            } else {
                out.push(PlayerCommand::Stop);
            }
        }
    });
    if toggle_full {
        toggle_fullscreen(ui);
    }
    if toggle_stats {
        toggle_stats_shown(ui.ctx());
    }
}

fn is_fullscreen(ctx: &egui::Context) -> bool {
    ctx.input(|i| i.viewport().fullscreen.unwrap_or(false))
}

fn toggle_fullscreen(ui: &Ui) {
    let on = !is_fullscreen(ui.ctx());
    ui.ctx().send_viewport_cmd(ViewportCommand::Fullscreen(on));
}

pub(crate) fn leave_fullscreen(ctx: &egui::Context) {
    if is_fullscreen(ctx) {
        ctx.send_viewport_cmd(ViewportCommand::Fullscreen(false));
    }
}

fn seek_icons(step: SeekStep) -> (&'static str, &'static str) {
    match step {
        SeekStep::S5 => ("rewind-backward-5", "rewind-forward-5"),
        SeekStep::S10 => ("rewind-backward-10", "rewind-forward-10"),
        SeekStep::S15 => ("rewind-backward-15", "rewind-forward-15"),
        SeekStep::S30 => ("rewind-backward-30", "rewind-forward-30"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_load_lasts_until_playback_is_no_longer_waiting() {
        let mut controls = Controls::default();
        let mut status = Status::default();
        assert!(first_load(&mut controls, &status));
        status.loaded = true;
        status.seeking = true;
        assert!(first_load(&mut controls, &status));
        status.seeking = false;
        assert!(!first_load(&mut controls, &status));
        status.buffering = true;
        assert!(
            !first_load(&mut controls, &status),
            "later waits are not the first load"
        );
    }

    #[test]
    fn a_held_seek_ends_when_the_player_finishes_seeking_or_too_long_passes() {
        let mut held = Held::default();
        held.commit(90.0, 0.0);
        held.settle(10.0, SEEK_SETTLE_S, true, 0.1);
        assert_eq!(held.or(10.0), 90.0, "still seeking");
        held.settle(86.0, SEEK_SETTLE_S, false, 0.2);
        assert_eq!(held.or(86.0), 86.0, "landed on an earlier keyframe");
        held.commit(90.0, 0.0);
        held.settle(10.0, SEEK_SETTLE_S, false, HOLD_TIMEOUT_S + 0.1);
        assert_eq!(held.value(), None, "a lost update does not pin the bar");
    }

    #[test]
    fn relative_seeks_add_up_from_the_pending_target_within_the_media() {
        let mut controls = Controls::default();
        let status = Status {
            position_s: 50.0,
            duration_s: 100.0,
            ..Status::default()
        };
        assert_eq!(
            controls.seek_by(10.0, &status, 0.0),
            PlayerCommand::SeekBy(10.0)
        );
        controls.seek_by(10.0, &status, 0.0);
        assert_eq!(controls.seek.value(), Some(70.0));
        controls.seek_by(60.0, &status, 0.0);
        assert_eq!(controls.seek.value(), Some(100.0));
        let mut controls = Controls::default();
        controls.seek_by(10.0, &Status::default(), 0.0);
        assert_eq!(
            controls.seek.value(),
            None,
            "nothing to hold without a duration"
        );
    }
}
