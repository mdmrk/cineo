use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};

use cineo_core::remote::{Computer, RemoteKey, is_lan};

use crate::FetchError;
use crate::sync::CodeError;
use crate::sync::code::{format_key, parse_key};

mod client;
mod keys;
mod server;

pub use client::RemoteClient;
pub use keys::{RemoteKeys, new_remote_key};
pub use server::{Handler, MAX_CONNECTIONS, RemoteServer, bind, serve};

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RemoteError {
    #[error("the computer's address is not on the local network")]
    NotLan,
    #[error("the computer could not be reached: {0}")]
    Fetch(#[from] FetchError),
    #[error("the computer does not know this key")]
    WrongKey,
    #[error("the computer refused the request's time")]
    Clock,
    #[error("the computer answered HTTP {0}")]
    Rejected(u16),
    #[error("the computer's answer was unreadable")]
    Malformed,
    #[error("encryption failed")]
    Crypto,
    #[error("the listener failed: {0}")]
    Io(#[from] std::io::Error),
}

pub fn format_remote_code(key: &RemoteKey) -> String {
    format_key(key.as_bytes())
}

pub fn parse_remote_code(text: &str) -> Result<RemoteKey, CodeError> {
    parse_key(text).map(RemoteKey::from_bytes)
}

pub fn remote_link(computer: &Computer) -> String {
    let encode =
        |text: &str| -> String { url::form_urlencoded::byte_serialize(text.as_bytes()).collect() };
    let mut link = format!(
        "cineo:///remote/{}?at={}",
        format_remote_code(&computer.peer.key),
        encode(&computer.peer.address.to_string())
    );
    if !computer.name.is_empty() {
        link.push_str("&name=");
        link.push_str(&encode(&computer.name));
    }
    link
}

pub fn lan_ip() -> Option<IpAddr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect(SocketAddr::from(([192, 0, 2, 1], 9))).ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (is_lan(ip) && !ip.is_loopback()).then_some(ip)
}
