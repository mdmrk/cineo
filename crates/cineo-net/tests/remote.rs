#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers panic on purpose"
)]

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use cineo_core::app::{Route, parse_link};
use cineo_core::remote::{
    Command, MAX_CLOCK_SKEW_MS, MAX_REQUEST_BYTES, NowPlaying, PATH, Peer, RemoteKey, Request,
    Status,
};
use cineo_net::remote::{
    Handler, RemoteClient, RemoteError, RemoteKeys, RemoteServer, bind, format_remote_code,
    new_remote_key, parse_remote_code, remote_link, serve,
};

fn now_ms() -> u64 {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap()
}

struct Desktop {
    server: RemoteServer,
    key: RemoteKey,
    received: Arc<Mutex<Vec<Option<Command>>>>,
}

impl Desktop {
    fn start() -> Self {
        let key = new_remote_key().unwrap();
        let received = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&received);
        let handler: Handler = Arc::new(move |command| {
            let paused = command == Some(Command::Pause(true));
            seen.lock().unwrap().push(command);
            Status {
                playing: Some(NowPlaying {
                    title: "Dune".into(),
                    paused,
                    ..NowPlaying::default()
                }),
            }
        });
        let server = serve(bind(0).unwrap(), &key, handler).unwrap();
        Self {
            server,
            key,
            received,
        }
    }

    fn address(&self) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], self.server.port()))
    }

    fn peer(&self) -> Peer {
        Peer {
            address: self.address(),
            key: self.key,
        }
    }

    fn url(&self) -> String {
        format!("http://{}{PATH}", self.address())
    }
}

#[tokio::test]
async fn the_phone_reads_the_status_and_its_commands_reach_the_desktop() {
    let desktop = Desktop::start();
    let client = RemoteClient::new(&desktop.peer()).unwrap();
    let idle = client.send(None, now_ms()).await.unwrap();
    assert_eq!(
        idle.playing.as_ref().map(|p| p.title.as_str()),
        Some("Dune")
    );
    assert!(!idle.playing.unwrap().paused);
    let paused = client
        .send(Some(Command::Pause(true)), now_ms())
        .await
        .unwrap();
    assert!(paused.playing.unwrap().paused);
    assert_eq!(
        *desktop.received.lock().unwrap(),
        [None, Some(Command::Pause(true))]
    );
}

#[tokio::test]
async fn a_phone_with_another_key_is_refused_and_nothing_runs() {
    let desktop = Desktop::start();
    let stranger = Peer {
        key: new_remote_key().unwrap(),
        ..desktop.peer()
    };
    let client = RemoteClient::new(&stranger).unwrap();
    let err = client
        .send(Some(Command::Stop), now_ms())
        .await
        .unwrap_err();
    assert!(matches!(err, RemoteError::WrongKey), "{err:?}");
    assert!(desktop.received.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_request_far_from_the_desktops_clock_is_refused() {
    let desktop = Desktop::start();
    let client = RemoteClient::new(&desktop.peer()).unwrap();
    let late = now_ms() - MAX_CLOCK_SKEW_MS - 60_000;
    let err = client.send(Some(Command::Stop), late).await.unwrap_err();
    assert!(matches!(err, RemoteError::Clock), "{err:?}");
    assert!(desktop.received.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_captured_request_cannot_be_replayed() {
    let desktop = Desktop::start();
    let keys = RemoteKeys::derive(&desktop.key).unwrap();
    let request = Request {
        time_ms: now_ms(),
        command: Some(Command::Pause(true)),
    };
    let (nonce, sealed) = keys.seal_request(request.to_json().as_bytes()).unwrap();
    let http = reqwest::Client::new();
    let first = http
        .post(desktop.url())
        .body(sealed.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(first.status(), 200);
    let answer = first.bytes().await.unwrap();
    assert!(keys.open_answer(&nonce, &answer).is_some());
    assert!(
        keys.open_answer(&[0; 12], &answer).is_none(),
        "the answer belongs to its request"
    );
    let again = http.post(desktop.url()).body(sealed).send().await.unwrap();
    assert_eq!(again.status(), 403);
    assert_eq!(desktop.received.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn the_desktop_answers_nothing_else() {
    let desktop = Desktop::start();
    let http = reqwest::Client::new();
    let base = format!("http://{}", desktop.address());
    let get = http.get(desktop.url()).send().await.unwrap();
    assert_eq!(get.status(), 405);
    let other = http.post(format!("{base}/")).send().await.unwrap();
    assert_eq!(other.status(), 404);
    let large = http
        .post(desktop.url())
        .body(vec![1; MAX_REQUEST_BYTES + 1])
        .send()
        .await
        .unwrap();
    assert_eq!(large.status(), 413);
    let garbage = http
        .post(desktop.url())
        .body(vec![1; 64])
        .send()
        .await
        .unwrap();
    assert_eq!(garbage.status(), 401);
    assert!(desktop.received.lock().unwrap().is_empty());
}

#[tokio::test]
async fn an_invalid_command_inside_a_sealed_request_is_refused() {
    let desktop = Desktop::start();
    let keys = RemoteKeys::derive(&desktop.key).unwrap();
    let body = format!(
        r#"{{"v":1,"time":{},"command":{{"volume":1000}}}}"#,
        now_ms()
    );
    let (_, sealed) = keys.seal_request(body.as_bytes()).unwrap();
    let response = reqwest::Client::new()
        .post(desktop.url())
        .body(sealed)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
    assert!(desktop.received.lock().unwrap().is_empty());
}

#[tokio::test]
async fn stopping_the_listener_closes_the_port() {
    let desktop = Desktop::start();
    let client = RemoteClient::new(&desktop.peer()).unwrap();
    client.send(None, now_ms()).await.unwrap();
    drop(desktop.server);
    let mut answered = 0;
    let err = loop {
        match client.send(None, now_ms()).await {
            Ok(_) if answered < 50 => {
                answered += 1;
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
            Ok(_) => panic!("the listener still answers"),
            Err(err) => break err,
        }
    };
    assert!(matches!(err, RemoteError::Fetch(_)), "{err:?}");
}

#[test]
fn the_phone_only_talks_to_the_local_network() {
    let peer = Peer {
        address: "93.184.216.34:18765".parse().unwrap(),
        key: RemoteKey::from_bytes([1; 16]),
    };
    assert!(matches!(
        RemoteClient::new(&peer).unwrap_err(),
        RemoteError::NotLan
    ));
}

#[test]
fn the_pairing_link_carries_the_code_and_the_address() {
    let peer = Peer {
        address: "192.168.1.20:18765".parse().unwrap(),
        key: RemoteKey::from_bytes([9; 16]),
    };
    let link = remote_link(&peer);
    let Ok(Route::Remote { code, address }) = parse_link(&link) else {
        panic!("{link} is not a remote link");
    };
    assert_eq!(code, format_remote_code(&peer.key));
    assert_eq!(parse_remote_code(&code).unwrap(), peer.key);
    assert_eq!(address, "192.168.1.20:18765");
}
