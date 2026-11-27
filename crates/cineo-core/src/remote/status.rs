use serde_json::{Value as Json, json};

use super::{PROTOCOL, clean_text};

pub const MAX_TITLE_CHARS: usize = 200;
pub const MAX_TRACK_LABEL_CHARS: usize = 60;
pub const MAX_TRACKS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    pub id: i64,
    pub label: Option<String>,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NowPlaying {
    pub title: String,
    pub loading: bool,
    pub paused: bool,
    pub position_s: f64,
    pub duration_s: f64,
    pub volume: f64,
    pub muted: bool,
    pub speed: f64,
    pub subtitle_delay_s: f64,
    pub audio: Vec<Track>,
    pub subtitles: Vec<Track>,
}

impl Default for NowPlaying {
    fn default() -> Self {
        Self {
            title: String::new(),
            loading: false,
            paused: false,
            position_s: 0.0,
            duration_s: 0.0,
            volume: 100.0,
            muted: false,
            speed: 1.0,
            subtitle_delay_s: 0.0,
            audio: Vec::new(),
            subtitles: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Status {
    pub playing: Option<NowPlaying>,
}

fn tracks_json(tracks: &[Track]) -> Json {
    tracks
        .iter()
        .take(MAX_TRACKS)
        .map(|t| json!({ "id": t.id, "label": t.label, "selected": t.selected }))
        .collect()
}

fn parse_tracks(json: Option<&Json>) -> Vec<Track> {
    json.and_then(Json::as_array)
        .into_iter()
        .flatten()
        .filter_map(|track| {
            Some(Track {
                id: track.get("id")?.as_i64().filter(|id| *id > 0)?,
                label: track
                    .get("label")
                    .and_then(Json::as_str)
                    .map(|label| clean_text(label, MAX_TRACK_LABEL_CHARS))
                    .filter(|label| !label.is_empty()),
                selected: track
                    .get("selected")
                    .and_then(Json::as_bool)
                    .unwrap_or(false),
            })
        })
        .take(MAX_TRACKS)
        .collect()
}

fn number(json: &Json, name: &str, low: f64, high: f64, default: f64) -> f64 {
    json.get(name)
        .and_then(Json::as_f64)
        .filter(|value| value.is_finite())
        .map_or(default, |value| value.clamp(low, high))
}

impl Status {
    pub fn to_json(&self) -> String {
        let playing = self.playing.as_ref().map(|p| {
            json!({
                "title": p.title,
                "loading": p.loading,
                "paused": p.paused,
                "position": p.position_s,
                "duration": p.duration_s,
                "volume": p.volume,
                "muted": p.muted,
                "speed": p.speed,
                "subtitle_delay": p.subtitle_delay_s,
                "audio": tracks_json(&p.audio),
                "subtitles": tracks_json(&p.subtitles),
            })
        });
        json!({ "v": PROTOCOL, "playing": playing }).to_string()
    }

    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let json: Json = serde_json::from_slice(bytes).ok()?;
        if json.get("v")?.as_u64()? != PROTOCOL {
            return None;
        }
        let playing = match json.get("playing") {
            None | Some(Json::Null) => None,
            Some(p) => {
                let flag = |name: &str| p.get(name).and_then(Json::as_bool).unwrap_or(false);
                let duration_s = number(p, "duration", 0.0, f64::MAX, 0.0);
                Some(NowPlaying {
                    title: clean_text(
                        p.get("title").and_then(Json::as_str).unwrap_or_default(),
                        MAX_TITLE_CHARS,
                    ),
                    loading: flag("loading"),
                    paused: flag("paused"),
                    position_s: number(p, "position", 0.0, f64::MAX, 0.0),
                    duration_s,
                    volume: number(p, "volume", 0.0, 100.0, 100.0),
                    muted: flag("muted"),
                    speed: number(p, "speed", super::MIN_SPEED, super::MAX_SPEED, 1.0),
                    subtitle_delay_s: number(
                        p,
                        "subtitle_delay",
                        -super::MAX_SUBTITLE_DELAY_S,
                        super::MAX_SUBTITLE_DELAY_S,
                        0.0,
                    ),
                    audio: parse_tracks(p.get("audio")),
                    subtitles: parse_tracks(p.get("subtitles")),
                })
            }
        };
        Some(Self { playing })
    }
}
