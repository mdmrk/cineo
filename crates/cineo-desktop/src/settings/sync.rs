use cineo_core::app::{Action, State, SyncFailure, SyncProblem, SyncState};
use cineo_core::sync::DEFAULT_SERVER;
use cineo_net::sync::{CodeError, format_code, pairing_link, parse_code, server_allowed};
use eframe::egui::{
    self, Align, Button, Color32, CornerRadius, Label, Layout, RichText, Role, Sense, TextEdit, Ui,
    WidgetInfo, vec2,
};
use qrcode::{Color, QrCode};
use url::Url;

use super::{Confirm, row};
use crate::i18n::{Text, t};
use crate::theme;
use crate::view::{SyncInputError, SyncRequest, ViewState};

const QR_MODULE: f32 = 4.0;
const CODE_HINT: &str = "XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX";
const QR_QUIET: usize = 4;

pub(super) fn sync(ui: &mut Ui, state: &State, view: &mut ViewState, out: &mut Vec<Action>) {
    match &state.sync {
        None => off(ui, view),
        Some(sync) => on(ui, state.now_ms, sync, view, out),
    }
}

fn server(view: &mut ViewState) -> Option<Url> {
    let typed = view.sync_server.trim();
    let text = if typed.is_empty() {
        DEFAULT_SERVER
    } else {
        typed
    };
    let server = Url::parse(text).ok().filter(server_allowed);
    if server.is_none() {
        view.sync_error = Some(SyncInputError::Server);
    }
    server
}

fn input_error(error: SyncInputError) -> Text {
    match error {
        SyncInputError::Code(CodeError::Length) => t!("A sync code has 32 letters and digits."),
        SyncInputError::Code(CodeError::Typo) => t!("This code has a typo."),
        SyncInputError::Code(_) => t!("This is not a sync code."),
        SyncInputError::Server => t!("The server address must start with https://."),
        SyncInputError::Random => t!("Cineo could not make a sync key."),
    }
}

fn field(ui: &mut Ui, text: &mut String, hint: &str, label: &str) {
    let response = ui.add(
        TextEdit::singleline(text)
            .hint_text(hint)
            .desired_width(ui.available_width()),
    );
    response.widget_info(|| WidgetInfo::labeled(Role::TextInput, ui.is_enabled(), label));
}

fn right(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    let line = vec2(ui.available_width(), ui.spacing().interact_size.y);
    ui.allocate_ui_with_layout(line, Layout::right_to_left(Align::Center), add);
}

fn off(ui: &mut Ui, view: &mut ViewState) {
    row(
        ui,
        &t!("Sync"),
        &t!(
            "Keeps your library, progress, addons and most settings the same on your devices. Everything is encrypted on this device, so the sync server cannot read it."
        ),
        |ui| {
            right(ui, |ui| {
                if ui.button(t!("Turn on")).clicked()
                    && let Some(server) = server(view)
                {
                    view.sync_error = None;
                    view.sync_request = Some(SyncRequest::TurnOn { server });
                }
            });
        },
    );
    row(
        ui,
        &t!("Pair with a code"),
        &t!("Enter the sync code shown on your other device, or scan its QR code."),
        |ui| {
            ui.vertical(|ui| pair(ui, view));
        },
    );
    row(
        ui,
        &t!("Sync server"),
        &t!("Cineo's own server unless you run your own. Every device must use the same one."),
        |ui| {
            field(
                ui,
                &mut view.sync_server,
                DEFAULT_SERVER,
                &t!("Sync server"),
            );
        },
    );
}

fn pair(ui: &mut Ui, view: &mut ViewState) {
    field(ui, &mut view.sync_code, CODE_HINT, &t!("Sync code"));
    right(ui, |ui| {
        if ui.button(t!("Pair")).clicked() {
            match parse_code(&view.sync_code) {
                Ok(key) => {
                    if let Some(server) = server(view) {
                        view.sync_error = None;
                        view.sync_code.clear();
                        view.sync_request = Some(SyncRequest::Pair { key, server });
                    }
                }
                Err(err) => view.sync_error = Some(SyncInputError::Code(err)),
            }
        }
    });
    if let Some(error) = view.sync_error {
        ui.add(Label::new(RichText::new(input_error(error)).color(theme::WARNING)).wrap());
    }
}

fn status(now_ms: u64, sync: &SyncState) -> Text {
    if sync.is_running() {
        return t!("Syncing…");
    }
    match &sync.problem {
        Some(SyncProblem::SpaceGone) => {
            return t!(
                "The sync key was changed on another device. Turn sync off, then pair with the new code."
            );
        }
        Some(SyncProblem::Busy) => return t!("The sync server is busy. Cineo will try again."),
        Some(SyncProblem::Failed(failure)) => {
            let detail = match failure {
                SyncFailure::Insecure => t!("the server must use https"),
                SyncFailure::Network(_) => t!("the server could not be reached"),
                SyncFailure::Rejected(code) => t!("the server answered HTTP {code}", code = code),
                SyncFailure::Malformed => t!("the server's answer was unreadable"),
                SyncFailure::Conflict(_) => t!("other devices kept changing the same items"),
                SyncFailure::SpaceGone | SyncFailure::Unavailable { .. } => {
                    t!("the server is unavailable")
                }
            };
            return t!("The last sync failed: {detail}.", detail = detail.as_ref());
        }
        None => {}
    }
    match sync.last_ok_ms {
        None => t!("Not synced yet."),
        Some(at) => match now_ms.saturating_sub(at) / 60_000 {
            0 => t!("Synced just now."),
            minutes => t!("Synced {minutes} min ago.", minutes = minutes),
        },
    }
}

fn on(ui: &mut Ui, now_ms: u64, sync: &SyncState, view: &mut ViewState, out: &mut Vec<Action>) {
    row(ui, &t!("Sync is on"), &status(now_ms, sync), |ui| {
        right(ui, |ui| {
            let can_sync = !sync.is_running() && sync.problem != Some(SyncProblem::SpaceGone);
            if ui
                .add_enabled(can_sync, Button::new(t!("Sync now")))
                .clicked()
            {
                out.push(Action::SyncNow);
            }
        });
    });
    let code = format_code(&sync.pairing.key);
    row(
        ui,
        &t!("Sync code"),
        &t!(
            "Scan the QR code or enter this code on your other devices. Anyone with the code can read your library, including addon addresses, which can contain keys."
        ),
        |ui| {
            ui.vertical(|ui| {
                qr(ui, &pairing_link(&sync.pairing));
                ui.add(
                    Label::new(RichText::new(&code).monospace().color(theme::TEXT_BRIGHT)).wrap(),
                );
                right(ui, |ui| {
                    if ui.button(t!("Copy code")).clicked() {
                        ui.ctx().copy_text(code.clone());
                    }
                });
            });
        },
    );
    row(
        ui,
        &t!("Change sync key"),
        &t!(
            "Makes a new code and deletes the data under the old one, for example after losing a device. Your other devices need the new code."
        ),
        |ui| {
            right(ui, |ui| {
                if ui.button(t!("Change…")).clicked() {
                    view.confirm = Some(Confirm::RotateSyncKey);
                }
            });
        },
    );
    row(
        ui,
        &t!("Turn off sync"),
        &t!("This device stops syncing and keeps its library."),
        |ui| {
            right(ui, |ui| {
                if ui.button(t!("Turn off and delete…")).clicked() {
                    view.confirm = Some(Confirm::LeaveSyncAndDelete);
                }
                if ui.button(t!("Turn off…")).clicked() {
                    view.confirm = Some(Confirm::LeaveSync);
                }
            });
        },
    );
}

fn qr(ui: &mut Ui, text: &str) {
    let Ok(code) = QrCode::new(text.as_bytes()) else {
        return;
    };
    let width = code.width();
    let modules = (width + 2 * QR_QUIET) as f32;
    let module = QR_MODULE
        .min((ui.available_width() / modules).floor())
        .max(1.0);
    let side = modules * module;
    let (rect, response) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Image, true, t!("Sync QR code").as_ref()));
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, CornerRadius::same(theme::RADIUS), Color32::WHITE);
    for y in 0..width {
        for x in 0..width {
            if code[(x, y)] == Color::Dark {
                let min = rect.min
                    + vec2(
                        (x + QR_QUIET) as f32 * module,
                        (y + QR_QUIET) as f32 * module,
                    );
                painter.rect_filled(
                    egui::Rect::from_min_size(min, vec2(module, module)),
                    CornerRadius::ZERO,
                    Color32::BLACK,
                );
            }
        }
    }
}
