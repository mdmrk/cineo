//! HTTP client for addon resources.

use std::error::Error as StdError;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use cineo_core::addon::{
    CatalogResponse, Manifest, ManifestError, ResourcePath, ResponseError, TransportUrl,
    parse_catalog_response, parse_manifest,
};
use cineo_core::diagnostics::Parsed;
use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use reqwest::{StatusCode, redirect};
use tracing::{Instrument, debug, info_span, warn};
use url::Url;

use crate::policy::{BlockReason, NetPolicy};

const USER_AGENT: &str = concat!("Cineo/", env!("CARGO_PKG_VERSION"));

/// Fetches and parses addon resources under a [`NetPolicy`].
#[derive(Debug, Clone)]
pub struct AddonClient {
    http: reqwest::Client,
    policy: Arc<NetPolicy>,
}

/// Why an addon request failed. Every variant is safe to show to users: none
/// contains the full addon URL, which may embed user configuration.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FetchError {
    #[error("blocked by network policy: {0}")]
    Blocked(#[from] BlockReason),
    #[error("request timed out")]
    Timeout,
    #[error("could not connect: {0}")]
    Connect(String),
    #[error("addon returned HTTP {0}")]
    Status(StatusCode),
    #[error("response exceeds the {limit}-byte limit")]
    TooLarge { limit: usize },
    #[error("transport error: {0}")]
    Transport(String),
    #[error("invalid manifest: {0}")]
    Manifest(#[from] ManifestError),
    #[error("invalid response: {0}")]
    Response(#[from] ResponseError),
}

impl AddonClient {
    pub fn new(policy: NetPolicy) -> Result<Self, FetchError> {
        let policy = Arc::new(policy);
        let redirect_policy = {
            let policy = Arc::clone(&policy);
            redirect::Policy::custom(move |attempt| {
                let hops = attempt.previous().len().saturating_sub(1);
                let from = attempt.previous().last().cloned();
                let checked = match &from {
                    Some(from) => policy.check_redirect(from, attempt.url(), hops),
                    None => policy.check_url(attempt.url()),
                };
                match checked {
                    Ok(()) => attempt.follow(),
                    Err(reason) => attempt.error(reason),
                }
            })
        };
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(policy.timeout)
            .connect_timeout(policy.connect_timeout)
            .redirect(redirect_policy)
            // A proxy would resolve hostnames itself and bypass the resolver check.
            .no_proxy()
            .dns_resolver(Arc::new(PolicyResolver {
                policy: Arc::clone(&policy),
            }))
            .build()
            .map_err(|err| FetchError::Transport(error_chain(&err)))?;
        Ok(Self { http, policy })
    }

    pub fn policy(&self) -> &NetPolicy {
        &self.policy
    }

    /// Fetches and validates `manifest.json`.
    pub async fn fetch_manifest(
        &self,
        addon: &TransportUrl,
    ) -> Result<Parsed<Manifest>, FetchError> {
        let span = info_span!("addon_request", req = next_request_id(), addon = %origin(addon.as_url()), resource = "manifest");
        async {
            let body = self.get(addon.as_url().clone()).await?;
            let parsed = parse_manifest(&body)?;
            log_warnings(&parsed.warnings);
            Ok(parsed)
        }
        .instrument(span)
        .await
    }

    /// Fetches a catalog page. The caller decides whether the addon supports
    /// the path (see `Manifest::supports`); this method only transports.
    pub async fn fetch_catalog(
        &self,
        addon: &TransportUrl,
        path: &ResourcePath,
    ) -> Result<Parsed<CatalogResponse>, FetchError> {
        let span = info_span!("addon_request", req = next_request_id(), addon = %origin(addon.as_url()), resource = %path.to_url_path());
        async {
            let body = self.get(addon.resource_url(path)).await?;
            let parsed = parse_catalog_response(&body)?;
            log_warnings(&parsed.warnings);
            Ok(parsed)
        }
        .instrument(span)
        .await
    }

    /// GETs `url` and returns the decoded body, enforcing the size limit.
    async fn get(&self, url: Url) -> Result<Vec<u8>, FetchError> {
        self.policy.check_url(&url)?;
        let limit = self.policy.max_body_bytes;
        let mut response = self
            .http
            .get(url)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await
            .map_err(|err| classify(&err))?;
        let status = response.status();
        debug!(%status, "response headers received");
        if !status.is_success() {
            return Err(FetchError::Status(status));
        }
        if response
            .content_length()
            .is_some_and(|len| len > limit as u64)
        {
            return Err(FetchError::TooLarge { limit });
        }
        // Count decoded bytes: this also bounds gzip/brotli expansion.
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|err| classify(&err))? {
            if body.len() + chunk.len() > limit {
                return Err(FetchError::TooLarge { limit });
            }
            body.extend_from_slice(&chunk);
        }
        debug!(bytes = body.len(), "response body received");
        Ok(body)
    }
}

/// Resolves hostnames and drops every address the policy forbids, so the
/// connection can only go to an address that was checked (no DNS rebinding).
#[derive(Debug)]
struct PolicyResolver {
    policy: Arc<NetPolicy>,
}

impl Resolve for PolicyResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let policy = Arc::clone(&self.policy);
        Box::pin(async move {
            let host = name.as_str().to_owned();
            let resolved: Vec<SocketAddr> =
                tokio::net::lookup_host((host.as_str(), 0)).await?.collect();
            let allowed: Vec<SocketAddr> = resolved
                .iter()
                .copied()
                .filter(|addr| policy.check_ip(addr.ip()).is_ok())
                .collect();
            if allowed.is_empty() {
                let reason = match resolved.first() {
                    Some(addr) => BlockReason::NonPublicAddress(addr.ip()),
                    None => BlockReason::NoAllowedAddress(host),
                };
                return Err(Box::new(reason) as Box<dyn StdError + Send + Sync>);
            }
            Ok(Box::new(allowed.into_iter()) as Addrs)
        })
    }
}

/// Maps a reqwest error to a [`FetchError`], surfacing policy blocks raised
/// inside the resolver or the redirect policy.
fn classify(err: &reqwest::Error) -> FetchError {
    let mut source: Option<&(dyn StdError + 'static)> = Some(err);
    while let Some(current) = source {
        if let Some(reason) = current.downcast_ref::<BlockReason>() {
            return FetchError::Blocked(reason.clone());
        }
        source = current.source();
    }
    if err.is_timeout() {
        FetchError::Timeout
    } else if err.is_connect() {
        FetchError::Connect(error_chain(err))
    } else {
        FetchError::Transport(error_chain(err))
    }
}

/// Renders an error and its sources without the request URL.
fn error_chain(err: &reqwest::Error) -> String {
    let mut out = match err.url() {
        Some(url) => err.to_string().replace(url.as_str(), "<url>"),
        None => err.to_string(),
    };
    let mut source = err.source();
    while let Some(current) = source {
        out.push_str(": ");
        out.push_str(&current.to_string());
        source = current.source();
    }
    out
}

/// `scheme://host[:port]` only: addon paths often embed user configuration
/// (API keys, tokens) and must not reach logs.
fn origin(url: &Url) -> String {
    url.origin().ascii_serialization()
}

fn next_request_id() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

fn log_warnings(warnings: &[cineo_core::diagnostics::Warning]) {
    for warning in warnings {
        warn!(%warning, "addon data problem");
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[tokio::test]
    async fn resolver_drops_non_public_addresses() {
        let resolver = PolicyResolver {
            policy: Arc::new(NetPolicy::default()),
        };
        // `localhost` resolves offline via the hosts file.
        let Err(err) = resolver.resolve(Name::from_str("localhost").unwrap()).await else {
            panic!("localhost must not resolve under the default policy");
        };
        assert!(matches!(
            err.downcast_ref::<BlockReason>(),
            Some(BlockReason::NonPublicAddress(_))
        ));
    }
}
