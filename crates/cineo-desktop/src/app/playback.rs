use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use cineo_core::app::{Action, Notice, Percent, PlayRequest, Setting, Settings, find_subtitle};
use cineo_player::PlayerError;
use cineo_player::embedded::{Player, PlayerCommand, Status, Video};
use cineo_player::subtitles::{self, AutoPick, SubtitleFiles};
use cineo_player::{PlayerEvent, forward_events};
use cineo_stream::PlayerFeed;
use eframe::egui;
use tracing::{error, warn};

use crate::player::{self, Controls, NextUp, Playback};

use super::{CineoApp, Embedded, Msg};

impl CineoApp {
    pub(super) fn play(&mut self, request: &PlayRequest) {
        self.end_playback();
        match self.start_player(request) {
            Ok(embedded) => self.playback = Some(Box::new(embedded)),
            Err(err) => {
                error!(%err, "playback could not start");
                self.sender()(Action::PlaybackFailed(err.to_string()));
            }
        }
    }

    fn start_player(&self, request: &PlayRequest) -> Result<Embedded, PlayerError> {
        let gl = self
            .io
            .gl
            .clone()
            .ok_or_else(|| PlayerError::Render("the window has no OpenGL context".into()))?;
        let (repaint, on_frame) = (self.ctx.clone(), self.ctx.clone());
        let video = Video {
            get_proc_address: gl,
            on_frame: Box::new(move || on_frame.request_repaint()),
            display: self.io.display,
        };
        let (player, renderer, events) =
            Player::start(request, Arc::new(move || repaint.request_repaint()), video)?;
        let (meta_id, video_id) = (request.meta_id.clone(), request.video_id.clone());
        let send = self.sender();
        let feed = self.torrents.feed();
        feed.playing(&request.url);
        let forward = self.io.runtime.spawn(forward_events(
            events,
            meta_id,
            video_id,
            move |event| feed_player(&feed, event),
            send,
        ));
        Ok(Embedded {
            player,
            renderer: Arc::new(Mutex::new(Some(renderer))),
            title: request.title.clone(),
            logo: request.logo.as_ref().map(ToString::to_string),
            background: request.background.as_ref().map(ToString::to_string),
            controls: Controls::default(),
            forward: Some(forward.abort_handle()),
            subtitle_files: SubtitleFiles::new(self.subtitle_dir()),
            auto_subtitle: request.settings.subtitle_languages().collect(),
            volume: None,
            minimized: false,
        })
    }

    fn subtitle_dir(&self) -> PathBuf {
        self.io.engine.cache_dir.join("subtitles")
    }

    fn load_addon_subtitle(&self, url: url::Url) {
        let Some(embedded) = &self.playback else {
            return;
        };
        let Some(subtitle) = find_subtitle(&self.state.subtitles, &url) else {
            return;
        };
        if let Some(path) = embedded.subtitle_files.get(&url) {
            if let Err(err) = embedded.player.add_subtitle(path, &subtitle.0, &subtitle.1) {
                warn!(%err, "could not select the subtitle");
            }
            return;
        }
        let client = self.io.client.clone();
        let post = self.post();
        self.io.runtime.spawn(async move {
            let result = client.fetch_subtitle(&url).await.map_err(|e| e.to_string());
            post(Msg::SubtitleFetched { url, result });
        });
    }

    pub(super) fn subtitle_fetched(&mut self, url: &url::Url, result: Result<Vec<u8>, String>) {
        let Some((title, lang)) = find_subtitle(&self.state.subtitles, url) else {
            return;
        };
        let Some(embedded) = &mut self.playback else {
            return;
        };
        let loaded = result.and_then(|bytes| {
            let path = embedded
                .subtitle_files
                .write(url, &bytes)
                .map_err(|e| format!("cannot save it: {e}"))?;
            embedded
                .player
                .add_subtitle(&path, &title, &lang)
                .map_err(|e| e.to_string())
        });
        if let Err(err) = loaded {
            warn!(%err, "addon subtitle failed");
            self.state.notice = Some(Notice::SubtitleFailed(err));
        }
    }

    pub(super) fn end_playback(&mut self) {
        let volume = self.playback.take().and_then(|p| p.volume);
        if let Some(volume) = volume_to_save(&self.state.settings, volume) {
            self.dispatch(Action::ChangeSetting(Setting::Volume(volume)));
        }
    }

    pub(super) fn pause_on_minimize(&mut self, ctx: &egui::Context) {
        let Some(embedded) = &mut self.playback else {
            return;
        };
        let minimized = ctx.input(|i| i.viewport().minimized.unwrap_or(false));
        if minimized
            && !embedded.minimized
            && should_pause_on_minimize(&self.state.settings, &embedded.player.status())
        {
            embedded.player.send(PlayerCommand::TogglePause);
        }
        embedded.minimized = minimized;
    }

    pub(super) fn show_connecting(&mut self, ui: &mut egui::Ui) -> bool {
        let Some(torrent) = &self.state.torrent else {
            return false;
        };
        let Some(request) = torrent.connecting() else {
            return false;
        };
        let mut commands = Vec::new();
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(egui::Color32::BLACK))
            .show(ui, |ui| {
                let rect = ui.max_rect();
                let playback = Playback {
                    status: &Status::default(),
                    title: &request.title,
                    logo: request.logo.as_ref().map(url::Url::as_str),
                    background: request.background.as_ref().map(url::Url::as_str),
                    addon_subtitles: &[],
                    settings: &self.state.settings,
                    next: None,
                    torrent: Some(&torrent.status),
                };
                commands = player::show(ui, rect, &playback, &mut self.connecting_controls);
            });
        if commands.contains(&PlayerCommand::Stop) {
            self.connecting_controls = Controls::default();
            player::leave_fullscreen(&self.ctx);
            self.dispatch(Action::PlaybackStopped);
        }
        true
    }

    pub(super) fn show_player(&mut self, ui: &mut egui::Ui) -> bool {
        let Some(embedded) = &mut self.playback else {
            return false;
        };
        if embedded.player.is_finished() {
            embedded.forward = None;
            self.end_playback();
            player::leave_fullscreen(&self.ctx);
            return false;
        }
        let status = embedded.player.status();
        if status.loaded {
            embedded.volume = Some(status.volume);
        }
        let addon_subtitles = embedded
            .subtitle_files
            .entries(&self.state.subtitles, &status.tracks);
        let renderer = Arc::clone(&embedded.renderer);
        let mut commands = Vec::new();
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(egui::Color32::BLACK))
            .show(ui, |ui| {
                let rect = ui.max_rect();
                ui.painter().add(egui::PaintCallback {
                    rect,
                    callback: Arc::new(eframe::egui_glow::CallbackFn::new(move |info, _| {
                        if let Some(renderer) = renderer
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner)
                            .as_mut()
                        {
                            let [w, h] = info.screen_size_px;
                            renderer.draw(0, w, h);
                        }
                    })),
                });
                let playback = Playback {
                    status: &status,
                    title: &embedded.title,
                    logo: embedded.logo.as_deref(),
                    background: embedded.background.as_deref(),
                    addon_subtitles,
                    settings: &self.state.settings,
                    next: self
                        .state
                        .playing
                        .as_ref()
                        .and_then(|p| p.next.as_ref())
                        .map(|n| NextUp {
                            title: &n.title,
                            image: n.image.as_ref().map(url::Url::as_str),
                        }),
                    torrent: self.state.torrent.as_ref().map(|t| &t.status),
                };
                commands = player::show(ui, rect, &playback, &mut embedded.controls);
            });
        let play_next = embedded.controls.take_next();
        let changes = embedded.controls.take_settings();
        let mut picked = embedded.controls.take_addon_subtitle();
        if picked.is_some()
            || commands
                .iter()
                .any(|c| matches!(c, PlayerCommand::SetSubtitle(_)))
        {
            embedded.auto_subtitle.clear();
        }
        for command in commands {
            embedded.player.send(command);
        }
        if !embedded.auto_subtitle.is_empty() && status.loaded && !status.tracks.is_empty() {
            match subtitles::auto_pick(&embedded.auto_subtitle, &status.tracks, addon_subtitles) {
                AutoPick::Done => embedded.auto_subtitle.clear(),
                AutoPick::Load(url) => {
                    embedded.auto_subtitle.clear();
                    picked = Some(url);
                }
                AutoPick::Wait => {}
            }
        }
        if let Some(url) = picked {
            self.load_addon_subtitle(url);
        }
        if !changes.is_empty() {
            for action in changes {
                self.dispatch(action);
            }
            if let Some(embedded) = &self.playback {
                embedded.player.set_subtitle_style(&self.state.settings);
            }
        }
        if play_next {
            self.end_playback();
            self.dispatch(Action::PlayNext);
            if self.playback.is_none() && self.state.playing.is_none() {
                player::leave_fullscreen(&self.ctx);
            }
        }
        true
    }
}

fn volume_to_save(settings: &Settings, volume: Option<f64>) -> Option<Percent> {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "mpv volume, rounded and clamped by Percent"
    )]
    let volume = Percent::new(volume?.round().max(0.0) as u32);
    (settings.remember_volume && settings.volume != volume).then_some(volume)
}

fn feed_player(feed: &PlayerFeed, event: &PlayerEvent) {
    match event {
        PlayerEvent::Buffered { seconds } => feed.buffered(*seconds),
        PlayerEvent::Progress { duration_ms, .. }
        | PlayerEvent::Ended { duration_ms, .. }
        | PlayerEvent::Closed { duration_ms, .. } => feed.duration(*duration_ms),
        PlayerEvent::Failed(_) => {}
    }
}

fn should_pause_on_minimize(settings: &Settings, status: &Status) -> bool {
    settings.pause_on_minimize && status.loaded && !status.paused
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_volume_is_kept_only_when_remembering() {
        let settings = Settings::default();
        assert_eq!(
            volume_to_save(&settings, Some(42.4)),
            Some(Percent::new(42))
        );
        assert_eq!(volume_to_save(&settings, Some(100.0)), None, "unchanged");
        assert_eq!(volume_to_save(&settings, None), None, "never loaded");
        let off = Settings {
            remember_volume: false,
            ..settings
        };
        assert_eq!(volume_to_save(&off, Some(42.0)), None);
    }

    #[test]
    fn minimizing_pauses_only_a_playing_video_when_asked() {
        let on = Settings {
            pause_on_minimize: true,
            ..Settings::default()
        };
        let playing = Status {
            loaded: true,
            ..Status::default()
        };
        assert!(should_pause_on_minimize(&on, &playing));
        assert!(!should_pause_on_minimize(&Settings::default(), &playing));
        let paused = Status {
            paused: true,
            ..playing
        };
        assert!(!should_pause_on_minimize(&on, &paused));
    }
}
