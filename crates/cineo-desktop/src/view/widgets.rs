use super::detail::tag;
use super::icons::{Icon, paint_icon};
use crate::i18n::t;
use crate::{brand, theme};
use eframe::egui::{
    self, Align, Button, Color32, CornerRadius, FontId, Frame, Label, Layout, Margin, Mesh, Modal,
    Pos2, Rect, Response, RichText, Role, Sense, Spinner, Stroke, TextFormat, Ui, UiBuilder, Vec2,
    WidgetInfo, pos2, text::LayoutJob, vec2,
};

pub(crate) fn dialog(
    ui: &Ui,
    id: &str,
    title: &str,
    text: &str,
    detail: Option<&str>,
    accept: &str,
) -> Option<bool> {
    let mut answer = None;
    let modal = Modal::new(egui::Id::unique(id))
        .frame(
            Frame::new()
                .fill(theme::PANEL)
                .stroke(Stroke::new(1.0, theme::RULE))
                .corner_radius(CornerRadius::same(theme::RADIUS))
                .inner_margin(Margin::same(20)),
        )
        .show(ui.ctx(), |ui| {
            ui.set_max_width(480.0);
            ui.label(RichText::new(title).font(theme::heading()));
            ui.add_space(theme::GAP);
            ui.add(Label::new(dim(text)).wrap());
            if let Some(detail) = detail {
                ui.add_space(theme::GAP);
                ui.add(
                    Label::new(RichText::new(detail).monospace().color(theme::TEXT_BRIGHT)).wrap(),
                );
            }
            ui.add_space(theme::GAP * 1.5);
            ui.horizontal(|ui| {
                if primary(ui, true, accept).clicked() {
                    answer = Some(true);
                }
                if ui.button(t!("cancel")).clicked() {
                    answer = Some(false);
                }
            });
        });
    answer.or_else(|| modal.should_close().then_some(false))
}

pub(super) fn offscreen_size(ui: &Ui, id: egui::Id, width: f32) -> Option<Vec2> {
    ui.data(|d| d.get_temp::<Vec2>(id))
        .filter(|size| size.x == width)
        .filter(|size| !ui.is_rect_visible(Rect::from_min_size(ui.cursor().min, *size)))
}

pub(crate) fn compact(ctx: &egui::Context) -> bool {
    ctx.content_rect().width() < theme::COMPACT_BELOW
}

pub(super) fn column<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let available = ui.available_rect_before_wrap();
    let margin = theme::PAGE_MARGIN;
    let width = (available.width() - 2.0 * margin).clamp(0.0, theme::CONTENT_MAX_WIDTH);
    let left = available.min.x + (available.width() - width) / 2.0;
    let rect = Rect::from_min_max(
        pos2(left, available.min.y),
        pos2(left + width, available.max.y),
    );
    ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
        ui.set_width(width);
        add(ui)
    })
    .inner
}

pub(crate) fn page_title(ui: &mut Ui, text: &str, dek: Option<&str>) {
    ui.label(
        RichText::new(text)
            .font(theme::heading())
            .color(theme::TEXT_BRIGHT),
    );
    if let Some(dek) = dek {
        ui.label(dim(dek));
    }
    ui.add_space(theme::GAP * 1.5);
}

pub(crate) fn section(ui: &mut Ui, title: &str, right: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(title)
                .font(theme::section())
                .color(theme::TEXT_BRIGHT),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), right);
    });
    ui.add_space(-2.0);
    rule(ui);
    ui.add_space(8.0);
}

pub(super) fn rule(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        Stroke::new(1.0, theme::RULE),
    );
}

pub(super) fn clear_button(ui: &Ui, field: &Response, text: &mut String) -> bool {
    if text.is_empty() {
        return false;
    }
    let center = pos2(field.rect.max.x - 15.0, field.rect.center().y);
    let rect = Rect::from_center_size(center, Vec2::splat(22.0));
    let response = ui
        .interact(rect, field.id.with("clear"), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, t!("clear-field")));
    let ink = if response.hovered() {
        theme::TEXT_BRIGHT
    } else {
        theme::TEXT_DIM
    };
    paint_icon(ui.painter(), Icon::Named("x"), center, 14.0, ink);
    if !response.clicked() {
        return false;
    }
    text.clear();
    field.request_focus();
    true
}

pub(crate) fn gradient(ui: &Ui, rect: Rect, from: Color32, to: Color32, horizontal: bool) {
    let (tl, tr, br, bl) = if horizontal {
        (from, to, to, from)
    } else {
        (from, from, to, to)
    };
    let mut mesh = Mesh::default();
    mesh.colored_vertex(rect.left_top(), tl);
    mesh.colored_vertex(rect.right_top(), tr);
    mesh.colored_vertex(rect.right_bottom(), br);
    mesh.colored_vertex(rect.left_bottom(), bl);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    ui.painter().add(mesh);
}

fn primary_button(text: &str) -> Button<'_> {
    Button::new(
        RichText::new(text)
            .font(theme::strong())
            .color(theme::ON_ACCENT),
    )
    .fill(theme::ACCENT)
}

pub(crate) fn primary(ui: &mut Ui, enabled: bool, text: &str) -> Response {
    let response = ui.add_enabled(enabled, primary_button(text));
    hover_glow(ui, &response);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn hover_glow(ui: &Ui, response: &Response) {
    if response.hovered() && response.enabled() {
        ui.painter().rect_filled(
            response.rect,
            CornerRadius::same(theme::RADIUS),
            theme::ACCENT_HOVER.gamma_multiply(0.25),
        );
    }
}

pub(super) fn chip(ui: &mut Ui, text: &str, selected: bool) -> Response {
    let ink = if selected { theme::BG } else { theme::TEXT };
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), theme::body(), ink);
    let size = galley.size() + vec2(24.0, 12.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| WidgetInfo::selected(Role::Button, ui.is_enabled(), selected, text));
    if ui.is_rect_visible(rect) {
        let fill = if selected {
            theme::TEXT_BRIGHT
        } else if response.hovered() {
            theme::SURFACE_HOVER
        } else {
            theme::SURFACE
        };
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(theme::RADIUS), fill);
        painter.galley(rect.center() - galley.size() / 2.0, galley, ink);
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub(super) fn badge(ui: &mut Ui, text: &str, color: Color32) {
    tag(
        ui,
        text,
        Color32::TRANSPARENT,
        color,
        Stroke::new(1.0, theme::RULE),
    );
}

pub(super) fn tags(ui: &mut Ui, items: &[String]) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::splat(5.0);
        for item in items {
            Frame::new()
                .fill(theme::SURFACE)
                .corner_radius(CornerRadius::same(theme::RADIUS))
                .inner_margin(Margin::symmetric(8, 3))
                .show(ui, |ui| {
                    ui.label(RichText::new(item).small().color(theme::TEXT));
                });
        }
    });
}

pub(super) fn spinner(ui: &mut Ui) {
    ui.add(Spinner::new().size(18.0).color(theme::TEXT_DIM));
}

pub(super) fn centered_spinner(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 48.0), Sense::hover());
    paint_spinner(ui, rect.center(), 24.0, theme::TEXT_DIM);
}

pub(crate) fn paint_spinner(ui: &Ui, center: Pos2, size: f32, color: Color32) {
    Spinner::new()
        .color(color)
        .paint_at(ui, Rect::from_center_size(center, Vec2::splat(size)));
}

pub(super) fn loading_screen(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 240.0), Sense::hover());
    paint_loading(ui, rect.center(), theme::LOADING_WIDTH, theme::TEXT_DIM);
}

pub(crate) fn paint_loading(ui: &Ui, center: Pos2, width: f32, tint: Color32) {
    let size = vec2(width, width * brand::LOADING_SIZE.y / brand::LOADING_SIZE.x);
    brand::paint_loading(ui, Rect::from_center_size(center, size), tint);
}

pub(super) fn icon_button(ui: &Ui, rect: Rect, icon: Icon, label: &str) -> Response {
    let response = ui.interact(rect, ui.scope_id().with(label), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, label));
    let painter = ui.painter();
    let fill = if response.hovered() {
        theme::SURFACE_HOVER
    } else {
        theme::SCRIM
    };
    painter.rect_filled(rect, CornerRadius::same(theme::RADIUS), fill);
    let galley = painter.layout_no_wrap(label.to_owned(), theme::body(), theme::TEXT_BRIGHT);
    let content = 16.0 + 6.0 + galley.size().x;
    let left = rect.center().x - content / 2.0;
    paint_icon(
        painter,
        icon,
        pos2(left + 8.0, rect.center().y),
        16.0,
        theme::TEXT_BRIGHT,
    );
    painter.galley(
        pos2(left + 22.0, rect.center().y - galley.size().y / 2.0),
        galley,
        theme::TEXT_BRIGHT,
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub(super) fn caps(text: &str, font: FontId, color: Color32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.append(
        &text.to_uppercase(),
        0.0,
        TextFormat {
            font_id: font,
            color,
            extra_letter_spacing: theme::CAPS_SPACING,
            ..TextFormat::default()
        },
    );
    job
}

pub(crate) fn lerp_color(from: Color32, to: Color32, t: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(
        lerp_u8(from.r(), to.r(), t),
        lerp_u8(from.g(), to.g(), t),
        lerp_u8(from.b(), to.b(), t),
        lerp_u8(from.a(), to.a(), t),
    )
}

fn lerp_u8(from: u8, to: u8, t: f32) -> u8 {
    let value = egui::lerp(f32::from(from)..=f32::from(to), t.clamp(0.0, 1.0));
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to 0–255"
    )]
    let value = value.round().clamp(0.0, 255.0) as u8;
    value
}

pub(super) fn empty(ui: &mut Ui, text: &str) {
    ui.add_space(theme::GAP / 2.0);
    ui.label(dim(text));
    ui.add_space(theme::GAP / 2.0);
}

pub(crate) fn dim(text: &str) -> RichText {
    RichText::new(text).color(theme::TEXT_DIM)
}

pub(super) fn faint(text: &str) -> RichText {
    RichText::new(text).small().color(theme::TEXT_FAINT)
}
