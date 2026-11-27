use std::net::SocketAddr;

use super::state::{Effect, State};
use crate::remote::{Choice, Computer, Peer, RemoteKey};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RemoteState {
    pub host: Option<RemoteKey>,
    pub address: Option<SocketAddr>,
    pub name: String,
    pub peer: Option<Computer>,
    pub synced: Vec<(u64, Computer)>,
    pub choice: Option<Choice>,
}

impl RemoteState {
    pub fn announced(&self) -> Option<Computer> {
        self.host.zip(self.address).map(|(key, address)| Computer {
            name: self.name.clone(),
            peer: Peer { address, key },
        })
    }

    pub fn active(&self) -> Option<Choice> {
        let synced = |device| self.synced.iter().any(|(d, _)| *d == device);
        match self.choice {
            Some(Choice::Synced(device)) if synced(device) => Some(Choice::Synced(device)),
            _ if self.peer.is_some() => Some(Choice::Paired),
            _ => self
                .synced
                .first()
                .map(|(device, _)| Choice::Synced(*device)),
        }
    }

    pub fn computer(&self) -> Option<&Computer> {
        match self.active()? {
            Choice::Paired => self.peer.as_ref(),
            Choice::Synced(device) => self
                .synced
                .iter()
                .find(|(d, _)| *d == device)
                .map(|(_, computer)| computer),
        }
    }
}

pub(crate) fn listening(
    state: &mut State,
    address: Option<SocketAddr>,
    name: String,
) -> Vec<Effect> {
    state.remote.address = address;
    state.remote.name = name;
    Vec::new()
}

pub(crate) fn restore(
    state: &mut State,
    host: Option<RemoteKey>,
    peer: Option<Computer>,
    choice: Option<Choice>,
) -> Vec<Effect> {
    state.remote.host = host;
    state.remote.peer = peer;
    state.remote.choice = choice;
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

pub(crate) fn set_peer(state: &mut State, peer: Option<Computer>) -> Vec<Effect> {
    if state.remote.peer == peer {
        return Vec::new();
    }
    let choice = match (&peer, state.remote.choice) {
        (Some(_), _) => Some(Choice::Paired),
        (None, Some(Choice::Paired)) => None,
        (None, other) => other,
    };
    state.remote.peer.clone_from(&peer);
    let mut effects = vec![Effect::SaveRemotePeer(peer)];
    if choice != state.remote.choice {
        state.remote.choice = choice;
        effects.push(Effect::SaveRemoteChoice(choice));
    }
    effects
}

pub(crate) fn choose(state: &mut State, choice: Choice) -> Vec<Effect> {
    if state.remote.choice == Some(choice) {
        return Vec::new();
    }
    state.remote.choice = Some(choice);
    vec![Effect::SaveRemoteChoice(Some(choice))]
}
