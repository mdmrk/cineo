use super::*;

#[tokio::test(flavor = "multi_thread")]
async fn the_player_gets_only_what_it_draws_and_the_lists_come_back_when_it_closes() {
    let server = MockServer::start().await;
    let manifest = movie_addon(&server).await;
    let dir = local_data_dir();
    let players = Arc::new(FakePlayers::default());

    tokio::task::block_in_place(|| {
        let (cineo, views) = start_with(dir.path(), &players);
        open_movie(&cineo, &views, manifest);

        cineo.play(0, 0);
        let playing = wait_for(&views, |v| v.playback.is_some());
        assert!(playing.addons.is_empty());
        assert!(playing.board.is_empty());
        assert!(playing.recent.is_empty());
        assert!(playing.detail.is_none());

        let events = players.events.lock().unwrap().take().unwrap();
        events
            .send(PlayerEvent::Closed {
                time_ms: 61_000,
                duration_ms: 120_000,
            })
            .unwrap();
        let closed = wait_for(&views, |v| v.playback.is_none());
        assert_eq!(closed.addons.len(), 1);
        assert_eq!(closed.recent.len(), 1);
        assert!(closed.detail.is_some_and(|d| !d.streams.is_empty()));
        drop(cineo);
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stream_plays_on_the_surface_and_its_progress_reaches_the_library() {
    let server = MockServer::start().await;
    let manifest = movie_addon(&server).await;
    let dir = local_data_dir();
    let players = Arc::new(FakePlayers::default());

    tokio::task::block_in_place(|| {
        let (cineo, views) = start_with(dir.path(), &players);
        cineo.install_addon(manifest);
        wait_for(&views, |v| v.addons.len() == 1);
        cineo.attach_surface(7);
        cineo.open_detail("movie".into(), "tt0000001".into());
        let detail = wait_for(&views, |v| {
            v.detail.as_ref().is_some_and(|d| {
                d.meta == MetaStatus::Ready
                    && d.streams
                        .iter()
                        .all(|g| !matches!(g.items, StreamItems::Loading))
            })
        });
        let detail = detail.detail.unwrap();
        assert_eq!(detail.name.as_deref(), Some("First Example Film"));
        assert_eq!(detail.runtime.as_deref(), Some("120"));
        let StreamItems::Ready { streams, .. } = &detail.streams[0].items else {
            panic!("streams did not load: {:?}", detail.streams);
        };
        assert_eq!(streams[0].name.as_deref(), Some("1080p"));

        cineo.play(0, 0);
        let playing = wait_for(&views, |v| v.playback.is_some());
        let art = playing.playback.unwrap().art;
        assert_eq!(art.title, "First Example Film");
        assert_eq!(
            art.background.as_deref(),
            Some("https://img.example/background/tt0000001.jpg")
        );
        let request = players.requests.lock().unwrap()[0].clone();
        assert_eq!(request.url.as_str(), "https://media.example/v/1.mp4");
        assert!(
            request
                .headers
                .iter()
                .any(|(k, v)| k == "User-Agent" && v == "Cineo-Test")
        );
        assert_eq!(players.calls.lock().unwrap().as_slice(), ["attach 7"]);
        let status = cineo.player_status().unwrap();
        assert!(status.loaded);
        assert_eq!(status.position_s, 12.0);
        assert_eq!(
            status.cached,
            [
                CachedSpanView {
                    start: 0.0,
                    end: 0.25
                },
                CachedSpanView {
                    start: 0.5,
                    end: 1.0
                }
            ]
        );
        assert_eq!(
            status.stats,
            PlayerStatsView {
                network_bytes_per_sec: Some(2_048.0),
                buffered_s: Some(31.4),
                resolution: Some(VideoSizeView {
                    width: 1920,
                    height: 1080
                }),
                video_codec: Some("h264 High".into()),
                video_bits_per_sec: Some(4_500_000.0),
                decoder: Some(DecoderView::Hardware {
                    name: "mediacodec".into()
                }),
                dropped_frames: Some(3),
            }
        );

        assert_eq!(
            status.audio,
            [
                TrackView {
                    id: 1,
                    number: 1,
                    label: Some("eng · Stereo".into()),
                    selected: true
                },
                TrackView {
                    id: 2,
                    number: 2,
                    label: None,
                    selected: false
                },
            ]
        );
        assert_eq!(
            status.subtitles,
            [TrackView {
                id: 1,
                number: 1,
                label: Some("eng".into()),
                selected: false
            }]
        );
        assert!(status.subtitles_off);
        cineo.select_audio(Some(2));
        cineo.select_subtitle(None);

        cineo.resize_surface(1080, 2340);
        cineo.detach_surface();
        cineo.toggle_pause();
        cineo.stop_playback();
        assert_eq!(
            players.calls.lock().unwrap().as_slice(),
            [
                "attach 7",
                "SetAudio(Some(2))",
                "SetSubtitle(None)",
                "resize 1080x2340",
                "detach",
                "TogglePause",
                "Stop"
            ]
        );
        play_a_minute_and_close(&players);
        wait_for(&views, |v| v.playback.is_none());
        assert!(cineo.player_status().is_none());
        drop(cineo);
    });

    let library = Store::open_in(dir.path()).unwrap().library().unwrap();
    let item = library.iter().find(|i| i.id == "tt0000001").unwrap();
    assert_eq!(item.time_offset_ms, 61_000);
    assert_eq!(item.duration_ms, 120_000);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_screen_answers_while_a_finished_player_is_released() {
    let server = MockServer::start().await;
    let manifest = movie_addon(&server).await;
    let dir = local_data_dir();
    let players = Arc::new(FakePlayers::default());
    let (releasing_tx, releasing) = channel();
    let (release, release_rx) = channel();
    *players.slow_release.lock().unwrap() = Some(SlowRelease {
        releasing: releasing_tx,
        release: release_rx,
    });

    tokio::task::block_in_place(|| {
        let (cineo, views) = start_with(dir.path(), &players);
        open_movie(&cineo, &views, manifest);
        cineo.play(0, 0);
        wait_for(&views, |v| v.playback.is_some());

        let events = players.events.lock().unwrap().take().unwrap();
        events
            .send(PlayerEvent::Closed {
                time_ms: 1_000,
                duration_ms: 120_000,
            })
            .unwrap();
        releasing.recv_timeout(Duration::from_secs(10)).unwrap();
        let (answer, answered) = channel();
        let screen = Arc::clone(&cineo);
        std::thread::spawn(move || {
            screen.detach_surface();
            let _ = answer.send(screen.player_status().is_none());
        });
        let status = answered.recv_timeout(Duration::from_secs(2));
        release.send(()).unwrap();
        assert_eq!(status, Ok(true), "the screen waited for the player release");
        wait_for(&views, |v| v.playback.is_none());
        drop(cineo);
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn addon_subtitles_load_by_the_preferred_language_and_when_picked() {
    let server = MockServer::start().await;
    let manifest = movie_addon(&server).await;
    serve(
        &server,
        "/subs/manifest.json",
        fixture("addons/basic/manifest-subtitles.json"),
    )
    .await;
    let subtitles = format!(
        r#"{{"subtitles":[{{"id":"1","url":"{0}/files/spa","lang":"spa"}},{{"id":"2","url":"{0}/files/fre","lang":"fre","label":"French"}}]}}"#,
        server.uri()
    );
    Mock::given(method("GET"))
        .and(path_regex(r"^/subs/subtitles/movie/tt0000001/.*\.json$"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(subtitles, "application/json"))
        .mount(&server)
        .await;
    for (at, body) in [("/files/spa", "hola"), ("/files/fre", "salut")] {
        Mock::given(method("GET"))
            .and(path(at))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;
    }
    let dir = local_data_dir();
    let players = Arc::new(FakePlayers::default());

    tokio::task::block_in_place(|| {
        let (cineo, views) = start_with(dir.path(), &players);
        cineo.change_setting(SettingChange::SubtitleLanguage {
            code: Some("spa".into()),
        });
        cineo.install_addon(manifest);
        wait_for(&views, |v| v.addons.len() == 1);
        cineo.install_addon(format!("{}/subs/manifest.json", server.uri()));
        wait_for(&views, |v| v.addons.len() == 2);
        cineo.open_detail("movie".into(), "tt0000001".into());
        wait_for(&views, streams_ready);
        cineo.play(0, 0);
        wait_for(&views, |v| v.playback.is_some());

        let calls = || players.calls.lock().unwrap().clone();
        let until = |done: &dyn Fn(&[String]) -> bool| {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                let status = cineo.player_status().unwrap();
                if done(&calls()) {
                    return status;
                }
                assert!(Instant::now() < deadline, "calls: {:?}", calls());
                std::thread::sleep(Duration::from_millis(20));
            }
        };
        let status = until(&|c| c.iter().any(|c| c.starts_with("subtitle ")));
        assert_eq!(
            calls()
                .iter()
                .filter(|c| c.starts_with("subtitle "))
                .collect::<Vec<_>>(),
            ["subtitle Subtitles Fixture spa hola"],
            "the preferred language loads once"
        );
        let fre = format!("{}/files/fre", server.uri());
        assert!(status.addon_subtitles.contains(&AddonSubtitleView {
            url: fre.clone(),
            label: "fre · French".into(),
            selected: false,
        }));

        cineo.select_addon_subtitle(&fre);
        until(&|c| c.iter().any(|c| c == "subtitle French fre salut"));
        cineo.select_addon_subtitle(&format!("{}/files/spa", server.uri()));
        until(&|c| {
            c.iter()
                .filter(|c| c.starts_with("subtitle Subtitles Fixture spa"))
                .count()
                == 2
        });
        drop(cineo);
    });

    let requests = server.received_requests().await.unwrap();
    let fetched = |at: &str| requests.iter().filter(|r| r.url.path() == at).count();
    assert_eq!(
        fetched("/files/spa"),
        1,
        "a loaded subtitle is not fetched again"
    );
    assert_eq!(fetched("/files/fre"), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_played_stream_resumes_where_it_stopped_after_a_restart() {
    let server = MockServer::start().await;
    let manifest = movie_addon(&server).await;
    let dir = local_data_dir();

    tokio::task::block_in_place(|| {
        let players = Arc::new(FakePlayers::default());
        let (cineo, views) = start_with(dir.path(), &players);
        open_movie(&cineo, &views, manifest);
        cineo.play(0, 0);
        wait_for(&views, |v| v.playback.is_some());
        play_a_minute_and_close(&players);
        let stopped = wait_for(&views, |v| {
            v.playback.is_none() && !v.continue_watching.is_empty()
        });
        let item = &stopped.continue_watching[0];
        assert_eq!(item.id, "tt0000001");
        assert!(item.resumable && item.played && !item.favorite);
        assert!((item.progress - 61.0 / 120.0).abs() < 1e-6);
        assert_eq!(stopped.recent, stopped.continue_watching);
        drop(cineo);

        let players = Arc::new(FakePlayers::default());
        let (cineo, views) = start_with(dir.path(), &players);
        wait_for(&views, |v| v.continue_watching.len() == 1);
        cineo.resume("tt0000001".into());
        let playing = wait_for(&views, |v| v.playback.is_some());
        assert_eq!(playing.playback.unwrap().art.title, "First Example Film");
        let request = players.requests.lock().unwrap()[0].clone();
        assert_eq!(request.url.as_str(), "https://media.example/v/1.mp4");
        assert_eq!(request.start_ms, 61_000);
        assert!(
            request
                .headers
                .iter()
                .any(|(k, v)| k == "User-Agent" && v == "Cineo-Test")
        );
        players
            .events
            .lock()
            .unwrap()
            .take()
            .unwrap()
            .send(PlayerEvent::Closed {
                time_ms: 61_000,
                duration_ms: 120_000,
            })
            .unwrap();
        wait_for(&views, |v| v.playback.is_none());
        cineo.remove_from_library("tt0000001".into());
        wait_for(&views, |v| {
            v.continue_watching.is_empty() && v.recent.is_empty()
        });
        drop(cineo);
    });

    assert!(
        Store::open_in(dir.path())
            .unwrap()
            .library()
            .unwrap()
            .is_empty()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_favourite_stays_after_a_restart_until_it_is_unmarked_or_cleared() {
    let server = MockServer::start().await;
    let manifest = movie_addon(&server).await;
    let dir = local_data_dir();

    tokio::task::block_in_place(|| {
        let (cineo, views) = start(dir.path());
        open_movie(&cineo, &views, manifest);
        cineo.set_favorite("tt0000001".into(), true);
        let marked = wait_for(&views, |v| !v.favorites.is_empty());
        assert!(marked.detail.unwrap().favorite);
        assert_eq!(marked.favorites[0].name, "First Example Film");
        assert!(!marked.favorites[0].played);
        assert!(marked.recent.is_empty() && marked.continue_watching.is_empty());
        drop(cineo);

        let (cineo, views) = start(dir.path());
        wait_for(&views, |v| v.favorites.len() == 1);
        cineo.open_detail("movie".into(), "tt0000001".into());
        let opened = wait_for(&views, |v| v.detail.is_some());
        let detail = opened.detail.unwrap();
        assert_eq!(detail.name.as_deref(), Some("First Example Film"));
        assert!(detail.favorite);
        cineo.set_favorite("tt0000001".into(), false);
        wait_for(&views, |v| {
            v.favorites.is_empty() && v.detail.as_ref().is_some_and(|d| !d.favorite)
        });
        cineo.set_favorite("tt0000001".into(), true);
        wait_for(&views, |v| v.favorites.len() == 1);
        cineo.clear_library();
        wait_for(&views, |v| v.favorites.is_empty());
        drop(cineo);
    });

    assert!(
        Store::open_in(dir.path())
            .unwrap()
            .library()
            .unwrap()
            .is_empty()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn leaving_the_app_pauses_the_video_unless_turned_off() {
    let server = MockServer::start().await;
    let manifest = movie_addon(&server).await;
    let dir = local_data_dir();
    let players = Arc::new(FakePlayers::default());
    let pauses = || {
        players
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| *c == "SetPause(true)")
            .count()
    };

    tokio::task::block_in_place(|| {
        let (cineo, views) = start_with(dir.path(), &players);
        cineo.change_setting(SettingChange::Value {
            key: "pause_on_minimize".into(),
            value: "true".into(),
        });
        wait_for(&views, |v| v.settings.pause_on_minimize.on);
        open_movie(&cineo, &views, manifest);
        cineo.play(0, 0);
        wait_for(&views, |v| v.playback.is_some());

        cineo.left_app();
        shell_caught_up(&cineo, &views);
        assert_eq!(pauses(), 1);

        cineo.change_setting(SettingChange::Value {
            key: "pause_on_minimize".into(),
            value: "false".into(),
        });
        wait_for(&views, |v| !v.settings.pause_on_minimize.on);
        cineo.left_app();
        shell_caught_up(&cineo, &views);
        assert_eq!(pauses(), 1);

        cineo.stop_playback();
        play_a_minute_and_close(&players);
        wait_for(&views, |v| v.playback.is_none());
        drop(cineo);
    });
}

fn binge_streams(file: &str) -> Vec<u8> {
    format!(
        r#"{{"streams":[{{"name":"1080p","url":"https://media.example/{file}.mp4","behaviorHints":{{"bingeGroup":"ex-1080"}}}}]}}"#
    )
    .into_bytes()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_player_offers_the_next_episode_and_plays_it_on_request() {
    let server = MockServer::start().await;
    serve_manifest(&server).await;
    serve(
        &server,
        "/cfg/meta/series/tt0000010.json",
        fixture("addons/basic/meta-series.json"),
    )
    .await;
    Mock::given(method("GET"))
        .and(path_regex(
            r"^/cfg/stream/series/tt0000010(:|%3A)1(:|%3A)1\.json$",
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(binge_streams("e1"), "application/json"),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(
            r"^/cfg/stream/series/tt0000010(:|%3A)1(:|%3A)2\.json$",
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(binge_streams("e2"), "application/json"),
        )
        .mount(&server)
        .await;
    let manifest = format!("{}/cfg/manifest.json", server.uri());
    let dir = local_data_dir();
    let players = Arc::new(FakePlayers::default());

    tokio::task::block_in_place(|| {
        let (cineo, views) = start_with(dir.path(), &players);
        cineo.install_addon(manifest);
        wait_for(&views, |v| v.addons.len() == 1);
        cineo.open_detail("series".into(), "tt0000010".into());
        wait_for(&views, |v| {
            v.detail
                .as_ref()
                .is_some_and(|d| d.meta == MetaStatus::Ready)
        });
        cineo.select_video("tt0000010:1:1".into());
        wait_for(&views, streams_ready);
        cineo.play(0, 0);
        let playing = wait_for(&views, |v| v.playback.is_some()).playback.unwrap();
        assert!(playing.binge_watching);
        assert_eq!(playing.next_notice_s, 35);
        assert_eq!(playing.next, None);

        players
            .events
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .send(PlayerEvent::Progress {
                time_ms: 10_000,
                duration_ms: 1_320_000,
            })
            .unwrap();
        let next = wait_for(&views, |v| {
            v.playback.as_ref().is_some_and(|p| p.next.is_some())
        })
        .playback
        .unwrap()
        .next
        .unwrap();
        assert!(next.title.contains("Pilot, Part Two"), "{}", next.title);

        cineo.pause();
        assert!(
            players
                .calls
                .lock()
                .unwrap()
                .iter()
                .any(|c| c == "SetPause(true)")
        );
        wait_for(&views, |v| {
            v.playback
                .as_ref()
                .and_then(|p| p.next.as_ref())
                .is_some_and(|n| n.ready)
        });
        let first_events = players.events.lock().unwrap().clone().unwrap();
        cineo.play_next();
        wait_for(&views, |v| {
            v.playback.as_ref().is_some_and(|p| {
                p.next
                    .as_ref()
                    .is_none_or(|n| !n.title.contains("Part Two"))
            })
        });
        let requests = players.requests.lock().unwrap().clone();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[1].url.as_str(), "https://media.example/e2.mp4");
        assert_eq!(requests[1].video_id, "tt0000010:1:2");

        let _ = first_events.send(PlayerEvent::Closed {
            time_ms: 20_000,
            duration_ms: 1_320_000,
        });
        std::thread::sleep(Duration::from_millis(300));
        assert!(
            cineo.player_status().is_some(),
            "the replaced player's close stopped the next episode"
        );
        drop(cineo);
    });
}
