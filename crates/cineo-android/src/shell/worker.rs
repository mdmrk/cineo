use std::sync::mpsc::Receiver;
use std::time::Duration;

use cineo_core::addon::ContentType;
use cineo_core::app::{Action, Effect, LibraryItem, Notice, PlayRequest, find_subtitle, update};
use cineo_net::fetch_task;
use cineo_player::embedded::PlayerCommand;
use cineo_player::subtitles::SubtitleFiles;
use cineo_player::{PlayerEvent, forward_events};
use tracing::{error, warn};
use url::Url;

use crate::view::{ArtView, StreamFilterView, view};

use super::{FilterChange, Msg, Shell, Subtitles, lock};

const ENGINE_SHUTDOWN: Duration = Duration::from_secs(2);

impl Shell {
    pub(super) fn run(mut self, rx: &Receiver<Msg>, restore: [Action; 2]) {
        for action in restore {
            self.dispatch(action);
        }
        self.publish();
        while let Ok(first) = rx.recv() {
            let mut running = self.handle(first);
            while running && let Ok(msg) = rx.try_recv() {
                running = self.handle(msg);
            }
            if !running {
                self.torrents.shutdown(ENGINE_SHUTDOWN);
                return;
            }
            self.publish();
        }
    }

    fn handle(&mut self, msg: Msg) -> bool {
        match msg {
            Msg::Action(action) => self.dispatch(action),
            Msg::OpenDetail { content_type, id } => self.open_detail(content_type, id),
            Msg::Notice(notice) => self.state.notice = Some(notice),
            Msg::LoadSubtitle(url) => self.load_subtitle(url),
            Msg::SubtitleFetched { url, result } => self.subtitle_fetched(&url, result),
            Msg::Filter(change) => match change {
                FilterChange::Query(query) => self.filter.query = query,
                FilterChange::Quality(quality) => self.filter.quality = quality,
                FilterChange::Addon(addon) => self.filter.addon = addon,
                FilterChange::Sort(sort) => self.filter.sort = sort,
            },
            Msg::NetworkUsable { renew } => {
                if renew && let Err(err) = self.client.reset_connections() {
                    warn!(%err, "the HTTP connections were not reset");
                }
                self.dispatch(Action::RetryFailed);
            }
            Msg::LeftApp => {
                if self.state.settings.pause_on_minimize
                    && let Some(playback) = &lock(&self.screen).playback
                {
                    playback.send(PlayerCommand::SetPause(true));
                }
            }
            Msg::Shutdown => return false,
        }
        true
    }

    fn open_detail(&mut self, content_type: String, id: String) {
        let Some(content_type) = ContentType::new(content_type) else {
            warn!("not a content type");
            return;
        };
        let preview = self
            .state
            .board
            .iter()
            .chain(&self.state.search)
            .filter_map(|row| row.items.ready())
            .flatten()
            .find(|item| item.id == id && item.content_type == content_type)
            .cloned()
            .or_else(|| {
                self.state
                    .library
                    .iter()
                    .find(|item| item.id == id && item.content_type == content_type)
                    .map(LibraryItem::preview)
            })
            .map(Box::new);
        self.dispatch(Action::OpenDetail {
            content_type,
            id,
            preview,
        });
    }

    fn dispatch(&mut self, action: Action) {
        for effect in update(&mut self.state, action) {
            self.run_effect(effect);
        }
    }

    fn run_effect(&mut self, effect: Effect) {
        match effect {
            effect @ (Effect::FetchManifest { .. }
            | Effect::FetchCatalog { .. }
            | Effect::FetchAddonList { .. }
            | Effect::FetchMeta { .. }
            | Effect::FetchStreams { .. }
            | Effect::FetchSubtitles { .. }) => {
                if let Some(task) = fetch_task(&self.client, effect) {
                    let tx = self.tx.clone();
                    self.runtime.spawn(async move {
                        let _ = tx.send(Msg::Action(task.await));
                    });
                }
            }
            effect @ (Effect::SaveAddons(_)
            | Effect::SaveLibraryItem(_)
            | Effect::DeleteLibraryItem(_)
            | Effect::ClearLibrary
            | Effect::SaveSettings(_)) => {
                if self.store_tx.send(effect).is_err() {
                    self.state.notice = Some(Notice::StoreClosed);
                }
            }
            Effect::Play(request) => self.play(&request),
            Effect::StartTorrent(request) => {
                let tx = self.tx.clone();
                self.torrents
                    .start(request, &self.state.settings, move |action| {
                        let _ = tx.send(Msg::Action(action));
                    });
            }
            Effect::StopTorrent => self.torrents.stop(),
            Effect::CheckForUpdate(_) => {}
        }
    }

    fn play(&mut self, request: &PlayRequest) {
        let old = {
            let mut screen = lock(&self.screen);
            if let Some(forward) = screen.forward.take() {
                forward.abort();
            }
            screen.take_player()
        };
        drop(old);
        match self.players.start(request) {
            Ok((playback, events)) => {
                let subtitles = Subtitles {
                    files: SubtitleFiles::new(self.subtitle_dir.clone()),
                    groups: self.state.subtitles.clone(),
                    auto: request.settings.subtitle_languages().collect(),
                };
                let mut screen = lock(&self.screen);
                screen.subtitles = Some(subtitles);
                if let Some(wid) = screen.wid
                    && let Err(err) = playback.attach_surface(wid)
                {
                    warn!(%err, "the video surface was not attached");
                }
                screen.playback = Some(playback);
                screen.art = Some(ArtView::of(request));
                drop(screen);
                let tx = self.tx.clone();
                let (meta_id, video_id) = (request.meta_id.clone(), request.video_id.clone());
                let feed = self.torrents.feed();
                feed.playing(&request.url);
                let forward = self.runtime.spawn(forward_events(
                    events,
                    meta_id,
                    video_id,
                    move |event| match event {
                        PlayerEvent::Buffered { seconds } => feed.buffered(*seconds),
                        PlayerEvent::Progress { duration_ms, .. }
                        | PlayerEvent::Ended { duration_ms, .. }
                        | PlayerEvent::Closed { duration_ms, .. } => feed.duration(*duration_ms),
                        PlayerEvent::Failed(_) => {}
                    },
                    move |action| {
                        let _ = tx.send(Msg::Action(action));
                    },
                ));
                lock(&self.screen).forward = Some(forward.abort_handle());
            }
            Err(err) => {
                error!(%err, "playback could not start");
                self.dispatch(Action::PlaybackFailed(err.to_string()));
            }
        }
    }

    fn load_subtitle(&self, url: Url) {
        let Some((title, lang)) = find_subtitle(&self.state.subtitles, &url) else {
            return;
        };
        {
            let screen = lock(&self.screen);
            let (Some(playback), Some(subtitles)) = (&screen.playback, &screen.subtitles) else {
                return;
            };
            if let Some(path) = subtitles.files.get(&url) {
                if let Err(err) = playback.add_subtitle(path, &title, &lang) {
                    warn!(%err, "could not select the subtitle");
                }
                return;
            }
        }
        let client = self.client.clone();
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            let result = client
                .fetch_subtitle(&url)
                .await
                .map_err(|err| err.to_string());
            let _ = tx.send(Msg::SubtitleFetched { url, result });
        });
    }

    fn subtitle_fetched(&mut self, url: &Url, result: Result<Vec<u8>, String>) {
        let Some((title, lang)) = find_subtitle(&self.state.subtitles, url) else {
            return;
        };
        let loaded = {
            let mut screen = lock(&self.screen);
            let screen = &mut *screen;
            let (Some(playback), Some(subtitles)) = (&screen.playback, &mut screen.subtitles)
            else {
                return;
            };
            result.and_then(|bytes| {
                let path = subtitles
                    .files
                    .write(url, &bytes)
                    .map_err(|err| format!("cannot save it: {err}"))?;
                playback
                    .add_subtitle(&path, &title, &lang)
                    .map_err(|err| err.to_string())
            })
        };
        if let Err(err) = loaded {
            warn!(%err, "addon subtitle failed");
            self.state.notice = Some(Notice::SubtitleFailed(err));
        }
    }

    fn publish(&mut self) {
        let (playing, finished) = {
            let mut screen = lock(&self.screen);
            let finished = if self.state.playing.is_none() {
                screen.art = None;
                screen.take_player()
            } else {
                if let Some(subtitles) = &mut screen.subtitles
                    && subtitles.groups != self.state.subtitles
                {
                    subtitles.groups.clone_from(&self.state.subtitles);
                }
                (None, None)
            };
            (screen.art.clone(), finished)
        };
        drop(finished);
        let detail = self.state.detail.as_ref().map(|d| d.id.clone());
        if detail != self.filtered {
            self.filter = StreamFilterView {
                sort: self.filter.sort,
                ..StreamFilterView::default()
            };
            self.filtered = detail;
        }
        let next = view(&self.state, playing.as_ref(), &self.filter);
        if self.shown.as_ref() != Some(&next) {
            self.listener.changed(next.clone());
            self.shown = Some(next);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::mpsc::channel;

    use cineo_core::app::{Settings, State};
    use cineo_net::{AddonClient, NetPolicy};
    use cineo_stream::{EngineOptions, Torrents};

    use super::*;
    use crate::player::Mpv;
    use crate::shell::StateListener;
    use crate::view::AppView;

    #[derive(Debug)]
    struct Ignore;

    impl StateListener for Ignore {
        fn changed(&self, _view: AppView) {}
    }

    #[test]
    fn the_desktop_update_check_does_not_run() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (tx, rx) = channel();
        let (store_tx, _store_rx) = channel();
        let mut shell = Shell {
            state: State::default(),
            client: AddonClient::new(NetPolicy::default()).unwrap(),
            runtime: runtime.handle().clone(),
            tx,
            store_tx,
            listener: Arc::new(Ignore),
            players: Arc::new(Mpv {
                fonts_dir: std::env::temp_dir(),
            }),
            screen: Arc::default(),
            subtitle_dir: std::env::temp_dir(),
            torrents: Torrents::new(
                runtime.handle().clone(),
                EngineOptions::new(std::env::temp_dir()),
            ),
            filter: StreamFilterView::default(),
            filtered: None,
            shown: None,
        };
        shell.dispatch(Action::RestoreSettings(Settings {
            check_for_updates: true,
            ..Settings::default()
        }));
        assert!(shell.state.update_checked);
        assert_eq!(runtime.metrics().num_alive_tasks(), 0);
        assert!(rx.try_recv().is_err());
    }
}
