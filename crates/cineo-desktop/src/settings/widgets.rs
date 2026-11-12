use cineo_core::app::{Action, Setting, TorrentPort};
use eframe::egui::{
    self, Align, Align2, CornerRadius, Label, Layout, Popup, PopupCloseBehavior, Rect, ScrollArea,
    Sense, TextEdit, Ui, WidgetInfo, WidgetType, pos2, vec2,
};

use crate::i18n::Text;
use crate::theme;
use crate::view::{Icon, dim, lerp_color, paint_icon};

const CONTROL_WIDTH: f32 = 220.0;

pub(super) fn switch(
    ui: &mut Ui,
    out: &mut Vec<Action>,
    title: &str,
    help: &str,
    on: bool,
    setting: fn(bool) -> Setting,
) {
    row(ui, title, help, |ui| {
        if toggle(ui, title, on) {
            out.push(Action::ChangeSetting(setting(!on)));
        }
    });
}

#[expect(
    clippy::too_many_arguments,
    reason = "one call per setting reads best flat"
)]
pub(super) fn pick<T: Copy + PartialEq>(
    ui: &mut Ui,
    out: &mut Vec<Action>,
    title: &str,
    help: &str,
    current: T,
    options: &[T],
    name: impl Fn(T) -> Text,
    setting: fn(T) -> Setting,
) {
    row(ui, title, help, |ui| {
        if let Some(value) = select(ui, title, current, || options.iter().copied(), name) {
            out.push(Action::ChangeSetting(setting(value)));
        }
    });
}

pub(super) fn number(
    ui: &mut Ui,
    out: &mut Vec<Action>,
    title: &str,
    help: &str,
    value: u32,
    range: (u32, u32, u32),
    setting: fn(u32) -> Setting,
) {
    row(ui, title, help, |ui| {
        if let Some(value) = slider(ui, title, value, range) {
            out.push(Action::ChangeSetting(setting(value)));
        }
    });
}

#[expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "small whole numbers within the range"
)]
fn slider(ui: &mut Ui, label: &str, value: u32, (min, max, step): (u32, u32, u32)) -> Option<u32> {
    let width = ui.available_width().min(CONTROL_WIDTH);
    let (rect, response) = ui.allocate_exact_size(vec2(width, 30.0), Sense::click_and_drag());
    response.widget_info(|| WidgetInfo::slider(ui.is_enabled(), f64::from(value), label));
    let track = Rect::from_min_max(
        pos2(rect.min.x + 8.0, rect.center().y - 2.0),
        pos2(rect.max.x - 60.0, rect.center().y + 2.0),
    );
    let mut new = value;
    if let Some(pointer) = response.interact_pointer_pos() {
        let t = ((pointer.x - track.min.x) / track.width()).clamp(0.0, 1.0);
        let raw = min as f32 + t * (max - min) as f32;
        new = ((raw / step as f32).round() as u32 * step).clamp(min, max);
    }
    if response.has_focus() {
        let (down, up) = ui.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowLeft) || i.key_pressed(egui::Key::ArrowDown),
                i.key_pressed(egui::Key::ArrowRight) || i.key_pressed(egui::Key::ArrowUp),
            )
        });
        if down {
            new = new.saturating_sub(step).max(min);
        }
        if up {
            new = (new + step).min(max);
        }
    }
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let t = (new - min) as f32 / (max - min) as f32;
        let knob = pos2(egui::lerp(track.x_range(), t), track.center().y);
        painter.rect_filled(track, CornerRadius::same(2), theme::SURFACE_HOVER);
        painter.rect_filled(
            Rect::from_min_max(track.min, pos2(knob.x, track.max.y)),
            CornerRadius::same(2),
            theme::ACCENT,
        );
        let knob_color = if response.hovered() || response.dragged() || response.has_focus() {
            theme::TEXT_BRIGHT
        } else {
            theme::TEXT
        };
        painter.circle_filled(knob, 7.0, knob_color);
        painter.text(
            pos2(rect.max.x, rect.center().y),
            Align2::RIGHT_CENTER,
            format!("{new} %"),
            theme::body(),
            theme::TEXT_BRIGHT,
        );
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand);
    (new != value).then_some(new)
}

pub(super) fn port_field(ui: &mut Ui, label: &str, port: TorrentPort) -> Option<TorrentPort> {
    let id = egui::Id::new("torrent-port");
    let draft = id.with("draft");
    let mut text = ui
        .data(|d| d.get_temp::<String>(draft))
        .unwrap_or_else(|| port.get().to_string());
    let response = ui.add(
        TextEdit::singleline(&mut text)
            .id(id)
            .char_limit(5)
            .desired_width(80.0),
    );
    response.widget_info(|| WidgetInfo::labeled(WidgetType::TextEdit, ui.is_enabled(), label));
    let mut chosen = None;
    if response.lost_focus() {
        chosen = text
            .trim()
            .parse::<u32>()
            .ok()
            .filter(|v| (TorrentPort::MIN..=TorrentPort::MAX).contains(v))
            .map(TorrentPort::new)
            .filter(|new| *new != port);
        ui.data_mut(|d| d.remove::<String>(draft));
    } else if response.has_focus() {
        let digits: String = text.chars().filter(char::is_ascii_digit).collect();
        ui.data_mut(|d| d.insert_temp(draft, digits));
    } else {
        ui.data_mut(|d| d.remove::<String>(draft));
    }
    chosen
}

pub(crate) fn row(ui: &mut Ui, title: &str, help: &str, control: impl FnOnce(&mut Ui)) {
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

pub(super) fn select<T: Copy + PartialEq, I: IntoIterator<Item = T>>(
    ui: &mut Ui,
    label: &str,
    current: T,
    options: impl FnOnce() -> I,
    name: impl Fn(T) -> Text,
) -> Option<T> {
    let (rect, response) = ui.allocate_exact_size(
        vec2(ui.available_width().min(CONTROL_WIDTH), 30.0),
        Sense::click(),
    );
    response.widget_info(|| {
        let mut info = WidgetInfo::labeled(WidgetType::ComboBox, ui.is_enabled(), label);
        info.current_text_value = Some(name(current).into_owned());
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
        .layout(Layout::top_down_justified(Align::Min))
        .close_behavior(PopupCloseBehavior::CloseOnClick)
        .show(|ui| {
            ui.set_min_width(ui.available_width());
            ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                for option in options() {
                    let selected = option == current;
                    if ui.selectable_label(selected, name(option)).clicked() && !selected {
                        picked = Some(option);
                    }
                }
            });
        });
    picked
}
