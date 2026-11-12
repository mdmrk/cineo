use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use serde_json::Value as Json;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

use super::ffi::{Core, Event, Value};
use super::{
    BUFFERING, CACHE_DURATION, CACHE_SPEED, CACHE_STATE, DURATION, FRAME_DROPS, HEIGHT,
    HWDEC_CURRENT, MUTE, PAUSE, SEEKING, Stats, Status, TIME_POS, TRACK_LIST, Track, TrackKind,
    VIDEO_BITRATE, VOLUME, WIDTH,
};
use crate::PlayerEvent;
use crate::tracker::{EndReason, Input, Tracker};

pub(super) fn parse_cached_ranges(json: &str) -> Vec<(f64, f64)> {
    let Ok(state) = serde_json::from_str::<Json>(json) else {
        return Vec::new();
    };
    let Some(ranges) = state.get("seekable-ranges").and_then(Json::as_array) else {
        return Vec::new();
    };
    ranges
        .iter()
        .filter_map(|range| {
            let start = range.get("start")?.as_f64()?;
            let end = range.get("end")?.as_f64()?;
            (start.is_finite() && end.is_finite() && end > start).then_some((start.max(0.0), end))
        })
        .collect()
}

pub(super) fn parse_tracks(json: &str) -> Vec<Track> {
    let Ok(Json::Array(items)) = serde_json::from_str::<Json>(json) else {
        return Vec::new();
    };
    let text = |item: &Json, key: &str| {
        item.get(key)
            .and_then(Json::as_str)
            .filter(|s| !s.trim().is_empty())
            .map(str::to_owned)
    };
    items
        .iter()
        .filter_map(|item| {
            let kind = match item.get("type")?.as_str()? {
                "video" => TrackKind::Video,
                "audio" => TrackKind::Audio,
                "sub" => TrackKind::Subtitle,
                _ => return None,
            };
            Some(Track {
                id: item.get("id")?.as_i64()?,
                kind,
                title: text(item, "title"),
                lang: text(item, "lang"),
                selected: item.get("selected").and_then(Json::as_bool) == Some(true),
                external_file: text(item, "external-filename"),
                codec: text(item, "codec"),
            })
        })
        .collect()
}

pub(super) fn apply_stat(stats: &mut Stats, id: u64, value: &Value) -> bool {
    let number = match *value {
        Value::Double(x) if x.is_finite() && x >= 0.0 => Some(x),
        _ => None,
    };
    let field = match id {
        CACHE_SPEED => &mut stats.cache_speed,
        CACHE_DURATION => &mut stats.cache_s,
        WIDTH => &mut stats.width,
        HEIGHT => &mut stats.height,
        VIDEO_BITRATE => &mut stats.video_bitrate,
        FRAME_DROPS => &mut stats.dropped_frames,
        HWDEC_CURRENT => {
            let hwdec = match value {
                Value::Text(text) if !text.is_empty() => Some(text.clone()),
                _ => None,
            };
            return replace(&mut stats.hwdec, hwdec);
        }
        _ => return false,
    };
    replace(field, number)
}

pub(super) struct EventLoop {
    pub(super) core: Arc<Core>,
    pub(super) status: Arc<Mutex<Status>>,
    pub(super) finished: Arc<AtomicBool>,
    pub(super) notify: Arc<dyn Fn() + Send + Sync>,
    pub(super) events: mpsc::UnboundedSender<PlayerEvent>,
    pub(super) tracker: Tracker,
}

pub(super) fn log_level(mpv_level: &str) -> tracing::Level {
    match mpv_level {
        "fatal" | "error" => tracing::Level::ERROR,
        _ => tracing::Level::INFO,
    }
}

impl EventLoop {
    pub(super) fn run(mut self) {
        let mut done = false;
        loop {
            let input = match self.core.wait_event(-1.0) {
                Event::Shutdown => break,
                Event::Log {
                    level,
                    prefix,
                    text,
                } => {
                    let text = text.trim_end();
                    if log_level(&level) == tracing::Level::ERROR {
                        error!(target: "libmpv", "[{prefix}] {text}");
                    } else {
                        info!(target: "libmpv", "[{prefix}] {text}");
                    }
                    None
                }
                Event::FileLoaded => Some(Input::FileLoaded),
                Event::PlaybackRestart => {
                    self.update(|status| replace(&mut status.loaded, true));
                    None
                }
                Event::EndFile { eof } => Some(Input::EndFile(match eof {
                    Ok(true) => EndReason::Eof,
                    Ok(false) => EndReason::Other,
                    Err(message) => EndReason::Error(message),
                })),
                Event::Property { id, value } => self.property(id, &value),
                Event::None | Event::Other => None,
            };
            let Some(input) = input else { continue };
            if done {
                continue;
            }
            let step = self.tracker.on(input, Instant::now());
            if let Some(event) = step.event {
                let _ = self.events.send(event);
            }
            if step.done {
                done = true;
                self.finished.store(true, Ordering::SeqCst);
                (self.notify)();
                let _ = self.core.command_async(&["quit"]);
            }
        }
        if !done {
            let _ = self.events.send(self.tracker.closed());
        }
        self.finished.store(true, Ordering::SeqCst);
        (self.notify)();
        debug!("mpv event loop ended");
    }

    fn property(&self, id: u64, value: &Value) -> Option<Input> {
        let number = match *value {
            Value::Double(x) if x.is_finite() => x,
            _ => 0.0,
        };
        let flag = *value == Value::Flag(true);
        match id {
            TIME_POS => {
                self.update(|s| replace(&mut s.position_s, number));
                return matches!(value, Value::Double(_)).then_some(Input::TimePos(number));
            }
            DURATION => {
                self.update(|s| replace(&mut s.duration_s, number));
                return matches!(value, Value::Double(_)).then_some(Input::Duration(number));
            }
            PAUSE => self.update(|s| replace(&mut s.paused, flag)),
            BUFFERING => self.update(|s| replace(&mut s.buffering, flag)),
            SEEKING => self.update(|s| replace(&mut s.seeking, flag)),
            VOLUME => self.update(|s| replace(&mut s.volume, number)),
            MUTE => self.update(|s| replace(&mut s.muted, flag)),
            TRACK_LIST => {
                let tracks = match value {
                    Value::Text(json) => parse_tracks(json),
                    _ => Vec::new(),
                };
                self.update(|s| replace_shared(&mut s.tracks, tracks));
            }
            CACHE_STATE => {
                let ranges = match value {
                    Value::Text(json) => parse_cached_ranges(json),
                    _ => Vec::new(),
                };
                self.update(|s| replace_shared(&mut s.cached, ranges));
            }
            CACHE_DURATION => {
                self.update(|s| apply_stat(&mut s.stats, id, value));
                return Some(Input::Buffered(number));
            }
            _ => self.update(|s| apply_stat(&mut s.stats, id, value)),
        }
        None
    }

    fn update(&self, change: impl FnOnce(&mut Status) -> bool) {
        if change(&mut self.status.lock().unwrap_or_else(PoisonError::into_inner)) {
            (self.notify)();
        }
    }
}

pub(super) fn replace<T: PartialEq>(slot: &mut T, value: T) -> bool {
    let changed = *slot != value;
    *slot = value;
    changed
}

pub(super) fn replace_shared<T: PartialEq>(slot: &mut Arc<[T]>, items: Vec<T>) -> bool {
    let changed = **slot != *items;
    if changed {
        *slot = items.into();
    }
    changed
}
