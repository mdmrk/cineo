#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers panic on purpose"
)]

use std::net::SocketAddr;

use cineo_core::remote::{
    Command, DEFAULT_PORT, MAX_CLOCK_SKEW_MS, MAX_REMEMBERED, MAX_TITLE_CHARS, MAX_TRACKS,
    NowPlaying, Refusal, Replays, Request, Status, Track, is_lan, parse_address,
};
use serde_json::json;

fn addr(text: &str) -> SocketAddr {
    text.parse().unwrap()
}

#[test]
fn only_local_network_addresses_can_be_paired() {
    assert_eq!(
        parse_address("192.168.1.20:4000"),
        Some(addr("192.168.1.20:4000"))
    );
    assert_eq!(
        parse_address(" 10.0.0.7 "),
        Some(addr(&format!("10.0.0.7:{DEFAULT_PORT}")))
    );
    assert_eq!(
        parse_address("[fd00::1]:4000"),
        Some(addr("[fd00::1]:4000"))
    );
    assert_eq!(
        parse_address("fe80::1"),
        Some(addr(&format!("[fe80::1]:{DEFAULT_PORT}")))
    );
    assert!(parse_address("127.0.0.1:4000").is_some(), "loopback");
    assert!(parse_address("172.16.0.1:4000").is_some());
    assert!(parse_address("169.254.3.4:4000").is_some(), "link-local");
    assert_eq!(parse_address("8.8.8.8:4000"), None, "a public address");
    assert_eq!(parse_address("[2001:db8::1]:4000"), None);
    assert_eq!(
        parse_address("example.com:4000"),
        None,
        "names are not resolved"
    );
    assert_eq!(parse_address("192.168.1.20:0"), None);
    assert_eq!(parse_address(""), None);
    assert!(is_lan("::ffff:192.168.0.2".parse().unwrap()), "mapped IPv4");
    assert!(!is_lan("::ffff:1.1.1.1".parse().unwrap()));
}

#[test]
fn every_command_survives_the_wire() {
    let commands = [
        Command::Pause(true),
        Command::Pause(false),
        Command::SeekTo(125.5),
        Command::SeekBy(-10.0),
        Command::Volume(40.0),
        Command::Mute(true),
        Command::Audio(Some(2)),
        Command::Audio(None),
        Command::Subtitle(Some(3)),
        Command::Subtitle(None),
        Command::SubtitleDelay(-1.5),
        Command::Speed(1.25),
        Command::Stop,
    ];
    for command in commands {
        let request = Request {
            time_ms: 1_700_000_000_000,
            command: Some(command.clone()),
        };
        assert_eq!(
            Request::parse(request.to_json().as_bytes()),
            Some(request),
            "{command:?}"
        );
    }
    let poll = Request {
        time_ms: 5,
        command: None,
    };
    assert_eq!(Request::parse(poll.to_json().as_bytes()), Some(poll));
}

#[test]
fn commands_out_of_range_or_unknown_are_refused() {
    let refused = [
        json!({ "volume": 101 }),
        json!({ "volume": -1 }),
        json!({ "speed": 0.1 }),
        json!({ "speed": 8 }),
        json!({ "seek_to": -3 }),
        json!({ "seek_by": 1e12 }),
        json!({ "subtitle_delay": 601 }),
        json!({ "audio": 0 }),
        json!({ "audio": "2" }),
        json!({ "stop": false }),
        json!({ "pause": true, "stop": true }),
        json!({ "run": "rm -rf" }),
        json!({}),
        json!("pause"),
    ];
    for command in refused {
        let body = json!({ "v": 1, "time": 1, "command": command }).to_string();
        assert_eq!(Request::parse(body.as_bytes()), None, "{command}");
    }
    assert_eq!(
        Request::parse(br#"{"v":2,"time":1}"#),
        None,
        "a newer protocol"
    );
    assert_eq!(Request::parse(br#"{"v":1}"#), None, "no time");
    assert_eq!(Request::parse(b"not json"), None);
}

fn playing() -> NowPlaying {
    NowPlaying {
        title: "Dune".into(),
        loading: false,
        paused: true,
        position_s: 61.5,
        duration_s: 9000.0,
        volume: 80.0,
        muted: false,
        speed: 1.5,
        subtitle_delay_s: 0.3,
        audio: vec![Track {
            id: 1,
            label: Some("en · Stereo".into()),
            selected: true,
        }],
        subtitles: vec![
            Track {
                id: 2,
                label: None,
                selected: false,
            },
            Track {
                id: 3,
                label: Some("es".into()),
                selected: true,
            },
        ],
    }
}

#[test]
fn a_status_survives_the_wire() {
    let status = Status {
        playing: Some(playing()),
    };
    assert_eq!(Status::parse(status.to_json().as_bytes()), Some(status));
    let idle = Status::default();
    assert_eq!(Status::parse(idle.to_json().as_bytes()), Some(idle));
}

#[test]
fn a_status_is_read_leniently_and_kept_small() {
    let tracks: Vec<_> = (1..=200)
        .map(|id| json!({ "id": id, "label": "x".repeat(500) }))
        .collect();
    let body = json!({
        "v": 1,
        "playing": {
            "title": format!("A\u{7}title{}", "y".repeat(1000)),
            "position": -5,
            "volume": 900,
            "speed": "fast",
            "audio": tracks,
            "subtitles": [{ "id": -1 }, { "label": "no id" }, { "id": 4 }],
        },
    })
    .to_string();
    let playing = Status::parse(body.as_bytes()).unwrap().playing.unwrap();
    assert!(playing.title.starts_with("A title"), "{}", playing.title);
    assert_eq!(playing.title.chars().count(), MAX_TITLE_CHARS);
    assert_eq!(playing.position_s, 0.0);
    assert_eq!(playing.volume, 100.0);
    assert_eq!(playing.speed, 1.0);
    assert_eq!(playing.audio.len(), MAX_TRACKS);
    assert_eq!(
        playing.audio[0].label.as_deref().map(str::len),
        Some(cineo_core::remote::MAX_TRACK_LABEL_CHARS)
    );
    assert_eq!(
        playing.subtitles,
        [Track {
            id: 4,
            label: None,
            selected: false
        }]
    );
}

#[test]
fn a_request_is_admitted_once_and_only_near_the_desktops_time() {
    let mut replays = Replays::default();
    let now = 10 * MAX_CLOCK_SKEW_MS;
    assert_eq!(replays.admit(now, now, [1; 12]), Ok(()));
    assert_eq!(replays.admit(now + 5, now, [1; 12]), Err(Refusal::Replayed));
    assert_eq!(
        replays.admit(now, now - MAX_CLOCK_SKEW_MS - 1, [2; 12]),
        Err(Refusal::Clock)
    );
    assert_eq!(
        replays.admit(now, now + MAX_CLOCK_SKEW_MS + 1, [2; 12]),
        Err(Refusal::Clock)
    );
    assert_eq!(replays.admit(now, now + MAX_CLOCK_SKEW_MS, [2; 12]), Ok(()));
}

#[test]
fn old_nonces_are_forgotten_and_the_memory_is_bounded() {
    let mut replays = Replays::default();
    let start = 10 * MAX_CLOCK_SKEW_MS;
    let nonce = |n: usize| {
        let mut bytes = [0; 12];
        bytes[..8].copy_from_slice(&(n as u64).to_le_bytes());
        bytes
    };
    for n in 0..MAX_REMEMBERED {
        assert_eq!(replays.admit(start, start, nonce(n)), Ok(()));
    }
    assert_eq!(
        replays.admit(start, start, nonce(MAX_REMEMBERED)),
        Err(Refusal::Busy)
    );
    let later = start + MAX_CLOCK_SKEW_MS + 1;
    assert_eq!(replays.admit(later, later, nonce(MAX_REMEMBERED)), Ok(()));
}

#[test]
fn a_play_command_needs_a_playable_stream_and_ids() {
    let stream = json!({
        "addon": "https://addon.example/manifest.json",
        "title": "Dune",
        "stream": { "url": "https://cdn.example/dune.mp4" },
    });
    let play = |content: serde_json::Value| {
        let body = json!({ "v": 1, "time": 1, "command": { "play": content } }).to_string();
        Request::parse(body.as_bytes())
    };
    let good = json!({
        "type": "movie", "id": "tt1", "video": "tt1", "name": "Dune", "stream": stream,
    });
    let Some(Request {
        command: Some(Command::Play(sent)),
        ..
    }) = play(good.clone())
    else {
        panic!("a good play command is refused");
    };
    assert_eq!(sent.meta_id, "tt1");
    assert_eq!(sent.saved.title, "Dune");
    let mut no_id = good.clone();
    no_id["id"] = json!("");
    assert_eq!(play(no_id), None);
    let mut ftp = good.clone();
    ftp["stream"]["stream"]["url"] = json!("ftp://cdn.example/dune.mp4");
    assert_eq!(play(ftp), None, "a source the player cannot open");
    let mut file = good;
    file["stream"]["stream"]["url"] = json!("file:///etc/passwd");
    assert_eq!(play(file), None);
}
