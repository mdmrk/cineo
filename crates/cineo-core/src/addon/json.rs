use serde_json::{Map, Value};
use url::Url;

use super::ResponseError;
use super::types::ContentType;
use crate::diagnostics::{Parsed, Warnings};

pub(crate) type Object = Map<String, Value>;

pub(crate) fn object<'a>(
    item: &'a Value,
    loc: &str,
    warnings: &mut Warnings,
) -> Option<&'a Object> {
    if let Value::Object(obj) = item {
        Some(obj)
    } else {
        warnings.skipped(loc, format!("expected object, got {}", kind(item)));
        None
    }
}

pub(crate) fn id(obj: &Object, loc: &str, warnings: &mut Warnings) -> Option<String> {
    match obj.get("id") {
        Some(Value::String(id)) if !id.is_empty() => Some(id.clone()),
        _ => {
            warnings.skipped(loc, "missing or empty `id`");
            None
        }
    }
}

pub(crate) fn content_type(
    obj: &Object,
    loc: &str,
    warnings: &mut Warnings,
) -> Option<ContentType> {
    let parsed = obj
        .get("type")
        .and_then(Value::as_str)
        .and_then(ContentType::new);
    if parsed.is_none() {
        warnings.skipped(loc, "missing or invalid `type`");
    }
    parsed
}

pub(crate) fn list<T>(
    obj: &Object,
    key: &str,
    loc: &str,
    warnings: &mut Warnings,
    parse_item: impl Fn(&Value, &str, &mut Warnings) -> Option<T>,
) -> Vec<T> {
    match obj.get(key) {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(items)) => {
            let base = field(loc, key);
            items
                .iter()
                .enumerate()
                .filter_map(|(i, item)| parse_item(item, &format!("{base}[{i}]"), warnings))
                .collect()
        }
        Some(other) => {
            warnings.ignored(
                field(loc, key),
                format!("expected array, got {}", kind(other)),
            );
            Vec::new()
        }
    }
}

pub(crate) fn root(bytes: &[u8]) -> Result<Object, ResponseError> {
    match serde_json::from_slice(bytes)? {
        Value::Object(obj) => Ok(obj),
        _ => Err(ResponseError::NotAnObject),
    }
}

pub(crate) fn response_list<T>(
    bytes: &[u8],
    key: &'static str,
    parse_item: impl Fn(&Value, &str, &mut Warnings) -> Option<T>,
) -> Result<Parsed<Vec<T>>, ResponseError> {
    let obj = root(bytes)?;
    if !obj.contains_key(key) {
        return Err(ResponseError::MissingField(key));
    }
    let mut warnings = Warnings::default();
    let items = list(&obj, key, "", &mut warnings, parse_item);
    Ok(warnings.finish(items))
}

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

pub(crate) fn opt_http_url(
    obj: &Object,
    key: &str,
    loc: &str,
    warnings: &mut Warnings,
) -> Option<Url> {
    let url = opt_any_url(obj, key, loc, warnings)?;
    if matches!(url.scheme(), "http" | "https") {
        return Some(url);
    }
    warnings.ignored(
        field(loc, key),
        format!("unsupported scheme `{}`", url.scheme()),
    );
    None
}

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
