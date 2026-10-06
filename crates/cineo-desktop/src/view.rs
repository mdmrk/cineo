//! Rendering. [`show`] draws a [`State`] snapshot and returns the
//! [`Action`]s the user triggered; it performs no IO and holds no business
//! rules (ADR-0001, ADR-0011). Addon strings are shown as plain text only.

use cineo_core::addon::{Meta, MetaPreview, PosterShape, Stream};
use cineo_core::app::{
    Action, CatalogTarget, Detail, LibraryItem, Loadable, Row, State, StreamGroup, board_targets,
    continue_watching,
};
use eframe::egui::{
    self, Align, Align2, Button, Color32, ComboBox, CornerRadius, Frame, Image, Label, Layout,
    Margin, Rect, RichText, ScrollArea, Sense, Stroke, StrokeKind, TextEdit, Ui, Vec2,
    text::LayoutJob,
};
use url::Url;

use crate::theme;

/// The top-level pages in the sidebar.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Board,
    Discover,
    Search,
    Library,
    Addons,
}

impl Page {
    const ALL: [Self; 5] = [
        Self::Board,
        Self::Discover,
        Self::Search,
        Self::Library,
        Self::Addons,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Board => "Board",
            Self::Discover => "Discover",
            Self::Search => "Search",
            Self::Library => "Library",
            Self::Addons => "Addons",
        }
    }
}

/// Presentation-only state: the open page and text being typed.
#[derive(Debug, Clone, Default)]
pub struct ViewState {
    pub page: Page,
    pub addon_input: String,
    pub search_input: String,
    /// Season shown on a series detail page; `None` means the first one.
    pub season: Option<u32>,
}

/// Draws the whole window and returns the actions to dispatch.
pub fn show(ui: &mut Ui, state: &State, view: &mut ViewState) -> Vec<Action> {
    let mut out = Vec::new();
    sidebar(ui, state, view, &mut out);
    if let Some(notice) = &state.notice {
        egui::Panel::top("notice")
            .frame(
                Frame::new()
                    .fill(theme::SURFACE)
                    .inner_margin(Margin::same(10)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(notice).color(theme::WARNING));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("Dismiss").clicked() {
                            out.push(Action::DismissNotice);
                        }
                    });
                });
            });
    }
    egui::CentralPanel::default()
        .frame(
            Frame::new()
                .fill(theme::BG)
                .inner_margin(Margin::same(theme::PAGE_MARGIN)),
        )
        .show(ui, |ui| {
            if let Some(detail) = &state.detail {
                detail_page(ui, detail, view, &mut out);
                return;
            }
            match view.page {
                Page::Board => board_page(ui, state, &mut out),
                Page::Discover => discover_page(ui, state, &mut out),
                Page::Search => search_page(ui, state, view, &mut out),
                Page::Library => library_page(ui, state, &mut out),
                Page::Addons => addons_page(ui, state, view, &mut out),
            }
        });
    out
}

fn sidebar(ui: &mut Ui, state: &State, view: &mut ViewState, out: &mut Vec<Action>) {
    egui::Panel::left("nav")
        .resizable(false)
        .exact_size(theme::SIDEBAR_WIDTH)
        .frame(
            Frame::new()
                .fill(theme::PANEL)
                .inner_margin(Margin::same(16)),
        )
        .show(ui, |ui| {
            ui.label(RichText::new("Cineo").font(theme::heading()).strong());
            ui.add_space(theme::GAP);
            for page in Page::ALL {
                let selected = view.page == page && state.detail.is_none();
                let button = Button::selectable(selected, page.label())
                    .min_size(Vec2::new(ui.available_width(), 32.0));
                if ui.add(button).clicked() {
                    if state.detail.is_some() {
                        out.push(Action::CloseDetail);
                    }
                    if page == Page::Discover
                        && state.discover.target.is_none()
                        && let Some(first) = board_targets(&state.addons).into_iter().next()
                    {
                        out.push(Action::OpenDiscover {
                            addon: first.addon,
                            path: first.path,
                        });
                    }
                    view.page = page;
                }
            }
            if !state.addons_loading.is_empty() {
                ui.add_space(theme::GAP);
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(dim("Loading addons…"));
                });
            }
        });
}

// --- pages ---

fn board_page(ui: &mut Ui, state: &State, out: &mut Vec<Action>) {
    ScrollArea::vertical().show(ui, |ui| {
        let resume = continue_watching(&state.library);
        if !resume.is_empty() {
            ui.label(RichText::new("Continue watching").font(theme::heading()));
            ScrollArea::horizontal().id_salt("continue").show(ui, |ui| {
                ui.horizontal(|ui| {
                    for item in resume {
                        library_card(ui, item, out);
                    }
                });
            });
            ui.add_space(theme::GAP);
        }
        if state.addons.is_empty() && state.addons_loading.is_empty() {
            ui.label(dim(
                "No addons installed. Open Addons and paste an addon's manifest URL.",
            ));
        }
        for (index, row) in state.board.iter().enumerate() {
            catalog_row(ui, ("board", index), row, true, out);
        }
    });
}

fn discover_page(ui: &mut Ui, state: &State, out: &mut Vec<Action>) {
    let targets = board_targets(&state.addons);
    let discover = &state.discover;
    ui.label(RichText::new("Discover").font(theme::heading()));
    if targets.is_empty() {
        ui.label(dim("No installed addon has a browsable catalog."));
        return;
    }
    ui.horizontal(|ui| {
        let current = discover
            .target
            .as_ref()
            .map_or_else(|| "Choose a catalog".to_owned(), target_label);
        ComboBox::from_id_salt("catalog")
            .selected_text(current)
            .width(280.0)
            .show_ui(ui, |ui| {
                for target in &targets {
                    let selected = discover.target.as_ref() == Some(target);
                    if ui
                        .selectable_label(selected, target_label(target))
                        .clicked()
                        && !selected
                    {
                        out.push(Action::OpenDiscover {
                            addon: target.addon.clone(),
                            path: target.path.clone(),
                        });
                    }
                }
            });
        if let Some((options, required)) = genre_options(state) {
            let current = discover.genre.as_deref().unwrap_or("All genres");
            ComboBox::from_id_salt("genre")
                .selected_text(current)
                .width(180.0)
                .show_ui(ui, |ui| {
                    if !required
                        && ui
                            .selectable_label(discover.genre.is_none(), "All genres")
                            .clicked()
                    {
                        out.push(Action::SetDiscoverGenre(None));
                    }
                    for option in options {
                        let selected = discover.genre.as_deref() == Some(option);
                        if ui.selectable_label(selected, option).clicked() && !selected {
                            out.push(Action::SetDiscoverGenre(Some(option.to_owned())));
                        }
                    }
                });
        }
    });
    ui.add_space(theme::GAP);
    if let Some(error) = &discover.error {
        ui.label(RichText::new(error).color(theme::DANGER));
    }
    ScrollArea::vertical().show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            for item in &discover.items {
                if preview_card(ui, item).clicked() {
                    out.push(open_detail(item));
                }
            }
        });
        if discover.pending.is_some() {
            ui.spinner();
        } else if discover.next_skip.is_some() && ui.button("Load more").clicked() {
            out.push(Action::LoadMoreDiscover);
        }
    });
}

fn search_page(ui: &mut Ui, state: &State, view: &mut ViewState, out: &mut Vec<Action>) {
    ui.label(RichText::new("Search").font(theme::heading()));
    ui.horizontal(|ui| {
        let field = ui.add(
            TextEdit::singleline(&mut view.search_input)
                .hint_text("Movie or series title")
                .desired_width(360.0),
        );
        let submitted = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if ui.button("Search").clicked() || submitted {
            out.push(Action::Search(view.search_input.clone()));
        }
    });
    ui.add_space(theme::GAP);
    if state.search_query.is_empty() {
        return;
    }
    if state.search.is_empty() {
        ui.label(dim("No installed addon supports search."));
    }
    ScrollArea::vertical().show(ui, |ui| {
        for (index, row) in state.search.iter().enumerate() {
            catalog_row(ui, ("search", index), row, false, out);
        }
    });
}

fn library_page(ui: &mut Ui, state: &State, out: &mut Vec<Action>) {
    ui.label(RichText::new("Library").font(theme::heading()));
    if state.library.is_empty() {
        ui.label(dim("Items you play appear here."));
        return;
    }
    let mut items: Vec<&LibraryItem> = state.library.iter().collect();
    items.sort_by_key(|i| std::cmp::Reverse(i.updated_ms));
    ScrollArea::vertical().show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            for item in items {
                ui.vertical(|ui| {
                    library_card(ui, item, out);
                    if ui.small_button("Remove").clicked() {
                        out.push(Action::RemoveFromLibrary(item.id.clone()));
                    }
                });
            }
        });
    });
}

fn addons_page(ui: &mut Ui, state: &State, view: &mut ViewState, out: &mut Vec<Action>) {
    ui.label(RichText::new("Addons").font(theme::heading()));
    ui.horizontal(|ui| {
        let field = ui.add(
            TextEdit::singleline(&mut view.addon_input)
                .hint_text("https://…/manifest.json")
                .desired_width(420.0),
        );
        let submitted = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let busy = state.install.as_ref().is_some_and(Loadable::is_loading);
        if ui.add_enabled(!busy, Button::new("Install")).clicked() || (submitted && !busy) {
            out.push(Action::InstallAddon(view.addon_input.clone()));
        }
    });
    match &state.install {
        Some(Loadable::Loading) => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Installing…");
            });
        }
        Some(Loadable::Ready(name)) => {
            ui.label(format!("Installed {name}"));
        }
        Some(Loadable::Failed(err)) => {
            ui.label(RichText::new(err).color(theme::DANGER));
        }
        None => {}
    }
    ui.add_space(theme::GAP);
    let count = state.addons.len();
    ScrollArea::vertical().show(ui, |ui| {
        for (index, addon) in state.addons.iter().enumerate() {
            let manifest = &addon.manifest;
            Frame::new()
                .fill(theme::PANEL)
                .corner_radius(CornerRadius::same(theme::RADIUS))
                .inner_margin(Margin::same(12))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(format!("{} {}", manifest.name, manifest.version))
                                    .strong(),
                            );
                            // The full URL can carry configuration; show the host only.
                            if let Some(host) = addon.transport.as_url().host_str() {
                                ui.label(dim(host));
                            }
                            if let Some(description) = &manifest.description {
                                ui.add(Label::new(description).wrap());
                            }
                            let hints = &manifest.behavior_hints;
                            ui.horizontal(|ui| {
                                if hints.adult {
                                    badge(ui, "Adult content", theme::WARNING);
                                }
                                if hints.p2p {
                                    badge(ui, "Peer-to-peer", theme::WARNING);
                                }
                                if hints.configuration_required {
                                    badge(ui, "Needs configuration", theme::WARNING);
                                }
                            });
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button("Remove").clicked() {
                                out.push(Action::RemoveAddon(addon.transport.clone()));
                            }
                            if ui
                                .add_enabled(index + 1 < count, Button::new("Move down"))
                                .clicked()
                            {
                                out.push(Action::MoveAddon {
                                    from: index,
                                    to: index + 1,
                                });
                            }
                            if ui.add_enabled(index > 0, Button::new("Move up")).clicked() {
                                out.push(Action::MoveAddon {
                                    from: index,
                                    to: index - 1,
                                });
                            }
                        });
                    });
                });
            ui.add_space(6.0);
        }
    });
}

fn detail_page(ui: &mut Ui, detail: &Detail, view: &mut ViewState, out: &mut Vec<Action>) {
    if ui.button("← Back").clicked() {
        out.push(Action::CloseDetail);
        view.season = None;
    }
    let meta = detail.meta.ready();
    let preview = meta.map(|m| &m.preview).or(detail.preview.as_ref());
    ScrollArea::vertical().show(ui, |ui| {
        hero(ui, preview, meta, detail);
        match &detail.meta {
            Loadable::Loading => {
                ui.spinner();
            }
            Loadable::Failed(err) => {
                ui.label(RichText::new(err).color(theme::DANGER));
            }
            Loadable::Ready(meta) if !meta.videos.is_empty() => {
                episodes(ui, meta, detail, view, out);
            }
            Loadable::Ready(_) => {}
        }
        if detail.selected_video.is_some() {
            ui.add_space(theme::GAP);
            ui.label(RichText::new("Streams").font(theme::heading()));
            if detail.streams.is_empty() {
                ui.label(dim("No installed addon provides streams for this item."));
            }
            for (index, group) in detail.streams.iter().enumerate() {
                stream_group(ui, index, group, out);
            }
        }
    });
}

fn hero(ui: &mut Ui, preview: Option<&MetaPreview>, meta: Option<&Meta>, detail: &Detail) {
    let height = 280.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
    let radius = CornerRadius::same(theme::RADIUS);
    ui.painter().rect_filled(rect, radius, theme::PANEL);
    if let Some(background) = preview.and_then(|p| p.background.as_ref()) {
        Image::new(background.as_str())
            .corner_radius(radius)
            .tint(Color32::from_gray(70))
            .paint_at(ui, rect);
    }
    let inner = rect.shrink(20.0);
    let poster_rect = Rect::from_min_size(
        inner.min,
        Vec2::new((inner.height()) * 0.675, inner.height()),
    );
    if let Some(poster) = preview.and_then(|p| p.poster.as_ref()) {
        Image::new(poster.as_str())
            .corner_radius(radius)
            .paint_at(ui, poster_rect);
    }
    let text_rect =
        Rect::from_min_max(egui::pos2(poster_rect.max.x + 20.0, inner.min.y), inner.max);
    ui.scope_builder(egui::UiBuilder::new().max_rect(text_rect), |ui| {
        let name = preview.map_or(detail.id.as_str(), |p| p.name.as_str());
        ui.label(RichText::new(name).font(theme::title()).strong());
        let mut facts = Vec::new();
        if let Some(p) = preview {
            facts.extend(p.release_info.clone());
            facts.extend(p.imdb_rating.as_ref().map(|r| format!("IMDb {r}")));
            if !p.genres.is_empty() {
                facts.push(p.genres.join(", "));
            }
        }
        facts.extend(meta.and_then(|m| m.runtime.clone()));
        if !facts.is_empty() {
            ui.label(dim(&facts.join("  ·  ")));
        }
        if let Some(description) = preview.and_then(|p| p.description.as_ref()) {
            ui.add(Label::new(description).wrap());
        }
        if let Some(m) = meta {
            if !m.director.is_empty() {
                ui.label(dim(&format!("Director: {}", m.director.join(", "))));
            }
            if !m.cast.is_empty() {
                ui.label(dim(&format!("Cast: {}", m.cast.join(", "))));
            }
        }
    });
    ui.add_space(theme::GAP);
}

fn episodes(
    ui: &mut Ui,
    meta: &Meta,
    detail: &Detail,
    view: &mut ViewState,
    out: &mut Vec<Action>,
) {
    let seasons = meta.seasons();
    let season = view
        .season
        .filter(|s| seasons.contains(s))
        .or_else(|| seasons.first().copied());
    if !seasons.is_empty() {
        ComboBox::from_id_salt("season")
            .selected_text(season_label(season.unwrap_or_default()))
            .show_ui(ui, |ui| {
                for s in &seasons {
                    if ui
                        .selectable_label(season == Some(*s), season_label(*s))
                        .clicked()
                    {
                        view.season = Some(*s);
                    }
                }
            });
    }
    for video in meta
        .videos
        .iter()
        .filter(|v| season.is_none() || v.season == season)
    {
        let label = match (video.season, video.episode) {
            (Some(_), Some(e)) => format!("{e}. {}", video.title),
            _ => video.title.clone(),
        };
        let selected = detail.selected_video.as_deref() == Some(video.id.as_str());
        if ui.selectable_label(selected, label).clicked() && !selected {
            out.push(Action::SelectVideo(video.id.clone()));
        }
    }
}

fn stream_group(ui: &mut Ui, index: usize, group: &StreamGroup, out: &mut Vec<Action>) {
    ui.add_space(6.0);
    ui.label(RichText::new(&group.addon_name).strong());
    match &group.streams {
        Loadable::Loading => {
            ui.spinner();
        }
        Loadable::Failed(err) => {
            ui.label(RichText::new(err).color(theme::DANGER));
        }
        Loadable::Ready(streams) if streams.is_empty() => {
            ui.label(dim("No streams"));
        }
        Loadable::Ready(streams) => {
            for (stream_index, stream) in streams.iter().enumerate() {
                if stream_row(ui, stream) {
                    out.push(Action::Play {
                        group: index,
                        stream: stream_index,
                    });
                }
            }
        }
    }
}

/// One stream; returns true if Play was clicked.
fn stream_row(ui: &mut Ui, stream: &Stream) -> bool {
    let mut clicked = false;
    Frame::new()
        .fill(theme::PANEL)
        .corner_radius(CornerRadius::same(theme::RADIUS))
        .inner_margin(Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let playable = stream.source.is_playable();
                let play = ui
                    .add_enabled(playable, Button::new("Play"))
                    .on_disabled_hover_text(format!(
                        "{} streams are not supported yet",
                        stream.source.kind_label()
                    ));
                clicked = play.clicked();
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        if let Some(name) = &stream.name {
                            ui.label(RichText::new(name).strong());
                        }
                        badge(ui, stream.source.kind_label(), theme::TEXT_DIM);
                    });
                    if let Some(description) = &stream.description {
                        ui.add(Label::new(dim(description)).wrap());
                    }
                });
            });
        });
    clicked
}

// --- building blocks ---

fn catalog_row(ui: &mut Ui, salt: (&str, usize), row: &Row, see_all: bool, out: &mut Vec<Action>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(&row.target.title).font(theme::heading()));
        ui.label(dim(&row.target.addon_name));
        if see_all && ui.small_button("See all").clicked() {
            out.push(Action::OpenDiscover {
                addon: row.target.addon.clone(),
                path: row.target.path.clone(),
            });
        }
    });
    match &row.items {
        Loadable::Loading => {
            ui.spinner();
        }
        Loadable::Failed(err) => {
            ui.label(RichText::new(err).color(theme::DANGER));
        }
        Loadable::Ready(items) if items.is_empty() => {
            ui.label(dim("Nothing here"));
        }
        Loadable::Ready(items) => {
            ScrollArea::horizontal().id_salt(salt).show(ui, |ui| {
                ui.horizontal(|ui| {
                    for item in items {
                        if preview_card(ui, item).clicked() {
                            out.push(open_detail(item));
                        }
                    }
                });
            });
        }
    }
    ui.add_space(theme::GAP);
}

fn preview_card(ui: &mut Ui, item: &MetaPreview) -> egui::Response {
    card(
        ui,
        &item.name,
        item.poster.as_ref(),
        item.poster_shape,
        None,
    )
}

fn library_card(ui: &mut Ui, item: &LibraryItem, out: &mut Vec<Action>) {
    let progress = (item.time_offset_ms > 0).then(|| item.progress());
    if card(
        ui,
        &item.name,
        item.poster.as_ref(),
        PosterShape::Poster,
        progress,
    )
    .clicked()
    {
        out.push(Action::OpenDetail {
            content_type: item.content_type.clone(),
            id: item.id.clone(),
            preview: None,
        });
    }
}

/// A clickable poster with its title below and an optional progress bar.
/// Accessible as a button labelled with the title.
fn card(
    ui: &mut Ui,
    name: &str,
    poster: Option<&Url>,
    shape: PosterShape,
    progress: Option<f32>,
) -> egui::Response {
    let size = card_image_size(shape);
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(size.x, size.y + theme::CARD_CAPTION),
        Sense::click(),
    );
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), name));
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let image_rect = Rect::from_min_size(rect.min, size);
    let radius = CornerRadius::same(theme::RADIUS);
    let painter = ui.painter();
    painter.rect_filled(image_rect, radius, theme::SURFACE);
    if let Some(poster) = poster {
        Image::new(poster.as_str())
            .corner_radius(radius)
            .show_loading_spinner(false)
            .paint_at(ui, image_rect);
    } else {
        painter.text(
            image_rect.center(),
            Align2::CENTER_CENTER,
            initials(name),
            theme::title(),
            theme::TEXT_DIM,
        );
    }
    if let Some(progress) = progress {
        let bar = Rect::from_min_max(
            egui::pos2(image_rect.min.x, image_rect.max.y - 4.0),
            image_rect.max,
        );
        painter.rect_filled(bar, 0.0, Color32::from_black_alpha(160));
        let mut done = bar;
        done.set_width(bar.width() * progress);
        painter.rect_filled(done, 0.0, theme::ACCENT);
    }
    if response.hovered() {
        painter.rect_stroke(
            image_rect,
            radius,
            Stroke::new(2.0, theme::ACCENT),
            StrokeKind::Outside,
        );
    }
    let mut job = LayoutJob::simple(
        name.to_owned(),
        theme::caption(),
        if response.hovered() {
            theme::TEXT
        } else {
            theme::TEXT_DIM
        },
        size.x,
    );
    job.wrap.max_rows = 2;
    let galley = ui.painter().layout_job(job);
    ui.painter().galley(
        egui::pos2(rect.min.x, image_rect.max.y + 4.0),
        galley,
        theme::TEXT,
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn card_image_size(shape: PosterShape) -> Vec2 {
    let w = theme::CARD_WIDTH;
    match shape {
        PosterShape::Poster => Vec2::new(w, w / 0.675),
        PosterShape::Square => Vec2::new(w, w),
        PosterShape::Landscape => Vec2::new(w * 1.77, w),
    }
}

fn badge(ui: &mut Ui, text: &str, color: Color32) {
    Frame::new()
        .stroke(Stroke::new(1.0, color))
        .corner_radius(CornerRadius::same(4))
        .inner_margin(Margin::symmetric(6, 1))
        .show(ui, |ui| {
            ui.label(RichText::new(text).small().color(color));
        });
}

fn dim(text: &str) -> RichText {
    RichText::new(text).color(theme::TEXT_DIM)
}

fn initials(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .collect()
}

fn season_label(season: u32) -> String {
    if season == 0 {
        "Specials".to_owned()
    } else {
        format!("Season {season}")
    }
}

fn target_label(target: &CatalogTarget) -> String {
    format!("{} — {}", target.title, target.addon_name)
}

fn open_detail(item: &MetaPreview) -> Action {
    Action::OpenDetail {
        content_type: item.content_type.clone(),
        id: item.id.clone(),
        preview: Some(Box::new(item.clone())),
    }
}

/// The genre options of the Discover catalog, and whether one is required.
fn genre_options(state: &State) -> Option<(&[String], bool)> {
    let target = state.discover.target.as_ref()?;
    let genre = state
        .addons
        .iter()
        .find(|a| a.transport == target.addon)?
        .manifest
        .catalog(&target.path.content_type, &target.path.id)?
        .extra
        .iter()
        .find(|e| e.name == "genre" && !e.options.is_empty())?;
    Some((&genre.options, genre.is_required))
}
