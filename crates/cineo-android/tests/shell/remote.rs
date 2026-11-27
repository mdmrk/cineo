use cineo_android::{RemoteCommandView, RemoteFailure, RemoteInputError, remote_link};
use cineo_core::remote::{Command, NowPlaying, Peer, RemoteKey, Status, Track};
use cineo_net::remote::{
    Handler, RemoteServer, bind, format_remote_code, new_remote_key, remote_link as link_for, serve,
};

use super::*;

struct Desktop {
    server: RemoteServer,
    key: RemoteKey,
    commands: Arc<Mutex<Vec<Command>>>,
}

impl Desktop {
    fn start() -> Self {
        let key = new_remote_key().unwrap();
        let commands = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&commands);
        let handler: Handler = Arc::new(move |command| {
            let mut seen = seen.lock().unwrap();
            seen.extend(command);
            let paused = seen.iter().rev().find_map(|c| match c {
                Command::Pause(paused) => Some(*paused),
                _ => None,
            });
            Status {
                playing: Some(NowPlaying {
                    title: "Dune".into(),
                    paused: paused.unwrap_or(false),
                    duration_s: 9000.0,
                    subtitles: vec![
                        Track {
                            id: 4,
                            label: None,
                            selected: false,
                        },
                        Track {
                            id: 7,
                            label: Some("es".into()),
                            selected: true,
                        },
                    ],
                    ..NowPlaying::default()
                }),
            }
        });
        let server = serve(bind(0).unwrap(), &key, handler).unwrap();
        Self {
            server,
            key,
            commands,
        }
    }

    fn address(&self) -> String {
        format!("127.0.0.1:{}", self.server.port())
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_paired_phone_controls_the_desktop_and_stays_paired_after_a_restart() {
    let desktop = Desktop::start();
    let dir = local_data_dir();
    let code = format_remote_code(&desktop.key);
    let (cineo, views) = tokio::task::block_in_place(|| start(dir.path()));
    tokio::task::block_in_place(|| wait_for(&views, |v| v.remote.is_none()));
    assert_eq!(
        cineo.remote_send(None).await.unwrap_err(),
        RemoteFailure::NotPaired
    );
    cineo.remote_pair(&desktop.address(), &code).unwrap();
    let paired = tokio::task::block_in_place(|| wait_for(&views, |v| v.remote.is_some()));
    assert_eq!(paired.remote.unwrap().address, desktop.address());

    let status = cineo.remote_send(None).await.unwrap();
    let playing = status.playing.unwrap();
    assert_eq!(playing.title, "Dune");
    assert!(!playing.paused);
    let numbers: Vec<_> = playing.subtitles.iter().map(|t| (t.id, t.number)).collect();
    assert_eq!(numbers, [(4, 1), (7, 2)]);
    let paused = cineo
        .remote_send(Some(RemoteCommandView::Pause { paused: true }))
        .await
        .unwrap();
    assert!(paused.playing.unwrap().paused);
    assert_eq!(*desktop.commands.lock().unwrap(), [Command::Pause(true)]);
    drop((cineo, views));

    let (cineo, views) = tokio::task::block_in_place(|| start(dir.path()));
    tokio::task::block_in_place(|| wait_for(&views, |v| v.remote.is_some()));
    cineo
        .remote_send(Some(RemoteCommandView::Volume { percent: 30.0 }))
        .await
        .unwrap();
    assert_eq!(
        desktop.commands.lock().unwrap().last(),
        Some(&Command::Volume(30.0))
    );

    cineo.remote_forget();
    tokio::task::block_in_place(|| wait_for(&views, |v| v.remote.is_none()));
    assert_eq!(
        cineo.remote_send(None).await.unwrap_err(),
        RemoteFailure::NotPaired
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_new_code_on_the_desktop_unpairs_the_phone() {
    let desktop = Desktop::start();
    let dir = local_data_dir();
    let (cineo, _views) = tokio::task::block_in_place(|| start(dir.path()));
    let old = format_remote_code(&new_remote_key().unwrap());
    cineo.remote_pair(&desktop.address(), &old).unwrap();
    assert_eq!(
        cineo.remote_send(None).await.unwrap_err(),
        RemoteFailure::WrongKey
    );
    assert!(desktop.commands.lock().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unreachable_computer_and_a_bad_command_are_reported() {
    let dir = local_data_dir();
    let (cineo, _views) = tokio::task::block_in_place(|| start(dir.path()));
    let free = bind(0).unwrap();
    let address = format!("127.0.0.1:{}", free.local_addr().unwrap().port());
    drop(free);
    let code = format_remote_code(&new_remote_key().unwrap());
    cineo.remote_pair(&address, &code).unwrap();
    assert_eq!(
        cineo.remote_send(None).await.unwrap_err(),
        RemoteFailure::Unreachable
    );
    assert_eq!(
        cineo
            .remote_send(Some(RemoteCommandView::Volume { percent: 500.0 }))
            .await
            .unwrap_err(),
        RemoteFailure::Invalid
    );
}

#[test]
fn pairing_needs_a_local_address_and_a_good_code() {
    let dir = local_data_dir();
    let (cineo, _views) = start(dir.path());
    let code = format_remote_code(&RemoteKey::from_bytes([3; 16]));
    assert_eq!(
        cineo.remote_pair("8.8.8.8:18765", &code),
        Err(RemoteInputError::Address)
    );
    assert_eq!(
        cineo.remote_pair("not an address", &code),
        Err(RemoteInputError::Address)
    );
    assert_eq!(
        cineo.remote_pair("192.168.1.20", "ABCD"),
        Err(RemoteInputError::CodeLength)
    );
    let typo = code.replacen(&code[..1], if &code[..1] == "A" { "B" } else { "A" }, 1);
    assert_eq!(
        cineo.remote_pair("192.168.1.20", &typo),
        Err(RemoteInputError::CodeTypo)
    );
    assert_eq!(cineo.remote_pair("192.168.1.20", &code), Ok(()));
}

#[test]
fn a_scanned_desktop_link_gives_the_code_and_address() {
    let peer = Peer {
        address: "192.168.1.20:18765".parse().unwrap(),
        key: RemoteKey::from_bytes([3; 16]),
    };
    let link = remote_link(&link_for(&peer)).unwrap();
    assert_eq!(link.address, "192.168.1.20:18765");
    assert_eq!(link.code, format_remote_code(&peer.key));
    assert!(remote_link("cineo:///sync/ABCD").is_none());
}

fn now_ms() -> u64 {
    u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap()
}

async fn drive(
    state: &mut cineo_core::app::State,
    client: &cineo_net::AddonClient,
    action: cineo_core::app::Action,
) {
    let mut effects = cineo_core::app::update(state, action);
    while let Some(effect) = effects.pop() {
        if let Some(task) = cineo_net::fetch_task(client, effect) {
            let answer = task.await;
            effects.extend(cineo_core::app::update(state, answer));
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_phone_in_the_same_sync_space_finds_and_controls_the_computer() {
    use cineo_core::app::{Action, State};
    use cineo_core::sync::{DeviceId, Pairing};
    use cineo_net::sync::{format_code, new_key};

    let server = super::sync::sync_server().await;
    let desktop = Desktop::start();
    let client = cineo_net::AddonClient::new(cineo_net::NetPolicy {
        allow_private_networks: true,
        ..cineo_net::NetPolicy::default()
    })
    .unwrap();
    let key = new_key().unwrap();
    let mut computer = State::default();
    drive(&mut computer, &client, Action::Tick(now_ms())).await;
    drive(
        &mut computer,
        &client,
        Action::SetRemoteHost(Some(desktop.key)),
    )
    .await;
    drive(
        &mut computer,
        &client,
        Action::RemoteListening(Some(desktop.address().parse().unwrap())),
    )
    .await;
    drive(
        &mut computer,
        &client,
        Action::EnableSync {
            pairing: Pairing {
                key,
                server: url::Url::parse(&server).unwrap(),
            },
            device: DeviceId(1),
        },
    )
    .await;
    assert!(computer.sync.as_ref().unwrap().last_ok_ms.is_some());

    let dir = local_data_dir();
    let (phone, views) = tokio::task::block_in_place(|| start(dir.path()));
    phone.sync_pair(&format_code(&key), &server).unwrap();
    let found = tokio::task::block_in_place(|| {
        wait_for(&views, |v| {
            v.remote.as_ref().is_some_and(|r| r.through_sync)
        })
    });
    assert_eq!(found.remote.unwrap().address, desktop.address());
    let status = phone
        .remote_send(Some(RemoteCommandView::Pause { paused: true }))
        .await
        .unwrap();
    assert!(status.playing.unwrap().paused);
    assert_eq!(*desktop.commands.lock().unwrap(), [Command::Pause(true)]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stream_picked_on_the_phone_is_sent_to_the_computer() {
    let desktop = Desktop::start();
    let server = MockServer::start().await;
    let manifest = movie_addon(&server).await;
    let dir = local_data_dir();
    let players = Arc::new(FakePlayers::default());
    let (cineo, views) = tokio::task::block_in_place(|| start_with(dir.path(), &players));
    tokio::task::block_in_place(|| open_movie(&cineo, &views, manifest));
    assert_eq!(
        cineo.play_on_computer(0, 0).await.unwrap_err(),
        RemoteFailure::NotPaired
    );
    cineo
        .remote_pair(&desktop.address(), &format_remote_code(&desktop.key))
        .unwrap();
    cineo.play_on_computer(0, 0).await.unwrap();
    let sent = desktop.commands.lock().unwrap().clone();
    let [Command::Play(sent)] = sent.as_slice() else {
        panic!("the computer got {sent:?}");
    };
    assert_eq!(sent.meta_id, "tt0000001");
    assert_eq!(sent.saved.title, "First Example Film");
    assert!(
        players.requests.lock().unwrap().is_empty(),
        "nothing plays on the phone"
    );
    assert_eq!(
        cineo.play_on_computer(0, 99).await.unwrap_err(),
        RemoteFailure::Invalid,
        "a stream the detail does not have"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn continue_watching_on_the_phone_resumes_on_the_computer() {
    let desktop = Desktop::start();
    let server = MockServer::start().await;
    let manifest = movie_addon(&server).await;
    let dir = local_data_dir();
    let players = Arc::new(FakePlayers::default());
    let (cineo, views) = tokio::task::block_in_place(|| start_with(dir.path(), &players));
    tokio::task::block_in_place(|| {
        open_movie(&cineo, &views, manifest);
        cineo.play(0, 0);
        wait_for(&views, |v| v.playback.is_some());
        play_a_minute_and_close(&players);
        wait_for(&views, |v| {
            v.playback.is_none() && !v.continue_watching.is_empty()
        });
    });
    cineo
        .remote_pair(&desktop.address(), &format_remote_code(&desktop.key))
        .unwrap();
    cineo.resume_on_computer("tt0000001".into()).await.unwrap();
    let sent = desktop.commands.lock().unwrap().clone();
    let [Command::Play(sent)] = sent.as_slice() else {
        panic!("the computer got {sent:?}");
    };
    assert_eq!(sent.meta_id, "tt0000001");
    assert_eq!(sent.offset_ms, 61_000);
    assert_eq!(sent.saved.title, "First Example Film");
    assert_eq!(
        cineo.resume_on_computer("tt404".into()).await.unwrap_err(),
        RemoteFailure::Invalid
    );
}
