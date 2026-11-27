use cineo_core::app::Route;
use cineo_core::sync::{DEFAULT_SERVER, DeviceId, Pairing, SyncKey};
use cineo_desktop::settings::Section;
use cineo_desktop::view::{SyncRequest, follow_link};
use cineo_net::sync::format_code;
use url::Url;

use super::*;

const KEY: SyncKey = SyncKey::from_bytes([7; 16]);

fn data_page() -> ViewState {
    ViewState {
        page: Page::Settings,
        settings_jump: Some(Section::Data),
        ..ViewState::default()
    }
}

fn open(state: State) -> Harness<'static, Ui> {
    let mut harness = harness(state, data_page());
    harness.run_steps(30);
    harness
}

fn synced() -> State {
    let mut state = State::default();
    update(
        &mut state,
        Action::EnableSync {
            pairing: Pairing {
                key: KEY,
                server: Url::parse(DEFAULT_SERVER).unwrap(),
            },
            device: DeviceId(1),
        },
    );
    state
}

fn type_into(harness: &mut Harness<'_, Ui>, label: &str, text: &str) {
    harness
        .get_by_role_and_label(Role::TextInput, label)
        .focus();
    harness.run();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.key_press(Key::Backspace);
    harness
        .get_by_role_and_label(Role::TextInput, label)
        .type_text(text);
    harness.run();
}

fn click(harness: &mut Harness<'_, Ui>, label: &str) {
    harness.get_by_role_and_label(Role::Button, label).click();
    harness.run_steps(4);
}

#[test]
fn pairing_rejects_a_mistyped_code_and_accepts_a_good_one() {
    let mut harness = open(State::default());
    let code = format_code(&KEY);
    let typo = code.replacen(&code[..1], if &code[..1] == "A" { "B" } else { "A" }, 1);

    type_into(&mut harness, "Sync code", &typo);
    click(&mut harness, "Pair");
    harness.get_by_label("This code has a typo.");
    assert_eq!(harness.state().1.sync_request, None);

    type_into(&mut harness, "Sync code", "ABCD");
    click(&mut harness, "Pair");
    harness.get_by_label("A sync code has 32 letters and digits.");

    type_into(&mut harness, "Sync code", &code.to_lowercase());
    click(&mut harness, "Pair");
    assert!(
        harness
            .query_by_label("A sync code has 32 letters and digits.")
            .is_none()
    );
    assert_eq!(
        harness.state().1.sync_request,
        Some(SyncRequest::Pair {
            key: KEY,
            server: Url::parse(DEFAULT_SERVER).unwrap(),
        })
    );
    assert!(harness.state().2.is_empty());
}

#[test]
fn turning_on_needs_an_https_server() {
    let mut harness = open(State::default());
    type_into(&mut harness, "Sync server", "http://sync.example/");
    click(&mut harness, "Turn on");
    harness.get_by_label("The server address must start with https://.");
    assert_eq!(harness.state().1.sync_request, None);

    type_into(&mut harness, "Sync server", "https://sync.example/");
    click(&mut harness, "Turn on");
    assert_eq!(
        harness.state().1.sync_request,
        Some(SyncRequest::TurnOn {
            server: Url::parse("https://sync.example/").unwrap(),
        })
    );
}

#[test]
fn turning_sync_off_asks_first_and_can_delete_the_server_data() {
    let mut harness = open(synced());
    harness.get_by_label(&format_code(&KEY));
    click(&mut harness, "Turn off…");
    harness.get_by_label("Turn off sync?");
    click(&mut harness, "Cancel");
    assert!(harness.state().2.is_empty());

    click(&mut harness, "Turn off…");
    click(&mut harness, "Turn off");
    click(&mut harness, "Turn off and delete…");
    harness.get_by_label("Turn off sync and delete the server data?");
    click(&mut harness, "Delete");
    assert_eq!(
        harness.state().2,
        vec![
            Action::LeaveSync {
                delete_server_data: false
            },
            Action::LeaveSync {
                delete_server_data: true
            },
        ]
    );
}

#[test]
fn changing_the_sync_key_asks_first() {
    let mut harness = open(synced());
    click(&mut harness, "Change…");
    harness.get_by_label("Change the sync key?");
    assert_eq!(harness.state().1.sync_request, None);
    click(&mut harness, "Change key");
    assert_eq!(harness.state().1.sync_request, Some(SyncRequest::NewKey));
    assert!(harness.state().2.is_empty());
}

#[test]
fn a_sync_link_fills_the_code_and_opens_data_settings() {
    let state = State::default();
    let mut view = ViewState {
        sync_error: Some(cineo_desktop::view::SyncInputError::Server),
        ..ViewState::default()
    };
    let server = Url::parse("https://sync.example/").unwrap();
    let route = Route::Sync {
        code: format_code(&KEY),
        server: Some(server.clone()),
    };
    let actions = follow_link(route.clone(), &state, &mut view);
    assert_eq!(view.page, Page::Settings);
    assert_eq!(view.settings_jump, Some(Section::Data));
    assert_eq!(view.sync_code, format_code(&KEY));
    assert_eq!(view.sync_server, server.as_str());
    assert_eq!(view.sync_error, None);
    assert_eq!(view.sync_request, None);
    assert!(actions.contains(&Action::OpenLink(route)));
}
