use std::fmt;

use serde_json::{Map, Value as Json, json};
use url::Url;

use super::clock::{DeviceId, Stamp};
use crate::addon::{ContentType, TransportUrl};
use crate::app::{LibraryItem, SavedStream};

pub const FORMAT: u64 = 1;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RecordName {
    Item(String),
    LibraryCleared,
    Addons,
    Setting(String),
    Other(String),
}

impl RecordName {
    pub fn parse(text: &str) -> Option<Self> {
        if text.is_empty() {
            return None;
        }
        Some(match text {
            "library-cleared" => Self::LibraryCleared,
            "addons" => Self::Addons,
            _ => match text.split_once('/') {
                Some(("library", id)) if !id.is_empty() => Self::Item(id.to_owned()),
                Some(("setting", key)) if !key.is_empty() => Self::Setting(key.to_owned()),
                _ => Self::Other(text.to_owned()),
            },
        })
    }
}

impl fmt::Display for RecordName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Item(id) => write!(f, "library/{id}"),
            Self::LibraryCleared => f.write_str("library-cleared"),
            Self::Addons => f.write_str("addons"),
            Self::Setting(key) => write!(f, "setting/{key}"),
            Self::Other(name) => f.write_str(name),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Item(Box<LibraryItem>),
    Deleted,
    Cleared,
    Addons(Vec<TransportUrl>),
    Setting(String),
    Other(Map<String, Json>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub name: RecordName,
    pub stamp: Stamp,
    pub value: Value,
}

impl Record {
    pub fn to_json(&self) -> String {
        let mut obj = match &self.value {
            Value::Other(raw) => return Json::Object(raw.clone()).to_string(),
            Value::Item(item) => item_json(item),
            Value::Deleted => json!({ "deleted": true }),
            Value::Cleared => json!({ "cleared": true }),
            Value::Addons(addons) => json!({
                "addons": addons.iter().map(|a| a.as_url().as_str()).collect::<Vec<_>>(),
            }),
            Value::Setting(value) => json!({ "value": value }),
        };
        if let Json::Object(map) = &mut obj {
            map.insert("v".into(), FORMAT.into());
            map.insert("name".into(), self.name.to_string().into());
            map.insert(
                "stamp".into(),
                json!({
                    "t": self.stamp.physical_ms,
                    "c": self.stamp.counter,
                    "d": self.stamp.device.0,
                }),
            );
        }
        obj.to_string()
    }

    pub fn parse(text: &str) -> Option<Self> {
        let Json::Object(obj) = serde_json::from_str(text).ok()? else {
            return None;
        };
        let name = RecordName::parse(obj.get("name")?.as_str()?)?;
        let stamp = parse_stamp(obj.get("stamp")?)?;
        let value = if obj.get("v").and_then(Json::as_u64) == Some(FORMAT) {
            parse_value(&name, &obj)?
        } else {
            Value::Other(obj)
        };
        Some(Self { name, stamp, value })
    }
}

fn parse_stamp(value: &Json) -> Option<Stamp> {
    Some(Stamp {
        physical_ms: value.get("t")?.as_u64()?,
        counter: u32::try_from(value.get("c")?.as_u64()?).ok()?,
        device: DeviceId(value.get("d")?.as_u64()?),
    })
}

fn parse_value(name: &RecordName, obj: &Map<String, Json>) -> Option<Value> {
    let flag = |key| obj.get(key).and_then(Json::as_bool) == Some(true);
    Some(match name {
        RecordName::Item(_) if flag("deleted") => Value::Deleted,
        RecordName::Item(id) => Value::Item(Box::new(parse_item(id, obj.get("item")?)?)),
        RecordName::LibraryCleared if flag("cleared") => Value::Cleared,
        RecordName::LibraryCleared => return None,
        RecordName::Addons => Value::Addons(
            obj.get("addons")?
                .as_array()?
                .iter()
                .filter_map(Json::as_str)
                .filter_map(|url| TransportUrl::parse(url).ok())
                .collect(),
        ),
        RecordName::Setting(_) => Value::Setting(obj.get("value")?.as_str()?.to_owned()),
        RecordName::Other(_) => Value::Other(obj.clone()),
    })
}

fn item_json(item: &LibraryItem) -> Json {
    let mut fields = Map::new();
    fields.insert("type".into(), item.content_type.as_str().into());
    fields.insert("name".into(), item.name.as_str().into());
    if let Some(poster) = &item.poster {
        fields.insert("poster".into(), poster.as_str().into());
    }
    fields.insert("video".into(), item.video_id.as_str().into());
    fields.insert("offset".into(), item.time_offset_ms.into());
    fields.insert("duration".into(), item.duration_ms.into());
    fields.insert("watched".into(), item.updated_ms.into());
    if let Some(order) = item.favorited {
        fields.insert("favorite".into(), order.into());
    }
    if let Some(stream) = item.stream.as_ref().and_then(|s| s.to_json()) {
        fields.insert("stream".into(), stream.into());
    }
    json!({ "item": fields })
}

fn parse_item(id: &str, item: &Json) -> Option<LibraryItem> {
    let millis = |key| item.get(key).and_then(Json::as_u64);
    Some(LibraryItem {
        id: id.to_owned(),
        content_type: ContentType::new(item.get("type")?.as_str()?)?,
        name: item.get("name")?.as_str()?.to_owned(),
        poster: item
            .get("poster")
            .and_then(Json::as_str)
            .and_then(|raw| Url::parse(raw).ok())
            .filter(|url| matches!(url.scheme(), "http" | "https")),
        video_id: item.get("video")?.as_str()?.to_owned(),
        time_offset_ms: millis("offset")?,
        duration_ms: millis("duration")?,
        updated_ms: millis("watched")?,
        favorited: millis("favorite").filter(|&order| order > 0),
        stream: item
            .get("stream")
            .and_then(Json::as_str)
            .and_then(SavedStream::parse)
            .map(Box::new),
    })
}
