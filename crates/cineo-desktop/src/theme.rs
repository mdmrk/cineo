//! The one place that defines how Cineo looks (ADR-0011). Widgets read these
//! values; they never hardcode colors or sizes.
//!
//! The look is a film journal: dark, moody pages where the poster art and
//! the type carry the personality. Film and page titles are set in an
//! editorial serif (DM Serif Display); section heads are small letter-spaced
//! capitals over a thin rule; everything else is plain Inter. Posters sit in
//! dense grids like printed cards with square-ish corners. Marquee amber is
//! the one action color. Fonts are bundled (OFL-1.1, docs/LEGAL.md).

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Stroke,
    TextStyle, Visuals,
};

/// Page background.
pub(crate) const BG: Color32 = Color32::from_rgb(0x11, 0x10, 0x13);
/// The navigation sidebar, a shade darker than the page.
pub(crate) const SIDEBAR: Color32 = Color32::from_rgb(0x0b, 0x0b, 0x0d);
/// Raised panels: banners, dialogs, the install box.
pub(crate) const PANEL: Color32 = Color32::from_rgb(0x19, 0x18, 0x1c);
/// Inputs, buttons, tags and poster placeholders.
pub(crate) const SURFACE: Color32 = Color32::from_rgb(0x26, 0x24, 0x2a);
pub(crate) const SURFACE_HOVER: Color32 = Color32::from_rgb(0x34, 0x31, 0x39);
/// Thin rules under section heads and between list rows.
pub(crate) const RULE: Color32 = Color32::from_rgb(0x2b, 0x29, 0x2f);
/// Titles and emphasised text.
pub(crate) const TEXT_BRIGHT: Color32 = Color32::from_rgb(0xf5, 0xf1, 0xea);
pub(crate) const TEXT: Color32 = Color32::from_rgb(0xd6, 0xd0, 0xc7);
pub(crate) const TEXT_DIM: Color32 = Color32::from_rgb(0x9a, 0x94, 0x8b);
pub(crate) const TEXT_FAINT: Color32 = Color32::from_rgb(0x6b, 0x66, 0x5f);
/// The action color: Play, hover outlines, selection, progress.
pub(crate) const ACCENT: Color32 = Color32::from_rgb(0xff, 0xb5, 0x47);
pub(crate) const ACCENT_HOVER: Color32 = Color32::from_rgb(0xff, 0xc8, 0x73);
/// Text on an [`ACCENT`] fill.
pub(crate) const ON_ACCENT: Color32 = Color32::from_rgb(0x1c, 0x13, 0x05);
/// Darkens backdrops so text over them stays readable.
pub(crate) const BACKDROP_TINT: Color32 = Color32::from_gray(140);
/// Translucent black under controls and text drawn over images.
pub(crate) const SCRIM: Color32 = Color32::from_black_alpha(170);
/// The faint edge every poster gets, as on a printed card.
pub(crate) const POSTER_EDGE: Color32 = Color32::from_rgba_premultiplied(22, 22, 22, 22);
pub(crate) const DANGER: Color32 = Color32::from_rgb(0xff, 0x6b, 0x5e);
pub(crate) const WARNING: Color32 = Color32::from_rgb(0xff, 0x91, 0x56);
pub(crate) const SUCCESS: Color32 = Color32::from_rgb(0x8e, 0xd8, 0xa4);

/// Buttons, inputs, tags and panels.
pub(crate) const RADIUS: u8 = 3;
pub(crate) const POSTER_RADIUS: u8 = 3;
pub(crate) const GAP: f32 = 12.0;
pub(crate) const SECTION_GAP: f32 = 36.0;
pub(crate) const PAGE_MARGIN: i8 = 40;
/// The widest the page content grows; wider windows get side margins.
pub(crate) const CONTENT_MAX_WIDTH: f32 = 1280.0;

/// Sidebar width with labels, and icons-only on narrow windows.
pub(crate) const SIDEBAR_WIDTH: f32 = 208.0;
pub(crate) const SIDEBAR_COMPACT_WIDTH: f32 = 68.0;
/// Below this window width the sidebar shows icons only.
pub(crate) const SIDEBAR_COMPACT_BELOW: f32 = 1000.0;

/// Poster width in rows; grids stretch between the min and max. Dense on
/// purpose: browsing should feel like flipping through a collection.
pub(crate) const CARD_WIDTH: f32 = 128.0;
pub(crate) const GRID_CARD_MIN: f32 = 112.0;
pub(crate) const GRID_CARD_MAX: f32 = 156.0;
pub(crate) const CARD_GAP: f32 = 8.0;

/// Detail page backdrop height and poster width.
pub(crate) const BACKDROP_HEIGHT: f32 = 440.0;
pub(crate) const DETAIL_POSTER_WIDTH: f32 = 230.0;

/// Seconds of hover and selection fades. Short, so the UI feels immediate.
pub(crate) const ANIM: f32 = 0.1;

/// Points one mouse-wheel notch scrolls. egui's native default (40) suits
/// text; poster pages need about what browsers use.
pub(crate) const WHEEL_LINE_POINTS: f32 = 100.0;

/// Extra spacing between the letters of small-caps heads and nav labels.
pub(crate) const CAPS_SPACING: f32 = 1.4;

fn strong_family() -> FontFamily {
    FontFamily::Name("strong".into())
}

fn display_family() -> FontFamily {
    FontFamily::Name("display".into())
}

/// Film titles on the detail page.
pub(crate) fn title() -> FontId {
    FontId::new(46.0, display_family())
}

/// The year next to a film title.
pub(crate) fn title_year() -> FontId {
    FontId::new(28.0, display_family())
}

/// Page headings (Discover, Search, …) and dialog titles.
pub(crate) fn heading() -> FontId {
    FontId::new(36.0, display_family())
}

pub(crate) fn logo() -> FontId {
    FontId::new(28.0, display_family())
}

/// Small-caps section heads (set with [`CAPS_SPACING`]).
pub(crate) fn section() -> FontId {
    FontId::new(12.5, strong_family())
}

/// Sidebar labels (small caps).
pub(crate) fn nav() -> FontId {
    FontId::new(12.5, strong_family())
}

/// Emphasised body text: names, buttons that matter.
pub(crate) fn strong() -> FontId {
    FontId::new(14.5, strong_family())
}

pub(crate) fn body() -> FontId {
    FontId::new(14.5, FontFamily::Proportional)
}

/// Longer reading text: synopses.
pub(crate) fn reading() -> FontId {
    FontId::new(16.0, FontFamily::Proportional)
}

pub(crate) fn caption() -> FontId {
    FontId::new(12.5, FontFamily::Proportional)
}

/// Inter for the UI, DM Serif Display for titles, Tabler for icons; egui's
/// bundled fonts are the fallback for symbols and emoji.
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
    let mut display = vec!["dm-serif-display".to_owned()];
    display.extend(proportional.iter().cloned());
    fonts
        .families
        .insert(FontFamily::Proportional, proportional);
    fonts.families.insert(strong_family(), strong);
    // Icon fonts (Tabler, via iconflow): each is its own family, with the
    // text fonts behind it so epaint finds a replacement glyph (`◻`/`?`).
    for font in iconflow::fonts() {
        fonts.font_data.insert(
            font.family.to_owned(),
            FontData::from_static(font.bytes).into(),
        );
        let mut family = vec![font.family.to_owned()];
        family.extend(display.iter().skip(1).cloned());
        fonts
            .families
            .insert(FontFamily::Name(font.family.into()), family);
    }
    fonts.families.insert(display_family(), display);
    fonts
}

fn applied_id() -> egui::Id {
    egui::Id::new("cineo-theme-applied")
}

/// Applies the theme unless `ctx` already has it. Returns true if it was
/// applied just now: the fonts only take effect from the next pass.
pub(crate) fn ensure(ctx: &egui::Context) -> bool {
    if ctx.data(|d| d.get_temp::<()>(applied_id())).is_some() {
        return false;
    }
    apply(ctx);
    true
}

pub(crate) fn apply(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(applied_id(), ()));
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
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 6.0);
        style.spacing.window_margin = Margin::same(24);
        style.spacing.menu_margin = Margin::same(6);
        style.spacing.interact_size.y = 30.0;
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

    /// epaint needs `◻` or `?` in every family for characters it cannot
    /// draw; an icon font alone has neither.
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
