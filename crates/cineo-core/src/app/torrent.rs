//! Torrent playback decisions (ADR-0010, ADR-0012): what the engine is asked
//! to serve and which file of a torrent to play. The engine (`cineo-stream`)
//! does the IO.

use url::Url;

use super::state::PlayRequest;
use crate::addon::{Stream, StreamSource};

/// A torrent the engine should serve, built from an addon's stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TorrentRequest {
    /// 40 lowercase hex characters (checked when the stream was parsed).
    pub info_hash: String,
    /// The addon's `fileIdx`.
    pub file_idx: Option<u32>,
    /// The addon's `behaviorHints.filename`.
    pub filename: Option<String>,
    /// `http(s)` and `udp` trackers from the stream's `sources`.
    pub trackers: Vec<Url>,
}

impl TorrentRequest {
    /// The request for a torrent stream; `None` for other sources.
    pub fn from_stream(stream: &Stream) -> Option<Self> {
        let StreamSource::Torrent {
            info_hash,
            file_idx,
            sources,
        } = &stream.source
        else {
            return None;
        };
        Some(Self {
            info_hash: info_hash.clone(),
            file_idx: *file_idx,
            filename: stream.filename.clone(),
            trackers: trackers(sources),
        })
    }

    /// `magnet:?xt=urn:btih:<hash>`, naming the torrent until the engine
    /// serves it.
    pub fn magnet(&self) -> Url {
        let raw = format!("magnet:?xt=urn:btih:{}", self.info_hash);
        Url::parse(&raw).unwrap_or_else(|_| unreachable_magnet())
    }
}

#[cold]
fn unreachable_magnet() -> Url {
    // A fixed, valid URL; only reached if the info hash invariant is broken.
    #[allow(clippy::unwrap_used)] // a literal that parses
    Url::parse("magnet:?xt=urn:btih:").unwrap()
}

fn trackers(sources: &[String]) -> Vec<Url> {
    let mut out: Vec<Url> = Vec::new();
    for source in sources {
        let Some(raw) = source.strip_prefix("tracker:") else {
            continue;
        };
        let Ok(url) = Url::parse(raw) else {
            continue;
        };
        if matches!(url.scheme(), "http" | "https" | "udp")
            && url.host().is_some()
            && !out.contains(&url)
        {
            out.push(url);
        }
    }
    out
}

/// One file of a torrent, as the engine sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TorrentFile<'a> {
    /// Path inside the torrent, `/`-separated. Display only.
    pub path: &'a str,
    pub len: u64,
}

const VIDEO_EXTENSIONS: &[&str] = &[
    "mkv", "mp4", "m4v", "avi", "mov", "webm", "wmv", "mpg", "mpeg", "ts", "m2ts", "flv", "ogv",
];

/// Picks the file to play: `fileIdx` if it names a file; else the file whose
/// name equals the `filename` hint (ignoring case and folders); else the
/// largest video file; else the largest file. `None` for an empty torrent.
pub fn choose_file(
    files: &[TorrentFile<'_>],
    file_idx: Option<u32>,
    filename: Option<&str>,
) -> Option<usize> {
    if let Some(index) = file_idx.and_then(|i| usize::try_from(i).ok())
        && index < files.len()
    {
        return Some(index);
    }
    if let Some(wanted) = filename
        && let Some(index) = files
            .iter()
            .position(|f| base_name(f.path).eq_ignore_ascii_case(base_name(wanted)))
    {
        return Some(index);
    }
    let largest = |videos_only: bool| {
        files
            .iter()
            .enumerate()
            .filter(|(_, f)| !videos_only || is_video(f.path))
            .max_by_key(|(i, f)| (f.len, std::cmp::Reverse(*i)))
            .map(|(i, _)| i)
    };
    largest(true).or_else(|| largest(false))
}

fn base_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn is_video(path: &str) -> bool {
    base_name(path)
        .rsplit_once('.')
        .is_some_and(|(_, ext)| VIDEO_EXTENSIONS.iter().any(|v| ext.eq_ignore_ascii_case(v)))
}

/// What the engine reports about the torrent being streamed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TorrentStatus {
    Starting,
    Streaming {
        peers: u32,
        download_bytes_per_sec: u64,
        downloaded: u64,
        size: u64,
    },
}

/// The torrent being played, from the Play click until playback stops.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TorrentPlayback {
    pub info_hash: String,
    pub status: TorrentStatus,
    pub(crate) pending: Option<PlayRequest>,
}

impl TorrentPlayback {
    /// The playback the engine is still preparing the file for; `None`
    /// once it has been handed to the player.
    pub fn connecting(&self) -> Option<&PlayRequest> {
        self.pending.as_ref()
    }
}

pub(crate) fn is_engine_url(url: &Url) -> bool {
    url.scheme() == "http"
        && match url.host() {
            Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
            Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
            _ => false,
        }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files<'a>(list: &[(&'a str, u64)]) -> Vec<TorrentFile<'a>> {
        list.iter()
            .map(|&(path, len)| TorrentFile { path, len })
            .collect()
    }

    #[test]
    fn file_idx_wins_when_it_names_a_file() {
        let f = files(&[("a.mkv", 10), ("b.mkv", 99)]);
        assert_eq!(choose_file(&f, Some(0), Some("b.mkv")), Some(0));
    }

    #[test]
    fn an_out_of_range_file_idx_falls_back_to_the_filename_hint() {
        let f = files(&[("Show/S01E01.mkv", 10), ("Show/S01E02.MKV", 9)]);
        assert_eq!(choose_file(&f, Some(7), Some("s01e02.mkv")), Some(1));
    }

    #[test]
    fn without_hints_the_largest_video_is_chosen_over_larger_other_files() {
        let f = files(&[("sample.mkv", 5), ("extras.zip", 500), ("movie.mp4", 50)]);
        assert_eq!(choose_file(&f, None, None), Some(2));
    }

    #[test]
    fn without_videos_the_largest_file_is_chosen() {
        let f = files(&[("a.bin", 5), ("b.bin", 6)]);
        assert_eq!(choose_file(&f, None, Some("missing.mkv")), Some(1));
        assert_eq!(choose_file(&[], None, None), None);
    }

    #[test]
    fn only_http_and_udp_trackers_are_kept() {
        let sources = [
            "tracker:udp://tracker.example:1337/announce".to_owned(),
            "tracker:https://t.example/announce".to_owned(),
            "tracker:https://t.example/announce".to_owned(),
            "tracker:wss://t.example".to_owned(),
            "tracker:not a url".to_owned(),
            "dht:0123456789abcdef0123456789abcdef01234567".to_owned(),
            "udp://no-prefix.example:1/announce".to_owned(),
        ];
        let kept: Vec<String> = trackers(&sources).iter().map(Url::to_string).collect();
        assert_eq!(
            kept,
            [
                "udp://tracker.example:1337/announce",
                "https://t.example/announce"
            ]
        );
    }

    #[test]
    fn only_loopback_http_urls_count_as_engine_urls() {
        let ok = |s: &str| is_engine_url(&Url::parse(s).unwrap_or_else(|e| panic!("{s}: {e}")));
        assert!(ok("http://127.0.0.1:1234/token/abc"));
        assert!(ok("http://[::1]:1234/token/abc"));
        assert!(!ok("https://127.0.0.1/x"));
        assert!(!ok("http://localhost:1234/x"));
        assert!(!ok("http://192.168.1.2:1234/x"));
        assert!(!ok("http://example.com/x"));
    }
}
