use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::mpsc as std_mpsc;
use std::time::Duration;

use super::*;
use cineo_core::app::{
    SubtitleBackground, SubtitleColor, SubtitleOpacity, SubtitleOutline, SubtitlePosition,
    SubtitleSize,
};

const AUDIO_ONLY: &[(&str, &str)] = &[("ao", "null"), ("vo", "null")];

fn request(url: &str) -> PlayRequest {
    PlayRequest {
        url: url.parse().unwrap(),
        title: "Film ${path} --script=x".into(),
        logo: None,
        background: None,
        headers: vec![
            ("User-Agent".into(), "Cineo-Test, with comma".into()),
            ("Bad Name".into(), "x".into()),
            ("X-Inject".into(), "a\r\nHost: evil".into()),
        ],
        settings: Settings::default(),
        start_ms: 0,
        meta_id: "tt1".into(),
        video_id: "tt1".into(),
    }
}

#[test]
fn a_surface_is_attached_before_video_output_starts_and_detached_after_it_stops() {
    let names = |steps: &[(Set, &str, String)]| {
        steps
            .iter()
            .map(|(set, name, value)| format!("{set:?} {name}={value}"))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        names(&attach_steps(0x7f00)),
        [
            "Option wid=32512",
            "Option force-window=yes",
            "Property vo=gpu"
        ]
    );
    assert_eq!(
        names(&detach_steps()),
        ["Property vo=null", "Option force-window=no", "Option wid=0"]
    );
}

#[test]
fn only_missing_script_options_are_tolerated() {
    let missing = ffi::MpvError {
        message: "option not found".into(),
        code: Some(-5),
    };
    let invalid = ffi::MpvError {
        message: "option parameter could not be parsed".into(),
        code: Some(-7),
    };
    assert!(missing.is_option_not_found());
    assert!(!invalid.is_option_not_found());
    assert!(
        SCRIPT_OPTIONS
            .iter()
            .all(|name| OPTIONS.iter().any(|(option, _)| option == name))
    );
    assert!(!SCRIPT_OPTIONS.contains(&"vo"));
}

#[test]
fn surface_playback_starts_without_video_output_decodes_through_the_ndk_and_draws_8_bit() {
    let option = |name| {
        SURFACE_OPTIONS
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| *v)
    };
    assert_eq!(option("vo"), Some("null"));
    assert_eq!(option("gpu-context"), Some("android"));
    assert_eq!(option("vd-lavc-o"), Some("ndk_codec=1"));
    assert_eq!(option("profile"), Some("fast"));
    assert_eq!(option("vf"), Some("format=fmt=yuv420p"));
    assert_eq!(option("hwdec"), None);
}

#[test]
fn surface_subtitles_are_drawn_with_the_font_in_the_given_folder() {
    assert_eq!(
        surface_font_options("/data/fonts"),
        [("sub-fonts-dir", "/data/fonts"), ("sub-font", "Inter")]
    );
}

#[test]
#[ignore = "needs libmpv"]
fn real_libmpv_accepts_the_surface_font_options_over_a_chosen_font() {
    let (url, _heads) = serve(wav(1));
    let request = PlayRequest {
        settings: Settings {
            subtitle_font: SubtitleFont::Serif,
            ..Settings::default()
        },
        ..request(&url)
    };
    let dir = tempfile::tempdir().unwrap();
    let mut options = AUDIO_ONLY.to_vec();
    options.extend(surface_font_options(dir.path().to_str().unwrap()));
    let started = Player::start_with(&request, Arc::new(|| {}), Output::None, &options);
    assert!(started.is_ok(), "mpv accepts the surface font options");
}

#[test]
fn commands_are_built_from_typed_values() {
    let args = |c| command_args(c).map(|v| v.join(" "));
    assert_eq!(
        args(PlayerCommand::TogglePause).as_deref(),
        Some("cycle pause")
    );
    assert_eq!(
        args(PlayerCommand::SetPause(true)).as_deref(),
        Some("set pause yes")
    );
    assert_eq!(
        args(PlayerCommand::SetPause(false)).as_deref(),
        Some("set pause no")
    );
    assert_eq!(
        args(PlayerCommand::SetMute(true)).as_deref(),
        Some("set mute yes")
    );
    assert_eq!(
        args(PlayerCommand::SetMute(false)).as_deref(),
        Some("set mute no")
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
    assert_eq!(
        args(PlayerCommand::SetSubtitleDelay(-0.25)).as_deref(),
        Some("set sub-delay -0.250")
    );
    assert_eq!(args(PlayerCommand::SetSubtitleDelay(f64::NAN)), None);
}

#[test]
fn track_list_is_parsed_leniently() {
    let tracks = parse_tracks(
        r#"[
            {"id":1,"type":"video","selected":true},
            {"id":1,"type":"audio","lang":"eng","title":"Commentary","codec":"aac","selected":true},
            {"id":2,"type":"audio","lang":"  "},
            {"id":1,"type":"sub","lang":"spa","external":true,"external-filename":"/c/subtitles/1"},
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
            external_file: None,
            codec: Some("aac".into()),
        }
    );
    assert_eq!(tracks[2].lang, None, "blank text is no text");
    assert_eq!(tracks[3].kind, TrackKind::Subtitle);
    assert_eq!(tracks[3].external_file.as_deref(), Some("/c/subtitles/1"));
    assert!(parse_tracks("not json").is_empty());
}

#[test]
fn stats_take_mpv_values_and_drop_unavailable_ones() {
    let mut stats = Stats::default();
    assert!(apply_stat(
        &mut stats,
        CACHE_SPEED,
        &Value::Double(2_500_000.0)
    ));
    assert!(!apply_stat(
        &mut stats,
        CACHE_SPEED,
        &Value::Double(2_500_000.0)
    ));
    assert!(apply_stat(&mut stats, WIDTH, &Value::Double(1920.0)));
    assert!(apply_stat(
        &mut stats,
        HWDEC_CURRENT,
        &Value::Text("vaapi".into())
    ));
    assert_eq!(stats.cache_speed, Some(2_500_000.0));
    assert_eq!(stats.width, Some(1920.0));
    assert_eq!(stats.hwdec.as_deref(), Some("vaapi"));
    assert!(apply_stat(&mut stats, CACHE_SPEED, &Value::None));
    assert!(apply_stat(&mut stats, WIDTH, &Value::Double(f64::NAN)));
    assert!(apply_stat(
        &mut stats,
        HWDEC_CURRENT,
        &Value::Text(String::new())
    ));
    assert_eq!(stats, Stats::default());
    assert!(!apply_stat(&mut stats, TIME_POS, &Value::Double(1.0)));
}

#[test]
fn cached_ranges_become_fractions_of_the_duration() {
    let mut status = Status {
        cached: Arc::from([(0.0, 25.0), (50.0, 150.0), (200.0, 300.0)]),
        ..Status::default()
    };
    assert_eq!(status.cached_spans().count(), 0, "no duration yet");
    status.duration_s = 100.0;
    assert_eq!(
        status.cached_spans().collect::<Vec<_>>(),
        vec![(0.0, 0.25), (0.5, 1.0)]
    );
}

#[test]
fn cached_ranges_come_from_the_seekable_ranges() {
    let json = r#"{"seekable-ranges":[{"start":0,"end":12.5},{"start":-1,"end":40},
        {"start":50,"end":50},{"start":"x","end":60},{"end":70}],"cache-end":40}"#;
    assert_eq!(parse_cached_ranges(json), vec![(0.0, 12.5), (0.0, 40.0)]);
    assert!(parse_cached_ranges(r#"{"cache-end":3}"#).is_empty());
    assert!(parse_cached_ranges("not json").is_empty());

    let mut cached: Arc<[(f64, f64)]> = Arc::from([]);
    assert!(replace_shared(&mut cached, vec![(0.0, 1.0)]));
    assert!(!replace_shared(&mut cached, vec![(0.0, 1.0)]));
}

#[test]
fn status_snapshots_share_the_track_list() {
    let status = Status {
        tracks: parse_tracks(r#"[{"id":1,"type":"audio","selected":true}]"#).into(),
        ..Status::default()
    };
    let snapshot = status.clone();
    assert!(Arc::ptr_eq(&status.tracks, &snapshot.tracks));
}

#[test]
fn an_unfinished_seek_counts_as_waiting() {
    let playing = Status {
        loaded: true,
        ..Status::default()
    };
    assert!(!playing.is_waiting());
    let seeking = Status {
        seeking: true,
        ..playing.clone()
    };
    assert!(seeking.is_waiting());
    let buffering = Status {
        buffering: true,
        ..playing
    };
    assert!(buffering.is_waiting());
}

#[test]
fn an_unchanged_track_list_keeps_its_snapshot() {
    let json = r#"[{"id":1,"type":"audio","selected":true}]"#;
    let mut tracks: Arc<[Track]> = parse_tracks(json).into();
    let before = Arc::clone(&tracks);
    assert!(!replace_shared(&mut tracks, parse_tracks(json)));
    assert!(Arc::ptr_eq(&tracks, &before));
    assert!(replace_shared(&mut tracks, Vec::new()));
    assert!(tracks.is_empty());
}

#[test]
fn non_http_urls_are_refused_before_loading_libmpv() {
    let err = Player::start_with(
        &request("file:///etc/passwd"),
        Arc::new(|| {}),
        Output::None,
        &[],
    )
    .unwrap_err();
    assert!(err.to_string().contains("only http(s)"), "{err}");
}

fn wav(seconds: u32) -> Vec<u8> {
    let samples = 8000 * seconds;
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + samples * 2).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16_u32.to_le_bytes());
    out.extend_from_slice(&1_u16.to_le_bytes());
    out.extend_from_slice(&1_u16.to_le_bytes());
    out.extend_from_slice(&8000_u32.to_le_bytes());
    out.extend_from_slice(&16000_u32.to_le_bytes());
    out.extend_from_slice(&2_u16.to_le_bytes());
    out.extend_from_slice(&16_u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(samples * 2).to_le_bytes());
    out.resize(out.len() + samples as usize * 2, 0);
    out
}

fn listen(
    mut respond: impl FnMut(std::net::TcpStream, &str) + Send + 'static,
) -> (String, std_mpsc::Receiver<String>) {
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
            let head = String::from_utf8_lossy(&head).into_owned();
            let _ = tx.send(head.clone());
            respond(stream, &head);
        }
    });
    (url, rx)
}

fn serve(body: Vec<u8>) -> (String, std_mpsc::Receiver<String>) {
    serve_ranges(body, true)
}

fn serve_ranges(body: Vec<u8>, advertised: bool) -> (String, std_mpsc::Receiver<String>) {
    listen(move |mut stream, head| {
        let len = body.len();
        let start = head
            .lines()
            .find_map(|line| line.strip_prefix("Range: bytes="))
            .and_then(|range| range.split('-').next()?.parse::<usize>().ok())
            .filter(|&start| start < len);
        let accept = if advertised {
            "Accept-Ranges: bytes\r\n"
        } else {
            ""
        };
        let status = match start {
            Some(start) => format!(
                "206 Partial Content\r\nContent-Range: bytes {start}-{}/{len}",
                len - 1
            ),
            None => "200 OK".to_owned(),
        };
        let part = &body[start.unwrap_or(0)..];
        let _ = write!(
            stream,
            "HTTP/1.1 {status}\r\n{accept}Content-Type: audio/wav\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            part.len()
        );
        let _ = stream.write_all(part);
    })
}

#[test]
#[ignore = "needs libmpv"]
fn real_libmpv_plays_to_the_end_with_headers_and_tracks() {
    let (url, heads) = serve(wav(1));
    let (player, _, mut events) =
        Player::start_with(&request(&url), Arc::new(|| {}), Output::None, AUDIO_ONLY).unwrap();

    let head = heads.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(
        head.contains("User-Agent: Cineo-Test, with comma\r\n"),
        "{head}"
    );
    assert!(!head.contains("evil"), "{head}");

    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let mut saw_audio_track = false;
    let mut saw_cached = false;
    let last = loop {
        saw_cached |= !player.status().cached.is_empty();
        saw_audio_track |= player
            .status()
            .tracks
            .iter()
            .any(|t| t.kind == TrackKind::Audio && t.selected);
        match events.try_recv() {
            Ok(PlayerEvent::Progress { .. } | PlayerEvent::Buffered { .. }) => {}
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
    assert!(
        saw_cached,
        "demuxer-cache-state arrives as JSON and is parsed"
    );
    drop(player);
}

fn next_end(
    events: &mut mpsc::UnboundedReceiver<PlayerEvent>,
    player: &Player,
) -> Option<PlayerEvent> {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        match events.try_recv() {
            Ok(PlayerEvent::Progress { .. } | PlayerEvent::Buffered { .. }) => {}
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
        Player::start_with(&req, Arc::new(|| {}), Output::None, AUDIO_ONLY).unwrap();
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

#[test]
#[ignore = "needs libmpv"]
fn real_libmpv_is_destroyed_before_an_ended_player_is_dropped() {
    let (url, _heads) = serve(wav(1));
    let (player, _, mut events) =
        Player::start_with(&request(&url), Arc::new(|| {}), Output::None, AUDIO_ONLY).unwrap();
    let last = next_end(&mut events, &player);
    assert!(matches!(last, Some(PlayerEvent::Ended { .. })), "{last:?}");
    let core = Arc::downgrade(&player.core);
    drop(player);
    assert!(core.upgrade().is_none(), "mpv is still alive after drop");
}

#[test]
#[ignore = "needs libmpv"]
fn real_libmpv_resumes_on_a_server_that_does_not_send_accept_ranges() {
    let (url, _heads) = serve_ranges(wav(60), false);
    let mut req = request(&url);
    req.start_ms = 30_000;
    let (player, _, mut events) =
        Player::start_with(&req, Arc::new(|| {}), Output::None, AUDIO_ONLY).unwrap();
    let last = next_end(&mut events, &player);
    let Some(PlayerEvent::Closed {
        time_ms,
        duration_ms,
    }) = last
    else {
        panic!("{last:?}");
    };
    assert!(
        time_ms >= 30_000,
        "resumed at the start position: {time_ms}"
    );
    assert_eq!(duration_ms, 60_000);
}

fn serve_nothing() -> (String, std_mpsc::Receiver<String>) {
    let mut open = Vec::new();
    listen(move |stream, _| open.push(stream))
}

#[test]
#[ignore = "needs libmpv"]
fn real_libmpv_stop_before_the_file_loads_closes_the_player() {
    let (url, heads) = serve_nothing();
    let (player, _, mut events) =
        Player::start_with(&request(&url), Arc::new(|| {}), Output::None, AUDIO_ONLY).unwrap();
    heads.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(!player.status().loaded);

    player.send(PlayerCommand::Stop);
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !player.is_finished() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(player.is_finished(), "Back while loading closes the player");
    let last = std::iter::from_fn(|| events.try_recv().ok())
        .find(|event| !matches!(event, PlayerEvent::Buffered { .. }));
    assert_eq!(
        last,
        Some(PlayerEvent::Closed {
            time_ms: 0,
            duration_ms: 0,
        })
    );
}

#[test]
fn subtitle_files_are_added_as_one_argument_each() {
    let path = std::env::temp_dir().join("subtitles").join("1");
    let args = sub_add_args(&path, "English, [CC] ${path}", "eng").unwrap();
    assert_eq!(
        args,
        [
            "sub-add",
            path.to_str().unwrap(),
            "cached",
            "English, [CC] ${path}",
            "eng"
        ]
    );
    assert!(
        sub_add_args(Path::new("http://evil/x"), "t", "eng").is_none(),
        "only absolute local paths"
    );
}

#[test]
#[ignore = "needs libmpv"]
fn real_libmpv_loads_a_subtitle_file_without_an_extension() {
    let dir = std::env::temp_dir().join(format!("cineo-sub-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("1");
    std::fs::write(&file, "1\n00:00:00,000 --> 00:00:05,000\nHello\n").unwrap();
    let (url, _heads) = serve(wav(3));
    let (player, _, _events) =
        Player::start_with(&request(&url), Arc::new(|| {}), Output::None, AUDIO_ONLY).unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !player.status().loaded && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    player.add_subtitle(&file, "Addon English", "eng").unwrap();
    let path = file.to_str().unwrap();
    let selected = loop {
        let found = player
            .status()
            .tracks
            .iter()
            .find(|t| {
                t.kind == TrackKind::Subtitle
                    && t.selected
                    && t.external_file.as_deref() == Some(path)
            })
            .cloned();
        if found.is_some() || std::time::Instant::now() > deadline {
            break found;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let selected = selected.expect("the added subtitle is selected");
    assert_eq!(selected.title.as_deref(), Some("Addon English"));
    assert_eq!(selected.lang.as_deref(), Some("eng"));
    drop(player);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn preferred_subtitle_language_covers_every_tag() {
    let settings = Settings {
        subtitle_language: Language::from_code("fre"),
        secondary_subtitle_language: Language::from_code("eng"),
        ..Settings::default()
    };
    let options = settings_options(&settings);
    assert!(options.contains(&("slang", "fre,fra,fr,eng,en".to_owned())));
}

#[test]
fn settings_become_mpv_options() {
    let defaults = settings_options(&Settings::default());
    assert_eq!(defaults, [("volume", "100".to_owned())]);
    let settings = Settings {
        audio_language: Language::from_code("spa"),
        hardware_decoding: false,
        remember_volume: false,
        audio_output: AudioOutput::Stereo,
        audio_passthrough: true,
        ..Settings::default()
    };
    assert_eq!(
        settings_options(&settings),
        [
            ("hwdec", "no".to_owned()),
            ("alang", "spa,es".to_owned()),
            ("audio-channels", "stereo".to_owned()),
            ("audio-spdif", "ac3,eac3,dts,dts-hd,truehd".to_owned()),
        ]
    );
}

#[test]
#[ignore = "needs libmpv"]
fn real_libmpv_accepts_the_subtitle_language() {
    let (url, _heads) = serve(wav(1));
    let request = PlayRequest {
        settings: Settings {
            subtitle_language: Language::from_code("fre"),
            audio_language: Language::from_code("spa"),
            hardware_decoding: false,
            audio_output: AudioOutput::Stereo,
            audio_passthrough: true,
            ..Settings::default()
        },
        ..request(&url)
    };
    let started = Player::start_with(&request, Arc::new(|| {}), Output::None, AUDIO_ONLY);
    assert!(started.is_ok(), "mpv accepts the options from settings");
}

fn styled() -> Settings {
    Settings {
        subtitle_size: SubtitleSize::new(150),
        subtitle_font: SubtitleFont::Serif,
        subtitle_bold: true,
        subtitle_position: SubtitlePosition::new(10),
        subtitle_color: SubtitleColor::Yellow,
        subtitle_opacity: SubtitleOpacity::new(50),
        subtitle_background: SubtitleBackground::Gray,
        keep_subtitle_styles: false,
        ..Settings::default()
    }
}

#[test]
fn subtitle_style_becomes_mpv_options_only_when_changed() {
    assert!(style_options(&Settings::default()).is_empty());
    assert_eq!(
        style_options(&styled()),
        [
            ("sub-scale", "1.50".to_owned()),
            ("sub-font", "serif".to_owned()),
            ("sub-bold", "yes".to_owned()),
            ("sub-pos", "90".to_owned()),
            ("sub-color", "#7FFFE45C".to_owned()),
            ("sub-border-style", "opaque-box".to_owned()),
            ("sub-border-color", "#CC303030".to_owned()),
            ("sub-ass-override", "force".to_owned()),
        ]
    );
    let no_outline = Settings {
        subtitle_outline: SubtitleOutline::None,
        ..Settings::default()
    };
    assert_eq!(
        style_options(&no_outline),
        [("sub-border-size", "0".to_owned())]
    );
}

#[test]
fn a_live_style_change_back_to_the_defaults_resets_every_property() {
    let defaults = style_properties(&Settings::default());
    for (name, _) in style_options(&styled()) {
        assert!(defaults.iter().any(|(n, _)| *n == name), "{name} is reset");
    }
    assert_eq!(
        defaults,
        [
            ("sub-scale", "1.00".to_owned()),
            ("sub-font", "sans-serif".to_owned()),
            ("sub-bold", "no".to_owned()),
            ("sub-pos", "100".to_owned()),
            ("sub-color", "#FFFFFFFF".to_owned()),
            ("sub-border-style", "outline-and-shadow".to_owned()),
            ("sub-border-color", "#FF000000".to_owned()),
            ("sub-border-size", "1.65".to_owned()),
            ("sub-ass-override", "scale".to_owned()),
        ],
        "mpv 0.41's own defaults"
    );
}

#[test]
#[ignore = "needs libmpv"]
fn real_libmpv_accepts_every_live_subtitle_style_property() {
    let lib = ffi::lib().unwrap();
    let core = Core::create(lib).unwrap();
    core.initialize().unwrap();
    for (name, value) in [styled(), Settings::default()]
        .iter()
        .flat_map(style_properties)
    {
        assert!(core.set_property(name, &value).is_ok(), "{name}={value}");
    }
}

#[test]
#[ignore = "needs libmpv"]
fn real_libmpv_accepts_every_subtitle_style_option() {
    let lib = ffi::lib().unwrap();
    let core = Core::create(lib).unwrap();
    let gray_outline = Settings {
        subtitle_outline: SubtitleOutline::Gray,
        ..Settings::default()
    };
    let no_outline = Settings {
        subtitle_outline: SubtitleOutline::None,
        ..Settings::default()
    };
    for (name, value) in [styled(), gray_outline, no_outline]
        .iter()
        .flat_map(style_options)
    {
        assert!(core.set_option(name, &value).is_ok(), "{name}={value}");
    }
}
