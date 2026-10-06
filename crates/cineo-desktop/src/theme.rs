//! The one place that defines how Cineo looks (ADR-0011). Widgets read these
//! values; they never hardcode colors or sizes.
//!
//! The look is a film-diary style: a dark slate page, small uppercase section
//! headings over a thin rule, bordered posters without captions, and green as
//! the one action color.

use eframe::egui::{
    self, Color32, CornerRadius, FontFamily, FontId, Margin, Stroke, TextStyle, Visuals,
};

/// Page background.
pub(crate) const BG: Color32 = Color32::from_rgb(0x14, 0x18, 0x1c);
/// Top bar and raised panels.
pub(crate) const PANEL: Color32 = Color32::from_rgb(0x1c, 0x22, 0x28);
/// Inputs, buttons and poster placeholders.
pub(crate) const SURFACE: Color32 = Color32::from_rgb(0x2c, 0x34, 0x40);
pub(crate) const SURFACE_HOVER: Color32 = Color32::from_rgb(0x45, 0x55, 0x66);
/// Thin rules under section headings and between list rows.
pub(crate) const RULE: Color32 = Color32::from_rgb(0x2c, 0x34, 0x40);
/// Titles and emphasised text.
pub(crate) const TEXT_BRIGHT: Color32 = Color32::from_rgb(0xff, 0xff, 0xff);
pub(crate) const TEXT: Color32 = Color32::from_rgb(0xdd, 0xee, 0xff);
pub(crate) const TEXT_DIM: Color32 = Color32::from_rgb(0x99, 0xaa, 0xbb);
pub(crate) const TEXT_FAINT: Color32 = Color32::from_rgb(0x67, 0x78, 0x89);
/// The action color: Play, hover outlines, progress.
pub(crate) const ACCENT: Color32 = Color32::from_rgb(0x00, 0xe0, 0x54);
/// Text on an [`ACCENT`] fill.
pub(crate) const ON_ACCENT: Color32 = Color32::from_rgb(0x0b, 0x1a, 0x10);
/// The logo dots: orange, green, blue.
pub(crate) const LOGO: [Color32; 3] = [
    Color32::from_rgb(0xff, 0x80, 0x00),
    ACCENT,
    Color32::from_rgb(0x40, 0xbc, 0xf4),
];
/// Darkens the detail backdrop so text over it stays readable.
pub(crate) const BACKDROP_TINT: Color32 = Color32::from_gray(150);
/// Translucent black under controls and bars drawn over images.
pub(crate) const SCRIM: Color32 = Color32::from_black_alpha(160);
pub(crate) const DANGER: Color32 = Color32::from_rgb(0xff, 0x6b, 0x6b);
pub(crate) const WARNING: Color32 = Color32::from_rgb(0xff, 0x80, 0x00);

/// Buttons and inputs.
pub(crate) const RADIUS: u8 = 3;
/// Posters.
pub(crate) const POSTER_RADIUS: u8 = 4;
/// The faint outline every poster gets, as on a printed card.
pub(crate) const POSTER_OUTLINE: Color32 = Color32::from_rgba_premultiplied(0x37, 0x3b, 0x40, 0x40);
pub(crate) const GAP: f32 = 12.0;
pub(crate) const SECTION_GAP: f32 = 28.0;
pub(crate) const PAGE_MARGIN: i8 = 32;
/// The widest the page content grows; wider windows get side margins.
pub(crate) const CONTENT_MAX_WIDTH: f32 = 1200.0;
pub(crate) const TOP_BAR_HEIGHT: f32 = 56.0;

/// Poster card width; heights follow the poster shape.
pub(crate) const CARD_WIDTH: f32 = 140.0;
pub(crate) const CARD_GAP: f32 = 10.0;

/// Detail page: backdrop height and poster width.
pub(crate) const BACKDROP_HEIGHT: f32 = 360.0;
pub(crate) const DETAIL_POSTER_WIDTH: f32 = 220.0;

/// Small uppercase section headings.
pub(crate) fn section() -> FontId {
    FontId::new(13.0, FontFamily::Proportional)
}

/// The film title on the detail page.
pub(crate) fn title() -> FontId {
    FontId::new(34.0, FontFamily::Proportional)
}

/// Page headings (Discover, Search, …).
pub(crate) fn heading() -> FontId {
    FontId::new(24.0, FontFamily::Proportional)
}

pub(crate) fn logo() -> FontId {
    FontId::new(20.0, FontFamily::Proportional)
}

pub(crate) fn nav() -> FontId {
    FontId::new(13.0, FontFamily::Proportional)
}

pub(crate) fn body() -> FontId {
    FontId::new(15.0, FontFamily::Proportional)
}

pub(crate) fn caption() -> FontId {
    FontId::new(12.0, FontFamily::Proportional)
}

/// Extra spacing between the letters of uppercase headings and nav links.
pub(crate) const CAPS_SPACING: f32 = 1.2;

/// Applies the Cineo theme to `ctx`.
pub(crate) fn apply(ctx: &egui::Context) {
    let radius = CornerRadius::same(RADIUS);
    let mut visuals = Visuals::dark();
    visuals.panel_fill = BG;
    visuals.window_fill = PANEL;
    visuals.window_stroke = Stroke::new(1.0, RULE);
    visuals.extreme_bg_color = SURFACE;
    visuals.faint_bg_color = PANEL;
    visuals.override_text_color = Some(TEXT);
    visuals.hyperlink_color = ACCENT;
    visuals.selection.bg_fill = ACCENT.gamma_multiply(0.35);
    visuals.selection.stroke = Stroke::new(1.0, TEXT_BRIGHT);
    visuals.window_corner_radius = CornerRadius::same(POSTER_RADIUS);
    visuals.menu_corner_radius = CornerRadius::same(POSTER_RADIUS);
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
    }
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, RULE);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT_BRIGHT);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, TEXT_BRIGHT);
    ctx.set_visuals(visuals);

    ctx.global_style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 6.0);
        style.spacing.window_margin = Margin::same(PAGE_MARGIN);
        style.spacing.interact_size.y = 28.0;
        style.text_styles.insert(
            TextStyle::Heading,
            FontId::new(20.0, FontFamily::Proportional),
        );
        style
            .text_styles
            .insert(TextStyle::Body, FontId::new(15.0, FontFamily::Proportional));
        style.text_styles.insert(
            TextStyle::Button,
            FontId::new(14.0, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Small,
            FontId::new(12.0, FontFamily::Proportional),
        );
    });
}
