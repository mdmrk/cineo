use std::cell::RefCell;
use std::collections::HashSet;

use serde_json::Value;

use super::ResponseError;
use super::json;
use super::manifest::{Manifest, manifest_from_value};
use super::request::TransportUrl;
use crate::diagnostics::{Parsed, Warnings};

#[derive(Debug, Clone, PartialEq)]
pub struct AddonDescriptor {
    pub transport: TransportUrl,
    pub manifest: Manifest,
}

pub fn parse_addon_catalog_response(
    bytes: &[u8],
) -> Result<Parsed<Vec<AddonDescriptor>>, ResponseError> {
    let seen = RefCell::new(HashSet::new());
    json::response_list(bytes, "addons", |item, loc, warnings| {
        let addon = parse_descriptor(item, loc, warnings)?;
        if seen.borrow_mut().insert(addon.transport.clone()) {
            Some(addon)
        } else {
            warnings.duplicate(loc, "transportUrl");
            None
        }
    })
}

fn parse_descriptor(item: &Value, loc: &str, warnings: &mut Warnings) -> Option<AddonDescriptor> {
    let obj = json::object(item, loc, warnings)?;
    let Some(transport) = obj
        .get("transportUrl")
        .and_then(Value::as_str)
        .and_then(|url| TransportUrl::parse(url).ok())
    else {
        warnings.skipped(loc, "missing or invalid `transportUrl`");
        return None;
    };
    match manifest_from_value(obj.get("manifest").cloned().unwrap_or(Value::Null)) {
        Ok(parsed) => Some(AddonDescriptor {
            transport,
            manifest: parsed.value,
        }),
        Err(err) => {
            warnings.skipped(loc, format!("invalid `manifest`: {err}"));
            None
        }
    }
}
