use std::num::{NonZeroU32, NonZeroU64};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use cineo_core::app::{Action, Lookahead, Settings, TorrentRequest, TorrentStatus};
use tokio::runtime::Handle;
use tokio::sync::{Mutex, watch};
use tokio::task::AbortHandle;
use tracing::error;
use url::Url;

use crate::{Engine, EngineOptions};

const STATUS_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, PartialEq)]
struct Feed {
    url: Url,
    buffered_s: Option<u64>,
    duration_ms: u64,
}

#[derive(Debug, Clone)]
pub struct PlayerFeed(watch::Sender<Option<Feed>>);

impl PlayerFeed {
    pub fn playing(&self, url: &Url) {
        self.0.send_replace(Some(Feed {
            url: url.clone(),
            buffered_s: None,
            duration_ms: 0,
        }));
    }

    pub fn buffered(&self, seconds: u64) {
        self.0.send_if_modified(|feed| {
            feed.as_mut()
                .is_some_and(|feed| feed.buffered_s.replace(seconds) != Some(seconds))
        });
    }

    pub fn duration(&self, duration_ms: u64) {
        if duration_ms == 0 {
            return;
        }
        self.0.send_if_modified(|feed| {
            feed.as_mut().is_some_and(|feed| {
                std::mem::replace(&mut feed.duration_ms, duration_ms) != duration_ms
            })
        });
    }
}

pub struct Torrents {
    runtime: Handle,
    base: EngineOptions,
    engine: Arc<Mutex<Option<Engine>>>,
    generation: Arc<AtomicU64>,
    task: Option<AbortHandle>,
    feed: PlayerFeed,
}

impl std::fmt::Debug for Torrents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Torrents")
            .field("base", &self.base)
            .finish_non_exhaustive()
    }
}

impl Torrents {
    pub fn new(runtime: Handle, base: EngineOptions) -> Self {
        Self {
            runtime,
            base,
            engine: Arc::default(),
            generation: Arc::default(),
            task: None,
            feed: PlayerFeed(watch::Sender::new(None)),
        }
    }

    pub fn base(&self) -> &EngineOptions {
        &self.base
    }

    pub fn feed(&self) -> PlayerFeed {
        self.feed.clone()
    }

    pub fn start(
        &mut self,
        request: TorrentRequest,
        settings: &Settings,
        send: impl Fn(Action) + Send + 'static,
    ) {
        let generation = self.next_generation();
        let current = Arc::clone(&self.generation);
        let engine = Arc::clone(&self.engine);
        let options = engine_options(&self.base, settings);
        let mut feed = self.feed.0.subscribe();
        let task = self.runtime.spawn(async move {
            let info_hash = request.info_hash.clone();
            let opened = {
                let mut engine = engine.lock().await;
                if current.load(Ordering::SeqCst) != generation {
                    return;
                }
                open_torrent(&mut engine, options, &request).await
            };
            let url = match opened {
                Ok(url) => url,
                Err(reason) => {
                    send(Action::TorrentFailed { info_hash, reason });
                    return;
                }
            };
            send(Action::TorrentReady {
                info_hash: info_hash.clone(),
                url: url.clone(),
            });
            let mut ticks = tokio::time::interval(STATUS_INTERVAL);
            let mut last = None;
            let mut lookahead = Lookahead::default();
            let mut size = None;
            loop {
                let ticked = tokio::select! {
                    _ = ticks.tick() => true,
                    Ok(()) = feed.changed() => false,
                };
                if current.load(Ordering::SeqCst) != generation {
                    return;
                }
                let running = engine.lock().await;
                let Some(running) = running.as_ref() else {
                    return;
                };
                if ticked {
                    let status = match running.status() {
                        Some((hash, status)) if hash == info_hash => status,
                        _ => return,
                    };
                    if let TorrentStatus::Streaming { size: bytes, .. } = status {
                        size = Some(bytes);
                    }
                    if last.as_ref() != Some(&status) {
                        last = Some(status.clone());
                        send(Action::TorrentStatus {
                            info_hash: info_hash.clone(),
                            status,
                        });
                    }
                }
                let player = feed
                    .borrow_and_update()
                    .as_ref()
                    .filter(|feed| feed.url == url)
                    .and_then(|feed| Some((feed.buffered_s?, feed.duration_ms)));
                if let (Some((buffered_s, duration_ms)), Some(size)) = (player, size)
                    && let Some(bytes) = lookahead.update(buffered_s, size, duration_ms)
                {
                    running.set_lookahead(&info_hash, bytes);
                }
            }
        });
        self.task = Some(task.abort_handle());
    }

    pub fn stop(&mut self) {
        let generation = self.next_generation();
        let current = Arc::clone(&self.generation);
        let engine = Arc::clone(&self.engine);
        self.runtime.spawn(async move {
            let engine = engine.lock().await;
            if current.load(Ordering::SeqCst) == generation
                && let Some(engine) = engine.as_ref()
            {
                engine.stop().await;
            }
        });
    }

    pub fn shutdown(&mut self, timeout: Duration) {
        self.next_generation();
        let engine = Arc::clone(&self.engine);
        let shutdown = async move {
            if let Some(engine) = engine.lock().await.take() {
                engine.shutdown().await;
            }
        };
        let _ = self
            .runtime
            .block_on(async { tokio::time::timeout(timeout, shutdown).await });
    }

    fn next_generation(&mut self) -> u64 {
        if let Some(task) = self.task.take() {
            task.abort();
        }
        self.generation.fetch_add(1, Ordering::SeqCst) + 1
    }
}

async fn open_torrent(
    slot: &mut Option<Engine>,
    options: EngineOptions,
    request: &TorrentRequest,
) -> Result<Url, String> {
    if let Some(stale) = slot.take_if(|engine| *engine.options() != options) {
        stale.shutdown().await;
    }
    let engine = match slot {
        Some(engine) => engine,
        None => slot.insert(Engine::start(options).await.map_err(|e| e.to_string())?),
    };
    engine.open(request).await.map_err(|err| {
        error!(error = %err, "opening the torrent failed");
        err.to_string()
    })
}

pub fn engine_options(base: &EngineOptions, settings: &Settings) -> EngineOptions {
    EngineOptions {
        dht: base.dht && settings.torrent_dht,
        upload: settings.torrent_upload,
        download_limit_bps: settings
            .download_limit
            .bytes_per_second()
            .and_then(NonZeroU32::new),
        upload_limit_bps: settings
            .upload_limit
            .bytes_per_second()
            .and_then(NonZeroU32::new),
        peer_limit: Some(settings.peer_limit.peers()),
        window: NonZeroU64::new(settings.torrent_storage.window_bytes()),
        in_memory: settings.torrent_storage.in_memory(),
        listen_port: u16::try_from(settings.torrent_port.get()).unwrap_or(u16::MAX),
        ..base.clone()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use cineo_core::app::{DownloadLimit, PeerLimit, TorrentPort, TorrentStorage, UploadLimit};

    use super::*;

    #[test]
    fn torrent_settings_shape_the_next_engine() {
        let base = EngineOptions::new(PathBuf::from("/c"));
        let defaults = engine_options(&base, &Settings::default());
        assert_eq!(
            defaults,
            EngineOptions {
                peer_limit: Some(128),
                window: NonZeroU64::new(1024 * 1024 * 1024),
                in_memory: true,
                listen_port: 42713,
                ..base.clone()
            },
            "the defaults match librqbit's"
        );
        let settings = Settings {
            torrent_upload: false,
            torrent_dht: false,
            download_limit: DownloadLimit::M5,
            upload_limit: UploadLimit::K100,
            peer_limit: PeerLimit::P50,
            torrent_storage: TorrentStorage::Disk8192,
            torrent_port: TorrentPort::new(51000),
            ..Settings::default()
        };
        let options = engine_options(&base, &settings);
        assert_eq!(options.listen_port, 51000);
        assert!(!options.upload && !options.dht);
        assert_eq!(options.window, NonZeroU64::new(8 << 30));
        assert!(!options.in_memory);
        assert_eq!(options.download_limit_bps, NonZeroU32::new(5_000_000));
        assert_eq!(options.upload_limit_bps, NonZeroU32::new(100_000));
        assert_eq!(options.peer_limit, Some(50));
    }
}
