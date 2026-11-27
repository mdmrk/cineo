use std::net::SocketAddr;

use cineo_core::app::State;
use cineo_core::remote::{Peer, RemoteKey};
use cineo_net::remote::{format_remote_code, remote_link};
use eframe::egui::{Label, RichText, Ui};

use super::row;
use super::sync::{qr, right};
use crate::i18n::{Text, t};
use crate::theme;
use crate::view::{RemoteListener, RemoteRequest, ViewState};

pub(super) fn remote(ui: &mut Ui, state: &State, view: &mut ViewState) {
    let Some(key) = state.remote.host else {
        row(
            ui,
            &t!("Remote control"),
            &t!(
                "Lets Cineo on your phone pause, seek, change the volume and pick tracks of what plays here. The phone must be on the same network."
            ),
            |ui| {
                right(ui, |ui| {
                    if ui.button(t!("Turn on remote control")).clicked() {
                        view.remote_request = Some(RemoteRequest::TurnOn);
                    }
                });
            },
        );
        return;
    };
    row(
        ui,
        &t!("Remote control is on"),
        &status(&view.remote_listener),
        |ui| {
            right(ui, |ui| {
                if ui.button(t!("Turn off")).clicked() {
                    view.remote_request = Some(RemoteRequest::TurnOff);
                }
            });
        },
    );
    if let RemoteListener::On { ip: Some(ip), port } = view.remote_listener {
        pair(ui, SocketAddr::new(ip, port), key);
    }
    row(
        ui,
        &t!("New code"),
        &t!("Phones paired with the old code stop working until they scan the new one."),
        |ui| {
            right(ui, |ui| {
                if ui.button(t!("New code")).clicked() {
                    view.remote_request = Some(RemoteRequest::NewCode);
                }
            });
        },
    );
}

fn status(listener: &RemoteListener) -> Text {
    match listener {
        RemoteListener::Off => t!("Starting…"),
        RemoteListener::On { ip: Some(_), port } => t!(
            "Listening on port {port}. If your phone cannot connect, allow Cineo through this computer's firewall.",
            port = port
        ),
        RemoteListener::On { ip: None, .. } => t!(
            "This computer has no address on a local network. Connect it to the same network as your phone."
        ),
        RemoteListener::Failed(detail) => {
            t!(
                "The remote control could not start: {detail}",
                detail = detail.as_str()
            )
        }
    }
}

fn pair(ui: &mut Ui, address: SocketAddr, key: RemoteKey) {
    let link = remote_link(&Peer { address, key });
    row(
        ui,
        &t!("Pair your phone"),
        &t!(
            "In Cineo on your phone, open Settings → Remote control and scan this QR code, or enter the address and code. Anyone with the code on this network can control playback."
        ),
        |ui| {
            ui.vertical(|ui| {
                qr(ui, &link, &t!("Remote control QR code"));
                for (label, value) in [
                    (t!("Address"), address.to_string()),
                    (t!("Code"), format_remote_code(&key)),
                ] {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(label).color(theme::TEXT_DIM));
                        ui.add(
                            Label::new(RichText::new(value).monospace().color(theme::TEXT_BRIGHT))
                                .wrap(),
                        );
                    });
                }
            });
        },
    );
}
