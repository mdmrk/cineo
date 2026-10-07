//! A loopback SOCKS5 proxy (RFC 1928, with RFC 1929 username/password)
//! that every peer connection goes through. It is the one place where peer
//! addresses are checked: `librqbit`'s own blocklist does not cover the
//! connections made while fetching a magnet's metadata (VERIFIED in its
//! 9.0.1 source and by `loopback_peers_are_blocked_unless_private_networks_are_allowed`).

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::debug;

use crate::blocklist;

const VERSION: u8 = 5;
const USER_PASS: u8 = 2;
const NO_METHOD: u8 = 0xff;
const CONNECT: u8 = 1;
const ATYP_V4: u8 = 1;
const ATYP_V6: u8 = 4;

const REPLY_OK: u8 = 0;
const REPLY_FAILURE: u8 = 1;
const REPLY_NOT_ALLOWED: u8 = 2;
const REPLY_HOST_UNREACHABLE: u8 = 4;
const REPLY_COMMAND_UNSUPPORTED: u8 = 7;
const REPLY_ADDRESS_UNSUPPORTED: u8 = 8;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug)]
pub(crate) struct Policy {
    pub(crate) user: String,
    pub(crate) password: String,
    pub(crate) allow_private_network: bool,
}

impl Policy {
    fn allows(&self, ip: IpAddr) -> bool {
        self.allow_private_network || !blocklist::is_blocked(ip)
    }
}

pub(crate) async fn run(listener: TcpListener, policy: Arc<Policy>) {
    loop {
        let Ok((client, _)) = listener.accept().await else {
            continue;
        };
        let policy = Arc::clone(&policy);
        tokio::spawn(async move {
            match tokio::time::timeout(HANDSHAKE_TIMEOUT, handshake(client, &policy)).await {
                Ok(Ok(Some((mut client, mut peer)))) => {
                    let _ = tokio::io::copy_bidirectional(&mut client, &mut peer).await;
                }
                Ok(Ok(None)) => {}
                Ok(Err(err)) => debug!(%err, "SOCKS handshake failed"),
                Err(_) => debug!("SOCKS handshake timed out"),
            }
        });
    }
}

fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0, |acc, (x, y)| acc | (x ^ y)) == 0
}

async fn reply(client: &mut TcpStream, code: u8) -> std::io::Result<()> {
    client
        .write_all(&[VERSION, code, 0, ATYP_V4, 0, 0, 0, 0, 0, 0])
        .await
}

async fn handshake(
    mut client: TcpStream,
    policy: &Policy,
) -> std::io::Result<Option<(TcpStream, TcpStream)>> {
    let mut head = [0u8; 2];
    client.read_exact(&mut head).await?;
    if head[0] != VERSION {
        return Ok(None);
    }
    let mut methods = vec![0u8; usize::from(head[1])];
    client.read_exact(&mut methods).await?;
    if !methods.contains(&USER_PASS) {
        client.write_all(&[VERSION, NO_METHOD]).await?;
        return Ok(None);
    }
    client.write_all(&[VERSION, USER_PASS]).await?;

    let mut ver_len = [0u8; 2];
    client.read_exact(&mut ver_len).await?;
    let mut user = vec![0u8; usize::from(ver_len[1])];
    client.read_exact(&mut user).await?;
    let mut plen = [0u8; 1];
    client.read_exact(&mut plen).await?;
    let mut password = vec![0u8; usize::from(plen[0])];
    client.read_exact(&mut password).await?;
    let authorized = ver_len[0] == 1
        && same(&user, policy.user.as_bytes())
        && same(&password, policy.password.as_bytes());
    client.write_all(&[1, u8::from(!authorized)]).await?;
    if !authorized {
        return Ok(None);
    }

    let mut request = [0u8; 4];
    client.read_exact(&mut request).await?;
    let ip = match request[3] {
        ATYP_V4 => {
            let mut octets = [0u8; 4];
            client.read_exact(&mut octets).await?;
            IpAddr::V4(Ipv4Addr::from(octets))
        }
        ATYP_V6 => {
            let mut octets = [0u8; 16];
            client.read_exact(&mut octets).await?;
            IpAddr::V6(Ipv6Addr::from(octets))
        }
        _ => {
            reply(&mut client, REPLY_ADDRESS_UNSUPPORTED).await?;
            return Ok(None);
        }
    };
    let mut port = [0u8; 2];
    client.read_exact(&mut port).await?;
    let target = SocketAddr::new(ip, u16::from_be_bytes(port));
    if request[0] != VERSION || request[1] != CONNECT {
        reply(&mut client, REPLY_COMMAND_UNSUPPORTED).await?;
        return Ok(None);
    }
    if !policy.allows(ip) {
        debug!("refused a peer on a non-public address");
        reply(&mut client, REPLY_NOT_ALLOWED).await?;
        return Ok(None);
    }
    match tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(target)).await {
        Ok(Ok(peer)) => {
            reply(&mut client, REPLY_OK).await?;
            Ok(Some((client, peer)))
        }
        Ok(Err(_)) => {
            reply(&mut client, REPLY_FAILURE).await?;
            Ok(None)
        }
        Err(_) => {
            reply(&mut client, REPLY_HOST_UNREACHABLE).await?;
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn proxy(allow_private_network: bool) -> SocketAddr {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap_or_else(|e| panic!("{e}"));
        let addr = listener.local_addr().unwrap_or_else(|e| panic!("{e}"));
        tokio::spawn(run(
            listener,
            Arc::new(Policy {
                user: "u".into(),
                password: "secret".into(),
                allow_private_network,
            }),
        ));
        addr
    }

    async fn connect(proxy: SocketAddr, password: &[u8], target: SocketAddr) -> u8 {
        let mut s = TcpStream::connect(proxy)
            .await
            .unwrap_or_else(|e| panic!("{e}"));
        s.write_all(&[5, 1, USER_PASS])
            .await
            .unwrap_or_else(|e| panic!("{e}"));
        let mut choice = [0u8; 2];
        s.read_exact(&mut choice)
            .await
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(choice, [5, USER_PASS]);
        let mut auth = vec![1, 1, b'u', u8::try_from(password.len()).unwrap_or(0)];
        auth.extend_from_slice(password);
        s.write_all(&auth).await.unwrap_or_else(|e| panic!("{e}"));
        let mut status = [0u8; 2];
        s.read_exact(&mut status)
            .await
            .unwrap_or_else(|e| panic!("{e}"));
        if status[1] != 0 {
            return 0xa0 | status[1];
        }
        let IpAddr::V4(ip) = target.ip() else {
            panic!("v4 only in this helper")
        };
        let mut req = vec![5, CONNECT, 0, ATYP_V4];
        req.extend_from_slice(&ip.octets());
        req.extend_from_slice(&target.port().to_be_bytes());
        s.write_all(&req).await.unwrap_or_else(|e| panic!("{e}"));
        let mut answer = [0u8; 10];
        s.read_exact(&mut answer)
            .await
            .unwrap_or_else(|e| panic!("{e}"));
        answer[1]
    }

    async fn local_target() -> SocketAddr {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap_or_else(|e| panic!("{e}"));
        let addr = listener.local_addr().unwrap_or_else(|e| panic!("{e}"));
        tokio::spawn(async move { while listener.accept().await.is_ok() {} });
        addr
    }

    #[tokio::test]
    async fn non_public_targets_are_refused_by_default() {
        let target = local_target().await;
        assert_eq!(
            connect(proxy(false).await, b"secret", target).await,
            REPLY_NOT_ALLOWED
        );
    }

    #[tokio::test]
    async fn private_targets_connect_when_allowed() {
        let target = local_target().await;
        assert_eq!(
            connect(proxy(true).await, b"secret", target).await,
            REPLY_OK
        );
    }

    #[tokio::test]
    async fn wrong_credentials_are_refused() {
        let target = local_target().await;
        assert_eq!(connect(proxy(true).await, b"guess", target).await, 0xa0 | 1);
    }
}
