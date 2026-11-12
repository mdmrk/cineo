use super::cards::{paint_cover, poster, shadow};
use super::icons::{Icon, addon_text, append_icon, paint_icon};
use super::widgets::{
    badge, caps, centered_spinner, chip, clear_button, column, dim, empty, faint, gradient,
    icon_button, lerp_color, loading_screen, offscreen_size, section, tags,
};
use super::{StreamFacts, StreamFilter, ViewState, problem_text};
use crate::i18n::{Text, t};
use crate::theme;
use cineo_core::addon::{Meta, MetaPreview, SourceKind, Stream};
use cineo_core::app::{
    Action, Detail, Loadable, Quality, Resolution, StreamGroup, StreamSort, Tier,
};
use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, Frame, Label, Margin, Rect, Response, RichText,
    Sense, Stroke, StrokeKind, TextEdit, TextFormat, Ui, UiBuilder, Vec2, WidgetInfo, WidgetType,
    pos2, text::LayoutJob, vec2,
};

pub(super) fn detail_page(
    ui: &mut Ui,
    detail: &Detail,
    p2p_enabled: bool,
    favorite: bool,
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
            true,
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
    if icon_button(ui, back, Icon::ArrowLeft, &t!("Back")).clicked() {
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
            let name = preview.map_or(detail.id.as_str(), |p| p.name.as_str());
            ui.vertical(|ui| {
                ui.set_width(poster_width);
                let (poster_rect, _) = ui.allocate_exact_size(poster_size, Sense::hover());
                shadow(ui, poster_rect, 1.0);
                poster(
                    ui,
                    poster_rect,
                    name,
                    preview.and_then(|p| p.poster.as_ref()),
                    true,
                );
                ui.add_space(6.0);
                if favorite_button(ui, favorite) {
                    out.push(Action::SetFavorite {
                        id: detail.id.clone(),
                        favorite: !favorite,
                    });
                }
            });

            let width = ui.available_width() - ui.spacing().item_spacing.x;
            ui.vertical(|ui| {
                ui.set_width(width);
                ui.add_space(if background.is_some() { 16.0 } else { 0.0 });
                about(ui, name, preview, meta);
                match &detail.meta {
                    Loadable::Loading if preview.is_none() => loading_screen(ui),
                    Loadable::Loading => centered_spinner(ui),
                    Loadable::Failed(err) => {
                        ui.label(RichText::new(problem_text(err)).color(theme::DANGER));
                    }
                    Loadable::Ready(meta) if !meta.videos.is_empty() => {
                        ui.add_space(theme::SECTION_GAP);
                        episodes(ui, meta, detail, view, out);
                    }
                    Loadable::Ready(_) => {}
                }
                if detail.selected_video.is_some() {
                    ui.add_space(theme::SECTION_GAP);
                    section(ui, &t!("Where to watch"), |_| {});
                    if detail.streams.is_empty() {
                        empty(
                            ui,
                            &t!("No installed addon provides streams for this item."),
                        );
                    }
                    stream_filters(
                        ui,
                        &detail.streams,
                        p2p_enabled,
                        &mut view.streams,
                        &mut view.stream_facts,
                    );
                    for (index, group) in detail.streams.iter().enumerate() {
                        if view.streams.addon.is_none_or(|addon| addon == index) {
                            stream_group(
                                ui,
                                index,
                                group,
                                p2p_enabled,
                                &view.streams,
                                &mut view.stream_facts,
                                out,
                            );
                        }
                    }
                }
            });
        });
    });
}

fn favorite_button(ui: &mut Ui, favorite: bool) -> bool {
    let (icon, text, label, ink) = if favorite {
        (
            Icon::Heart,
            t!("Favourited"),
            t!("Remove from favourites"),
            theme::ACCENT,
        )
    } else {
        (
            Icon::Named("heart"),
            t!("Favourite"),
            t!("Add to favourites"),
            theme::TEXT_BRIGHT,
        )
    };
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &label));
    let fill = if response.hovered() {
        theme::SURFACE_HOVER
    } else {
        theme::SURFACE
    };
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(theme::RADIUS), fill);
    let galley = painter.layout_no_wrap(text.into_owned(), theme::body(), theme::TEXT_BRIGHT);
    let left = rect.center().x - (16.0 + 6.0 + galley.size().x) / 2.0;
    paint_icon(painter, icon, pos2(left + 8.0, rect.center().y), 16.0, ink);
    painter.galley(
        pos2(left + 22.0, rect.center().y - galley.size().y / 2.0),
        galley,
        theme::TEXT_BRIGHT,
    );
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
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
            ui.label(dim(&t!("Directed by")));
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
        section(ui, &t!("Genres"), |_| {});
        tags(ui, &p.genres);
    }
    if let Some(m) = meta
        && !m.cast.is_empty()
    {
        ui.add_space(theme::SECTION_GAP);
        section(ui, &t!("Cast"), |_| {});
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
        job.append(
            &format!(" {}", t!("{rating} IMDb", rating = rating)),
            0.0,
            format,
        );
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
    section(ui, &t!("Episodes"), |_| {});
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

fn sort_label(sort: StreamSort) -> Text {
    match sort {
        StreamSort::AddonOrder => t!("Addon order"),
        StreamSort::Quality => t!("Quality"),
        StreamSort::Seeders => t!("Seeders"),
        StreamSort::Size => t!("Size"),
    }
}

fn stream_filters(
    ui: &mut Ui,
    groups: &[StreamGroup],
    p2p_enabled: bool,
    filter: &mut StreamFilter,
    facts: &mut StreamFacts,
) {
    let shown = |group| shown_streams(group, p2p_enabled);
    if groups.iter().flat_map(shown).nth(1).is_none() {
        return;
    }
    let field = ui.add(
        TextEdit::singleline(&mut filter.query)
            .id_salt("stream-search")
            .hint_text(t!("Search streams"))
            .margin(Margin {
                left: 30,
                right: 30,
                top: 6,
                bottom: 6,
            })
            .desired_width(ui.available_width().min(360.0)),
    );
    clear_button(ui, &field, &mut filter.query);
    paint_icon(
        ui.painter(),
        Icon::Search,
        pos2(field.rect.min.x + 15.0, field.rect.center().y),
        14.0,
        theme::TEXT_DIM,
    );
    ui.add_space(theme::GAP);
    let mut found = [false; Resolution::ALL.len()];
    for (index, group) in groups.iter().enumerate() {
        let Some(streams) = group.streams.ready() else {
            continue;
        };
        let group_facts = facts.group(index, streams);
        for (stream, stream_facts) in streams.iter().zip(&group_facts.streams) {
            if !p2p_enabled && stream.source.is_p2p() {
                continue;
            }
            if let Some(quality) = stream_facts.quality {
                found[quality.resolution as usize] = true;
            }
        }
    }
    let present = Resolution::ALL
        .into_iter()
        .zip(found)
        .filter_map(|(resolution, found)| found.then_some(resolution));
    let addons: Vec<usize> = (0..groups.len())
        .filter(|&i| shown(&groups[i]).next().is_some())
        .collect();
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::splat(6.0);
        if chip(
            ui,
            &t!("All"),
            filter.quality.is_none() && filter.addon.is_none(),
        )
        .clicked()
        {
            filter.quality = None;
            filter.addon = None;
        }
        for resolution in present {
            let selected = filter.quality == Some(resolution);
            if chip(ui, resolution.label(), selected).clicked() {
                filter.quality = (!selected).then_some(resolution);
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
    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::splat(6.0);
        ui.label(faint(&t!("Sort by")));
        for sort in StreamSort::ALL {
            if chip(ui, &sort_label(sort), filter.sort == sort).clicked() {
                filter.sort = sort;
            }
        }
    });
    ui.add_space(theme::GAP);
}

fn shown_streams(group: &StreamGroup, p2p_enabled: bool) -> impl Iterator<Item = &Stream> {
    group
        .streams
        .ready()
        .into_iter()
        .flatten()
        .filter(move |s| p2p_enabled || !s.source.is_p2p())
}

fn stream_group(
    ui: &mut Ui,
    index: usize,
    group: &StreamGroup,
    p2p_enabled: bool,
    filter: &StreamFilter,
    facts: &mut StreamFacts,
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
            ui.label(faint(&t!("Streams: {count}", count = streams.len())));
        }
    });
    match &group.streams {
        Loadable::Loading => {
            centered_spinner(ui);
        }
        Loadable::Failed(err) => {
            ui.label(RichText::new(problem_text(err)).color(theme::DANGER));
        }
        Loadable::Ready(streams) if streams.is_empty() => {
            ui.label(faint(&t!("No streams")));
        }
        Loadable::Ready(streams) => {
            ui.spacing_mut().item_spacing.y = 4.0;
            let matches = filter.matcher();
            let (order, stream_facts) = facts.group(index, streams).sorted(filter.sort);
            let mut hidden = 0;
            let mut shown = 0;
            for &stream_index in order {
                let stream = &streams[stream_index];
                if stream.source.is_p2p() && !p2p_enabled {
                    hidden += 1;
                    continue;
                }
                if !matches(&stream_facts[stream_index]) {
                    continue;
                }
                shown += 1;
                let id = ui.id().with(("stream", index, stream_index));
                let width = ui.available_width();
                if let Some(size) = offscreen_size(ui, id, width) {
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
                ui.label(faint(&t!("No matching streams")));
            }
            if hidden > 0 {
                ui.label(faint(&t!(
                    "Torrent streams hidden: {count} (peer-to-peer is off in Settings)",
                    count = hidden
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
                                    badge(ui, &source_name(stream.source.kind()), theme::TEXT_DIM);
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
        response
            .widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), t!("Play")));
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
    let (fill, ink, stroke) = match quality.tier() {
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
    tag(ui, quality.label(), fill, ink, stroke);
    for flag in quality.flags {
        badge(ui, flag, theme::TEXT_DIM);
    }
}

pub(super) fn tag(ui: &mut Ui, text: &str, fill: Color32, ink: Color32, stroke: Stroke) {
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

fn season_label(season: u32) -> Text {
    if season == 0 {
        t!("Specials")
    } else {
        t!("Season {number}", number = season)
    }
}

pub(super) fn source_name(kind: SourceKind) -> Text {
    match kind {
        SourceKind::Http => t!("HTTP"),
        SourceKind::OtherUrl => t!("URL"),
        SourceKind::YouTube => t!("YouTube"),
        SourceKind::Torrent => t!("Torrent"),
        SourceKind::External => t!("External"),
        SourceKind::Archive => t!("Archive"),
        SourceKind::Usenet => t!("Usenet"),
    }
}
