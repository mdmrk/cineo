use super::cards::{
    Shelf, catalog_row, grid_card_width, library_card, poster_grid, poster_strip, preview_card,
};
use super::icons::{Icon, paint_icon};
use super::widgets::{
    chip, clear_button, empty, faint, loading_screen, page_title, paint_spinner, primary, section,
};
use super::{Page, SEARCH_DEBOUNCE, ViewState, go, open_detail, problem_text};
use crate::i18n::{Text, pt, t};
use crate::theme;
use cineo_core::addon::ContentType;
use cineo_core::app::{
    Action, CatalogTarget, LibraryItem, State, board_targets, continue_watching,
};
use eframe::egui::{
    self, ComboBox, CornerRadius, FontId, Key, Margin, RichText, Sense, Stroke, StrokeKind,
    TextEdit, Ui, Vec2, pos2, vec2,
};
use std::time::Duration;

pub(super) fn board_page(ui: &mut Ui, state: &State, view: &mut ViewState, out: &mut Vec<Action>) {
    let resume = continue_watching(&state.library, state.settings.watched_at);
    if !resume.is_empty() {
        section(ui, &t!("Continue watching"), |_| {});
        poster_strip(ui, "continue", |ui| {
            for item in resume {
                library_card(ui, theme::CARD_WIDTH, item, Shelf::ContinueWatching, out);
            }
        });
        ui.add_space(theme::SECTION_GAP);
    }
    if state.installed.is_empty() {
        page_title(ui, &t!("Home"), None);
        empty(
            ui,
            &t!("No addons installed. Catalogs show up here once you add one by its manifest URL."),
        );
        if primary(ui, true, &t!("Open Addons")).clicked() {
            go(Page::Addons, state, view, out);
        }
    }
    for (index, row) in state.board.iter().enumerate() {
        catalog_row(ui, ("board", index), row, Some(&mut *view), out);
    }
}

pub(super) fn discover_page(ui: &mut Ui, state: &State, out: &mut Vec<Action>) {
    let targets = board_targets(&state.addons);
    let discover = &state.discover;
    page_title(ui, &t!("Discover"), None);
    if targets.is_empty() {
        empty(ui, &t!("No installed addon has a browsable catalog."));
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
                catalog_title(target)
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
            let all = t!("All genres");
            let current = discover.genre.as_deref().unwrap_or(&all);
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
                            .selectable_label(discover.genre.is_none(), &*all)
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
                })
                .response
                .on_hover_cursor(egui::CursorIcon::PointingHand);
        });
    }
    ui.add_space(theme::GAP);
    if let Some(error) = &discover.error {
        ui.label(RichText::new(problem_text(error)).color(theme::DANGER));
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
    if discover.pending.is_some() && discover.items.is_empty() {
        loading_screen(ui);
    } else if discover.pending.is_some() {
        paint_spinner(ui, end.center(), 24.0, theme::TEXT_DIM);
    } else if discover.next_skip.is_some() && ui.is_rect_visible(end) {
        out.push(Action::LoadMoreDiscover);
    }
}

pub(super) fn search_page(ui: &mut Ui, state: &State, view: &mut ViewState, out: &mut Vec<Action>) {
    page_title(ui, &pt!("page-search", "Search"), None);
    let width = ui.available_width().min(640.0);
    let field = ui.add(
        TextEdit::singleline(&mut view.search_input)
            .id_salt("search")
            .hint_text(t!("Search movies and series"))
            .font(FontId::new(15.0, egui::FontFamily::Proportional))
            .margin(Margin {
                left: 34,
                right: 32,
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
    let cleared = clear_button(ui, &field, &mut view.search_input);
    let now = ui.input(|i| i.time);
    if field.changed() {
        view.search_edited_at = Some(now);
    }
    let submitted = cleared || field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
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
            &t!("Results come from every installed addon that supports search."),
        );
        return;
    }
    if state.search.is_empty() {
        empty(ui, &t!("No installed addon supports search."));
    }
    for (index, row) in state.search.iter().enumerate() {
        catalog_row(ui, ("search", index), row, None, out);
    }
}

pub(super) fn library_page(ui: &mut Ui, state: &State, out: &mut Vec<Action>) {
    page_title(ui, &t!("Library"), None);
    let mut favorites: Vec<&LibraryItem> =
        state.library.iter().filter(|i| i.is_favorite()).collect();
    favorites.sort_by_key(|i| std::cmp::Reverse(i.favorited));
    let mut rest: Vec<&LibraryItem> = state.library.iter().filter(|i| i.was_played()).collect();
    rest.sort_by_key(|i| std::cmp::Reverse(i.updated_ms));
    if !favorites.is_empty() {
        section(ui, &t!("Favourites"), |ui| {
            ui.label(faint(&t!("Titles: {count}", count = favorites.len())));
        });
        library_grid(ui, "favorites", Shelf::Favorites, &favorites, out);
        ui.add_space(theme::SECTION_GAP);
    }
    section(ui, &t!("Recently played"), |ui| {
        if !rest.is_empty() {
            ui.label(faint(&t!("Titles: {count}", count = rest.len())));
        }
    });
    if rest.is_empty() {
        empty(ui, &t!("Anything you play shows up here."));
        return;
    }
    library_grid(ui, "recent", Shelf::Library, &rest, out);
}

fn library_grid(
    ui: &mut Ui,
    salt: &str,
    shelf: Shelf,
    items: &[&LibraryItem],
    out: &mut Vec<Action>,
) {
    let width = grid_card_width(ui.available_width());
    ui.push_id(salt, |ui| {
        poster_grid(ui, |ui| {
            for item in items {
                library_card(ui, width, item, shelf, out);
            }
        });
    });
}

pub(super) fn catalog_title(target: &CatalogTarget) -> Text {
    let content_type = &target.path.content_type;
    let kind = match content_type.as_str() {
        ContentType::MOVIE => t!("Movies"),
        ContentType::SERIES => t!("Series"),
        ContentType::CHANNEL => t!("Channels"),
        ContentType::TV => t!("TV"),
        other => other.to_owned().into(),
    };
    t!("{name} {kind}", name = target.name.as_str(), kind = kind)
}

fn target_label(target: &CatalogTarget) -> Text {
    format!("{} — {}", catalog_title(target), target.addon_name).into()
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
