//! Rendering. [`show`] draws a [`State`] snapshot and returns the
//! [`Action`]s the user triggered; it performs no IO and holds no business
//! rules (ADR-0001, ADR-0011). Addon strings are shown as plain text only.

use std::time::Duration;

use cineo_core::addon::{Meta, MetaPreview, PosterShape, Stream};
use cineo_core::app::{
    Action, CatalogTarget, Detail, LibraryItem, Loadable, Row, State, StreamGroup, TorrentStatus,
    board_targets, continue_watching,
};
use eframe::egui::{
    self, Align, Align2, Button, Color32, ComboBox, CornerRadius, FontId, Frame, Image, Key, Label,
    Layout, Margin, Mesh, Modal, Modifiers, Pos2, Rect, Response, RichText, ScrollArea, Sense,
    Stroke, StrokeKind, TextEdit, TextFormat, Ui, UiBuilder, Vec2, WidgetInfo, WidgetType, pos2,
    scroll_area::ScrollBarVisibility, style::ScrollAnimation, text::LayoutJob, vec2,
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
    Settings,
}

impl Page {
    const ALL: [Self; 6] = [
        Self::Board,
        Self::Discover,
        Self::Search,
        Self::Library,
        Self::Addons,
        Self::Settings,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Board => "Home",
            Self::Discover => "Discover",
            Self::Search => "Search",
            Self::Library => "Library",
            Self::Addons => "Addons",
            Self::Settings => "Settings",
        }
    }

    fn icon(self) -> Icon {
        match self {
            Self::Board => Icon::Home,
            Self::Discover => Icon::Compass,
            Self::Search => Icon::Search,
            Self::Library => Icon::Library,
            Self::Addons => Icon::Addons,
            Self::Settings => Icon::Settings,
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
    /// When the search text last changed (egui time, seconds); a search
    /// runs once typing pauses.
    pub search_edited_at: Option<f64>,
    /// Focus the search field on the next frame.
    pub focus_search: bool,
}

/// Seconds of no typing before a search runs.
const SEARCH_DEBOUNCE: f64 = 0.45;
/// Draws the whole window and returns the actions to dispatch.
pub fn show(ui: &mut Ui, state: &State, view: &mut ViewState) -> Vec<Action> {
    let mut out = Vec::new();
    if theme::ensure(ui.ctx()) {
        // Text laid out now would not find the theme's fonts yet.
        ui.ctx().request_discard("theme applied");
        return out;
    }
    shortcuts(ui, state, view, &mut out);
    sidebar(ui, state, view, &mut out);
    if let Some(notice) = &state.notice {
        notice_bar(ui, notice, &mut out);
    }
    if let Some(torrent) = &state.torrent {
        torrent_bar(ui, &torrent.status);
    }
    if state.p2p_prompt.is_some() {
        p2p_prompt(ui, &mut out);
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
                        detail_page(ui, detail, state.settings.p2p_enabled, view, &mut out);
                    } else {
                        ui.add_space(page_margin(ui));
                        column(ui, |ui| match view.page {
                            Page::Board => board_page(ui, state, view, &mut out),
                            Page::Discover => discover_page(ui, state, &mut out),
                            Page::Search => search_page(ui, state, view, &mut out),
                            Page::Library => library_page(ui, state, &mut out),
                            Page::Addons => addons_page(ui, state, view, &mut out),
                            Page::Settings => settings_page(ui, state, &mut out),
                        });
                    }
                    ui.add_space(page_margin(ui));
                });
        });
    out
}

/// Opens `page`, leaving the detail page if one is open.
fn go(page: Page, state: &State, view: &mut ViewState, out: &mut Vec<Action>) {
    if state.detail.is_some() {
        out.push(Action::CloseDetail);
        view.season = None;
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
    if page == Page::Search {
        view.focus_search = true;
    }
    view.page = page;
}

/// Keyboard and mouse shortcuts: Ctrl+F or `/` searches, Ctrl+1…6 switch
/// pages, Escape, Alt+Left or the mouse back button leave a detail page.
fn shortcuts(ui: &Ui, state: &State, view: &mut ViewState, out: &mut Vec<Action>) {
    let typing = ui.ctx().egui_wants_keyboard_input();
    let modal = state.p2p_prompt.is_some();
    let in_detail = state.detail.is_some();
    let (search, page, back) = ui.input_mut(|i| {
        let search = i.consume_key(Modifiers::COMMAND, Key::F)
            || (!typing && i.consume_key(Modifiers::NONE, Key::Slash));
        let keys = [
            Key::Num1,
            Key::Num2,
            Key::Num3,
            Key::Num4,
            Key::Num5,
            Key::Num6,
        ];
        let page = keys
            .iter()
            .position(|key| i.consume_key(Modifiers::COMMAND, *key))
            .map(|index| Page::ALL[index]);
        let back = in_detail
            && !modal
            && (i.consume_key(Modifiers::ALT, Key::ArrowLeft)
                || i.pointer.button_pressed(egui::PointerButton::Extra1)
                || (!typing && i.consume_key(Modifiers::NONE, Key::Escape)));
        (search, page, back)
    });
    if modal {
        return;
    }
    if search {
        go(Page::Search, state, view, out);
    } else if let Some(page) = page {
        go(page, state, view, out);
    } else if back {
        out.push(Action::CloseDetail);
        view.season = None;
    }
}

fn sidebar(ui: &mut Ui, state: &State, view: &mut ViewState, out: &mut Vec<Action>) {
    let compact = ui.ctx().content_rect().width() < theme::SIDEBAR_COMPACT_BELOW;
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
                .inner_margin(Margin::symmetric(12, 22)),
        )
        .show(ui, |ui| {
            let edge = ui.max_rect().right() + 11.5;
            ui.painter().vline(
                edge,
                ui.max_rect().y_range().expand(22.0),
                Stroke::new(1.0, theme::RULE),
            );
            logo(ui, compact);
            ui.add_space(32.0);
            ui.spacing_mut().item_spacing.y = 2.0;
            for page in Page::ALL {
                let selected = view.page == page && state.detail.is_none();
                if nav_item(ui, page, selected, compact).clicked() {
                    go(page, state, view, out);
                }
            }
            if !state.addons_loading.is_empty() {
                ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        spinner(ui);
                        if !compact {
                            ui.label(faint("Loading addons…"));
                        }
                    });
                });
            }
        });
}

/// The mark (an amber crescent "C", a lens catching light) and the serif
/// wordmark.
fn logo(ui: &mut Ui, compact: bool) {
    ui.horizontal(|ui| {
        if !compact {
            ui.add_space(8.0);
        }
        let size = 22.0;
        let slot = if compact {
            vec2(ui.available_width(), 34.0)
        } else {
            vec2(size, 34.0)
        };
        let (slot, _) = ui.allocate_exact_size(slot, Sense::hover());
        let c = slot.center();
        let painter = ui.painter();
        painter.circle_filled(c, size / 2.0, theme::ACCENT);
        painter.circle_filled(c + vec2(4.5, 0.0), size * 0.3, theme::SIDEBAR);
        painter.circle_filled(c + vec2(4.5, 0.0), 2.0, theme::ACCENT);
        if !compact {
            ui.add_space(2.0);
            ui.label(
                RichText::new("Cineo")
                    .font(theme::logo())
                    .color(theme::TEXT_BRIGHT),
            );
        }
    });
}

/// A sidebar entry: icon and small-caps label, or the icon alone when
/// `compact`. Accessible as a button labelled with the page name.
fn nav_item(ui: &mut Ui, page: Page, selected: bool, compact: bool) -> Response {
    let label = page.label();
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::click());
    response.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, selected, label));
    if ui.is_rect_visible(rect) {
        let hover = ui
            .ctx()
            .animate_bool_with_time(response.id, response.hovered(), theme::ANIM);
        let painter = ui.painter();
        if selected {
            painter.rect_filled(
                Rect::from_min_size(pos2(rect.min.x - 12.0, rect.min.y + 8.0), vec2(3.0, 22.0)),
                0.0,
                theme::ACCENT,
            );
        }
        let color = if selected {
            theme::TEXT_BRIGHT
        } else {
            lerp_color(theme::TEXT_DIM, theme::TEXT_BRIGHT, hover)
        };
        let icon_color = if selected { theme::ACCENT } else { color };
        let icon_center = if compact {
            rect.center()
        } else {
            pos2(rect.min.x + 18.0, rect.center().y)
        };
        paint_icon(painter, page.icon(), icon_center, 19.0, icon_color);
        if !compact {
            let galley = painter.layout_job(caps(label, theme::nav(), color));
            painter.galley(
                pos2(rect.min.x + 40.0, rect.center().y - galley.size().y / 2.0),
                galley,
                color,
            );
        }
    }
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    if compact {
        response.on_hover_text(label)
    } else {
        response
    }
}

fn notice_bar(ui: &mut Ui, notice: &str, out: &mut Vec<Action>) {
    egui::Panel::top("notice")
        .show_separator_line(false)
        .frame(
            Frame::new()
                .fill(theme::PANEL)
                .inner_margin(Margin::symmetric(0, 10)),
        )
        .show(ui, |ui| {
            let full = ui.max_rect();
            ui.painter().hline(
                full.x_range(),
                full.bottom() + 9.5,
                Stroke::new(1.0, theme::WARNING),
            );
            column(ui, |ui| {
                ui.horizontal(|ui| {
                    let (slot, _) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
                    paint_icon(
                        ui.painter(),
                        Icon::Alert,
                        slot.center(),
                        18.0,
                        theme::WARNING,
                    );
                    ui.label(RichText::new(notice).color(theme::TEXT_BRIGHT));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("Dismiss").clicked() {
                            out.push(Action::DismissNotice);
                        }
                    });
                });
            });
        });
}

fn torrent_bar(ui: &mut Ui, status: &TorrentStatus) {
    egui::Panel::bottom("torrent")
        .show_separator_line(false)
        .frame(
            Frame::new()
                .fill(theme::PANEL)
                .inner_margin(Margin::symmetric(0, 10)),
        )
        .show(ui, |ui| {
            let full = ui.max_rect();
            let track = Rect::from_min_size(full.min - vec2(0.0, 10.0), vec2(full.width(), 2.0));
            let painter = ui.painter();
            painter.rect_filled(track, 0.0, theme::SURFACE);
            if let TorrentStatus::Streaming {
                downloaded, size, ..
            } = status
            {
                #[expect(clippy::cast_precision_loss, reason = "display only")]
                let done = if *size == 0 {
                    1.0
                } else {
                    *downloaded as f32 / *size as f32
                };
                let mut bar = track;
                bar.set_width(track.width() * done.clamp(0.0, 1.0));
                painter.rect_filled(bar, 0.0, theme::ACCENT);
            }
            column(ui, |ui| {
                ui.horizontal(|ui| {
                    if *status == TorrentStatus::Starting {
                        spinner(ui);
                    }
                    ui.label(dim(&torrent_status_text(status)));
                });
            });
        });
}

/// The notice shown before the first torrent plays (ADR-0012).
fn p2p_prompt(ui: &mut Ui, out: &mut Vec<Action>) {
    let modal = Modal::new(egui::Id::new("p2p_prompt"))
        .frame(
            Frame::new()
                .fill(theme::PANEL)
                .stroke(Stroke::new(1.0, theme::RULE))
                .corner_radius(CornerRadius::same(theme::RADIUS))
                .inner_margin(Margin::same(28)),
        )
        .show(ui.ctx(), |ui| {
            ui.set_max_width(480.0);
            ui.label(RichText::new("Peer-to-peer streaming").font(theme::heading()));
            ui.add_space(theme::GAP);
            ui.add(Label::new(dim(P2P_NOTICE)).wrap());
            ui.add_space(theme::GAP * 1.5);
            ui.horizontal(|ui| {
                if primary(ui, true, "Accept and play").clicked() {
                    out.push(Action::AcceptP2p);
                }
                if ui.button("Cancel").clicked() {
                    out.push(Action::DeclineP2p);
                }
            });
        });
    if modal.should_close() && out.is_empty() {
        out.push(Action::DeclineP2p);
    }
}

const P2P_NOTICE: &str = "Torrent streams come from other people's computers. While one \
plays, your IP address is visible to the peers and trackers it connects to, and Cineo \
uploads the parts it has already downloaded to those peers. Downloaded data is kept in \
a local cache. You can turn peer-to-peer streaming off in Settings.";

fn board_page(ui: &mut Ui, state: &State, view: &mut ViewState, out: &mut Vec<Action>) {
    let resume = continue_watching(&state.library);
    if !resume.is_empty() {
        section(ui, "Continue watching", |_| {});
        poster_strip(ui, "continue", |ui| {
            for item in resume {
                library_card(ui, theme::CARD_WIDTH, item, out);
            }
        });
        ui.add_space(theme::SECTION_GAP);
    }
    if state.installed.is_empty() {
        page_title(ui, "The projector is warm.", Some("The reels are missing."));
        empty(
            ui,
            "No addons installed. Open Addons and paste an addon's manifest URL.",
        );
        if primary(ui, true, "Open Addons").clicked() {
            go(Page::Addons, state, view, out);
        }
    }
    for (index, row) in state.board.iter().enumerate() {
        catalog_row(ui, ("board", index), row, Some(&mut *view), out);
    }
}

fn discover_page(ui: &mut Ui, state: &State, out: &mut Vec<Action>) {
    let targets = board_targets(&state.addons);
    let discover = &state.discover;
    page_title(
        ui,
        "Discover",
        Some("Browse a catalog, narrow it by genre."),
    );
    if targets.is_empty() {
        empty(ui, "No installed addon has a browsable catalog.");
        return;
    }
    let several_addons = targets.iter().any(|t| t.addon != targets[0].addon);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::splat(6.0);
        for target in &targets {
            let selected = discover.target.as_ref() == Some(target);
            let label = if several_addons {
                target_label(target)
            } else {
                target.title.clone()
            };
            if chip(ui, &label, selected).clicked() && !selected {
                out.push(Action::OpenDiscover {
                    addon: target.addon.clone(),
                    path: target.path.clone(),
                });
            }
        }
    });
    if let Some((options, required)) = genre_options(state) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let current = discover.genre.as_deref().unwrap_or("All genres");
            ComboBox::from_id_salt("genre")
                .icon(|ui, rect, visuals, _open| {
                    paint_icon(
                        ui.painter(),
                        Icon::ChevronDown,
                        rect.center(),
                        16.0,
                        visuals.fg_stroke.color,
                    );
                })
                .selected_text(current)
                .width(200.0)
                .height(420.0)
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
        });
    }
    ui.add_space(theme::GAP * 2.0);
    if let Some(error) = &discover.error {
        ui.label(RichText::new(error).color(theme::DANGER));
    }
    let width = grid_card_width(ui.available_width());
    poster_grid(ui, |ui| {
        for item in &discover.items {
            if preview_card(ui, width, item).clicked() {
                out.push(open_detail(item));
            }
        }
    });
    ui.add_space(theme::GAP);
    // Infinite scroll: the next page loads as soon as the end comes into view.
    let (end, _) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::hover());
    if discover.pending.is_some() {
        paint_spinner(ui, end.center(), 22.0);
    } else if discover.next_skip.is_some() && ui.is_rect_visible(end) {
        out.push(Action::LoadMoreDiscover);
    }
}

fn search_page(ui: &mut Ui, state: &State, view: &mut ViewState, out: &mut Vec<Action>) {
    page_title(ui, "Search", None);
    let width = ui.available_width().min(640.0);
    let field = ui.add(
        TextEdit::singleline(&mut view.search_input)
            .id_salt("search")
            .hint_text("A film, a series, a guilty pleasure…")
            .font(FontId::new(17.0, egui::FontFamily::Proportional))
            .margin(Margin {
                left: 40,
                right: 12,
                top: 11,
                bottom: 11,
            })
            .desired_width(width),
    );
    paint_icon(
        ui.painter(),
        Icon::Search,
        pos2(field.rect.min.x + 20.0, field.rect.center().y),
        18.0,
        if field.has_focus() {
            theme::ACCENT
        } else {
            theme::TEXT_DIM
        },
    );
    if field.has_focus() {
        ui.painter().rect_stroke(
            field.rect,
            CornerRadius::same(theme::RADIUS),
            Stroke::new(1.0, theme::ACCENT),
            StrokeKind::Inside,
        );
    }
    if view.focus_search {
        field.request_focus();
        view.focus_search = false;
    }
    // Search as you type, once typing pauses; Enter searches right away.
    let now = ui.input(|i| i.time);
    if field.changed() {
        view.search_edited_at = Some(now);
    }
    let submitted = field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    let due = view
        .search_edited_at
        .is_some_and(|edited| now - edited >= SEARCH_DEBOUNCE);
    if submitted || (due && view.search_input.trim() != state.search_query) {
        view.search_edited_at = None;
        out.push(Action::Search(view.search_input.clone()));
    } else if due {
        view.search_edited_at = None;
    } else if let Some(edited) = view.search_edited_at {
        ui.ctx()
            .request_repaint_after(Duration::from_secs_f64(SEARCH_DEBOUNCE - (now - edited)));
    }
    ui.add_space(theme::SECTION_GAP);
    if state.search_query.is_empty() {
        empty(
            ui,
            "Type a title. Every installed addon that supports search is asked.",
        );
        return;
    }
    if state.search.is_empty() {
        empty(ui, "No installed addon supports search.");
    }
    for (index, row) in state.search.iter().enumerate() {
        catalog_row(ui, ("search", index), row, None, out);
    }
}

fn library_page(ui: &mut Ui, state: &State, out: &mut Vec<Action>) {
    page_title(
        ui,
        "Library",
        Some("Everything you've pressed play on, most recent first."),
    );
    section(ui, "Your films", |ui| {
        if !state.library.is_empty() {
            ui.label(caps_text(
                &count_label(state.library.len(), "film", "films"),
                theme::TEXT_FAINT,
            ));
        }
    });
    if state.library.is_empty() {
        empty(
            ui,
            "Items you play appear here. Every collection starts with a single film.",
        );
        return;
    }
    let mut items: Vec<&LibraryItem> = state.library.iter().collect();
    items.sort_by_key(|i| std::cmp::Reverse(i.updated_ms));
    let width = grid_card_width(ui.available_width());
    poster_grid(ui, |ui| {
        for item in items {
            ui.vertical(|ui| {
                ui.set_width(width);
                ui.spacing_mut().item_spacing.y = 0.0;
                library_card(ui, width, item, out);
                if ui
                    .add(
                        Button::new(caps("Remove", theme::caption(), theme::TEXT_FAINT))
                            .frame(false),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    out.push(Action::RemoveFromLibrary(item.id.clone()));
                }
            });
        }
    });
}

fn addons_page(ui: &mut Ui, state: &State, view: &mut ViewState, out: &mut Vec<Action>) {
    page_title(
        ui,
        "Addons",
        Some("Where the films come from. Order matters: earlier addons win ties."),
    );
    section(ui, "Install an addon", |_| {});
    ui.horizontal(|ui| {
        let busy = state.install.as_ref().is_some_and(Loadable::is_loading);
        let field_width = (ui.available_width() - 110.0).clamp(160.0, 560.0);
        let field = ui.add(
            TextEdit::singleline(&mut view.addon_input)
                .hint_text("https://…/manifest.json")
                .margin(Margin::symmetric(10, 8))
                .desired_width(field_width),
        );
        let submitted = field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
        if primary(ui, !busy, "Install").clicked() || (submitted && !busy) {
            out.push(Action::InstallAddon(view.addon_input.clone()));
        }
    });
    match &state.install {
        Some(Loadable::Loading) => {
            ui.horizontal(|ui| {
                spinner(ui);
                ui.label(dim("Installing…"));
            });
        }
        Some(Loadable::Ready(name)) => {
            ui.label(RichText::new(format!("Installed {name}")).color(theme::SUCCESS));
        }
        Some(Loadable::Failed(err)) => {
            ui.label(RichText::new(err).color(theme::DANGER));
        }
        None => {}
    }
    ui.add_space(theme::SECTION_GAP);
    let count = state.addons.len();
    let unavailable = state.unavailable_addons();
    section(ui, "Installed", |ui| {
        if count > 0 {
            ui.label(caps_text(
                &count_label(count, "addon", "addons"),
                theme::TEXT_FAINT,
            ));
        }
    });
    if state.installed.is_empty() {
        empty(ui, "Nothing installed yet.");
    }
    for (index, addon) in state.addons.iter().enumerate() {
        let manifest = &addon.manifest;
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let actions_width = 250.0;
            ui.vertical(|ui| {
                ui.set_max_width((ui.available_width() - actions_width).max(200.0));
                ui.spacing_mut().item_spacing.y = 4.0;
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{} {}", manifest.name, manifest.version))
                            .font(theme::strong())
                            .color(theme::TEXT_BRIGHT),
                    );
                    // The full URL can carry configuration; show the host only.
                    if let Some(host) = addon.transport.as_url().host_str() {
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
        ui.add_space(6.0);
        rule(ui);
    }
    for transport in unavailable {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let url = transport.as_url();
            ui.label(
                RichText::new(url.host_str().unwrap_or("Addon"))
                    .font(theme::strong())
                    .color(theme::TEXT_BRIGHT),
            );
            badge(ui, "Could not load", theme::DANGER);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add(Button::new(RichText::new("Remove").color(theme::DANGER)))
                    .clicked()
                {
                    out.push(Action::RemoveAddon(transport.clone()));
                }
                if ui.button("Retry").clicked() {
                    out.push(Action::InstallAddon(url.as_str().to_owned()));
                }
            });
        });
        ui.add_space(6.0);
        rule(ui);
    }
}

fn settings_page(ui: &mut Ui, state: &State, out: &mut Vec<Action>) {
    page_title(ui, "Settings", None);
    section(ui, "Peer-to-peer", |_| {});
    let enabled = state.settings.p2p_enabled;
    if toggle_row(ui, enabled, "Show and play torrent streams").clicked() {
        out.push(Action::SetP2pEnabled(!enabled));
    }
    ui.scope(|ui| {
        ui.set_max_width(ui.available_width().min(720.0));
        ui.add(Label::new(dim(P2P_NOTICE)).wrap());
    });
    ui.add_space(theme::SECTION_GAP);
    section(ui, "Keyboard", |_| {});
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
    ui.add_space(theme::SECTION_GAP);
    section(ui, "About", |_| {});
    ui.label(dim(&format!(
        "Cineo {}. Made by people who stay for the credits.",
        env!("CARGO_PKG_VERSION")
    )));
}

fn torrent_status_text(status: &TorrentStatus) -> String {
    match status {
        TorrentStatus::Starting => "Torrent: looking for peers…".to_owned(),
        TorrentStatus::Streaming {
            peers,
            download_bytes_per_sec,
            downloaded,
            size,
        } => {
            let percent = if *size == 0 {
                100
            } else {
                downloaded.saturating_mul(100) / size
            };
            format!(
                "Torrent: {} · {}/s · {percent}% of {}",
                count_label(
                    usize::try_from(*peers).unwrap_or(usize::MAX),
                    "peer",
                    "peers"
                ),
                format_bytes(*download_bytes_per_sec),
                format_bytes(*size),
            )
        }
    }
}

/// `1.5 GiB`-style sizes.
fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut unit = 0;
    let mut whole = bytes;
    while whole >= 1024 && unit + 1 < UNITS.len() {
        whole /= 1024;
        unit += 1;
    }
    if unit == 0 {
        return format!("{bytes} B");
    }
    #[expect(clippy::cast_precision_loss, reason = "display only")]
    let value = bytes as f64 / 1024f64.powi(i32::try_from(unit).unwrap_or(0));
    format!("{value:.1} {}", UNITS[unit])
}

fn detail_page(
    ui: &mut Ui,
    detail: &Detail,
    p2p_enabled: bool,
    view: &mut ViewState,
    out: &mut Vec<Action>,
) {
    let meta = detail.meta.ready();
    let preview = meta.map(|m| &m.preview).or(detail.preview.as_ref());
    let background = preview.and_then(|p| p.background.as_ref());

    // A full-width backdrop fading into the page; the content overlaps its
    // lower part.
    let height = if background.is_some() {
        theme::BACKDROP_HEIGHT
    } else {
        72.0
    };
    let (backdrop, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    if let Some(background) = background {
        paint_cover(
            ui,
            background.as_str(),
            backdrop,
            CornerRadius::ZERO,
            theme::BACKDROP_TINT,
            0.25,
        );
        let fade_start = backdrop.min.y + backdrop.height() * 0.25;
        gradient(
            ui,
            Rect::from_min_max(pos2(backdrop.min.x, fade_start), backdrop.max),
            Color32::TRANSPARENT,
            theme::BG,
            false,
        );
        gradient(
            ui,
            Rect::from_min_size(
                backdrop.min,
                vec2(backdrop.width() * 0.6, backdrop.height()),
            ),
            theme::BG.gamma_multiply(0.85),
            Color32::TRANSPARENT,
            true,
        );
    }
    let back = Rect::from_min_size(backdrop.min + Vec2::splat(20.0), vec2(92.0, 32.0));
    if icon_button(ui, back, Icon::ArrowLeft, "Back").clicked() {
        out.push(Action::CloseDetail);
        view.season = None;
    }
    if background.is_some() {
        ui.add_space(-theme::BACKDROP_HEIGHT * 0.6);
    }

    column(ui, |ui| {
        let narrow = ui.available_width() < 760.0;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = if narrow { 20.0 } else { 40.0 };
            let poster_width = if narrow {
                150.0
            } else {
                theme::DETAIL_POSTER_WIDTH
            };
            let poster_size = vec2(poster_width, poster_width * 1.5);
            let (poster_rect, _) = ui.allocate_exact_size(poster_size, Sense::hover());
            let name = preview.map_or(detail.id.as_str(), |p| p.name.as_str());
            shadow(ui, poster_rect, 1.0);
            poster(
                ui,
                poster_rect,
                name,
                preview.and_then(|p| p.poster.as_ref()),
            );

            let width = ui.available_width() - ui.spacing().item_spacing.x;
            ui.vertical(|ui| {
                ui.set_width(width);
                ui.add_space(if background.is_some() { 24.0 } else { 0.0 });
                about(ui, name, preview, meta);
                match &detail.meta {
                    Loadable::Loading => {
                        spinner(ui);
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
                    section(ui, "Where to watch", |_| {});
                    if detail.streams.is_empty() {
                        empty(ui, "No installed addon provides streams for this item.");
                    }
                    for (index, group) in detail.streams.iter().enumerate() {
                        stream_group(ui, index, group, p2p_enabled, out);
                    }
                }
            });
        });
    });
}

/// The journal header: title and year, credits, facts, synopsis, genres
/// and cast.
fn about(ui: &mut Ui, name: &str, preview: Option<&MetaPreview>, meta: Option<&Meta>) {
    let mut job = LayoutJob::default();
    job.append(
        name,
        0.0,
        TextFormat::simple(theme::title(), theme::TEXT_BRIGHT),
    );
    if let Some(year) = preview.and_then(|p| p.release_info.as_deref()) {
        job.append(
            year,
            14.0,
            TextFormat::simple(theme::title_year(), theme::TEXT_DIM),
        );
    }
    ui.add(Label::new(job).wrap());
    if let Some(m) = meta
        && !m.director.is_empty()
    {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 5.0;
            ui.label(dim("Directed by"));
            ui.label(
                RichText::new(m.director.join(", "))
                    .font(theme::strong())
                    .color(theme::TEXT_BRIGHT),
            );
        });
    }
    if let Some(facts) = facts(preview, meta) {
        ui.add_space(2.0);
        ui.label(facts);
    }
    if let Some(description) = preview.and_then(|p| p.description.as_ref()) {
        ui.add_space(theme::GAP);
        ui.scope(|ui| {
            ui.set_max_width(ui.available_width().min(720.0));
            ui.add(
                Label::new(
                    RichText::new(description)
                        .font(theme::reading())
                        .color(theme::TEXT),
                )
                .wrap(),
            );
        });
    }
    if let Some(p) = preview
        && !p.genres.is_empty()
    {
        ui.add_space(theme::SECTION_GAP);
        section(ui, "Genres", |_| {});
        tags(ui, &p.genres);
    }
    if let Some(m) = meta
        && !m.cast.is_empty()
    {
        ui.add_space(theme::SECTION_GAP);
        section(ui, "Cast", |_| {});
        tags(ui, &m.cast);
    }
}

/// Runtime and rating (with a star), from what the addon provided.
fn facts(preview: Option<&MetaPreview>, meta: Option<&Meta>) -> Option<LayoutJob> {
    let runtime = meta.and_then(|m| m.runtime.as_deref());
    let rating = preview.and_then(|p| p.imdb_rating.as_deref());
    if runtime.is_none() && rating.is_none() {
        return None;
    }
    let mut job = LayoutJob::default();
    let caps_format = TextFormat {
        font_id: theme::caption(),
        color: theme::TEXT_FAINT,
        extra_letter_spacing: theme::CAPS_SPACING,
        valign: Align::Center,
        ..TextFormat::default()
    };
    if let Some(runtime) = runtime {
        job.append(&runtime.to_uppercase(), 0.0, caps_format.clone());
    }
    if let Some(rating) = rating {
        if runtime.is_some() {
            job.append("  ·  ", 0.0, caps_format.clone());
        }
        append_icon(&mut job, Icon::Star, 13.0, theme::ACCENT);
        job.append(&format!(" {rating} IMDB"), 0.0, caps_format);
    }
    Some(job)
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
    section(ui, "Episodes", |_| {});
    if seasons.len() > 1 {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(6.0);
            for s in &seasons {
                if chip(ui, &season_label(*s), season == Some(*s)).clicked() {
                    view.season = Some(*s);
                }
            }
        });
        ui.add_space(theme::GAP);
    }
    ui.spacing_mut().item_spacing.y = 0.0;
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
fn list_row(ui: &mut Ui, number: Option<&str>, text: &str, selected: bool) -> Response {
    let height = 40.0;
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    response
        .widget_info(|| WidgetInfo::selected(WidgetType::Button, ui.is_enabled(), selected, text));
    if ui.is_rect_visible(rect) {
        let hover = ui
            .ctx()
            .animate_bool_with_time(response.id, response.hovered(), theme::ANIM);
        let painter = ui.painter();
        if hover > 0.0 {
            painter.rect_filled(rect, 0.0, theme::PANEL.gamma_multiply(hover));
        }
        if selected {
            painter.rect_filled(
                Rect::from_min_size(rect.min, vec2(2.0, rect.height())),
                0.0,
                theme::ACCENT,
            );
        }
        let color = if selected {
            theme::TEXT_BRIGHT
        } else {
            lerp_color(theme::TEXT, theme::TEXT_BRIGHT, hover)
        };
        let mut x = rect.min.x + 12.0;
        if let Some(number) = number {
            painter.text(
                pos2(x, rect.center().y),
                Align2::LEFT_CENTER,
                number,
                theme::strong(),
                if selected {
                    theme::ACCENT
                } else {
                    theme::TEXT_FAINT
                },
            );
            x += 36.0;
        }
        let mut job = LayoutJob::simple_singleline(text.to_owned(), theme::body(), color);
        job.wrap.max_width = rect.max.x - x - 12.0;
        job.wrap.max_rows = 1;
        let galley = painter.layout_job(job);
        let y = rect.center().y - galley.size().y / 2.0;
        painter.galley(pos2(x, y), galley, color);
        painter.hline(
            rect.x_range(),
            rect.bottom() - 0.5,
            Stroke::new(1.0, theme::RULE),
        );
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// One addon's streams as cards. Torrents are left out when P2P is off.
fn stream_group(
    ui: &mut Ui,
    index: usize,
    group: &StreamGroup,
    p2p_enabled: bool,
    out: &mut Vec<Action>,
) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(&group.addon_name)
                .font(theme::strong())
                .color(theme::TEXT_BRIGHT),
        );
        if let Loadable::Ready(streams) = &group.streams
            && !streams.is_empty()
        {
            ui.label(caps_text(
                &count_label(streams.len(), "stream", "streams"),
                theme::TEXT_FAINT,
            ));
        }
    });
    match &group.streams {
        Loadable::Loading => {
            spinner(ui);
        }
        Loadable::Failed(err) => {
            ui.label(RichText::new(err).color(theme::DANGER));
        }
        Loadable::Ready(streams) if streams.is_empty() => {
            ui.label(faint("No streams"));
        }
        Loadable::Ready(streams) => {
            ui.spacing_mut().item_spacing.y = 6.0;
            let mut hidden = 0;
            for (stream_index, stream) in streams.iter().enumerate() {
                if stream.source.is_p2p() && !p2p_enabled {
                    hidden += 1;
                    continue;
                }
                if stream_card(ui, stream) {
                    out.push(Action::Play {
                        group: index,
                        stream: stream_index,
                    });
                }
            }
            if hidden > 0 {
                ui.label(faint(&format!(
                    "{} hidden: peer-to-peer is off in Settings",
                    count_label(hidden, "torrent stream", "torrent streams")
                )));
            }
        }
    }
    ui.add_space(theme::GAP);
}

/// One stream as a card: a round play button, the release title after
/// small resolution and HDR tags, and the addon's details with their emoji
/// as amber icons. The whole card plays when the stream is playable.
/// Returns true if it was clicked.
fn stream_card(ui: &mut Ui, stream: &Stream) -> bool {
    let playable = stream.source.is_playable();
    let mut lines = stream
        .description
        .as_deref()
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty());
    let name = stream.name.as_deref().map(|n| n.replace('\n', " · "));
    let title = lines.next().map(str::to_owned).or_else(|| name.clone());
    let details: Vec<&str> = lines.collect();
    let quality = Quality::of(stream);

    let background = ui.painter().add(egui::Shape::Noop);
    let card = ui.scope_builder(
        UiBuilder::new().sense(if playable {
            Sense::click()
        } else {
            Sense::hover()
        }),
        |ui| {
            Frame::new()
                .inner_margin(Margin::same(10))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 14.0;
                        let play = play_button(ui, playable).on_disabled_hover_text(format!(
                            "{} streams are not supported yet",
                            stream.source.kind_label()
                        ));
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 3.0;
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 5.0;
                                if let Some(quality) = &quality {
                                    quality_tags(ui, quality);
                                }
                                if !stream.source.is_p2p() {
                                    badge(ui, stream.source.kind_label(), theme::TEXT_DIM);
                                }
                                if let Some(title) = &title {
                                    ui.add_space(3.0);
                                    ui.add(
                                        Label::new(addon_text(
                                            title,
                                            &theme::strong(),
                                            theme::TEXT_BRIGHT,
                                            theme::ACCENT,
                                        ))
                                        .truncate(),
                                    );
                                }
                            });
                            if let Some(name) = name.as_ref().filter(|n| Some(*n) != title.as_ref())
                            {
                                ui.add(
                                    Label::new(addon_text(
                                        name,
                                        &theme::caption(),
                                        theme::TEXT_DIM,
                                        theme::ACCENT,
                                    ))
                                    .truncate(),
                                );
                            }
                            for line in &details {
                                ui.add(
                                    Label::new(addon_text(
                                        line,
                                        &theme::caption(),
                                        theme::TEXT_DIM,
                                        theme::ACCENT,
                                    ))
                                    .wrap(),
                                );
                            }
                        });
                        play
                    })
                    .inner
                })
                .inner
        },
    );
    let play = card.inner;
    let response = card.response;
    let hover = ui.ctx().animate_bool_with_time(
        response.id,
        playable && (response.hovered() || play.hovered()),
        theme::ANIM,
    );
    let rect = response.rect;
    let radius = CornerRadius::same(theme::RADIUS);
    ui.painter().set(
        background,
        egui::Shape::rect_filled(
            rect,
            radius,
            lerp_color(theme::PANEL, theme::SURFACE, hover),
        ),
    );
    if hover > 0.0 {
        ui.painter().rect_filled(
            Rect::from_min_size(rect.min, vec2(3.0, rect.height())),
            radius,
            theme::ACCENT.gamma_multiply(hover),
        );
    }
    let response = if playable {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    };
    play.clicked() || response.clicked()
}

/// A round amber button with a play icon, greyed when the source cannot be
/// played. Accessible as a button labelled "Play".
fn play_button(ui: &mut Ui, playable: bool) -> Response {
    ui.add_enabled_ui(playable, |ui| {
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(40.0), Sense::click());
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), "Play"));
        if ui.is_rect_visible(rect) {
            let hover = ui.ctx().animate_bool_with_time(
                response.id,
                playable && response.hovered(),
                theme::ANIM,
            );
            let (fill, ink) = if playable {
                (
                    lerp_color(theme::ACCENT, theme::ACCENT_HOVER, hover),
                    theme::ON_ACCENT,
                )
            } else {
                (theme::SURFACE, theme::TEXT_FAINT)
            };
            let painter = ui.painter();
            painter.circle_filled(rect.center(), 18.0 + 2.0 * hover, fill);
            // The triangle's visual center sits right of its box's center.
            paint_icon(
                painter,
                Icon::Play,
                rect.center() + vec2(1.5, 0.0),
                19.0,
                ink,
            );
        }
        if playable {
            response
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text("Play")
        } else {
            response
        }
    })
    .inner
}

/// Small tags for the resolution, colored by tier, and its HDR flags.
fn quality_tags(ui: &mut Ui, quality: &Quality) {
    let (fill, ink, stroke) = match quality.tier {
        Tier::Ultra => (theme::ACCENT, theme::ON_ACCENT, Stroke::NONE),
        Tier::High => (
            Color32::TRANSPARENT,
            theme::ACCENT,
            Stroke::new(1.0, theme::ACCENT.gamma_multiply(0.7)),
        ),
        Tier::Standard => (theme::SURFACE, theme::TEXT_BRIGHT, Stroke::NONE),
        Tier::Low => (theme::SURFACE, theme::TEXT_DIM, Stroke::NONE),
        Tier::Cam => (
            Color32::TRANSPARENT,
            theme::WARNING,
            Stroke::new(1.0, theme::WARNING),
        ),
    };
    tag(ui, quality.label, fill, ink, stroke);
    for flag in &quality.flags {
        badge(ui, flag, theme::TEXT_DIM);
    }
}

fn tag(ui: &mut Ui, text: &str, fill: Color32, ink: Color32, stroke: Stroke) {
    Frame::new()
        .fill(fill)
        .stroke(stroke)
        .corner_radius(CornerRadius::same(theme::RADIUS))
        .inner_margin(Margin::symmetric(6, 1))
        .show(ui, |ui| {
            ui.label(caps(text, FontId::new(10.5, theme::section().family), ink));
        });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tier {
    Ultra,
    High,
    Standard,
    Low,
    Cam,
}

/// A stream's resolution and HDR flags, read from the words of its name or,
/// failing that, its description (e.g. `Torrentio\n4k DV | HDR`).
#[derive(Debug, PartialEq, Eq)]
struct Quality {
    label: &'static str,
    tier: Tier,
    flags: Vec<&'static str>,
}

impl Quality {
    fn of(stream: &Stream) -> Option<Self> {
        let name = stream.name.as_deref().unwrap_or_default();
        let description = stream.description.as_deref().unwrap_or_default();
        let (label, tier) = resolution(name).or_else(|| resolution(description))?;
        let mut flags = Vec::new();
        let all = || words(name).chain(words(description));
        if all().any(|w| w.starts_with("hdr")) {
            flags.push("HDR");
        }
        if all().any(|w| w == "dv" || w == "dovi") {
            flags.push("DV");
        }
        Some(Self { label, tier, flags })
    }
}

/// Lowercase ASCII words of `text`.
fn words(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_ascii_lowercase)
}

fn resolution(text: &str) -> Option<(&'static str, Tier)> {
    words(text).find_map(|word| match word.as_str() {
        "2160p" | "4k" | "uhd" => Some(("4K", Tier::Ultra)),
        "1440p" | "2k" => Some(("1440p", Tier::High)),
        "1080p" | "fhd" => Some(("1080p", Tier::High)),
        "720p" => Some(("720p", Tier::Standard)),
        "576p" | "480p" | "360p" | "sd" => Some(("SD", Tier::Low)),
        "cam" | "hdcam" | "camrip" | "telesync" | "hdts" => Some(("CAM", Tier::Cam)),
        _ => None,
    })
}

/// The side margin of pages: smaller on narrow windows.
fn page_margin(ui: &Ui) -> f32 {
    if ui.available_width() < 900.0 {
        24.0
    } else {
        f32::from(theme::PAGE_MARGIN)
    }
}

/// Lays `add` out in a centered column at most
/// [`theme::CONTENT_MAX_WIDTH`] wide, with the page margin on narrow windows.
fn column<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let available = ui.available_rect_before_wrap();
    let margin = page_margin(ui);
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

/// A page's serif heading, with an optional dek under it.
fn page_title(ui: &mut Ui, text: &str, dek: Option<&str>) {
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

/// A small-caps section head over a thin rule, with `right` laid out at its
/// right end.
fn section(ui: &mut Ui, title: &str, right: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.label(caps(title, theme::section(), theme::TEXT_DIM));
        ui.with_layout(Layout::right_to_left(Align::Center), right);
    });
    ui.add_space(-4.0);
    rule(ui);
    ui.add_space(6.0);
}

fn rule(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        Stroke::new(1.0, theme::RULE),
    );
}

/// A catalog row. With `see_all`, its header links to the whole catalog
/// on the Discover page.
fn catalog_row(
    ui: &mut Ui,
    salt: (&str, usize),
    row: &Row,
    see_all: Option<&mut ViewState>,
    out: &mut Vec<Action>,
) {
    section(ui, &row.target.title, |ui| {
        if let Some(view) = see_all
            && ui
                .add(Button::new(caps("See all", theme::caption(), theme::ACCENT)).frame(false))
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
                ui.label(faint("This reel jammed:"));
                ui.label(RichText::new(err).color(theme::DANGER));
            });
        }
        Loadable::Ready(items) if items.is_empty() => {
            ui.label(faint("This reel is empty."));
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

/// Placeholder posters while a row loads, so the page does not jump.
fn skeleton_row(ui: &mut Ui) {
    let size = card_size(theme::CARD_WIDTH, PosterShape::Poster);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), size.y), Sense::hover());
    let time = ui.input(|i| i.time);
    #[expect(clippy::cast_possible_truncation, reason = "a small periodic value")]
    let pulse = (((time * 2.0).sin() * 0.5 + 0.5) as f32).mul_add(0.5, 0.5);
    let mut x = rect.min.x;
    while x + size.x <= rect.max.x {
        ui.painter().rect_filled(
            Rect::from_min_size(pos2(x, rect.min.y), size),
            CornerRadius::same(theme::POSTER_RADIUS),
            theme::PANEL.gamma_multiply(pulse),
        );
        x += size.x + theme::CARD_GAP;
    }
    ui.ctx().request_repaint_after(Duration::from_millis(50));
}

/// A horizontally scrolling row of posters with page arrows that show on
/// hover. The mouse wheel keeps scrolling the page; Shift+wheel, a
/// touchpad or the arrows scroll the row.
fn poster_strip(
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
                ui.scroll_with_delta_animation(vec2(delta, 0.0), ScrollAnimation::duration(0.35));
            }
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = theme::CARD_GAP;
                add(ui);
            });
        });
    let inner = output.inner_rect;
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
    if offset > 1.0 && pager(ui, pos2(inner.min.x + 22.0, y), false, shown).clicked() {
        ui.data_mut(|d| d.insert_temp(id, page));
    }
    if offset < max_offset - 1.0 && pager(ui, pos2(inner.max.x - 22.0, y), true, shown).clicked() {
        ui.data_mut(|d| d.insert_temp(id, -page));
    }
}

/// A tall arrow button over a row's edge.
fn pager(ui: &mut Ui, center: Pos2, right: bool, opacity: f32) -> Response {
    let rect = Rect::from_center_size(center, vec2(34.0, 56.0));
    let label = if right { "Scroll right" } else { "Scroll left" };
    let response = ui.interact(rect, ui.id().with(label), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, label));
    let fill = if response.hovered() {
        theme::ACCENT
    } else {
        Color32::from_black_alpha(210)
    };
    let ink = if response.hovered() {
        theme::ON_ACCENT
    } else {
        theme::TEXT_BRIGHT
    };
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

fn poster_grid(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(theme::CARD_GAP, theme::CARD_GAP);
        add(ui);
    });
}

/// The card width that fills `available` exactly with as many columns as
/// fit between the theme's min and max card widths.
fn grid_card_width(available: f32) -> f32 {
    let gap = theme::CARD_GAP;
    let columns = ((available + gap) / (theme::GRID_CARD_MIN + gap))
        .floor()
        .max(1.0);
    let width = (available - gap * (columns - 1.0)) / columns;
    // Rounding down keeps the last column from wrapping.
    width.min(theme::GRID_CARD_MAX).floor()
}

fn preview_card(ui: &mut Ui, width: f32, item: &MetaPreview) -> Response {
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

fn library_card(ui: &mut Ui, width: f32, item: &LibraryItem, out: &mut Vec<Action>) {
    let progress = (item.time_offset_ms > 0).then(|| item.progress());
    if card(
        ui,
        width,
        &item.name,
        None,
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

/// A clickable poster like a printed card, with an optional progress bar.
/// On hover it gets an amber outline and a band with the title and year.
/// Accessible as a button labelled with the title.
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
    poster(ui, rect, name, poster_url);
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

/// A soft drop shadow under `rect`, at `strength` (0–1).
fn shadow(ui: &Ui, rect: Rect, strength: f32) {
    let shadow = egui::Shadow {
        offset: [0, 8],
        blur: 20,
        spread: 0,
        color: Color32::from_black_alpha(150).gamma_multiply(strength),
    };
    ui.painter()
        .add(shadow.as_shape(rect, CornerRadius::same(theme::POSTER_RADIUS)));
}

/// A poster image cropped to `rect`, or the title in serif on a plain card
/// when there is none, with the printed-card edge.
fn poster(ui: &Ui, rect: Rect, name: &str, url: Option<&Url>) {
    let radius = CornerRadius::same(theme::POSTER_RADIUS);
    let painter = ui.painter();
    painter.rect_filled(rect, radius, theme::SURFACE);
    if let Some(url) = url {
        paint_cover(ui, url.as_str(), rect, radius, Color32::WHITE, 0.5);
    } else {
        let font = FontId::new(
            (rect.width() / 7.0).clamp(14.0, 26.0),
            theme::heading().family,
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

/// Smooth downscaling for posters and backdrops shown smaller than their
/// source.
const IMAGE_FILTER: egui::TextureOptions = egui::TextureOptions {
    mipmap_mode: Some(egui::TextureFilter::Linear),
    ..egui::TextureOptions::LINEAR
};

/// Paints the image at `src` filling `rect`, cropping instead of
/// stretching. `focus_y` (0 = top, 1 = bottom) picks which part of a tall
/// image stays visible. It fades in once loaded.
fn paint_cover(ui: &Ui, src: &str, rect: Rect, radius: CornerRadius, tint: Color32, focus_y: f32) {
    let image = Image::new(src)
        .corner_radius(radius)
        .texture_options(IMAGE_FILTER)
        .show_loading_spinner(false);
    let size = image
        .load_for_size(ui.ctx(), rect.size())
        .ok()
        .and_then(|poll| poll.size());
    let loaded = ui
        .ctx()
        .animate_bool_with_time(egui::Id::new(src), size.is_some(), 0.2);
    if loaded <= 0.0 {
        return;
    }
    let uv = size.map_or(Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), |size| {
        cover_uv(size, rect.size(), focus_y)
    });
    image
        .uv(uv)
        .tint(tint.gamma_multiply(loaded))
        .paint_at(ui, rect);
}

/// The part of an image of `image` size to show in `target` so it covers
/// the target without distortion.
fn cover_uv(image: Vec2, target: Vec2, focus_y: f32) -> Rect {
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

/// Paints a gradient over `rect` from `from` at its min edge to `to` at its
/// max edge: left to right if `horizontal`, else top to bottom.
fn gradient(ui: &Ui, rect: Rect, from: Color32, to: Color32, horizontal: bool) {
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

/// A card's size: `width` wide, in the poster's shape.
fn card_size(width: f32, shape: PosterShape) -> Vec2 {
    let height = match shape {
        PosterShape::Poster => width * 1.5,
        PosterShape::Square => width,
        PosterShape::Landscape => width / 1.77,
    };
    vec2(width, height)
}

/// The primary action button: amber with dark text.
fn primary_button(text: &str) -> Button<'_> {
    Button::new(
        RichText::new(text)
            .font(theme::strong())
            .color(theme::ON_ACCENT),
    )
    .fill(theme::ACCENT)
}

/// Adds a primary button, brightened on hover.
fn primary(ui: &mut Ui, enabled: bool, text: &str) -> Response {
    let response = ui.add_enabled(enabled, primary_button(text));
    hover_glow(ui, &response);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Brightens a filled button under the pointer.
fn hover_glow(ui: &Ui, response: &Response) {
    if response.hovered() && response.enabled() {
        ui.painter().rect_filled(
            response.rect,
            CornerRadius::same(theme::RADIUS),
            theme::ACCENT_HOVER.gamma_multiply(0.25),
        );
    }
}

/// A selectable small-caps tab. Accessible as a button labelled `text`.
fn chip(ui: &mut Ui, text: &str, selected: bool) -> Response {
    let ink = if selected {
        theme::ON_ACCENT
    } else {
        theme::TEXT
    };
    let galley = ui.painter().layout_job(caps(text, theme::caption(), ink));
    let size = galley.size() + vec2(22.0, 14.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response
        .widget_info(|| WidgetInfo::selected(WidgetType::Button, ui.is_enabled(), selected, text));
    if ui.is_rect_visible(rect) {
        let fill = if selected {
            theme::ACCENT
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

/// A settings row: `label` with an on/off switch at its right end.
/// Accessible as a checkbox labelled `label`.
fn toggle_row(ui: &mut Ui, on: bool, label: &str) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 36.0), Sense::click());
    response.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, ui.is_enabled(), on, label));
    if ui.is_rect_visible(rect) {
        let t = ui
            .ctx()
            .animate_bool_with_time(response.id, on, theme::ANIM);
        let painter = ui.painter();
        painter.text(
            pos2(rect.min.x, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            theme::strong(),
            theme::TEXT_BRIGHT,
        );
        let track =
            Rect::from_center_size(pos2(rect.max.x - 22.0, rect.center().y), vec2(40.0, 20.0));
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
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn badge(ui: &mut Ui, text: &str, color: Color32) {
    tag(
        ui,
        text,
        Color32::TRANSPARENT,
        color,
        Stroke::new(1.0, theme::RULE),
    );
}

/// Small boxed tags, as genres and cast are shown.
fn tags(ui: &mut Ui, items: &[String]) {
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

/// The icons in use, drawn from the Tabler set (via `iconflow`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Icon {
    Home,
    Compass,
    Search,
    Library,
    Addons,
    Settings,
    ChevronLeft,
    ChevronRight,
    ChevronDown,
    ArrowLeft,
    Loader,
    Star,
    Alert,
    Play,
    /// Any other Tabler icon, by name (the emoji stand-ins).
    Named(&'static str),
}

impl Icon {
    #[cfg(test)]
    const ALL: [Self; 14] = [
        Self::Home,
        Self::Compass,
        Self::Search,
        Self::Library,
        Self::Addons,
        Self::Settings,
        Self::ChevronLeft,
        Self::ChevronRight,
        Self::ChevronDown,
        Self::ArrowLeft,
        Self::Loader,
        Self::Star,
        Self::Alert,
        Self::Play,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Compass => "compass",
            Self::Search => "search",
            Self::Library => "books",
            Self::Addons => "puzzle",
            Self::Settings => "adjustments-horizontal",
            Self::ChevronLeft => "chevron-left",
            Self::ChevronRight => "chevron-right",
            Self::ChevronDown => "chevron-down",
            Self::ArrowLeft => "arrow-left",
            Self::Loader => "loader-2",
            Self::Star => "star",
            Self::Alert => "alert-circle",
            Self::Play => "player-play",
            Self::Named(name) => name,
        }
    }

    /// The glyph and the font family that draws it.
    fn glyph(self) -> Option<(char, egui::FontFamily)> {
        let icon = iconflow::try_icon(
            iconflow::Pack::Tabler,
            self.name(),
            if matches!(self, Self::Star | Self::Play) {
                iconflow::Style::Filled
            } else {
                iconflow::Style::Regular
            },
            iconflow::Size::Regular,
        )
        .ok()?;
        let glyph = char::from_u32(icon.codepoint)?;
        Some((glyph, egui::FontFamily::Name(icon.family.into())))
    }
}

/// Paints `icon` centered at `c`, `size` points tall.
fn paint_icon(painter: &egui::Painter, icon: Icon, c: Pos2, size: f32, color: Color32) {
    if let Some((glyph, family)) = icon.glyph() {
        painter.text(
            c,
            Align2::CENTER_CENTER,
            glyph,
            FontId::new(size, family),
            color,
        );
    }
}

/// Emoji that addons put in stream and addon text (seeders, size, source,
/// quality …) and the Tabler icon drawn in their place.
const EMOJI_ICONS: &[(char, &str)] = &[
    ('👤', "user"),
    ('👥', "users"),
    ('💾', "device-floppy"),
    ('📦', "package"),
    ('⚙', "settings"),
    ('🔗', "link"),
    ('🌐', "world"),
    ('🌍', "world"),
    ('📁', "folder"),
    ('📂', "folder"),
    ('📄', "file"),
    ('🗄', "database"),
    ('🖥', "server"),
    ('📺', "device-tv"),
    ('🎞', "movie"),
    ('🎬', "movie"),
    ('🎥', "video"),
    ('📹', "video"),
    ('🔊', "volume"),
    ('🎧', "headphones"),
    ('🗣', "language"),
    ('💬', "message"),
    ('⏱', "clock"),
    ('🕒', "clock"),
    ('⌛', "clock"),
    ('🔥', "flame"),
    ('⬇', "download"),
    ('⬆', "upload"),
    ('🧲', "magnet"),
    ('💿', "disc"),
    ('📀', "disc"),
    ('⚡', "bolt"),
    ('✅', "check"),
    ('✔', "check"),
    ('❌', "x"),
    ('ℹ', "info-circle"),
    ('🏷', "tag"),
    ('🔍', "search"),
    ('⭐', "star"),
    ('🌟', "star"),
    ('★', "star"),
];

/// The regional-indicator letter (`🇦` … `🇿` → `A` … `Z`), if `c` is one.
fn regional_letter(c: char) -> Option<char> {
    let offset = u32::from(c).checked_sub(0x1F1E6)?;
    (offset < 26).then(|| char::from(b'A' + u8::try_from(offset).unwrap_or(0)))
}

/// Emoji blocks (misc. technical, symbols, dingbats, pictographs) and
/// emoji joiners/variation selectors: what only egui's emoji fonts would
/// draw. Arrows and geometric shapes are left alone; Inter has them.
fn is_emoji(c: char) -> bool {
    matches!(
        u32::from(c),
        0x200D | 0xFE0E | 0xFE0F | 0x20E3
            | 0x2300..=0x23FF
            | 0x2600..=0x27BF
            | 0x2B00..=0x2BFF
            | 0x1F000..=0x1FAFF
            | 0xE0020..=0xE007F
    )
}

/// Addon text laid out with its emoji drawn as Tabler icons. Flags become
/// their two-letter region code; other emoji are left out, so nothing
/// falls back to egui's emoji fonts. The text is otherwise shown as is.
fn addon_text(text: &str, font: &FontId, color: Color32, icon_color: Color32) -> LayoutJob {
    let format = TextFormat {
        font_id: font.clone(),
        color,
        valign: Align::Center,
        ..TextFormat::default()
    };
    let mut job = LayoutJob::default();
    let mut plain = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if let Some(first) = regional_letter(c) {
            if let Some(second) = chars.peek().copied().and_then(regional_letter) {
                chars.next();
                plain.push(first);
                plain.push(second);
            }
            continue;
        }
        if let Some(&(_, name)) = EMOJI_ICONS.iter().find(|(emoji, _)| *emoji == c) {
            job.append(&std::mem::take(&mut plain), 0.0, format.clone());
            append_icon(&mut job, Icon::Named(name), font.size, icon_color);
            continue;
        }
        if !is_emoji(c) {
            plain.push(c);
        }
    }
    job.append(&plain, 0.0, format);
    job
}

/// Appends `icon` to `job` as a glyph `size` points tall.
fn append_icon(job: &mut LayoutJob, icon: Icon, size: f32, color: Color32) {
    if let Some((glyph, family)) = icon.glyph() {
        job.append(
            &glyph.to_string(),
            0.0,
            TextFormat {
                font_id: FontId::new(size, family),
                color,
                valign: Align::Center,
                ..TextFormat::default()
            },
        );
    }
}

/// A rotating Tabler loader in its own slot.
fn spinner(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::hover());
    paint_spinner(ui, rect.center(), 18.0);
}

/// Paints a rotating Tabler loader centered at `center`, one turn per
/// second, and keeps repainting while it is visible.
fn paint_spinner(ui: &Ui, center: Pos2, size: f32) {
    if !ui.is_rect_visible(Rect::from_center_size(center, Vec2::splat(size))) {
        return;
    }
    let Some((glyph, family)) = Icon::Loader.glyph() else {
        return;
    };
    let turns = ui.input(|i| i.time).fract();
    #[expect(clippy::cast_possible_truncation, reason = "an angle in 0..2π")]
    let angle = (turns * std::f64::consts::TAU) as f32;
    let galley = ui.painter().layout_no_wrap(
        glyph.to_string(),
        FontId::new(size, family),
        theme::TEXT_DIM,
    );
    ui.painter().add(
        egui::epaint::TextShape::new(center, galley, theme::TEXT_DIM)
            .with_angle_and_anchor(angle, Align2::CENTER_CENTER),
    );
    ui.ctx().request_repaint();
}

/// A framed button with an icon before its small-caps label. Accessible
/// as a button labelled `label`.
fn icon_button(ui: &mut Ui, rect: Rect, icon: Icon, label: &str) -> Response {
    let response = ui.interact(rect, ui.id().with(label), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, label));
    let painter = ui.painter();
    let fill = if response.hovered() {
        theme::SURFACE_HOVER
    } else {
        theme::SCRIM
    };
    painter.rect_filled(rect, CornerRadius::same(theme::RADIUS), fill);
    let galley = painter.layout_job(caps(label, theme::caption(), theme::TEXT_BRIGHT));
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

/// Uppercase, letter-spaced text for section heads, nav and labels.
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

/// [`caps`] in the caption size.
fn caps_text(text: &str, color: Color32) -> LayoutJob {
    caps(text, theme::caption(), color)
}

fn lerp_color(from: Color32, to: Color32, t: f32) -> Color32 {
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
    fn sizes_use_binary_units_with_one_decimal() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1536), "1.5 KiB");
        assert_eq!(format_bytes(5 * 1024 * 1024 * 1024), "5.0 GiB");
        assert_eq!(format_bytes(u64::MAX), "16777216.0 TiB");
    }

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

    #[test]
    fn every_icon_exists_in_the_tabler_set() {
        for icon in Icon::ALL {
            assert!(icon.glyph().is_some(), "{icon:?} ({})", icon.name());
        }
    }

    #[test]
    fn every_emoji_stand_in_exists_in_the_tabler_set() {
        for (emoji, name) in EMOJI_ICONS {
            assert!(Icon::Named(name).glyph().is_some(), "{emoji} → {name}");
        }
    }

    #[test]
    fn addon_emoji_become_tabler_icons_flags_and_nothing_else() {
        let job = addon_text(
            "👤 12 💾 1.2 GB ⚙️ YTS\nMulti / 🇬🇧 / 🇮🇹 😀✨",
            &theme::body(),
            theme::TEXT,
            theme::TEXT,
        );
        let text = &job.text;
        assert!(!text.chars().any(is_emoji), "{text:?}");
        assert!(
            !text.chars().any(|c| regional_letter(c).is_some()),
            "{text:?}"
        );
        for kept in ["12", "1.2 GB", "YTS", "Multi / GB / IT"] {
            assert!(text.contains(kept), "{kept} in {text:?}");
        }
        let tabler_sections = job
            .sections
            .iter()
            .filter(|s| matches!(&s.format.font_id.family, egui::FontFamily::Name(f) if f.starts_with("Tabler")))
            .count();
        assert_eq!(tabler_sections, 3, "user, floppy and gear");
    }

    #[test]
    fn plain_addon_text_is_unchanged() {
        let text = "Example HTTP stream — 1080p → 720p ▲";
        let job = addon_text(text, &theme::body(), theme::TEXT, theme::TEXT);
        assert_eq!(job.text, text);
    }

    fn stream(name: Option<&str>, description: Option<&str>) -> Stream {
        let field = |key: &str, value: Option<&str>| {
            value.map_or(String::new(), |v| {
                format!(r#","{key}":"{}""#, v.replace('\n', "\\n"))
            })
        };
        let json = format!(
            r#"{{"streams":[{{"infoHash":"0123456789abcdef0123456789abcdef01234567"{}{}}}]}}"#,
            field("name", name),
            field("description", description),
        );
        cineo_core::addon::parse_stream_response(json.as_bytes())
            .unwrap()
            .value
            .remove(0)
    }

    #[test]
    fn quality_comes_from_the_name_then_the_description() {
        let q = Quality::of(&stream(Some("Torrentio\n4k DV | HDR"), None)).unwrap();
        assert_eq!(
            q,
            Quality {
                label: "4K",
                tier: Tier::Ultra,
                flags: vec!["HDR", "DV"]
            }
        );
        let q = Quality::of(&stream(
            Some("Torrentio"),
            Some("Film.2023.1080p.WEB-DL.x264\n👤 12 💾 1.2 GB"),
        ))
        .unwrap();
        assert_eq!((q.label, q.tier), ("1080p", Tier::High));
        assert!(q.flags.is_empty());
        let q = Quality::of(&stream(Some("HDCAM"), None)).unwrap();
        assert_eq!(q.tier, Tier::Cam);
    }

    #[test]
    fn quality_needs_a_whole_word() {
        assert_eq!(
            Quality::of(&stream(Some("Torrent"), Some("Scam 4kHz"))),
            None
        );
        assert_eq!(Quality::of(&stream(None, None)), None);
    }

    #[test]
    fn grid_cards_fill_the_width_within_the_size_limits() {
        for available in [300.0, 777.0, 1000.0, 1440.0] {
            let width = grid_card_width(available);
            assert!(
                width >= theme::GRID_CARD_MIN.min(available),
                "{available}: {width}"
            );
            assert!(width <= theme::GRID_CARD_MAX, "{available}: {width}");
            let columns = ((available + theme::CARD_GAP) / (width + theme::CARD_GAP)).floor();
            let used = columns * width + (columns - 1.0) * theme::CARD_GAP;
            assert!(used <= available, "{available}: {used}");
        }
        // A window narrower than one card still gets one column.
        assert_eq!(grid_card_width(100.0), 100.0);
    }
}
