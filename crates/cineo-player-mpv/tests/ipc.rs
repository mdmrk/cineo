//! The IPC conversation against a fake mpv peer (an in-memory pipe), plus an
//! opt-in smoke test with a real mpv.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use cineo_core::app::PlayRequest;
use cineo_player_mpv::{PlayerEvent, drive_session};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

fn request() -> PlayRequest {
    PlayRequest {
        url: "https://media.example/v/1.mp4".parse().unwrap(),
        title: "Film ${path} --script=x".into(),
        headers: vec![
            ("User-Agent".into(), "Cineo-Test".into()),
            ("Bad Name".into(), "x".into()),
            ("X-Inject".into(), "a\r\nHost: evil".into()),
        ],
        subtitles: Vec::new(),
        start_ms: 61_500,
        meta_id: "tt1".into(),
        video_id: "tt1".into(),
    }
}

#[tokio::test]
async fn sends_typed_commands_and_reports_events() {
    let (ours, theirs) = tokio::io::duplex(64 * 1024);
    let (tx, mut rx) = mpsc::unbounded_channel();
    let session = tokio::spawn(async move { drive_session(ours, &request(), tx).await });

    let (reader, mut writer) = tokio::io::split(theirs);
    let mut lines = BufReader::new(reader).lines();
    let mut commands = Vec::new();
    for _ in 0..5 {
        let line = lines.next_line().await.unwrap().unwrap();
        let message: Value = serde_json::from_str(&line).unwrap();
        commands.push(message["command"].clone());
    }
    assert_eq!(
        commands[0],
        serde_json::json!([
            "set_property",
            "http-header-fields",
            ["User-Agent: Cineo-Test"]
        ]),
        "unsafe headers are not sent"
    );
    assert_eq!(
        commands[1],
        serde_json::json!([
            "set_property",
            "force-media-title",
            "Film ${path} --script=x"
        ]),
        "title is data, not an option"
    );
    assert_eq!(
        commands[4],
        serde_json::json!(["loadfile", "https://media.example/v/1.mp4", "replace"])
    );

    writer
        .write_all(b"{\"event\":\"file-loaded\"}\n")
        .await
        .unwrap();
    let seek: Value = serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
    assert_eq!(
        seek["command"],
        serde_json::json!(["seek", 61.5, "absolute"]),
        "resume after load"
    );

    for event in [
        r#"{"event":"property-change","id":2,"name":"duration","data":600.0}"#,
        r#"{"event":"property-change","id":1,"name":"time-pos","data":62.25}"#,
        r#"{"event":"property-change","id":1,"name":"time-pos","data":63.0}"#,
        r#"{"event":"end-file","reason":"eof"}"#,
    ] {
        writer
            .write_all(format!("{event}\n").as_bytes())
            .await
            .unwrap();
    }
    session.await.unwrap();
    assert_eq!(
        rx.recv().await,
        Some(PlayerEvent::Progress {
            time_ms: 62_250,
            duration_ms: 600_000
        })
    );
    // The second position is within the throttle interval: not reported.
    assert_eq!(
        rx.recv().await,
        Some(PlayerEvent::Ended {
            time_ms: 63_000,
            duration_ms: 600_000
        })
    );
}

#[tokio::test]
async fn load_errors_are_reported() {
    let (ours, theirs) = tokio::io::duplex(64 * 1024);
    let (tx, mut rx) = mpsc::unbounded_channel();
    let (_reader, mut writer) = tokio::io::split(theirs);
    writer
        .write_all(
            b"{\"event\":\"end-file\",\"reason\":\"error\",\"file_error\":\"loading failed\"}\n",
        )
        .await
        .unwrap();
    drive_session(ours, &request(), tx).await;
    assert_eq!(
        rx.recv().await,
        Some(PlayerEvent::Failed(
            "mpv could not play the stream: loading failed".into()
        ))
    );
}

#[tokio::test]
async fn closing_the_connection_reports_last_position() {
    let (ours, theirs) = tokio::io::duplex(64 * 1024);
    let (tx, mut rx) = mpsc::unbounded_channel();
    let (_reader, mut writer) = tokio::io::split(theirs);
    writer
        .write_all(b"{\"event\":\"property-change\",\"id\":1,\"data\":5.0}\n")
        .await
        .unwrap();
    // mpv quits: its side of the stream ends (the read half stays open so
    // our commands can still be written).
    writer.shutdown().await.unwrap();
    drive_session(ours, &request(), tx).await;
    assert!(matches!(
        rx.recv().await,
        Some(PlayerEvent::Progress { time_ms: 5000, .. })
    ));
    assert_eq!(
        rx.recv().await,
        Some(PlayerEvent::Closed {
            time_ms: 5000,
            duration_ms: 0
        })
    );
}

#[tokio::test]
async fn non_http_urls_are_refused_before_spawning() {
    let mut req = request();
    req.url = "file:///etc/passwd".parse().unwrap();
    let err = cineo_player_mpv::launch(Some("/nonexistent/mpv".into()), req)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("only http(s)"), "{err}");
}

#[tokio::test]
async fn missing_mpv_is_a_clear_error() {
    let err = cineo_player_mpv::launch(Some("/nonexistent/mpv".into()), request())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("mpv was not found"), "{err}");
}
