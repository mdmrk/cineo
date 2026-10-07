//! Catalog responses: `{ "metas": [MetaPreview, ...] }`.

use serde_json::Value;
use url::Url;

use super::ResponseError;
use super::json::{self, kind};
use super::types::ContentType;
use crate::diagnostics::{Parsed, Warnings};

/// Aspect ratio hint for a poster image.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PosterShape {
    #[default]
    Poster,
    Square,
    Landscape,
}

/// A catalog entry. Only fields Cineo uses are kept; see
/// `docs/ADDON_PROTOCOL.md` for ignored fields.
#[derive(Debug, Clone, PartialEq)]
pub struct MetaPreview {
    /// Non-empty content id, e.g. `tt0111161`.
    pub id: String,
    pub content_type: ContentType,
    /// May be empty: some addons omit it, and the reference client accepts that.
    pub name: String,
    pub poster: Option<Url>,
    pub poster_shape: PosterShape,
    pub background: Option<Url>,
    pub logo: Option<Url>,
    pub description: Option<String>,
    pub release_info: Option<String>,
    pub imdb_rating: Option<String>,
    pub genres: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CatalogResponse {
    pub metas: Vec<MetaPreview>,
}

/// Parses a catalog response.
pub fn parse_catalog_response(bytes: &[u8]) -> Result<Parsed<CatalogResponse>, ResponseError> {
    let root: Value = serde_json::from_slice(bytes)?;
    let Value::Object(obj) = root else {
        return Err(ResponseError::NotAnObject);
    };
    let mut warnings = Warnings::default();
    let metas = match obj.get("metas") {
        None => return Err(ResponseError::MissingField("metas")),
        Some(Value::Null) => Vec::new(),
        Some(Value::Array(items)) => items
            .iter()
            .enumerate()
            .filter_map(|(i, item)| parse_meta_preview(item, &format!("metas[{i}]"), &mut warnings))
            .collect(),
        Some(other) => {
            warnings.ignored("metas", format!("expected array, got {}", kind(other)));
            Vec::new()
        }
    };
    Ok(warnings.finish(CatalogResponse { metas }))
}

pub(super) fn parse_meta_preview(
    item: &Value,
    loc: &str,
    warnings: &mut Warnings,
) -> Option<MetaPreview> {
    let Value::Object(obj) = item else {
        warnings.skipped(loc, format!("expected object, got {}", kind(item)));
        return None;
    };
    let id = match obj.get("id") {
        Some(Value::String(id)) if !id.is_empty() => id.clone(),
        _ => {
            warnings.skipped(loc, "missing or empty `id`");
            return None;
        }
    };
    let content_type = match obj.get("type") {
        Some(Value::String(t)) => ContentType::new(t.clone()),
        _ => None,
    };
    let Some(content_type) = content_type else {
        warnings.skipped(loc, "missing or invalid `type`");
        return None;
    };
    let poster_shape = match json::opt_string(obj, "posterShape", loc, warnings).as_deref() {
        None | Some("poster") => PosterShape::Poster,
        Some("square") => PosterShape::Square,
        Some("landscape") => PosterShape::Landscape,
        Some(other) => {
            warnings.ignored(
                json::field(loc, "posterShape"),
                format!("unknown shape `{other}`"),
            );
            PosterShape::Poster
        }
    };
    Some(MetaPreview {
        name: json::opt_string(obj, "name", loc, warnings).unwrap_or_default(),
        poster: json::opt_http_url(obj, "poster", loc, warnings),
        poster_shape,
        background: json::opt_http_url(obj, "background", loc, warnings),
        logo: json::opt_http_url(obj, "logo", loc, warnings),
        description: json::opt_string(obj, "description", loc, warnings),
        release_info: json::opt_string_or_number(obj, "releaseInfo", loc, warnings),
        imdb_rating: json::opt_string_or_number(obj, "imdbRating", loc, warnings),
        genres: json::string_list(obj, "genres", loc, warnings).unwrap_or_default(),
        id,
        content_type,
    })
}
