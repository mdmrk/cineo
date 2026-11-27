use std::net::SocketAddr;

use super::state::{Effect, State};
use crate::remote::{Peer, RemoteKey};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RemoteState {
    pub host: Option<RemoteKey>,
    pub address: Option<SocketAddr>,
    pub peer: Option<Peer>,
    pub synced: Option<Peer>,
}

impl RemoteState {
    pub fn announced(&self) -> Option<Peer> {
        self.host
            .zip(self.address)
            .map(|(key, address)| Peer { address, key })
    }

    pub fn computer(&self) -> Option<Peer> {
        self.peer.or(self.synced)
    }
}

pub(crate) fn listening(state: &mut State, address: Option<SocketAddr>) -> Vec<Effect> {
    state.remote.address = address;
    Vec::new()
}

pub(crate) fn restore(
    state: &mut State,
    host: Option<RemoteKey>,
    peer: Option<Peer>,
) -> Vec<Effect> {
    state.remote = RemoteState {
        host,
        peer,
        ..state.remote
    };
    host.map(|key| Effect::ServeRemote(Some(key)))
        .into_iter()
        .collect()
}

pub(crate) fn set_host(state: &mut State, host: Option<RemoteKey>) -> Vec<Effect> {
    if state.remote.host == host {
        return Vec::new();
    }
    state.remote.host = host;
    vec![Effect::SaveRemoteHost(host), Effect::ServeRemote(host)]
}

pub(crate) fn set_peer(state: &mut State, peer: Option<Peer>) -> Vec<Effect> {
    if state.remote.peer == peer {
        return Vec::new();
    }
    state.remote.peer = peer;
    vec![Effect::SaveRemotePeer(peer)]
}
