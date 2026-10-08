//! The application state machine (ADR-0001), driven by actions and fake IO
//! results. Asserts on state and on the effects requested.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers panic on purpose"
)]

use cineo_core::addon::{
    ContentType, Manifest, MetaPreview, ResourcePath, SourceKind, TransportUrl,
    parse_catalog_response, parse_manifest, parse_meta_response, parse_stream_response,
    parse_subtitles_response,
};
use cineo_core::app::{
    Action, Effect, Language, LibraryItem, Loadable, Notice, PlaybackFailure, Problem, SavedStream,
    Setting, Settings, State, TorrentRequest, TorrentStatus, WatchedAt, continue_watching, update,
};

fn player_failed(reason: &str, pick_another: bool) -> Notice {
    Notice::PlaybackFailed {
        failure: PlaybackFailure::Player(reason.into()),
        pick_another,
    }
}

fn fixture(path: &str) -> Vec<u8> {
    let full = format!("{}/../../tests/addons/{path}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&full).unwrap_or_else(|err| panic!("reading {full}: {err}"))
}

fn manifest(path: &str) -> Box<Manifest> {
    Box::new(parse_manifest(&fixture(path)).unwrap().value)
}

fn url(s: &str) -> TransportUrl {
    TransportUrl::parse(s).unwrap()
}

fn ty(s: &str) -> ContentType {
    ContentType::new(s).unwrap()
}

const BASIC: &str = "https://basic.example/manifest.json";
const STREAMS: &str = "https://streams.example/manifest.json";
const SUBS: &str = "https://subs.example/manifest.json";

fn restored() -> (State, Vec<Effect>) {
    let mut state = State::default();
    let effects = update(
        &mut state,
        Action::Restore {
            addons: vec![url(BASIC), url(STREAMS)],
            library: Vec::new(),
        },
    );
    assert_eq!(effects.len(), 2, "one manifest fetch per addon");
    let mut board = update(&mut state, loaded(BASIC, "basic/manifest.json"));
    board.extend(update(
        &mut state,
        loaded(STREAMS, "basic/manifest-streams.json"),
    ));
    (state, board)
}

fn loaded(transport: &str, fixture_path: &str) -> Action {
    Action::ManifestLoaded {
        transport: url(transport),
        result: Ok(manifest(fixture_path)),
        install: false,
    }
}

fn load_failed(transport: &str) -> Action {
    Action::ManifestLoaded {
        transport: url(transport),
        result: Err("HTTP 503".into()),
        install: false,
    }
}

fn restore(addons: &[&str]) -> State {
    let mut state = State::default();
    update(
        &mut state,
        Action::Restore {
            addons: addons.iter().map(|a| url(a)).collect(),
            library: Vec::new(),
        },
    );
    state
}

fn addon_names(state: &State) -> Vec<&str> {
    state
        .addons
        .iter()
        .map(|a| a.manifest.name.as_str())
        .collect()
}

fn catalog_items() -> Vec<MetaPreview> {
    parse_catalog_response(&fixture("basic/catalog-movie-top.json"))
        .unwrap()
        .value
        .metas
}

#[test]
fn restore_loads_addons_in_order_then_the_board() {
    let (state, board) = restored();
    let names: Vec<_> = state
        .addons
        .iter()
        .map(|a| a.manifest.name.as_str())
        .collect();
    assert_eq!(names, vec!["Basic Fixture", "Streams Fixture"]);
    let catalogs: Vec<_> = state
        .board
        .iter()
        .map(|r| (r.target.name.as_str(), r.target.path.content_type.as_str()))
        .collect();
    assert_eq!(
        catalogs,
        vec![("Top Movies", "movie"), ("Multi-genre", "series")]
    );
    assert_eq!(board.len(), 2);
    assert!(
        board
            .iter()
            .all(|e| matches!(e, Effect::FetchCatalog { .. }))
    );
}

#[test]
fn board_rows_load_as_soon_as_their_addon_does() {
    let mut state = restore(&[STREAMS, BASIC]);
    let effects = update(&mut state, loaded(BASIC, "basic/manifest.json"));
    assert_eq!(state.board.len(), 2, "no wait for the streams addon");
    assert_eq!(effects.len(), 2);
    let effects = update(&mut state, loaded(STREAMS, "basic/manifest-streams.json"));
    assert!(effects.is_empty(), "loaded rows are not fetched again");
}

#[test]
fn addons_keep_the_saved_order_whatever_order_they_load_in() {
    let mut state = restore(&[BASIC, STREAMS]);
    update(&mut state, loaded(STREAMS, "basic/manifest-streams.json"));
    update(&mut state, loaded(BASIC, "basic/manifest.json"));
    assert_eq!(
        addon_names(&state),
        vec!["Basic Fixture", "Streams Fixture"]
    );
}

#[test]
fn an_addon_that_fails_to_load_stays_installed() {
    const OTHER: &str = "https://other.example/manifest.json";
    let mut state = restore(&[OTHER, BASIC, STREAMS]);
    update(&mut state, load_failed(OTHER));
    update(&mut state, loaded(BASIC, "basic/manifest.json"));
    update(&mut state, loaded(STREAMS, "basic/manifest-streams.json"));
    assert!(state.notice.is_some());

    let effects = update(&mut state, Action::MoveAddon { from: 1, to: 0 });
    assert_eq!(
        effects[0],
        Effect::SaveAddons(vec![url(OTHER), url(STREAMS), url(BASIC)])
    );
    let effects = update(&mut state, Action::RemoveAddon(url(BASIC)));
    assert_eq!(
        effects[0],
        Effect::SaveAddons(vec![url(OTHER), url(STREAMS)])
    );
    assert_eq!(state.unavailable_addons(), vec![&url(OTHER)]);

    let effects = update(&mut state, Action::InstallAddon(OTHER.into()));
    assert!(
        matches!(
            effects.as_slice(),
            [Effect::FetchManifest { install: true, .. }]
        ),
        "an unavailable addon can be retried"
    );
    update(
        &mut state,
        Action::ManifestLoaded {
            transport: url(OTHER),
            result: Ok(manifest("basic/manifest.json")),
            install: true,
        },
    );
    assert_eq!(state.installed, vec![url(OTHER), url(STREAMS)]);
    assert_eq!(state.addons.len(), 2);
    assert!(state.unavailable_addons().is_empty());
}

#[test]
fn a_manifest_that_arrives_after_removal_is_ignored() {
    let mut state = restore(&[BASIC, STREAMS]);
    update(&mut state, Action::RemoveAddon(url(BASIC)));
    let effects = update(&mut state, loaded(BASIC, "basic/manifest.json"));
    assert!(effects.is_empty(), "{effects:?}");
    assert!(state.addons.is_empty());
    assert_eq!(state.installed, vec![url(STREAMS)]);
}

#[test]
fn reordering_and_removing_addons_reuse_loaded_rows() {
    let (mut state, _) = restored();
    let effects = update(&mut state, Action::MoveAddon { from: 0, to: 1 });
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::FetchCatalog { .. })),
        "{effects:?}"
    );
    assert_eq!(state.board.len(), 2);
}

#[test]
fn install_validates_and_rejects_duplicates() {
    let (mut state, _) = restored();
    assert!(
        update(
            &mut state,
            Action::InstallAddon("ftp://x/manifest.json".into())
        )
        .is_empty()
    );
    assert!(matches!(state.install, Some(Loadable::Failed(_))));

    assert!(update(&mut state, Action::InstallAddon(BASIC.into())).is_empty());
    assert_eq!(
        state.install,
        Some(Loadable::Failed(Problem::AlreadyInstalled))
    );

    let effects = update(
        &mut state,
        Action::InstallAddon("https://new.example/manifest.json".into()),
    );
    assert!(matches!(
        effects.as_slice(),
        [Effect::FetchManifest { install: true, .. }]
    ));
    let effects = update(
        &mut state,
        Action::ManifestLoaded {
            transport: url("https://new.example/manifest.json"),
            result: Err("HTTP 404".into()),
            install: true,
        },
    );
    assert!(effects.is_empty());
    assert_eq!(
        state.install,
        Some(Loadable::Failed(Problem::Request("HTTP 404".into())))
    );
    assert_eq!(state.addons.len(), 2, "failed install adds nothing");
}

#[test]
fn removing_an_addon_saves_and_reloads_the_board() {
    let (mut state, _) = restored();
    let effects = update(&mut state, Action::RemoveAddon(url(BASIC)));
    assert_eq!(effects[0], Effect::SaveAddons(vec![url(STREAMS)]));
    assert!(state.board.is_empty(), "the streams addon has no catalogs");
}

#[test]
fn catalog_results_fill_matching_rows_and_stale_ones_are_dropped() {
    let (mut state, _) = restored();
    let path = state.board[0].target.path.clone();
    update(
        &mut state,
        Action::CatalogLoaded {
            addon: url(BASIC),
            path: path.clone(),
            result: Ok(catalog_items()),
        },
    );
    assert_eq!(state.board[0].items.ready().map(Vec::len), Some(2));
    assert!(state.board[1].items.is_loading(), "other rows untouched");

    let before = state.clone();
    update(
        &mut state,
        Action::CatalogLoaded {
            addon: url(STREAMS),
            path,
            result: Ok(Vec::new()),
        },
    );
    assert_eq!(state, before, "a result nobody asked for changes nothing");
}

#[test]
fn search_asks_only_catalogs_that_support_search() {
    let (mut state, _) = restored();
    let effects = update(&mut state, Action::Search("  matrix ".into()));
    assert_eq!(state.search_query, "matrix");
    let [Effect::FetchCatalog { path, .. }] = effects.as_slice() else {
        panic!("expected one catalog fetch, got {effects:?}");
    };
    assert_eq!(
        path.to_url_path(),
        "catalog/movie/search/search=matrix.json"
    );
}

#[test]
fn meta_falls_back_to_the_next_addon_then_movie_streams_load_from_all() {
    let (mut state, _) = restored();
    let effects = update(
        &mut state,
        Action::OpenDetail {
            content_type: ty("movie"),
            id: "tt0000001".into(),
            preview: None,
        },
    );
    let [Effect::FetchMeta { addon, path }] = effects.as_slice() else {
        panic!("{effects:?}");
    };
    assert_eq!(addon, &url(BASIC));
    let meta = parse_meta_response(&fixture("basic/meta-movie.json"))
        .unwrap()
        .value;
    let effects = update(
        &mut state,
        Action::MetaLoaded {
            addon: addon.clone(),
            path: path.clone(),
            result: Ok(Box::new(meta)),
        },
    );
    assert_eq!(effects.len(), 2);
    let detail = state.detail.as_ref().unwrap();
    assert_eq!(detail.selected_video.as_deref(), Some("tt0000001"));

    let groups: Vec<(TransportUrl, ResourcePath)> = detail
        .streams
        .iter()
        .map(|g| (g.addon.clone(), g.path.clone()))
        .collect();
    update(
        &mut state,
        Action::StreamsLoaded {
            addon: groups[0].0.clone(),
            path: groups[0].1.clone(),
            result: Err("timed out".into()),
        },
    );
    let streams = parse_stream_response(&fixture("basic/streams-movie.json"))
        .unwrap()
        .value;
    update(
        &mut state,
        Action::StreamsLoaded {
            addon: groups[1].0.clone(),
            path: groups[1].1.clone(),
            result: Ok(streams),
        },
    );
    let detail = state.detail.as_ref().unwrap();
    assert_eq!(
        detail.streams[0].streams,
        Loadable::Failed(Problem::Request("timed out".into()))
    );
    assert_eq!(detail.streams[1].streams.ready().map(Vec::len), Some(6));
}

#[test]
fn meta_failure_tries_the_next_candidate() {
    let mut state = State::default();
    update(
        &mut state,
        Action::Restore {
            addons: vec![url(BASIC), url("https://basic2.example/manifest.json")],
            library: Vec::new(),
        },
    );
    for t in [BASIC, "https://basic2.example/manifest.json"] {
        update(
            &mut state,
            Action::ManifestLoaded {
                transport: url(t),
                result: Ok(manifest("basic/manifest.json")),
                install: false,
            },
        );
    }
    let effects = update(
        &mut state,
        Action::OpenDetail {
            content_type: ty("series"),
            id: "tt0000010".into(),
            preview: None,
        },
    );
    let [Effect::FetchMeta { addon, path }] = effects.as_slice() else {
        panic!()
    };
    let effects = update(
        &mut state,
        Action::MetaLoaded {
            addon: addon.clone(),
            path: path.clone(),
            result: Err("HTTP 500".into()),
        },
    );
    let [Effect::FetchMeta { addon: next, .. }] = effects.as_slice() else {
        panic!("{effects:?}")
    };
    assert_eq!(next, &url("https://basic2.example/manifest.json"));
    assert!(state.detail.as_ref().unwrap().meta.is_loading());
}

fn detail_with_streams() -> State {
    let (mut state, _) = restored();
    update(
        &mut state,
        Action::OpenDetail {
            content_type: ty("movie"),
            id: "tt0000001".into(),
            preview: None,
        },
    );
    let meta = parse_meta_response(&fixture("basic/meta-movie.json"))
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
    let streams = parse_stream_response(&fixture("basic/streams-movie.json"))
        .unwrap()
        .value;
    let g = state.detail.as_ref().unwrap().streams[0].clone();
    update(
        &mut state,
        Action::StreamsLoaded {
            addon: g.addon,
            path: g.path,
            result: Ok(streams),
        },
    );
    state
}

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

fn play_effect(effects: &[Effect]) -> Option<&cineo_core::app::PlayRequest> {
    effects.iter().find_map(|e| match e {
        Effect::Play(play) => Some(&**play),
        _ => None,
    })
}

fn watched_a_minute(state: &mut State, play: Action) {
    update(state, play);
    update(
        state,
        Action::PlaybackProgress {
            meta_id: "tt0000001".into(),
            video_id: "tt0000001".into(),
            time_ms: 60_000,
            duration_ms: 600_000,
            now_ms: 5,
        },
    );
    update(state, Action::PlaybackStopped);
    update(state, Action::CloseDetail);
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
fn dismissing_from_continue_watching_keeps_the_item_and_its_favourite() {
    let mut state = with_library(vec![played_item("a", Some(1))]);
    let effects = update(&mut state, Action::DismissContinueWatching("a".into()));
    let [Effect::SaveLibraryItem(item)] = effects.as_slice() else {
        panic!("{effects:?}")
    };
    assert_eq!(item.time_offset_ms, 0);
    assert!(item.is_favorite());
    assert!(continue_watching(&state.library, WatchedAt::P92).is_empty());
    assert_eq!(state.library.len(), 1);
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

const FIXTURE_HASH: &str = "0123456789abcdef0123456789abcdef01234567";
const PLAY_TORRENT: Action = Action::Play {
    group: 0,
    stream: 1,
};

fn detail_with_p2p_accepted() -> State {
    let mut state = detail_with_streams();
    update(
        &mut state,
        Action::RestoreSettings(Settings {
            p2p_acknowledged: true,
            ..Settings::default()
        }),
    );
    state
}

#[test]
fn the_first_torrent_play_asks_for_p2p_consent_and_accepting_starts_the_engine() {
    let mut state = detail_with_streams();
    assert!(update(&mut state, PLAY_TORRENT).is_empty());
    assert_eq!(state.p2p_prompt, Some((0, 1)));
    assert!(state.torrent.is_none(), "nothing starts before consent");

    let effects = update(&mut state, Action::AcceptP2p);
    let [
        Effect::SaveSettings(settings),
        Effect::SaveLibraryItem(item),
        Effect::StartTorrent(request),
        ..,
    ] = effects.as_slice()
    else {
        panic!("{effects:?}");
    };
    assert!(settings.p2p_acknowledged);
    assert_eq!(item.id, "tt0000001");
    assert_eq!(
        request,
        &TorrentRequest {
            info_hash: FIXTURE_HASH.into(),
            file_idx: Some(2),
            filename: None,
            trackers: vec![url::Url::parse("udp://tracker.example:1337").unwrap()],
        }
    );
    assert_eq!(state.p2p_prompt, None);
    let torrent = state.torrent.as_ref().unwrap();
    assert_eq!(torrent.status, TorrentStatus::Starting);
    assert!(
        torrent.connecting().is_some(),
        "the shell can show the player while the engine prepares the file"
    );
}

#[test]
fn declining_p2p_consent_plays_nothing_and_asks_again_next_time() {
    let mut state = detail_with_streams();
    update(&mut state, PLAY_TORRENT);
    assert!(update(&mut state, Action::DeclineP2p).is_empty());
    assert_eq!(state.p2p_prompt, None);
    assert!(!state.settings.p2p_acknowledged);
    update(&mut state, PLAY_TORRENT);
    assert_eq!(state.p2p_prompt, Some((0, 1)));
}

#[test]
fn a_served_torrent_plays_from_the_loopback_url_and_stopping_playback_stops_the_engine() {
    let mut state = detail_with_p2p_accepted();
    update(&mut state, PLAY_TORRENT);
    let served = url::Url::parse("http://127.0.0.1:40000/token/0").unwrap();

    let stale = update(
        &mut state,
        Action::TorrentReady {
            info_hash: "f".repeat(40),
            url: served.clone(),
        },
    );
    assert!(stale.is_empty());

    let effects = update(
        &mut state,
        Action::TorrentReady {
            info_hash: FIXTURE_HASH.into(),
            url: served.clone(),
        },
    );
    let [Effect::Play(play)] = effects.as_slice() else {
        panic!("{effects:?}");
    };
    assert_eq!(play.url, served);
    assert!(play.headers.is_empty());
    assert_eq!(play.meta_id, "tt0000001");
    assert_eq!(
        state.torrent.as_ref().unwrap().connecting(),
        None,
        "handed to the player"
    );

    let status = TorrentStatus::Streaming {
        peers: 3,
        download_bytes_per_sec: 1000,
        downloaded: 10,
        size: 100,
    };
    update(
        &mut state,
        Action::TorrentStatus {
            info_hash: FIXTURE_HASH.into(),
            status: status.clone(),
        },
    );
    assert_eq!(state.torrent.as_ref().unwrap().status, status);

    assert_eq!(
        update(&mut state, Action::PlaybackStopped),
        vec![
            Effect::StopTorrent,
            Effect::DeleteLibraryItem("tt0000001".into())
        ],
        "it never played, so it is not kept"
    );
    assert!(state.torrent.is_none());
    assert!(update(&mut state, Action::PlaybackStopped).is_empty());
}

#[test]
fn an_engine_url_that_is_not_loopback_http_is_refused() {
    let mut state = detail_with_p2p_accepted();
    update(&mut state, PLAY_TORRENT);
    let effects = update(
        &mut state,
        Action::TorrentReady {
            info_hash: FIXTURE_HASH.into(),
            url: url::Url::parse("http://192.168.1.10:8080/x").unwrap(),
        },
    );
    assert_eq!(effects, vec![Effect::StopTorrent]);
    assert!(state.torrent.is_none());
    assert!(state.notice.is_some());
}

#[test]
fn an_engine_failure_is_shown_and_stops_the_torrent() {
    let mut state = detail_with_p2p_accepted();
    update(&mut state, PLAY_TORRENT);
    let effects = update(
        &mut state,
        Action::TorrentFailed {
            info_hash: FIXTURE_HASH.into(),
            reason: "no peers found".into(),
        },
    );
    assert_eq!(
        effects,
        vec![
            Effect::StopTorrent,
            Effect::DeleteLibraryItem("tt0000001".into())
        ]
    );
    assert_eq!(
        state.notice,
        Some(Notice::PlaybackFailed {
            failure: PlaybackFailure::Torrent("no peers found".into()),
            pick_another: false,
        })
    );
}

#[test]
fn turning_p2p_off_blocks_torrents_and_stops_a_running_one() {
    let mut state = detail_with_p2p_accepted();
    update(&mut state, PLAY_TORRENT);
    let effects = update(
        &mut state,
        Action::ChangeSetting(Setting::P2pEnabled(false)),
    );
    let [Effect::SaveSettings(settings), Effect::StopTorrent] = effects.as_slice() else {
        panic!("{effects:?}");
    };
    assert!(!settings.p2p_enabled);
    assert!(state.torrent.is_none());

    assert!(update(&mut state, PLAY_TORRENT).is_empty());
    assert_eq!(state.notice, Some(Notice::TorrentsOff));
}

#[test]
fn playing_an_http_stream_stops_a_running_torrent() {
    let mut state = detail_with_p2p_accepted();
    update(&mut state, PLAY_TORRENT);
    let effects = update(
        &mut state,
        Action::Play {
            group: 0,
            stream: 0,
        },
    );
    assert!(
        matches!(
            effects.as_slice(),
            [
                Effect::SaveLibraryItem(_),
                Effect::StopTorrent,
                Effect::Play(_),
                ..
            ]
        ),
        "{effects:?}"
    );
}

#[test]
fn discover_pages_with_skip_and_deduplicates() {
    let (mut state, _) = restored();
    let target = state.board[0].target.clone();
    let effects = update(
        &mut state,
        Action::OpenDiscover {
            addon: target.addon.clone(),
            path: target.path.clone(),
        },
    );
    let [Effect::FetchCatalog { path, .. }] = effects.as_slice() else {
        panic!()
    };
    update(
        &mut state,
        Action::CatalogLoaded {
            addon: target.addon.clone(),
            path: path.clone(),
            result: Ok(catalog_items()),
        },
    );
    assert_eq!(state.discover.items.len(), 2);
    assert_eq!(state.discover.next_skip, Some(2));

    let effects = update(&mut state, Action::LoadMoreDiscover);
    let [Effect::FetchCatalog { path, .. }] = effects.as_slice() else {
        panic!()
    };
    assert_eq!(path.to_url_path(), "catalog/movie/top/skip=2.json");
    let mut page = catalog_items();
    page.truncate(1); // a repeated item
    update(
        &mut state,
        Action::CatalogLoaded {
            addon: target.addon,
            path: path.clone(),
            result: Ok(page),
        },
    );
    assert_eq!(state.discover.items.len(), 2, "duplicates dropped");

    let effects = update(&mut state, Action::SetDiscoverGenre(Some("Drama".into())));
    let [Effect::FetchCatalog { path, .. }] = effects.as_slice() else {
        panic!()
    };
    assert_eq!(path.to_url_path(), "catalog/movie/top/genre=Drama.json");
    assert!(state.discover.items.is_empty());
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

#[test]
fn playing_asks_subtitle_addons_with_the_stream_hints() {
    let mut state = detail_with_streams();
    update(&mut state, Action::InstallAddon(SUBS.into()));
    update(
        &mut state,
        Action::ManifestLoaded {
            transport: url(SUBS),
            result: Ok(manifest("basic/manifest-subtitles.json")),
            install: true,
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
    update(
        &mut state,
        Action::OpenDetail {
            content_type: ty("movie"),
            id: "tt0000001".into(),
            preview: None,
        },
    );
    let meta = parse_meta_response(&fixture("basic/meta-movie.json"))
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

#[test]
fn resetting_settings_restores_defaults_but_keeps_the_p2p_notice_accepted() {
    let mut state = detail_with_p2p_accepted();
    update(
        &mut state,
        Action::ChangeSetting(Setting::SubtitleLanguage(Language::from_code("spa"))),
    );
    let effects = update(&mut state, Action::ResetSettings);
    let expected = Settings {
        p2p_acknowledged: true,
        ..Settings::default()
    };
    assert_eq!(effects, vec![Effect::SaveSettings(expected)]);
    assert_eq!(state.settings, expected);
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
