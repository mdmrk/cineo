//! UI smoke tests: the real view over core states built from fixtures,
//! rendered headlessly. They check what is shown and which actions clicks
//! produce; behavior itself is tested in `cineo-core`.

// Test helpers panic on purpose: a panic is a failed assertion.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use cineo_core::addon::{
    ContentType, TransportUrl, parse_catalog_response, parse_manifest, parse_meta_response,
    parse_stream_response,
};
use cineo_core::app::{
    Action, DownloadLimit, Effect, InterfaceScale, Language, SeekStep, Setting, Settings,
    StartPage, State, SubtitleColor, SubtitleSize, TorrentStatus, update,
};
use cineo_desktop::view::{Page, ViewState, show};
use eframe::egui::{Event, Key, Modifiers, MouseWheelUnit, TouchPhase, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

fn fixture(rel: &str) -> Vec<u8> {
    let full = format!(
        "{}/../../tests/fixtures/addons/{rel}",
        env!("CARGO_MANIFEST_DIR")
    );
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
            install: false,
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

#[test]
fn empty_board_explains_how_to_add_addons() {
    let harness = harness(State::default(), ViewState::default());
    harness.get_by_label_contains("No addons installed");
}

#[test]
fn board_shows_rows_with_independent_failures_and_a_card_opens_the_detail() {
    let mut harness = harness(board_state(), ViewState::default());
    harness.get_by_label("TOP MOVIES MOVIES");
    harness.get_by_label("MULTI-GENRE SERIES");
    harness.get_by_label("HTTP status 500");
    harness.get_by_label("Second Example Film");
    harness.get_by_label("First Example Film").click();
    harness.run();
    let actions = &harness.state().2;
    assert!(
        matches!(
            actions.as_slice(),
            [Action::OpenDetail { id, preview: Some(_), .. }] if id == "tt0000001"
        ),
        "{actions:?}"
    );
}

#[test]
fn detail_lists_streams_and_only_playable_ones_can_be_played() {
    let mut harness = harness(detail_state(), ViewState::default());
    harness.get_by_label("First Example Film");
    harness.get_by_label("Example HTTP stream");
    harness.get_by_label("Basic Fixture");
    let play: Vec<_> = harness.get_all_by_label("Play").collect();
    assert_eq!(play.len(), 6, "one button per stream");
    let enabled: Vec<bool> = play
        .iter()
        .map(|b| !b.accesskit_node().is_disabled())
        .collect();
    assert_eq!(enabled, vec![true, true, false, false, false, false]);

    play[0].click();
    harness.run();
    assert_eq!(
        harness.state().2,
        vec![Action::Play {
            group: 0,
            stream: 0
        }]
    );
}

#[test]
fn addons_page_installs_from_the_typed_url_and_lists_installed_addons() {
    let view = ViewState {
        page: Page::Addons,
        ..ViewState::default()
    };
    let mut harness = harness(board_state(), view);
    harness.get_by_label_contains("Basic Fixture 1.2.0");
    harness.get_by_label("basic.example");
    harness
        .get_by_role(eframe::egui::accesskit::Role::TextInput)
        .focus();
    harness.run();
    harness
        .get_by_role(eframe::egui::accesskit::Role::TextInput)
        .type_text("https://new.example/manifest.json");
    harness.run();
    harness.get_by_label("Install").click();
    harness.run();
    assert_eq!(
        harness.state().2,
        vec![Action::InstallAddon(
            "https://new.example/manifest.json".into()
        )]
    );
}

#[test]
fn addons_that_failed_to_load_can_be_retried_or_removed() {
    const DOWN: &str = "https://down.example/manifest.json";
    let mut state = State::default();
    let down = TransportUrl::parse(DOWN).unwrap();
    update(
        &mut state,
        Action::Restore {
            addons: vec![down.clone()],
            library: Vec::new(),
        },
    );
    update(
        &mut state,
        Action::ManifestLoaded {
            transport: down.clone(),
            result: Err("HTTP 503".into()),
            install: false,
        },
    );
    let view = ViewState {
        page: Page::Addons,
        ..ViewState::default()
    };
    let mut harness = harness(state, view);
    harness.get_by_label("down.example");
    harness.get_by_label("Retry").click();
    harness.run();
    harness.get_by_label("Remove").click();
    harness.run();
    assert_eq!(
        harness.state().2,
        vec![Action::InstallAddon(DOWN.into()), Action::RemoveAddon(down)]
    );
}

#[test]
fn notices_are_shown_and_can_be_dismissed() {
    let mut state = State::default();
    update(
        &mut state,
        Action::PlaybackFailed("mpv was not found".into()),
    );
    let mut harness = harness(state, ViewState::default());
    harness.get_by_label("mpv was not found");
    harness.get_by_label("Dismiss").click();
    harness.run();
    assert_eq!(harness.state().2, vec![Action::DismissNotice]);
}

#[test]
fn turning_p2p_off_hides_torrent_streams() {
    let mut state = detail_state();
    update(
        &mut state,
        Action::ChangeSetting(Setting::P2pEnabled(false)),
    );
    let harness = harness(state, ViewState::default());
    assert_eq!(harness.get_all_by_label("Play").count(), 5);
    harness.get_by_label("1 torrent stream hidden: peer-to-peer is off in Settings");
}

#[test]
fn the_first_torrent_play_shows_the_p2p_notice() {
    let mut state = detail_state();
    update(
        &mut state,
        Action::Play {
            group: 0,
            stream: 1,
        },
    );
    assert!(state.p2p_prompt.is_some());

    let mut harness = harness(state.clone(), ViewState::default());
    harness.get_by_label("Peer-to-peer streaming");
    harness.get_by_label_contains("uploads the parts it has already downloaded");
    harness.get_by_label("Accept and play").click();
    harness.run();
    assert_eq!(harness.state().2, vec![Action::AcceptP2p]);

    let mut harness = self::harness(state, ViewState::default());
    harness.get_by_label("Cancel").click();
    harness.run();
    assert_eq!(harness.state().2, vec![Action::DeclineP2p]);
}

#[test]
fn a_streaming_torrent_shows_no_status_text() {
    let mut state = detail_state();
    update(&mut state, Action::AcceptP2p);
    update(
        &mut state,
        Action::Play {
            group: 0,
            stream: 1,
        },
    );
    update(
        &mut state,
        Action::TorrentStatus {
            info_hash: "0123456789abcdef0123456789abcdef01234567".into(),
            status: TorrentStatus::Streaming {
                peers: 3,
                download_bytes_per_sec: 1024 * 1024,
                downloaded: 1024 * 1024 * 1024,
                size: 2 * 1024 * 1024 * 1024,
            },
        },
    );
    let harness = harness(state, ViewState::default());
    assert!(harness.query_by_label_contains("Torrent:").is_none());
    assert!(harness.query_by_label_contains("peers").is_none());
}

#[test]
fn settings_switch_p2p_off() {
    let view = ViewState {
        page: Page::Settings,
        ..ViewState::default()
    };
    let mut harness = harness(State::default(), view);
    open_section(&mut harness, "Torrents");
    harness
        .get_by_label("Show and play torrent streams")
        .click();
    harness.run();
    assert_eq!(
        harness.state().2,
        vec![Action::ChangeSetting(Setting::P2pEnabled(false))]
    );
}

#[test]
fn settings_pick_a_subtitle_language() {
    let view = ViewState {
        page: Page::Settings,
        ..ViewState::default()
    };
    let mut harness = harness(State::default(), view);
    harness.get_by_label("Subtitle language").click();
    harness.run();
    harness.get_by_label("English").click();
    harness.run();
    assert_eq!(
        harness.state().2,
        vec![Action::ChangeSetting(Setting::SubtitleLanguage(
            Language::from_code("eng")
        ))]
    );
}

#[test]
fn search_runs_once_typing_pauses() {
    let view = ViewState {
        page: Page::Search,
        ..ViewState::default()
    };
    let mut harness = Harness::builder()
        .with_size([1280.0, 900.0])
        .with_step_dt(0.1)
        .build_ui_state(
            |ui, (state, view, actions): &mut Ui| actions.extend(show(ui, state, view)),
            (board_state(), view, Vec::new()),
        );
    harness
        .get_by_role(eframe::egui::accesskit::Role::TextInput)
        .focus();
    harness.step();
    harness
        .get_by_role(eframe::egui::accesskit::Role::TextInput)
        .type_text("example");
    harness.step();
    assert!(harness.state().2.is_empty(), "no search while typing");
    for _ in 0..10 {
        harness.step();
    }
    assert_eq!(harness.state().2, vec![Action::Search("example".into())]);
}

#[test]
fn ctrl_f_opens_search_from_anywhere() {
    let mut harness = harness(board_state(), ViewState::default());
    harness.key_press_modifiers(Modifiers::COMMAND, Key::F);
    harness.run();
    assert_eq!(harness.state().1.page, Page::Search);
    assert!(harness.state().2.is_empty(), "{:?}", harness.state().2);
}

#[test]
fn escape_leaves_the_detail_page() {
    let mut harness = harness(detail_state(), ViewState::default());
    harness.key_press(Key::Escape);
    harness.run();
    assert_eq!(harness.state().2, vec![Action::CloseDetail]);
}

#[test]
fn sidebar_switches_pages_and_leaves_the_detail_page() {
    let mut harness = harness(detail_state(), ViewState::default());
    harness.get_by_label("Library").click();
    harness.run();
    assert_eq!(harness.state().1.page, Page::Library);
    assert_eq!(harness.state().2, vec![Action::CloseDetail]);
}

#[test]
fn discover_lists_catalogs_as_choices() {
    let mut state = board_state();
    let target = cineo_core::app::board_targets(&state.addons)
        .into_iter()
        .next()
        .unwrap();
    update(
        &mut state,
        Action::OpenDiscover {
            addon: target.addon,
            path: target.path,
        },
    );
    let view = ViewState {
        page: Page::Discover,
        ..ViewState::default()
    };
    let mut harness = harness(state, view);
    harness.get_by_label("Multi-genre Series").click();
    harness.run_steps(2);
    let actions = &harness.state().2;
    assert!(
        matches!(actions.as_slice(), [Action::OpenDiscover { path, .. }] if path.id == "multi"),
        "{actions:?}"
    );
}

#[test]
fn narrow_windows_keep_an_icon_sidebar() {
    let mut harness = Harness::builder().with_size([820.0, 600.0]).build_ui_state(
        |ui, (state, view, actions): &mut Ui| actions.extend(show(ui, state, view)),
        (board_state(), ViewState::default(), Vec::new()),
    );
    harness.get_by_label("Home");
    harness.get_by_label("Addons").click();
    harness.run();
    assert_eq!(harness.state().1.page, Page::Addons);
}

#[test]
fn see_all_opens_the_row_in_discover() {
    let mut harness = harness(board_state(), ViewState::default());
    harness.get_all_by_label("SEE ALL").next().unwrap().click();
    harness.run();
    assert_eq!(harness.state().1.page, Page::Discover);
    let actions = &harness.state().2;
    assert!(
        matches!(actions.as_slice(), [Action::OpenDiscover { path, .. }] if path.id == "top"),
        "{actions:?}"
    );
}

#[test]
fn home_has_no_spotlight_banner_even_with_backdrops() {
    let mut state = board_state();
    for row in &mut state.board {
        if let cineo_core::app::Loadable::Ready(items) = &mut row.items {
            for item in items {
                item.background = Some("https://img.example/backdrop.jpg".parse().unwrap());
            }
        }
    }
    let harness = harness(state, ViewState::default());
    assert!(harness.query_by_label_contains("Spotlight").is_none());
    assert!(harness.query_by_label("More info").is_none());
    harness.get_by_label("First Example Film");
}

#[test]
fn a_wheel_notch_scrolls_about_as_far_as_in_a_browser() {
    let harness = harness(State::default(), ViewState::default());
    let speed = harness.ctx.options(|o| o.input_options.line_scroll_speed);
    assert!(speed >= 100.0, "{speed} points per wheel notch");
}

#[test]
fn torrent_streams_carry_no_kind_tag() {
    let harness = harness(detail_state(), ViewState::default());
    assert!(harness.query_by_label("TORRENT").is_none());
    assert!(harness.query_all_by_label("HTTP").count() > 0);
}

#[test]
fn long_stream_lists_lay_out_only_visible_cards_and_still_scroll_to_the_end() {
    let streams: Vec<String> = (0..300)
        .map(|i| {
            format!(r#"{{"name":"1080p","description":"Release {i}","url":"https://media.example/{i}.mp4"}}"#)
        })
        .collect();
    let body = format!(r#"{{"streams":[{}]}}"#, streams.join(","));
    let mut harness = harness(detail_with_streams(body.as_bytes()), ViewState::default());
    harness.run();
    assert!(harness.query_all_by_label("Play").count() < 50);
    assert!(harness.query_by_label("Release 299").is_none());

    harness.hover_at(pos2(640.0, 450.0));
    harness.event(Event::MouseWheel {
        unit: MouseWheelUnit::Point,
        delta: vec2(0.0, -1.0e6),
        phase: TouchPhase::Move,
        modifiers: Modifiers::NONE,
    });
    harness.run_steps(60);
    harness.run();
    harness.get_by_label("Release 299");
}

fn open_section(harness: &mut Harness<'_, Ui>, name: &str) {
    harness.get_by_label(name).click();
    harness.run_steps(30);
    harness.run();
}

fn settings_page() -> ViewState {
    ViewState {
        page: Page::Settings,
        ..ViewState::default()
    }
}

#[test]
fn resetting_settings_asks_first() {
    let mut harness = harness(State::default(), settings_page());
    open_section(&mut harness, "Data");
    harness.get_by_label("Reset…").click();
    harness.run();
    harness.get_by_label("Reset all settings?");
    harness.get_by_label("Cancel").click();
    harness.run();
    assert!(harness.query_by_label("Reset all settings?").is_none());
    assert!(harness.state().2.is_empty());

    harness.get_by_label("Reset…").click();
    harness.run();
    harness.get_by_label("Reset").click();
    harness.run();
    assert_eq!(harness.state().2, vec![Action::ResetSettings]);
}

#[test]
fn the_settings_index_jumps_to_a_section() {
    let mut harness = Harness::builder()
        .with_size([1280.0, 500.0])
        .build_ui_state(
            |ui, (state, view, actions): &mut Ui| actions.extend(show(ui, state, view)),
            (State::default(), settings_page(), Vec::new()),
        );
    let about = |h: &Harness<'_, Ui>| h.get_by_label_contains("Made by people").rect().top();
    assert!(about(&harness) > 500.0, "About starts below the window");
    harness.get_by_label("About").click();
    harness.run_steps(30);
    harness.run();
    let top = about(&harness);
    assert!(top < 500.0, "About is scrolled into view: {top}");
}

#[test]
fn player_and_language_settings_change_from_the_page() {
    let mut harness = harness(State::default(), settings_page());
    harness.get_by_label("Hardware decoding").click();
    harness.run();
    harness.get_by_label("Seek step").click();
    harness.run();
    harness.get_by_label("30 seconds").click();
    harness.run();
    harness.get_by_label("Second audio language").click();
    harness.run();
    harness.get_by_label("Japanese").scroll_to_me();
    harness.run();
    harness.get_by_label("Japanese").click();
    harness.run();
    assert_eq!(
        harness.state().2,
        vec![
            Action::ChangeSetting(Setting::HardwareDecoding(false)),
            Action::ChangeSetting(Setting::SeekStep(SeekStep::S30)),
            Action::ChangeSetting(Setting::SecondaryAudioLanguage(Language::from_code("jpn"))),
        ]
    );
}

#[test]
fn subtitle_style_changes_from_the_page() {
    let mut harness = harness(State::default(), settings_page());
    open_section(&mut harness, "Subtitles");
    harness.get_by_label("Size").focus();
    harness.run();
    harness.key_press(Key::ArrowRight);
    harness.run();
    harness.get_by_label("Text color").click();
    harness.run();
    harness.get_by_label("Yellow").click();
    harness.run();
    harness
        .get_by_label("Keep the look of styled subtitles")
        .click();
    harness.run();
    assert_eq!(
        harness.state().2,
        vec![
            Action::ChangeSetting(Setting::SubtitleSize(SubtitleSize::new(105))),
            Action::ChangeSetting(Setting::SubtitleColor(SubtitleColor::Yellow)),
            Action::ChangeSetting(Setting::KeepSubtitleStyles(false)),
        ]
    );
}

#[test]
fn audio_and_torrent_settings_change_from_the_page() {
    let mut harness = harness(State::default(), settings_page());
    open_section(&mut harness, "Audio");
    harness.get_by_label("Passthrough").click();
    harness.run();
    open_section(&mut harness, "Torrents");
    harness.get_by_label("Download limit").click();
    harness.run();
    harness.get_by_label("5 MB/s").click();
    harness.run();
    harness
        .get_by_label("Allow local network addresses")
        .click();
    harness.run();
    assert_eq!(
        harness.state().2,
        vec![
            Action::ChangeSetting(Setting::AudioPassthrough(true)),
            Action::ChangeSetting(Setting::DownloadLimit(DownloadLimit::M5)),
            Action::ChangeSetting(Setting::AllowPrivateNetwork(true)),
        ]
    );
}

#[test]
fn the_interface_scale_sets_the_zoom_and_keys_do_not_change_it() {
    let mut state = State::default();
    update(
        &mut state,
        Action::RestoreSettings(Settings {
            interface_scale: InterfaceScale::S125,
            ..Settings::default()
        }),
    );
    let mut harness = harness(state, ViewState::default());
    harness.run();
    assert!((harness.ctx.zoom_factor() - 1.25).abs() < 1e-6);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Equals);
    harness.run();
    assert!((harness.ctx.zoom_factor() - 1.25).abs() < 1e-6);
}

#[test]
fn the_start_page_setting_picks_the_first_page() {
    assert_eq!(Page::from(StartPage::Home), Page::Board);
    assert_eq!(Page::from(StartPage::Library), Page::Library);
    assert_eq!(Page::from(StartPage::Discover), Page::Discover);
}

#[test]
fn clearing_watch_history_asks_first() {
    let mut harness = harness(State::default(), settings_page());
    open_section(&mut harness, "Data");
    harness.get_by_label("Clear…").click();
    harness.run();
    harness.get_by_label("Clear watch history?");
    harness.get_by_label("Clear").click();
    harness.run();
    assert_eq!(harness.state().2, vec![Action::ClearLibrary]);
}
