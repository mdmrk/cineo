use serde_json::Value;
use url::Url;

use super::ResponseError;
use super::catalog::{MetaPreview, parse_meta_preview};
use super::json;
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
    /// Distinct season numbers in ascending order; season 0 (specials) last.
    pub fn seasons(&self) -> Vec<u32> {
        let mut seasons: Vec<u32> = self.videos.iter().filter_map(|v| v.season).collect();
        seasons.sort_unstable_by_key(|s| (*s == 0, *s));
        seasons.dedup();
        seasons
    }

    pub fn next_video(&self, video_id: &str, now_ms: u64) -> Option<&Video> {
        let position = self.videos.iter().position(|v| v.id == video_id)?;
        let next = self.videos.get(position + 1)?;
        let season = |v: &Video| v.season.unwrap_or(0);
        let current = season(&self.videos[position]);
        let released = next
            .released
            .as_deref()
            .and_then(epoch_ms)
            .is_none_or(|at| at <= i64::try_from(now_ms).unwrap_or(i64::MAX));
        ((season(next) != 0 || current == 0) && released).then_some(next)
    }
}

fn epoch_ms(text: &str) -> Option<i64> {
    let num = |at: usize| text.get(at..at + 2)?.parse::<i64>().ok();
    let year: i64 = text.get(..4)?.parse().ok()?;
    let (month, day) = (num(5)?, num(8)?);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let seconds = match text.as_bytes().get(10) {
        Some(b'T' | b' ') => num(11)? * 3600 + num(14)? * 60 + num(17).unwrap_or(0),
        _ => 0,
    };
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let days = era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468;
    Some((days * 86_400 + seconds) * 1000)
}

/// Parses a meta response. Fails if there is no `meta` object or it lacks a
/// valid `id`/`type`. Videos are sorted by season, then episode; invalid
/// videos are skipped with a warning.
pub fn parse_meta_response(bytes: &[u8]) -> Result<Parsed<Meta>, ResponseError> {
    let obj = json::root(bytes)?;
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

    let mut videos = json::list(meta, "videos", "meta", &mut warnings, parse_video);
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
    let obj = json::object(item, loc, warnings)?;
    let id = json::id(obj, loc, warnings)?;
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
