use self::addons::addons_page;
use self::detail::{detail_page, source_name};
use self::pages::{board_page, discover_page, library_page, search_page};
use self::sidebar::sidebar;
use self::widgets::column;
use crate::i18n::{self, Locale, Text, pt, t};
use crate::settings::{self, Confirm, Section};
use crate::theme;
use cineo_core::addon::{MetaPreview, ResourcePath, Stream, TransportUrl};
use cineo_core::app::{
    Action, InterfaceScale, Notice, PlaybackFailure, Problem, Resolution, Route, StartPage, State,
    StreamFacts as Facts, StreamSort, board_targets, stream_matcher,
};
use eframe::egui::{
    self, Align, Frame, Key, Label, Layout, Margin, Modifiers, RichText, ScrollArea, Sense, Stroke,
    Ui, Vec2, vec2,
};

mod addons;
mod cards;
mod detail;
mod icons;
mod pages;
mod sidebar;
#[cfg(test)]
mod tests;
mod widgets;

pub(crate) use self::cards::{IMAGE_FILTER, paint_cover};
pub(crate) use self::icons::{Icon, append_icon, paint_icon, paint_nav_label};
pub(crate) use self::widgets::{
    compact, dialog, dim, gradient, lerp_color, page_title, paint_loading, section,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
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

    pub(crate) fn label(self) -> Text {
        match self {
            Self::Board => t!("Home"),
            Self::Discover => t!("Discover"),
            Self::Search => pt!("page-search", "Search"),
            Self::Library => t!("Library"),
            Self::Addons => t!("Addons"),
            Self::Settings => t!("Settings"),
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

#[derive(Debug, Clone, Default)]
pub struct ViewState {
    pub desktop_text_scale: Option<f32>,
    pub page: Page,
    pub addon_input: String,
    pub search_input: String,
    pub season: Option<u32>,
    pub search_edited_at: Option<f64>,
    pub focus_search: bool,
    pub settings_jump: Option<Section>,
    pub addon_list: Option<(TransportUrl, ResourcePath)>,
    pub confirm: Option<Confirm>,
    pub streams: StreamFilter,
    pub stream_facts: StreamFacts,
    pub system_locale: Locale,
    pub register_links: bool,
    pub links_registered: Option<Result<(), String>>,
}

impl ViewState {
    fn leave_detail(&mut self) {
        self.season = None;
        self.streams = StreamFilter {
            sort: self.streams.sort,
            ..StreamFilter::default()
        };
        self.stream_facts = StreamFacts::default();
    }
}

#[derive(Debug, Clone, Default)]
pub struct StreamFilter {
    pub query: String,
    pub quality: Option<Resolution>,
    pub addon: Option<usize>,
    pub sort: StreamSort,
}

#[derive(Debug, Clone, Default)]
pub struct StreamFacts {
    groups: Vec<GroupFacts>,
}

impl StreamFacts {
    fn group(&mut self, index: usize, streams: &[Stream]) -> &mut GroupFacts {
        if self.groups.len() <= index {
            self.groups.resize_with(index + 1, GroupFacts::default);
        }
        let group = &mut self.groups[index];
        if group.source != streams {
            *group = GroupFacts {
                source: streams.to_vec(),
                streams: streams.iter().map(Facts::of).collect(),
                ..GroupFacts::default()
            };
        }
        group
    }
}

#[derive(Debug, Clone, Default)]
struct GroupFacts {
    source: Vec<Stream>,
    streams: Vec<Facts>,
    sorted_by: Option<StreamSort>,
    order: Vec<usize>,
}

impl GroupFacts {
    fn sorted(&mut self, sort: StreamSort) -> (&[usize], &[Facts]) {
        if self.sorted_by != Some(sort) {
            self.order = sort.order(&self.streams);
            self.sorted_by = Some(sort);
        }
        (&self.order, &self.streams)
    }
}

impl StreamFilter {
    fn is_active(&self) -> bool {
        !self.query.trim().is_empty() || self.quality.is_some() || self.addon.is_some()
    }

    fn matcher(&self) -> impl Fn(&Facts) -> bool {
        stream_matcher(&self.query, self.quality)
    }
}

const SEARCH_DEBOUNCE: f64 = 0.45;

pub fn show(ui: &mut Ui, state: &State, view: &mut ViewState) -> Vec<Action> {
    let mut out = Vec::new();
    i18n::set(Locale::resolve(
        state.settings.ui_language,
        view.system_locale,
    ));
    if theme::ensure(ui.ctx()) {
        ui.ctx().request_discard("theme applied");
        return out;
    }
    apply_scale(
        ui.ctx(),
        state.settings.interface_scale,
        view.desktop_text_scale,
    );
    shortcuts(ui, state, view, &mut out);
    sidebar(ui, state, view, &mut out);
    if let Some(notice) = &state.notice {
        notice_bar(ui, notice, &mut out);
    }
    if state.p2p_prompt.is_some() {
        p2p_prompt(ui, &mut out);
    }
    if let Some(transport) = &state.link_prompt {
        link_prompt(ui, transport.as_url().as_str(), &mut out);
    }
    egui::CentralPanel::default()
        .frame(Frame::new().fill(theme::BG))
        .show(ui, |ui| {
            let salt = state.detail.is_none().then_some(view.page);
            if state.detail.is_none() && view.page == Page::Settings {
                settings::page(ui, state, view, &mut out);
                return;
            }
            ScrollArea::vertical()
                .id_salt(salt)
                .auto_shrink(false)
                .show(ui, |ui| {
                    if let Some(detail) = &state.detail {
                        let favorite = state
                            .library
                            .iter()
                            .any(|i| i.id == detail.id && i.is_favorite());
                        detail_page(
                            ui,
                            detail,
                            state.settings.p2p_enabled,
                            favorite,
                            view,
                            &mut out,
                        );
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
fn zoom_factor(scale: InterfaceScale, desktop_text_scale: Option<f32>) -> f32 {
    scale.percent() as f32 / 100.0 * desktop_text_scale.unwrap_or(1.0)
}

fn apply_scale(ctx: &egui::Context, scale: InterfaceScale, desktop_text_scale: Option<f32>) {
    let zoom = zoom_factor(scale, desktop_text_scale);
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
    if page == Page::Addons {
        out.push(Action::LoadAddonLists);
    }
    if page == Page::Search {
        view.focus_search = true;
    }
    view.page = page;
}

fn shortcuts(ui: &Ui, state: &State, view: &mut ViewState, out: &mut Vec<Action>) {
    let typing = ui.ctx().egui_wants_keyboard_input();
    let modal = state.p2p_prompt.is_some() || state.link_prompt.is_some();
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

fn notice_bar(ui: &mut Ui, notice: &Notice, out: &mut Vec<Action>) {
    let notice = notice_text(notice);
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
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button(t!("Dismiss")).clicked() {
                            out.push(Action::DismissNotice);
                        }
                        ui.allocate_ui_with_layout(
                            vec2(ui.available_width(), 0.0),
                            Layout::left_to_right(Align::Center),
                            |ui| {
                                ui.add(
                                    Label::new(RichText::new(&*notice).color(theme::TEXT_BRIGHT))
                                        .wrap(),
                                );
                            },
                        );
                    });
                });
            });
        });
}

pub(crate) fn follow_link(route: Route, state: &State, view: &mut ViewState) -> Vec<Action> {
    let mut out = Vec::new();
    let page = match &route {
        Route::InstallAddon(_) => Some(Page::Addons),
        Route::Board => Some(Page::Board),
        Route::Discover(_) => Some(Page::Discover),
        Route::Library => Some(Page::Library),
        Route::Search(query) => {
            view.search_input.clone_from(query);
            view.search_edited_at = None;
            Some(Page::Search)
        }
        Route::Detail { .. } => None,
    };
    match page {
        Some(page) => go(page, state, view, &mut out),
        None => view.leave_detail(),
    }
    out.push(Action::OpenLink(route));
    out
}

fn link_prompt(ui: &Ui, url: &str, out: &mut Vec<Action>) {
    let title = t!("Install this addon?");
    let notice = t!("A link asks to install the addon below. Only install addons you trust.");
    if let Some(accepted) = dialog(
        ui,
        "link_prompt",
        &title,
        &notice,
        Some(url),
        &t!("Install"),
    ) {
        out.push(if accepted {
            Action::AcceptLink
        } else {
            Action::DeclineLink
        });
    }
}

fn p2p_prompt(ui: &Ui, out: &mut Vec<Action>) {
    let (title, notice) = (
        t!("Peer-to-peer streaming"),
        t!(
            "Torrent streams come from other people's computers. While one plays, your IP address is visible to the peers and trackers it connects to, and Cineo uploads the parts it has already downloaded to those peers. Downloaded data is kept in a local cache. You can turn peer-to-peer streaming off in Settings."
        ),
    );
    if let Some(accepted) = dialog(
        ui,
        "p2p_prompt",
        &title,
        &notice,
        None,
        &t!("Accept and play"),
    ) {
        out.push(if accepted {
            Action::AcceptP2p
        } else {
            Action::DeclineP2p
        });
    }
}

fn problem_text(problem: &Problem) -> Text {
    match problem {
        Problem::AlreadyInstalled => t!("This addon is already installed"),
        Problem::ConfigurationRequired(_) => t!(
            "This addon must be configured before it can be installed. Configure it, then install the link it gives you."
        ),
        Problem::InvalidAddonUrl(detail) => {
            t!("Invalid addon URL: {detail}", detail = detail.as_str())
        }
        Problem::NoMetaAddon => t!("No installed addon provides details for this item"),
        Problem::UnsupportedFilter => t!("This catalog does not support that filter"),
        Problem::Request(detail) => detail.clone().into(),
    }
}

fn notice_text(notice: &Notice) -> Text {
    match notice {
        Notice::AddonUnavailable(detail) => {
            t!(
                "An addon could not be loaded: {detail}",
                detail = detail.as_str()
            )
        }
        Notice::UnsupportedScheme(scheme) => {
            t!(
                "Unsupported stream scheme `{scheme}`",
                scheme = scheme.as_str()
            )
        }
        Notice::TorrentsOff => t!("Torrent streams are turned off in Settings"),
        Notice::UnsupportedSource(kind) => {
            t!(
                "{kind} streams are not supported yet",
                kind = source_name(*kind)
            )
        }
        Notice::PlaybackFailed {
            failure,
            pick_another,
        } => {
            let reason: Text = match failure {
                PlaybackFailure::Player(detail) => detail.clone().into(),
                PlaybackFailure::Torrent(detail) => {
                    t!(
                        "The torrent could not be played: {detail}",
                        detail = detail.as_str()
                    )
                }
            };
            if *pick_another {
                t!(
                    "The last stream could not be played, pick another one. {reason}",
                    reason = reason
                )
            } else {
                reason
            }
        }
        Notice::UnexpectedEngineAddress => t!("The torrent engine returned an unexpected address"),
        Notice::SubtitleFailed(detail) => t!(
            "Could not load the subtitle: {detail}",
            detail = detail.as_str()
        ),
        Notice::SaveFailed(detail) => {
            t!("Could not save changes: {detail}", detail = detail.as_str())
        }
        Notice::StoreClosed => t!("Changes can no longer be saved"),
        Notice::InvalidLink => t!("That link cannot be opened by Cineo"),
    }
}

fn open_detail(item: &MetaPreview) -> Action {
    Action::OpenDetail {
        content_type: item.content_type.clone(),
        id: item.id.clone(),
        preview: Some(Box::new(item.clone())),
    }
}

#[cfg(test)]
mod scale_tests {
    use super::*;

    #[test]
    fn the_desktop_text_size_multiplies_the_interface_scale() {
        assert_eq!(zoom_factor(InterfaceScale::S100, None), 1.0);
        assert_eq!(zoom_factor(InterfaceScale::S100, Some(1.25)), 1.25);
        assert_eq!(zoom_factor(InterfaceScale::S75, Some(2.0)), 1.5);
    }
}
