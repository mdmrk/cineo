//! The Settings page: a section index beside one scrolling list of settings.

use cineo_core::app::{Action, LANGUAGES, Language, Setting, State};
use eframe::egui::{
    self, Align, Align2, CornerRadius, Frame, Label, Layout, Margin, Modal, Popup,
    PopupCloseBehavior, Rect, Response, RichText, ScrollArea, Sense, Stroke, TextFormat, Ui,
    UiBuilder, WidgetInfo, WidgetType, pos2, text::LayoutJob, vec2,
};

use crate::theme;
use crate::view::{
    Icon, P2P_NOTICE, ViewState, append_icon, caps, dim, lerp_color, page_margin, page_title,
    paint_icon, primary, section,
};

/// The parts of the Settings page, in page order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Languages,
    Torrents,
    Data,
    Keyboard,
    About,
}

impl Section {
    const ALL: [Self; 5] = [
        Self::Languages,
        Self::Torrents,
        Self::Data,
        Self::Keyboard,
        Self::About,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Languages => "Languages",
            Self::Torrents => "Torrents",
            Self::Data => "Data",
            Self::Keyboard => "Keyboard",
            Self::About => "About",
        }
    }
}

/// A question asked before an action that cannot be undone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirm {
    ResetSettings,
}

const NAV_WIDTH: f32 = 168.0;
const CONTROL_WIDTH: f32 = 220.0;

pub(crate) fn page(ui: &mut Ui, state: &State, view: &mut ViewState, out: &mut Vec<Action>) {
    let jump = view.settings_jump.take();
    let full = ui.available_rect_before_wrap();
    let margin = page_margin(ui);
    let show_nav = full.width() >= 900.0;
    let nav = Rect::from_min_size(
        full.min + vec2(margin, margin),
        vec2(NAV_WIDTH, full.height() - margin),
    );
    let content_left = if show_nav {
        nav.max.x + 24.0
    } else {
        full.min.x
    };
    let content = Rect::from_min_max(pos2(content_left, full.min.y), full.max);

    let mut current = Section::ALL[0];
    ui.scope_builder(UiBuilder::new().max_rect(content), |ui| {
        ScrollArea::vertical()
            .id_salt("Settings")
            .auto_shrink(false)
            .show(ui, |ui| {
                let visible_top = ui.clip_rect().top();
                ui.add_space(margin);
                let width = (ui.available_width() - margin).clamp(0.0, theme::CONTENT_MAX_WIDTH);
                ui.scope_builder(
                    UiBuilder::new().max_rect(Rect::from_min_size(
                        ui.cursor().min + vec2(if show_nav { 0.0 } else { margin }, 0.0),
                        vec2(width, f32::INFINITY),
                    )),
                    |ui| {
                        ui.set_width(width);
                        page_title(ui, "Settings", None);
                        for part in Section::ALL {
                            let top = ui.cursor().min.y;
                            section(ui, part.label(), |_| {});
                            ui.add_space(theme::GAP / 2.0);
                            if jump == Some(part) {
                                ui.scroll_to_rect(
                                    Rect::from_min_size(
                                        pos2(ui.min_rect().min.x, top),
                                        vec2(1.0, 1.0),
                                    ),
                                    Some(Align::TOP),
                                );
                            }
                            if top <= visible_top + 48.0 {
                                current = part;
                            }
                            body(ui, part, state, view, out);
                            ui.add_space(theme::SECTION_GAP);
                        }
                    },
                );
                ui.add_space(margin);
            });
    });
    if show_nav {
        ui.scope_builder(UiBuilder::new().max_rect(nav), |ui| {
            ui.add_space(theme::GAP * 4.0);
            for part in Section::ALL {
                if nav_item(ui, part.label(), part == jump.unwrap_or(current)).clicked() {
                    view.settings_jump = Some(part);
                }
            }
        });
    }
    confirm(ui, view, out);
}

fn body(ui: &mut Ui, section: Section, state: &State, view: &mut ViewState, out: &mut Vec<Action>) {
    let settings = &state.settings;
    match section {
        Section::Languages => {
            row(
                ui,
                "Subtitle language",
                SUBTITLE_HELP,
                |ui| {
                    language(ui, "Subtitle language", settings.subtitle_language)
                        .map(Setting::SubtitleLanguage)
                },
                out,
            );
        }
        Section::Torrents => {
            row(
                ui,
                "Show and play torrent streams",
                P2P_NOTICE,
                |ui| {
                    toggle(ui, "Show and play torrent streams", settings.p2p_enabled)
                        .then_some(Setting::P2pEnabled(!settings.p2p_enabled))
                },
                out,
            );
        }
        Section::Data => {
            row_with(
                ui,
                "Reset all settings",
                "Every setting on this page goes back to its default.",
                |ui| {
                    if ui.button("Reset…").clicked() {
                        view.confirm = Some(Confirm::ResetSettings);
                    }
                },
            );
        }
        Section::Keyboard => keyboard(ui),
        Section::About => {
            ui.label(dim(&format!(
                "Cineo {}. Made by people who stay for the credits.",
                env!("CARGO_PKG_VERSION")
            )));
        }
    }
}

const SUBTITLE_HELP: &str = "Turned on when a video starts: from the file if it has \
them, otherwise from a subtitles addon.";

/// A setting: its title and help on the left, its control on the right.
fn row(
    ui: &mut Ui,
    title: &str,
    help: &str,
    control: impl FnOnce(&mut Ui) -> Option<Setting>,
    out: &mut Vec<Action>,
) {
    let mut changed = None;
    row_with(ui, title, help, |ui| changed = control(ui));
    if let Some(setting) = changed {
        out.push(Action::ChangeSetting(setting));
    }
}

fn row_with(ui: &mut Ui, title: &str, help: &str, control: impl FnOnce(&mut Ui)) {
    let width = ui.available_width();
    let control_width = CONTROL_WIDTH.min(width * 0.45);
    let text_width = (width - control_width - 24.0).max(0.0);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(text_width);
            let galley = ui.painter().layout(
                title.to_owned(),
                theme::strong(),
                theme::TEXT_BRIGHT,
                text_width,
            );
            let (rect, _) = ui.allocate_exact_size(galley.size(), Sense::hover());
            ui.painter().galley(rect.min, galley, theme::TEXT_BRIGHT);
            if !help.is_empty() {
                ui.add(Label::new(dim(help).small()).wrap());
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
            ui.set_width(control_width);
            control(ui);
        });
    });
    ui.add_space(theme::GAP);
}

fn toggle(ui: &mut Ui, label: &str, on: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(40.0, 22.0), Sense::click());
    response.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, ui.is_enabled(), on, label));
    if ui.is_rect_visible(rect) {
        let t = ui
            .ctx()
            .animate_bool_with_time(response.id, on, theme::ANIM);
        let track = Rect::from_center_size(rect.center(), vec2(40.0, 20.0));
        let painter = ui.painter();
        painter.rect_filled(
            track,
            CornerRadius::same(10),
            lerp_color(theme::SURFACE_HOVER, theme::ACCENT, t),
        );
        let knob_x = egui::lerp(track.min.x + 10.0..=track.max.x - 10.0, t);
        painter.circle_filled(
            pos2(knob_x, track.center().y),
            7.0,
            lerp_color(theme::TEXT_DIM, theme::ON_ACCENT, t),
        );
    }
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

/// A dropdown; returns the option picked this frame.
fn select<T: Copy + PartialEq>(
    ui: &mut Ui,
    label: &str,
    current: T,
    options: impl IntoIterator<Item = T>,
    name: impl Fn(T) -> &'static str,
) -> Option<T> {
    let (rect, response) = ui.allocate_exact_size(
        vec2(ui.available_width().min(CONTROL_WIDTH), 30.0),
        Sense::click(),
    );
    response.widget_info(|| {
        let mut info = WidgetInfo::labeled(WidgetType::ComboBox, ui.is_enabled(), label);
        info.current_text_value = Some(name(current).to_owned());
        info
    });
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let fill = if response.hovered() {
            theme::SURFACE_HOVER
        } else {
            theme::SURFACE
        };
        painter.rect_filled(rect, CornerRadius::same(theme::RADIUS), fill);
        painter.text(
            pos2(rect.min.x + 10.0, rect.center().y),
            Align2::LEFT_CENTER,
            name(current),
            theme::body(),
            theme::TEXT_BRIGHT,
        );
        paint_icon(
            painter,
            Icon::ChevronDown,
            pos2(rect.max.x - 16.0, rect.center().y),
            16.0,
            theme::TEXT_DIM,
        );
    }
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    let mut picked = None;
    Popup::from_toggle_button_response(&response)
        .width(rect.width())
        .close_behavior(PopupCloseBehavior::CloseOnClick)
        .show(|ui| {
            ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                for option in options {
                    let selected = option == current;
                    if ui.selectable_label(selected, name(option)).clicked() && !selected {
                        picked = Some(option);
                    }
                }
            });
        });
    picked
}

fn language(ui: &mut Ui, label: &str, current: Option<Language>) -> Option<Option<Language>> {
    select(
        ui,
        label,
        current,
        std::iter::once(None).chain(LANGUAGES.iter().copied().map(Some)),
        |l| l.map_or("None", |l| l.name),
    )
}

fn nav_item(ui: &mut Ui, label: &str, selected: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
    response.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, selected, label));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if selected {
            painter.rect_filled(
                Rect::from_min_size(pos2(rect.min.x, rect.min.y + 6.0), vec2(3.0, 18.0)),
                0.0,
                theme::ACCENT,
            );
        }
        let color = if selected || response.hovered() {
            theme::TEXT_BRIGHT
        } else {
            theme::TEXT_DIM
        };
        let galley = painter.layout_job(caps(label, theme::nav(), color));
        painter.galley(
            pos2(rect.min.x + 14.0, rect.center().y - galley.size().y / 2.0),
            galley,
            color,
        );
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn keyboard(ui: &mut Ui) {
    egui::Grid::new("shortcuts")
        .num_columns(2)
        .spacing(vec2(32.0, 10.0))
        .show(ui, |ui| {
            let key = TextFormat::simple(theme::body(), theme::TEXT_BRIGHT);
            let plain = |text: &str| LayoutJob::single_section(text.to_owned(), key.clone());
            let mut back = plain("Esc  ·  Alt+");
            append_icon(&mut back, Icon::ArrowLeft, 15.0, theme::TEXT_BRIGHT);
            back.append("  ·  mouse back", 0.0, key.clone());
            let rows = [
                (plain("Ctrl+F  or  /"), "Search"),
                (plain("Ctrl+1 … Ctrl+6"), "Switch page"),
                (back, "Leave a detail page"),
                (plain("Shift+wheel"), "Scroll a row sideways"),
            ];
            for (keys, what) in rows {
                ui.label(keys);
                ui.label(dim(what));
                ui.end_row();
            }
        });
}

fn confirm(ui: &Ui, view: &mut ViewState, out: &mut Vec<Action>) {
    let Some(question) = view.confirm else {
        return;
    };
    let (title, text, button, action) = match question {
        Confirm::ResetSettings => (
            "Reset all settings?",
            "Every setting goes back to its default. Your addons and library are kept.",
            "Reset",
            Action::ResetSettings,
        ),
    };
    let mut done = false;
    let modal = Modal::new(egui::Id::new("settings_confirm"))
        .frame(
            Frame::new()
                .fill(theme::PANEL)
                .stroke(Stroke::new(1.0, theme::RULE))
                .corner_radius(CornerRadius::same(theme::RADIUS))
                .inner_margin(Margin::same(20)),
        )
        .show(ui.ctx(), |ui| {
            ui.set_max_width(420.0);
            ui.label(RichText::new(title).font(theme::heading()));
            ui.add_space(theme::GAP);
            ui.add(Label::new(dim(text)).wrap());
            ui.add_space(theme::GAP * 1.5);
            ui.horizontal(|ui| {
                if primary(ui, true, button).clicked() {
                    out.push(action);
                    done = true;
                }
                if ui.button("Cancel").clicked() {
                    done = true;
                }
            });
        });
    if done || modal.should_close() {
        view.confirm = None;
    }
}
