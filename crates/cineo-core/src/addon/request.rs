//! Addon transport URLs and resource request paths.

use std::fmt;

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use url::Url;

use super::types::{ContentType, ResourceName};

const URI_COMPONENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'!')
    .remove(b'~')
    .remove(b'*')
    .remove(b'\'')
    .remove(b'(')
    .remove(b')');

const MANIFEST_SUFFIX: &str = "/manifest.json";

/// The URL of an addon's `manifest.json`, the identity of an installed addon.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TransportUrl(Url);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum TransportUrlError {
    #[error("not a valid URL: {0}")]
    Invalid(#[from] url::ParseError),
    #[error("unsupported scheme `{0}` (only http and https addons are supported)")]
    UnsupportedScheme(String),
    #[error("URL has no host")]
    NoHost,
    #[error("URL must not contain credentials")]
    HasCredentials,
    #[error("URL must not contain a fragment")]
    HasFragment,
    #[error("URL path must end with /manifest.json")]
    NotAManifest,
}

impl TransportUrl {
    pub fn parse(input: &str) -> Result<Self, TransportUrlError> {
        Self::try_from(Url::parse(input.trim())?)
    }

    pub fn as_url(&self) -> &Url {
        &self.0
    }

    /// Builds the URL for `path`:
    /// `{base}/{resource}/{type}/{id}[/{extra}].json[?{query}]`, where `base`
    /// is the transport URL without the trailing `/manifest.json`.
    pub fn resource_url(&self, path: &ResourcePath) -> Url {
        let mut url = self.0.clone();
        let base = url
            .path()
            .strip_suffix(MANIFEST_SUFFIX)
            .unwrap_or_default()
            .to_owned();
        url.set_path(&format!("{base}/{}", path.to_url_path()));
        url
    }
}

impl TryFrom<Url> for TransportUrl {
    type Error = TransportUrlError;

    fn try_from(url: Url) -> Result<Self, Self::Error> {
        if !matches!(url.scheme(), "http" | "https") {
            return Err(TransportUrlError::UnsupportedScheme(
                url.scheme().to_owned(),
            ));
        }
        if url.host().is_none() {
            return Err(TransportUrlError::NoHost);
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(TransportUrlError::HasCredentials);
        }
        if url.fragment().is_some() {
            return Err(TransportUrlError::HasFragment);
        }
        if !url.path().ends_with(MANIFEST_SUFFIX) {
            return Err(TransportUrlError::NotAManifest);
        }
        Ok(Self(url))
    }
}

impl fmt::Display for TransportUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// One `name=value` extra argument, e.g. `search=matrix` or `skip=100`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExtraValue {
    pub name: String,
    pub value: String,
}

impl ExtraValue {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

/// A request for one addon resource, independent of which addon serves it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResourcePath {
    pub resource: ResourceName,
    pub content_type: ContentType,
    pub id: String,
    /// Order is preserved in the URL. Repeated names are allowed.
    pub extra: Vec<ExtraValue>,
}

impl ResourcePath {
    pub fn catalog(content_type: ContentType, id: impl Into<String>) -> Self {
        Self {
            resource: ResourceName::Catalog,
            content_type,
            id: id.into(),
            extra: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_extra(mut self, extra: Vec<ExtraValue>) -> Self {
        self.extra = extra;
        self
    }

    /// `{resource}/{type}/{id}.json` or `{resource}/{type}/{id}/{extra}.json`,
    /// each component encoded like `encodeURIComponent`. The extra segment is
    /// `name=value` pairs joined by `&`, with `=` and `&` left literal.
    pub fn to_url_path(&self) -> String {
        let enc = |s: &str| utf8_percent_encode(s, URI_COMPONENT).to_string();
        let mut out = format!(
            "{}/{}/{}",
            enc(self.resource.as_str()),
            enc(self.content_type.as_str()),
            enc(&self.id)
        );
        if !self.extra.is_empty() {
            let extra = self
                .extra
                .iter()
                .map(|e| format!("{}={}", enc(&e.name), enc(&e.value)))
                .collect::<Vec<_>>()
                .join("&");
            out.push('/');
            out.push_str(&extra);
        }
        out.push_str(".json");
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ty(s: &str) -> ContentType {
        ContentType::new(s).unwrap()
    }

    fn url_for(transport: &str, path: &ResourcePath) -> String {
        TransportUrl::parse(transport)
            .unwrap()
            .resource_url(path)
            .to_string()
    }

    #[test]
    fn builds_plain_resource_url() {
        let path = ResourcePath::catalog(ty("movie"), "top");
        assert_eq!(
            url_for("https://addon.example/manifest.json", &path),
            "https://addon.example/catalog/movie/top.json"
        );
    }

    #[test]
    fn keeps_path_prefix_and_query() {
        let path = ResourcePath::catalog(ty("movie"), "top");
        assert_eq!(
            url_for("https://addon.example/cfg%7B1%7D/manifest.json?k=v", &path),
            "https://addon.example/cfg%7B1%7D/catalog/movie/top.json?k=v"
        );
    }

    #[test]
    fn encodes_extra_like_encode_uri_component() {
        let path = ResourcePath::catalog(ty("series"), "top").with_extra(vec![
            ExtraValue::new("search", "game of thrones"),
            ExtraValue::new("skip", "100"),
        ]);
        assert_eq!(
            path.to_url_path(),
            "catalog/series/top/search=game%20of%20thrones&skip=100.json"
        );
    }

    #[test]
    fn encodes_reserved_and_unicode_characters() {
        let path = ResourcePath {
            resource: ResourceName::Stream,
            content_type: ty("series"),
            id: "tt0944947:1:2".to_owned(),
            extra: vec![ExtraValue::new("q", "a/b&c=d Amélie (x)!*'~")],
        };
        assert_eq!(
            path.to_url_path(),
            "stream/series/tt0944947%3A1%3A2/q=a%2Fb%26c%3Dd%20Am%C3%A9lie%20(x)!*'~.json"
        );
    }

    #[test]
    fn rejects_unusable_transport_urls() {
        use TransportUrlError as E;
        let err = |s: &str| TransportUrl::parse(s).unwrap_err();
        assert_eq!(
            err("ftp://a.example/manifest.json"),
            E::UnsupportedScheme("ftp".into())
        );
        assert_eq!(
            err("file:///manifest.json"),
            E::UnsupportedScheme("file".into())
        );
        assert_eq!(
            err("https://u:p@a.example/manifest.json"),
            E::HasCredentials
        );
        assert_eq!(err("https://a.example/manifest.json#x"), E::HasFragment);
        assert_eq!(err("https://a.example/stremio/v1"), E::NotAManifest);
        assert!(matches!(err("not a url"), E::Invalid(_)));
        assert!(TransportUrl::parse("  http://a.example/x/manifest.json ").is_ok());
    }
}
