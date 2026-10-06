//! Stream and subtitle responses: `{ "streams": [...] }`, `{ "subtitles": [...] }`.

use serde_json::{Map, Value};
use url::Url;

use super::ResponseError;
use super::json::{self, Object, field, kind};
use crate::diagnostics::{Parsed, Warnings};

/// Where a stream's bytes come from. The variant is chosen by which field is
/// present, in reference-client order: `url`, `ytId`, `infoHash`,
/// `externalUrl`, archive lists, `nzbUrl`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamSource {
    /// A direct URL. Only `http(s)` is playable today; others (e.g. `rtmp`,
    /// `magnet`) are kept so the UI can explain why they are not.
    Url(Url),
    YouTube {
        id: String,
    },
    Torrent {
        /// 40 lowercase hex characters.
        info_hash: String,
        file_idx: Option<u32>,
        /// Tracker / DHT hints (`tracker:…`, `dht:…`).
        sources: Vec<String>,
    },
    /// To be opened in a browser, never played.
    External(Url),
    Archive {
        kind: ArchiveKind,
        urls: Vec<Url>,
    },
    Nzb {
        url: Url,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveKind {
    Rar,
    Zip,
    SevenZip,
    Tgz,
    Tar,
}

impl StreamSource {
    /// Whether Cineo can play this source today (see COMPATIBILITY.md):
    /// `http(s)` URLs, and torrents through the streaming engine.
    pub fn is_playable(&self) -> bool {
        match self {
            Self::Url(url) => matches!(url.scheme(), "http" | "https"),
            Self::Torrent { .. } => true,
            _ => false,
        }
    }

    /// Whether playing this source means peer-to-peer traffic.
    pub fn is_p2p(&self) -> bool {
        matches!(self, Self::Torrent { .. })
    }

    /// Short human label for the source kind.
    pub fn kind_label(&self) -> &'static str {
        match self {
            Self::Url(url) if matches!(url.scheme(), "http" | "https") => "HTTP",
            Self::Url(_) => "URL",
            Self::YouTube { .. } => "YouTube",
            Self::Torrent { .. } => "Torrent",
            Self::External(_) => "External",
            Self::Archive { .. } => "Archive",
            Self::Nzb { .. } => "Usenet",
        }
    }
}

/// One stream offered by an addon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stream {
    pub source: StreamSource,
    /// Usually the quality, e.g. `1080p`.
    pub name: Option<String>,
    /// `description`, falling back to the deprecated `title`.
    pub description: Option<String>,
    pub subtitles: Vec<Subtitle>,
    pub not_web_ready: bool,
    pub binge_group: Option<String>,
    pub filename: Option<String>,
    pub video_size: Option<u64>,
    /// Validated `behaviorHints.proxyHeaders.request` entries.
    pub request_headers: Vec<(String, String)>,
}

/// A subtitle track offered by an addon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subtitle {
    pub id: String,
    /// Always `http(s)`.
    pub url: Url,
    /// ISO 639-2 code or free text.
    pub lang: String,
    pub label: Option<String>,
}

/// Parses a stream response. Missing `streams` is an error, `null` is empty,
/// and streams with no recognizable source are skipped with a warning.
pub fn parse_stream_response(bytes: &[u8]) -> Result<Parsed<Vec<Stream>>, ResponseError> {
    parse_list(bytes, "streams", parse_stream)
}

/// Parses a subtitles response (`{ "subtitles": [...] }`).
pub fn parse_subtitles_response(bytes: &[u8]) -> Result<Parsed<Vec<Subtitle>>, ResponseError> {
    parse_list(bytes, "subtitles", parse_subtitle)
}

fn parse_list<T>(
    bytes: &[u8],
    key: &'static str,
    parse_item: fn(&Value, &str, &mut Warnings) -> Option<T>,
) -> Result<Parsed<Vec<T>>, ResponseError> {
    let root: Value = serde_json::from_slice(bytes)?;
    let Value::Object(obj) = root else {
        return Err(ResponseError::NotAnObject);
    };
    let mut warnings = Warnings::default();
    let items = match obj.get(key) {
        None => return Err(ResponseError::MissingField(key)),
        Some(Value::Null) => Vec::new(),
        Some(Value::Array(items)) => items
            .iter()
            .enumerate()
            .filter_map(|(i, item)| parse_item(item, &format!("{key}[{i}]"), &mut warnings))
            .collect(),
        Some(other) => {
            warnings.ignored(key, format!("expected array, got {}", kind(other)));
            Vec::new()
        }
    };
    Ok(warnings.finish(items))
}

fn parse_stream(item: &Value, loc: &str, warnings: &mut Warnings) -> Option<Stream> {
    let Value::Object(obj) = item else {
        warnings.skipped(loc, format!("expected object, got {}", kind(item)));
        return None;
    };
    let Some(source) = parse_source(obj, loc, warnings) else {
        warnings.skipped(loc, "no recognizable stream source");
        return None;
    };
    let hloc = field(loc, "behaviorHints");
    let empty = Map::new();
    let hints_obj = match obj.get("behaviorHints") {
        Some(Value::Object(hints)) => hints,
        _ => &empty,
    };
    let subtitles = match obj.get("subtitles") {
        Some(Value::Array(items)) => items
            .iter()
            .enumerate()
            .filter_map(|(i, s)| parse_subtitle(s, &format!("{loc}.subtitles[{i}]"), warnings))
            .collect(),
        _ => Vec::new(),
    };
    Some(Stream {
        source,
        name: json::opt_string(obj, "name", loc, warnings),
        description: json::opt_string(obj, "description", loc, warnings)
            .or_else(|| json::opt_string(obj, "title", loc, warnings)),
        subtitles,
        not_web_ready: json::bool_or_false(hints_obj, "notWebReady", &hloc, warnings),
        binge_group: json::opt_string(hints_obj, "bingeGroup", &hloc, warnings),
        filename: json::opt_string(hints_obj, "filename", &hloc, warnings),
        video_size: hints_obj.get("videoSize").and_then(Value::as_u64),
        request_headers: parse_request_headers(hints_obj, &hloc, warnings),
    })
}

fn parse_source(obj: &Object, loc: &str, warnings: &mut Warnings) -> Option<StreamSource> {
    if let Some(url) = json::opt_any_url(obj, "url", loc, warnings) {
        return Some(StreamSource::Url(url));
    }
    if let Some(id) = json::opt_string(obj, "ytId", loc, warnings) {
        return Some(StreamSource::YouTube { id });
    }
    if let Some(hash) = json::opt_string(obj, "infoHash", loc, warnings) {
        let hash = hash.to_ascii_lowercase();
        if hash.len() == 40 && hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Some(StreamSource::Torrent {
                info_hash: hash,
                file_idx: json::opt_u32(obj, "fileIdx", loc, warnings),
                sources: json::string_list(obj, "sources", loc, warnings).unwrap_or_default(),
            });
        }
        warnings.ignored(field(loc, "infoHash"), "expected 40 hex characters");
    }
    if let Some(url) = json::opt_http_url(obj, "externalUrl", loc, warnings) {
        return Some(StreamSource::External(url));
    }
    for (key, kind) in [
        ("rarUrls", ArchiveKind::Rar),
        ("zipUrls", ArchiveKind::Zip),
        ("7zipUrls", ArchiveKind::SevenZip),
        ("tgzUrls", ArchiveKind::Tgz),
        ("tarUrls", ArchiveKind::Tar),
    ] {
        if let Some(Value::Array(items)) = obj.get(key) {
            let urls: Vec<Url> = items
                .iter()
                .filter_map(|item| match item {
                    Value::String(s) => Url::parse(s).ok(),
                    Value::Object(o) => o
                        .get("url")
                        .and_then(Value::as_str)
                        .and_then(|s| Url::parse(s).ok()),
                    _ => None,
                })
                .collect();
            if !urls.is_empty() {
                return Some(StreamSource::Archive { kind, urls });
            }
        }
    }
    json::opt_any_url(obj, "nzbUrl", loc, warnings).map(|url| StreamSource::Nzb { url })
}

/// `proxyHeaders.request`: header names must be RFC 7230 tokens and values
/// free of CR, LF and NUL; anything else is dropped with a warning.
fn parse_request_headers(
    hints: &Object,
    loc: &str,
    warnings: &mut Warnings,
) -> Vec<(String, String)> {
    let Some(Value::Object(proxy)) = hints.get("proxyHeaders") else {
        return Vec::new();
    };
    let Some(Value::Object(request)) = proxy.get("request") else {
        return Vec::new();
    };
    let loc = format!("{loc}.proxyHeaders.request");
    request
        .iter()
        .filter_map(|(name, value)| {
            let valid_name = !name.is_empty()
                && name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b));
            match value {
                Value::String(v)
                    if valid_name && !v.bytes().any(|b| matches!(b, b'\r' | b'\n' | 0)) =>
                {
                    Some((name.clone(), v.clone()))
                }
                _ => {
                    warnings.ignored(field(&loc, name), "invalid header name or value");
                    None
                }
            }
        })
        .collect()
}

fn parse_subtitle(item: &Value, loc: &str, warnings: &mut Warnings) -> Option<Subtitle> {
    let Value::Object(obj) = item else {
        warnings.skipped(loc, format!("expected object, got {}", kind(item)));
        return None;
    };
    let Some(url) = json::opt_http_url(obj, "url", loc, warnings) else {
        warnings.skipped(loc, "missing or unusable `url`");
        return None;
    };
    let lang = json::opt_string(obj, "lang", loc, warnings).unwrap_or_else(|| "und".to_owned());
    Some(Subtitle {
        id: json::opt_string(obj, "id", loc, warnings).unwrap_or_else(|| url.to_string()),
        url,
        lang,
        label: json::opt_string(obj, "label", loc, warnings),
    })
}
