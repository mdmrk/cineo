use super::*;

#[test]
fn the_interface_follows_the_language_setting_and_the_system() {
    let mut state = board_state();
    update(
        &mut state,
        Action::ChangeSetting(Setting::UiLanguage(UiLanguage::Spanish)),
    );
    let chosen = harness(state, ViewState::default());
    chosen.get_by_label("Biblioteca");
    chosen.get_by_label("Top Movies · Películas");
    chosen.get_by_label("Ajustes");

    let system = ViewState {
        system_locale: Locale::Spanish,
        ..ViewState::default()
    };
    let followed = harness(board_state(), system);
    followed.get_by_label("Biblioteca");

    let mut english = board_state();
    update(
        &mut english,
        Action::ChangeSetting(Setting::UiLanguage(UiLanguage::English)),
    );
    let system = ViewState {
        system_locale: Locale::Spanish,
        ..ViewState::default()
    };
    let overridden = harness(english, system);
    overridden.get_by_label("Library");

    let latin_america = ViewState {
        system_locale: Locale::LatinAmericanSpanish,
        ..ViewState::default()
    };
    let regional = harness(board_state(), latin_america);
    regional.get_by_label("Configuración");
}

#[test]
fn spanish_notices_wrap_the_technical_detail() {
    let mut state = State::default();
    update(
        &mut state,
        Action::ChangeSetting(Setting::UiLanguage(UiLanguage::Spanish)),
    );
    update(&mut state, Action::InstallAddon("not a url".into()));
    let view = ViewState {
        page: Page::Addons,
        ..ViewState::default()
    };
    let harness = harness(state, view);
    harness.get_by_label_contains("URL de add-on no válida: ");
}

#[test]
fn the_interface_language_is_picked_in_settings() {
    let view = ViewState {
        system_locale: Locale::Spanish,
        ..settings_page()
    };
    let mut harness = harness(State::default(), view);
    settle(&mut harness);
    harness.get_by_label("Idioma").click();
    settle(&mut harness);
    harness.get_by_label("Sistema (Español de España)");
    harness.get_by_label("Español de Latinoamérica");
    harness.get_by_label("English").click();
    settle(&mut harness);
    assert_eq!(
        harness.state().2,
        vec![Action::ChangeSetting(Setting::UiLanguage(
            UiLanguage::English
        ))]
    );
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
fn settings_take_a_torrent_port_and_ignore_one_out_of_range() {
    let view = ViewState {
        page: Page::Settings,
        ..ViewState::default()
    };
    let mut harness = harness(State::default(), view);
    open_section(&mut harness, "Torrents");
    for typed in ["80", "51000"] {
        harness
            .get_by_role_and_label(Role::TextInput, "Listening port")
            .focus();
        harness.run();
        harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
        harness.key_press(Key::Backspace);
        harness
            .get_by_role_and_label(Role::TextInput, "Listening port")
            .type_text(typed);
        harness.run();
        harness.key_press(Key::Enter);
        harness.run();
    }
    assert_eq!(
        harness.state().2,
        vec![Action::ChangeSetting(Setting::TorrentPort(
            TorrentPort::new(51000)
        ))]
    );
}

#[test]
fn settings_pick_a_subtitle_language() {
    let view = ViewState {
        page: Page::Settings,
        ..ViewState::default()
    };
    let mut harness = harness(State::default(), view);
    open_section(&mut harness, "Languages");
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

fn open_section(harness: &mut Harness<'_, Ui>, name: &str) {
    harness.get_by_role_and_label(Role::Button, name).click();
    harness.run_steps(30);
}

fn settle(harness: &mut Harness<'_, Ui>) {
    harness.run_steps(4);
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
    settle(&mut harness);
    harness.get_by_label("Reset all settings?");
    harness.get_by_label("Cancel").click();
    settle(&mut harness);
    assert!(harness.query_by_label("Reset all settings?").is_none());
    assert!(harness.state().2.is_empty());

    harness.get_by_label("Reset…").click();
    settle(&mut harness);
    harness.get_by_label("Reset").click();
    settle(&mut harness);
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
    let about = |h: &Harness<'_, Ui>| {
        h.get_all_by_label(concat!("Cineo v", env!("CARGO_PKG_VERSION")))
            .find(|node| node.rect().left() > 200.0)
            .expect("About, not the sidebar")
            .rect()
            .top()
    };
    assert!(about(&harness) > 500.0, "About starts below the window");
    harness.get_by_role_and_label(Role::Button, "About").click();
    harness.run_steps(30);
    settle(&mut harness);
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
    harness.get_by_label("30 s").click();
    harness.run();
    harness.get_by_label("Second audio language").scroll_to_me();
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
    settle(&mut harness);
    harness.get_by_label("Clear watch history?");
    harness.get_by_label("Clear").click();
    settle(&mut harness);
    assert_eq!(harness.state().2, vec![Action::ClearLibrary]);
}

#[test]
fn update_checks_can_be_turned_off_in_about() {
    let mut harness = harness(State::default(), settings_page());
    open_section(&mut harness, "About");
    harness.get_by_label("Check for updates").click();
    settle(&mut harness);
    assert_eq!(
        harness.state().2,
        vec![Action::ChangeSetting(Setting::CheckForUpdates(false))]
    );
}
