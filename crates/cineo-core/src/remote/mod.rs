use std::fmt;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

mod command;
mod replays;
mod status;

pub use command::{Command, MAX_SPEED, MAX_SUBTITLE_DELAY_S, MIN_SPEED, Request};
pub use replays::{MAX_REMEMBERED, Refusal, Replays};
pub use status::{MAX_TITLE_CHARS, MAX_TRACK_LABEL_CHARS, MAX_TRACKS, NowPlaying, Status, Track};

pub const PROTOCOL: u64 = 1;
pub const DEFAULT_PORT: u16 = 18_765;
pub const PATH: &str = "/v1/remote";
pub const MAX_REQUEST_BYTES: usize = 4 * 1024;
pub const MAX_ANSWER_BYTES: usize = 64 * 1024;
pub const MAX_CLOCK_SKEW_MS: u64 = 2 * 60 * 1000;
pub const NONCE_LEN: usize = 12;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct RemoteKey([u8; 16]);

impl RemoteKey {
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

impl fmt::Debug for RemoteKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RemoteKey(..)")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Peer {
    pub address: SocketAddr,
    pub key: RemoteKey,
}

pub fn is_lan(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_lan_v4(v4),
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_lan_v4(v4);
            }
            let first = v6.segments()[0];
            v6.is_loopback() || (first & 0xfe00) == 0xfc00 || (first & 0xffc0) == 0xfe80
        }
    }
}

fn is_lan_v4(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || (a == 100 && (64..=127).contains(&b))
}

pub fn parse_address(text: &str) -> Option<SocketAddr> {
    let text = text.trim();
    let address = text.parse::<SocketAddr>().ok().or_else(|| {
        text.trim_start_matches('[')
            .trim_end_matches(']')
            .parse::<IpAddr>()
            .ok()
            .map(|ip| SocketAddr::new(ip, DEFAULT_PORT))
    })?;
    (address.port() != 0 && is_lan(address.ip())).then_some(address)
}

pub(crate) fn clean_text(text: &str, max_chars: usize) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(max_chars)
        .collect::<String>()
        .trim()
        .to_owned()
}
