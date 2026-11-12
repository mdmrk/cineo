use super::*;

#[test]
fn playing_an_http_stream_records_library_and_resumes() {
    let mut state = detail_with_streams();
    let effects = update(
        &mut state,
        Action::Play {
            group: 0,
            stream: 0,
        },
    );
    let [Effect::SaveLibraryItem(item), Effect::Play(play), ..] = effects.as_slice() else {
        panic!("{effects:?}");
    };
    assert_eq!(item.id, "tt0000001");
    assert_eq!(play.start_ms, 0);
    assert_eq!(play.url.as_str(), "https://media.example/v/1.mp4");
    assert_eq!(play.headers.len(), 2);
    assert_eq!(
        play.logo.as_ref().map(url::Url::as_str),
        Some("https://img.example/logo/tt0000001.png"),
        "the loading screen shows the item's logo"
    );
    assert_eq!(
        play.background.as_ref().map(url::Url::as_str),
        Some("https://img.example/background/tt0000001.jpg"),
        "the loading screen shows the item's backdrop behind the logo"
    );

    update(
        &mut state,
        Action::PlaybackProgress {
            meta_id: "tt0000001".into(),
            video_id: "tt0000001".into(),
            time_ms: 60_000,
            duration_ms: 600_000,
            now_ms: 5,
        },
    );
    assert_eq!(continue_watching(&state.library, WatchedAt::P92).len(), 1);
    let effects = update(
        &mut state,
        Action::Play {
            group: 0,
            stream: 0,
        },
    );
    let Some(play) = effects.iter().find_map(|e| match e {
        Effect::Play(play) => Some(play),
        _ => None,
    }) else {
        panic!("{effects:?}")
    };
    assert_eq!(play.start_ms, 60_000, "resumes");
}

#[test]
fn resuming_replays_the_saved_stream_at_the_saved_position() {
    let mut state = detail_with_streams();
    watched_a_minute(
        &mut state,
        Action::Play {
            group: 0,
            stream: 0,
        },
    );
    let saved = state.library[0].stream.as_deref().unwrap();
    assert_eq!(
        SavedStream::parse(&saved.to_json().unwrap()).as_ref(),
        Some(saved)
    );

    let effects = update(&mut state, Action::Resume("tt0000001".into()));
    let play = play_effect(&effects).unwrap();
    assert_eq!(play.url.as_str(), "https://media.example/v/1.mp4");
    assert_eq!(play.start_ms, 60_000);
    assert_eq!(play.headers.len(), 2);
    assert_eq!(
        play.logo.as_ref().map(url::Url::as_str),
        Some("https://img.example/logo/tt0000001.png")
    );
    assert!(state.detail.is_none(), "no detail page on the way");
    assert!(state.subtitles[0].path.is_none(), "the stream's own first");
    let fetches = effects
        .iter()
        .filter(|e| matches!(e, Effect::FetchSubtitles { .. }))
        .count();
    assert_eq!(fetches, state.subtitles.len() - 1, "subtitle addons asked");
}

fn favorite(id: &str, favorite: bool) -> Action {
    Action::SetFavorite {
        id: id.into(),
        favorite,
    }
}

fn played_item(id: &str, favorited: Option<u64>) -> LibraryItem {
    LibraryItem {
        id: id.into(),
        content_type: ty("movie"),
        name: format!("Film {id}"),
        poster: None,
        video_id: id.into(),
        time_offset_ms: 60_000,
        duration_ms: 600_000,
        updated_ms: 1,
        favorited,
        stream: None,
    }
}

fn with_library(items: Vec<LibraryItem>) -> State {
    let mut state = State::default();
    update(
        &mut state,
        Action::Restore {
            addons: Vec::new(),
            library: items,
        },
    );
    state
}

#[test]
fn favourites_keep_the_order_they_were_added_in_and_survive_playing() {
    let mut state = with_library(vec![played_item("a", None), played_item("b", Some(4))]);
    let effects = update(&mut state, favorite("a", true));
    let [Effect::SaveLibraryItem(item)] = effects.as_slice() else {
        panic!("{effects:?}")
    };
    assert_eq!(item.favorited, Some(5), "newer than every other favourite");
    assert!(
        update(&mut state, favorite("a", true)).is_empty(),
        "favouriting again keeps the order"
    );
    assert!(update(&mut state, favorite("missing", true)).is_empty());

    let mut state = detail_with_streams();
    update(&mut state, favorite("tt0000001", true));
    watched_a_minute(
        &mut state,
        Action::Play {
            group: 0,
            stream: 0,
        },
    );
    assert!(
        state.library[0].is_favorite(),
        "playing keeps the favourite"
    );
}

#[test]
fn favoriting_from_the_detail_page_adds_an_unplayed_item_and_unfavoriting_drops_it() {
    let mut state = detail_with_streams();
    let effects = update(&mut state, favorite("tt0000001", true));
    let [Effect::SaveLibraryItem(item)] = effects.as_slice() else {
        panic!("{effects:?}")
    };
    assert!(item.is_favorite());
    assert!(!item.was_played());
    assert_eq!(item.name, "First Example Film");
    assert_eq!(item.content_type, ty("movie"));
    assert!(continue_watching(&state.library, WatchedAt::P92).is_empty());

    assert_eq!(
        update(&mut state, favorite("tt0000001", false)),
        vec![Effect::DeleteLibraryItem("tt0000001".into())]
    );
    assert!(state.library.is_empty());

    update(&mut state, favorite("tt0000001", true));
    update(
        &mut state,
        Action::Play {
            group: 0,
            stream: 0,
        },
    );
    update(
        &mut state,
        Action::PlaybackProgress {
            meta_id: "tt0000001".into(),
            video_id: "tt0000001".into(),
            time_ms: 1_000,
            duration_ms: 600_000,
            now_ms: 5,
        },
    );
    let effects = update(&mut state, favorite("tt0000001", false));
    assert!(matches!(effects.as_slice(), [Effect::SaveLibraryItem(_)]));
    assert_eq!(state.library.len(), 1, "a played item stays");
}

#[test]
fn a_stream_is_saved_once_it_plays_and_an_unplayed_item_is_forgotten() {
    let mut state = detail_with_streams();
    let play = Action::Play {
        group: 0,
        stream: 0,
    };
    update(&mut state, play.clone());
    assert!(state.library[0].stream.is_none(), "not before it plays");
    let effects = update(&mut state, Action::PlaybackFailed("HTTP 404".into()));
    assert_eq!(effects, vec![Effect::DeleteLibraryItem("tt0000001".into())]);
    assert!(state.library.is_empty());
    assert!(state.detail.is_some(), "still on the detail page");

    update(&mut state, favorite("tt0000001", true));
    update(&mut state, play);
    update(&mut state, Action::PlaybackStopped);
    assert_eq!(state.library.len(), 1, "a favourite stays");
    assert!(state.library[0].stream.is_none());
}

#[test]
fn a_resume_that_played_does_not_fall_back_to_the_detail_page() {
    let mut state = detail_with_streams();
    watched_a_minute(
        &mut state,
        Action::Play {
            group: 0,
            stream: 0,
        },
    );
    update(&mut state, Action::Resume("tt0000001".into()));
    update(
        &mut state,
        Action::PlaybackProgress {
            meta_id: "tt0000001".into(),
            video_id: "tt0000001".into(),
            time_ms: 3_000_000,
            duration_ms: 6_000_000,
            now_ms: 9,
        },
    );
    update(&mut state, Action::PlaybackFailed("connection lost".into()));
    assert!(state.detail.is_none());
    assert_eq!(state.notice, Some(player_failed("connection lost", false)));
}

#[test]
fn removing_from_the_library_shows_no_notice() {
    let mut state = with_library(vec![played_item("a", None)]);
    let effects = update(&mut state, Action::RemoveFromLibrary("a".into()));
    assert_eq!(effects, vec![Effect::DeleteLibraryItem("a".into())]);
    assert!(state.library.is_empty());
    assert!(state.notice.is_none());
}

#[test]
fn removing_a_favourite_from_the_library_forgets_only_its_playback() {
    let mut item = played_item("a", Some(1));
    item.video_id = "a:1:2".into();
    let mut state = with_library(vec![item]);
    let effects = update(&mut state, Action::RemoveFromLibrary("a".into()));
    let kept = &state.library[0];
    assert!(kept.is_favorite());
    assert!(!kept.was_played());
    assert_eq!(
        (
            kept.video_id.as_str(),
            kept.time_offset_ms,
            kept.duration_ms
        ),
        ("a", 0, 0)
    );
    assert!(kept.stream.is_none());
    assert_eq!(effects, vec![Effect::SaveLibraryItem(kept.clone())]);
}

#[test]
fn resuming_without_a_saved_stream_opens_the_detail_page() {
    let (mut state, _) = restored();
    update(
        &mut state,
        Action::Restore {
            addons: Vec::new(),
            library: vec![LibraryItem {
                id: "tt0000001".into(),
                content_type: ty("movie"),
                name: "Film".into(),
                poster: None,
                video_id: "tt0000001".into(),
                time_offset_ms: 60_000,
                duration_ms: 600_000,
                updated_ms: 1,
                favorited: None,
                stream: None,
            }],
        },
    );
    let effects = update(&mut state, Action::Resume("tt0000001".into()));
    assert!(play_effect(&effects).is_none());
    assert!(matches!(effects.as_slice(), [Effect::FetchMeta { .. }]));
    assert_eq!(state.detail.as_ref().unwrap().id, "tt0000001");
}

#[test]
fn a_saved_stream_that_fails_opens_the_detail_page_with_a_notice() {
    let mut state = detail_with_streams();
    watched_a_minute(
        &mut state,
        Action::Play {
            group: 0,
            stream: 0,
        },
    );
    update(&mut state, Action::Resume("tt0000001".into()));
    let effects = update(&mut state, Action::PlaybackFailed("HTTP 403".into()));
    assert!(matches!(effects.as_slice(), [Effect::FetchMeta { .. }]));
    assert_eq!(state.detail.as_ref().unwrap().id, "tt0000001");
    assert_eq!(state.notice, Some(player_failed("HTTP 403", true)));

    update(&mut state, Action::CloseDetail);
    update(&mut state, Action::PlaybackFailed("HTTP 500".into()));
    assert!(state.detail.is_none(), "only a resumed playback goes back");
    assert_eq!(state.notice, Some(player_failed("HTTP 500", false)));
}

#[test]
fn a_saved_torrent_opens_the_detail_page_when_p2p_is_off() {
    let mut state = detail_with_p2p_accepted();
    watched_a_minute(&mut state, PLAY_TORRENT);
    update(
        &mut state,
        Action::ChangeSetting(Setting::P2pEnabled(false)),
    );
    let effects = update(&mut state, Action::Resume("tt0000001".into()));
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::StartTorrent(_) | Effect::Play(_)))
    );
    assert!(state.detail.is_some());
}

#[test]
fn unsupported_sources_are_explained_not_played() {
    let mut state = detail_with_streams();
    let effects = update(
        &mut state,
        Action::Play {
            group: 0,
            stream: 2,
        },
    );
    assert!(effects.is_empty());
    assert_eq!(
        state.notice,
        Some(Notice::UnsupportedSource(SourceKind::YouTube))
    );
}

#[test]
fn continue_watching_excludes_finished_and_sorts_by_recency() {
    let item = |id: &str, t: u64, d: u64, u: u64| LibraryItem {
        id: id.into(),
        content_type: ty("movie"),
        name: id.into(),
        poster: None,
        video_id: id.into(),
        time_offset_ms: t,
        duration_ms: d,
        updated_ms: u,
        favorited: None,
        stream: None,
    };
    let items = vec![
        item("a", 10, 100, 1),
        item("b", 99, 100, 3),
        item("c", 50, 100, 2),
        item("d", 0, 100, 4),
    ];
    let ids = |watched_at| -> Vec<_> {
        continue_watching(&items, watched_at)
            .iter()
            .map(|i| i.id.as_str())
            .collect()
    };
    assert_eq!(ids(WatchedAt::P92), vec!["c", "a"]);
    assert_eq!(
        items[1].resume_ms("b", WatchedAt::P92),
        0,
        "finished restarts"
    );
    assert_eq!(items[2].resume_ms("other", WatchedAt::P92), 0);
}

#[test]
fn the_watched_threshold_decides_what_is_finished() {
    let item = LibraryItem {
        id: "a".into(),
        content_type: ty("movie"),
        name: "a".into(),
        poster: None,
        video_id: "a".into(),
        time_offset_ms: 86,
        duration_ms: 100,
        updated_ms: 1,
        favorited: None,
        stream: None,
    };
    assert!(item.is_finished(WatchedAt::P85));
    assert!(!item.is_finished(WatchedAt::P90));
    assert_eq!(item.resume_ms("a", WatchedAt::P85), 0);
    assert_eq!(item.resume_ms("a", WatchedAt::P90), 86);
    let items = [item];
    assert!(continue_watching(&items, WatchedAt::P85).is_empty());
}

#[test]
fn clearing_the_library_forgets_every_item() {
    let mut state = State::default();
    let item = LibraryItem {
        id: "a".into(),
        content_type: ty("movie"),
        name: "a".into(),
        poster: None,
        video_id: "a".into(),
        time_offset_ms: 1,
        duration_ms: 100,
        updated_ms: 1,
        favorited: None,
        stream: None,
    };
    update(
        &mut state,
        Action::Restore {
            addons: Vec::new(),
            library: vec![item],
        },
    );
    assert_eq!(
        update(&mut state, Action::ClearLibrary),
        vec![Effect::ClearLibrary]
    );
    assert!(state.library.is_empty());
}
