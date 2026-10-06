//! Meta responses: `{ "meta": Meta }`.

use serde_json::Value;
use url::Url;

use super::ResponseError;
use super::catalog::{MetaPreview, parse_meta_preview};
use super::json::{self, kind};
use crate::diagnostics::{Parsed, Warnings};

/// Detailed metadata for one item.
#[derive(Debug, Clone, PartialEq)]
pub struct Meta {
    /// The fields shared with catalog entries.
    pub preview: MetaPreview,
    pub runtime: Option<String>,
    pub cast: Vec<String>,
    pub director: Vec<String>,
    /// Episodes / uploads. Empty for single-video items (movies), whose only
    /// video id is the meta id.
    pub videos: Vec<Video>,
    /// `behaviorHints.defaultVideoId`: open this video's streams directly.
    pub default_video_id: Option<String>,
}

/// One playable video of a meta item (an episode, an upload, ...).
#[derive(Debug, Clone, PartialEq)]
pub struct Video {
    /// Non-empty, e.g. `tt0386676:2:1`.
    pub id: String,
    /// `title`, or `name` (reference client alias). May be empty.
    pub title: String,
    /// ISO 8601 as sent by the addon; not interpreted yet.
    pub released: Option<String>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub thumbnail: Option<Url>,
    /// `overview`, falling back to `description`.
    pub overview: Option<String>,
}

impl Meta {
    /// The ids to request streams for: the videos, or the meta id itself.
    pub fn video_ids(&self) -> Vec<&str> {
        if self.videos.is_empty() {
            vec![self.preview.id.as_str()]
        } else {
            self.videos.iter().map(|v| v.id.as_str()).collect()
        }
    }

    /// Distinct season numbers in ascending order; season 0 (specials) last.
    pub fn seasons(&self) -> Vec<u32> {
        let mut seasons: Vec<u32> = self.videos.iter().filter_map(|v| v.season).collect();
        seasons.sort_unstable_by_key(|s| (*s == 0, *s));
        seasons.dedup();
        seasons
    }
}

/// Parses a meta response. Fails if there is no `meta` object or it lacks a
/// valid `id`/`type`. Videos are sorted by season, then episode; invalid
/// videos are skipped with a warning.
pub fn parse_meta_response(bytes: &[u8]) -> Result<Parsed<Meta>, ResponseError> {
    let root: Value = serde_json::from_slice(bytes)?;
    let Value::Object(obj) = root else {
        return Err(ResponseError::NotAnObject);
    };
    let Some(meta) = obj.get("meta").filter(|m| !m.is_null()) else {
        return Err(ResponseError::MissingField("meta"));
    };
    let mut warnings = Warnings::default();
    let Some(preview) = parse_meta_preview(meta, "meta", &mut warnings) else {
        return Err(ResponseError::MissingField("meta.id/meta.type"));
    };
    let Value::Object(meta) = meta else {
        return Err(ResponseError::NotAnObject);
    };

    let mut videos: Vec<Video> = match meta.get("videos") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(items)) => items
            .iter()
            .enumerate()
            .filter_map(|(i, item)| parse_video(item, &format!("meta.videos[{i}]"), &mut warnings))
            .collect(),
        Some(other) => {
            warnings.ignored(
                "meta.videos",
                format!("expected array, got {}", kind(other)),
            );
            Vec::new()
        }
    };
    videos.sort_by_key(|v| (v.season, v.episode));

    let default_video_id = match meta.get("behaviorHints") {
        Some(Value::Object(hints)) => {
            json::opt_string(hints, "defaultVideoId", "meta.behaviorHints", &mut warnings)
        }
        _ => None,
    };

    let value = Meta {
        runtime: json::opt_string_or_number(meta, "runtime", "meta", &mut warnings),
        cast: json::string_list(meta, "cast", "meta", &mut warnings).unwrap_or_default(),
        director: json::string_list(meta, "director", "meta", &mut warnings).unwrap_or_default(),
        videos,
        default_video_id,
        preview,
    };
    Ok(warnings.finish(value))
}

fn parse_video(item: &Value, loc: &str, warnings: &mut Warnings) -> Option<Video> {
    let Value::Object(obj) = item else {
        warnings.skipped(loc, format!("expected object, got {}", kind(item)));
        return None;
    };
    let id = match obj.get("id") {
        Some(Value::String(id)) if !id.is_empty() => id.clone(),
        _ => {
            warnings.skipped(loc, "missing or empty `id`");
            return None;
        }
    };
    let title = json::opt_string(obj, "title", loc, warnings)
        .or_else(|| json::opt_string(obj, "name", loc, warnings))
        .unwrap_or_default();
    let overview = json::opt_string(obj, "overview", loc, warnings)
        .or_else(|| json::opt_string(obj, "description", loc, warnings));
    Some(Video {
        id,
        title,
        released: json::opt_string(obj, "released", loc, warnings),
        season: json::opt_u32(obj, "season", loc, warnings),
        episode: json::opt_u32(obj, "episode", loc, warnings),
        thumbnail: json::opt_http_url(obj, "thumbnail", loc, warnings),
        overview,
    })
}
