//! Small protocol value types with invariants.

use std::fmt;

/// A content type such as `movie`, `series`, `channel` or `tv`.
///
/// The protocol allows arbitrary types; the well-known ones have constants.
/// Invariant: non-empty and free of surrounding whitespace.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ContentType(String);

impl ContentType {
    pub const MOVIE: &'static str = "movie";
    pub const SERIES: &'static str = "series";
    pub const CHANNEL: &'static str = "channel";
    pub const TV: &'static str = "tv";

    /// Returns `None` if `value` is empty or has surrounding whitespace.
    pub fn new(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        if value.is_empty() || value.trim() != value {
            return None;
        }
        Some(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ContentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// An addon resource name. Unknown names are preserved, not rejected, so that
/// manifests using newer resources still load.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ResourceName {
    Catalog,
    Meta,
    Stream,
    Subtitles,
    AddonCatalog,
    Other(String),
}

impl ResourceName {
    pub fn parse(value: &str) -> Self {
        match value {
            "catalog" => Self::Catalog,
            "meta" => Self::Meta,
            "stream" => Self::Stream,
            "subtitles" => Self::Subtitles,
            "addon_catalog" => Self::AddonCatalog,
            other => Self::Other(other.to_owned()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Catalog => "catalog",
            Self::Meta => "meta",
            Self::Stream => "stream",
            Self::Subtitles => "subtitles",
            Self::AddonCatalog => "addon_catalog",
            Self::Other(other) => other,
        }
    }
}

impl fmt::Display for ResourceName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
