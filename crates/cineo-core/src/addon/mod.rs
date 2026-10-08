//! The addon protocol model.

mod catalog;
mod json;
mod manifest;
mod meta;
mod request;
mod stream;
mod types;

pub use catalog::{CatalogResponse, MetaPreview, PosterShape, parse_catalog_response};
pub use manifest::{
    BehaviorHints, CatalogDef, ExtraError, ExtraProp, IdFilter, Manifest, ManifestError, Resource,
    parse_manifest,
};
pub use meta::{Meta, Video, parse_meta_response};
pub use request::{ExtraValue, ResourcePath, TransportUrl, TransportUrlError};
pub use stream::{
    ArchiveKind, SourceKind, Stream, StreamSource, Subtitle, is_safe_header, parse_stream_json,
    parse_stream_response, parse_subtitles_response, stream_to_json,
};
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
