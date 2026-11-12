#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers panic on purpose"
)]

use cineo_core::addon::{
    ContentType, ResourceName, ResourcePath, TransportUrl, parse_addon_catalog_response,
    parse_catalog_response, parse_manifest, parse_meta_response, parse_stream_response,
};
use cineo_core::app::{
    Action, DownloadLimit, Effect, InterfaceScale, Language, LibraryItem, ManifestOrigin, SeekStep,
    Setting, Settings, StartPage, State, SubtitleColor, SubtitleSize, TorrentPort, UiLanguage,
    update,
};
use cineo_desktop::i18n::Locale;
use cineo_desktop::view::{Page, ViewState, show};
use eframe::egui::accesskit::Role;
use eframe::egui::{Event, Key, Modifiers, MouseWheelUnit, TouchPhase, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

mod addons;
mod browse;
mod detail;
mod library;
mod settings;

fn fixture(rel: &str) -> Vec<u8> {
    let full = format!("{}/../../tests/addons/{rel}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&full).unwrap_or_else(|err| panic!("reading {full}: {err}"))
}

const BASIC: &str = "https://basic.example/manifest.json";

fn board_state() -> State {
    let mut state = State::default();
    let addon = TransportUrl::parse(BASIC).unwrap();
    update(
        &mut state,
        Action::Restore {
            addons: vec![addon.clone()],
            library: Vec::new(),
        },
    );
    let manifest = parse_manifest(&fixture("basic/manifest.json"))
        .unwrap()
        .value;
    let effects = update(
        &mut state,
        Action::ManifestLoaded {
            transport: addon,
            result: Ok(Box::new(manifest)),
            origin: ManifestOrigin::Restore,
        },
    );
    for effect in effects {
        if let Effect::FetchCatalog { addon, path } = effect {
            let result = if path.id == "top" {
                Ok(
                    parse_catalog_response(&fixture("basic/catalog-movie-top.json"))
                        .unwrap()
                        .value
                        .metas,
                )
            } else {
                Err("HTTP status 500".to_owned())
            };
            update(
                &mut state,
                Action::CatalogLoaded {
                    addon,
                    path,
                    result,
                },
            );
        }
    }
    state
}

fn detail_state() -> State {
    detail_with_streams(&fixture("basic/streams-movie.json"))
}

fn detail_with_streams(body: &[u8]) -> State {
    let mut state = board_state();
    let effects = update(
        &mut state,
        Action::OpenDetail {
            content_type: ContentType::new("movie").unwrap(),
            id: "tt0000001".into(),
            preview: None,
        },
    );
    let [Effect::FetchMeta { addon, path }] = effects.as_slice() else {
        panic!("{effects:?}")
    };
    let meta = parse_meta_response(&fixture("basic/meta-movie.json")).unwrap();
    let effects = update(
        &mut state,
        Action::MetaLoaded {
            addon: addon.clone(),
            path: path.clone(),
            result: Ok(Box::new(meta.value)),
        },
    );
    for effect in effects {
        if let Effect::FetchStreams { addon, path } = effect {
            let streams = parse_stream_response(body).unwrap();
            update(
                &mut state,
                Action::StreamsLoaded {
                    addon,
                    path,
                    result: Ok(streams.value),
                },
            );
        }
    }
    state
}

type Ui = (State, ViewState, Vec<Action>);

fn harness(state: State, view: ViewState) -> Harness<'static, Ui> {
    Harness::builder()
        .with_size([1280.0, 900.0])
        .build_ui_state(
            |ui, (state, view, actions): &mut Ui| actions.extend(show(ui, state, view)),
            (state, view, Vec::new()),
        )
}
