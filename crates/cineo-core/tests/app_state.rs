//! The application state machine (ADR-0001), driven by actions and fake IO
//! results. Asserts on state and on the effects requested.

// Test helpers panic on purpose: a panic is a failed assertion.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use cineo_core::addon::{
    ContentType, Manifest, MetaPreview, ResourcePath, TransportUrl, parse_catalog_response,
    parse_manifest, parse_meta_response, parse_stream_response,
};
use cineo_core::app::{
    Action, Effect, LibraryItem, Loadable, Settings, State, TorrentRequest, TorrentStatus,
    continue_watching, update,
};

fn fixture(path: &str) -> Vec<u8> {
    let full = format!(
        "{}/../../tests/fixtures/addons/{path}",
        env!("CARGO_MANIFEST_DIR")
    );
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

/// State with both fixture addons installed via `Restore`.
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
    // Browsable catalogs only: `search` requires an extra, so 2 rows.
    let titles: Vec<_> = state
        .board
        .iter()
        .map(|r| r.target.title.as_str())
        .collect();
    assert_eq!(titles, vec!["Top Movies Movies", "Multi-genre Series"]);
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
    assert!(matches!(&state.install, Some(Loadable::Failed(m)) if m.contains("already")));

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
    assert_eq!(state.install, Some(Loadable::Failed("HTTP 404".into())));
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
    // Only the basic addon serves meta, so there is no fallback.
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
    // Movie: its own id is the video; both addons serve `stream` for `tt`.
    assert_eq!(effects.len(), 2);
    let detail = state.detail.as_ref().unwrap();
    assert_eq!(detail.selected_video.as_deref(), Some("tt0000001"));

    // One addon fails, the other answers: both outcomes stay visible.
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
        Loadable::Failed("timed out".into())
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
    let [Effect::SaveLibraryItem(item), Effect::Play(play)] = effects.as_slice() else {
        panic!("{effects:?}");
    };
    assert_eq!(item.id, "tt0000001");
    assert_eq!(play.start_ms, 0);
    assert_eq!(play.url.as_str(), "https://media.example/v/1.mp4");
    assert_eq!(play.headers.len(), 2);
    assert_eq!(play.subtitles.len(), 1);

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
    assert_eq!(continue_watching(&state.library).len(), 1);
    let effects = update(
        &mut state,
        Action::Play {
            group: 0,
            stream: 0,
        },
    );
    let Some(Effect::Play(play)) = effects.last() else {
        panic!()
    };
    assert_eq!(play.start_ms, 60_000, "resumes");
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
        state.notice.as_deref(),
        Some("YouTube streams are not supported yet")
    );
}

const FIXTURE_HASH: &str = "0123456789abcdef0123456789abcdef01234567";
const PLAY_TORRENT: Action = Action::Play {
    group: 0,
    stream: 1,
};

/// The detail page with the P2P notice already accepted.
fn detail_with_p2p_accepted() -> State {
    let mut state = detail_with_streams();
    update(
        &mut state,
        Action::RestoreSettings(Settings {
            p2p_enabled: true,
            p2p_acknowledged: true,
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
        torrent.connecting_title().is_some(),
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

    // A late answer for another torrent is ignored.
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
        state.torrent.as_ref().unwrap().connecting_title(),
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
        vec![Effect::StopTorrent]
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
    assert_eq!(effects, vec![Effect::StopTorrent]);
    assert_eq!(
        state.notice.as_deref(),
        Some("The torrent could not be played: no peers found")
    );
}

#[test]
fn turning_p2p_off_blocks_torrents_and_stops_a_running_one() {
    let mut state = detail_with_p2p_accepted();
    update(&mut state, PLAY_TORRENT);
    let effects = update(&mut state, Action::SetP2pEnabled(false));
    let [Effect::SaveSettings(settings), Effect::StopTorrent] = effects.as_slice() else {
        panic!("{effects:?}");
    };
    assert!(!settings.p2p_enabled);
    assert!(state.torrent.is_none());

    assert!(update(&mut state, PLAY_TORRENT).is_empty());
    assert_eq!(
        state.notice.as_deref(),
        Some("Torrent streams are turned off in Settings")
    );
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
                Effect::Play(_)
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
    };
    let items = vec![
        item("a", 10, 100, 1),
        item("b", 99, 100, 3),
        item("c", 50, 100, 2),
        item("d", 0, 100, 4),
    ];
    let ids: Vec<_> = continue_watching(&items)
        .iter()
        .map(|i| i.id.as_str())
        .collect();
    assert_eq!(ids, vec!["c", "a"]);
    assert_eq!(items[1].resume_ms("b"), 0, "finished restarts");
    assert_eq!(items[2].resume_ms("other"), 0);
}
