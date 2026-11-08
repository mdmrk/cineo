use super::icons::{Icon, paint_icon};
use super::pages::catalog_title;
use super::widgets::{faint, gradient, lerp_color, section};
use super::{Page, ViewState, open_detail, problem_text};
use crate::i18n::t;
use crate::theme;
use cineo_core::addon::{MetaPreview, PosterShape};
use cineo_core::app::{Action, LibraryItem, Loadable, Row};
use eframe::egui::{
    self, Align, Button, Color32, CornerRadius, FontId, Image, Pos2, Rect, Response, RichText,
    ScrollArea, Sense, Stroke, StrokeKind, TextFormat, Ui, Vec2, WidgetInfo, WidgetType, pos2,
    scroll_area::ScrollBarVisibility, style::ScrollAnimation, text::LayoutJob, vec2,
};
use url::Url;

pub(super) fn catalog_row(
    ui: &mut Ui,
    salt: (&str, usize),
    row: &Row,
    see_all: Option<&mut ViewState>,
    out: &mut Vec<Action>,
) {
    section(ui, &catalog_title(&row.target), |ui| {
        if let Some(view) = see_all
            && ui
                .add(Button::new(RichText::new(t!("see-all")).color(theme::TEXT_DIM)).frame(false))
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
        {
            out.push(Action::OpenDiscover {
                addon: row.target.addon.clone(),
                path: row.target.path.clone(),
            });
            view.page = Page::Discover;
        }
        ui.label(faint(&row.target.addon_name));
    });
    match &row.items {
        Loadable::Loading => {
            skeleton_row(ui);
        }
        Loadable::Failed(err) => {
            ui.horizontal(|ui| {
                ui.label(faint(&t!("catalog-failed")));
                ui.label(RichText::new(problem_text(err)).color(theme::DANGER));
            });
        }
        Loadable::Ready(items) if items.is_empty() => {
            ui.label(faint(&t!("catalog-empty")));
        }
        Loadable::Ready(items) => {
            poster_strip(ui, salt, |ui| {
                for item in items {
                    if preview_card(ui, theme::CARD_WIDTH, item).clicked() {
                        out.push(open_detail(item));
                    }
                }
            });
        }
    }
    ui.add_space(theme::SECTION_GAP);
}

fn skeleton_row(ui: &mut Ui) {
    let size = card_size(theme::CARD_WIDTH, PosterShape::Poster);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), size.y), Sense::hover());
    let mut x = rect.min.x;
    while x + size.x <= rect.max.x {
        ui.painter().rect_filled(
            Rect::from_min_size(pos2(x, rect.min.y), size),
            CornerRadius::same(theme::POSTER_RADIUS),
            theme::PANEL,
        );
        x += size.x + theme::CARD_GAP;
    }
}

pub(super) fn poster_strip(
    ui: &mut Ui,
    salt: impl std::hash::Hash + std::fmt::Debug,
    add: impl FnOnce(&mut Ui),
) {
    let id = ui.make_persistent_id(salt);
    let pending: Option<f32> = ui.data_mut(|d| d.remove_temp(id));
    let output = ScrollArea::horizontal()
        .id_salt(id)
        .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
        .show(ui, |ui| {
            if let Some(delta) = pending {
                ui.scroll_with_delta_animation(vec2(delta, 0.0), ScrollAnimation::duration(0.12));
            }
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = theme::CARD_GAP;
                add(ui);
            });
        });
    let inner = Rect::from_min_size(
        output.inner_rect.min,
        vec2(
            output.inner_rect.width(),
            output.content_size.y.min(output.inner_rect.height()),
        ),
    );
    let max_offset = (output.content_size.x - inner.width()).max(0.0);
    let offset = output.state.offset.x;
    let hovered = ui.rect_contains_pointer(inner);
    let shown = ui
        .ctx()
        .animate_bool_with_time(id.with("arrows"), hovered, theme::ANIM);
    if shown <= 0.0 {
        return;
    }
    let page = inner.width() * 0.85;
    let y = inner.center().y;
    if offset > 1.0 && pager(ui, id, pos2(inner.min.x + 22.0, y), false, shown).clicked() {
        ui.data_mut(|d| d.insert_temp(id, page));
    }
    if offset < max_offset - 1.0
        && pager(ui, id, pos2(inner.max.x - 22.0, y), true, shown).clicked()
    {
        ui.data_mut(|d| d.insert_temp(id, -page));
    }
}

fn pager(ui: &Ui, strip: egui::Id, center: Pos2, right: bool, opacity: f32) -> Response {
    let rect = Rect::from_center_size(center, vec2(34.0, 56.0));
    let label = if right {
        t!("scroll-right")
    } else {
        t!("scroll-left")
    };
    let response = ui.interact(rect, strip.with(right), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &label));
    let fill = if response.hovered() {
        theme::SURFACE_HOVER
    } else {
        Color32::from_black_alpha(200)
    };
    let ink = theme::TEXT_BRIGHT;
    let painter = ui.painter();
    painter.rect_filled(
        rect,
        CornerRadius::same(theme::RADIUS),
        fill.gamma_multiply(opacity),
    );
    let icon = if right {
        Icon::ChevronRight
    } else {
        Icon::ChevronLeft
    };
    paint_icon(painter, icon, center, 22.0, ink.gamma_multiply(opacity));
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub(super) fn poster_grid(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(theme::CARD_GAP, theme::CARD_GAP);
        add(ui);
    });
}

pub(super) fn grid_card_width(available: f32) -> f32 {
    let gap = theme::CARD_GAP;
    let columns = ((available + gap) / (theme::GRID_CARD_MIN + gap))
        .floor()
        .max(1.0);
    let width = (available - gap * (columns - 1.0)) / columns;
    width.min(theme::GRID_CARD_MAX).floor()
}

pub(super) fn preview_card(ui: &mut Ui, width: f32, item: &MetaPreview) -> Response {
    card(
        ui,
        width,
        &item.name,
        item.release_info.as_deref(),
        item.poster.as_ref(),
        item.poster_shape,
        None,
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Shelf {
    ContinueWatching,
    Favorites,
    Library,
}

pub(super) fn library_card(
    ui: &mut Ui,
    width: f32,
    item: &LibraryItem,
    shelf: Shelf,
    out: &mut Vec<Action>,
) {
    let progress = (item.time_offset_ms > 0).then(|| item.progress());
    let response = card(
        ui,
        width,
        &item.name,
        None,
        item.poster.as_ref(),
        PosterShape::Poster,
        progress,
    );
    let rect = response.rect;
    let active = ui.rect_contains_pointer(rect) || response.has_focus();
    let hover = ui
        .ctx()
        .animate_bool_with_time(response.id.with("hover"), active, theme::ANIM);
    if shelf != Shelf::Favorites && item.stream.is_some() && ui.is_rect_visible(rect) {
        let center = rect.center();
        let radius = (rect.width() / 6.0).clamp(18.0, 28.0);
        let fill = lerp_color(Color32::from_black_alpha(160), theme::ACCENT, hover);
        let ink = lerp_color(theme::TEXT_BRIGHT, theme::ON_ACCENT, hover);
        let painter = ui.painter();
        painter.circle_filled(center, radius, fill);
        paint_icon(
            painter,
            Icon::Play,
            center + vec2(1.5, 0.0),
            radius * 0.9,
            ink,
        );
    }
    let corner = 20.0;
    let (heart, label) = if item.is_favorite() {
        (Icon::Heart, t!("favourite-remove"))
    } else {
        (Icon::Named("heart"), t!("favourite-add"))
    };
    let favorite = rect.left_top() + vec2(corner, corner);
    let shown = if item.is_favorite() { 1.0 } else { hover };
    if card_button(ui, item, ("favorite", favorite), heart, &label, shown) {
        out.push(Action::SetFavorite {
            id: item.id.clone(),
            favorite: !item.is_favorite(),
        });
    }
    let remove = rect.right_top() + vec2(-corner, corner);
    let removable = match shelf {
        Shelf::ContinueWatching => Some((
            t!("remove-continue"),
            Action::RemoveFromLibrary(item.id.clone()),
        )),
        Shelf::Library if item.was_played() => Some((
            t!("remove-library"),
            Action::RemoveFromLibrary(item.id.clone()),
        )),
        Shelf::Library | Shelf::Favorites => None,
    };
    if let Some((label, action)) = removable
        && card_button(
            ui,
            item,
            ("remove", remove),
            Icon::Named("x"),
            &label,
            hover,
        )
    {
        out.push(action);
    }
    if response.clicked() && shelf == Shelf::Favorites {
        out.push(open_detail(&item.preview()));
    } else if response.clicked() {
        out.push(Action::Resume(item.id.clone()));
    }
}

fn card_button(
    ui: &Ui,
    item: &LibraryItem,
    (role, center): (&str, Pos2),
    icon: Icon,
    label: &str,
    shown: f32,
) -> bool {
    let rect = Rect::from_center_size(center, vec2(28.0, 28.0));
    let response = ui
        .interact(rect, ui.id().with((&item.id, role)), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, label));
    if shown > 0.0 && ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let fill = if response.hovered() {
            Color32::from_black_alpha(220)
        } else {
            Color32::from_black_alpha(150)
        };
        painter.circle_filled(center, 14.0, fill.gamma_multiply(shown));
        let ink = if matches!(icon, Icon::Heart) {
            theme::ACCENT
        } else {
            theme::TEXT_BRIGHT
        };
        paint_icon(painter, icon, center, 16.0, ink.gamma_multiply(shown));
    }
    response.clicked()
}

fn card(
    ui: &mut Ui,
    width: f32,
    name: &str,
    year: Option<&str>,
    poster_url: Option<&Url>,
    shape: PosterShape,
    progress: Option<f32>,
) -> Response {
    let size = card_size(width, shape);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), name));
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let hover = ui.ctx().animate_bool_with_time(
        response.id,
        response.hovered() || response.has_focus(),
        theme::ANIM,
    );
    poster(ui, rect, name, poster_url, false);
    let painter = ui.painter();
    if let Some(progress) = progress {
        let track = Rect::from_min_max(pos2(rect.min.x, rect.max.y - 3.0), rect.max);
        painter.rect_filled(track, 0.0, theme::SCRIM);
        let mut done = track;
        done.set_width(track.width() * progress.clamp(0.0, 1.0));
        painter.rect_filled(done, 0.0, theme::ACCENT);
    }
    if hover > 0.0 && poster_url.is_some() {
        let band = Rect::from_min_max(pos2(rect.min.x, rect.max.y - 72.0), rect.max);
        gradient(
            ui,
            band,
            Color32::TRANSPARENT,
            Color32::from_black_alpha(235).gamma_multiply(hover),
            false,
        );
        let text = theme::TEXT_BRIGHT.gamma_multiply(hover);
        let mut job = LayoutJob::simple(name.to_owned(), theme::strong(), text, width - 14.0);
        job.wrap.max_rows = 2;
        if let Some(year) = year {
            job.append(
                &format!("  {year}"),
                0.0,
                TextFormat::simple(theme::caption(), theme::TEXT_DIM.gamma_multiply(hover)),
            );
        }
        let galley = painter.layout_job(job);
        let pos = pos2(rect.min.x + 7.0, rect.max.y - 8.0 - galley.size().y);
        painter.galley(pos, galley, text);
    }
    if hover > 0.0 {
        painter.rect_stroke(
            rect,
            CornerRadius::same(theme::POSTER_RADIUS),
            Stroke::new(2.0, theme::ACCENT.gamma_multiply(hover)),
            StrokeKind::Inside,
        );
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub(super) fn shadow(ui: &Ui, rect: Rect, strength: f32) {
    let shadow = egui::Shadow {
        offset: [0, 8],
        blur: 20,
        spread: 0,
        color: Color32::from_black_alpha(150).gamma_multiply(strength),
    };
    ui.painter()
        .add(shadow.as_shape(rect, CornerRadius::same(theme::POSTER_RADIUS)));
}

pub(super) fn poster(ui: &Ui, rect: Rect, name: &str, url: Option<&Url>, fade_in: bool) {
    let radius = CornerRadius::same(theme::POSTER_RADIUS);
    let painter = ui.painter();
    painter.rect_filled(rect, radius, theme::SURFACE);
    if let Some(url) = url {
        paint_cover(ui, url.as_str(), rect, radius, Color32::WHITE, 0.5, fade_in);
    } else {
        let font = FontId::new(
            (rect.width() / 7.0).clamp(14.0, 26.0),
            theme::strong_family(),
        );
        let mut job =
            LayoutJob::simple(name.to_owned(), font, theme::TEXT_DIM, rect.width() - 20.0);
        job.halign = Align::Center;
        job.wrap.max_rows = 4;
        let galley = painter.layout_job(job);
        let pos = pos2(rect.center().x, rect.center().y - galley.size().y / 2.0);
        painter.galley(pos, galley, theme::TEXT_DIM);
    }
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(1.0, theme::POSTER_EDGE),
        StrokeKind::Inside,
    );
}

pub(crate) const IMAGE_FILTER: egui::TextureOptions = egui::TextureOptions {
    mipmap_mode: Some(egui::TextureFilter::Linear),
    ..egui::TextureOptions::LINEAR
};

pub(crate) fn paint_cover(
    ui: &Ui,
    src: &str,
    rect: Rect,
    radius: CornerRadius,
    tint: Color32,
    focus_y: f32,
    fade_in: bool,
) {
    let image = Image::new(src)
        .corner_radius(radius)
        .texture_options(IMAGE_FILTER)
        .show_loading_spinner(false);
    let poll = image.load_for_size(ui.ctx(), rect.size()).ok();
    let opacity = if fade_in {
        let ready = matches!(poll, Some(egui::load::TexturePoll::Ready { .. }));
        ui.ctx().animate_bool_with_time_and_easing(
            egui::Id::new(("cover-fade", src)),
            ready,
            COVER_FADE,
            egui::emath::easing::cubic_out,
        )
    } else {
        1.0
    };
    let Some(size) = poll.and_then(|poll| poll.size()) else {
        return;
    };
    if opacity > 0.0 {
        image
            .uv(cover_uv(size, rect.size(), focus_y))
            .tint(tint.gamma_multiply(opacity))
            .paint_at(ui, rect);
    }
}

const COVER_FADE: f32 = 0.4;

pub(super) fn cover_uv(image: Vec2, target: Vec2, focus_y: f32) -> Rect {
    let full = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    if image.x <= 0.0 || image.y <= 0.0 || target.x <= 0.0 || target.y <= 0.0 {
        return full;
    }
    let image_aspect = image.x / image.y;
    let target_aspect = target.x / target.y;
    if image_aspect > target_aspect {
        let visible = target_aspect / image_aspect;
        let left = (1.0 - visible) / 2.0;
        Rect::from_min_max(pos2(left, 0.0), pos2(left + visible, 1.0))
    } else {
        let visible = image_aspect / target_aspect;
        let top = (1.0 - visible) * focus_y.clamp(0.0, 1.0);
        Rect::from_min_max(pos2(0.0, top), pos2(1.0, top + visible))
    }
}

fn card_size(width: f32, shape: PosterShape) -> Vec2 {
    let height = match shape {
        PosterShape::Poster => width * 1.5,
        PosterShape::Square => width,
        PosterShape::Landscape => width / 1.77,
    };
    vec2(width, height)
}
