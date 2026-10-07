//! Local library and watch progress. Time is always passed in (ms since the
//! Unix epoch) so this stays deterministic.

use url::Url;

use crate::addon::ContentType;

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
}

const FINISHED: f64 = 0.92;

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

    pub fn is_finished(&self) -> bool {
        f64::from(self.progress()) >= FINISHED
    }

    /// Where to resume `video_id`: the saved offset if it is the same,
    /// unfinished video; otherwise the start.
    pub fn resume_ms(&self, video_id: &str) -> u64 {
        if self.video_id == video_id && !self.is_finished() {
            self.time_offset_ms
        } else {
            0
        }
    }
}

/// Unfinished items with progress, most recently watched first.
pub fn continue_watching(items: &[LibraryItem]) -> Vec<&LibraryItem> {
    let mut list: Vec<&LibraryItem> = items
        .iter()
        .filter(|i| i.time_offset_ms > 0 && !i.is_finished())
        .collect();
    list.sort_by_key(|i| std::cmp::Reverse(i.updated_ms));
    list
}
