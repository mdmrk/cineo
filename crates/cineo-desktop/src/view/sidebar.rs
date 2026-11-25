use super::icons::{Icon, paint_icon, paint_nav_label};
use super::widgets::{compact, faint, lerp_color, spinner};
use super::{Page, ViewState, go};
use crate::i18n::{Text, t};
use crate::player::stats::bytes;
use crate::{brand, theme};
use cineo_core::app::{Action, State, UpdateFailure, UpdateProgress};
use eframe::egui::{
    self, Align, Color32, CornerRadius, Frame, Layout, Margin, Mesh, Rect, Response, Role, Sense,
    Stroke, Ui, WidgetInfo, pos2, vec2,
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
            let rest_id = ui.id().with("nav_rest");
            let rest = ui.data(|data| data.get_temp::<f32>(rest_id)).unwrap_or(0.0);
            logo(ui, width, rest);
            let items_top = ui.cursor().min.y;
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
            let items = ui.cursor().min.y - items_top;
            let mut clicked = None;
            let bottom = ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                if let Some(latest) = state.available_update() {
                    clicked = update_link(ui, state, latest, compact);
                }
                let version = faint(&t!("Cineo v{version}", version = env!("CARGO_PKG_VERSION")));
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
                            ui.label(faint(&t!("Loading addons…")));
                        }
                    });
                }
            });
            let measured = items + bottom.response.rect.height() + NAV_GAP;
            if (measured - rest).abs() > 0.5 {
                ui.data_mut(|data| data.insert_temp(rest_id, measured));
                ui.ctx().request_discard("sidebar height changed");
            }
            out.extend(clicked);
        });
}

enum UpdateClick {
    OpenPage,
    Install,
    Restart,
    Nothing,
}

fn update_failure(failure: UpdateFailure) -> Text {
    match failure {
        UpdateFailure::Download => t!("The download failed. Check the connection and try again."),
        UpdateFailure::Signature => {
            t!("The release's signature did not verify, so nothing was installed.")
        }
        UpdateFailure::Checksum => {
            t!("A downloaded file did not match its checksum, so nothing was installed.")
        }
        UpdateFailure::Install => t!("Cineo could not replace its executable."),
    }
}

#[expect(clippy::cast_precision_loss, reason = "a size shown to one decimal")]
fn download_hover(received: u64, total: Option<u64>) -> Text {
    let keeps_working = t!("Cineo keeps working while it downloads.");
    let size = match total {
        Some(total) => format!("{} / {}", bytes(received as f64), bytes(total as f64)),
        None if received > 0 => bytes(received as f64),
        None => return keeps_working,
    };
    format!("{size}\n{keeps_working}").into()
}

const UPDATE_INDENT: f32 = 9.0;
const UPDATE_ICON: f32 = 15.0;
const UPDATE_GAP: f32 = 6.0;
const UPDATE_ROW_GAP: f32 = 5.0;
const UPDATE_BAR: f32 = 3.0;
const UPDATE_COMPACT_BAR: f32 = 20.0;

fn update_link(
    ui: &mut Ui,
    state: &State,
    latest: &semver::Version,
    compact: bool,
) -> Option<Action> {
    let version = latest.to_string();
    let mut shown = None;
    let mut percent = None;
    let (text, hover, click) = if state.self_update.is_some() {
        match state.update_progress {
            UpdateProgress::Idle => (
                t!("Update to v{version}", version = version),
                t!("Download and check it; it runs after a restart."),
                UpdateClick::Install,
            ),
            progress @ UpdateProgress::Downloading { received, total } => {
                percent = progress.percent();
                let downloading = t!("Downloading v{version}…", version = version);
                let text = match percent {
                    Some(percent) => t!(
                        "Downloading v{version}… {percent}%",
                        version = version,
                        percent = percent
                    ),
                    None => downloading.clone(),
                };
                shown = Some(downloading);
                (text, download_hover(received, total), UpdateClick::Nothing)
            }
            UpdateProgress::Ready => (
                t!("Restart to update"),
                t!("Cineo closes and starts v{version}.", version = version),
                UpdateClick::Restart,
            ),
            UpdateProgress::Failed(failure) => (
                t!("Update failed, try again"),
                update_failure(failure),
                UpdateClick::Install,
            ),
        }
    } else {
        (
            t!("v{version} available", version = version),
            t!("Open the release page to download it."),
            UpdateClick::OpenPage,
        )
    };
    let shown = shown.unwrap_or_else(|| text.clone());
    let clickable = !matches!(click, UpdateClick::Nothing);
    let width = ui.available_width();
    let text_left = UPDATE_INDENT + UPDATE_ICON + UPDATE_GAP;
    let galley = (!compact).then(|| {
        ui.painter().layout(
            shown.into_owned(),
            theme::caption(),
            Color32::PLACEHOLDER,
            (width - text_left).max(1.0),
        )
    });
    let side = percent.filter(|_| !compact).map(|percent| {
        ui.painter().layout_no_wrap(
            t!("{percent}%", percent = percent).into_owned(),
            theme::caption(),
            theme::TEXT_DIM,
        )
    });
    let line = galley
        .as_ref()
        .and_then(|galley| galley.rows.first())
        .map_or(UPDATE_ICON, |row| row.rect().height());
    let top = line.max(UPDATE_ICON);
    let text_height = galley
        .as_ref()
        .map_or(top, |galley| galley.size().y + (top - line) / 2.0)
        .max(top);
    let row_height = match (&side, percent) {
        (Some(side), _) => side.size().y,
        (None, Some(_)) => UPDATE_BAR,
        (None, None) => 0.0,
    };
    let height = if percent.is_some() {
        text_height + UPDATE_ROW_GAP + row_height
    } else {
        text_height
    };
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Role::Link, true, &text));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let hovered = clickable && (response.hovered() || response.has_focus());
        let color = if hovered {
            theme::ACCENT_HOVER
        } else {
            theme::ACCENT
        };
        let icon = pos2(
            rect.left() + UPDATE_INDENT + UPDATE_ICON / 2.0,
            rect.top() + top / 2.0,
        );
        paint_icon(
            painter,
            Icon::Named("circle-arrow-up"),
            icon,
            UPDATE_ICON,
            color,
        );
        if let Some(galley) = galley {
            let underline = if hovered {
                Stroke::new(1.0, color)
            } else {
                Stroke::NONE
            };
            let at = pos2(rect.left() + text_left, rect.top() + (top - line) / 2.0);
            painter.add(egui::epaint::TextShape::new(at, galley, color).with_underline(underline));
        }
        if let Some(percent) = percent {
            let row_top = rect.top() + text_height + UPDATE_ROW_GAP;
            let middle = row_top + row_height / 2.0;
            let (left, right) = match &side {
                Some(side) => {
                    let right = rect.right();
                    painter.galley(
                        pos2(right - side.size().x, row_top),
                        side.clone(),
                        theme::TEXT_DIM,
                    );
                    (rect.left() + text_left, right - side.size().x - UPDATE_GAP)
                }
                None => (
                    icon.x - UPDATE_COMPACT_BAR / 2.0,
                    icon.x + UPDATE_COMPACT_BAR / 2.0,
                ),
            };
            let track = Rect::from_min_max(
                pos2(left, middle - UPDATE_BAR / 2.0),
                pos2(right.max(left), middle + UPDATE_BAR / 2.0),
            );
            let radius = CornerRadius::same(2);
            painter.rect_filled(track, radius, theme::SURFACE);
            let mut done = track;
            done.set_width(track.width() * f32::from(percent.min(100)) / 100.0);
            if done.width() > 0.0 {
                painter.rect_filled(done, radius, theme::ACCENT);
            }
        }
    }
    let response = if clickable {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    };
    if !response.on_hover_text(hover).clicked() {
        return None;
    }
    match click {
        UpdateClick::OpenPage => {
            ui.ctx()
                .open_url(egui::OpenUrl::new_tab(cineo_core::app::release_page(
                    latest,
                )));
            None
        }
        UpdateClick::Install => Some(Action::InstallUpdate),
        UpdateClick::Restart => Some(Action::RestartToUpdate),
        UpdateClick::Nothing => None,
    }
}

const NAV_GAP: f32 = 8.0;

fn logo(ui: &mut Ui, sidebar_width: f32, rest: f32) {
    let window = ui.ctx().content_rect();
    let size = vec2(theme::LOGO_WIDTH, theme::LOGO_WIDTH * theme::LOGO_ASPECT);
    let left = if sidebar_width < theme::SIDEBAR_WIDTH {
        sidebar_width / 2.0 - size.x * theme::LOGO_MOUSTACHE_X
    } else {
        theme::LOGO_BLEED.x
    };
    let rect = Rect::from_min_size(window.min + vec2(left, theme::LOGO_BLEED.y), size);
    let height = (window.min.y + theme::NAV_TOP - ui.cursor().min.y)
        .min(ui.available_height() - rest)
        .max(0.0);
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
