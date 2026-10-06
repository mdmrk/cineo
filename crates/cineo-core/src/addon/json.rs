//! Lenient accessors over `serde_json::Value` for untrusted addon documents.
//!
//! Each helper either returns a usable value or records a warning and returns
//! the "absent" value. None of them fail.

use serde_json::{Map, Value};
use url::Url;

use crate::diagnostics::Warnings;

pub(crate) type Object = Map<String, Value>;

/// Optional string field. Empty strings count as absent (reference behavior).
pub(crate) fn opt_string(
    obj: &Object,
    key: &str,
    loc: &str,
    warnings: &mut Warnings,
) -> Option<String> {
    match obj.get(key) {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) if s.is_empty() => None,
        Some(Value::String(s)) => Some(s.clone()),
        Some(other) => {
            warnings.ignored(
                field(loc, key),
                format!("expected string, got {}", kind(other)),
            );
            None
        }
    }
}

/// Optional field that addons send either as a string or as a number
/// (e.g. `releaseInfo: 2010`, `imdbRating: 7.5`). Normalized to a string.
pub(crate) fn opt_string_or_number(
    obj: &Object,
    key: &str,
    loc: &str,
    warnings: &mut Warnings,
) -> Option<String> {
    match obj.get(key) {
        Some(Value::Number(n)) => Some(n.to_string()),
        _ => opt_string(obj, key, loc, warnings),
    }
}

/// Optional `http(s)` URL field, e.g. an image. Other schemes are ignored so
/// that addon data can never point the client at `file://` and similar.
pub(crate) fn opt_http_url(
    obj: &Object,
    key: &str,
    loc: &str,
    warnings: &mut Warnings,
) -> Option<Url> {
    let raw = opt_string(obj, key, loc, warnings)?;
    match Url::parse(&raw) {
        Ok(url) if matches!(url.scheme(), "http" | "https") => Some(url),
        Ok(url) => {
            warnings.ignored(
                field(loc, key),
                format!("unsupported scheme `{}`", url.scheme()),
            );
            None
        }
        Err(err) => {
            warnings.ignored(field(loc, key), format!("invalid URL: {err}"));
            None
        }
    }
}

/// Optional boolean; anything that is not a boolean counts as `false`.
pub(crate) fn bool_or_false(obj: &Object, key: &str, loc: &str, warnings: &mut Warnings) -> bool {
    match obj.get(key) {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(other) => {
            warnings.ignored(
                field(loc, key),
                format!("expected boolean, got {}", kind(other)),
            );
            false
        }
    }
}

/// Optional array of strings. `null`/missing is empty; non-string elements
/// are skipped with a warning.
pub(crate) fn string_list(
    obj: &Object,
    key: &str,
    loc: &str,
    warnings: &mut Warnings,
) -> Option<Vec<String>> {
    match obj.get(key) {
        None | Some(Value::Null) => None,
        Some(Value::Array(items)) => Some(
            items
                .iter()
                .enumerate()
                .filter_map(|(i, item)| match item {
                    Value::String(s) => Some(s.clone()),
                    other => {
                        warnings.skipped(
                            format!("{}[{i}]", field(loc, key)),
                            format!("expected string, got {}", kind(other)),
                        );
                        None
                    }
                })
                .collect(),
        ),
        Some(other) => {
            warnings.ignored(
                field(loc, key),
                format!("expected array, got {}", kind(other)),
            );
            None
        }
    }
}

pub(crate) fn field(loc: &str, key: &str) -> String {
    if loc.is_empty() {
        key.to_owned()
    } else {
        format!("{loc}.{key}")
    }
}

pub(crate) fn kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// Optional non-negative integer; accepts JSON numbers and numeric strings
/// (addons send both, e.g. `"season": "1"`).
pub(crate) fn opt_u32(obj: &Object, key: &str, loc: &str, warnings: &mut Warnings) -> Option<u32> {
    let parsed = match obj.get(key) {
        None | Some(Value::Null) => return None,
        Some(Value::Number(n)) => n.as_u64().and_then(|n| u32::try_from(n).ok()),
        Some(Value::String(s)) => s.trim().parse::<u32>().ok(),
        Some(_) => None,
    };
    if parsed.is_none() {
        warnings.ignored(field(loc, key), "expected a non-negative integer");
    }
    parsed
}

/// Optional URL with any scheme; used for stream sources whose playability is
/// decided later. Invalid URLs are absent + warning.
pub(crate) fn opt_any_url(
    obj: &Object,
    key: &str,
    loc: &str,
    warnings: &mut Warnings,
) -> Option<Url> {
    let raw = opt_string(obj, key, loc, warnings)?;
    match Url::parse(&raw) {
        Ok(url) => Some(url),
        Err(err) => {
            warnings.ignored(field(loc, key), format!("invalid URL: {err}"));
            None
        }
    }
}
