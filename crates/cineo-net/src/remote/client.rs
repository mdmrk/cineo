use std::time::Duration;

use cineo_core::remote::{Command, MAX_ANSWER_BYTES, PATH, Peer, Request, Status, is_lan};
use reqwest::StatusCode;
use url::Url;

use super::{RemoteError, RemoteKeys};
use crate::{AddonClient, NetPolicy};

const TIMEOUT: Duration = Duration::from_secs(5);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug)]
pub struct RemoteClient {
    http: AddonClient,
    keys: RemoteKeys,
    url: Url,
}

impl RemoteClient {
    pub fn new(peer: &Peer) -> Result<Self, RemoteError> {
        if !is_lan(peer.address.ip()) {
            return Err(RemoteError::NotLan);
        }
        let http = AddonClient::new(NetPolicy {
            allow_private_networks: true,
            max_body_bytes: MAX_ANSWER_BYTES,
            timeout: TIMEOUT,
            connect_timeout: CONNECT_TIMEOUT,
            max_redirects: 0,
        })?;
        let url = Url::parse(&format!("http://{}{PATH}", peer.address))
            .map_err(|_| RemoteError::NotLan)?;
        Ok(Self {
            http,
            keys: RemoteKeys::derive(&peer.key)?,
            url,
        })
    }

    pub async fn send(&self, command: Option<Command>, now_ms: u64) -> Result<Status, RemoteError> {
        let request = Request {
            time_ms: now_ms,
            command,
        };
        let (nonce, body) = self.keys.seal_request(request.to_json().as_bytes())?;
        let (status, answer) = self
            .http
            .post(self.url.clone(), body, MAX_ANSWER_BYTES)
            .await?;
        match status {
            StatusCode::OK => {}
            StatusCode::UNAUTHORIZED => return Err(RemoteError::WrongKey),
            StatusCode::FORBIDDEN => return Err(RemoteError::Clock),
            other => return Err(RemoteError::Rejected(other.as_u16())),
        }
        let plain = self
            .keys
            .open_answer(&nonce, &answer)
            .ok_or(RemoteError::Malformed)?;
        Status::parse(&plain).ok_or(RemoteError::Malformed)
    }
}
