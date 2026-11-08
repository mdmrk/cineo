use super::icons::{Icon, append_icon, paint_icon, paint_nav_label};
use super::widgets::{compact, faint, lerp_color, spinner};
use super::{Page, ViewState, go};
use crate::i18n::t;
use crate::{brand, theme};
use cineo_core::app::{Action, State};
use eframe::egui::{
    self, Align, Color32, CornerRadius, Frame, Layout, Margin, Mesh, Rect, Response, Role, Sense,
    Stroke, TextFormat, Ui, WidgetInfo, pos2, text::LayoutJob, vec2,
};

pub(super) fn sidebar(ui: &mut Ui, state: &State, view: &mut ViewState, out: &mut Vec<Action>) {
    let compact = compact(ui.ctx());
    let width = if compact {
        theme::SIDEBAR_COMPACT_WIDTH
    } else {
        theme::SIDEBAR_WIDTH
    };
    egui::Panel::left("nav")
        .resizable(false)
        .exact_size(width)
        .show_separator_line(false)
        .frame(
            Frame::new()
                .fill(theme::SIDEBAR)
                .inner_margin(Margin::symmetric(10, 14)),
        )
        .show(ui, |ui| {
            let edge = ui.max_rect().right() + 9.5;
            ui.painter().vline(
                edge,
                ui.max_rect().y_range().expand(14.0),
                Stroke::new(1.0, theme::RULE),
            );
            logo(ui, width);
            ui.spacing_mut().item_spacing.y = 2.0;
            let mut item = |ui: &mut Ui, page: Page| {
                let selected = view.page == page && state.detail.is_none();
                if nav_item(ui, page, selected, compact).clicked() {
                    go(page, state, view, out);
                }
            };
            for page in Page::ALL {
                if page != Page::Settings {
                    item(ui, page);
                }
            }
            ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                if let Some(latest) = state.available_update() {
                    update_link(ui, latest, compact);
                }
                let version = faint(&t!("app-version", version = env!("CARGO_PKG_VERSION")));
                ui.horizontal(|ui| {
                    ui.add_space(9.0);
                    ui.add_visible(!compact, egui::Label::new(version).truncate());
                });
                ui.add_space(2.0);
                item(ui, Page::Settings);
                if !state.addons_loading.is_empty() {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        spinner(ui);
                        if !compact {
                            ui.label(faint(&t!("loading-addons")));
                        }
                    });
                }
            });
        });
}

fn update_link(ui: &mut Ui, latest: &semver::Version, compact: bool) {
    let text = t!("update-available", version = latest.to_string());
    let mut job = LayoutJob::default();
    append_icon(
        &mut job,
        Icon::Named("circle-arrow-up"),
        15.0,
        theme::ACCENT,
    );
    if !compact {
        job.append(
            &text,
            6.0,
            TextFormat::simple(theme::caption(), theme::ACCENT),
        );
    }
    ui.horizontal(|ui| {
        ui.add_space(9.0);
        let link = ui
            .add(egui::Link::new(job))
            .on_hover_text(t!("update-available-help"));
        link.widget_info(|| WidgetInfo::labeled(Role::Link, true, &text));
        if link.clicked() {
            ui.ctx()
                .open_url(egui::OpenUrl::new_tab(cineo_core::app::release_page(
                    latest,
                )));
        }
    });
}

fn logo(ui: &mut Ui, sidebar_width: f32) {
    let window = ui.ctx().content_rect();
    let size = vec2(theme::LOGO_WIDTH, theme::LOGO_WIDTH * theme::LOGO_ASPECT);
    let left = if sidebar_width < theme::SIDEBAR_WIDTH {
        sidebar_width / 2.0 - size.x * theme::LOGO_MOUSTACHE_X
    } else {
        theme::LOGO_BLEED.x
    };
    let rect = Rect::from_min_size(window.min + vec2(left, theme::LOGO_BLEED.y), size);
    let height = window.min.y + theme::NAV_TOP - ui.cursor().min.y;
    let (_, response) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Image, true, "Cineo"));
    let sidebar = Rect::from_min_max(window.min, pos2(window.min.x + sidebar_width, window.max.y));
    let painter = ui
        .ctx()
        .layer_painter(ui.layer_id())
        .with_clip_rect(sidebar);
    let backdrop = Rect::from_min_size(
        window.min,
        vec2(theme::SIDEBAR_WIDTH, theme::NAV_TOP + theme::GRAIN_FADE),
    );
    let ppp = ui.ctx().pixels_per_point();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a small positive size in pixels"
    )]
    let pixels = [
        (backdrop.width() * ppp).round() as usize,
        (backdrop.height() * ppp).round() as usize,
    ];
    let full = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    painter.image(
        brand::grain(ui.ctx(), pixels),
        backdrop,
        full,
        Color32::WHITE,
    );
    if let Some(texture) = brand::logo(ui.ctx()) {
        let mut mesh = Mesh::with_texture(texture);
        for row in 0..=LOGO_FADE_ROWS {
            #[expect(clippy::cast_precision_loss, reason = "a small row count")]
            let v = row as f32 / LOGO_FADE_ROWS as f32;
            let y = rect.top() + v * rect.height();
            let tint = theme::LOGO_TINT
                .gamma_multiply(brand::fade((y - backdrop.top()) / backdrop.height()));
            for (x, u) in [(rect.left(), 0.0), (rect.right(), 1.0)] {
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: pos2(x, y),
                    uv: pos2(u, v),
                    color: tint,
                });
            }
            if row > 0 {
                let i = row * 2;
                mesh.add_triangle(i - 2, i - 1, i + 1);
                mesh.add_triangle(i - 2, i + 1, i);
            }
        }
        painter.add(mesh);
    }
}

const LOGO_FADE_ROWS: u32 = 24;

fn nav_item(ui: &mut Ui, page: Page, selected: bool, compact: bool) -> Response {
    let label = page.label();
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::click());
    response.widget_info(|| WidgetInfo::selected(Role::Button, true, selected, &label));
    if ui.is_rect_visible(rect) {
        let hover = ui
            .ctx()
            .animate_bool_with_time(response.id, response.hovered(), theme::ANIM);
        let painter = ui.painter();
        let radius = CornerRadius::same(theme::RADIUS);
        if selected {
            painter.rect_filled(rect, radius, theme::SURFACE);
        } else if hover > 0.0 {
            painter.rect_filled(rect, radius, theme::PANEL.gamma_multiply(hover));
        }
        let color = if selected {
            theme::TEXT_BRIGHT
        } else {
            lerp_color(theme::TEXT_DIM, theme::TEXT_BRIGHT, hover)
        };
        if compact {
            paint_icon(painter, page.icon(), rect.center(), 18.0, color);
        } else {
            paint_nav_label(painter, rect, page.icon(), 18.0, &label, color);
        }
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}
