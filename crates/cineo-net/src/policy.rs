//! Network policy: which destinations Cineo may contact.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::Duration;

use url::{Host, Url};

/// Limits applied to every outbound request.
#[derive(Debug, Clone)]
pub struct NetPolicy {
    /// Allow loopback, private (RFC 1918), link-local and similar addresses.
    /// Off by default; turned on explicitly for self-hosted addons and tests.
    pub allow_private_networks: bool,
    /// Maximum decoded body size, in bytes.
    pub max_body_bytes: usize,
    /// Total time allowed for one request, including reading the body.
    pub timeout: Duration,
    pub connect_timeout: Duration,
    pub max_redirects: usize,
}

impl Default for NetPolicy {
    fn default() -> Self {
        Self {
            allow_private_networks: false,
            max_body_bytes: 8 * 1024 * 1024,
            timeout: Duration::from_secs(20),
            connect_timeout: Duration::from_secs(10),
            max_redirects: 5,
        }
    }
}

/// Why the policy refused a destination.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum BlockReason {
    #[error("scheme `{0}` is not allowed")]
    Scheme(String),
    #[error("address {0} is not public (enable private network access to allow it)")]
    NonPublicAddress(IpAddr),
    #[error("host `{0}` resolved to no allowed address")]
    NoAllowedAddress(String),
    #[error("redirect from https to http is not allowed")]
    Downgrade,
    #[error("too many redirects (limit {0})")]
    TooManyRedirects(usize),
}

impl NetPolicy {
    /// Checks a URL before connecting. Hostnames are checked again after DNS
    /// resolution (see `addon_client`); IP literals and `localhost` are
    /// checked here because they never reach the resolver.
    pub fn check_url(&self, url: &Url) -> Result<(), BlockReason> {
        if !matches!(url.scheme(), "http" | "https") {
            return Err(BlockReason::Scheme(url.scheme().to_owned()));
        }
        if self.allow_private_networks {
            return Ok(());
        }
        match url.host() {
            Some(Host::Ipv4(ip)) => self.check_ip(IpAddr::V4(ip)),
            Some(Host::Ipv6(ip)) => self.check_ip(IpAddr::V6(ip)),
            Some(Host::Domain(domain)) => {
                let domain = domain.trim_end_matches('.').to_ascii_lowercase();
                if domain == "localhost" || domain.ends_with(".localhost") {
                    Err(BlockReason::NonPublicAddress(IpAddr::V4(
                        Ipv4Addr::LOCALHOST,
                    )))
                } else {
                    Ok(())
                }
            }
            None => Err(BlockReason::NoAllowedAddress(String::new())),
        }
    }

    pub fn check_ip(&self, ip: IpAddr) -> Result<(), BlockReason> {
        if self.allow_private_networks || is_public_ip(ip) {
            Ok(())
        } else {
            Err(BlockReason::NonPublicAddress(ip))
        }
    }

    /// Checks one redirect hop. `hops` is the number of redirects already
    /// followed before this one.
    pub fn check_redirect(&self, from: &Url, to: &Url, hops: usize) -> Result<(), BlockReason> {
        if hops >= self.max_redirects {
            return Err(BlockReason::TooManyRedirects(self.max_redirects));
        }
        if from.scheme() == "https" && to.scheme() == "http" {
            return Err(BlockReason::Downgrade);
        }
        self.check_url(to)
    }
}

/// Whether `ip` is a globally routable unicast address.
///
/// Conservative: anything special-purpose (loopback, private, link-local,
/// CGNAT, multicast, documentation, benchmarking, reserved, unspecified) is
/// not public. IPv6 addresses embedding IPv4 (mapped, NAT64, 6to4) are judged
/// by the embedded IPv4 address.
pub fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_public_v4(v4),
        IpAddr::V6(v6) => is_public_v6(v6),
    }
}

fn is_public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_multicast()
        || ip.is_documentation()
        || a == 0 // 0.0.0.0/8 "this network"
        || (a == 100 && (64..=127).contains(&b)) // 100.64.0.0/10 CGNAT
        || (a == 192 && b == 0 && c == 0) // 192.0.0.0/24 IETF protocol assignments
        || (a == 198 && (b == 18 || b == 19)) // 198.18.0.0/15 benchmarking
        || a >= 240) // 240.0.0.0/4 reserved
}

fn is_public_v6(ip: Ipv6Addr) -> bool {
    if let Some(v4) = ip.to_ipv4_mapped() {
        return is_public_v4(v4);
    }
    let seg = ip.segments();
    // 64:ff9b::/96 NAT64: judge the embedded IPv4 address.
    if seg[0] == 0x64 && seg[1] == 0xff9b && seg[2..6] == [0, 0, 0, 0] {
        return is_public_v4(embedded_v4(seg[6], seg[7]));
    }
    // 2002::/16 6to4: judge the embedded IPv4 address.
    if seg[0] == 0x2002 {
        return is_public_v4(embedded_v4(seg[1], seg[2]));
    }
    !(ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_multicast()
        || ip.is_unique_local()
        || ip.is_unicast_link_local()
        || (seg[0] == 0x2001 && seg[1] == 0x0db8) // 2001:db8::/32 documentation
        || (seg[0] == 0x2001 && seg[1] == 0) // 2001::/32 Teredo
        || seg[..6] == [0, 0, 0, 0, 0, 0]) // ::/96 deprecated IPv4-compatible
}

fn embedded_v4(hi: u16, lo: u16) -> Ipv4Addr {
    let [a, b] = hi.to_be_bytes();
    let [c, d] = lo.to_be_bytes();
    Ipv4Addr::new(a, b, c, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn non_public_addresses_are_rejected() {
        for s in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "0.0.0.0",
            "0.1.2.3",
            "255.255.255.255",
            "224.0.0.1",
            "198.18.0.1",
            "192.0.2.1",
            "240.0.0.1",
            "::",
            "::1",
            "fe80::1",
            "fc00::1",
            "fd12:3456::1",
            "ff02::1",
            "2001:db8::1",
            "::ffff:127.0.0.1",
            "::ffff:192.168.0.1",
            "64:ff9b::a00:1",
            "2002:c0a8:0101::1",
            "::127.0.0.1",
        ] {
            assert!(!is_public_ip(ip(s)), "{s} must not be public");
        }
    }

    #[test]
    fn public_addresses_are_accepted() {
        for s in [
            "1.1.1.1",
            "8.8.8.8",
            "93.184.216.34",
            "2606:4700::1111",
            "::ffff:8.8.8.8",
        ] {
            assert!(is_public_ip(ip(s)), "{s} must be public");
        }
    }

    #[test]
    fn check_url_blocks_literals_and_localhost_by_default() {
        let policy = NetPolicy::default();
        for url in [
            "http://127.0.0.1/manifest.json",
            "http://[::1]:8080/manifest.json",
            "http://localhost:7000/manifest.json",
            "http://foo.localhost/manifest.json",
            "http://2130706433/manifest.json", // 127.0.0.1 in decimal; `url` normalizes it
        ] {
            let url = Url::parse(url).unwrap();
            assert!(policy.check_url(&url).is_err(), "{url} must be blocked");
        }
        let url = Url::parse("file:///etc/passwd").unwrap();
        assert_eq!(
            policy.check_url(&url),
            Err(BlockReason::Scheme("file".into()))
        );
    }

    #[test]
    fn check_url_allows_private_when_enabled() {
        let policy = NetPolicy {
            allow_private_networks: true,
            ..NetPolicy::default()
        };
        let url = Url::parse("http://127.0.0.1:7000/manifest.json").unwrap();
        assert_eq!(policy.check_url(&url), Ok(()));
    }

    #[test]
    fn redirects_are_revalidated() {
        let policy = NetPolicy::default();
        let from = Url::parse("https://addon.example/manifest.json").unwrap();
        let to_private = Url::parse("http://192.168.0.1/").unwrap();
        let to_http = Url::parse("http://addon.example/x").unwrap();
        let to_ok = Url::parse("https://cdn.example/x").unwrap();
        assert_eq!(
            policy.check_redirect(&from, &to_http, 0),
            Err(BlockReason::Downgrade)
        );
        assert!(matches!(
            policy.check_redirect(&from, &to_private, 0),
            Err(BlockReason::Downgrade | BlockReason::NonPublicAddress(_))
        ));
        assert_eq!(policy.check_redirect(&from, &to_ok, 0), Ok(()));
        assert_eq!(
            policy.check_redirect(&from, &to_ok, 5),
            Err(BlockReason::TooManyRedirects(5))
        );
    }
}
