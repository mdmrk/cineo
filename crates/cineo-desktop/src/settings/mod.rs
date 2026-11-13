use cineo_core::app::{
    Action, AudioOutput, DownloadLimit, HideControls, InterfaceScale, Language, NextVideoNotice,
    PeerLimit, SeekStep, Setting, Settings, ShortSeekStep, StartPage, State, TorrentStorage,
    UiLanguage, UploadLimit, WatchedAt,
};
use eframe::egui::{
    self, Align, CornerRadius, Label, Layout, Rect, Response, RichText, ScrollArea, Sense,
    TextFormat, Ui, UiBuilder, WidgetInfo, WidgetType, pos2, text::LayoutJob, vec2,
};

use crate::brand;
use crate::i18n::{Locale, Text, pt, t};
use crate::theme;
use crate::view::{
    Icon, Page, ViewState, append_icon, compact, dialog, dim, page_title, paint_loading,
    paint_nav_label, section,
};

mod subtitles;
mod widgets;

pub(crate) use subtitles::subtitle_controls;
use subtitles::subtitle_style;
pub(crate) use widgets::row;
use widgets::{pick, port_field, select, switch};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Interface,
    Player,
    Languages,
    Subtitles,
    Audio,
    Torrents,
    Data,
    Keyboard,
    About,
}

impl Section {
    const ALL: [Self; 9] = [
        Self::Interface,
        Self::Player,
        Self::Languages,
        Self::Subtitles,
        Self::Audio,
        Self::Torrents,
        Self::Data,
        Self::Keyboard,
        Self::About,
    ];

    fn label(self) -> Text {
        match self {
            Self::Interface => t!("Interface"),
            Self::Player => t!("Player"),
            Self::Languages => t!("Languages"),
            Self::Subtitles => t!("Subtitles"),
            Self::Audio => t!("Audio"),
            Self::Torrents => t!("Torrents"),
            Self::Data => t!("Data"),
            Self::Keyboard => t!("Keyboard"),
            Self::About => t!("About"),
        }
    }

    fn icon(self) -> Icon {
        Icon::Named(match self {
            Self::Interface => "layout-dashboard",
            Self::Player => "movie",
            Self::Languages => "language",
            Self::Subtitles => "badge-cc",
            Self::Audio => "volume",
            Self::Torrents => "magnet",
            Self::Data => "database",
            Self::Keyboard => "keyboard",
            Self::About => "info-circle",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirm {
    ResetSettings,
    ClearLibrary,
}

const NAV_WIDTH: f32 = 168.0;

pub(crate) fn page(ui: &mut Ui, state: &State, view: &mut ViewState, out: &mut Vec<Action>) {
    let jump = view.settings_jump.take();
    let full = ui.available_rect_before_wrap();
    let margin = theme::PAGE_MARGIN;
    let show_nav = !compact(ui.ctx());
    let nav = Rect::from_min_size(
        full.min + vec2(margin, margin),
        vec2(NAV_WIDTH, full.height() - margin),
    );
    let content_left = if show_nav {
        nav.max.x + 24.0
    } else {
        full.min.x
    };
    let content = Rect::from_min_max(pos2(content_left, full.min.y), full.max);

    let mut current = Section::ALL[0];
    ui.scope_builder(UiBuilder::new().max_rect(content), |ui| {
        ScrollArea::vertical()
            .id_salt("Settings")
            .auto_shrink(false)
            .show(ui, |ui| {
                let (visible_top, visible_height) = (ui.clip_rect().top(), ui.clip_rect().height());
                ui.add_space(margin);
                let margins = if show_nav { margin } else { 2.0 * margin };
                let width = (ui.available_width() - margins).clamp(0.0, theme::CONTENT_MAX_WIDTH);
                ui.scope_builder(
                    UiBuilder::new().max_rect(Rect::from_min_size(
                        ui.cursor().min + vec2(if show_nav { 0.0 } else { margin }, 0.0),
                        vec2(width, f32::INFINITY),
                    )),
                    |ui| {
                        ui.set_width(width);
                        page_title(ui, &t!("Settings"), None);
                        let mut last_top = 0.0;
                        for part in Section::ALL {
                            let top = ui.cursor().min.y;
                            last_top = top;
                            section(ui, &part.label(), |_| {});
                            ui.add_space(theme::GAP / 2.0);
                            if jump == Some(part) {
                                ui.scroll_to_rect(
                                    Rect::from_min_size(
                                        pos2(ui.min_rect().min.x, top),
                                        vec2(1.0, 1.0),
                                    ),
                                    Some(Align::TOP),
                                );
                            }
                            if top <= visible_top + 48.0 {
                                current = part;
                            }
                            body(ui, part, full.center().x, state, view, out);
                            ui.add_space(theme::SECTION_GAP);
                        }
                        let tail = ui.cursor().min.y - last_top;
                        ui.add_space((visible_height - tail - margin).max(0.0));
                    },
                );
                ui.add_space(margin);
            });
    });
    if show_nav {
        ui.scope_builder(UiBuilder::new().max_rect(nav), |ui| {
            ui.add_space(theme::GAP * 4.0);
            for part in Section::ALL {
                if nav_item(ui, part, part == jump.unwrap_or(current)).clicked() {
                    view.settings_jump = Some(part);
                }
            }
        });
    }
    confirm(ui, view, out);
}

fn body(
    ui: &mut Ui,
    section: Section,
    center_x: f32,
    state: &State,
    view: &mut ViewState,
    out: &mut Vec<Action>,
) {
    let s = &state.settings;
    match section {
        Section::Interface => {
            let system = view.system_locale;
            pick(
                ui,
                out,
                &t!("Language"),
                &t!("The language of Cineo's menus and messages."),
                s.ui_language,
                &ui_languages(),
                |language| ui_language_name(language, system),
                Setting::UiLanguage,
            );
            pick(
                ui,
                out,
                &t!("Interface size"),
                &t!("Makes text, posters and controls larger or smaller."),
                s.interface_scale,
                InterfaceScale::ALL,
                scale_name,
                Setting::InterfaceScale,
            );
            pick(
                ui,
                out,
                &t!("Start page"),
                &t!("Shown when Cineo opens."),
                s.start_page,
                StartPage::ALL,
                start_name,
                Setting::StartPage,
            );
            if cfg!(not(target_os = "macos")) {
                row(
                    ui,
                    &t!("Open stremio:// and cineo:// links"),
                    &t!(
                        "Links to addons and pages open in Cineo. Stremio stops receiving stremio:// links."
                    ),
                    |ui| {
                        let line = vec2(ui.available_width(), ui.spacing().interact_size.y);
                        ui.allocate_ui_with_layout(
                            line,
                            Layout::right_to_left(Align::Center),
                            |ui| {
                                if ui.button(t!("Use Cineo")).clicked() {
                                    view.register_links = true;
                                }
                                let status = match &view.links_registered {
                                    Some(Ok(())) => t!("Done"),
                                    Some(Err(err)) => t!("Failed: {detail}", detail = err.as_str()),
                                    None => return,
                                };
                                ui.add(
                                    Label::new(RichText::new(status).color(theme::TEXT_DIM)).wrap(),
                                );
                            },
                        );
                    },
                );
            }
        }
        Section::Player => {
            switch(
                ui,
                out,
                &t!("Hardware decoding"),
                &t!(
                    "Lets the graphics card decode video, which saves power. Turn it off if videos show artifacts or a black picture."
                ),
                s.hardware_decoding,
                Setting::HardwareDecoding,
            );
            pick(
                ui,
                out,
                &t!("Seek step"),
                &t!("How far ←/→ and the seek buttons jump."),
                s.seek_step,
                SeekStep::ALL,
                |step: SeekStep| seconds(step.seconds()),
                Setting::SeekStep,
            );
            pick(
                ui,
                out,
                &t!("Short seek step"),
                &t!("How far Shift+←/→ jump."),
                s.short_seek_step,
                ShortSeekStep::ALL,
                |step: ShortSeekStep| seconds(step.seconds()),
                Setting::ShortSeekStep,
            );
            pick(
                ui,
                out,
                &t!("Hide controls after"),
                &t!("Without mouse or key input while playing."),
                s.hide_controls,
                HideControls::ALL,
                |after: HideControls| seconds(after.seconds()),
                Setting::HideControls,
            );
            switch(
                ui,
                out,
                &t!("Esc leaves fullscreen first"),
                &t!("When off, Esc leaves the player at once."),
                s.escape_leaves_fullscreen,
                Setting::EscapeLeavesFullscreen,
            );
            switch(
                ui,
                out,
                &t!("Pause when minimized"),
                "",
                s.pause_on_minimize,
                Setting::PauseOnMinimize,
            );
            switch(
                ui,
                out,
                &t!("Remember volume"),
                &t!("Each video starts at the volume the last one ended with."),
                s.remember_volume,
                Setting::RememberVolume,
            );
            switch(
                ui,
                out,
                &t!("Play the next episode automatically"),
                &t!(
                    "When an episode ends, the next one plays from the same addon, or its streams open."
                ),
                s.binge_watching,
                Setting::BingeWatching,
            );
            pick(
                ui,
                out,
                &t!("Next episode notice"),
                &t!("How long before the end the next episode is offered."),
                s.next_video_notice,
                NextVideoNotice::ALL,
                notice_name,
                Setting::NextVideoNotice,
            );
        }
        Section::Languages => {
            language(
                ui,
                out,
                &t!("Audio language"),
                &t!("The file's track in this language plays; otherwise its default track."),
                s.audio_language,
                Setting::AudioLanguage,
            );
            language(
                ui,
                out,
                &t!("Second audio language"),
                &t!("Used when the file has no audio in the first."),
                s.secondary_audio_language,
                Setting::SecondaryAudioLanguage,
            );
            language(
                ui,
                out,
                &t!("Subtitle language"),
                &t!(
                    "Turned on when a video starts: from the file if it has them, otherwise from a subtitles addon."
                ),
                s.subtitle_language,
                Setting::SubtitleLanguage,
            );
            language(
                ui,
                out,
                &t!("Second subtitle language"),
                &t!("Used when no subtitle is in the first."),
                s.secondary_subtitle_language,
                Setting::SecondarySubtitleLanguage,
            );
        }
        Section::Subtitles => subtitle_style(ui, s, out),
        Section::Audio => {
            pick(
                ui,
                out,
                &t!("Audio output"),
                &t!("Stereo mixes surround sound down to two speakers or headphones."),
                s.audio_output,
                AudioOutput::ALL,
                output_name,
                Setting::AudioOutput,
            );
            switch(
                ui,
                out,
                &t!("Passthrough"),
                &t!(
                    "Sends Dolby and DTS audio undecoded to a receiver over HDMI or S/PDIF. Leave off unless your receiver decodes them, or you may hear silence."
                ),
                s.audio_passthrough,
                Setting::AudioPassthrough,
            );
        }
        Section::Torrents => torrents(ui, s, out),
        Section::Data => {
            pick(
                ui,
                out,
                &t!("Count as watched at"),
                &t!("Watched videos start over and leave Continue Watching."),
                s.watched_at,
                WatchedAt::ALL,
                watched_name,
                Setting::WatchedAt,
            );
            row(
                ui,
                &t!("Clear watch history"),
                &t!("Empties the Library and Continue Watching."),
                |ui| {
                    if ui.button(t!("Clear…")).clicked() {
                        view.confirm = Some(Confirm::ClearLibrary);
                    }
                },
            );
            row(
                ui,
                &t!("Reset all settings"),
                &t!("Every setting on this page goes back to its default."),
                |ui| {
                    if ui.button(t!("Reset…")).clicked() {
                        view.confirm = Some(Confirm::ResetSettings);
                    }
                },
            );
        }
        Section::Keyboard => keyboard(ui),
        Section::About => {
            ui.label(dim(&t!(
                "Cineo v{version}",
                version = env!("CARGO_PKG_VERSION")
            )));
            ui.add_space(theme::GAP);
            switch(
                ui,
                out,
                &t!("Check for updates"),
                &t!("Asks GitHub once per start whether a newer version exists."),
                s.check_for_updates,
                Setting::CheckForUpdates,
            );
            ui.add_space(theme::GAP);
            let mut job = LayoutJob::default();
            append_icon(&mut job, Icon::Named("brand-github"), 18.0, theme::ACCENT);
            job.append(
                "GitHub",
                6.0,
                TextFormat::simple(theme::body(), theme::ACCENT),
            );
            ui.hyperlink_to(job, "https://github.com/mdmrk/cineo");
            ui.add_space(theme::GAP);
            ui.label(dim(&t!("Cineo is free software under the MIT License.")));
            ui.label(dim(&t!("Made with Rust, egui, mpv and librqbit.")));
            ui.add_space(theme::SECTION_GAP * 4.0);
            let height = theme::LOADING_WIDTH * brand::LOADING_SIZE.y / brand::LOADING_SIZE.x;
            let (rect, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
            paint_loading(
                ui,
                pos2(center_x, rect.center().y),
                theme::LOADING_WIDTH,
                theme::TEXT_FAINT,
            );
        }
    }
}

fn torrents(ui: &mut Ui, s: &Settings, out: &mut Vec<Action>) {
    switch(
        ui,
        out,
        &t!("Show and play torrent streams"),
        &t!(
            "Torrent streams come from other people's computers. While one plays, your IP address is visible to the peers and trackers it connects to, and Cineo uploads the parts it has already downloaded to those peers. Downloaded data is kept in a local cache. You can turn peer-to-peer streaming off in Settings."
        ),
        s.p2p_enabled,
        Setting::P2pEnabled,
    );
    switch(
        ui,
        out,
        &t!("Upload to other peers"),
        &t!(
            "When off, Cineo only downloads. Some peers then send less, so torrents can be slower. Your IP address is still visible to peers."
        ),
        s.torrent_upload,
        Setting::TorrentUpload,
    );
    pick(
        ui,
        out,
        &t!("Torrent data"),
        &t!(
            "Cineo keeps only the part around the playback position, up to the size chosen, in memory or on disk. Seeking back past it downloads that part again. Nothing is kept as a cache: the data is deleted when playback stops. Applies from the next torrent you play."
        ),
        s.torrent_storage,
        TorrentStorage::ALL,
        storage_name,
        Setting::TorrentStorage,
    );
    pick(
        ui,
        out,
        &t!("Download limit"),
        &t!("Applies from the next torrent you play."),
        s.download_limit,
        DownloadLimit::ALL,
        download_name,
        Setting::DownloadLimit,
    );
    pick(
        ui,
        out,
        &t!("Upload limit"),
        &t!("Applies from the next torrent you play."),
        s.upload_limit,
        UploadLimit::ALL,
        upload_name,
        Setting::UploadLimit,
    );
    pick(
        ui,
        out,
        &t!("Peers per torrent"),
        &t!("More peers can be faster but use more connections."),
        s.peer_limit,
        PeerLimit::ALL,
        peer_name,
        Setting::PeerLimit,
    );
    switch(
        ui,
        out,
        &t!("Find peers through the DHT"),
        &t!("Finds peers without trackers. Off means fewer peers for many torrents."),
        s.torrent_dht,
        Setting::TorrentDht,
    );
    row(
        ui,
        &t!("Listening port"),
        &t!(
            "Peers can connect to Cineo on this port, over TCP and uTP. Forward it on your router for more peers. If it is taken, a random port is used. Applies from the next torrent you play."
        ),
        |ui| {
            if let Some(port) = port_field(ui, &t!("Listening port"), s.torrent_port) {
                out.push(Action::ChangeSetting(Setting::TorrentPort(port)));
            }
        },
    );
    switch(
        ui,
        out,
        &t!("Allow local network addresses"),
        &t!(
            "Lets addons, images and torrent peers on 127.0.0.1 or your home network be reached, for self-hosted addons. Off is safer. Takes effect after restarting Cineo."
        ),
        s.allow_private_network,
        Setting::AllowPrivateNetwork,
    );
}

fn ui_languages() -> Vec<UiLanguage> {
    let mut languages = UiLanguage::ALL.to_vec();
    languages.sort_by_cached_key(|&l| {
        (
            l != UiLanguage::System,
            sort_key(&ui_language_name(l, Locale::English)),
        )
    });
    languages
}

fn ui_language_name(language: UiLanguage, system: Locale) -> Text {
    match language {
        UiLanguage::System => t!("System ({language})", language = system.endonym()),
        other => Locale::resolve(other, system).endonym().into(),
    }
}

fn output_name(output: AudioOutput) -> Text {
    match output {
        AudioOutput::Auto => t!("Automatic (surround)"),
        AudioOutput::Stereo => t!("Stereo"),
    }
}

fn download_name(limit: DownloadLimit) -> Text {
    match limit {
        DownloadLimit::Unlimited => t!("No limit"),
        DownloadLimit::M1 => "1 MB/s".into(),
        DownloadLimit::M2 => "2 MB/s".into(),
        DownloadLimit::M5 => "5 MB/s".into(),
        DownloadLimit::M10 => "10 MB/s".into(),
        DownloadLimit::M20 => "20 MB/s".into(),
    }
}

fn storage_name(storage: TorrentStorage) -> Text {
    match storage {
        TorrentStorage::Ram256 => t!("In memory, {size}", size = "256 MB"),
        TorrentStorage::Ram512 => t!("In memory, {size}", size = "512 MB"),
        TorrentStorage::Ram1024 => t!("In memory, {size}", size = "1 GB"),
        TorrentStorage::Ram2048 => t!("In memory, {size}", size = "2 GB"),
        TorrentStorage::Disk1024 => t!("On disk, {size}", size = "1 GB"),
        TorrentStorage::Disk2048 => t!("On disk, {size}", size = "2 GB"),
        TorrentStorage::Disk4096 => t!("On disk, {size}", size = "4 GB"),
        TorrentStorage::Disk8192 => t!("On disk, {size}", size = "8 GB"),
    }
}

fn upload_name(limit: UploadLimit) -> Text {
    match limit {
        UploadLimit::Unlimited => t!("No limit"),
        UploadLimit::K100 => "100 kB/s".into(),
        UploadLimit::K500 => "500 kB/s".into(),
        UploadLimit::M1 => "1 MB/s".into(),
        UploadLimit::M5 => "5 MB/s".into(),
    }
}

fn peer_name(limit: PeerLimit) -> Text {
    match limit {
        PeerLimit::P50 => "50",
        PeerLimit::P128 => "128",
        PeerLimit::P200 => "200",
    }
    .into()
}

fn scale_name(scale: InterfaceScale) -> Text {
    format!("{} %", scale.percent()).into()
}

fn start_name(page: StartPage) -> Text {
    Page::from(page).label()
}

#[expect(clippy::cast_possible_truncation, reason = "a whole percentage")]
fn watched_name(at: WatchedAt) -> Text {
    t!(
        "{percent} % played",
        percent = (at.fraction() * 100.0).round() as u32
    )
}

fn notice_name(notice: NextVideoNotice) -> Text {
    match notice {
        NextVideoNotice::Off => pt!("next-notice-off", "Off"),
        _ => seconds(notice.seconds()),
    }
}

fn seconds(count: f64) -> Text {
    t!("{count} s", count = count)
}

fn sort_key(name: &str) -> String {
    name.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            other => other,
        })
        .collect()
}

fn language_name(language: Option<Language>) -> Text {
    language.map_or_else(|| t!("None"), |l| crate::i18n::language_name(l.code()))
}

fn language(
    ui: &mut Ui,
    out: &mut Vec<Action>,
    title: &str,
    help: &str,
    current: Option<Language>,
    setting: fn(Option<Language>) -> Setting,
) {
    row(ui, title, help, |ui| {
        let options = || {
            let mut languages: Vec<Language> = Language::all().collect();
            languages.sort_by_cached_key(|l| sort_key(&language_name(Some(*l))));
            std::iter::once(None).chain(languages.into_iter().map(Some))
        };
        if let Some(value) = select(ui, title, current, options, language_name) {
            out.push(Action::ChangeSetting(setting(value)));
        }
    });
}

fn nav_item(ui: &mut Ui, part: Section, selected: bool) -> Response {
    let label = part.label();
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
    response.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, selected, &label));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if selected || response.hovered() {
            let fill = if selected {
                theme::SURFACE
            } else {
                theme::PANEL
            };
            painter.rect_filled(rect, CornerRadius::same(theme::RADIUS), fill);
        }
        let color = if selected || response.hovered() {
            theme::TEXT_BRIGHT
        } else {
            theme::TEXT_DIM
        };
        paint_nav_label(painter, rect, part.icon(), 16.0, &label, color);
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn keyboard(ui: &mut Ui) {
    egui::Grid::new("shortcuts")
        .num_columns(2)
        .spacing(vec2(32.0, 10.0))
        .show(ui, |ui| {
            let key = TextFormat::simple(theme::body(), theme::TEXT_BRIGHT);
            let plain = |text: &str| LayoutJob::single_section(text.to_owned(), key.clone());
            let or = |a: &str, b: &str| plain(&t!("{a}  or  {b}", a = a, b = b));
            let mut back = plain("Esc  ·  Alt+");
            append_icon(&mut back, Icon::ArrowLeft, 15.0, theme::TEXT_BRIGHT);
            back.append(&format!("  ·  {}", t!("mouse back")), 0.0, key.clone());
            let rows = [
                (or("Ctrl+F", "/"), pt!("shortcut-search", "Search")),
                (plain("Ctrl+1 … Ctrl+6"), t!("Switch page")),
                (back, t!("Leave a detail page")),
                (plain(&t!("Shift+wheel")), t!("Scroll a row sideways")),
                (or(&t!("Space"), "K"), t!("Play or pause")),
                (or("←/→", "J/L"), t!("Seek by the seek step")),
                (plain("Shift+←/→"), t!("Seek by the short seek step")),
                (plain("↑/↓  ·  M"), t!("Volume  ·  mute")),
                (or("F", "F11"), t!("Fullscreen")),
            ];
            for (keys, what) in rows {
                ui.label(keys);
                ui.label(dim(&what));
                ui.end_row();
            }
        });
}

fn confirm(ui: &Ui, view: &mut ViewState, out: &mut Vec<Action>) {
    let Some(question) = view.confirm else {
        return;
    };
    let (title, text, button, action) = match question {
        Confirm::ResetSettings => (
            t!("Reset all settings?"),
            t!("Every setting goes back to its default. Your addons and library are kept."),
            t!("Reset"),
            Action::ResetSettings,
        ),
        Confirm::ClearLibrary => (
            t!("Clear watch history?"),
            t!("Every item and its progress leaves the Library. This cannot be undone."),
            pt!("clear", "Clear"),
            Action::ClearLibrary,
        ),
    };
    if let Some(accepted) = dialog(ui, "settings_confirm", &title, &text, None, &button) {
        if accepted {
            out.push(action);
        }
        view.confirm = None;
    }
}

#[cfg(test)]
mod tests {
    use crate::i18n::{self, Locale, Text};

    #[test]
    fn interface_languages_list_the_system_first_then_by_name() {
        let names: Vec<Text> = super::ui_languages()
            .into_iter()
            .map(|l| super::ui_language_name(l, Locale::English))
            .collect();
        let names: Vec<&str> = names.iter().map(|n| n.as_ref()).collect();
        assert_eq!(
            names[1..],
            [
                "Deutsch",
                "English",
                "Español de España",
                "Español de Latinoamérica",
                "Français",
                "Italiano",
                "Português do Brasil",
                "Русский",
                "简体中文",
            ]
        );
        assert_eq!(
            super::ui_languages()[0],
            cineo_core::app::UiLanguage::System
        );
    }

    #[test]
    fn every_section_icon_exists_in_the_tabler_set() {
        for part in super::Section::ALL {
            assert!(part.icon().glyph().is_some(), "{part:?}");
        }
    }

    #[test]
    fn languages_sort_by_their_translated_name_ignoring_accents() {
        i18n::set(Locale::Spanish);
        let mut names: Vec<Text> = cineo_core::app::Language::all()
            .map(|l| super::language_name(Some(l)))
            .collect();
        names.sort_by_cached_key(|n| super::sort_key(n));
        assert_eq!(names.first().map(|n| &**n), Some("Alemán"));
        assert_eq!(names.get(1).map(|n| &**n), Some("Árabe"));
    }
}
