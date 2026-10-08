use std::collections::HashSet;
use std::fmt::Write as _;
use std::net::{IpAddr, SocketAddr};
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use cineo_core::app::{TorrentFile, TorrentRequest, TorrentStatus, choose_file};
use librqbit::api::TorrentIdOrHash;
use librqbit::dht::DhtPersistenceConfig;
use librqbit::limits::LimitsConfig;
use librqbit::storage::StorageFactoryExt as _;
use librqbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse, ConnectionOptions, DhtSessionConfig,
    Session, SessionOptions,
};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};
use url::Url;

use crate::server::{self, Ctx, Served};
use crate::storage::{CacheStorageFactory, torrent_dir};
use crate::{blocklist, cache, socks};

/// How the engine runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineOptions {
    /// Holds the data of the torrent being played and DHT state. Created
    /// private to the user. Torrent data is deleted when the torrent stops
    /// and, if a run did not stop cleanly, when the engine starts.
    pub cache_dir: PathBuf,
    /// Allow peers and trackers on loopback/private addresses (the same
    /// user choice as `NetPolicy`'s private networks).
    pub allow_private_network: bool,
    /// Find peers through the DHT.
    pub dht: bool,
    pub upload: bool,
    pub download_limit_bps: Option<NonZeroU32>,
    pub upload_limit_bps: Option<NonZeroU32>,
    pub peer_limit: Option<usize>,
    /// Peers to contact for every torrent, e.g. a local seeder in tests.
    pub extra_peers: Vec<SocketAddr>,
    /// How long to wait for a torrent's file list.
    pub metadata_timeout: Duration,
}

const SESSION_STOP_GRACE: Duration = Duration::from_millis(50);

impl EngineOptions {
    /// Defaults: private networks blocked, DHT and upload on, no rate
    /// limits, 60 s metadata timeout.
    pub fn new(cache_dir: PathBuf) -> Self {
        Self {
            cache_dir,
            allow_private_network: false,
            dht: true,
            upload: true,
            download_limit_bps: None,
            upload_limit_bps: None,
            peer_limit: None,
            extra_peers: Vec::new(),
            metadata_timeout: Duration::from_secs(60),
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum StreamError {
    #[error("cannot prepare the torrent cache: {0}")]
    Cache(#[source] std::io::Error),
    #[error("cannot start the torrent engine: {0}")]
    Start(String),
    #[error("cannot start the local stream server: {0}")]
    Server(#[source] std::io::Error),
    #[error("no peers sent the torrent's file list in time")]
    MetadataTimeout,
    #[error("the torrent could not be added: {0}")]
    Add(String),
    #[error("the torrent has no files")]
    NoFiles,
}

/// A running engine. One torrent is served at a time.
pub struct Engine {
    session: Arc<Session>,
    ctx: Arc<Ctx>,
    options: EngineOptions,
    tasks: [tokio::task::JoinHandle<()>; 2],
    active: Mutex<Option<(usize, String)>>,
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("addr", &self.ctx.addr)
            .finish_non_exhaustive()
    }
}

fn random_token() -> Result<String, StreamError> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|err| StreamError::Start(err.to_string()))?;
    Ok(bytes.iter().fold(String::with_capacity(32), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    }))
}

impl Engine {
    /// Prepares the cache, binds the server and starts the session. Must
    /// run inside a tokio runtime.
    pub async fn start(options: EngineOptions) -> Result<Self, StreamError> {
        let cache_dir = options.cache_dir.clone();
        let prepared = tokio::task::spawn_blocking({
            let cache_dir = cache_dir.clone();
            move || {
                cache::create_private_dir(&cache_dir)?;
                cache::remove_all(&cache_dir)
            }
        })
        .await
        .map_err(|err| StreamError::Cache(std::io::Error::other(err)))?;
        prepared.map_err(StreamError::Cache)?;
        let proxy = TcpListener::bind((IpAddr::from([127, 0, 0, 1]), 0))
            .await
            .map_err(StreamError::Server)?;
        let proxy_addr = proxy.local_addr().map_err(StreamError::Server)?;
        let policy = Arc::new(socks::Policy {
            user: "cineo".into(),
            password: random_token()?,
            allow_private_network: options.allow_private_network,
        });
        let proxy_url = format!("socks5://{}:{}@{proxy_addr}", policy.user, policy.password);
        let proxy = tokio::spawn(socks::run(proxy, policy));
        let dht = options.dht.then(|| DhtSessionConfig {
            persistence: Some(DhtPersistenceConfig {
                config_filename: Some(cache_dir.join("dht.json")),
                ..DhtPersistenceConfig::default()
            }),
            ..DhtSessionConfig::default()
        });
        let session = Session::new_with_opts(
            cache_dir.clone(),
            SessionOptions {
                dht,
                persistence: None,
                fastresume: false,
                listen: None,
                disable_local_service_discovery: true,
                connect: Some(ConnectionOptions {
                    proxy_url: Some(proxy_url),
                    ..ConnectionOptions::default()
                }),
                default_storage_factory: Some(
                    CacheStorageFactory {
                        cache: cache_dir.clone(),
                    }
                    .boxed(),
                ),
                disable_upload: !options.upload,
                ratelimits: LimitsConfig {
                    download_bps: options.download_limit_bps,
                    upload_bps: options.upload_limit_bps,
                },
                peer_limit: options.peer_limit,
                ..SessionOptions::default()
            },
        )
        .await
        .map_err(|err| {
            proxy.abort();
            StreamError::Start(format!("{err:#}"))
        })?;

        let listener = TcpListener::bind((IpAddr::from([127, 0, 0, 1]), 0))
            .await
            .map_err(StreamError::Server)?;
        let addr = listener.local_addr().map_err(StreamError::Server)?;
        let ctx = Arc::new(Ctx {
            token: random_token()?,
            addr,
            current: Arc::new(RwLock::new(None)),
        });
        let server = tokio::spawn(server::run(listener, Arc::clone(&ctx)));
        info!(%addr, "torrent engine started");
        Ok(Self {
            session,
            ctx,
            options,
            tasks: [server, proxy],
            active: Mutex::new(None),
        })
    }

    pub fn options(&self) -> &EngineOptions {
        &self.options
    }

    /// The server's address (always loopback).
    pub fn local_addr(&self) -> SocketAddr {
        self.ctx.addr
    }

    /// Starts serving the torrent of `request`, replacing any other, and
    /// returns the URL for the player.
    pub async fn open(&self, request: &TorrentRequest) -> Result<Url, StreamError> {
        self.stop().await;
        let hash = request.info_hash.to_ascii_lowercase();
        let magnet = magnet(&hash, &request.trackers, self.options.allow_private_network);
        let add = AddTorrentOptions {
            paused: true,
            overwrite: true,
            output_folder: Some(self.options.cache_dir.to_string_lossy().into_owned()),
            initial_peers: (!self.options.extra_peers.is_empty())
                .then(|| self.options.extra_peers.clone()),
            ..AddTorrentOptions::default()
        };
        let wait = self.options.metadata_timeout;
        let added = tokio::time::timeout(
            wait,
            self.session
                .add_torrent(AddTorrent::from_url(magnet), Some(add)),
        )
        .await
        .map_err(|_| StreamError::MetadataTimeout)?
        .map_err(|err| {
            // librqbit's message may name trackers; keep it out of the UI.
            debug!(error = %format!("{err:#}"), "adding the torrent failed");
            StreamError::Add("no usable metadata".into())
        })?;
        let (id, torrent) = match added {
            AddTorrentResponse::Added(id, torrent)
            | AddTorrentResponse::AlreadyManaged(id, torrent) => (id, torrent),
            AddTorrentResponse::ListOnly(_) => {
                return Err(StreamError::Add("unexpected list-only answer".into()));
            }
        };
        *self.active.lock().await = Some((id, hash.clone()));
        tokio::time::timeout(wait, torrent.wait_until_initialized())
            .await
            .map_err(|_| StreamError::MetadataTimeout)?
            .map_err(|err| StreamError::Add(format!("{err:#}")))?;

        let files: Vec<(String, u64)> = torrent
            .with_metadata(|m| {
                m.file_infos
                    .iter()
                    .map(|f| (f.relative_filename.to_string_lossy().into_owned(), f.len))
                    .collect()
            })
            .map_err(|err| StreamError::Add(format!("{err:#}")))?;
        let views: Vec<TorrentFile<'_>> = files
            .iter()
            .map(|(path, len)| TorrentFile { path, len: *len })
            .collect();
        let index = choose_file(&views, request.file_idx, request.filename.as_deref())
            .ok_or(StreamError::NoFiles)?;
        let (name, len) = files[index].clone();

        self.session
            .update_only_files(&torrent, &HashSet::from([index]))
            .await
            .map_err(|err| StreamError::Add(format!("{err:#}")))?;
        self.session
            .unpause(&torrent)
            .await
            .map_err(|err| StreamError::Add(format!("{err:#}")))?;

        if let Ok(mut current) = self.ctx.current.write() {
            *current = Some(Arc::new(Served {
                info_hash: hash.clone(),
                file: index,
                len,
                name,
                torrent,
            }));
        }
        info!(info_hash = %hash, file = index, bytes = len, "serving a torrent");
        Url::parse(&format!(
            "http://{}/{}/{hash}/{index}",
            self.ctx.addr, self.ctx.token
        ))
        .map_err(|err| StreamError::Start(err.to_string()))
    }

    /// The status of the torrent being served, if any.
    pub fn status(&self) -> Option<(String, TorrentStatus)> {
        let served = self.ctx.current.read().ok()?.clone()?;
        let stats = served.torrent.stats();
        let status = match stats.live {
            None => TorrentStatus::Starting,
            Some(live) => TorrentStatus::Streaming {
                peers: live.snapshot.peer_stats.live,
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "a non-negative speed far below u64::MAX"
                )]
                download_bytes_per_sec: (live.download_speed.mbps * 1024.0 * 1024.0).max(0.0)
                    as u64,
                downloaded: stats
                    .file_progress
                    .get(served.file)
                    .copied()
                    .unwrap_or_default(),
                size: served.len,
            },
        };
        Some((served.info_hash.clone(), status))
    }

    /// Stops the current torrent and deletes its data.
    pub async fn stop(&self) {
        if let Ok(mut current) = self.ctx.current.write() {
            *current = None;
        }
        let Some((id, hash)) = self.active.lock().await.take() else {
            return;
        };
        if let Err(err) = self.session.delete(TorrentIdOrHash::Id(id), false).await {
            warn!(error = %format!("{err:#}"), "stopping the torrent failed");
        }
        let dir = torrent_dir(&self.options.cache_dir, &hash);
        let removed = tokio::task::spawn_blocking(move || cache::remove_torrent(&dir))
            .await
            .map_err(std::io::Error::other)
            .and_then(|result| result);
        if let Err(err) = removed {
            warn!(%err, "deleting the torrent data failed");
        }
    }

    /// Stops everything: the torrent, the session and the server. Returns
    /// without librqbit's 1 s grace period.
    pub async fn shutdown(self) {
        self.stop().await;
        // `Session::stop` sleeps a fixed second after cancelling; nothing needs that at exit.
        let _ = tokio::time::timeout(SESSION_STOP_GRACE, self.session.stop()).await;
        for task in &self.tasks {
            task.abort();
        }
    }
}

fn magnet(hash: &str, trackers: &[Url], allow_private_network: bool) -> String {
    let mut out = format!("magnet:?xt=urn:btih:{hash}");
    for tracker in trackers {
        let blocked = match tracker.host() {
            Some(url::Host::Ipv4(ip)) => blocklist::is_blocked(ip.into()),
            Some(url::Host::Ipv6(ip)) => blocklist::is_blocked(ip.into()),
            Some(url::Host::Domain(_)) => false,
            None => true,
        };
        if blocked && !allow_private_network {
            continue;
        }
        out.push_str("&tr=");
        out.extend(url::form_urlencoded::byte_serialize(
            tracker.as_str().as_bytes(),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn urls(list: &[&str]) -> Vec<Url> {
        list.iter()
            .map(|s| Url::parse(s).unwrap_or_else(|e| panic!("{s}: {e}")))
            .collect()
    }

    #[test]
    fn trackers_on_non_public_addresses_are_dropped_by_default() {
        let trackers = urls(&[
            "udp://tracker.example:1337/announce",
            "http://192.168.1.1:80/announce",
            "http://[::1]:80/announce",
            "https://8.8.8.8/announce",
        ]);
        let hash = "a".repeat(40);
        assert_eq!(
            magnet(&hash, &trackers, false),
            format!(
                "magnet:?xt=urn:btih:{hash}\
                 &tr=udp%3A%2F%2Ftracker.example%3A1337%2Fannounce\
                 &tr=https%3A%2F%2F8.8.8.8%2Fannounce"
            )
        );
        assert_eq!(magnet(&hash, &trackers, true).matches("&tr=").count(), 4);
    }
}
