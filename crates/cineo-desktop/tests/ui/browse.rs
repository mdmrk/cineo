use super::*;

#[test]
fn board_shows_rows_with_independent_failures_and_a_card_opens_the_detail() {
    let mut harness = harness(board_state(), ViewState::default());
    harness.get_by_label("Top Movies Movies");
    harness.get_by_label("Multi-genre Series");
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
fn data_left_by_an_earlier_version_is_pointed_out_until_dismissed() {
    let view = ViewState {
        left_behind: Some(LeftBehind {
            earlier: "/home/u/.local/share/cineo".into(),
            current: "/home/u/.local/state/cineo".into(),
        }),
        ..ViewState::default()
    };
    let mut harness = harness(State::default(), view);
    harness.get_by_label_contains("/home/u/.local/share/cineo");
    harness.get_by_label("Dismiss").click();
    harness.run();
    assert_eq!(harness.state().1.left_behind, None);
    assert!(harness.state().2.is_empty());
    assert!(
        harness
            .query_by_label_contains("/home/u/.local/share/cineo")
            .is_none()
    );
}

#[test]
fn the_clear_button_empties_the_search_and_searches_at_once() {
    let view = ViewState {
        page: Page::Search,
        ..ViewState::default()
    };
    let mut harness = harness(board_state(), view);
    assert!(
        harness.query_by_label("Clear").is_none(),
        "nothing to clear"
    );
    harness.state_mut().1.search_input = "example".into();
    harness.run();
    harness.get_by_label("Clear").click();
    harness.run();
    assert!(harness.state().1.search_input.is_empty());
    assert_eq!(harness.state().2, vec![Action::Search(String::new())]);
    assert!(harness.query_by_label("Clear").is_none());
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
fn see_all_opens_the_row_in_discover() {
    let mut harness = harness(board_state(), ViewState::default());
    harness.get_all_by_label("See all").next().unwrap().click();
    harness.run();
    assert_eq!(harness.state().1.page, Page::Discover);
    let actions = &harness.state().2;
    assert!(
        matches!(actions.as_slice(), [Action::OpenDiscover { path, .. }] if path.id == "top"),
        "{actions:?}"
    );
}

#[test]
fn a_newer_release_shows_in_the_sidebar_and_opens_its_page() {
    let mut state = State::default();
    update(&mut state, Action::RestoreSettings(Settings::default()));
    let mut before = harness(state.clone(), ViewState::default());
    before.run();
    assert!(before.query_by_label_contains("available").is_none());

    update(
        &mut state,
        Action::LatestReleaseLoaded(Ok(semver::Version::new(999, 0, 0))),
    );
    let mut harness = harness(state, ViewState::default());
    harness.run();
    harness.get_by_label("v999.0.0 available").click();
    harness.step();
    let opened: Vec<_> = harness
        .output()
        .platform_output
        .commands
        .iter()
        .filter_map(|command| match command {
            eframe::egui::OutputCommand::OpenUrl(open) => Some(open.url.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        opened,
        ["https://github.com/mdmrk/cineo/releases/tag/v999.0.0"]
    );
}
