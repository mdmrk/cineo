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
    Url(Url),
    YouTube {
        id: String,
    },
    Torrent {
        info_hash: String,
        file_idx: Option<u32>,
        sources: Vec<String>,
    },
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
    /// OpenSubtitles hash of the file (`behaviorHints.videoHash`).
    pub video_hash: Option<String>,
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

pub fn parse_stream_json(value: &Value) -> Option<Stream> {
    parse_stream(value, "stream", &mut Warnings::default())
}

pub fn stream_to_json(stream: &Stream) -> Option<Value> {
    let mut obj = Map::new();
    match &stream.source {
        StreamSource::Url(url) => {
            obj.insert("url".into(), url.as_str().into());
        }
        StreamSource::Torrent {
            info_hash,
            file_idx,
            sources,
        } => {
            obj.insert("infoHash".into(), info_hash.as_str().into());
            if let Some(idx) = file_idx {
                obj.insert("fileIdx".into(), (*idx).into());
            }
            obj.insert("sources".into(), sources.clone().into());
        }
        _ => return None,
    }
    let put = |obj: &mut Map<String, Value>, key: &str, value: &Option<String>| {
        if let Some(value) = value {
            obj.insert(key.into(), value.as_str().into());
        }
    };
    put(&mut obj, "name", &stream.name);
    put(&mut obj, "description", &stream.description);
    let subtitles: Vec<Value> = stream
        .subtitles
        .iter()
        .map(|s| {
            let mut sub = Map::new();
            sub.insert("id".into(), s.id.as_str().into());
            sub.insert("url".into(), s.url.as_str().into());
            sub.insert("lang".into(), s.lang.as_str().into());
            put(&mut sub, "label", &s.label);
            sub.into()
        })
        .collect();
    obj.insert("subtitles".into(), subtitles.into());
    let mut hints = Map::new();
    hints.insert("notWebReady".into(), stream.not_web_ready.into());
    put(&mut hints, "bingeGroup", &stream.binge_group);
    put(&mut hints, "filename", &stream.filename);
    put(&mut hints, "videoHash", &stream.video_hash);
    if let Some(size) = stream.video_size {
        hints.insert("videoSize".into(), size.into());
    }
    if !stream.request_headers.is_empty() {
        let request: Map<String, Value> = stream
            .request_headers
            .iter()
            .map(|(name, value)| (name.clone(), value.as_str().into()))
            .collect();
        let mut proxy = Map::new();
        proxy.insert("request".into(), request.into());
        hints.insert("proxyHeaders".into(), proxy.into());
    }
    obj.insert("behaviorHints".into(), hints.into());
    Some(obj.into())
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
        video_hash: json::opt_string(hints_obj, "videoHash", &hloc, warnings),
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
