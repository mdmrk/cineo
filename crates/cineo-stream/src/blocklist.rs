//! Non-public address ranges that peer connections and trackers may not use
//! unless the user allowed private networks (ADR-0012, SECURITY.md
//! §Streaming engine). Enforced by the SOCKS proxy ([`crate::socks`]) and
//! the tracker filter.
//!
//! The list mirrors `cineo-net`'s `is_public_ip` (ADR-0006). IO crates do
//! not depend on each other (ADR-0002), so it is repeated here; the
//! `addresses_match_the_net_policy_examples` test pins the same examples.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// IPv4 ranges that are not public, as (network, prefix length).
const V4: &[(Ipv4Addr, u8)] = &[
    (Ipv4Addr::new(0, 0, 0, 0), 8),       // "this network"
    (Ipv4Addr::new(10, 0, 0, 0), 8),      // private
    (Ipv4Addr::new(100, 64, 0, 0), 10),   // CGNAT
    (Ipv4Addr::new(127, 0, 0, 0), 8),     // loopback
    (Ipv4Addr::new(169, 254, 0, 0), 16),  // link-local
    (Ipv4Addr::new(172, 16, 0, 0), 12),   // private
    (Ipv4Addr::new(192, 0, 0, 0), 24),    // IETF protocol assignments
    (Ipv4Addr::new(192, 0, 2, 0), 24),    // documentation
    (Ipv4Addr::new(192, 168, 0, 0), 16),  // private
    (Ipv4Addr::new(198, 18, 0, 0), 15),   // benchmarking
    (Ipv4Addr::new(198, 51, 100, 0), 24), // documentation
    (Ipv4Addr::new(203, 0, 113, 0), 24),  // documentation
    (Ipv4Addr::new(224, 0, 0, 0), 4),     // multicast
    (Ipv4Addr::new(240, 0, 0, 0), 4),     // reserved, broadcast
];

/// IPv6 ranges that are not public. Ranges embedding IPv4 addresses
/// (mapped, NAT64, 6to4) are added per IPv4 range in [`v6_ranges`].
const V6: &[(Ipv6Addr, u8)] = &[
    (Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0), 96), // unspecified, loopback, v4-compatible
    (Ipv6Addr::new(0xfc00, 0, 0, 0, 0, 0, 0, 0), 7), // unique local
    (Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 0), 10), // link-local
    (Ipv6Addr::new(0xff00, 0, 0, 0, 0, 0, 0, 0), 8), // multicast
    (Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0), 32), // documentation
    (Ipv6Addr::new(0x2001, 0, 0, 0, 0, 0, 0, 0), 32), // Teredo
];

fn v4_bounds((net, prefix): (Ipv4Addr, u8)) -> (u32, u32) {
    let mask = u32::MAX.checked_shl(32 - u32::from(prefix)).unwrap_or(0);
    let start = net.to_bits() & mask;
    (start, start | !mask)
}

fn v6_bounds((net, prefix): (Ipv6Addr, u8)) -> (u128, u128) {
    let mask = u128::MAX.checked_shl(128 - u32::from(prefix)).unwrap_or(0);
    let start = net.to_bits() & mask;
    (start, start | !mask)
}

/// Every blocked IPv6 range: [`V6`] plus each IPv4 range embedded as
/// IPv4-mapped (`::ffff:a.b.c.d`), NAT64 (`64:ff9b::a.b.c.d`) and 6to4
/// (`2002:aabb:ccdd::/48`).
fn v6_ranges() -> Vec<(u128, u128)> {
    let mut out: Vec<(u128, u128)> = V6.iter().copied().map(v6_bounds).collect();
    for &range in V4 {
        let (start, end) = v4_bounds(range);
        let (start, end) = (u128::from(start), u128::from(end));
        let mapped = 0xffff_u128 << 32;
        let nat64 = 0x0064_ff9b_u128 << 96;
        out.push((mapped | start, mapped | end));
        out.push((nat64 | start, nat64 | end));
        let six_to_four = 0x2002_u128 << 112;
        out.push((
            six_to_four | (start << 80),
            six_to_four | (end << 80) | ((1_u128 << 80) - 1),
        ));
    }
    out
}

/// Whether BitTorrent traffic to `ip` is blocked by default.
pub(crate) fn is_blocked(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let bits = v4.to_bits();
            V4.iter()
                .map(|&r| v4_bounds(r))
                .any(|(start, end)| (start..=end).contains(&bits))
        }
        IpAddr::V6(v6) => {
            let bits = v6.to_bits();
            v6_ranges()
                .into_iter()
                .any(|(start, end)| (start..=end).contains(&bits))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap_or_else(|e| panic!("{s}: {e}"))
    }

    #[test]
    fn addresses_match_the_net_policy_examples() {
        for blocked in [
            "0.1.2.3",
            "10.0.0.1",
            "100.64.0.1",
            "127.0.0.1",
            "169.254.169.254",
            "172.16.0.1",
            "172.31.255.255",
            "192.0.0.8",
            "192.168.1.1",
            "198.18.0.1",
            "224.0.0.251",
            "255.255.255.255",
            "::",
            "::1",
            "fd00::1",
            "fe80::1",
            "ff02::1",
            "2001:db8::1",
            "::ffff:192.168.1.1",
            "64:ff9b::10.0.0.1",
            "2002:c0a8:0101::1",
        ] {
            assert!(is_blocked(ip(blocked)), "{blocked} should be blocked");
        }
        for public in [
            "1.1.1.1",
            "8.8.8.8",
            "172.32.0.1",
            "100.128.0.1",
            "2606:4700::1111",
            "::ffff:8.8.8.8",
            "2002:0808:0808::1",
        ] {
            assert!(!is_blocked(ip(public)), "{public} should be allowed");
        }
    }
}
