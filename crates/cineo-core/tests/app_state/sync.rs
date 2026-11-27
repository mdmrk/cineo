use std::collections::{BTreeMap, HashMap, VecDeque};

use cineo_core::app::{
    Action, Effect, LibraryItem, ManifestOrigin, PulledPage, PushedBatch, SeekStep, Setting, State,
    SyncFailure, SyncProblem, update,
};
use cineo_core::sync::{DeviceId, Pairing, Record, RecordName, SyncKey};

use super::{BASIC, STREAMS, ty, url};

const MINUTE: u64 = 60_000;

#[derive(Default)]
struct Space {
    records: BTreeMap<RecordName, (Record, u64)>,
    seq: u64,
    born: u64,
    deleted: bool,
}

#[derive(Default)]
struct FakeServer {
    spaces: HashMap<[u8; 16], Space>,
    busy: Option<u64>,
    next_born: u64,
}

impl FakeServer {
    fn space(&mut self, pairing: &Pairing) -> &mut Space {
        self.spaces.entry(*pairing.key.as_bytes()).or_default()
    }

    fn answer(&mut self, effect: &Effect) -> Option<Action> {
        if let Some(retry_after_s) = self.busy
            && matches!(effect, Effect::SyncPull { .. } | Effect::SyncPush { .. })
        {
            let failure = SyncFailure::Unavailable {
                retry_after_s: Some(retry_after_s),
            };
            return Some(match effect {
                Effect::SyncPull { pairing, .. } => Action::SyncPulled {
                    pairing: pairing.clone(),
                    result: Err(failure),
                },
                Effect::SyncPush { pairing, .. } => Action::SyncPushed {
                    pairing: pairing.clone(),
                    batch: PushedBatch {
                        failure: Some(failure),
                        ..PushedBatch::default()
                    },
                },
                _ => unreachable!(),
            });
        }
        match effect {
            Effect::SyncPull { pairing, since } => {
                let space = self.space(pairing);
                let result = if space.deleted {
                    Err(SyncFailure::SpaceGone)
                } else {
                    let mut records: Vec<(Record, u64)> = space
                        .records
                        .values()
                        .filter(|(_, seq)| seq > since)
                        .cloned()
                        .collect();
                    records.sort_by_key(|(_, seq)| *seq);
                    Ok(PulledPage {
                        records,
                        more: false,
                        space_seq: space.seq,
                        born: space.born,
                    })
                };
                Some(Action::SyncPulled {
                    pairing: pairing.clone(),
                    result,
                })
            }
            Effect::SyncPush { pairing, outgoing } => {
                self.next_born += 1;
                let born = self.next_born;
                let space = self.space(pairing);
                let mut batch = PushedBatch::default();
                if space.deleted {
                    batch.failure = Some(SyncFailure::SpaceGone);
                } else {
                    let conflicts: Vec<(RecordName, u64)> = outgoing
                        .iter()
                        .map(|o| {
                            let current = space.records.get(&o.record.name).map_or(0, |r| r.1);
                            (o.record.name.clone(), current, o.base_seq)
                        })
                        .filter(|(_, current, base)| current != base)
                        .map(|(name, current, _)| (name, current))
                        .collect();
                    if conflicts.is_empty() {
                        if space.born == 0 {
                            space.born = born;
                        }
                        for out in outgoing {
                            space.seq += 1;
                            space
                                .records
                                .insert(out.record.name.clone(), (out.record.clone(), space.seq));
                            batch.pushed.push((
                                out.record.name.clone(),
                                out.record.stamp,
                                space.seq,
                            ));
                        }
                    } else {
                        batch.failure = Some(SyncFailure::Conflict(conflicts));
                    }
                }
                Some(Action::SyncPushed {
                    pairing: pairing.clone(),
                    batch,
                })
            }
            Effect::DeleteSyncSpace(pairing) => {
                let space = self.space(pairing);
                space.records.clear();
                space.deleted = true;
                Some(Action::SyncSpaceDeleted(Ok(())))
            }
            _ => None,
        }
    }
}

fn drive(state: &mut State, server: &mut FakeServer, effects: Vec<Effect>) -> Vec<Effect> {
    let mut queue: VecDeque<Effect> = effects.into();
    let mut seen = Vec::new();
    while let Some(effect) = queue.pop_front() {
        if let Some(action) = server.answer(&effect) {
            queue.extend(update(state, action));
        }
        seen.push(effect);
    }
    seen
}

fn act(state: &mut State, server: &mut FakeServer, action: Action) -> Vec<Effect> {
    let effects = update(state, action);
    drive(state, server, effects)
}

fn pairing(key: u8) -> Pairing {
    Pairing {
        key: SyncKey::from_bytes([key; 16]),
        server: "https://sync.example/".parse().unwrap(),
    }
}

fn item(id: &str, offset_ms: u64) -> LibraryItem {
    LibraryItem {
        id: id.to_owned(),
        content_type: ty("movie"),
        name: format!("Name of {id}"),
        poster: None,
        video_id: id.to_owned(),
        time_offset_ms: offset_ms,
        duration_ms: 6_000_000,
        updated_ms: 1,
        favorited: None,
        stream: None,
    }
}

fn device(library: Vec<LibraryItem>, now_ms: u64) -> State {
    let mut state = State::default();
    update(
        &mut state,
        Action::Restore {
            addons: vec![url(BASIC)],
            library,
        },
    );
    update(&mut state, Action::Tick(now_ms));
    state
}

fn enable(state: &mut State, server: &mut FakeServer, key: u8, id: u64) -> Vec<Effect> {
    act(
        state,
        server,
        Action::EnableSync {
            pairing: pairing(key),
            device: DeviceId(id),
        },
    )
}

fn ids(state: &State) -> Vec<(String, u64)> {
    let mut ids: Vec<(String, u64)> = state
        .library
        .iter()
        .map(|i| (i.id.clone(), i.time_offset_ms))
        .collect();
    ids.sort();
    ids
}

fn pulls(effects: &[Effect]) -> usize {
    effects
        .iter()
        .filter(|e| matches!(e, Effect::SyncPull { .. }))
        .count()
}

#[test]
fn turning_sync_on_saves_the_pairing_and_pushes_what_is_here() {
    let mut server = FakeServer::default();
    let mut a = device(vec![item("tt1", 10)], 1_000);
    update(
        &mut a,
        Action::ChangeSetting(Setting::SeekStep(SeekStep::S30)),
    );
    let effects = enable(&mut a, &mut server, 1, 1);
    assert!(
        matches!(effects[0], Effect::SavePairing { .. }),
        "{effects:?}"
    );
    assert!(
        effects.iter().any(|e| matches!(e, Effect::SaveSync(_))),
        "the replica is saved"
    );
    let space = &server.spaces[&[1; 16]];
    assert!(space.records.contains_key(&RecordName::Item("tt1".into())));
    assert!(
        space
            .records
            .contains_key(&RecordName::Setting("seek_step".into()))
    );
    assert!(space.records.contains_key(&RecordName::Addons));
    let sync = a.sync.as_ref().unwrap();
    assert_eq!(sync.last_ok_ms, Some(1_000));
    assert!(!sync.is_running());
}

#[test]
fn a_second_device_gets_the_library_settings_and_addons() {
    let mut server = FakeServer::default();
    let mut a = device(vec![item("tt1", 10)], 1_000);
    update(
        &mut a,
        Action::ChangeSetting(Setting::SeekStep(SeekStep::S30)),
    );
    update(&mut a, Action::InstallAddon(STREAMS.into()));
    update(
        &mut a,
        Action::ManifestLoaded {
            transport: url(STREAMS),
            result: Ok(super::manifest("basic/manifest-streams.json")),
            origin: ManifestOrigin::Install,
        },
    );
    assert_eq!(a.installed, [url(BASIC), url(STREAMS)]);
    enable(&mut a, &mut server, 1, 1);

    let mut b = device(vec![item("tt2", 20)], 2_000);
    let effects = enable(&mut b, &mut server, 1, 2);
    assert_eq!(ids(&b), [("tt1".to_owned(), 10), ("tt2".to_owned(), 20)]);
    assert_eq!(b.settings.seek_step, SeekStep::S30);
    assert_eq!(b.installed, [url(BASIC), url(STREAMS)]);
    assert!(effects.iter().any(|e| matches!(
        e,
        Effect::SaveLibraryItem(i) if i.id == "tt1"
    )));
    assert!(effects.contains(&Effect::SaveSettings(b.settings)));
    assert!(effects.iter().any(|e| matches!(
        e,
        Effect::FetchManifest { transport, .. } if *transport == url(STREAMS)
    )));
    assert!(effects.iter().any(|e| matches!(e, Effect::SaveAddons(_))));

    act(&mut a, &mut server, Action::SyncNow);
    assert_eq!(ids(&a), ids(&b), "the first device gets the second's items");
}

#[test]
fn a_device_local_setting_never_travels() {
    let mut server = FakeServer::default();
    let mut a = device(Vec::new(), 1_000);
    update(
        &mut a,
        Action::ChangeSetting(Setting::AllowPrivateNetwork(true)),
    );
    enable(&mut a, &mut server, 1, 1);
    let mut b = device(Vec::new(), 2_000);
    enable(&mut b, &mut server, 1, 2);
    assert!(!b.settings.allow_private_network);
    assert!(
        !server.spaces[&[1; 16]]
            .records
            .contains_key(&RecordName::Setting("allow_private_network".into()))
    );
}

#[test]
fn an_edit_made_during_a_round_is_not_lost() {
    let mut server = FakeServer::default();
    let mut a = device(vec![item("tt1", 10)], 1_000);
    let effects = update(
        &mut a,
        Action::EnableSync {
            pairing: pairing(1),
            device: DeviceId(1),
        },
    );
    update(&mut a, Action::RemoveFromLibrary("tt1".into()));
    drive(&mut a, &mut server, effects);
    let mut b = device(Vec::new(), 2_000);
    enable(&mut b, &mut server, 1, 2);
    assert!(b.library.is_empty(), "{:?}", ids(&b));
}

#[test]
fn stopping_playback_syncs_at_once_and_other_edits_wait_a_minute() {
    let mut server = FakeServer::default();
    let mut a = device(vec![item("tt1", 10), item("tt2", 10)], 1_000);
    enable(&mut a, &mut server, 1, 1);
    update(&mut a, Action::Tick(2_000));
    let edit = update(&mut a, Action::RemoveFromLibrary("tt1".into()));
    assert_eq!(pulls(&edit), 0);
    let stop = update(&mut a, Action::PlaybackStopped);
    assert_eq!(pulls(&stop), 1);
    drive(&mut a, &mut server, stop);
    update(&mut a, Action::RemoveFromLibrary("tt2".into()));
    assert_eq!(pulls(&update(&mut a, Action::Tick(2_000 + MINUTE - 1))), 0);
    assert_eq!(pulls(&update(&mut a, Action::Tick(2_000 + MINUTE))), 1);
}

#[test]
fn a_quiet_device_still_pulls_every_ten_minutes() {
    let mut server = FakeServer::default();
    let mut a = device(Vec::new(), 1_000);
    enable(&mut a, &mut server, 1, 1);
    assert_eq!(
        pulls(&update(&mut a, Action::Tick(1_000 + 10 * MINUTE - 1))),
        0
    );
    assert_eq!(pulls(&update(&mut a, Action::Tick(1_000 + 10 * MINUTE))), 1);
}

#[test]
fn a_conflict_is_settled_by_pulling_merging_and_pushing_again() {
    let mut server = FakeServer::default();
    let mut a = device(vec![item("tt1", 10)], 1_000);
    enable(&mut a, &mut server, 1, 1);
    let mut b = device(Vec::new(), 1_500);
    enable(&mut b, &mut server, 1, 2);
    update(&mut a, Action::Tick(3_000));
    update(&mut b, Action::Tick(4_000));
    update(
        &mut b,
        Action::SetFavorite {
            id: "tt1".into(),
            favorite: true,
        },
    );
    let b_round = update(&mut b, Action::SyncNow);
    let b_pull = b_round
        .iter()
        .find(|e| matches!(e, Effect::SyncPull { .. }))
        .cloned()
        .unwrap();
    let b_answer = server.answer(&b_pull).unwrap();
    update(&mut a, Action::RemoveFromLibrary("tt1".into()));
    act(&mut a, &mut server, Action::SyncNow);
    let effects = update(&mut b, b_answer);
    let effects = drive(&mut b, &mut server, effects);
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::SyncPush { .. }))
            .count(),
        2,
        "pushed, refused, then pushed again: {effects:?}"
    );
    assert_eq!(pulls(&effects), 1, "pulled again after the conflict");
    assert!(b.sync.as_ref().unwrap().problem.is_none());
    act(&mut a, &mut server, Action::SyncNow);
    assert_eq!(ids(&a), ids(&b));
    assert!(a.library[0].is_favorite(), "the later edit wins");
}

#[test]
fn a_deleted_space_pauses_sync_until_it_is_paired_again() {
    let mut server = FakeServer::default();
    let mut a = device(Vec::new(), 1_000);
    enable(&mut a, &mut server, 1, 1);
    server.space(&pairing(1)).deleted = true;
    act(&mut a, &mut server, Action::SyncNow);
    assert_eq!(
        a.sync.as_ref().unwrap().problem,
        Some(SyncProblem::SpaceGone)
    );
    assert_eq!(pulls(&update(&mut a, Action::Tick(1_000 + 60 * MINUTE))), 0);
    assert_eq!(pulls(&update(&mut a, Action::SyncNow)), 0);
    assert!(!a.sync.as_ref().unwrap().can_sync_now());
    let effects = enable(&mut a, &mut server, 2, 1);
    assert!(matches!(effects[0], Effect::SavePairing { .. }));
    let sync = a.sync.as_ref().unwrap();
    assert_eq!(sync.pairing, pairing(2));
    assert!(sync.problem.is_none() && sync.can_sync_now());
}

#[test]
fn a_busy_server_is_asked_again_after_its_delay() {
    let mut server = FakeServer::default();
    let mut a = device(Vec::new(), 1_000);
    enable(&mut a, &mut server, 1, 1);
    server.busy = Some(120);
    act(&mut a, &mut server, Action::SyncNow);
    assert_eq!(a.sync.as_ref().unwrap().problem, Some(SyncProblem::Busy));
    server.busy = None;
    update(&mut a, Action::RemoveFromLibrary("x".into()));
    assert_eq!(pulls(&update(&mut a, Action::Tick(1_000 + 119_000))), 0);
    let effects = update(&mut a, Action::Tick(1_000 + 20 * MINUTE));
    assert_eq!(pulls(&effects), 1);
    drive(&mut a, &mut server, effects);
    assert!(a.sync.as_ref().unwrap().problem.is_none());
}

#[test]
fn a_new_key_moves_everything_then_deletes_the_old_space() {
    let mut server = FakeServer::default();
    let mut a = device(vec![item("tt1", 10)], 1_000);
    enable(&mut a, &mut server, 1, 1);
    let effects = act(
        &mut a,
        &mut server,
        Action::RotateSyncKey(SyncKey::from_bytes([2; 16])),
    );
    assert!(
        matches!(&effects[0], Effect::SavePairing { pairing, .. } if *pairing == self::pairing(2))
    );
    let deleted = effects
        .iter()
        .position(|e| *e == Effect::DeleteSyncSpace(pairing(1)))
        .expect("the old space is deleted");
    let pushed = effects
        .iter()
        .position(|e| matches!(e, Effect::SyncPush { pairing, .. } if *pairing == self::pairing(2)))
        .expect("the new space is filled");
    assert!(pushed < deleted, "filled before the old one goes");
    assert!(
        server.spaces[&[2; 16]]
            .records
            .contains_key(&RecordName::Item("tt1".into()))
    );
    assert!(server.spaces[&[1; 16]].deleted);
}

#[test]
fn turning_sync_off_forgets_it_and_can_delete_the_server_data() {
    let mut server = FakeServer::default();
    let mut a = device(vec![item("tt1", 10)], 1_000);
    enable(&mut a, &mut server, 1, 1);
    let kept = update(
        &mut a,
        Action::LeaveSync {
            delete_server_data: false,
        },
    );
    assert_eq!(kept, [Effect::ForgetSync]);
    assert!(a.sync.is_none());
    assert_eq!(ids(&a), [("tt1".to_owned(), 10)], "local data stays");
    enable(&mut a, &mut server, 1, 1);
    let deleted = update(
        &mut a,
        Action::LeaveSync {
            delete_server_data: true,
        },
    );
    assert_eq!(
        deleted,
        [Effect::ForgetSync, Effect::DeleteSyncSpace(pairing(1))]
    );
}

#[test]
fn answers_for_a_pairing_that_is_gone_are_ignored() {
    let mut server = FakeServer::default();
    let mut a = device(Vec::new(), 1_000);
    let effects = update(
        &mut a,
        Action::EnableSync {
            pairing: pairing(1),
            device: DeviceId(1),
        },
    );
    let pull = effects
        .iter()
        .find(|e| matches!(e, Effect::SyncPull { .. }))
        .cloned()
        .unwrap();
    update(
        &mut a,
        Action::LeaveSync {
            delete_server_data: false,
        },
    );
    let answer = server.answer(&pull).unwrap();
    assert!(update(&mut a, answer).is_empty());
    assert!(a.sync.is_none());
}

#[test]
fn without_sync_nothing_extra_happens() {
    let mut a = device(vec![item("tt1", 10)], 1_000);
    let effects = update(&mut a, Action::RemoveFromLibrary("tt1".into()));
    assert_eq!(effects, [Effect::DeleteLibraryItem("tt1".into())]);
    assert!(update(&mut a, Action::Tick(1_000 + 60 * MINUTE)).is_empty());
}

fn desktop_key(byte: u8) -> cineo_core::remote::RemoteKey {
    cineo_core::remote::RemoteKey::from_bytes([byte; 16])
}

fn listening(state: &mut State, server: &mut FakeServer, key: u8, address: &str) {
    act(state, server, Action::SetRemoteHost(Some(desktop_key(key))));
    act(
        state,
        server,
        Action::RemoteListening {
            address: Some(address.parse().unwrap()),
            name: "Study".into(),
        },
    );
    let later = state.now_ms + MINUTE;
    act(state, server, Action::Tick(later));
    act(state, server, Action::SyncNow);
}

#[test]
fn a_phone_finds_the_computer_through_sync_without_pairing() {
    let mut server = FakeServer::default();
    let mut desktop = device(Vec::new(), 1_000);
    enable(&mut desktop, &mut server, 1, 1);
    listening(&mut desktop, &mut server, 9, "192.168.100.86:18765");
    assert!(
        server.spaces[&[1; 16]]
            .records
            .contains_key(&RecordName::Computer(1))
    );
    assert!(
        desktop.remote.synced.is_empty(),
        "a desktop never finds itself"
    );

    let mut phone = device(Vec::new(), 2_000);
    enable(&mut phone, &mut server, 1, 2);
    let found = phone.remote.computer().unwrap();
    assert_eq!(found.name, "Study");
    assert_eq!(found.peer.address, "192.168.100.86:18765".parse().unwrap());
    assert_eq!(found.peer.key, desktop_key(9));
    assert_eq!(phone.remote.peer, None, "nothing was paired by hand");

    listening(&mut desktop, &mut server, 9, "192.168.100.90:18765");
    act(&mut phone, &mut server, Action::SyncNow);
    assert_eq!(
        phone.remote.computer().unwrap().peer.address,
        "192.168.100.90:18765".parse().unwrap(),
        "a new address reaches the phone"
    );

    act(&mut desktop, &mut server, Action::SetRemoteHost(None));
    let later = desktop.now_ms + MINUTE;
    act(&mut desktop, &mut server, Action::Tick(later));
    act(&mut desktop, &mut server, Action::SyncNow);
    act(&mut phone, &mut server, Action::SyncNow);
    assert_eq!(phone.remote.computer(), None, "turned off on the desktop");
}

#[test]
fn a_computer_paired_by_hand_wins_over_the_synced_one() {
    let mut server = FakeServer::default();
    let mut desktop = device(Vec::new(), 1_000);
    enable(&mut desktop, &mut server, 1, 1);
    listening(&mut desktop, &mut server, 9, "192.168.100.86:18765");
    let mut phone = device(Vec::new(), 2_000);
    enable(&mut phone, &mut server, 1, 2);
    let by_hand = cineo_core::remote::Computer {
        name: String::new(),
        peer: cineo_core::remote::Peer {
            address: "10.0.0.5:18765".parse().unwrap(),
            key: desktop_key(3),
        },
    };
    update(&mut phone, Action::SetRemotePeer(Some(by_hand.clone())));
    assert_eq!(phone.remote.computer(), Some(&by_hand));
    update(&mut phone, Action::SetRemotePeer(None));
    assert_eq!(
        phone.remote.computer().map(|c| c.peer.key),
        Some(desktop_key(9)),
        "forgetting it falls back to the synced one"
    );
    act(
        &mut phone,
        &mut server,
        Action::LeaveSync {
            delete_server_data: false,
        },
    );
    assert_eq!(phone.remote.computer(), None, "nothing without sync");
}
