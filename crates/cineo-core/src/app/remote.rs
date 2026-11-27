use super::state::{Effect, State};
use crate::remote::{Peer, RemoteKey};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RemoteState {
    pub host: Option<RemoteKey>,
    pub peer: Option<Peer>,
}

pub(crate) fn restore(
    state: &mut State,
    host: Option<RemoteKey>,
    peer: Option<Peer>,
) -> Vec<Effect> {
    state.remote = RemoteState { host, peer };
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
