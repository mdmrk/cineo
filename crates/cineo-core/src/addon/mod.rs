//! The addon protocol model.
//!
//! Compatibility target: the publicly documented Stremio addon protocol
//! (HTTP transport). Every behavior here is described in
//! `docs/ADDON_PROTOCOL.md`; deviations from the reference client are listed
//! there explicitly. Responses are **untrusted input**: parsing is lenient
//! per field and strict only where an invariant is required.

mod catalog;
mod json;
mod manifest;
mod request;
mod types;

pub use catalog::{CatalogResponse, MetaPreview, PosterShape, parse_catalog_response};
pub use manifest::{
    BehaviorHints, CatalogDef, ExtraError, ExtraProp, IdFilter, Manifest, ManifestError, Resource,
    parse_manifest,
};
pub use request::{ExtraValue, ResourcePath, TransportUrl, TransportUrlError};
pub use types::{ContentType, ResourceName};

/// Errors for a response that cannot be interpreted at all.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ResponseError {
    #[error("response is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("response is not a JSON object")]
    NotAnObject,
    #[error("response has no `{0}` field")]
    MissingField(&'static str),
}
