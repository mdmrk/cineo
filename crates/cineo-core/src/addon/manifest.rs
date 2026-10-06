//! Addon manifest: parsing, validation and resource filtering.
//!
//! Filtering semantics intentionally match the reference client; see
//! `docs/ADDON_PROTOCOL.md#resource-filtering`.

use std::collections::HashSet;

use serde_json::Value;
use url::Url;

use super::json::{self, Object, field, kind};
use super::request::{ExtraValue, ResourcePath};
use super::types::{ContentType, ResourceName};
use crate::diagnostics::{Parsed, Warnings};

/// A validated addon manifest.
#[derive(Debug, Clone, PartialEq)]
pub struct Manifest {
    /// Dot-separated identifier chosen by the addon author. Non-empty.
    pub id: String,
    pub version: semver::Version,
    pub name: String,
    pub description: Option<String>,
    pub logo: Option<Url>,
    pub background: Option<Url>,
    pub contact_email: Option<String>,
    /// Manifest-level types, inherited by short-form resources.
    pub types: Vec<ContentType>,
    /// Declared resources, de-duplicated by name (first wins).
    pub resources: Vec<Resource>,
    /// Catalogs, de-duplicated by `(type, id)` (first wins).
    pub catalogs: Vec<CatalogDef>,
    pub addon_catalogs: Vec<CatalogDef>,
    pub behavior_hints: BehaviorHints,
}

/// A resource the addon serves, with inheritance already resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resource {
    pub name: ResourceName,
    /// Types this resource answers for. Empty means it answers for none.
    pub types: Vec<ContentType>,
    pub ids: IdFilter,
}

/// Which content ids a resource answers for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdFilter {
    /// No `idPrefixes` declared, or an empty list: every id matches.
    Any,
    /// The id must start with one of these prefixes. Never empty.
    Prefixes(Vec<String>),
}

impl IdFilter {
    fn from_prefixes(prefixes: Option<Vec<String>>) -> Self {
        match prefixes {
            Some(prefixes) if !prefixes.is_empty() => Self::Prefixes(prefixes),
            _ => Self::Any,
        }
    }

    pub fn matches(&self, id: &str) -> bool {
        match self {
            Self::Any => true,
            Self::Prefixes(prefixes) => prefixes.iter().any(|p| id.starts_with(p.as_str())),
        }
    }
}

/// A catalog (or addon catalog) declared in the manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogDef {
    pub content_type: ContentType,
    pub id: String,
    pub name: Option<String>,
    /// Extra properties, de-duplicated by name (first wins).
    pub extra: Vec<ExtraProp>,
}

/// A declared catalog extra property such as `search`, `genre` or `skip`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtraProp {
    pub name: String,
    pub is_required: bool,
    pub options: Vec<String>,
    /// Maximum number of values that may be sent for this property. `>= 1`.
    pub options_limit: usize,
}

/// Why a set of extra values is not acceptable for a catalog.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ExtraError {
    #[error("extra `{0}` is not declared by the catalog")]
    Unsupported(String),
    #[error("required extra `{0}` is missing")]
    MissingRequired(String),
    #[error("extra `{name}` has more than {limit} value(s)")]
    TooManyValues { name: String, limit: usize },
}

impl CatalogDef {
    /// Checks extra values against the declaration: every name declared,
    /// every required name present, and no name exceeding its `optionsLimit`.
    pub fn check_extra(&self, values: &[ExtraValue]) -> Result<(), ExtraError> {
        for value in values {
            let Some(prop) = self.extra.iter().find(|p| p.name == value.name) else {
                return Err(ExtraError::Unsupported(value.name.clone()));
            };
            let count = values.iter().filter(|v| v.name == value.name).count();
            if count > prop.options_limit {
                return Err(ExtraError::TooManyValues {
                    name: prop.name.clone(),
                    limit: prop.options_limit,
                });
            }
        }
        for prop in self.extra.iter().filter(|p| p.is_required) {
            if !values.iter().any(|v| v.name == prop.name) {
                return Err(ExtraError::MissingRequired(prop.name.clone()));
            }
        }
        Ok(())
    }

    /// Whether the catalog can be requested without any user input, i.e. it
    /// has no required extra properties. Such catalogs appear on the board.
    pub fn is_browsable(&self) -> bool {
        !self.extra.iter().any(|p| p.is_required)
    }
}

/// Manifest-level behavior hints. Unknown hints are ignored.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BehaviorHints {
    pub adult: bool,
    pub p2p: bool,
    pub configurable: bool,
    pub configuration_required: bool,
}

impl Manifest {
    /// Whether this addon should be asked for `path`.
    ///
    /// * `catalog` / `addon_catalog`: a declared catalog with the same type and
    ///   id must accept the extra values. The `resources` list is not consulted
    ///   (reference-client behavior).
    /// * other resources: the resource must be declared, the type must be one
    ///   of its types and the id must match its id filter.
    pub fn supports(&self, path: &ResourcePath) -> bool {
        let catalog_match = |catalogs: &[CatalogDef]| {
            catalogs.iter().any(|c| {
                c.content_type == path.content_type
                    && c.id == path.id
                    && c.check_extra(&path.extra).is_ok()
            })
        };
        match &path.resource {
            ResourceName::Catalog => catalog_match(&self.catalogs),
            ResourceName::AddonCatalog => catalog_match(&self.addon_catalogs),
            name => self
                .resources
                .iter()
                .find(|r| &r.name == name)
                .is_some_and(|r| r.types.contains(&path.content_type) && r.ids.matches(&path.id)),
        }
    }

    pub fn catalog(&self, content_type: &ContentType, id: &str) -> Option<&CatalogDef> {
        self.catalogs
            .iter()
            .find(|c| &c.content_type == content_type && c.id == id)
    }
}

/// A manifest that cannot be used at all.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ManifestError {
    #[error("manifest is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("manifest is not a JSON object")]
    NotAnObject,
    #[error("manifest field `{0}` is missing")]
    MissingField(&'static str),
    #[error("manifest field `{field}` is invalid: {reason}")]
    InvalidField { field: &'static str, reason: String },
}

/// Parses and validates a manifest document.
///
/// Fails only when a required field (`id`, `version`, `name`, `types`,
/// `resources`) is missing or unusable. Everything else degrades to a warning.
pub fn parse_manifest(bytes: &[u8]) -> Result<Parsed<Manifest>, ManifestError> {
    let root: Value = serde_json::from_slice(bytes)?;
    let Value::Object(obj) = root else {
        return Err(ManifestError::NotAnObject);
    };
    let mut warnings = Warnings::default();

    let id = required_string(&obj, "id")?;
    let version_raw = required_string(&obj, "version")?;
    let version =
        semver::Version::parse(&version_raw).map_err(|err| ManifestError::InvalidField {
            field: "version",
            reason: err.to_string(),
        })?;
    let name = required_string(&obj, "name")?;

    let types = match obj.get("types") {
        Some(Value::Array(_)) => content_types(&obj, "types", "", &mut warnings),
        Some(other) => {
            return Err(ManifestError::InvalidField {
                field: "types",
                reason: format!("expected array, got {}", kind(other)),
            });
        }
        None => return Err(ManifestError::MissingField("types")),
    };
    let id_prefixes = json::string_list(&obj, "idPrefixes", "", &mut warnings);

    let resources = match obj.get("resources") {
        Some(Value::Array(items)) => {
            parse_resources(items, &types, id_prefixes.as_ref(), &mut warnings)
        }
        Some(other) => {
            return Err(ManifestError::InvalidField {
                field: "resources",
                reason: format!("expected array, got {}", kind(other)),
            });
        }
        None => return Err(ManifestError::MissingField("resources")),
    };

    let catalogs = parse_catalogs(&obj, "catalogs", &mut warnings);
    let addon_catalogs = parse_catalogs(&obj, "addonCatalogs", &mut warnings);

    if resources.is_empty() && catalogs.is_empty() {
        warnings.ignored("resources", "addon declares no resources and no catalogs");
    }

    let behavior_hints = match obj.get("behaviorHints") {
        Some(Value::Object(hints)) => {
            let loc = "behaviorHints";
            BehaviorHints {
                adult: json::bool_or_false(hints, "adult", loc, &mut warnings),
                p2p: json::bool_or_false(hints, "p2p", loc, &mut warnings),
                configurable: json::bool_or_false(hints, "configurable", loc, &mut warnings),
                configuration_required: json::bool_or_false(
                    hints,
                    "configurationRequired",
                    loc,
                    &mut warnings,
                ),
            }
        }
        None | Some(Value::Null) => BehaviorHints::default(),
        Some(other) => {
            warnings.ignored(
                "behaviorHints",
                format!("expected object, got {}", kind(other)),
            );
            BehaviorHints::default()
        }
    };

    let manifest = Manifest {
        id,
        version,
        name,
        description: json::opt_string(&obj, "description", "", &mut warnings),
        logo: json::opt_http_url(&obj, "logo", "", &mut warnings),
        background: json::opt_http_url(&obj, "background", "", &mut warnings),
        contact_email: json::opt_string(&obj, "contactEmail", "", &mut warnings),
        types,
        resources,
        catalogs,
        addon_catalogs,
        behavior_hints,
    };
    Ok(warnings.finish(manifest))
}

fn required_string(obj: &Object, key: &'static str) -> Result<String, ManifestError> {
    match obj.get(key) {
        Some(Value::String(s)) if !s.trim().is_empty() => Ok(s.clone()),
        Some(Value::String(_)) => Err(ManifestError::InvalidField {
            field: key,
            reason: "must not be empty".to_owned(),
        }),
        None | Some(Value::Null) => Err(ManifestError::MissingField(key)),
        Some(other) => Err(ManifestError::InvalidField {
            field: key,
            reason: format!("expected string, got {}", kind(other)),
        }),
    }
}

/// Parses a list of content types, skipping invalid entries.
fn content_types(obj: &Object, key: &str, loc: &str, warnings: &mut Warnings) -> Vec<ContentType> {
    json::string_list(obj, key, loc, warnings)
        .unwrap_or_default()
        .into_iter()
        .enumerate()
        .filter_map(|(i, raw)| {
            let parsed = ContentType::new(raw);
            if parsed.is_none() {
                warnings.skipped(format!("{}[{i}]", field(loc, key)), "invalid content type");
            }
            parsed
        })
        .collect()
}

fn parse_resources(
    items: &[Value],
    manifest_types: &[ContentType],
    manifest_prefixes: Option<&Vec<String>>,
    warnings: &mut Warnings,
) -> Vec<Resource> {
    let mut seen = HashSet::new();
    let mut resources = Vec::with_capacity(items.len());
    for (i, item) in items.iter().enumerate() {
        let loc = format!("resources[{i}]");
        let resource = match item {
            // Short form inherits manifest-level types and idPrefixes.
            Value::String(name) => Resource {
                name: ResourceName::parse(name),
                types: manifest_types.to_vec(),
                ids: IdFilter::from_prefixes(manifest_prefixes.cloned()),
            },
            // Full form inherits nothing: missing `types` matches no type,
            // missing `idPrefixes` matches every id (reference behavior).
            Value::Object(obj) => {
                let Some(Value::String(name)) = obj.get("name") else {
                    warnings.skipped(loc, "resource object without a string `name`");
                    continue;
                };
                if obj.get("types").is_none_or(Value::is_null) {
                    warnings.ignored(
                        field(&loc, "types"),
                        "missing: resource will not match any type",
                    );
                }
                Resource {
                    name: ResourceName::parse(name),
                    types: content_types(obj, "types", &loc, warnings),
                    ids: IdFilter::from_prefixes(json::string_list(
                        obj,
                        "idPrefixes",
                        &loc,
                        warnings,
                    )),
                }
            }
            other => {
                warnings.skipped(
                    loc,
                    format!("expected string or object, got {}", kind(other)),
                );
                continue;
            }
        };
        if seen.insert(resource.name.clone()) {
            resources.push(resource);
        } else {
            warnings.duplicate(loc, resource.name.as_str());
        }
    }
    resources
}

fn parse_catalogs(obj: &Object, key: &str, warnings: &mut Warnings) -> Vec<CatalogDef> {
    let items = match obj.get(key) {
        None | Some(Value::Null) => return Vec::new(),
        Some(Value::Array(items)) => items,
        Some(other) => {
            warnings.ignored(key, format!("expected array, got {}", kind(other)));
            return Vec::new();
        }
    };
    let mut seen = HashSet::new();
    let mut catalogs = Vec::with_capacity(items.len());
    for (i, item) in items.iter().enumerate() {
        let loc = format!("{key}[{i}]");
        let Value::Object(cat) = item else {
            warnings.skipped(loc, format!("expected object, got {}", kind(item)));
            continue;
        };
        let content_type = match cat.get("type") {
            Some(Value::String(t)) => ContentType::new(t.clone()),
            _ => None,
        };
        let Some(content_type) = content_type else {
            warnings.skipped(loc, "missing or invalid `type`");
            continue;
        };
        let id = match cat.get("id") {
            Some(Value::String(id)) if !id.is_empty() => id.clone(),
            _ => {
                warnings.skipped(loc, "missing or empty `id`");
                continue;
            }
        };
        let key_pair = (content_type.clone(), id.clone());
        if !seen.insert(key_pair) {
            warnings.duplicate(loc, format!("{content_type}/{id}"));
            continue;
        }
        catalogs.push(CatalogDef {
            name: json::opt_string(cat, "name", &loc, warnings),
            extra: parse_extra(cat, &loc, warnings),
            content_type,
            id,
        });
    }
    catalogs
}

/// Parses `extra` (full form) or `extraRequired`/`extraSupported` (short form).
fn parse_extra(cat: &Object, loc: &str, warnings: &mut Warnings) -> Vec<ExtraProp> {
    let mut props = match cat.get("extra") {
        Some(Value::Array(items)) => items
            .iter()
            .enumerate()
            .filter_map(|(i, item)| parse_extra_prop(item, &format!("{loc}.extra[{i}]"), warnings))
            .collect(),
        None | Some(Value::Null) => {
            let supported =
                json::string_list(cat, "extraSupported", loc, warnings).unwrap_or_default();
            let required =
                json::string_list(cat, "extraRequired", loc, warnings).unwrap_or_default();
            // Reference behavior: only names listed in `extraSupported` exist.
            for name in required.iter().filter(|r| !supported.contains(r)) {
                warnings.ignored(
                    field(loc, "extraRequired"),
                    format!("`{name}` is not listed in extraSupported"),
                );
            }
            supported
                .into_iter()
                .map(|name| ExtraProp {
                    is_required: required.contains(&name),
                    name,
                    options: Vec::new(),
                    options_limit: 1,
                })
                .collect()
        }
        Some(other) => {
            warnings.ignored(
                field(loc, "extra"),
                format!("expected array, got {}", kind(other)),
            );
            Vec::new()
        }
    };
    let mut seen = HashSet::new();
    props.retain(|p: &ExtraProp| {
        let first = seen.insert(p.name.clone());
        if !first {
            warnings.duplicate(field(loc, "extra"), p.name.as_str());
        }
        first
    });
    props
}

fn parse_extra_prop(item: &Value, loc: &str, warnings: &mut Warnings) -> Option<ExtraProp> {
    let Value::Object(obj) = item else {
        warnings.skipped(loc, format!("expected object, got {}", kind(item)));
        return None;
    };
    let Some(Value::String(name)) = obj.get("name").filter(|n| n.as_str() != Some("")) else {
        warnings.skipped(loc, "missing or empty `name`");
        return None;
    };
    // Reference behavior: `skip` is always optional, single-valued, option-less.
    if name == "skip" {
        return Some(ExtraProp {
            name: name.clone(),
            is_required: false,
            options: Vec::new(),
            options_limit: 1,
        });
    }
    let options_limit = match obj.get("optionsLimit") {
        None | Some(Value::Null) => 1,
        Some(value) => match value.as_u64().and_then(|n| usize::try_from(n).ok()) {
            Some(n) if n >= 1 => n,
            _ => {
                warnings.ignored(field(loc, "optionsLimit"), "expected integer >= 1, using 1");
                1
            }
        },
    };
    Some(ExtraProp {
        name: name.clone(),
        is_required: json::bool_or_false(obj, "isRequired", loc, warnings),
        options: json::string_list(obj, "options", loc, warnings).unwrap_or_default(),
        options_limit,
    })
}
