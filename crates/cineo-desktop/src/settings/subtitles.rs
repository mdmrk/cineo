use cineo_core::app::{
    Action, Setting, Settings, SubtitleBackground, SubtitleColor, SubtitleFont, SubtitleOpacity,
    SubtitleOutline, SubtitlePosition, SubtitleSize,
};
use eframe::egui::{
    Align2, Color32, CornerRadius, FontFamily, FontId, Rect, Sense, Ui, pos2, vec2,
};

use crate::i18n::{Text, t};
use crate::theme;

use super::widgets::{number, pick, switch};

pub(super) fn subtitle_style(ui: &mut Ui, s: &Settings, out: &mut Vec<Action>) {
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
        t!("subtitle-preview").into_owned(),
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

fn font_name(font: SubtitleFont) -> Text {
    match font {
        SubtitleFont::Sans => t!("font-sans"),
        SubtitleFont::Serif => t!("font-serif"),
        SubtitleFont::Mono => t!("font-mono"),
    }
}

fn color_name(color: SubtitleColor) -> Text {
    match color {
        SubtitleColor::White => t!("color-white"),
        SubtitleColor::Yellow => t!("color-yellow"),
        SubtitleColor::Cyan => t!("color-cyan"),
        SubtitleColor::Green => t!("color-green"),
    }
}

fn outline_name(outline: SubtitleOutline) -> Text {
    match outline {
        SubtitleOutline::Black => t!("outline-black"),
        SubtitleOutline::Gray => t!("outline-gray"),
        SubtitleOutline::None => t!("outline-none"),
    }
}

fn background_name(background: SubtitleBackground) -> Text {
    match background {
        SubtitleBackground::None => t!("background-none"),
        SubtitleBackground::Black => t!("background-black"),
        SubtitleBackground::Gray => t!("background-gray"),
    }
}
