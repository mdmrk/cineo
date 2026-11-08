use crate::theme;
use eframe::egui::{
    self, Align, Align2, Color32, FontId, Pos2, Rect, TextFormat, pos2, text::LayoutJob,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Icon {
    Home,
    Compass,
    Search,
    Library,
    Addons,
    Settings,
    ChevronLeft,
    ChevronRight,
    ChevronDown,
    ArrowLeft,
    Star,
    Alert,
    Play,
    Pause,
    Heart,
    Named(&'static str),
}

impl Icon {
    #[cfg(test)]
    pub(super) const ALL: [Self; 15] = [
        Self::Home,
        Self::Compass,
        Self::Search,
        Self::Library,
        Self::Addons,
        Self::Settings,
        Self::ChevronLeft,
        Self::ChevronRight,
        Self::ChevronDown,
        Self::ArrowLeft,
        Self::Star,
        Self::Alert,
        Self::Play,
        Self::Pause,
        Self::Heart,
    ];

    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Compass => "compass",
            Self::Search => "search",
            Self::Library => "books",
            Self::Addons => "puzzle",
            Self::Settings => "adjustments-horizontal",
            Self::ChevronLeft => "chevron-left",
            Self::ChevronRight => "chevron-right",
            Self::ChevronDown => "chevron-down",
            Self::ArrowLeft => "arrow-left",
            Self::Star => "star",
            Self::Alert => "alert-circle",
            Self::Play => "player-play",
            Self::Pause => "player-pause",
            Self::Heart => "heart",
            Self::Named(name) => name,
        }
    }

    pub(crate) fn glyph(self) -> Option<(char, egui::FontFamily)> {
        let icon = iconflow::try_icon(
            iconflow::Pack::Tabler,
            self.name(),
            if matches!(self, Self::Star | Self::Play | Self::Pause | Self::Heart) {
                iconflow::Style::Filled
            } else {
                iconflow::Style::Regular
            },
            iconflow::Size::Regular,
        )
        .ok()?;
        let glyph = char::from_u32(icon.codepoint)?;
        Some((glyph, icon_family(icon.family)))
    }
}

fn icon_family(name: &'static str) -> egui::FontFamily {
    thread_local! {
        static FAMILIES: std::cell::RefCell<Vec<(&'static str, egui::FontFamily)>> =
            const { std::cell::RefCell::new(Vec::new()) };
    }
    FAMILIES.with_borrow_mut(|families| {
        if let Some((_, family)) = families.iter().find(|(n, _)| *n == name) {
            return family.clone();
        }
        let family = egui::FontFamily::Name(name.into());
        families.push((name, family.clone()));
        family
    })
}

pub(crate) fn paint_nav_label(
    painter: &egui::Painter,
    rect: Rect,
    icon: Icon,
    icon_size: f32,
    label: &str,
    color: Color32,
) {
    let y = rect.center().y;
    paint_icon(painter, icon, pos2(rect.min.x + 18.0, y), icon_size, color);
    painter.text(
        pos2(rect.min.x + 36.0, y),
        Align2::LEFT_CENTER,
        label,
        theme::nav(),
        color,
    );
}

pub(crate) fn paint_icon(painter: &egui::Painter, icon: Icon, c: Pos2, size: f32, color: Color32) {
    let Some((glyph, family)) = icon.glyph() else {
        return;
    };
    let galley = painter.layout_no_wrap(glyph.to_string(), FontId::new(size, family), color);
    let center = ink_center(&galley).unwrap_or_else(|| galley.rect.center());
    painter.galley(c - center.to_vec2(), galley, color);
}

fn ink_center(galley: &egui::Galley) -> Option<Pos2> {
    let row = galley.rows.first()?;
    let glyph = row.glyphs.first()?;
    let ink = Rect::from_min_size(glyph.pos + glyph.uv_rect.offset, glyph.uv_rect.size);
    Some(row.pos + ink.center().to_vec2())
}

pub(super) const EMOJI_ICONS: &[(char, &str)] = &[
    ('👤', "user"),
    ('👥', "users"),
    ('💾', "device-floppy"),
    ('📦', "package"),
    ('⚙', "settings"),
    ('🔗', "link"),
    ('🌐', "world"),
    ('🌍', "world"),
    ('📁', "folder"),
    ('📂', "folder"),
    ('📄', "file"),
    ('🗄', "database"),
    ('🖥', "server"),
    ('📺', "device-tv"),
    ('🎞', "movie"),
    ('🎬', "movie"),
    ('🎥', "video"),
    ('📹', "video"),
    ('🔊', "volume"),
    ('🎧', "headphones"),
    ('🗣', "language"),
    ('💬', "message"),
    ('⏱', "clock"),
    ('🕒', "clock"),
    ('⌛', "clock"),
    ('🔥', "flame"),
    ('⬇', "download"),
    ('⬆', "upload"),
    ('🧲', "magnet"),
    ('💿', "disc"),
    ('📀', "disc"),
    ('⚡', "bolt"),
    ('✅', "check"),
    ('✔', "check"),
    ('❌', "x"),
    ('ℹ', "info-circle"),
    ('🏷', "tag"),
    ('🔍', "search"),
    ('⭐', "star"),
    ('🌟', "star"),
    ('★', "star"),
];

pub(super) fn regional_letter(c: char) -> Option<char> {
    let offset = u32::from(c).checked_sub(0x1F1E6)?;
    (offset < 26).then(|| char::from(b'A' + u8::try_from(offset).unwrap_or(0)))
}

pub(super) fn is_emoji(c: char) -> bool {
    matches!(
        u32::from(c),
        0x200D | 0xFE0E | 0xFE0F | 0x20E3
            | 0x2300..=0x23FF
            | 0x2600..=0x27BF
            | 0x2B00..=0x2BFF
            | 0x1F000..=0x1FAFF
            | 0xE0020..=0xE007F
    )
}

pub(super) fn addon_text(
    text: &str,
    font: &FontId,
    color: Color32,
    icon_color: Color32,
) -> LayoutJob {
    let format = TextFormat {
        font_id: font.clone(),
        color,
        valign: Align::Center,
        ..TextFormat::default()
    };
    let mut job = LayoutJob::default();
    let mut plain = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if let Some(first) = regional_letter(c) {
            if let Some(second) = chars.peek().copied().and_then(regional_letter) {
                chars.next();
                plain.push(first);
                plain.push(second);
            }
            continue;
        }
        if let Some(&(_, name)) = (!c.is_ascii())
            .then(|| EMOJI_ICONS.iter().find(|(emoji, _)| *emoji == c))
            .flatten()
        {
            job.append(&std::mem::take(&mut plain), 0.0, format.clone());
            append_icon(&mut job, Icon::Named(name), font.size, icon_color);
            continue;
        }
        if !is_emoji(c) {
            plain.push(c);
        }
    }
    job.append(&plain, 0.0, format);
    job
}

pub(crate) fn append_icon(job: &mut LayoutJob, icon: Icon, size: f32, color: Color32) {
    if let Some((glyph, family)) = icon.glyph() {
        job.append(
            &glyph.to_string(),
            0.0,
            TextFormat {
                font_id: FontId::new(size, family),
                color,
                valign: Align::Center,
                ..TextFormat::default()
            },
        );
    }
}
