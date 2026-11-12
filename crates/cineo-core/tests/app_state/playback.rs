use super::*;

#[test]
fn playing_asks_subtitle_addons_with_the_stream_hints() {
    let mut state = detail_with_streams();
    update(&mut state, Action::InstallAddon(SUBS.into()));
    update(
        &mut state,
        Action::ManifestLoaded {
            transport: url(SUBS),
            result: Ok(manifest("basic/manifest-subtitles.json")),
            origin: ManifestOrigin::Install,
        },
    );
    let effects = update(
        &mut state,
        Action::Play {
            group: 0,
            stream: 0,
        },
    );
    let fetches: Vec<_> = effects
        .iter()
        .filter_map(|e| match e {
            Effect::FetchSubtitles { addon, path } => Some((addon, path)),
            _ => None,
        })
        .collect();
    assert_eq!(fetches.len(), 2, "every subtitles addon is asked");
    let (addon, path) = fetches[1];
    assert_eq!(*addon, url(SUBS));
    assert_eq!(
        path.to_url_path(),
        "subtitles/movie/tt0000001/videoHash=8e245d9679d31e12&videoSize=1234567&filename=film.mp4.json"
    );
    let stream_subs = &state.subtitles[0];
    assert_eq!(
        stream_subs.path, None,
        "the stream's own subtitles come first"
    );
    assert_eq!(stream_subs.subtitles.ready().map(Vec::len), Some(1));
    assert!(
        state.subtitles[1..]
            .iter()
            .all(|g| g.subtitles.is_loading())
    );

    let loaded = parse_subtitles_response(&fixture("basic/subtitles.json"))
        .unwrap()
        .value;
    update(
        &mut state,
        Action::SubtitlesLoaded {
            addon: addon.clone(),
            path: path.clone(),
            result: Ok(loaded),
        },
    );
    let addon_subs = &state.subtitles[2];
    assert_eq!(addon_subs.addon_name, "Subtitles Fixture");
    assert_eq!(addon_subs.subtitles.ready().map(Vec::len), Some(1));

    update(&mut state, Action::PlaybackStopped);
    assert!(
        state.subtitles.is_empty(),
        "subtitles belong to one playback"
    );
}

#[test]
fn subtitle_request_without_hints_has_no_extras() {
    let mut state = restore(&[BASIC, STREAMS, SUBS]);
    for (transport, file) in [
        (BASIC, "basic/manifest.json"),
        (STREAMS, "basic/manifest-streams.json"),
        (SUBS, "basic/manifest-subtitles.json"),
    ] {
        update(&mut state, loaded(transport, file));
    }
    ask_movie(&mut state);
    let meta = movie_meta();
    let (addon, path) = state.detail.as_ref().unwrap().meta_request.clone().unwrap();
    update(
        &mut state,
        Action::MetaLoaded {
            addon,
            path,
            result: Ok(Box::new(meta)),
        },
    );
    let mut streams = parse_stream_response(&fixture("basic/streams-movie.json"))
        .unwrap()
        .value;
    streams[0].video_hash = None;
    streams[0].video_size = None;
    streams[0].filename = None;
    let g = state.detail.as_ref().unwrap().streams[0].clone();
    update(
        &mut state,
        Action::StreamsLoaded {
            addon: g.addon,
            path: g.path,
            result: Ok(streams),
        },
    );
    let effects = update(
        &mut state,
        Action::Play {
            group: 0,
            stream: 0,
        },
    );
    let Some(Effect::FetchSubtitles { path, .. }) = effects.last() else {
        panic!("{effects:?}");
    };
    assert_eq!(path.to_url_path(), "subtitles/movie/tt0000001.json");
}

#[test]
fn the_subtitle_language_is_saved_and_sent_with_the_play_request() {
    let mut state = detail_with_streams();
    let spanish = Language::from_code("spa");
    let effects = update(
        &mut state,
        Action::ChangeSetting(Setting::SubtitleLanguage(spanish)),
    );
    assert!(
        matches!(effects.as_slice(), [Effect::SaveSettings(s)] if s.subtitle_language == spanish),
        "{effects:?}"
    );
    let effects = update(
        &mut state,
        Action::Play {
            group: 0,
            stream: 0,
        },
    );
    let play = effects.iter().find_map(|e| match e {
        Effect::Play(play) => Some(play),
        _ => None,
    });
    assert_eq!(play.and_then(|p| p.settings.subtitle_language), spanish);
}

fn binge_streams(file: &str) -> Vec<cineo_core::addon::Stream> {
    let json = format!(
        r#"{{"streams":[
            {{"url":"https://media.example/{file}-720.mp4","behaviorHints":{{"bingeGroup":"ex-720"}}}},
            {{"url":"https://media.example/{file}-1080.mp4","behaviorHints":{{"bingeGroup":"ex-1080"}}}}
        ]}}"#
    );
    parse_stream_response(json.as_bytes()).unwrap().value
}

fn series_playing(binge_watching: bool) -> State {
    let (mut state, _) = restored();
    update(
        &mut state,
        Action::ChangeSetting(Setting::BingeWatching(binge_watching)),
    );
    update(
        &mut state,
        Action::OpenDetail {
            content_type: ty("series"),
            id: "tt0000010".into(),
            preview: None,
        },
    );
    let meta = parse_meta_response(&fixture("basic/meta-series.json"))
        .unwrap()
        .value;
    let (addon, path) = state.detail.as_ref().unwrap().meta_request.clone().unwrap();
    update(
        &mut state,
        Action::MetaLoaded {
            addon,
            path,
            result: Ok(Box::new(meta)),
        },
    );
    update(&mut state, Action::SelectVideo("tt0000010:1:1".into()));
    let g = state.detail.as_ref().unwrap().streams[0].clone();
    update(
        &mut state,
        Action::StreamsLoaded {
            addon: g.addon,
            path: g.path,
            result: Ok(binge_streams("e1")),
        },
    );
    update(
        &mut state,
        Action::Play {
            group: 0,
            stream: 1,
        },
    );
    state
}

fn episode_progress(time_ms: u64) -> Action {
    Action::PlaybackProgress {
        meta_id: "tt0000010".into(),
        video_id: "tt0000010:1:1".into(),
        time_ms,
        duration_ms: 1_000_000,
        now_ms: 2_000_000_000_000,
    }
}

#[test]
fn the_next_video_skips_specials_and_unreleased_episodes() {
    let meta = parse_meta_response(&fixture("basic/meta-series.json"))
        .unwrap()
        .value;
    let next = |id: &str, now_ms: u64| meta.next_video(id, now_ms).map(|v| v.id.as_str());
    assert_eq!(next("tt0000010:1:1", 0), Some("tt0000010:1:2"));
    assert_eq!(next("tt0000010:1:2", 1_030_838_399_000), None);
    assert_eq!(
        next("tt0000010:1:2", 1_030_838_400_000),
        Some("tt0000010:2:1")
    );
    assert_eq!(next("tt0000010:2:1", u64::MAX), None);
}

#[test]
fn an_ended_episode_plays_the_next_one_from_the_same_binge_group() {
    let mut state = series_playing(true);
    let effects = update(&mut state, episode_progress(60_000));
    let fetches: Vec<_> = effects
        .iter()
        .filter_map(|e| match e {
            Effect::FetchStreams { addon, path } => Some((addon.clone(), path.clone())),
            _ => None,
        })
        .collect();
    let [(addon, path)] = fetches.as_slice() else {
        panic!("{effects:?}");
    };
    assert_eq!(addon, &url(BASIC), "only the addon of the playing stream");
    assert_eq!(path.id, "tt0000010:1:2");
    update(
        &mut state,
        Action::StreamsLoaded {
            addon: addon.clone(),
            path: path.clone(),
            result: Ok(binge_streams("e2")),
        },
    );
    update(&mut state, episode_progress(990_000));
    let effects = update(&mut state, Action::PlaybackEnded);
    let play = play_effect(&effects).expect("the next episode plays");
    assert_eq!(play.url.as_str(), "https://media.example/e2-1080.mp4");
    assert_eq!(play.video_id, "tt0000010:1:2");
}

#[test]
fn without_binge_watching_a_finished_episode_moves_continue_watching_on() {
    let mut state = series_playing(false);
    update(&mut state, episode_progress(990_000));
    let effects = update(&mut state, Action::PlaybackEnded);
    assert!(play_effect(&effects).is_none());
    let items = continue_watching(&state.library, WatchedAt::P92);
    let videos: Vec<&str> = items.iter().map(|i| i.video_id.as_str()).collect();
    assert_eq!(videos, ["tt0000010:1:2"]);
}

#[test]
fn an_addon_subtitle_is_named_by_its_label_or_else_its_addon() {
    let subtitles = parse_subtitles_response(
        br#"{"subtitles":[
            {"id":"1","url":"https://subs.example/a.srt","lang":"eng","label":"Forced"},
            {"id":"2","url":"https://subs.example/b.srt","lang":"spa"}
        ]}"#,
    )
    .unwrap()
    .value;
    let groups = [SubtitleGroup {
        addon: url("https://subs.example/manifest.json"),
        addon_name: "Subs".into(),
        path: None,
        subtitles: Loadable::Ready(subtitles),
    }];
    let find = |u: &str| find_subtitle(&groups, &u.parse().unwrap());
    assert_eq!(
        find("https://subs.example/a.srt"),
        Some(("Forced".into(), "eng".into()))
    );
    assert_eq!(
        find("https://subs.example/b.srt"),
        Some(("Subs".into(), "spa".into()))
    );
    assert_eq!(find("https://subs.example/c.srt"), None);
}
