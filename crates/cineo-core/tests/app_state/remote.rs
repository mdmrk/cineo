use cineo_core::remote::{Peer, RemoteKey};

use super::*;

fn peer() -> Peer {
    Peer {
        address: "192.168.1.20:18765".parse().unwrap(),
        key: RemoteKey::from_bytes([7; 16]),
    }
}

#[test]
fn a_restored_remote_key_starts_the_listener() {
    let mut state = State::default();
    let key = RemoteKey::from_bytes([1; 16]);
    let effects = update(
        &mut state,
        Action::RestoreRemote {
            host: Some(key),
            peer: Some(peer()),
        },
    );
    assert_eq!(effects, [Effect::ServeRemote(Some(key))]);
    assert_eq!(state.remote.host, Some(key));
    assert_eq!(state.remote.peer, Some(peer()));
    let none = update(
        &mut State::default(),
        Action::RestoreRemote {
            host: None,
            peer: None,
        },
    );
    assert_eq!(none, []);
}

#[test]
fn turning_the_remote_on_off_or_to_a_new_key_saves_and_restarts_it() {
    let mut state = State::default();
    let first = RemoteKey::from_bytes([1; 16]);
    assert_eq!(
        update(&mut state, Action::SetRemoteHost(Some(first))),
        [
            Effect::SaveRemoteHost(Some(first)),
            Effect::ServeRemote(Some(first))
        ]
    );
    assert_eq!(update(&mut state, Action::SetRemoteHost(Some(first))), []);
    let second = RemoteKey::from_bytes([2; 16]);
    assert_eq!(
        update(&mut state, Action::SetRemoteHost(Some(second))),
        [
            Effect::SaveRemoteHost(Some(second)),
            Effect::ServeRemote(Some(second))
        ]
    );
    assert_eq!(
        update(&mut state, Action::SetRemoteHost(None)),
        [Effect::SaveRemoteHost(None), Effect::ServeRemote(None)]
    );
    assert_eq!(state.remote.host, None);
}

#[test]
fn pairing_with_a_computer_is_saved_and_can_be_forgotten() {
    let mut state = State::default();
    assert_eq!(
        update(&mut state, Action::SetRemotePeer(Some(peer()))),
        [Effect::SaveRemotePeer(Some(peer()))]
    );
    assert_eq!(state.remote.peer, Some(peer()));
    assert_eq!(
        update(&mut state, Action::SetRemotePeer(None)),
        [Effect::SaveRemotePeer(None)]
    );
    assert_eq!(state.remote.peer, None);
}
