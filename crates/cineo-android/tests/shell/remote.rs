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
