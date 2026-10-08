use cineo_core::app::{
    Action, AudioOutput, DownloadLimit, HideControls, InterfaceScale, Language, NextVideoNotice,
    PeerLimit, SeekStep, Setting, Settings, ShortSeekStep, StartPage, State, SubtitleBackground,
    SubtitleColor, SubtitleFont, SubtitleOpacity, SubtitleOutline, SubtitlePosition, SubtitleSize,
    UiLanguage, UploadLimit, WatchedAt,
};
use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontFamily, FontId, Frame, Label, Layout, Margin,
    Modal, Popup, PopupCloseBehavior, Rect, Response, RichText, ScrollArea, Sense, Stroke,
    TextFormat, Ui, UiBuilder, WidgetInfo, WidgetType, pos2, text::LayoutJob, vec2,
};

use crate::brand;
use crate::i18n::{Locale, t};
use crate::theme;
use crate::view::{
    Icon, Page, ViewState, append_icon, compact, dim, lerp_color, page_title, paint_icon,
    paint_loading, primary, section,
};

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

    fn label(self) -> String {
        match self {
            Self::Interface => t!("section-interface"),
            Self::Player => t!("section-player"),
            Self::Languages => t!("section-languages"),
            Self::Subtitles => t!("section-subtitles"),
            Self::Audio => t!("section-audio"),
            Self::Torrents => t!("section-torrents"),
            Self::Data => t!("section-data"),
            Self::Keyboard => t!("section-keyboard"),
            Self::About => t!("section-about"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirm {
    ResetSettings,
    ClearLibrary,
}

const NAV_WIDTH: f32 = 168.0;
const CONTROL_WIDTH: f32 = 220.0;

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
                        page_title(ui, &t!("page-settings"), None);
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
                            body(ui, part, state, view, out);
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
                if nav_item(ui, &part.label(), part == jump.unwrap_or(current)).clicked() {
                    view.settings_jump = Some(part);
                }
            }
        });
    }
    confirm(ui, view, out);
}

fn body(ui: &mut Ui, section: Section, state: &State, view: &mut ViewState, out: &mut Vec<Action>) {
    let s = &state.settings;
    match section {
        Section::Interface => {
            let system = view.system_locale;
            pick(
                ui,
                out,
                &t!("ui-language"),
                &t!("ui-language-help"),
                s.ui_language,
                UiLanguage::ALL,
                |language| ui_language_name(language, system),
                Setting::UiLanguage,
            );
            pick(
                ui,
                out,
                &t!("interface-size"),
                &t!("interface-size-help"),
                s.interface_scale,
                InterfaceScale::ALL,
                scale_name,
                Setting::InterfaceScale,
            );
            pick(
                ui,
                out,
                &t!("start-page"),
                &t!("start-page-help"),
                s.start_page,
                StartPage::ALL,
                start_name,
                Setting::StartPage,
            );
        }
        Section::Player => {
            switch(
                ui,
                out,
                &t!("hardware-decoding"),
                &t!("hardware-decoding-help"),
                s.hardware_decoding,
                Setting::HardwareDecoding,
            );
            pick(
                ui,
                out,
                &t!("seek-step"),
                &t!("seek-step-help"),
                s.seek_step,
                SeekStep::ALL,
                seek_name,
                Setting::SeekStep,
            );
            pick(
                ui,
                out,
                &t!("short-seek-step"),
                &t!("short-seek-step-help"),
                s.short_seek_step,
                ShortSeekStep::ALL,
                short_seek_name,
                Setting::ShortSeekStep,
            );
            pick(
                ui,
                out,
                &t!("hide-controls"),
                &t!("hide-controls-help"),
                s.hide_controls,
                HideControls::ALL,
                hide_name,
                Setting::HideControls,
            );
            switch(
                ui,
                out,
                &t!("escape-fullscreen"),
                &t!("escape-fullscreen-help"),
                s.escape_leaves_fullscreen,
                Setting::EscapeLeavesFullscreen,
            );
            switch(
                ui,
                out,
                &t!("pause-minimized"),
                "",
                s.pause_on_minimize,
                Setting::PauseOnMinimize,
            );
            switch(
                ui,
                out,
                &t!("remember-volume"),
                &t!("remember-volume-help"),
                s.remember_volume,
                Setting::RememberVolume,
            );
            switch(
                ui,
                out,
                &t!("binge-watching"),
                &t!("binge-watching-help"),
                s.binge_watching,
                Setting::BingeWatching,
            );
            pick(
                ui,
                out,
                &t!("next-notice"),
                &t!("next-notice-help"),
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
                &t!("audio-language"),
                &t!("audio-language-help"),
                s.audio_language,
                Setting::AudioLanguage,
            );
            language(
                ui,
                out,
                &t!("audio-language-second"),
                &t!("audio-language-second-help"),
                s.secondary_audio_language,
                Setting::SecondaryAudioLanguage,
            );
            language(
                ui,
                out,
                &t!("subtitle-language"),
                &t!("subtitle-language-help"),
                s.subtitle_language,
                Setting::SubtitleLanguage,
            );
            language(
                ui,
                out,
                &t!("subtitle-language-second"),
                &t!("subtitle-language-second-help"),
                s.secondary_subtitle_language,
                Setting::SecondarySubtitleLanguage,
            );
        }
        Section::Subtitles => subtitle_style(ui, s, out),
        Section::Audio => {
            pick(
                ui,
                out,
                &t!("audio-output"),
                &t!("audio-output-help"),
                s.audio_output,
                AudioOutput::ALL,
                output_name,
                Setting::AudioOutput,
            );
            switch(
                ui,
                out,
                &t!("passthrough"),
                &t!("passthrough-help"),
                s.audio_passthrough,
                Setting::AudioPassthrough,
            );
        }
        Section::Torrents => torrents(ui, s, out),
        Section::Data => {
            pick(
                ui,
                out,
                &t!("watched-at"),
                &t!("watched-at-help"),
                s.watched_at,
                WatchedAt::ALL,
                watched_name,
                Setting::WatchedAt,
            );
            row(ui, &t!("clear-history"), &t!("clear-history-help"), |ui| {
                if ui.button(t!("clear-ellipsis")).clicked() {
                    view.confirm = Some(Confirm::ClearLibrary);
                }
            });
            row(
                ui,
                &t!("reset-settings"),
                &t!("reset-settings-help"),
                |ui| {
                    if ui.button(t!("reset-ellipsis")).clicked() {
                        view.confirm = Some(Confirm::ResetSettings);
                    }
                },
            );
        }
        Section::Keyboard => keyboard(ui),
        Section::About => {
            ui.label(dim(&t!("app-version", version = env!("CARGO_PKG_VERSION"))));
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
            ui.label(dim(&t!("about-license")));
            ui.label(dim(&t!("about-credits")));
            ui.add_space(theme::SECTION_GAP * 2.0);
            let height = theme::LOADING_WIDTH * brand::LOADING_SIZE.y / brand::LOADING_SIZE.x;
            let (rect, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
            paint_loading(ui, rect.center(), theme::LOADING_WIDTH, theme::TEXT_FAINT);
        }
    }
}

fn subtitle_style(ui: &mut Ui, s: &Settings, out: &mut Vec<Action>) {
    preview(ui, s);
    ui.add_space(theme::GAP);
    subtitle_controls(ui, s, out);
}

pub(crate) fn subtitle_controls(ui: &mut Ui, s: &Settings, out: &mut Vec<Action>) {
    let size = s.subtitle_size;
    number(
        ui,
        out,
        &t!("subtitle-size"),
        "",
        size.get(),
        (SubtitleSize::MIN, SubtitleSize::MAX, 5),
        |v| Setting::SubtitleSize(SubtitleSize::new(v)),
    );
    pick(
        ui,
        out,
        &t!("subtitle-font"),
        "",
        s.subtitle_font,
        SubtitleFont::ALL,
        font_name,
        Setting::SubtitleFont,
    );
    switch(
        ui,
        out,
        &t!("subtitle-bold"),
        "",
        s.subtitle_bold,
        Setting::SubtitleBold,
    );
    pick(
        ui,
        out,
        &t!("subtitle-color"),
        "",
        s.subtitle_color,
        SubtitleColor::ALL,
        color_name,
        Setting::SubtitleColor,
    );
    let opacity = s.subtitle_opacity;
    number(
        ui,
        out,
        &t!("subtitle-opacity"),
        "",
        opacity.get(),
        (SubtitleOpacity::MIN, SubtitleOpacity::MAX, 5),
        |v| Setting::SubtitleOpacity(SubtitleOpacity::new(v)),
    );
    pick(
        ui,
        out,
        &t!("subtitle-outline"),
        &t!("subtitle-outline-help"),
        s.subtitle_outline,
        SubtitleOutline::ALL,
        outline_name,
        Setting::SubtitleOutline,
    );
    pick(
        ui,
        out,
        &t!("subtitle-background"),
        "",
        s.subtitle_background,
        SubtitleBackground::ALL,
        background_name,
        Setting::SubtitleBackground,
    );
    let position = s.subtitle_position;
    number(
        ui,
        out,
        &t!("subtitle-raise"),
        &t!("subtitle-raise-help"),
        position.get(),
        (SubtitlePosition::MIN, SubtitlePosition::MAX, 1),
        |v| Setting::SubtitlePosition(SubtitlePosition::new(v)),
    );
    switch(
        ui,
        out,
        &t!("subtitle-keep-styles"),
        &t!("subtitle-keep-styles-help"),
        s.keep_subtitle_styles,
        Setting::KeepSubtitleStyles,
    );
}

fn preview(ui: &mut Ui, s: &Settings) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, 150.0), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let painter = ui.painter();
    painter.rect_filled(
        rect,
        CornerRadius::same(theme::RADIUS),
        Color32::from_gray(24),
    );
    let family = match (s.subtitle_font, s.subtitle_bold) {
        (SubtitleFont::Sans, false) => FontFamily::Proportional,
        (SubtitleFont::Sans, true) => theme::strong_family(),
        (SubtitleFont::Serif, _) => theme::serif_family(),
        (SubtitleFont::Mono, _) => FontFamily::Monospace,
    };
    #[expect(clippy::cast_precision_loss, reason = "a percentage")]
    let scale = s.subtitle_size.get() as f32 / 100.0;
    #[expect(clippy::cast_precision_loss, reason = "a percentage")]
    let raise = s.subtitle_position.get() as f32 / 100.0;
    let [r, g, b] = s.subtitle_color.rgb();
    let alpha = u8::try_from(s.subtitle_opacity.get() * 255 / 100).unwrap_or(u8::MAX);
    let ink = Color32::from_rgba_unmultiplied(r, g, b, alpha);
    let galley = painter.layout_no_wrap(
        t!("subtitle-preview"),
        FontId::new(22.0 * scale, family),
        ink,
    );
    let bottom = rect.bottom() - 14.0 - rect.height() * raise;
    let pos = pos2(
        rect.center().x - galley.size().x / 2.0,
        bottom - galley.size().y,
    );
    let text_rect = Rect::from_min_size(pos, galley.size());
    if let Some([r, g, b]) = s.subtitle_background.rgb() {
        painter.rect_filled(
            text_rect.expand(4.0),
            0.0,
            Color32::from_rgba_unmultiplied(r, g, b, 0xCC),
        );
    } else if let Some([r, g, b]) = s.subtitle_outline.rgb() {
        let outline = Color32::from_rgb(r, g, b);
        for (dx, dy) in [
            (-1.5, 0.0),
            (1.5, 0.0),
            (0.0, -1.5),
            (0.0, 1.5),
            (-1.0, -1.0),
            (1.0, 1.0),
            (-1.0, 1.0),
            (1.0, -1.0),
        ] {
            painter.galley_with_override_text_color(pos + vec2(dx, dy), galley.clone(), outline);
        }
    }
    painter.galley(pos, galley, ink);
    painter.text(
        rect.left_top() + vec2(10.0, 8.0),
        Align2::LEFT_TOP,
        t!("subtitle-preview-badge"),
        theme::caption(),
        theme::TEXT_FAINT,
    );
}

fn font_name(font: SubtitleFont) -> String {
    match font {
        SubtitleFont::Sans => t!("font-sans"),
        SubtitleFont::Serif => t!("font-serif"),
        SubtitleFont::Mono => t!("font-mono"),
    }
}

fn color_name(color: SubtitleColor) -> String {
    match color {
        SubtitleColor::White => t!("color-white"),
        SubtitleColor::Yellow => t!("color-yellow"),
        SubtitleColor::Cyan => t!("color-cyan"),
        SubtitleColor::Green => t!("color-green"),
    }
}

fn outline_name(outline: SubtitleOutline) -> String {
    match outline {
        SubtitleOutline::Black => t!("outline-black"),
        SubtitleOutline::Gray => t!("outline-gray"),
        SubtitleOutline::None => t!("outline-none"),
    }
}

fn background_name(background: SubtitleBackground) -> String {
    match background {
        SubtitleBackground::None => t!("background-none"),
        SubtitleBackground::Black => t!("background-black"),
        SubtitleBackground::Gray => t!("background-gray"),
    }
}

fn torrents(ui: &mut Ui, s: &Settings, out: &mut Vec<Action>) {
    switch(
        ui,
        out,
        &t!("p2p-enabled"),
        &t!("p2p-notice"),
        s.p2p_enabled,
        Setting::P2pEnabled,
    );
    switch(
        ui,
        out,
        &t!("torrent-upload"),
        &t!("torrent-upload-help"),
        s.torrent_upload,
        Setting::TorrentUpload,
    );
    pick(
        ui,
        out,
        &t!("download-limit"),
        &t!("next-torrent"),
        s.download_limit,
        DownloadLimit::ALL,
        download_name,
        Setting::DownloadLimit,
    );
    pick(
        ui,
        out,
        &t!("upload-limit"),
        &t!("next-torrent"),
        s.upload_limit,
        UploadLimit::ALL,
        upload_name,
        Setting::UploadLimit,
    );
    pick(
        ui,
        out,
        &t!("peer-limit"),
        &t!("peer-limit-help"),
        s.peer_limit,
        PeerLimit::ALL,
        peer_name,
        Setting::PeerLimit,
    );
    switch(
        ui,
        out,
        &t!("dht"),
        &t!("dht-help"),
        s.torrent_dht,
        Setting::TorrentDht,
    );
    switch(
        ui,
        out,
        &t!("private-network"),
        &t!("private-network-help"),
        s.allow_private_network,
        Setting::AllowPrivateNetwork,
    );
}

fn ui_language_name(language: UiLanguage, system: Locale) -> String {
    match language {
        UiLanguage::System => t!("ui-language-system", language = system.endonym()),
        UiLanguage::English => Locale::English.endonym().to_owned(),
        UiLanguage::Spanish => Locale::Spanish.endonym().to_owned(),
    }
}

fn output_name(output: AudioOutput) -> String {
    match output {
        AudioOutput::Auto => t!("output-auto"),
        AudioOutput::Stereo => t!("output-stereo"),
    }
}

fn download_name(limit: DownloadLimit) -> String {
    match limit {
        DownloadLimit::Unlimited => t!("no-limit"),
        DownloadLimit::M1 => "1 MB/s".to_owned(),
        DownloadLimit::M2 => "2 MB/s".to_owned(),
        DownloadLimit::M5 => "5 MB/s".to_owned(),
        DownloadLimit::M10 => "10 MB/s".to_owned(),
        DownloadLimit::M20 => "20 MB/s".to_owned(),
    }
}

fn upload_name(limit: UploadLimit) -> String {
    match limit {
        UploadLimit::Unlimited => t!("no-limit"),
        UploadLimit::K100 => "100 kB/s".to_owned(),
        UploadLimit::K500 => "500 kB/s".to_owned(),
        UploadLimit::M1 => "1 MB/s".to_owned(),
        UploadLimit::M5 => "5 MB/s".to_owned(),
    }
}

fn peer_name(limit: PeerLimit) -> String {
    match limit {
        PeerLimit::P50 => "50",
        PeerLimit::P128 => "128",
        PeerLimit::P200 => "200",
    }
    .to_owned()
}

fn scale_name(scale: InterfaceScale) -> String {
    format!("{} %", scale.percent())
}

fn start_name(page: StartPage) -> String {
    Page::from(page).label()
}

#[expect(clippy::cast_possible_truncation, reason = "a whole percentage")]
fn watched_name(at: WatchedAt) -> String {
    t!(
        "watched-percent",
        percent = (at.fraction() * 100.0).round() as u32
    )
}

fn notice_name(notice: NextVideoNotice) -> String {
    match notice {
        NextVideoNotice::Off => t!("next-notice-off"),
        _ => seconds(notice.seconds()),
    }
}

fn seconds(count: f64) -> String {
    t!("seconds", count = count)
}

fn seek_name(step: SeekStep) -> String {
    seconds(step.seconds())
}

fn short_seek_name(step: ShortSeekStep) -> String {
    seconds(step.seconds())
}

fn hide_name(after: HideControls) -> String {
    seconds(after.seconds())
}

fn switch(
    ui: &mut Ui,
    out: &mut Vec<Action>,
    title: &str,
    help: &str,
    on: bool,
    setting: fn(bool) -> Setting,
) {
    row(ui, title, help, |ui| {
        if toggle(ui, title, on) {
            out.push(Action::ChangeSetting(setting(!on)));
        }
    });
}

#[expect(
    clippy::too_many_arguments,
    reason = "one call per setting reads best flat"
)]
fn pick<T: Copy + PartialEq>(
    ui: &mut Ui,
    out: &mut Vec<Action>,
    title: &str,
    help: &str,
    current: T,
    options: &[T],
    name: impl Fn(T) -> String,
    setting: fn(T) -> Setting,
) {
    row(ui, title, help, |ui| {
        if let Some(value) = select(ui, title, current, options.iter().copied(), name) {
            out.push(Action::ChangeSetting(setting(value)));
        }
    });
}

fn number(
    ui: &mut Ui,
    out: &mut Vec<Action>,
    title: &str,
    help: &str,
    value: u32,
    range: (u32, u32, u32),
    setting: fn(u32) -> Setting,
) {
    row(ui, title, help, |ui| {
        if let Some(value) = slider(ui, title, value, range) {
            out.push(Action::ChangeSetting(setting(value)));
        }
    });
}

#[expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "small whole numbers within the range"
)]
fn slider(ui: &mut Ui, label: &str, value: u32, (min, max, step): (u32, u32, u32)) -> Option<u32> {
    let width = ui.available_width().min(CONTROL_WIDTH);
    let (rect, response) = ui.allocate_exact_size(vec2(width, 30.0), Sense::click_and_drag());
    response.widget_info(|| WidgetInfo::slider(ui.is_enabled(), f64::from(value), label));
    let track = Rect::from_min_max(
        pos2(rect.min.x + 8.0, rect.center().y - 2.0),
        pos2(rect.max.x - 60.0, rect.center().y + 2.0),
    );
    let mut new = value;
    if let Some(pointer) = response.interact_pointer_pos() {
        let t = ((pointer.x - track.min.x) / track.width()).clamp(0.0, 1.0);
        let raw = min as f32 + t * (max - min) as f32;
        new = ((raw / step as f32).round() as u32 * step).clamp(min, max);
    }
    if response.has_focus() {
        let (down, up) = ui.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowLeft) || i.key_pressed(egui::Key::ArrowDown),
                i.key_pressed(egui::Key::ArrowRight) || i.key_pressed(egui::Key::ArrowUp),
            )
        });
        if down {
            new = new.saturating_sub(step).max(min);
        }
        if up {
            new = (new + step).min(max);
        }
    }
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let t = (new - min) as f32 / (max - min) as f32;
        let knob = pos2(egui::lerp(track.x_range(), t), track.center().y);
        painter.rect_filled(track, CornerRadius::same(2), theme::SURFACE_HOVER);
        painter.rect_filled(
            Rect::from_min_max(track.min, pos2(knob.x, track.max.y)),
            CornerRadius::same(2),
            theme::ACCENT,
        );
        let knob_color = if response.hovered() || response.dragged() || response.has_focus() {
            theme::TEXT_BRIGHT
        } else {
            theme::TEXT
        };
        painter.circle_filled(knob, 7.0, knob_color);
        painter.text(
            pos2(rect.max.x, rect.center().y),
            Align2::RIGHT_CENTER,
            format!("{new} %"),
            theme::body(),
            theme::TEXT_BRIGHT,
        );
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand);
    (new != value).then_some(new)
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

fn language_name(language: Option<Language>) -> String {
    language.map_or_else(
        || t!("language-none"),
        |l| t!(&format!("language-{}", l.code())),
    )
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
        let mut languages: Vec<Language> = Language::all().collect();
        languages.sort_by_cached_key(|l| sort_key(&language_name(Some(*l))));
        let options = std::iter::once(None).chain(languages.into_iter().map(Some));
        if let Some(value) = select(ui, title, current, options, language_name) {
            out.push(Action::ChangeSetting(setting(value)));
        }
    });
}

pub(crate) fn row(ui: &mut Ui, title: &str, help: &str, control: impl FnOnce(&mut Ui)) {
    let width = ui.available_width();
    let control_width = CONTROL_WIDTH.min(width * 0.45);
    let text_width = (width - control_width - 24.0).max(0.0);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(text_width);
            let galley = ui.painter().layout(
                title.to_owned(),
                theme::strong(),
                theme::TEXT_BRIGHT,
                text_width,
            );
            let (rect, _) = ui.allocate_exact_size(galley.size(), Sense::hover());
            ui.painter().galley(rect.min, galley, theme::TEXT_BRIGHT);
            if !help.is_empty() {
                ui.add(Label::new(dim(help).small()).wrap());
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
            ui.set_width(control_width);
            control(ui);
        });
    });
    ui.add_space(theme::GAP);
}

fn toggle(ui: &mut Ui, label: &str, on: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(40.0, 22.0), Sense::click());
    response.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, ui.is_enabled(), on, label));
    if ui.is_rect_visible(rect) {
        let t = ui
            .ctx()
            .animate_bool_with_time(response.id, on, theme::ANIM);
        let track = Rect::from_center_size(rect.center(), vec2(40.0, 20.0));
        let painter = ui.painter();
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
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

fn select<T: Copy + PartialEq>(
    ui: &mut Ui,
    label: &str,
    current: T,
    options: impl IntoIterator<Item = T>,
    name: impl Fn(T) -> String,
) -> Option<T> {
    let (rect, response) = ui.allocate_exact_size(
        vec2(ui.available_width().min(CONTROL_WIDTH), 30.0),
        Sense::click(),
    );
    response.widget_info(|| {
        let mut info = WidgetInfo::labeled(WidgetType::ComboBox, ui.is_enabled(), label);
        info.current_text_value = Some(name(current));
        info
    });
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let fill = if response.hovered() {
            theme::SURFACE_HOVER
        } else {
            theme::SURFACE
        };
        painter.rect_filled(rect, CornerRadius::same(theme::RADIUS), fill);
        painter.text(
            pos2(rect.min.x + 10.0, rect.center().y),
            Align2::LEFT_CENTER,
            name(current),
            theme::body(),
            theme::TEXT_BRIGHT,
        );
        paint_icon(
            painter,
            Icon::ChevronDown,
            pos2(rect.max.x - 16.0, rect.center().y),
            16.0,
            theme::TEXT_DIM,
        );
    }
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    let mut picked = None;
    Popup::from_toggle_button_response(&response)
        .width(rect.width())
        .close_behavior(PopupCloseBehavior::CloseOnClick)
        .show(|ui| {
            ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                for option in options {
                    let selected = option == current;
                    if ui.selectable_label(selected, name(option)).clicked() && !selected {
                        picked = Some(option);
                    }
                }
            });
        });
    picked
}

fn nav_item(ui: &mut Ui, label: &str, selected: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
    response.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, selected, label));
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
        painter.text(
            pos2(rect.min.x + 12.0, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            theme::nav(),
            color,
        );
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
            let or = |a: &str, b: &str| plain(&t!("key-or", a = a, b = b));
            let mut back = plain("Esc  ·  Alt+");
            append_icon(&mut back, Icon::ArrowLeft, 15.0, theme::TEXT_BRIGHT);
            back.append(&format!("  ·  {}", t!("key-mouse-back")), 0.0, key.clone());
            let rows = [
                (or("Ctrl+F", "/"), t!("shortcut-search")),
                (plain("Ctrl+1 … Ctrl+6"), t!("shortcut-switch-page")),
                (back, t!("shortcut-leave-detail")),
                (plain(&t!("key-shift-wheel")), t!("shortcut-scroll-row")),
                (or(&t!("key-space"), "K"), t!("shortcut-play-pause")),
                (or("←/→", "J/L"), t!("shortcut-seek")),
                (plain("Shift+←/→"), t!("shortcut-short-seek")),
                (plain("↑/↓  ·  M"), t!("shortcut-volume")),
                (or("F", "F11"), t!("shortcut-fullscreen")),
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
            t!("reset-confirm-title"),
            t!("reset-confirm-text"),
            t!("reset"),
            Action::ResetSettings,
        ),
        Confirm::ClearLibrary => (
            t!("clear-confirm-title"),
            t!("clear-confirm-text"),
            t!("clear"),
            Action::ClearLibrary,
        ),
    };
    let mut done = false;
    let modal = Modal::new(egui::Id::new("settings_confirm"))
        .frame(
            Frame::new()
                .fill(theme::PANEL)
                .stroke(Stroke::new(1.0, theme::RULE))
                .corner_radius(CornerRadius::same(theme::RADIUS))
                .inner_margin(Margin::same(20)),
        )
        .show(ui.ctx(), |ui| {
            ui.set_max_width(420.0);
            ui.label(RichText::new(title).font(theme::heading()));
            ui.add_space(theme::GAP);
            ui.add(Label::new(dim(&text)).wrap());
            ui.add_space(theme::GAP * 1.5);
            ui.horizontal(|ui| {
                if primary(ui, true, &button).clicked() {
                    out.push(action);
                    done = true;
                }
                if ui.button(t!("cancel")).clicked() {
                    done = true;
                }
            });
        });
    if done || modal.should_close() {
        view.confirm = None;
    }
}

#[cfg(test)]
mod tests {
    use crate::i18n::{self, Locale};

    #[test]
    fn languages_sort_by_their_translated_name_ignoring_accents() {
        i18n::set(Locale::Spanish);
        let mut names: Vec<String> = cineo_core::app::Language::all()
            .map(|l| super::language_name(Some(l)))
            .collect();
        names.sort_by_cached_key(|n| super::sort_key(n));
        assert_eq!(names.first().map(String::as_str), Some("Alemán"));
        assert_eq!(names.get(1).map(String::as_str), Some("Árabe"));
    }
}
