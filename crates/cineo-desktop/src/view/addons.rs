use super::icons::addon_text;
use super::widgets::{
    badge, centered_spinner, chip, clear_button, dim, empty, faint, offscreen_size, page_title,
    primary, rule, section, spinner,
};
use super::{ViewState, problem_text};
use crate::i18n::{Text, pt, t};
use crate::theme;
use cineo_core::addon::{AddonDescriptor, Manifest, TransportUrl};
use cineo_core::app::{Action, Loadable, Problem, State};
use eframe::egui::{
    self, Align, Button, Key, Label, Layout, Margin, RichText, TextEdit, Ui, Vec2, vec2,
};

fn buttons_width(ui: &Ui, labels: &[Text]) -> f32 {
    let style = ui.style();
    let font = egui::TextStyle::Button.resolve(style);
    let text: f32 = labels
        .iter()
        .map(|label| {
            ui.painter()
                .layout_no_wrap(label.to_string(), font.clone(), theme::TEXT_DIM)
                .size()
                .x
        })
        .sum();
    let count = labels.len() as f32;
    text + count * 2.0 * style.spacing.button_padding.x
        + (count - 1.0).max(0.0) * style.spacing.item_spacing.x
}

fn configure_button(ui: &mut Ui, transport: &TransportUrl) {
    if ui.button(t!("Configure")).clicked() {
        ui.ctx()
            .open_url(egui::OpenUrl::new_tab(transport.configure_url()));
    }
}

pub(super) fn addons_page(ui: &mut Ui, state: &State, view: &mut ViewState, out: &mut Vec<Action>) {
    page_title(
        ui,
        &t!("Addons"),
        Some(&t!(
            "When two addons return the same item, the one higher in the list wins."
        )),
    );
    section(ui, &t!("Install an addon"), |_| {});
    ui.horizontal(|ui| {
        let busy = state.install.as_ref().is_some_and(Loadable::is_loading);
        let field_width = (ui.available_width() - 110.0).clamp(160.0, 560.0);
        let field = ui.add(
            TextEdit::singleline(&mut view.addon_input)
                .hint_text("https://…/manifest.json")
                .margin(Margin {
                    left: 8,
                    right: 30,
                    top: 5,
                    bottom: 5,
                })
                .desired_width(field_width),
        );
        clear_button(ui, &field, &mut view.addon_input);
        let submitted = field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
        if primary(ui, !busy, &t!("Install")).clicked() || (submitted && !busy) {
            out.push(Action::InstallAddon(view.addon_input.clone()));
        }
    });
    match &state.install {
        Some(Loadable::Loading) => {
            ui.horizontal(|ui| {
                spinner(ui);
                ui.label(dim(&t!("Installing…")));
            });
        }
        Some(Loadable::Ready(name)) => {
            ui.label(
                RichText::new(t!("Installed {name}", name = name.as_str())).color(theme::SUCCESS),
            );
        }
        Some(Loadable::Failed(err)) => {
            ui.horizontal(|ui| {
                ui.label(RichText::new(problem_text(err)).color(theme::DANGER));
                if let Problem::ConfigurationRequired(transport) = err {
                    configure_button(ui, transport);
                }
            });
        }
        None => {}
    }
    ui.add_space(theme::SECTION_GAP);
    let count = state.addons.len();
    let unavailable = state.unavailable_addons();
    section(ui, &pt!("addons-installed", "Installed"), |ui| {
        if count > 0 {
            ui.label(faint(&t!("Addons: {count}", count = count)));
        }
    });
    if state.installed.is_empty() {
        empty(ui, &t!("Nothing installed yet."));
    }
    for (index, addon) in state.addons.iter().enumerate() {
        let manifest = &addon.manifest;
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let mut labels = vec![t!("Move up"), t!("Move down"), t!("Remove")];
            if manifest.behavior_hints.configurable {
                labels.push(t!("Configure"));
            }
            let actions_width = buttons_width(ui, &labels) + ui.spacing().item_spacing.x * 2.0;
            addon_info(ui, &addon.transport, manifest, actions_width);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if remove_button(ui) {
                    out.push(Action::RemoveAddon(addon.transport.clone()));
                }
                if manifest.behavior_hints.configurable {
                    configure_button(ui, &addon.transport);
                }
                if ui
                    .add_enabled(index + 1 < count, Button::new(t!("Move down")))
                    .clicked()
                {
                    out.push(Action::MoveAddon {
                        from: index,
                        to: index + 1,
                    });
                }
                if ui
                    .add_enabled(index > 0, Button::new(t!("Move up")))
                    .clicked()
                {
                    out.push(Action::MoveAddon {
                        from: index,
                        to: index - 1,
                    });
                }
            });
        });
        ui.add_space(6.0);
        rule(ui);
    }
    for transport in unavailable {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let url = transport.as_url();
            let unnamed = t!("Addon");
            ui.label(
                RichText::new(url.host_str().unwrap_or(&unnamed))
                    .font(theme::strong())
                    .color(theme::TEXT_BRIGHT),
            );
            badge(ui, &t!("Could not load"), theme::DANGER);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if remove_button(ui) {
                    out.push(Action::RemoveAddon(transport.clone()));
                }
                if ui.button(t!("Retry")).clicked() {
                    out.push(Action::InstallAddon(url.as_str().to_owned()));
                }
            });
        });
        ui.add_space(6.0);
        rule(ui);
    }
    addon_lists(ui, state, view, out);
}

fn addon_info(ui: &mut Ui, transport: &TransportUrl, manifest: &Manifest, actions_width: f32) {
    ui.vertical(|ui| {
        ui.set_max_width((ui.available_width() - actions_width).max(200.0));
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("{} {}", manifest.name, manifest.version))
                    .font(theme::strong())
                    .color(theme::TEXT_BRIGHT),
            );
            if let Some(host) = transport.as_url().host_str() {
                ui.label(faint(host));
            }
        });
        if let Some(description) = &manifest.description {
            ui.add(
                Label::new(addon_text(
                    description,
                    &theme::body(),
                    theme::TEXT_DIM,
                    theme::TEXT_DIM,
                ))
                .wrap(),
            );
        }
        let hints = &manifest.behavior_hints;
        if hints.adult || hints.p2p || hints.configuration_required {
            ui.horizontal(|ui| {
                if hints.adult {
                    badge(ui, &t!("Adult content"), theme::WARNING);
                }
                if hints.p2p {
                    badge(ui, &t!("Peer-to-peer"), theme::WARNING);
                }
                if hints.configuration_required {
                    badge(ui, &t!("Needs configuration"), theme::WARNING);
                }
            });
        }
    });
}

fn addon_lists(ui: &mut Ui, state: &State, view: &mut ViewState, out: &mut Vec<Action>) {
    let Some(lists) = state.addon_lists.as_ref().filter(|l| !l.is_empty()) else {
        return;
    };
    ui.add_space(theme::SECTION_GAP);
    section(ui, &t!("Discover addons"), |_| {});
    ui.add(
        Label::new(faint(&t!(
            "Addons in these lists are made by third parties. Install only those you trust."
        )))
        .wrap(),
    );
    ui.add_space(theme::GAP / 2.0);
    let selected = lists
        .iter()
        .position(|l| {
            view.addon_list
                .as_ref()
                .is_some_and(|(addon, path)| *addon == l.target.addon && *path == l.target.path)
        })
        .unwrap_or(0);
    let several_addons = lists
        .iter()
        .any(|l| l.target.addon != lists[0].target.addon);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::splat(6.0);
        for (index, list) in lists.iter().enumerate() {
            let target = &list.target;
            let label = if several_addons {
                format!("{} — {}", target.name, target.addon_name)
            } else {
                target.name.clone()
            };
            if chip(ui, &label, index == selected).clicked() {
                view.addon_list = Some((target.addon.clone(), target.path.clone()));
            }
        }
    });
    ui.add_space(theme::GAP);
    let busy = state.install.as_ref().is_some_and(Loadable::is_loading);
    let actions_width = [
        t!("Install"),
        t!("Configure"),
        pt!("addon-installed", "Installed"),
    ]
    .into_iter()
    .map(|label| buttons_width(ui, &[label]))
    .fold(0.0, f32::max)
        + ui.spacing().item_spacing.x * 2.0;
    match &lists[selected].addons {
        Loadable::Loading => centered_spinner(ui),
        Loadable::Failed(err) => {
            ui.horizontal(|ui| {
                ui.label(RichText::new(problem_text(err)).color(theme::DANGER));
                if ui.button(t!("Retry")).clicked() {
                    out.push(Action::LoadAddonLists);
                }
            });
        }
        Loadable::Ready(addons) if addons.is_empty() => empty(ui, &t!("This list is empty.")),
        Loadable::Ready(addons) => {
            let width = ui.available_width();
            for addon in addons {
                let id = ui.id().with(("listed", &addon.transport));
                if let Some(size) = offscreen_size(ui, id, width) {
                    ui.add_space(size.y);
                    continue;
                }
                let top = ui.cursor().min.y;
                listed_addon(ui, state, addon, busy, actions_width, out);
                let height = ui.cursor().min.y - top;
                ui.data_mut(|d| d.insert_temp(id, vec2(width, height)));
            }
        }
    }
}

fn remove_button(ui: &mut Ui) -> bool {
    ui.add(Button::new(
        RichText::new(t!("Remove")).color(theme::DANGER),
    ))
    .clicked()
}

fn listed_addon(
    ui: &mut Ui,
    state: &State,
    addon: &AddonDescriptor,
    busy: bool,
    actions_width: f32,
    out: &mut Vec<Action>,
) {
    let manifest = &addon.manifest;
    let installed = state.installed.contains(&addon.transport);
    let needs_setup = manifest.behavior_hints.configuration_required;
    let label = if installed {
        pt!("addon-installed", "Installed")
    } else if needs_setup {
        t!("Configure")
    } else {
        t!("Install")
    };
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        addon_info(ui, &addon.transport, manifest, actions_width);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if installed {
                ui.label(RichText::new(label).color(theme::SUCCESS));
            } else if needs_setup {
                configure_button(ui, &addon.transport);
            } else if ui.add_enabled(!busy, Button::new(label)).clicked() {
                out.push(Action::InstallAddon(addon.transport.to_string()));
            }
        });
    });
    ui.add_space(6.0);
    rule(ui);
}
