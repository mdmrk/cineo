use serde_json::{Value as Json, json};

use super::PROTOCOL;

pub const MIN_SPEED: f64 = 0.25;
pub const MAX_SPEED: f64 = 4.0;
pub const MAX_SUBTITLE_DELAY_S: f64 = 600.0;
const MAX_VOLUME: f64 = 100.0;
const MAX_SEEK_S: f64 = 24.0 * 60.0 * 60.0;
const MAX_POSITION_S: f64 = 10_000_000.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Command {
    Pause(bool),
    SeekTo(f64),
    SeekBy(f64),
    Volume(f64),
    Mute(bool),
    Audio(Option<i64>),
    Subtitle(Option<i64>),
    SubtitleDelay(f64),
    Speed(f64),
    Stop,
}

impl Command {
    pub fn is_valid(&self) -> bool {
        let within =
            |value: f64, low: f64, high: f64| value.is_finite() && (low..=high).contains(&value);
        match *self {
            Self::SeekTo(seconds) => within(seconds, 0.0, MAX_POSITION_S),
            Self::SeekBy(seconds) => within(seconds, -MAX_SEEK_S, MAX_SEEK_S),
            Self::Volume(percent) => within(percent, 0.0, MAX_VOLUME),
            Self::SubtitleDelay(seconds) => {
                within(seconds, -MAX_SUBTITLE_DELAY_S, MAX_SUBTITLE_DELAY_S)
            }
            Self::Speed(speed) => within(speed, MIN_SPEED, MAX_SPEED),
            Self::Audio(Some(id)) | Self::Subtitle(Some(id)) => id > 0,
            Self::Pause(_)
            | Self::Mute(_)
            | Self::Audio(None)
            | Self::Subtitle(None)
            | Self::Stop => true,
        }
    }

    pub fn to_json(&self) -> Json {
        match *self {
            Self::Pause(paused) => json!({ "pause": paused }),
            Self::SeekTo(seconds) => json!({ "seek_to": seconds }),
            Self::SeekBy(seconds) => json!({ "seek_by": seconds }),
            Self::Volume(percent) => json!({ "volume": percent }),
            Self::Mute(muted) => json!({ "mute": muted }),
            Self::Audio(id) => json!({ "audio": id }),
            Self::Subtitle(id) => json!({ "subtitle": id }),
            Self::SubtitleDelay(seconds) => json!({ "subtitle_delay": seconds }),
            Self::Speed(speed) => json!({ "speed": speed }),
            Self::Stop => json!({ "stop": true }),
        }
    }

    pub fn from_json(json: &Json) -> Option<Self> {
        let object = json.as_object()?;
        let mut entries = object.iter();
        let (name, value) = entries.next()?;
        if entries.next().is_some() {
            return None;
        }
        let track = |value: &Json| match value {
            Json::Null => Some(None),
            other => other.as_i64().map(Some),
        };
        let command = match name.as_str() {
            "pause" => Self::Pause(value.as_bool()?),
            "seek_to" => Self::SeekTo(value.as_f64()?),
            "seek_by" => Self::SeekBy(value.as_f64()?),
            "volume" => Self::Volume(value.as_f64()?),
            "mute" => Self::Mute(value.as_bool()?),
            "audio" => Self::Audio(track(value)?),
            "subtitle" => Self::Subtitle(track(value)?),
            "subtitle_delay" => Self::SubtitleDelay(value.as_f64()?),
            "speed" => Self::Speed(value.as_f64()?),
            "stop" if value.as_bool() == Some(true) => Self::Stop,
            _ => return None,
        };
        command.is_valid().then_some(command)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Request {
    pub time_ms: u64,
    pub command: Option<Command>,
}

impl Request {
    pub fn to_json(&self) -> String {
        json!({
            "v": PROTOCOL,
            "time": self.time_ms,
            "command": self.command.as_ref().map(Command::to_json),
        })
        .to_string()
    }

    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let json: Json = serde_json::from_slice(bytes).ok()?;
        if json.get("v")?.as_u64()? != PROTOCOL {
            return None;
        }
        let command = match json.get("command") {
            None | Some(Json::Null) => None,
            Some(command) => Some(Command::from_json(command)?),
        };
        Some(Self {
            time_ms: json.get("time")?.as_u64()?,
            command,
        })
    }
}
