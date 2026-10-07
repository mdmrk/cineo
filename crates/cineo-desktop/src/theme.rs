//! The one place that defines how Cineo looks (ADR-0011). Widgets read these
//! values; they never hardcode colors or sizes.

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Stroke,
    TextStyle, Visuals,
};

pub(crate) const BG: Color32 = Color32::from_rgb(0x14, 0x18, 0x1d);
pub(crate) const SIDEBAR: Color32 = Color32::from_rgb(0x10, 0x13, 0x17);
pub(crate) const PANEL: Color32 = Color32::from_rgb(0x1b, 0x21, 0x28);
pub(crate) const SURFACE: Color32 = Color32::from_rgb(0x25, 0x2d, 0x36);
pub(crate) const SURFACE_HOVER: Color32 = Color32::from_rgb(0x31, 0x3b, 0x46);
pub(crate) const RULE: Color32 = Color32::from_rgb(0x26, 0x2e, 0x37);
pub(crate) const TEXT_BRIGHT: Color32 = Color32::from_rgb(0xf1, 0xf4, 0xf7);
pub(crate) const TEXT: Color32 = Color32::from_rgb(0xc3, 0xcc, 0xd6);
pub(crate) const TEXT_DIM: Color32 = Color32::from_rgb(0x8b, 0x98, 0xa8);
pub(crate) const TEXT_FAINT: Color32 = Color32::from_rgb(0x60, 0x6c, 0x7b);
pub(crate) const ACCENT: Color32 = Color32::from_rgb(0x34, 0xc7, 0x7b);
pub(crate) const ACCENT_HOVER: Color32 = Color32::from_rgb(0x52, 0xd6, 0x91);
pub(crate) const ON_ACCENT: Color32 = Color32::from_rgb(0x06, 0x18, 0x0e);
pub(crate) const BACKDROP_TINT: Color32 = Color32::from_gray(150);
pub(crate) const SCRIM: Color32 = Color32::from_black_alpha(170);
pub(crate) const POSTER_EDGE: Color32 = Color32::from_rgba_premultiplied(26, 26, 26, 26);
pub(crate) const DANGER: Color32 = Color32::from_rgb(0xff, 0x6b, 0x6b);
pub(crate) const WARNING: Color32 = Color32::from_rgb(0xf2, 0xa1, 0x4b);
pub(crate) const SUCCESS: Color32 = Color32::from_rgb(0x7f, 0xdc, 0xa5);

pub(crate) const RADIUS: u8 = 4;
pub(crate) const POSTER_RADIUS: u8 = 4;
pub(crate) const GAP: f32 = 8.0;
pub(crate) const SECTION_GAP: f32 = 26.0;
pub(crate) const PAGE_MARGIN: f32 = 24.0;
pub(crate) const CONTENT_MAX_WIDTH: f32 = 1440.0;

pub(crate) const SIDEBAR_WIDTH: f32 = 176.0;
pub(crate) const SIDEBAR_COMPACT_WIDTH: f32 = 56.0;
pub(crate) const COMPACT_BELOW: f32 = 1000.0;

pub(crate) const LOGO_WIDTH: f32 = 144.0;
pub(crate) const LOGO_ASPECT: f32 = 360.0 / 304.0;
pub(crate) const LOGO_MOUSTACHE_X: f32 = 110.5 / 304.0;
pub(crate) const LOGO_BLEED: egui::Vec2 = egui::vec2(-12.0, -10.0);
pub(crate) const NAV_TOP: f32 = LOGO_WIDTH * LOGO_ASPECT + 8.0;
pub(crate) const GRAIN_FADE: f32 = 40.0;
pub(crate) const LOGO_TINT: Color32 = Color32::from_rgba_premultiplied(191, 191, 191, 217);

pub(crate) const CARD_WIDTH: f32 = 116.0;
pub(crate) const GRID_CARD_MIN: f32 = 104.0;
pub(crate) const GRID_CARD_MAX: f32 = 140.0;
pub(crate) const CARD_GAP: f32 = 6.0;

pub(crate) const BACKDROP_HEIGHT: f32 = 320.0;
pub(crate) const DETAIL_POSTER_WIDTH: f32 = 180.0;

pub(crate) const ANIM: f32 = 0.0;

pub(crate) const WHEEL_LINE_POINTS: f32 = 100.0;

pub(crate) const CAPS_SPACING: f32 = 0.6;

pub(crate) fn strong_family() -> FontFamily {
    FontFamily::Name("strong".into())
}

pub(crate) fn serif_family() -> FontFamily {
    FontFamily::Name("serif".into())
}

pub(crate) fn title() -> FontId {
    FontId::new(32.0, strong_family())
}

pub(crate) fn title_year() -> FontId {
    FontId::new(22.0, FontFamily::Proportional)
}

pub(crate) fn heading() -> FontId {
    FontId::new(24.0, strong_family())
}

pub(crate) fn section() -> FontId {
    FontId::new(15.0, strong_family())
}

pub(crate) fn nav() -> FontId {
    FontId::new(14.0, FontFamily::Proportional)
}

pub(crate) fn tag() -> FontId {
    FontId::new(10.0, strong_family())
}

pub(crate) fn strong() -> FontId {
    FontId::new(14.0, strong_family())
}

pub(crate) fn body() -> FontId {
    FontId::new(14.0, FontFamily::Proportional)
}

pub(crate) fn reading() -> FontId {
    FontId::new(15.0, FontFamily::Proportional)
}

pub(crate) fn caption() -> FontId {
    FontId::new(12.0, FontFamily::Proportional)
}

fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "inter".into(),
        FontData::from_static(include_bytes!("../assets/fonts/Inter-Regular.ttf")).into(),
    );
    fonts.font_data.insert(
        "inter-semibold".into(),
        FontData::from_static(include_bytes!("../assets/fonts/Inter-SemiBold.ttf")).into(),
    );
    fonts.font_data.insert(
        "dm-serif-display".into(),
        FontData::from_static(include_bytes!("../assets/fonts/DMSerifDisplay-Regular.ttf")).into(),
    );
    let fallback = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let mut proportional = vec!["inter".to_owned()];
    proportional.extend(fallback.iter().cloned());
    let mut strong = vec!["inter-semibold".to_owned()];
    strong.extend(proportional.iter().cloned());
    let mut serif = vec!["dm-serif-display".to_owned()];
    serif.extend(proportional.iter().cloned());
    fonts
        .families
        .insert(FontFamily::Proportional, proportional);
    fonts.families.insert(strong_family(), strong);
    for font in iconflow::fonts() {
        fonts.font_data.insert(
            font.family.to_owned(),
            FontData::from_static(font.bytes).into(),
        );
        let mut family = vec![font.family.to_owned()];
        family.extend(serif.iter().skip(1).cloned());
        fonts
            .families
            .insert(FontFamily::Name(font.family.into()), family);
    }
    fonts.families.insert(serif_family(), serif);
    fonts
}

fn applied_id() -> egui::Id {
    egui::Id::new("cineo-theme-applied")
}

pub(crate) fn ensure(ctx: &egui::Context) -> bool {
    if ctx.data(|d| d.get_temp::<()>(applied_id())).is_some() {
        return false;
    }
    apply(ctx);
    true
}

pub(crate) fn apply(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(applied_id(), ()));
    ctx.options_mut(|o| o.zoom_with_keyboard = false);
    ctx.set_fonts(fonts());

    let radius = CornerRadius::same(RADIUS);
    let mut visuals = Visuals::dark();
    visuals.panel_fill = BG;
    visuals.window_fill = PANEL;
    visuals.window_stroke = Stroke::new(1.0, RULE);
    visuals.extreme_bg_color = SURFACE;
    visuals.text_edit_bg_color = Some(SURFACE);
    visuals.faint_bg_color = PANEL;
    visuals.override_text_color = Some(TEXT);
    visuals.hyperlink_color = ACCENT;
    visuals.selection.bg_fill = ACCENT.gamma_multiply(0.35);
    visuals.selection.stroke = Stroke::new(1.5, ACCENT);
    visuals.window_corner_radius = CornerRadius::same(RADIUS);
    visuals.menu_corner_radius = CornerRadius::same(RADIUS);
    visuals.window_shadow = egui::Shadow {
        offset: [0, 12],
        blur: 32,
        spread: 0,
        color: Color32::from_black_alpha(140),
    };
    visuals.popup_shadow = egui::Shadow {
        offset: [0, 6],
        blur: 18,
        spread: 0,
        color: Color32::from_black_alpha(120),
    };
    for (widget, fill) in [
        (&mut visuals.widgets.noninteractive, PANEL),
        (&mut visuals.widgets.inactive, SURFACE),
        (&mut visuals.widgets.hovered, SURFACE_HOVER),
        (&mut visuals.widgets.active, SURFACE_HOVER),
        (&mut visuals.widgets.open, SURFACE_HOVER),
    ] {
        widget.corner_radius = radius;
        widget.bg_fill = fill;
        widget.weak_bg_fill = fill;
        widget.bg_stroke = Stroke::NONE;
        widget.expansion = 0.0;
    }
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, RULE);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT_BRIGHT);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, TEXT_BRIGHT);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0, TEXT_BRIGHT);
    ctx.set_visuals(visuals);

    ctx.options_mut(|o| o.input_options.line_scroll_speed = WHEEL_LINE_POINTS);

    ctx.global_style_mut(|style| {
        style.animation_time = ANIM;
        style.scroll_animation = egui::style::ScrollAnimation::none();
        style.spacing.item_spacing = egui::vec2(6.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 4.0);
        style.spacing.window_margin = Margin::same(18);
        style.spacing.menu_margin = Margin::same(6);
        style.spacing.interact_size.y = 26.0;
        style.spacing.combo_width = 200.0;
        style.spacing.scroll.floating = true;
        style.spacing.scroll.bar_width = 8.0;
        style
            .text_styles
            .insert(TextStyle::Heading, FontId::new(22.0, strong_family()));
        style.text_styles.insert(TextStyle::Body, body());
        style
            .text_styles
            .insert(TextStyle::Button, FontId::new(14.0, strong_family()));
        style.text_styles.insert(TextStyle::Small, caption());
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_font_family_falls_back_to_inter() {
        for (family, list) in &fonts().families {
            if *family == FontFamily::Monospace {
                continue;
            }
            assert!(
                list.iter().any(|font| font == "inter"),
                "{family:?}: {list:?}"
            );
        }
    }
}
