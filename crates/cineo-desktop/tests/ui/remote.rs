use std::net::{IpAddr, Ipv4Addr};

use cineo_core::app::Route;
use cineo_core::remote::RemoteKey;
use cineo_desktop::settings::Section;
use cineo_desktop::view::{RemoteListener, RemoteRequest, follow_link};
use cineo_net::remote::format_remote_code;

use super::*;

const KEY: RemoteKey = RemoteKey::from_bytes([5; 16]);

fn open(state: State, listener: RemoteListener) -> Harness<'static, Ui> {
    let view = ViewState {
        page: Page::Settings,
        settings_jump: Some(Section::Remote),
        remote_listener: listener,
        ..ViewState::default()
    };
    let mut harness = harness(state, view);
    harness.run_steps(30);
    harness
}

fn hosting() -> State {
    let mut state = State::default();
    update(&mut state, Action::SetRemoteHost(Some(KEY)));
    state
}

fn click(harness: &mut Harness<'_, Ui>, label: &str) {
    harness.get_by_role_and_label(Role::Button, label).click();
    harness.run_steps(4);
}

#[test]
fn the_remote_control_is_off_until_turned_on() {
    let mut harness = open(State::default(), RemoteListener::Off);
    assert!(
        harness
            .query_by_role_and_label(Role::Image, "Remote control QR code")
            .is_none()
    );
    click(&mut harness, "Turn on remote control");
    assert_eq!(
        harness.state().1.remote_request,
        Some(RemoteRequest::TurnOn)
    );
}

#[test]
fn a_listening_desktop_shows_its_address_and_code() {
    let listener = RemoteListener::On {
        ip: Some(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20))),
        port: 18_765,
    };
    let mut harness = open(hosting(), listener);
    harness.get_by_label("192.168.1.20:18765");
    harness.get_by_label(&format_remote_code(&KEY));
    harness.get_by_role_and_label(Role::Image, "Remote control QR code");
    click(&mut harness, "New code");
    assert_eq!(
        harness.state().1.remote_request,
        Some(RemoteRequest::NewCode)
    );
    harness.state_mut().1.remote_request = None;
    click(&mut harness, "Turn off");
    assert_eq!(
        harness.state().1.remote_request,
        Some(RemoteRequest::TurnOff)
    );
}

#[test]
fn without_a_local_address_there_is_nothing_to_scan() {
    let harness = open(hosting(), RemoteListener::On { ip: None, port: 1 });
    harness.get_by_label(
        "This computer has no address on a local network. Connect it to the same network as your phone.",
    );
    assert!(
        harness
            .query_by_role_and_label(Role::Image, "Remote control QR code")
            .is_none()
    );
}

#[test]
fn a_failed_listener_says_why() {
    let harness = open(hosting(), RemoteListener::Failed("port in use".into()));
    harness.get_by_label("The remote control could not start: port in use");
}

#[test]
fn a_remote_link_opens_the_remote_settings() {
    let mut view = ViewState::default();
    let route = Route::Remote {
        code: format_remote_code(&KEY),
        address: "192.168.1.20:18765".into(),
        name: "Salon PC".into(),
    };
    follow_link(route, &State::default(), &mut view);
    assert_eq!(view.page, Page::Settings);
    assert_eq!(view.settings_jump, Some(Section::Remote));
}
