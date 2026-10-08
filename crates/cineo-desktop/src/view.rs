//! Rendering. [`show`] draws a [`State`] snapshot and returns the
//! [`Action`]s the user triggered; it performs no IO and holds no business
//! rules (ADR-0001, ADR-0011). Addon strings are shown as plain text only.

use std::time::Duration;

use cineo_core::addon::{Meta, MetaPreview, PosterShape, Stream};
use cineo_core::app::{
    Action, CatalogTarget, Detail, InterfaceScale, LibraryItem, Loadable, Row, StartPage, State,
    StreamGroup, board_targets, continue_watching,
};
use eframe::egui::{
    self, Align, Align2, Button, Color32, ComboBox, CornerRadius, FontId, Frame, Image, Key, Label,
    Layout, Margin, Mesh, Modal, Modifiers, Pos2, Rect, Response, RichText, ScrollArea, Sense,
    Stroke, StrokeKind, TextEdit, TextFormat, Ui, UiBuilder, Vec2, WidgetInfo, WidgetType, pos2,
    scroll_area::ScrollBarVisibility, style::ScrollAnimation, text::LayoutJob, vec2,
};
use url::Url;

use crate::brand;
use crate::settings::{self, Confirm, Section};
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

impl From<StartPage> for Page {
    fn from(page: StartPage) -> Self {
        match page {
            StartPage::Home => Self::Board,
            StartPage::Discover => Self::Discover,
            StartPage::Library => Self::Library,
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
    pub settings_jump: Option<Section>,
    pub confirm: Option<Confirm>,
    pub streams: StreamFilter,
}

impl ViewState {
    fn leave_detail(&mut self) {
        self.season = None;
        self.streams = StreamFilter::default();
    }
}

#[derive(Debug, Clone, Default)]
pub struct StreamFilter {
    pub query: String,
    pub quality: Option<&'static str>,
    pub addon: Option<usize>,
}

impl StreamFilter {
    fn is_active(&self) -> bool {
        !self.query.trim().is_empty() || self.quality.is_some() || self.addon.is_some()
    }

    fn matches(&self, stream: &Stream) -> bool {
        if self.quality.is_some() && Quality::of(stream).map(|q| q.label) != self.quality {
            return false;
        }
        let text = [stream.name.as_deref(), stream.description.as_deref()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        self.query
            .split_whitespace()
            .all(|word| text.contains(&word.to_lowercase()))
    }
}

const SEARCH_DEBOUNCE: f64 = 0.45;
/// Draws the whole window and returns the actions to dispatch.
pub fn show(ui: &mut Ui, state: &State, view: &mut ViewState) -> Vec<Action> {
    let mut out = Vec::new();
    if theme::ensure(ui.ctx()) {
        ui.ctx().request_discard("theme applied");
        return out;
    }
    apply_scale(ui.ctx(), state.settings.interface_scale);
    shortcuts(ui, state, view, &mut out);
    sidebar(ui, state, view, &mut out);
    if let Some(notice) = &state.notice {
        notice_bar(ui, notice, &mut out);
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
            if state.detail.is_none() && view.page == Page::Settings {
                settings::page(ui, state, view, &mut out);
                return;
            }
            ScrollArea::vertical()
                .id_salt(salt)
                .auto_shrink(false)
                .show(ui, |ui| {
                    if let Some(detail) = &state.detail {
                        detail_page(ui, detail, state.settings.p2p_enabled, view, &mut out);
                    } else {
                        ui.add_space(theme::PAGE_MARGIN);
                        column(ui, |ui| match view.page {
                            Page::Board => board_page(ui, state, view, &mut out),
                            Page::Discover => discover_page(ui, state, &mut out),
                            Page::Search => search_page(ui, state, view, &mut out),
                            Page::Library => library_page(ui, state, &mut out),
                            Page::Addons => addons_page(ui, state, view, &mut out),
                            Page::Settings => {}
                        });
                    }
                    ui.add_space(theme::PAGE_MARGIN);
                });
        });
    out
}

#[expect(clippy::cast_precision_loss, reason = "a percentage")]
fn apply_scale(ctx: &egui::Context, scale: InterfaceScale) {
    let zoom = scale.percent() as f32 / 100.0;
    if (ctx.zoom_factor() - zoom).abs() > f32::EPSILON {
        ctx.set_zoom_factor(zoom);
    }
}

fn go(page: Page, state: &State, view: &mut ViewState, out: &mut Vec<Action>) {
    if state.detail.is_some() {
        out.push(Action::CloseDetail);
        view.leave_detail();
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
        view.leave_detail();
    }
}

fn sidebar(ui: &mut Ui, state: &State, view: &mut ViewState, out: &mut Vec<Action>) {
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
                let version = faint(concat!("Cineo v", env!("CARGO_PKG_VERSION")));
                if compact {
                    ui.vertical_centered(|ui| ui.label(version));
                } else {
                    ui.horizontal(|ui| {
                        ui.add_space(9.0);
                        ui.label(version);
                    });
                }
                ui.add_space(2.0);
                item(ui, Page::Settings);
                if !state.addons_loading.is_empty() {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        spinner(ui);
                        if !compact {
                            ui.label(faint("Loading addons…"));
                        }
                    });
                }
            });
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
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Image, true, "Cineo"));
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
        brand::grain(ui.ctx(), pixels).id(),
        backdrop,
        full,
        Color32::WHITE,
    );
    if let Some(texture) = brand::logo(ui.ctx()) {
        painter.image(texture.id(), rect, full, theme::LOGO_TINT);
    }
}

fn nav_item(ui: &mut Ui, page: Page, selected: bool, compact: bool) -> Response {
    let label = page.label();
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::click());
    response.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, selected, label));
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
        let icon_center = if compact {
            rect.center()
        } else {
            pos2(rect.min.x + 18.0, rect.center().y)
        };
        paint_icon(painter, page.icon(), icon_center, 18.0, color);
        if !compact {
            painter.text(
                pos2(rect.min.x + 36.0, rect.center().y),
                Align2::LEFT_CENTER,
                label,
                theme::nav(),
                color,
            );
        }
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn notice_bar(ui: &mut Ui, notice: &str, out: &mut Vec<Action>) {
    egui::Panel::top("notice")
        .show_separator_line(false)
        .frame(
            Frame::new()
                .fill(theme::PANEL)
                .inner_margin(Margin::symmetric(0, 6)),
        )
        .show(ui, |ui| {
            let full = ui.max_rect();
            ui.painter().hline(
                full.x_range(),
                full.bottom() + 5.5,
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

fn p2p_prompt(ui: &mut Ui, out: &mut Vec<Action>) {
    let modal = Modal::new(egui::Id::new("p2p_prompt"))
        .frame(
            Frame::new()
                .fill(theme::PANEL)
                .stroke(Stroke::new(1.0, theme::RULE))
                .corner_radius(CornerRadius::same(theme::RADIUS))
                .inner_margin(Margin::same(20)),
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

pub(crate) const P2P_NOTICE: &str = "Torrent streams come from other people's computers. While one \
plays, your IP address is visible to the peers and trackers it connects to, and Cineo \
uploads the parts it has already downloaded to those peers. Downloaded data is kept in \
a local cache. You can turn peer-to-peer streaming off in Settings.";

fn board_page(ui: &mut Ui, state: &State, view: &mut ViewState, out: &mut Vec<Action>) {
    let resume = continue_watching(&state.library, state.settings.watched_at);
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
        page_title(ui, "Home", None);
        empty(
            ui,
            "No addons installed. Catalogs show up here once you add one by its manifest URL.",
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
    page_title(ui, "Discover", None);
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
    ui.add_space(theme::GAP);
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
    let (end, _) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::hover());
    if discover.pending.is_some() {
        paint_spinner(ui, end.center(), 24.0, theme::TEXT_DIM);
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
            .hint_text("Search movies and series")
            .font(FontId::new(15.0, egui::FontFamily::Proportional))
            .margin(Margin {
                left: 34,
                right: 10,
                top: 7,
                bottom: 7,
            })
            .desired_width(width),
    );
    paint_icon(
        ui.painter(),
        Icon::Search,
        pos2(field.rect.min.x + 17.0, field.rect.center().y),
        16.0,
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
            "Results come from every installed addon that supports search.",
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
    page_title(ui, "Library", None);
    section(ui, "Recently played", |ui| {
        if !state.library.is_empty() {
            ui.label(faint(&count_label(state.library.len(), "title", "titles")));
        }
    });
    if state.library.is_empty() {
        empty(ui, "Anything you play shows up here.");
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
                    .add(Button::new(faint("Remove")).frame(false))
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
        Some("When two addons return the same item, the one higher in the list wins."),
    );
    section(ui, "Install an addon", |_| {});
    ui.horizontal(|ui| {
        let busy = state.install.as_ref().is_some_and(Loadable::is_loading);
        let field_width = (ui.available_width() - 110.0).clamp(160.0, 560.0);
        let field = ui.add(
            TextEdit::singleline(&mut view.addon_input)
                .hint_text("https://…/manifest.json")
                .margin(Margin::symmetric(8, 5))
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
            ui.label(faint(&count_label(count, "addon", "addons")));
        }
    });
    if state.installed.is_empty() {
        empty(ui, "Nothing installed yet.");
    }
    for (index, addon) in state.addons.iter().enumerate() {
        let manifest = &addon.manifest;
        ui.add_space(4.0);
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

    let height = if background.is_some() {
        theme::BACKDROP_HEIGHT
    } else {
        56.0
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
    let back = Rect::from_min_size(backdrop.min + Vec2::splat(14.0), vec2(76.0, 30.0));
    if icon_button(ui, back, Icon::ArrowLeft, "Back").clicked() {
        out.push(Action::CloseDetail);
        view.leave_detail();
    }
    if background.is_some() {
        ui.add_space(-theme::BACKDROP_HEIGHT * 0.6);
    }

    column(ui, |ui| {
        let narrow = ui.available_width() < 760.0;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = if narrow { 16.0 } else { 28.0 };
            let poster_width = if narrow {
                128.0
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
                ui.add_space(if background.is_some() { 16.0 } else { 0.0 });
                about(ui, name, preview, meta);
                match &detail.meta {
                    Loadable::Loading => {
                        centered_spinner(ui);
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
                    stream_filters(ui, &detail.streams, p2p_enabled, &mut view.streams);
                    for (index, group) in detail.streams.iter().enumerate() {
                        if view.streams.addon.is_none_or(|addon| addon == index) {
                            stream_group(ui, index, group, p2p_enabled, &view.streams, out);
                        }
                    }
                }
            });
        });
    });
}

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

fn facts(preview: Option<&MetaPreview>, meta: Option<&Meta>) -> Option<LayoutJob> {
    let runtime = meta.and_then(|m| m.runtime.as_deref());
    let rating = preview.and_then(|p| p.imdb_rating.as_deref());
    if runtime.is_none() && rating.is_none() {
        return None;
    }
    let mut job = LayoutJob::default();
    let format = TextFormat {
        font_id: theme::body(),
        color: theme::TEXT_DIM,
        valign: Align::Center,
        ..TextFormat::default()
    };
    if let Some(runtime) = runtime {
        job.append(runtime, 0.0, format.clone());
    }
    if let Some(rating) = rating {
        if runtime.is_some() {
            job.append("  ·  ", 0.0, format.clone());
        }
        append_icon(&mut job, Icon::Star, 14.0, theme::ACCENT);
        job.append(&format!(" {rating} IMDb"), 0.0, format);
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

fn list_row(ui: &mut Ui, number: Option<&str>, text: &str, selected: bool) -> Response {
    let height = 32.0;
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
            painter.rect_filled(rect, 0.0, theme::SURFACE);
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
            x += 30.0;
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

const QUALITIES: [&str; 6] = ["4K", "1440p", "1080p", "720p", "SD", "CAM"];

fn stream_filters(
    ui: &mut Ui,
    groups: &[StreamGroup],
    p2p_enabled: bool,
    filter: &mut StreamFilter,
) {
    let streams: Vec<Vec<&Stream>> = groups
        .iter()
        .map(|group| match &group.streams {
            Loadable::Ready(streams) => streams
                .iter()
                .filter(|s| p2p_enabled || !s.source.is_p2p())
                .collect(),
            _ => Vec::new(),
        })
        .collect();
    if streams.iter().map(Vec::len).sum::<usize>() < 2 {
        return;
    }
    let field = ui.add(
        TextEdit::singleline(&mut filter.query)
            .id_salt("stream-search")
            .hint_text("Search streams")
            .margin(Margin {
                left: 30,
                right: 10,
                top: 6,
                bottom: 6,
            })
            .desired_width(ui.available_width().min(360.0)),
    );
    paint_icon(
        ui.painter(),
        Icon::Search,
        pos2(field.rect.min.x + 15.0, field.rect.center().y),
        14.0,
        theme::TEXT_DIM,
    );
    ui.add_space(theme::GAP);
    let found: Vec<&'static str> = streams
        .iter()
        .flatten()
        .filter_map(|s| Quality::of(s).map(|q| q.label))
        .collect();
    let present = QUALITIES.into_iter().filter(|label| found.contains(label));
    let addons: Vec<usize> = (0..groups.len())
        .filter(|i| !streams[*i].is_empty())
        .collect();
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::splat(6.0);
        if chip(
            ui,
            "All",
            filter.quality.is_none() && filter.addon.is_none(),
        )
        .clicked()
        {
            filter.quality = None;
            filter.addon = None;
        }
        for label in present {
            let selected = filter.quality == Some(label);
            if chip(ui, label, selected).clicked() {
                filter.quality = (!selected).then_some(label);
            }
        }
        if addons.len() > 1 {
            for index in addons {
                let selected = filter.addon == Some(index);
                if chip(ui, &groups[index].addon_name, selected).clicked() {
                    filter.addon = (!selected).then_some(index);
                }
            }
        }
    });
    ui.add_space(theme::GAP);
}

fn stream_group(
    ui: &mut Ui,
    index: usize,
    group: &StreamGroup,
    p2p_enabled: bool,
    filter: &StreamFilter,
    out: &mut Vec<Action>,
) {
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(&group.addon_name)
                .font(theme::strong())
                .color(theme::TEXT_BRIGHT),
        );
        if let Loadable::Ready(streams) = &group.streams
            && !streams.is_empty()
        {
            ui.label(faint(&count_label(streams.len(), "stream", "streams")));
        }
    });
    match &group.streams {
        Loadable::Loading => {
            centered_spinner(ui);
        }
        Loadable::Failed(err) => {
            ui.label(RichText::new(err).color(theme::DANGER));
        }
        Loadable::Ready(streams) if streams.is_empty() => {
            ui.label(faint("No streams"));
        }
        Loadable::Ready(streams) => {
            ui.spacing_mut().item_spacing.y = 4.0;
            let mut hidden = 0;
            let mut shown = 0;
            for (stream_index, stream) in streams.iter().enumerate() {
                if stream.source.is_p2p() && !p2p_enabled {
                    hidden += 1;
                    continue;
                }
                if !filter.matches(stream) {
                    continue;
                }
                shown += 1;
                let id = ui.id().with(("stream", index, stream_index));
                let width = ui.available_width();
                let known = ui.data(|d| d.get_temp::<Vec2>(id)).filter(|s| s.x == width);
                if let Some(size) = known
                    && !ui.is_rect_visible(Rect::from_min_size(ui.cursor().min, size))
                {
                    ui.allocate_space(size);
                    continue;
                }
                let (clicked, rect) = stream_card(ui, stream);
                ui.data_mut(|d| d.insert_temp(id, vec2(width, rect.height())));
                if clicked {
                    out.push(Action::Play {
                        group: index,
                        stream: stream_index,
                    });
                }
            }
            if shown == 0 && filter.is_active() {
                ui.label(faint("No matching streams"));
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

fn stream_card(ui: &mut Ui, stream: &Stream) -> (bool, Rect) {
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
                .inner_margin(Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 10.0;
                        let play = play_button(ui, playable);
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
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
                                            theme::TEXT_DIM,
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
                                        theme::TEXT_FAINT,
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
                                        theme::TEXT_FAINT,
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
    let response = if playable {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    };
    (play.clicked() || response.clicked(), rect)
}

fn play_button(ui: &mut Ui, playable: bool) -> Response {
    ui.add_enabled_ui(playable, |ui| {
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::click());
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
            painter.circle_filled(rect.center(), 15.0 + hover, fill);
            paint_icon(
                painter,
                Icon::Play,
                rect.center() + vec2(1.0, 0.0),
                15.0,
                ink,
            );
        }
        if playable {
            response.on_hover_cursor(egui::CursorIcon::PointingHand)
        } else {
            response
        }
    })
    .inner
}

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
    let galley = ui.painter().layout_job(caps(text, theme::tag(), ink));
    let (rect, response) = ui.allocate_exact_size(galley.size() + vec2(10.0, 2.0), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, galley.text()));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let radius = CornerRadius::same(theme::RADIUS);
        painter.rect_filled(rect, radius, fill);
        painter.rect_stroke(rect, radius, stroke, StrokeKind::Inside);
        painter.galley(rect.center() - galley.size() / 2.0, galley, ink);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tier {
    Ultra,
    High,
    Standard,
    Low,
    Cam,
}

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

pub(crate) fn compact(ctx: &egui::Context) -> bool {
    ctx.content_rect().width() < theme::COMPACT_BELOW
}

fn column<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
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

fn rule(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        Stroke::new(1.0, theme::RULE),
    );
}

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
                .add(Button::new(RichText::new("See all").color(theme::TEXT_DIM)).frame(false))
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
                ui.label(faint("Could not load:"));
                ui.label(RichText::new(err).color(theme::DANGER));
            });
        }
        Loadable::Ready(items) if items.is_empty() => {
            ui.label(faint("Nothing in this catalog."));
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

fn pager(ui: &mut Ui, strip: egui::Id, center: Pos2, right: bool, opacity: f32) -> Response {
    let rect = Rect::from_center_size(center, vec2(34.0, 56.0));
    let label = if right { "Scroll right" } else { "Scroll left" };
    let response = ui.interact(rect, strip.with(label), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, label));
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

fn poster_grid(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(theme::CARD_GAP, theme::CARD_GAP);
        add(ui);
    });
}

fn grid_card_width(available: f32) -> f32 {
    let gap = theme::CARD_GAP;
    let columns = ((available + gap) / (theme::GRID_CARD_MIN + gap))
        .floor()
        .max(1.0);
    let width = (available - gap * (columns - 1.0)) / columns;
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
    let response = card(
        ui,
        width,
        &item.name,
        None,
        item.poster.as_ref(),
        PosterShape::Poster,
        progress,
    );
    if item.stream.is_some() && ui.is_rect_visible(response.rect) {
        let hover = ui.ctx().animate_bool_with_time(
            response.id.with("play"),
            response.hovered() || response.has_focus(),
            theme::ANIM,
        );
        let center = response.rect.center();
        let radius = (response.rect.width() / 6.0).clamp(18.0, 28.0);
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
    if response.clicked() {
        out.push(Action::Resume(item.id.clone()));
    }
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

fn poster(ui: &Ui, rect: Rect, name: &str, url: Option<&Url>) {
    let radius = CornerRadius::same(theme::POSTER_RADIUS);
    let painter = ui.painter();
    painter.rect_filled(rect, radius, theme::SURFACE);
    if let Some(url) = url {
        paint_cover(ui, url.as_str(), rect, radius, Color32::WHITE, 0.5);
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
) {
    let image = Image::new(src)
        .corner_radius(radius)
        .texture_options(IMAGE_FILTER)
        .show_loading_spinner(false);
    let Some(size) = image
        .load_for_size(ui.ctx(), rect.size())
        .ok()
        .and_then(|poll| poll.size())
    else {
        return;
    };
    image
        .uv(cover_uv(size, rect.size(), focus_y))
        .tint(tint)
        .paint_at(ui, rect);
}

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

fn card_size(width: f32, shape: PosterShape) -> Vec2 {
    let height = match shape {
        PosterShape::Poster => width * 1.5,
        PosterShape::Square => width,
        PosterShape::Landscape => width / 1.77,
    };
    vec2(width, height)
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

fn chip(ui: &mut Ui, text: &str, selected: bool) -> Response {
    let ink = if selected { theme::BG } else { theme::TEXT };
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), theme::body(), ink);
    let size = galley.size() + vec2(24.0, 12.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response
        .widget_info(|| WidgetInfo::selected(WidgetType::Button, ui.is_enabled(), selected, text));
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

fn badge(ui: &mut Ui, text: &str, color: Color32) {
    tag(
        ui,
        text,
        Color32::TRANSPARENT,
        color,
        Stroke::new(1.0, theme::RULE),
    );
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Icon {
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
    Pause,
    Named(&'static str),
}

impl Icon {
    #[cfg(test)]
    const ALL: [Self; 15] = [
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
        Self::Pause,
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
            Self::Pause => "player-pause",
            Self::Named(name) => name,
        }
    }

    pub(crate) fn glyph(self) -> Option<(char, egui::FontFamily)> {
        let icon = iconflow::try_icon(
            iconflow::Pack::Tabler,
            self.name(),
            if matches!(self, Self::Star | Self::Play | Self::Pause) {
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

pub(crate) fn paint_icon(painter: &egui::Painter, icon: Icon, c: Pos2, size: f32, color: Color32) {
    let Some((glyph, family)) = icon.glyph() else {
        return;
    };
    let galley = painter.layout_no_wrap(glyph.to_string(), FontId::new(size, family), color);
    let center = ink_center(&galley).unwrap_or_else(|| galley.rect.center());
    painter.galley(c - center.to_vec2(), galley, color);
}

fn ink_center(galley: &egui::Galley) -> Option<Pos2> {
    let row = galley.rows.first()?;
    let glyph = row.glyphs.first()?;
    let ink = Rect::from_min_size(glyph.pos + glyph.uv_rect.offset, glyph.uv_rect.size);
    Some(row.pos + ink.center().to_vec2())
}

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

fn regional_letter(c: char) -> Option<char> {
    let offset = u32::from(c).checked_sub(0x1F1E6)?;
    (offset < 26).then(|| char::from(b'A' + u8::try_from(offset).unwrap_or(0)))
}

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

pub(crate) fn append_icon(job: &mut LayoutJob, icon: Icon, size: f32, color: Color32) {
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

fn spinner(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::hover());
    paint_spinner(ui, rect.center(), 18.0, theme::TEXT_DIM);
}

fn centered_spinner(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 48.0), Sense::hover());
    paint_spinner(ui, rect.center(), 24.0, theme::TEXT_DIM);
}

pub(crate) fn paint_spinner(ui: &Ui, center: Pos2, size: f32, color: Color32) {
    if !ui.is_rect_visible(Rect::from_center_size(center, Vec2::splat(size))) {
        return;
    }
    let Some((glyph, family)) = Icon::Loader.glyph() else {
        return;
    };
    let galley = ui
        .painter()
        .layout_no_wrap(glyph.to_string(), FontId::new(size, family), color);
    let Some(pivot) = ink_center(&galley) else {
        return;
    };
    let turns = ui.input(|i| i.time).fract();
    #[expect(clippy::cast_possible_truncation, reason = "an angle in 0..2π")]
    let angle = (turns * std::f64::consts::TAU) as f32;
    let pos = center - egui::emath::Rot2::from_angle(angle) * pivot.to_vec2();
    ui.painter()
        .add(egui::epaint::TextShape::new(pos, galley, color).with_angle(angle));
    ui.ctx().request_repaint();
}

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

fn empty(ui: &mut Ui, text: &str) {
    ui.add_space(theme::GAP / 2.0);
    ui.label(dim(text));
    ui.add_space(theme::GAP / 2.0);
}

pub(crate) fn dim(text: &str) -> RichText {
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
    fn stream_filter_needs_every_word_and_the_quality() {
        let s = stream(Some("Torrentio\n1080p"), Some("Film.2024.WEB-DL 👤 12"));
        let filter = |query: &str, quality| StreamFilter {
            query: query.into(),
            quality,
            addon: None,
        };
        assert!(filter("", None).matches(&s));
        assert!(filter("web-dl  TORRENTIO", None).matches(&s));
        assert!(!filter("web-dl hevc", None).matches(&s));
        assert!(filter("film", Some("1080p")).matches(&s));
        assert!(!filter("", Some("4K")).matches(&s));
        assert!(!filter("   ", None).is_active());
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
        assert_eq!(grid_card_width(100.0), 100.0);
    }
}
