use std::collections::HashSet;
use std::fmt::Write as _;
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::num::{NonZeroU32, NonZeroU64};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use cineo_core::app::{TorrentFile, TorrentRequest, TorrentStatus, choose_file};
use librqbit::api::TorrentIdOrHash;
use librqbit::dht::DhtPersistenceConfig;
use librqbit::limits::LimitsConfig;
use librqbit::storage::StorageFactoryExt as _;
use librqbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse, DhtSessionConfig, ListenerMode,
    ListenerOptions, Session, SessionOptions,
};
use tokio::net::TcpListener;
use tracing::{debug, info, warn};
use url::Url;

use crate::memory::MemoryStorageFactory;
use crate::server::{self, Ctx, Served};
use crate::storage::{CacheStorageFactory, torrent_dir};
use crate::{blocklist, cache, socks};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineOptions {
    pub cache_dir: PathBuf,
    pub allow_private_network: bool,
    pub dht: bool,
    pub upload: bool,
    pub download_limit_bps: Option<NonZeroU32>,
    pub upload_limit_bps: Option<NonZeroU32>,
    pub peer_limit: Option<usize>,
    pub extra_peers: Vec<SocketAddr>,
    pub metadata_timeout: Duration,
    pub window: Option<NonZeroU64>,
    pub in_memory: bool,
    pub listen_port: u16,
    pub trackers: Vec<Url>,
}

const SESSION_STOP_GRACE: Duration = Duration::from_millis(50);

const FALLBACK_TRACKERS: &[&str] = &[
    "udp://tracker.opentrackr.org:1337/announce",
    "udp://open.stealth.si:80/announce",
    "udp://open.demonii.com:1337/announce",
    "udp://open.tracker.cl:1337/announce",
    "udp://tracker.qu.ax:6969/announce",
    "udp://tracker.ducks.party:1984/announce",
    "udp://explodie.org:6969/announce",
    "udp://tracker.tryhackx.org:6969/announce",
];

fn fallback_trackers() -> Vec<Url> {
    FALLBACK_TRACKERS
        .iter()
        .filter_map(|t| Url::parse(t).ok())
        .collect()
}

struct SessionPlan<'a> {
    options: &'a EngineOptions,
    proxy_url: &'a str,
    blocklist_url: Option<&'a str>,
}

impl SessionPlan<'_> {
    fn session_options(&self, port: u16) -> SessionOptions {
        let options = self.options;
        let cache_dir = &options.cache_dir;
        let dht = options.dht.then(|| DhtSessionConfig {
            persistence: Some(DhtPersistenceConfig {
                config_filename: Some(cache_dir.join("dht.json")),
                ..DhtPersistenceConfig::default()
            }),
            ..DhtSessionConfig::default()
        });
        SessionOptions {
            dht,
            persistence: None,
            fastresume: false,
            listen: Some(ListenerOptions {
                mode: ListenerMode::TcpAndUtp,
                listen_addr: (Ipv6Addr::UNSPECIFIED, port).into(),
                enable_upnp_port_forwarding: false,
                ..ListenerOptions::default()
            }),
            disable_local_service_discovery: true,
            connect: None,
            http_proxy_url: Some(self.proxy_url.to_owned()),
            blocklist_url: self.blocklist_url.map(str::to_owned),
            trackers: options.trackers.iter().cloned().collect(),
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
        }
    }
}

fn write_blocklist(cache_dir: &Path) -> std::io::Result<Url> {
    let path = cache_dir.join("blocklist.txt");
    std::fs::write(&path, blocklist::p2p_list())?;
    Url::from_file_path(&path).map_err(|()| std::io::Error::other("the cache path is not absolute"))
}

impl EngineOptions {
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
            window: None,
            in_memory: false,
            listen_port: 0,
            trackers: fallback_trackers(),
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

pub struct Engine {
    session: Arc<Session>,
    ctx: Arc<Ctx>,
    options: EngineOptions,
    tasks: [tokio::task::JoinHandle<()>; 2],
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
    pub async fn start(options: EngineOptions) -> Result<Self, StreamError> {
        let cache_dir = options.cache_dir.clone();
        let block = !options.allow_private_network;
        let prepared = tokio::task::spawn_blocking({
            let cache_dir = cache_dir.clone();
            move || {
                cache::create_private_dir(&cache_dir)?;
                cache::remove_all(&cache_dir)?;
                block.then(|| write_blocklist(&cache_dir)).transpose()
            }
        })
        .await
        .map_err(|err| StreamError::Cache(std::io::Error::other(err)))?;
        let blocklist_url = prepared.map_err(StreamError::Cache)?;
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
        let plan = SessionPlan {
            options: &options,
            proxy_url: &proxy_url,
            blocklist_url: blocklist_url.as_ref().map(Url::as_str),
        };
        let session = match Session::new_with_opts(
            cache_dir.clone(),
            plan.session_options(options.listen_port),
        )
        .await
        {
            Ok(session) => Ok(session),
            Err(err) if options.listen_port != 0 => {
                warn!(
                    port = options.listen_port,
                    error = %format!("{err:#}"),
                    "the torrent port is not usable, listening on a random one"
                );
                Session::new_with_opts(cache_dir.clone(), plan.session_options(0)).await
            }
            Err(err) => Err(err),
        }
        .map_err(|err| {
            proxy.abort();
            StreamError::Start(format!("{err:#}"))
        })?;
        info!(port = ?session.listen_addr().map(|a| a.port()), "torrent engine listening");

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
        })
    }

    pub fn options(&self) -> &EngineOptions {
        &self.options
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.ctx.addr
    }

    pub async fn open(&self, request: &TorrentRequest) -> Result<Url, StreamError> {
        self.stop().await;
        let opened = Instant::now();
        let hash = request.info_hash.to_ascii_lowercase();
        let magnet = magnet(&hash, &request.trackers, self.options.allow_private_network);
        let paused = self.options.window.is_none();
        let add = AddTorrentOptions {
            paused,
            overwrite: true,
            output_folder: Some(self.options.cache_dir.to_string_lossy().into_owned()),
            initial_peers: (!self.options.extra_peers.is_empty())
                .then(|| self.options.extra_peers.clone()),
            storage_factory: self.options.in_memory.then(|| MemoryStorageFactory.boxed()),
            stream_window: self.options.window.map(NonZeroU64::get),
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
            debug!(error = %format!("{err:#}"), "adding the torrent failed");
            StreamError::Add("no usable metadata".into())
        })?;
        let torrent = match added {
            AddTorrentResponse::Added(_, torrent)
            | AddTorrentResponse::AlreadyManaged(_, torrent) => torrent,
            AddTorrentResponse::ListOnly(_) => {
                return Err(StreamError::Add("unexpected list-only answer".into()));
            }
        };
        info!(info_hash = %hash, ms = elapsed_ms(opened), "torrent metadata received");
        tokio::time::timeout(wait, torrent.wait_until_initialized())
            .await
            .map_err(|_| StreamError::MetadataTimeout)?
            .map_err(add_failed)?;
        info!(info_hash = %hash, ms = elapsed_ms(opened), "torrent initialized");

        let mut files: Vec<(String, u64)> = torrent
            .with_metadata(|m| {
                m.file_infos
                    .iter()
                    .map(|f| (f.relative_filename.to_string_lossy().into_owned(), f.len))
                    .collect()
            })
            .map_err(add_failed)?;
        let views: Vec<TorrentFile<'_>> = files
            .iter()
            .map(|(path, len)| TorrentFile { path, len: *len })
            .collect();
        let index = choose_file(&views, request.file_idx, request.filename.as_deref())
            .ok_or(StreamError::NoFiles)?;
        let (name, len) = files.swap_remove(index);

        self.session
            .update_only_files(&torrent, &HashSet::from([index]))
            .await
            .map_err(add_failed)?;
        if paused {
            self.session.unpause(&torrent).await.map_err(add_failed)?;
        }

        if let Ok(mut current) = self.ctx.current.write() {
            *current = Some(Arc::new(Served {
                info_hash: hash.clone(),
                file: index,
                len,
                name,
                torrent,
                opened,
                first_bytes: std::sync::Once::new(),
            }));
        }
        info!(
            info_hash = %hash,
            file = index,
            bytes = len,
            ms = elapsed_ms(opened),
            "serving a torrent"
        );
        Url::parse(&format!(
            "http://{}/{}/{hash}/{index}",
            self.ctx.addr, self.ctx.token
        ))
        .map_err(|err| StreamError::Start(err.to_string()))
    }

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

    pub fn set_lookahead(&self, info_hash: &str, bytes: u64) {
        let Some(served) = self.ctx.current.read().ok().and_then(|c| c.clone()) else {
            return;
        };
        if served.info_hash != info_hash {
            return;
        }
        match served.torrent.set_stream_lookahead(Some(bytes)) {
            Ok(()) => debug!(bytes, "torrent lookahead set"),
            Err(err) => debug!(error = %format!("{err:#}"), "setting the lookahead failed"),
        }
    }

    pub async fn stop(&self) {
        if let Ok(mut current) = self.ctx.current.write() {
            *current = None;
        }
        let torrents: Vec<(usize, String)> = self.session.with_torrents(|all| {
            all.map(|(id, torrent)| (id, torrent.info_hash().as_string()))
                .collect()
        });
        for (id, hash) in torrents {
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
    }

    pub async fn shutdown(self) {
        self.stop().await;
        let _ = tokio::time::timeout(SESSION_STOP_GRACE, self.session.stop()).await;
        for task in &self.tasks {
            task.abort();
        }
    }
}

pub(crate) fn elapsed_ms(since: Instant) -> u64 {
    u64::try_from(since.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn add_failed(err: impl std::fmt::Display) -> StreamError {
    StreamError::Add(format!("{err:#}"))
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
    fn the_fallback_trackers_are_udp_trackers_with_a_host_name() {
        let trackers = fallback_trackers();
        assert_eq!(trackers.len(), FALLBACK_TRACKERS.len());
        for tracker in &trackers {
            assert_eq!(tracker.scheme(), "udp", "{tracker}");
            assert!(
                matches!(tracker.host(), Some(url::Host::Domain(_))),
                "{tracker}"
            );
            assert!(tracker.port().is_some(), "{tracker}");
        }
        assert_eq!(EngineOptions::new(PathBuf::from("/tmp")).trackers, trackers);
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
