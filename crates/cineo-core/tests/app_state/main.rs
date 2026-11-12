#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers panic on purpose"
)]

use cineo_core::addon::{
    ContentType, Manifest, Meta, MetaPreview, ResourceName, ResourcePath, SourceKind, TransportUrl,
    parse_addon_catalog_response, parse_catalog_response, parse_manifest, parse_meta_response,
    parse_stream_response, parse_subtitles_response,
};
use cineo_core::app::{
    Action, Effect, Language, LibraryItem, Loadable, Lookahead, ManifestOrigin, Notice,
    PlaybackFailure, Problem, SavedStream, Setting, Settings, State, SubtitleGroup, TorrentRequest,
    TorrentStatus, TorrentStorage, WatchedAt, continue_watching, find_subtitle, update,
};

mod addons;
mod library;
mod playback;
mod settings;
mod torrent;

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
const LISTS: &str = "https://lists.example/manifest.json";

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
        origin: ManifestOrigin::Restore,
    }
}

fn load_failed(transport: &str) -> Action {
    Action::ManifestLoaded {
        transport: url(transport),
        result: Err("HTTP 503".into()),
        origin: ManifestOrigin::Restore,
    }
}

fn movie_meta() -> Meta {
    parse_meta_response(&fixture("basic/meta-movie.json"))
        .unwrap()
        .value
}

fn ask_movie(state: &mut State) -> Vec<Effect> {
    update(
        state,
        Action::OpenDetail {
            content_type: ty("movie"),
            id: "tt0000001".into(),
            preview: None,
        },
    )
}

fn open_movie(state: &mut State) -> Vec<Effect> {
    let effects = ask_movie(state);
    let [Effect::FetchMeta { addon, path }] = effects.as_slice() else {
        panic!("{effects:?}");
    };
    update(
        state,
        Action::MetaLoaded {
            addon: addon.clone(),
            path: path.clone(),
            result: Ok(Box::new(movie_meta())),
        },
    )
}

fn retried(action: Action) -> Action {
    let Action::ManifestLoaded {
        transport, result, ..
    } = action
    else {
        panic!("{action:?}");
    };
    Action::ManifestLoaded {
        transport,
        result,
        origin: ManifestOrigin::Retry,
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

fn detail_with_streams() -> State {
    let (mut state, _) = restored();
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
