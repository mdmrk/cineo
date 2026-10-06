//! The one place that defines how Cineo looks (ADR-0011). Widgets read these
//! values; they never hardcode colors or sizes.

use eframe::egui::{
    self, Color32, CornerRadius, FontFamily, FontId, Margin, Stroke, TextStyle, Visuals,
};

pub(crate) const BG: Color32 = Color32::from_rgb(0x10, 0x11, 0x16);
pub(crate) const PANEL: Color32 = Color32::from_rgb(0x17, 0x18, 0x20);
pub(crate) const SURFACE: Color32 = Color32::from_rgb(0x22, 0x24, 0x2f);
pub(crate) const SURFACE_HOVER: Color32 = Color32::from_rgb(0x2c, 0x2f, 0x3d);
pub(crate) const TEXT: Color32 = Color32::from_rgb(0xe8, 0xe9, 0xee);
pub(crate) const TEXT_DIM: Color32 = Color32::from_rgb(0x9a, 0x9d, 0xab);
pub(crate) const ACCENT: Color32 = Color32::from_rgb(0x7b, 0x6c, 0xf6);
pub(crate) const DANGER: Color32 = Color32::from_rgb(0xef, 0x6b, 0x6b);
pub(crate) const WARNING: Color32 = Color32::from_rgb(0xf0, 0xb4, 0x5a);

pub(crate) const RADIUS: u8 = 8;
pub(crate) const GAP: f32 = 12.0;
pub(crate) const PAGE_MARGIN: i8 = 24;
pub(crate) const SIDEBAR_WIDTH: f32 = 190.0;

/// Poster card width; heights follow the poster shape.
pub(crate) const CARD_WIDTH: f32 = 140.0;
/// Room under a card's image for two lines of title.
pub(crate) const CARD_CAPTION: f32 = 40.0;

pub(crate) fn heading() -> FontId {
    FontId::new(22.0, FontFamily::Proportional)
}

pub(crate) fn title() -> FontId {
    FontId::new(30.0, FontFamily::Proportional)
}

pub(crate) fn caption() -> FontId {
    FontId::new(13.0, FontFamily::Proportional)
}

/// Applies the Cineo theme to `ctx`.
pub(crate) fn apply(ctx: &egui::Context) {
    let radius = CornerRadius::same(RADIUS);
    let mut visuals = Visuals::dark();
    visuals.panel_fill = BG;
    visuals.window_fill = PANEL;
    visuals.extreme_bg_color = SURFACE;
    visuals.faint_bg_color = PANEL;
    visuals.override_text_color = Some(TEXT);
    visuals.hyperlink_color = ACCENT;
    visuals.selection.bg_fill = ACCENT.gamma_multiply(0.6);
    visuals.selection.stroke = Stroke::new(1.0, TEXT);
    visuals.window_corner_radius = radius;
    for (widget, fill) in [
        (&mut visuals.widgets.noninteractive, PANEL),
        (&mut visuals.widgets.inactive, SURFACE),
        (&mut visuals.widgets.hovered, SURFACE_HOVER),
        (&mut visuals.widgets.active, ACCENT),
        (&mut visuals.widgets.open, SURFACE_HOVER),
    ] {
        widget.corner_radius = radius;
        widget.bg_fill = fill;
        widget.weak_bg_fill = fill;
    }
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
    ctx.set_visuals(visuals);

    ctx.global_style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 6.0);
        style.spacing.window_margin = Margin::same(PAGE_MARGIN);
        style.text_styles.insert(
            TextStyle::Heading,
            FontId::new(20.0, FontFamily::Proportional),
        );
        style
            .text_styles
            .insert(TextStyle::Body, FontId::new(15.0, FontFamily::Proportional));
        style.text_styles.insert(
            TextStyle::Button,
            FontId::new(15.0, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Small,
            FontId::new(12.0, FontFamily::Proportional),
        );
    });
}
