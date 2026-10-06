//! Rendering. [`show`] draws a [`State`] snapshot and returns the
//! [`Action`]s the user triggered; it performs no IO and holds no business
//! rules (ADR-0001, ADR-0011). Addon strings are shown as plain text only.

use cineo_core::addon::{Meta, MetaPreview, PosterShape, Stream};
use cineo_core::app::{
    Action, CatalogTarget, Detail, LibraryItem, Loadable, Row, State, StreamGroup, board_targets,
    continue_watching,
};
use eframe::egui::{
    self, Align, Button, Color32, ComboBox, CornerRadius, FontId, Frame, Image, Label, Layout,
    Margin, Mesh, Rect, RichText, ScrollArea, Sense, Stroke, StrokeKind, TextEdit, TextFormat, Ui,
    UiBuilder, Vec2, text::LayoutJob,
};
use url::Url;

use crate::theme;

/// The top-level pages in the top bar.
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
    top_bar(ui, state, view, &mut out);
    if let Some(notice) = &state.notice {
        egui::Panel::top("notice")
            .show_separator_line(false)
            .frame(
                Frame::new()
                    .fill(theme::PANEL)
                    .inner_margin(Margin::symmetric(0, 8)),
            )
            .show(ui, |ui| {
                column(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(notice).color(theme::WARNING));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button("Dismiss").clicked() {
                                out.push(Action::DismissNotice);
                            }
                        });
                    });
                });
            });
    }
    egui::CentralPanel::default()
        .frame(Frame::new().fill(theme::BG))
        .show(ui, |ui| {
            let salt = if state.detail.is_some() {
                "detail"
            } else {
                view.page.label()
            };
            ScrollArea::vertical()
                .id_salt(salt)
                .auto_shrink(false)
                .show(ui, |ui| {
                    if let Some(detail) = &state.detail {
                        detail_page(ui, detail, view, &mut out);
                    } else {
                        ui.add_space(f32::from(theme::PAGE_MARGIN));
                        column(ui, |ui| match view.page {
                            Page::Board => board_page(ui, state, &mut out),
                            Page::Discover => discover_page(ui, state, &mut out),
                            Page::Search => search_page(ui, state, view, &mut out),
                            Page::Library => library_page(ui, state, &mut out),
                            Page::Addons => addons_page(ui, state, view, &mut out),
                        });
                    }
                    ui.add_space(f32::from(theme::PAGE_MARGIN));
                });
        });
    out
}

fn top_bar(ui: &mut Ui, state: &State, view: &mut ViewState, out: &mut Vec<Action>) {
    egui::Panel::top("nav")
        .resizable(false)
        .exact_size(theme::TOP_BAR_HEIGHT)
        .show_separator_line(false)
        .frame(Frame::new().fill(theme::PANEL))
        .show(ui, |ui| {
            let bottom = ui.max_rect().bottom() - 0.5;
            ui.painter().hline(
                ui.max_rect().x_range(),
                bottom,
                Stroke::new(1.0, theme::RULE),
            );
            column(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    logo(ui);
                    if !state.addons_loading.is_empty() {
                        ui.add_space(theme::GAP);
                        ui.spinner();
                        ui.label(faint("Loading addons…"));
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 20.0;
                        for page in Page::ALL.into_iter().rev() {
                            let selected = view.page == page && state.detail.is_none();
                            if nav_link(ui, page.label(), selected).clicked() {
                                if state.detail.is_some() {
                                    out.push(Action::CloseDetail);
                                }
                                if page == Page::Discover
                                    && state.discover.target.is_none()
                                    && let Some(first) =
                                        board_targets(&state.addons).into_iter().next()
                                {
                                    out.push(Action::OpenDiscover {
                                        addon: first.addon,
                                        path: first.path,
                                    });
                                }
                                view.page = page;
                            }
                        }
                    });
                });
            });
        });
}

/// Three dots and the name.
fn logo(ui: &mut Ui) {
    let radius = 6.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(radius * 5.6, radius * 2.0), Sense::hover());
    for (i, color) in theme::LOGO.into_iter().enumerate() {
        #[expect(clippy::cast_precision_loss, reason = "i is at most 2")]
        let x = rect.min.x + radius + i as f32 * radius * 1.8;
        ui.painter()
            .circle_filled(egui::pos2(x, rect.center().y), radius, color);
    }
    ui.label(
        RichText::new("Cineo")
            .font(theme::logo())
            .color(theme::TEXT_BRIGHT),
    );
}

fn nav_link(ui: &mut Ui, label: &str, selected: bool) -> egui::Response {
    let color = if selected {
        theme::TEXT_BRIGHT
    } else {
        theme::TEXT_DIM
    };
    let response = ui
        .add(Button::new(caps(label, theme::nav(), color)).frame(false))
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    if selected || response.hovered() {
        let rect = response.rect;
        let underline = if selected {
            theme::ACCENT
        } else {
            theme::TEXT_FAINT
        };
        ui.painter().hline(
            rect.x_range(),
            rect.bottom() + 3.0,
            Stroke::new(2.0, underline),
        );
    }
    response
}

// --- pages ---

fn board_page(ui: &mut Ui, state: &State, out: &mut Vec<Action>) {
    let resume = continue_watching(&state.library);
    if !resume.is_empty() {
        section(ui, "Continue watching", |_| {});
        ScrollArea::horizontal().id_salt("continue").show(ui, |ui| {
            poster_row(ui, |ui| {
                for item in resume {
                    library_card(ui, item, out);
                }
            });
        });
        ui.add_space(theme::SECTION_GAP);
    }
    if state.addons.is_empty() && state.addons_loading.is_empty() {
        empty(
            ui,
            "No addons installed. Open Addons and paste an addon's manifest URL.",
        );
    }
    for (index, row) in state.board.iter().enumerate() {
        catalog_row(ui, ("board", index), row, true, out);
    }
}

fn discover_page(ui: &mut Ui, state: &State, out: &mut Vec<Action>) {
    let targets = board_targets(&state.addons);
    let discover = &state.discover;
    section(ui, "Discover", |_| {});
    if targets.is_empty() {
        empty(ui, "No installed addon has a browsable catalog.");
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
    poster_grid(ui, |ui| {
        for item in &discover.items {
            if preview_card(ui, item).clicked() {
                out.push(open_detail(item));
            }
        }
    });
    ui.add_space(theme::GAP);
    if discover.pending.is_some() {
        ui.spinner();
    } else if discover.next_skip.is_some() && ui.button("Load more").clicked() {
        out.push(Action::LoadMoreDiscover);
    }
}

fn search_page(ui: &mut Ui, state: &State, view: &mut ViewState, out: &mut Vec<Action>) {
    section(ui, "Search", |_| {});
    ui.horizontal(|ui| {
        let field = ui.add(
            TextEdit::singleline(&mut view.search_input)
                .hint_text("Movie or series title")
                .margin(Vec2::new(10.0, 6.0))
                .desired_width(360.0),
        );
        let submitted = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if ui.add(primary_button("Search")).clicked() || submitted {
            out.push(Action::Search(view.search_input.clone()));
        }
    });
    ui.add_space(theme::SECTION_GAP);
    if state.search_query.is_empty() {
        return;
    }
    if state.search.is_empty() {
        empty(ui, "No installed addon supports search.");
    }
    for (index, row) in state.search.iter().enumerate() {
        catalog_row(ui, ("search", index), row, false, out);
    }
}

fn library_page(ui: &mut Ui, state: &State, out: &mut Vec<Action>) {
    section(ui, "Library", |ui| {
        if !state.library.is_empty() {
            ui.label(faint(&count_label(state.library.len(), "item", "items")));
        }
    });
    if state.library.is_empty() {
        empty(ui, "Items you play appear here.");
        return;
    }
    let mut items: Vec<&LibraryItem> = state.library.iter().collect();
    items.sort_by_key(|i| std::cmp::Reverse(i.updated_ms));
    poster_grid(ui, |ui| {
        for item in items {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                library_card(ui, item, out);
                if ui
                    .add(
                        Button::new(RichText::new("Remove").small().color(theme::TEXT_FAINT))
                            .frame(false),
                    )
                    .clicked()
                {
                    out.push(Action::RemoveFromLibrary(item.id.clone()));
                }
            });
        }
    });
}

fn addons_page(ui: &mut Ui, state: &State, view: &mut ViewState, out: &mut Vec<Action>) {
    section(ui, "Install an addon", |_| {});
    ui.horizontal(|ui| {
        let field = ui.add(
            TextEdit::singleline(&mut view.addon_input)
                .hint_text("https://…/manifest.json")
                .margin(Vec2::new(10.0, 6.0))
                .desired_width(420.0),
        );
        let submitted = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let busy = state.install.as_ref().is_some_and(Loadable::is_loading);
        if ui.add_enabled(!busy, primary_button("Install")).clicked() || (submitted && !busy) {
            out.push(Action::InstallAddon(view.addon_input.clone()));
        }
    });
    match &state.install {
        Some(Loadable::Loading) => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(dim("Installing…"));
            });
        }
        Some(Loadable::Ready(name)) => {
            ui.label(RichText::new(format!("Installed {name}")).color(theme::ACCENT));
        }
        Some(Loadable::Failed(err)) => {
            ui.label(RichText::new(err).color(theme::DANGER));
        }
        None => {}
    }
    ui.add_space(theme::SECTION_GAP);
    let count = state.addons.len();
    section(ui, "Installed", |ui| {
        if count > 0 {
            ui.label(faint(&count_label(count, "addon", "addons")));
        }
    });
    if count == 0 {
        empty(ui, "Nothing installed yet.");
    }
    for (index, addon) in state.addons.iter().enumerate() {
        let manifest = &addon.manifest;
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{} {}", manifest.name, manifest.version))
                            .color(theme::TEXT_BRIGHT),
                    );
                    // The full URL can carry configuration; show the host only.
                    if let Some(host) = addon.transport.as_url().host_str() {
                        ui.label(faint(host));
                    }
                });
                if let Some(description) = &manifest.description {
                    ui.add(Label::new(dim(description)).wrap());
                }
                let hints = &manifest.behavior_hints;
                if hints.adult || hints.p2p || hints.configuration_required {
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
                }
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add(Button::new(RichText::new("Remove").color(theme::DANGER)))
                    .clicked()
                {
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
        rule(ui);
    }
}

fn detail_page(ui: &mut Ui, detail: &Detail, view: &mut ViewState, out: &mut Vec<Action>) {
    let meta = detail.meta.ready();
    let preview = meta.map(|m| &m.preview).or(detail.preview.as_ref());
    let background = preview.and_then(|p| p.background.as_ref());

    // A full-width backdrop fading into the page; the content overlaps its
    // lower part.
    let height = if background.is_some() {
        theme::BACKDROP_HEIGHT
    } else {
        64.0
    };
    let (backdrop, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
    if let Some(background) = background {
        paint_cover(
            ui,
            background.as_str(),
            backdrop,
            CornerRadius::ZERO,
            theme::BACKDROP_TINT,
            0.3,
        );
        let fade_start = backdrop.min.y + backdrop.height() * 0.3;
        fade(
            ui,
            Rect::from_min_max(egui::pos2(backdrop.min.x, fade_start), backdrop.max),
            false,
        );
        let edge = (backdrop.width() * 0.12).min(160.0);
        fade(
            ui,
            Rect::from_min_size(backdrop.min, Vec2::new(edge, backdrop.height())),
            true,
        );
        let mut right = Rect::from_min_size(
            egui::pos2(backdrop.max.x - edge, backdrop.min.y),
            Vec2::new(edge, backdrop.height()),
        );
        // Mirror the left fade: swap the edges so the dark side is outside.
        std::mem::swap(&mut right.min.x, &mut right.max.x);
        fade(ui, right, true);
    }
    let back = Rect::from_min_size(
        backdrop.min + Vec2::splat(f32::from(theme::PAGE_MARGIN) / 2.0),
        Vec2::new(90.0, 28.0),
    );
    if ui
        .put(
            back,
            Button::new(RichText::new("← Back").color(theme::TEXT_BRIGHT)).fill(theme::SCRIM),
        )
        .clicked()
    {
        out.push(Action::CloseDetail);
        view.season = None;
    }
    if background.is_some() {
        ui.add_space(-theme::BACKDROP_HEIGHT * 0.4);
    }

    column(ui, |ui| {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 32.0;
            let poster_size = Vec2::new(
                theme::DETAIL_POSTER_WIDTH,
                theme::DETAIL_POSTER_WIDTH / 0.675,
            );
            let (poster_rect, _) = ui.allocate_exact_size(poster_size, Sense::hover());
            let name = preview.map_or(detail.id.as_str(), |p| p.name.as_str());
            poster(
                ui,
                poster_rect,
                name,
                preview.and_then(|p| p.poster.as_ref()),
            );

            let width = ui.available_width();
            ui.vertical(|ui| {
                ui.set_width(width);
                about(ui, name, preview, meta);
                match &detail.meta {
                    Loadable::Loading => {
                        ui.spinner();
                    }
                    Loadable::Failed(err) => {
                        ui.label(RichText::new(err).color(theme::DANGER));
                    }
                    Loadable::Ready(meta) if !meta.videos.is_empty() => {
                        ui.add_space(theme::SECTION_GAP);
                        episodes(ui, meta, detail, view, out);
                    }
                    Loadable::Ready(_) => {}
                }
                if detail.selected_video.is_some() {
                    ui.add_space(theme::SECTION_GAP);
                    section(ui, "Streams", |_| {});
                    if detail.streams.is_empty() {
                        empty(ui, "No installed addon provides streams for this item.");
                    }
                    for (index, group) in detail.streams.iter().enumerate() {
                        stream_group(ui, index, group, out);
                    }
                }
            });
        });
    });
}

/// Title, year, credits, facts and description.
fn about(ui: &mut Ui, name: &str, preview: Option<&MetaPreview>, meta: Option<&Meta>) {
    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new(name)
                .font(theme::title())
                .color(theme::TEXT_BRIGHT),
        );
        if let Some(year) = preview.and_then(|p| p.release_info.as_ref()) {
            ui.label(
                RichText::new(year)
                    .font(theme::heading())
                    .color(theme::TEXT_DIM),
            );
        }
    });
    if let Some(m) = meta
        && !m.director.is_empty()
    {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(dim("Directed by"));
            ui.label(RichText::new(m.director.join(", ")).color(theme::TEXT_BRIGHT));
        });
    }
    let mut facts = Vec::new();
    facts.extend(meta.and_then(|m| m.runtime.clone()));
    if let Some(rating) = preview.and_then(|p| p.imdb_rating.as_ref()) {
        facts.push(format!("IMDb {rating}"));
    }
    if !facts.is_empty() {
        ui.label(faint(&facts.join("   ·   ")));
    }
    if let Some(description) = preview.and_then(|p| p.description.as_ref()) {
        ui.add_space(theme::GAP);
        ui.add(Label::new(RichText::new(description).color(theme::TEXT)).wrap());
    }
    if let Some(p) = preview
        && !p.genres.is_empty()
    {
        ui.add_space(theme::GAP);
        pills(ui, &p.genres);
    }
    if let Some(m) = meta
        && !m.cast.is_empty()
    {
        ui.add_space(theme::SECTION_GAP);
        section(ui, "Cast", |_| {});
        pills(ui, &m.cast);
    }
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
    section(ui, "Episodes", |ui| {
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
    });
    for video in meta
        .videos
        .iter()
        .filter(|v| season.is_none() || v.season == season)
    {
        let number = match (video.season, video.episode) {
            (Some(_), Some(e)) => Some(e.to_string()),
            _ => None,
        };
        let selected = detail.selected_video.as_deref() == Some(video.id.as_str());
        if list_row(ui, number.as_deref(), &video.title, selected).clicked() && !selected {
            out.push(Action::SelectVideo(video.id.clone()));
        }
    }
}

/// A full-width clickable row with an optional leading number, separated by
/// a rule. Accessible as a button labelled with `text`.
fn list_row(ui: &mut Ui, number: Option<&str>, text: &str, selected: bool) -> egui::Response {
    let height = 36.0;
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), selected, text)
    });
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if response.hovered() {
            painter.rect_filled(rect, 0.0, theme::PANEL);
        }
        if selected {
            painter.rect_filled(
                Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height())),
                0.0,
                theme::ACCENT,
            );
        }
        let color = if selected || response.hovered() {
            theme::TEXT_BRIGHT
        } else {
            theme::TEXT
        };
        let mut x = rect.min.x + 12.0;
        if let Some(number) = number {
            painter.text(
                egui::pos2(x, rect.center().y),
                egui::Align2::LEFT_CENTER,
                number,
                theme::nav(),
                theme::TEXT_FAINT,
            );
            x += 36.0;
        }
        let mut job = LayoutJob::simple_singleline(text.to_owned(), theme::body(), color);
        job.wrap.max_width = rect.max.x - x - 8.0;
        job.wrap.max_rows = 1;
        let galley = painter.layout_job(job);
        let y = rect.center().y - galley.size().y / 2.0;
        painter.galley(egui::pos2(x, y), galley, color);
        painter.hline(
            rect.x_range(),
            rect.bottom() - 0.5,
            Stroke::new(1.0, theme::RULE),
        );
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn stream_group(ui: &mut Ui, index: usize, group: &StreamGroup, out: &mut Vec<Action>) {
    ui.add_space(6.0);
    ui.label(RichText::new(&group.addon_name).color(theme::TEXT_BRIGHT));
    match &group.streams {
        Loadable::Loading => {
            ui.spinner();
        }
        Loadable::Failed(err) => {
            ui.label(RichText::new(err).color(theme::DANGER));
        }
        Loadable::Ready(streams) if streams.is_empty() => {
            ui.label(faint("No streams"));
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
    ui.add_space(theme::GAP);
}

/// One stream; returns true if Play was clicked.
fn stream_row(ui: &mut Ui, stream: &Stream) -> bool {
    let mut clicked = false;
    Frame::new()
        .inner_margin(Margin::symmetric(0, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                // Torrents need the streaming engine, which the shell does
                // not run yet.
                let playable = stream.source.is_playable() && !stream.source.is_p2p();
                let play = if playable {
                    primary_button("Play")
                } else {
                    Button::new(RichText::new("Play").color(theme::TEXT_FAINT))
                };
                let play = ui
                    .add_enabled(playable, play.min_size(Vec2::new(64.0, 30.0)))
                    .on_disabled_hover_text(format!(
                        "{} streams are not supported yet",
                        stream.source.kind_label()
                    ));
                clicked = play.clicked();
                ui.add_space(4.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 4.0;
                    ui.horizontal(|ui| {
                        if let Some(name) = &stream.name {
                            ui.label(RichText::new(name).color(theme::TEXT_BRIGHT));
                        }
                        badge(ui, stream.source.kind_label(), theme::TEXT_DIM);
                    });
                    if let Some(description) = &stream.description {
                        ui.add(Label::new(dim(description)).wrap());
                    }
                });
            });
        });
    rule(ui);
    clicked
}

// --- building blocks ---

/// Lays `add` out in a centered column at most
/// [`theme::CONTENT_MAX_WIDTH`] wide, with the page margin on narrow windows.
fn column<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let available = ui.available_rect_before_wrap();
    let margin = f32::from(theme::PAGE_MARGIN);
    let width = (available.width() - 2.0 * margin).clamp(0.0, theme::CONTENT_MAX_WIDTH);
    let left = available.min.x + (available.width() - width) / 2.0;
    let rect = Rect::from_min_max(
        egui::pos2(left, available.min.y),
        egui::pos2(left + width, available.max.y),
    );
    ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
        ui.set_width(width);
        add(ui)
    })
    .inner
}

/// A small uppercase heading over a thin rule, with `right` laid out at its
/// right end.
fn section(ui: &mut Ui, title: &str, right: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.label(caps(title, theme::section(), theme::TEXT_DIM));
        ui.with_layout(Layout::right_to_left(Align::Center), right);
    });
    ui.add_space(-4.0);
    rule(ui);
    ui.add_space(4.0);
}

fn rule(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        Stroke::new(1.0, theme::RULE),
    );
}

fn catalog_row(ui: &mut Ui, salt: (&str, usize), row: &Row, see_all: bool, out: &mut Vec<Action>) {
    section(ui, &row.target.title, |ui| {
        if see_all
            && ui
                .add(Button::new(caps("See all", theme::caption(), theme::TEXT_DIM)).frame(false))
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
        {
            out.push(Action::OpenDiscover {
                addon: row.target.addon.clone(),
                path: row.target.path.clone(),
            });
        }
        ui.label(faint(&row.target.addon_name));
    });
    match &row.items {
        Loadable::Loading => {
            ui.spinner();
        }
        Loadable::Failed(err) => {
            ui.label(RichText::new(err).color(theme::DANGER));
        }
        Loadable::Ready(items) if items.is_empty() => {
            ui.label(faint("Nothing here"));
        }
        Loadable::Ready(items) => {
            ScrollArea::horizontal().id_salt(salt).show(ui, |ui| {
                poster_row(ui, |ui| {
                    for item in items {
                        if preview_card(ui, item).clicked() {
                            out.push(open_detail(item));
                        }
                    }
                });
            });
        }
    }
    ui.add_space(theme::SECTION_GAP);
}

fn poster_row(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme::CARD_GAP;
        add(ui);
    });
}

fn poster_grid(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::splat(theme::CARD_GAP);
        add(ui);
    });
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

/// A clickable poster with an optional progress bar. The title shows on
/// hover. Accessible as a button labelled with the title.
fn card(
    ui: &mut Ui,
    name: &str,
    poster_url: Option<&Url>,
    shape: PosterShape,
    progress: Option<f32>,
) -> egui::Response {
    let size = card_image_size(shape);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), name));
    if !ui.is_rect_visible(rect) {
        return response;
    }
    poster(ui, rect, name, poster_url);
    let painter = ui.painter();
    if let Some(progress) = progress {
        let bar = Rect::from_min_max(egui::pos2(rect.min.x, rect.max.y - 4.0), rect.max);
        painter.rect_filled(bar, 0.0, theme::SCRIM);
        let mut done = bar;
        done.set_width(bar.width() * progress.clamp(0.0, 1.0));
        painter.rect_filled(done, 0.0, theme::ACCENT);
    }
    if response.hovered() {
        painter.rect_stroke(
            rect,
            CornerRadius::same(theme::POSTER_RADIUS),
            Stroke::new(3.0, theme::ACCENT),
            StrokeKind::Inside,
        );
    }
    let response = if poster_url.is_some() {
        response.on_hover_text(name)
    } else {
        response
    };
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A poster image cropped to `rect`, or the title on a plain card when
/// there is none, with the faint poster outline.
fn poster(ui: &Ui, rect: Rect, name: &str, url: Option<&Url>) {
    let radius = CornerRadius::same(theme::POSTER_RADIUS);
    let painter = ui.painter();
    painter.rect_filled(rect, radius, theme::SURFACE);
    if let Some(url) = url {
        paint_cover(ui, url.as_str(), rect, radius, Color32::WHITE, 0.5);
    } else {
        let mut job = LayoutJob::simple(
            name.to_owned(),
            theme::caption(),
            theme::TEXT_DIM,
            rect.width() - 16.0,
        );
        job.halign = Align::Center;
        job.wrap.max_rows = 4;
        let galley = painter.layout_job(job);
        let pos = egui::pos2(rect.center().x, rect.center().y - galley.size().y / 2.0);
        painter.galley(pos, galley, theme::TEXT_DIM);
    }
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(1.0, theme::POSTER_OUTLINE),
        StrokeKind::Inside,
    );
}

/// Paints the image at `src` filling `rect`, cropping instead of
/// stretching. `focus_y` (0 = top, 1 = bottom) picks which part of a tall
/// image stays visible.
fn paint_cover(ui: &Ui, src: &str, rect: Rect, radius: CornerRadius, tint: Color32, focus_y: f32) {
    let image = Image::new(src)
        .corner_radius(radius)
        .tint(tint)
        .show_loading_spinner(false);
    let uv = image
        .load_for_size(ui.ctx(), rect.size())
        .ok()
        .and_then(|poll| poll.size())
        .map_or(
            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            |size| cover_uv(size, rect.size(), focus_y),
        );
    image.uv(uv).paint_at(ui, rect);
}

/// The part of an image of `image` size to show in `target` so it covers
/// the target without distortion.
fn cover_uv(image: Vec2, target: Vec2, focus_y: f32) -> Rect {
    let full = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    if image.x <= 0.0 || image.y <= 0.0 || target.x <= 0.0 || target.y <= 0.0 {
        return full;
    }
    let image_aspect = image.x / image.y;
    let target_aspect = target.x / target.y;
    if image_aspect > target_aspect {
        let visible = target_aspect / image_aspect;
        let left = (1.0 - visible) / 2.0;
        Rect::from_min_max(egui::pos2(left, 0.0), egui::pos2(left + visible, 1.0))
    } else {
        let visible = image_aspect / target_aspect;
        let top = (1.0 - visible) * focus_y.clamp(0.0, 1.0);
        Rect::from_min_max(egui::pos2(0.0, top), egui::pos2(1.0, top + visible))
    }
}

/// Paints a gradient over `rect` from transparent to the page background:
/// top to bottom, or (if `horizontal`) right to left, so the min edge is
/// fully covered. A rect with swapped x edges mirrors a horizontal fade.
fn fade(ui: &Ui, rect: Rect, horizontal: bool) {
    let clear = Color32::TRANSPARENT;
    let solid = theme::BG;
    let (tl, tr, br, bl) = if horizontal {
        (solid, clear, clear, solid)
    } else {
        (clear, clear, solid, solid)
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

fn card_image_size(shape: PosterShape) -> Vec2 {
    let w = theme::CARD_WIDTH;
    match shape {
        PosterShape::Poster => Vec2::new(w, w / 0.675),
        PosterShape::Square => Vec2::new(w, w),
        PosterShape::Landscape => Vec2::new(w * 1.77, w),
    }
}

/// The primary action button: green with dark text.
fn primary_button(text: &str) -> Button<'_> {
    Button::new(RichText::new(text).color(theme::ON_ACCENT)).fill(theme::ACCENT)
}

fn badge(ui: &mut Ui, text: &str, color: Color32) {
    Frame::new()
        .fill(theme::SURFACE)
        .corner_radius(CornerRadius::same(theme::RADIUS))
        .inner_margin(Margin::symmetric(6, 1))
        .show(ui, |ui| {
            ui.label(RichText::new(text).small().color(color));
        });
}

/// Tag-like boxes, as genres and cast are shown.
fn pills(ui: &mut Ui, items: &[String]) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::splat(6.0);
        for item in items {
            Frame::new()
                .fill(theme::SURFACE)
                .corner_radius(CornerRadius::same(theme::RADIUS))
                .inner_margin(Margin::symmetric(8, 3))
                .show(ui, |ui| {
                    ui.label(RichText::new(item).small().color(theme::TEXT_DIM));
                });
        }
    });
}

/// Uppercase letter-spaced text.
fn caps(text: &str, font: FontId, color: Color32) -> LayoutJob {
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

fn empty(ui: &mut Ui, text: &str) {
    ui.add_space(theme::GAP);
    ui.label(dim(text));
    ui.add_space(theme::GAP);
}

fn dim(text: &str) -> RichText {
    RichText::new(text).color(theme::TEXT_DIM)
}

fn faint(text: &str) -> RichText {
    RichText::new(text).small().color(theme::TEXT_FAINT)
}

fn count_label(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cover_crops_a_wide_image_at_the_sides() {
        let uv = cover_uv(Vec2::new(200.0, 100.0), Vec2::new(100.0, 100.0), 0.5);
        assert_eq!(
            uv,
            Rect::from_min_max(egui::pos2(0.25, 0.0), egui::pos2(0.75, 1.0))
        );
    }

    #[test]
    fn cover_crops_a_tall_image_around_the_focus() {
        let uv = cover_uv(Vec2::new(100.0, 200.0), Vec2::new(100.0, 100.0), 0.0);
        assert_eq!(
            uv,
            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 0.5))
        );
        let uv = cover_uv(Vec2::new(100.0, 200.0), Vec2::new(100.0, 100.0), 1.0);
        assert_eq!(
            uv,
            Rect::from_min_max(egui::pos2(0.0, 0.5), egui::pos2(1.0, 1.0))
        );
    }

    #[test]
    fn cover_shows_everything_for_an_unknown_size() {
        let uv = cover_uv(Vec2::ZERO, Vec2::new(100.0, 100.0), 0.5);
        assert_eq!(
            uv,
            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0))
        );
    }
}
