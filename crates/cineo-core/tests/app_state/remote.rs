use cineo_core::remote::{Choice, Computer, Peer, RemoteKey};

use super::*;

fn peer() -> Computer {
    Computer {
        name: "Study".into(),
        peer: Peer {
            address: "192.168.1.20:18765".parse().unwrap(),
            key: RemoteKey::from_bytes([7; 16]),
        },
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
            choice: Some(Choice::Synced(4)),
        },
    );
    assert_eq!(effects, [Effect::ServeRemote(Some(key))]);
    assert_eq!(state.remote.host, Some(key));
    assert_eq!(state.remote.peer, Some(peer()));
    assert_eq!(state.remote.choice, Some(Choice::Synced(4)));
    let none = update(
        &mut State::default(),
        Action::RestoreRemote {
            host: None,
            peer: None,
            choice: None,
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
fn pairing_with_a_computer_chooses_it_and_forgetting_it_drops_the_choice() {
    let mut state = State::default();
    assert_eq!(
        update(&mut state, Action::SetRemotePeer(Some(peer()))),
        [
            Effect::SaveRemotePeer(Some(peer())),
            Effect::SaveRemoteChoice(Some(Choice::Paired))
        ]
    );
    assert_eq!(state.remote.computer(), Some(&peer()));
    assert_eq!(
        update(&mut state, Action::SetRemotePeer(None)),
        [Effect::SaveRemotePeer(None), Effect::SaveRemoteChoice(None)]
    );
    assert_eq!(state.remote.computer(), None);
}

fn synced(byte: u8, name: &str) -> Computer {
    Computer {
        name: name.into(),
        peer: Peer {
            address: format!("192.168.1.{byte}:18765").parse().unwrap(),
            key: RemoteKey::from_bytes([byte; 16]),
        },
    }
}

#[test]
fn one_computer_at_a_time_the_chosen_one_else_the_paired_else_the_first() {
    let mut remote = cineo_core::app::RemoteState {
        synced: vec![(2, synced(2, "Living room")), (3, synced(3, "Study"))],
        ..cineo_core::app::RemoteState::default()
    };
    assert_eq!(remote.active(), Some(Choice::Synced(2)), "the first");
    remote.choice = Some(Choice::Synced(3));
    assert_eq!(remote.computer(), Some(&synced(3, "Study")));
    remote.peer = Some(peer());
    remote.choice = Some(Choice::Paired);
    assert_eq!(remote.computer(), Some(&peer()));
    remote.choice = Some(Choice::Synced(2));
    assert_eq!(remote.computer(), Some(&synced(2, "Living room")));
    remote.synced.remove(0);
    assert_eq!(
        remote.active(),
        Some(Choice::Paired),
        "a chosen computer that is gone falls back to the paired one"
    );
    remote.peer = None;
    assert_eq!(remote.active(), Some(Choice::Synced(3)));
}

#[test]
fn choosing_a_computer_is_saved() {
    let mut state = State::default();
    assert_eq!(
        update(&mut state, Action::ChooseComputer(Choice::Synced(3))),
        [Effect::SaveRemoteChoice(Some(Choice::Synced(3)))]
    );
    assert_eq!(
        update(&mut state, Action::ChooseComputer(Choice::Synced(3))),
        []
    );
}

#[test]
fn the_desktop_announces_its_name_with_its_address() {
    let mut state = State::default();
    update(
        &mut state,
        Action::SetRemoteHost(Some(RemoteKey::from_bytes([1; 16]))),
    );
    update(
        &mut state,
        Action::RemoteListening {
            address: Some("192.168.1.9:18765".parse().unwrap()),
            name: "Study".into(),
        },
    );
    let announced = state.remote.announced().unwrap();
    assert_eq!(announced.name, "Study");
    assert_eq!(announced.peer.address, "192.168.1.9:18765".parse().unwrap());
}

fn sent_from_the_phone(stream: usize) -> cineo_core::app::SentPlay {
    let phone = detail_with_streams();
    let sent = cineo_core::app::sent_play(&phone, 0, stream).unwrap();
    let request = cineo_core::remote::Request {
        time_ms: 1,
        command: Some(cineo_core::remote::Command::Play(Box::new(sent))),
    };
    match cineo_core::remote::Request::parse(request.to_json().as_bytes()) {
        Some(cineo_core::remote::Request {
            command: Some(cineo_core::remote::Command::Play(sent)),
            ..
        }) => *sent,
        other => panic!("the play command did not survive the wire: {other:?}"),
    }
}

#[test]
fn a_stream_sent_from_the_phone_plays_on_the_computer() {
    let sent = sent_from_the_phone(0);
    assert_eq!(sent.meta_id, "tt0000001");
    let (mut computer, _) = restored();
    let effects = update(&mut computer, Action::PlaySent(Box::new(sent.clone())));
    let play = play_effect(&effects).expect("the computer plays it");
    assert_eq!(play.meta_id, "tt0000001");
    assert_eq!(play.title, sent.saved.title);
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::SaveLibraryItem(i) if i.id == "tt0000001")),
        "it joins the computer's library"
    );
    assert_eq!(
        computer.playing.as_ref().map(|p| p.meta_id.as_str()),
        Some("tt0000001")
    );
}

#[test]
fn a_torrent_sent_from_the_phone_waits_for_the_computers_p2p_consent() {
    let sent = sent_from_the_phone(1);
    let (mut computer, _) = restored();
    assert!(update(&mut computer, Action::PlaySent(Box::new(sent.clone()))).is_empty());
    assert!(computer.p2p_sent.is_some());
    assert!(computer.torrent.is_none(), "nothing starts before consent");
    let effects = update(&mut computer, Action::AcceptP2p);
    assert!(
        effects.iter().any(|e| matches!(e, Effect::StartTorrent(_))),
        "{effects:?}"
    );
    assert!(computer.p2p_sent.is_none());

    let (mut declined, _) = restored();
    update(&mut declined, Action::PlaySent(Box::new(sent)));
    assert!(update(&mut declined, Action::DeclineP2p).is_empty());
    assert!(declined.p2p_sent.is_none());
}

#[test]
fn a_computer_with_torrents_off_refuses_a_sent_torrent() {
    let sent = sent_from_the_phone(1);
    let (mut computer, _) = restored();
    update(
        &mut computer,
        Action::RestoreSettings(Settings {
            p2p_enabled: false,
            ..Settings::default()
        }),
    );
    assert!(update(&mut computer, Action::PlaySent(Box::new(sent))).is_empty());
    assert_eq!(computer.notice, Some(Notice::TorrentsOff));
    assert!(computer.playing.is_none());
}

fn watched_on_the_phone() -> State {
    let mut phone = detail_with_streams();
    update(
        &mut phone,
        Action::Play {
            group: 0,
            stream: 0,
        },
    );
    update(
        &mut phone,
        Action::PlaybackProgress {
            meta_id: "tt0000001".into(),
            video_id: "tt0000001".into(),
            time_ms: 120_000,
            duration_ms: 600_000,
            now_ms: 5,
        },
    );
    update(&mut phone, Action::PlaybackStopped);
    phone
}

#[test]
fn continue_watching_sent_to_the_computer_resumes_where_the_phone_stopped() {
    let phone = watched_on_the_phone();
    let sent = cineo_core::app::sent_resume(&phone, "tt0000001").unwrap();
    assert_eq!(sent.offset_ms, 120_000);
    let request = cineo_core::remote::Request {
        time_ms: 1,
        command: Some(cineo_core::remote::Command::Play(Box::new(sent))),
    };
    let Some(cineo_core::remote::Request {
        command: Some(cineo_core::remote::Command::Play(sent)),
        ..
    }) = cineo_core::remote::Request::parse(request.to_json().as_bytes())
    else {
        panic!("the resume did not survive the wire");
    };
    let (mut computer, _) = restored();
    let effects = update(&mut computer, Action::PlaySent(sent.clone()));
    assert_eq!(play_effect(&effects).map(|p| p.start_ms), Some(120_000));

    let (mut ahead, _) = restored();
    update(&mut ahead, Action::PlaySent(sent.clone()));
    update(
        &mut ahead,
        Action::PlaybackProgress {
            meta_id: "tt0000001".into(),
            video_id: "tt0000001".into(),
            time_ms: 300_000,
            duration_ms: 600_000,
            now_ms: 9,
        },
    );
    update(&mut ahead, Action::PlaybackStopped);
    let effects = update(&mut ahead, Action::PlaySent(sent));
    assert_eq!(
        play_effect(&effects).map(|p| p.start_ms),
        Some(300_000),
        "the computer's own position wins"
    );
}

#[test]
fn only_an_item_with_a_saved_stream_can_be_resumed_on_the_computer() {
    let phone = watched_on_the_phone();
    assert!(cineo_core::app::sent_resume(&phone, "tt9999999").is_none());
    let mut without = phone;
    for item in &mut without.library {
        item.stream = None;
    }
    assert!(cineo_core::app::sent_resume(&without, "tt0000001").is_none());
}
