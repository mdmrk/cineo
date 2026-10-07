//! The Settings page: a section index beside one scrolling list of settings.

use cineo_core::app::{
    Action, AudioOutput, DownloadLimit, HideControls, InterfaceScale, Language, PeerLimit,
    SeekStep, Setting, Settings, ShortSeekStep, StartPage, State, SubtitleBackground,
    SubtitleColor, SubtitleFont, SubtitleOpacity, SubtitleOutline, SubtitlePosition, SubtitleSize,
    UploadLimit, WatchedAt,
};
use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontFamily, FontId, Frame, Label, Layout, Margin,
    Modal, Popup, PopupCloseBehavior, Rect, Response, RichText, ScrollArea, Sense, Stroke,
    TextFormat, Ui, UiBuilder, WidgetInfo, WidgetType, pos2, text::LayoutJob, vec2,
};

use crate::theme;
use crate::view::{
    Icon, P2P_NOTICE, ViewState, append_icon, caps, dim, lerp_color, page_margin, page_title,
    paint_icon, primary, section,
};

/// The parts of the Settings page, in page order.
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

    fn label(self) -> &'static str {
        match self {
            Self::Interface => "Interface",
            Self::Player => "Player",
            Self::Languages => "Languages",
            Self::Subtitles => "Subtitles",
            Self::Audio => "Audio",
            Self::Torrents => "Torrents",
            Self::Data => "Data",
            Self::Keyboard => "Keyboard",
            Self::About => "About",
        }
    }
}

/// A question asked before an action that cannot be undone.
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
    let margin = page_margin(ui);
    let show_nav = full.width() >= 900.0;
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
                let width = (ui.available_width() - margin).clamp(0.0, theme::CONTENT_MAX_WIDTH);
                ui.scope_builder(
                    UiBuilder::new().max_rect(Rect::from_min_size(
                        ui.cursor().min + vec2(if show_nav { 0.0 } else { margin }, 0.0),
                        vec2(width, f32::INFINITY),
                    )),
                    |ui| {
                        ui.set_width(width);
                        page_title(ui, "Settings", None);
                        let mut last_top = 0.0;
                        for part in Section::ALL {
                            let top = ui.cursor().min.y;
                            last_top = top;
                            section(ui, part.label(), |_| {});
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
                        // Room for the last section to scroll to the top, so it can be marked.
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
                if nav_item(ui, part.label(), part == jump.unwrap_or(current)).clicked() {
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
            pick(
                ui,
                out,
                "Interface size",
                "Makes text, posters and controls larger or smaller.",
                s.interface_scale,
                InterfaceScale::ALL,
                scale_name,
                Setting::InterfaceScale,
            );
            pick(
                ui,
                out,
                "Start page",
                "Shown when Cineo opens.",
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
                "Hardware decoding",
                HWDEC_HELP,
                s.hardware_decoding,
                Setting::HardwareDecoding,
            );
            pick(
                ui,
                out,
                "Seek step",
                "How far ←/→ and the seek buttons jump.",
                s.seek_step,
                SeekStep::ALL,
                seek_name,
                Setting::SeekStep,
            );
            pick(
                ui,
                out,
                "Short seek step",
                "How far Shift+←/→ jump.",
                s.short_seek_step,
                ShortSeekStep::ALL,
                short_seek_name,
                Setting::ShortSeekStep,
            );
            pick(
                ui,
                out,
                "Hide controls after",
                "Without mouse or key input while playing.",
                s.hide_controls,
                HideControls::ALL,
                hide_name,
                Setting::HideControls,
            );
            switch(
                ui,
                out,
                "Esc leaves fullscreen first",
                "When off, Esc leaves the player at once.",
                s.escape_leaves_fullscreen,
                Setting::EscapeLeavesFullscreen,
            );
            switch(
                ui,
                out,
                "Pause when minimized",
                "",
                s.pause_on_minimize,
                Setting::PauseOnMinimize,
            );
            switch(
                ui,
                out,
                "Remember volume",
                "Each video starts at the volume the last one ended with.",
                s.remember_volume,
                Setting::RememberVolume,
            );
        }
        Section::Languages => {
            language(
                ui,
                out,
                "Audio language",
                AUDIO_HELP,
                s.audio_language,
                Setting::AudioLanguage,
            );
            language(
                ui,
                out,
                "Second audio language",
                "Used when the file has no audio in the first.",
                s.secondary_audio_language,
                Setting::SecondaryAudioLanguage,
            );
            language(
                ui,
                out,
                "Subtitle language",
                SUBTITLE_HELP,
                s.subtitle_language,
                Setting::SubtitleLanguage,
            );
            language(
                ui,
                out,
                "Second subtitle language",
                "Used when no subtitle is in the first.",
                s.secondary_subtitle_language,
                Setting::SecondarySubtitleLanguage,
            );
        }
        Section::Subtitles => subtitle_style(ui, s, out),
        Section::Audio => {
            pick(
                ui,
                out,
                "Audio output",
                "Stereo mixes surround sound down to two speakers or headphones.",
                s.audio_output,
                AudioOutput::ALL,
                output_name,
                Setting::AudioOutput,
            );
            switch(
                ui,
                out,
                "Passthrough",
                PASSTHROUGH_HELP,
                s.audio_passthrough,
                Setting::AudioPassthrough,
            );
        }
        Section::Torrents => torrents(ui, s, out),
        Section::Data => {
            pick(
                ui,
                out,
                "Count as watched at",
                "Watched videos start over and leave Continue Watching.",
                s.watched_at,
                WatchedAt::ALL,
                watched_name,
                Setting::WatchedAt,
            );
            row(
                ui,
                "Clear watch history",
                "Empties the Library and Continue Watching.",
                |ui| {
                    if ui.button("Clear…").clicked() {
                        view.confirm = Some(Confirm::ClearLibrary);
                    }
                },
            );
            row(
                ui,
                "Reset all settings",
                "Every setting on this page goes back to its default.",
                |ui| {
                    if ui.button("Reset…").clicked() {
                        view.confirm = Some(Confirm::ResetSettings);
                    }
                },
            );
        }
        Section::Keyboard => keyboard(ui),
        Section::About => {
            ui.label(dim(&format!(
                "Cineo {}. Made by people who stay for the credits.",
                env!("CARGO_PKG_VERSION")
            )));
        }
    }
}

const HWDEC_HELP: &str = "Lets the graphics card decode video, which saves power. \
Turn it off if videos show artifacts or a black picture.";
const AUDIO_HELP: &str = "The file's track in this language plays; otherwise its default track.";
const SUBTITLE_HELP: &str = "Turned on when a video starts: from the file if it has \
them, otherwise from a subtitles addon.";

fn subtitle_style(ui: &mut Ui, s: &Settings, out: &mut Vec<Action>) {
    preview(ui, s);
    ui.add_space(theme::GAP);
    let size = s.subtitle_size;
    number(
        ui,
        out,
        "Size",
        "",
        size.get(),
        (SubtitleSize::MIN, SubtitleSize::MAX, 5),
        |v| Setting::SubtitleSize(SubtitleSize::new(v)),
    );
    pick(
        ui,
        out,
        "Font",
        "",
        s.subtitle_font,
        SubtitleFont::ALL,
        font_name,
        Setting::SubtitleFont,
    );
    switch(ui, out, "Bold", "", s.subtitle_bold, Setting::SubtitleBold);
    pick(
        ui,
        out,
        "Text color",
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
        "Text opacity",
        "",
        opacity.get(),
        (SubtitleOpacity::MIN, SubtitleOpacity::MAX, 5),
        |v| Setting::SubtitleOpacity(SubtitleOpacity::new(v)),
    );
    pick(
        ui,
        out,
        "Outline",
        "Not drawn when there is a background.",
        s.subtitle_outline,
        SubtitleOutline::ALL,
        outline_name,
        Setting::SubtitleOutline,
    );
    pick(
        ui,
        out,
        "Background",
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
        "Raise from the bottom",
        "Percent of the picture's height.",
        position.get(),
        (SubtitlePosition::MIN, SubtitlePosition::MAX, 1),
        |v| Setting::SubtitlePosition(SubtitlePosition::new(v)),
    );
    switch(
        ui,
        out,
        "Keep the look of styled subtitles",
        STYLED_HELP,
        s.keep_subtitle_styles,
        Setting::KeepSubtitleStyles,
    );
}

const STYLED_HELP: &str = "Styled (ASS) subtitles, common for anime, bring their own fonts, \
colors and positions. When off, the settings above replace them.";

/// An approximation of the subtitle look over a dark frame.
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
        (SubtitleFont::Serif, _) => theme::display_family(),
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
        "Subtitles look like this.".to_owned(),
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
        "PREVIEW",
        theme::caption(),
        theme::TEXT_FAINT,
    );
}

fn font_name(font: SubtitleFont) -> &'static str {
    match font {
        SubtitleFont::Sans => "Sans-serif",
        SubtitleFont::Serif => "Serif",
        SubtitleFont::Mono => "Monospace",
    }
}

fn color_name(color: SubtitleColor) -> &'static str {
    match color {
        SubtitleColor::White => "White",
        SubtitleColor::Yellow => "Yellow",
        SubtitleColor::Cyan => "Cyan",
        SubtitleColor::Green => "Green",
    }
}

fn outline_name(outline: SubtitleOutline) -> &'static str {
    match outline {
        SubtitleOutline::Black => "Black",
        SubtitleOutline::Gray => "Gray",
        SubtitleOutline::None => "No outline",
    }
}

fn background_name(background: SubtitleBackground) -> &'static str {
    match background {
        SubtitleBackground::None => "No background",
        SubtitleBackground::Black => "Black box",
        SubtitleBackground::Gray => "Gray box",
    }
}

const PASSTHROUGH_HELP: &str = "Sends Dolby and DTS audio undecoded to a receiver over \
HDMI or S/PDIF. Leave off unless your receiver decodes them, or you may hear silence.";

fn torrents(ui: &mut Ui, s: &Settings, out: &mut Vec<Action>) {
    switch(
        ui,
        out,
        "Show and play torrent streams",
        P2P_NOTICE,
        s.p2p_enabled,
        Setting::P2pEnabled,
    );
    switch(
        ui,
        out,
        "Upload to other peers",
        UPLOAD_HELP,
        s.torrent_upload,
        Setting::TorrentUpload,
    );
    pick(
        ui,
        out,
        "Download limit",
        NEXT_TORRENT,
        s.download_limit,
        DownloadLimit::ALL,
        download_name,
        Setting::DownloadLimit,
    );
    pick(
        ui,
        out,
        "Upload limit",
        NEXT_TORRENT,
        s.upload_limit,
        UploadLimit::ALL,
        upload_name,
        Setting::UploadLimit,
    );
    pick(
        ui,
        out,
        "Peers per torrent",
        "More peers can be faster but use more connections.",
        s.peer_limit,
        PeerLimit::ALL,
        peer_name,
        Setting::PeerLimit,
    );
    switch(
        ui,
        out,
        "Find peers through the DHT",
        DHT_HELP,
        s.torrent_dht,
        Setting::TorrentDht,
    );
    switch(
        ui,
        out,
        "Allow local network addresses",
        PRIVATE_HELP,
        s.allow_private_network,
        Setting::AllowPrivateNetwork,
    );
}

const NEXT_TORRENT: &str = "Applies from the next torrent you play.";
const UPLOAD_HELP: &str = "When off, Cineo only downloads. Some peers then send less, \
so torrents can be slower. Your IP address is still visible to peers.";
const DHT_HELP: &str = "Finds peers without trackers. Off means fewer peers for many torrents.";
const PRIVATE_HELP: &str = "Lets addons, images and torrent peers on 127.0.0.1 or your \
home network be reached, for self-hosted addons. Off is safer. Takes effect after \
restarting Cineo.";

fn output_name(output: AudioOutput) -> &'static str {
    match output {
        AudioOutput::Auto => "Automatic (surround)",
        AudioOutput::Stereo => "Stereo",
    }
}

fn download_name(limit: DownloadLimit) -> &'static str {
    match limit {
        DownloadLimit::Unlimited => "No limit",
        DownloadLimit::M1 => "1 MB/s",
        DownloadLimit::M2 => "2 MB/s",
        DownloadLimit::M5 => "5 MB/s",
        DownloadLimit::M10 => "10 MB/s",
        DownloadLimit::M20 => "20 MB/s",
    }
}

fn upload_name(limit: UploadLimit) -> &'static str {
    match limit {
        UploadLimit::Unlimited => "No limit",
        UploadLimit::K100 => "100 kB/s",
        UploadLimit::K500 => "500 kB/s",
        UploadLimit::M1 => "1 MB/s",
        UploadLimit::M5 => "5 MB/s",
    }
}

fn peer_name(limit: PeerLimit) -> &'static str {
    match limit {
        PeerLimit::P50 => "50",
        PeerLimit::P128 => "128",
        PeerLimit::P200 => "200",
    }
}

fn scale_name(scale: InterfaceScale) -> &'static str {
    match scale {
        InterfaceScale::S75 => "75 %",
        InterfaceScale::S90 => "90 %",
        InterfaceScale::S100 => "100 %",
        InterfaceScale::S110 => "110 %",
        InterfaceScale::S125 => "125 %",
        InterfaceScale::S150 => "150 %",
        InterfaceScale::S175 => "175 %",
        InterfaceScale::S200 => "200 %",
    }
}

fn start_name(page: StartPage) -> &'static str {
    match page {
        StartPage::Home => "Home",
        StartPage::Discover => "Discover",
        StartPage::Library => "Library",
    }
}

fn watched_name(at: WatchedAt) -> &'static str {
    match at {
        WatchedAt::P80 => "80 % played",
        WatchedAt::P85 => "85 % played",
        WatchedAt::P90 => "90 % played",
        WatchedAt::P92 => "92 % played",
        WatchedAt::P95 => "95 % played",
    }
}

fn seek_name(step: SeekStep) -> &'static str {
    match step {
        SeekStep::S5 => "5 seconds",
        SeekStep::S10 => "10 seconds",
        SeekStep::S15 => "15 seconds",
        SeekStep::S30 => "30 seconds",
    }
}

fn short_seek_name(step: ShortSeekStep) -> &'static str {
    match step {
        ShortSeekStep::S1 => "1 second",
        ShortSeekStep::S3 => "3 seconds",
        ShortSeekStep::S5 => "5 seconds",
    }
}

fn hide_name(after: HideControls) -> &'static str {
    match after {
        HideControls::Short => "1.5 seconds",
        HideControls::Normal => "2.5 seconds",
        HideControls::Long => "5 seconds",
    }
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
    name: fn(T) -> &'static str,
    setting: fn(T) -> Setting,
) {
    row(ui, title, help, |ui| {
        if let Some(value) = select(ui, title, current, options.iter().copied(), name) {
            out.push(Action::ChangeSetting(setting(value)));
        }
    });
}

/// A whole number in `range` = (min, max, step), set by dragging or with
/// the arrow keys.
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

fn language(
    ui: &mut Ui,
    out: &mut Vec<Action>,
    title: &str,
    help: &str,
    current: Option<Language>,
    setting: fn(Option<Language>) -> Setting,
) {
    row(ui, title, help, |ui| {
        let options = std::iter::once(None).chain(Language::all().map(Some));
        if let Some(value) = select(ui, title, current, options, |l| {
            l.map_or("None", Language::name)
        }) {
            out.push(Action::ChangeSetting(setting(value)));
        }
    });
}

/// A setting: its title and help on the left, its control on the right.
fn row(ui: &mut Ui, title: &str, help: &str, control: impl FnOnce(&mut Ui)) {
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

/// A dropdown; returns the option picked this frame.
fn select<T: Copy + PartialEq>(
    ui: &mut Ui,
    label: &str,
    current: T,
    options: impl IntoIterator<Item = T>,
    name: impl Fn(T) -> &'static str,
) -> Option<T> {
    let (rect, response) = ui.allocate_exact_size(
        vec2(ui.available_width().min(CONTROL_WIDTH), 30.0),
        Sense::click(),
    );
    response.widget_info(|| {
        let mut info = WidgetInfo::labeled(WidgetType::ComboBox, ui.is_enabled(), label);
        info.current_text_value = Some(name(current).to_owned());
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
        if selected {
            painter.rect_filled(
                Rect::from_min_size(pos2(rect.min.x, rect.min.y + 6.0), vec2(3.0, 18.0)),
                0.0,
                theme::ACCENT,
            );
        }
        let color = if selected || response.hovered() {
            theme::TEXT_BRIGHT
        } else {
            theme::TEXT_DIM
        };
        let galley = painter.layout_job(caps(label, theme::nav(), color));
        painter.galley(
            pos2(rect.min.x + 14.0, rect.center().y - galley.size().y / 2.0),
            galley,
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
            let mut back = plain("Esc  ·  Alt+");
            append_icon(&mut back, Icon::ArrowLeft, 15.0, theme::TEXT_BRIGHT);
            back.append("  ·  mouse back", 0.0, key.clone());
            let rows = [
                (plain("Ctrl+F  or  /"), "Search"),
                (plain("Ctrl+1 … Ctrl+6"), "Switch page"),
                (back, "Leave a detail page"),
                (plain("Shift+wheel"), "Scroll a row sideways"),
                (plain("Space  or  K"), "Play or pause"),
                (plain("←/→  or  J/L"), "Seek by the seek step"),
                (plain("Shift+←/→"), "Seek by the short seek step"),
                (plain("↑/↓  ·  M"), "Volume  ·  mute"),
                (plain("F  or  F11"), "Fullscreen"),
            ];
            for (keys, what) in rows {
                ui.label(keys);
                ui.label(dim(what));
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
            "Reset all settings?",
            "Every setting goes back to its default. Your addons and library are kept.",
            "Reset",
            Action::ResetSettings,
        ),
        Confirm::ClearLibrary => (
            "Clear watch history?",
            "Every item and its progress leaves the Library. This cannot be undone.",
            "Clear",
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
            ui.add(Label::new(dim(text)).wrap());
            ui.add_space(theme::GAP * 1.5);
            ui.horizontal(|ui| {
                if primary(ui, true, button).clicked() {
                    out.push(action);
                    done = true;
                }
                if ui.button("Cancel").clicked() {
                    done = true;
                }
            });
        });
    if done || modal.should_close() {
        view.confirm = None;
    }
}
