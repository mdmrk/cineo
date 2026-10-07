#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc as std_mpsc;
use std::time::Duration;

use super::*;

const AUDIO_ONLY: &[(&str, &str)] = &[("ao", "null"), ("vo", "null")];

fn request(url: &str) -> PlayRequest {
    PlayRequest {
        url: url.parse().unwrap(),
        title: "Film ${path} --script=x".into(),
        logo: None,
        headers: vec![
            ("User-Agent".into(), "Cineo-Test, with comma".into()),
            ("Bad Name".into(), "x".into()),
            ("X-Inject".into(), "a\r\nHost: evil".into()),
        ],
        subtitles: Vec::new(),
        start_ms: 0,
        meta_id: "tt1".into(),
        video_id: "tt1".into(),
    }
}

#[test]
#[cfg(target_os = "linux")]
fn linux_tries_vaapi_before_the_other_hardware_decoders() {
    let hwdec = OPTIONS.iter().find(|(name, _)| *name == "hwdec");
    assert_eq!(hwdec, Some(&("hwdec", "vaapi,auto-safe")));
}

#[test]
fn mpv_warnings_log_as_info_and_errors_as_errors() {
    assert_eq!(log_level("fatal"), tracing::Level::ERROR);
    assert_eq!(log_level("error"), tracing::Level::ERROR);
    assert_eq!(log_level("warn"), tracing::Level::INFO);
}

#[test]
fn commands_are_built_from_typed_values() {
    let args = |c| command_args(c).map(|v| v.join(" "));
    assert_eq!(
        args(PlayerCommand::TogglePause).as_deref(),
        Some("cycle pause")
    );
    assert_eq!(
        args(PlayerCommand::SeekTo(61.5)).as_deref(),
        Some("seek 61.500 absolute")
    );
    assert_eq!(
        args(PlayerCommand::SeekTo(-3.0)).as_deref(),
        Some("seek 0.000 absolute")
    );
    assert_eq!(
        args(PlayerCommand::SeekBy(-10.0)).as_deref(),
        Some("seek -10.000 relative")
    );
    assert_eq!(
        args(PlayerCommand::SetVolume(250.0)).as_deref(),
        Some("set volume 100")
    );
    assert_eq!(
        args(PlayerCommand::SetAudio(Some(2))).as_deref(),
        Some("set aid 2")
    );
    assert_eq!(
        args(PlayerCommand::SetSubtitle(None)).as_deref(),
        Some("set sid no")
    );
    assert_eq!(args(PlayerCommand::Stop).as_deref(), Some("quit"));
    assert_eq!(
        args(PlayerCommand::SeekTo(f64::NAN)),
        None,
        "NaN is not sent"
    );
    assert_eq!(args(PlayerCommand::SetVolume(f64::INFINITY)), None);
}

#[test]
fn track_list_is_parsed_leniently() {
    let tracks = parse_tracks(
        r#"[
            {"id":1,"type":"video","selected":true},
            {"id":1,"type":"audio","lang":"eng","title":"Commentary","selected":true},
            {"id":2,"type":"audio","lang":"  "},
            {"id":1,"type":"sub","lang":"spa"},
            {"type":"sub"},
            {"id":3,"type":"other"},
            "junk"
        ]"#,
    );
    assert_eq!(tracks.len(), 4);
    assert_eq!(
        tracks[1],
        Track {
            id: 1,
            kind: TrackKind::Audio,
            title: Some("Commentary".into()),
            lang: Some("eng".into()),
            selected: true,
        }
    );
    assert_eq!(tracks[2].lang, None, "blank text is no text");
    assert_eq!(tracks[3].kind, TrackKind::Subtitle);
    assert!(parse_tracks("not json").is_empty());
}

#[test]
fn non_http_urls_are_refused_before_loading_libmpv() {
    let err =
        Player::start_with(&request("file:///etc/passwd"), Arc::new(|| {}), None, &[]).unwrap_err();
    assert!(err.to_string().contains("only http(s)"), "{err}");
}

fn wav(seconds: u32) -> Vec<u8> {
    let samples = 8000 * seconds;
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + samples * 2).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16_u32.to_le_bytes());
    out.extend_from_slice(&1_u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1_u16.to_le_bytes()); // mono
    out.extend_from_slice(&8000_u32.to_le_bytes());
    out.extend_from_slice(&16000_u32.to_le_bytes());
    out.extend_from_slice(&2_u16.to_le_bytes());
    out.extend_from_slice(&16_u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(samples * 2).to_le_bytes());
    out.resize(out.len() + samples as usize * 2, 0);
    out
}

fn serve(body: Vec<u8>) -> (String, std_mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/film.wav", listener.local_addr().unwrap());
    let (tx, rx) = std_mpsc::channel();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut head = Vec::new();
            let mut byte = [0_u8];
            while !head.ends_with(b"\r\n\r\n") && stream.read(&mut byte).unwrap_or(0) == 1 {
                head.push(byte[0]);
            }
            let _ = tx.send(String::from_utf8_lossy(&head).into_owned());
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: audio/wav\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(&body);
        }
    });
    (url, rx)
}

#[test]
#[ignore = "needs libmpv"]
fn real_libmpv_plays_to_the_end_with_headers_and_tracks() {
    let (url, heads) = serve(wav(1));
    let (player, _, mut events) =
        Player::start_with(&request(&url), Arc::new(|| {}), None, AUDIO_ONLY).unwrap();

    let head = heads.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(
        head.contains("User-Agent: Cineo-Test, with comma\r\n"),
        "{head}"
    );
    assert!(!head.contains("evil"), "{head}");

    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let mut saw_audio_track = false;
    let last = loop {
        saw_audio_track |= player
            .status()
            .tracks
            .iter()
            .any(|t| t.kind == TrackKind::Audio && t.selected);
        match events.try_recv() {
            Ok(PlayerEvent::Progress { .. }) => {}
            Ok(event) => break Some(event),
            Err(_) if std::time::Instant::now() > deadline => break None,
            Err(_) => std::thread::sleep(Duration::from_millis(5)),
        }
    };
    assert!(
        matches!(
            last,
            Some(PlayerEvent::Ended {
                duration_ms: 1000,
                ..
            })
        ),
        "{last:?}"
    );
    assert!(saw_audio_track, "track-list arrives as JSON and is parsed");
    drop(player);
}

fn next_end(
    events: &mut mpsc::UnboundedReceiver<PlayerEvent>,
    player: &Player,
) -> Option<PlayerEvent> {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        match events.try_recv() {
            Ok(PlayerEvent::Progress { .. }) => {}
            Ok(event) => return Some(event),
            Err(_) if std::time::Instant::now() > deadline => return None,
            Err(_) if player.status().loaded && player.status().position_s > 1.0 => {
                player.send(PlayerCommand::Stop);
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => std::thread::sleep(Duration::from_millis(5)),
        }
    }
}

#[test]
#[ignore = "needs libmpv"]
fn real_libmpv_stop_reports_the_position_and_shuts_down() {
    let (url, _heads) = serve(wav(60));
    let mut req = request(&url);
    req.start_ms = 2_000;
    let (player, _, mut events) =
        Player::start_with(&req, Arc::new(|| {}), None, AUDIO_ONLY).unwrap();
    let last = next_end(&mut events, &player);
    let Some(PlayerEvent::Closed {
        time_ms,
        duration_ms,
    }) = last
    else {
        panic!("{last:?}");
    };
    assert!(time_ms >= 2_000, "resumed at the start position: {time_ms}");
    assert_eq!(duration_ms, 60_000);
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !player.is_finished() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(player.is_finished(), "mpv shut down after the stop");
}

fn serve_nothing() -> (String, std_mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/film.wav", listener.local_addr().unwrap());
    let (tx, rx) = std_mpsc::channel();
    std::thread::spawn(move || {
        let mut open = Vec::new();
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut head = Vec::new();
            let mut byte = [0_u8];
            while !head.ends_with(b"\r\n\r\n") && stream.read(&mut byte).unwrap_or(0) == 1 {
                head.push(byte[0]);
            }
            let _ = tx.send(String::from_utf8_lossy(&head).into_owned());
            open.push(stream);
        }
    });
    (url, rx)
}

#[test]
#[ignore = "needs libmpv"]
fn real_libmpv_stop_before_the_file_loads_closes_the_player() {
    let (url, heads) = serve_nothing();
    let (player, _, mut events) =
        Player::start_with(&request(&url), Arc::new(|| {}), None, AUDIO_ONLY).unwrap();
    heads.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(!player.status().loaded);

    player.send(PlayerCommand::Stop);
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !player.is_finished() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(player.is_finished(), "Back while loading closes the player");
    assert_eq!(
        events.try_recv().ok(),
        Some(PlayerEvent::Closed {
            time_ms: 0,
            duration_ms: 0,
        })
    );
}
