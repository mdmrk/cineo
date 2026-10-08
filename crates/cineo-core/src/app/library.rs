//! Local library and watch progress. Time is always passed in (ms since the
//! Unix epoch) so this stays deterministic.

use serde_json::{Map, Value};
use url::Url;

use super::settings::WatchedAt;
use crate::addon::{ContentType, Stream, TransportUrl, parse_stream_json, stream_to_json};

/// Progress for one meta item; the last watched video wins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryItem {
    /// Meta id, e.g. `tt0386676`.
    pub id: String,
    pub content_type: ContentType,
    pub name: String,
    pub poster: Option<Url>,
    /// The video last played, e.g. `tt0386676:1:1`.
    pub video_id: String,
    pub time_offset_ms: u64,
    pub duration_ms: u64,
    pub updated_ms: u64,
    /// The stream `video_id` was last played from.
    pub stream: Option<Box<SavedStream>>,
}

/// A played stream, kept to resume without asking the addons again. The
/// source is always playable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedStream {
    pub addon: TransportUrl,
    pub title: String,
    pub logo: Option<Url>,
    pub background: Option<Url>,
    pub stream: Stream,
}

impl SavedStream {
    pub fn to_json(&self) -> Option<String> {
        let mut obj = Map::new();
        obj.insert("addon".into(), self.addon.as_url().as_str().into());
        obj.insert("title".into(), self.title.as_str().into());
        for (key, url) in [("logo", &self.logo), ("background", &self.background)] {
            if let Some(url) = url {
                obj.insert(key.into(), url.as_str().into());
            }
        }
        obj.insert("stream".into(), stream_to_json(&self.stream)?);
        Some(Value::Object(obj).to_string())
    }

    /// Parses [`Self::to_json`] output; `None` if it is no longer valid.
    pub fn parse(json: &str) -> Option<Self> {
        let value: Value = serde_json::from_str(json).ok()?;
        let url = |key| {
            value
                .get(key)
                .and_then(Value::as_str)
                .and_then(|s| Url::parse(s).ok())
                .filter(|u| matches!(u.scheme(), "http" | "https"))
        };
        let stream = parse_stream_json(value.get("stream")?)?;
        if !stream.source.is_playable() {
            return None;
        }
        Some(Self {
            addon: TransportUrl::parse(value.get("addon")?.as_str()?).ok()?,
            title: value.get("title")?.as_str()?.to_owned(),
            logo: url("logo"),
            background: url("background"),
            stream,
        })
    }
}

impl LibraryItem {
    pub fn progress(&self) -> f32 {
        if self.duration_ms == 0 {
            return 0.0;
        }
        #[expect(clippy::cast_precision_loss, reason = "ms values are far below 2^52")]
        let p = self.time_offset_ms as f64 / self.duration_ms as f64;
        #[expect(clippy::cast_possible_truncation, reason = "clamped to 0..=1")]
        let p = p.clamp(0.0, 1.0) as f32;
        p
    }

    pub fn is_finished(&self, watched_at: WatchedAt) -> bool {
        f64::from(self.progress()) >= watched_at.fraction()
    }

    /// Where to resume `video_id`: the saved offset if it is the same,
    /// unfinished video; otherwise the start.
    pub fn resume_ms(&self, video_id: &str, watched_at: WatchedAt) -> u64 {
        if self.video_id == video_id && !self.is_finished(watched_at) {
            self.time_offset_ms
        } else {
            0
        }
    }
}

/// Unfinished items with progress, most recently watched first.
pub fn continue_watching(items: &[LibraryItem], watched_at: WatchedAt) -> Vec<&LibraryItem> {
    let mut list: Vec<&LibraryItem> = items
        .iter()
        .filter(|i| i.time_offset_ms > 0 && !i.is_finished(watched_at))
        .collect();
    list.sort_by_key(|i| std::cmp::Reverse(i.updated_ms));
    list
}
