use std::sync::Arc;

use eframe::egui::{
    self, Align2, CornerRadius, Galley, Rect, Role, Sense, Stroke, Ui, WidgetInfo, pos2, vec2,
};

use crate::i18n::{Text, pt, t};
use crate::theme;
use crate::view::{Icon, paint_icon};

const ROW_HEIGHT: f32 = 28.0;
const COLUMN_GAP: f32 = 32.0;
const CAP_HEIGHT: f32 = 20.0;
const CAP_MIN_WIDTH: f32 = 20.0;
const CAP_PADDING: f32 = 6.0;
const CAP_GAP: f32 = 4.0;
const JOINER_GAP: f32 = 6.0;

enum Part {
    Key(Text),
    Icon(Icon),
    Plus,
    Or,
    Then,
}

use Part::{Or, Plus, Then};

fn key(text: &str) -> Part {
    Part::Key(Text::Owned(text.to_owned()))
}

fn word(text: Text) -> Part {
    Part::Key(text)
}

pub(super) fn keyboard(ui: &mut Ui) {
    let rows: [(Vec<Part>, Text); 11] = [
        (
            vec![key("Ctrl"), Plus, key("F"), Or, key("/")],
            pt!("shortcut-search", "Search"),
        ),
        (vec![key("Ctrl"), Plus, key("1 – 6")], t!("Switch page")),
        (
            vec![
                key("Esc"),
                Or,
                key("Alt"),
                Plus,
                Part::Icon(Icon::ArrowLeft),
                Or,
                word(t!("mouse back")),
            ],
            t!("Leave a detail page"),
        ),
        (
            vec![key("Shift"), Plus, word(t!("wheel"))],
            t!("Scroll a row sideways"),
        ),
        (vec![word(t!("Space")), Or, key("K")], t!("Play or pause")),
        (
            vec![key("←"), key("→"), Or, key("J"), key("L")],
            t!("Seek by the seek step"),
        ),
        (
            vec![key("Shift"), Plus, key("←"), key("→")],
            t!("Seek by the short seek step"),
        ),
        (
            vec![key("↑"), key("↓"), Then, key("M")],
            t!("Volume  ·  mute"),
        ),
        (vec![key("C")], t!("Subtitles on or off")),
        (
            vec![key("["), key("]"), Then, word(t!("Backspace"))],
            t!("Slower or faster  ·  normal speed"),
        ),
        (vec![key("F"), Or, key("F11")], t!("Fullscreen")),
    ];
    let font = theme::caption();
    let laid: Vec<Vec<(f32, Option<Arc<Galley>>)>> = rows
        .iter()
        .map(|(parts, _)| {
            parts
                .iter()
                .map(|part| match part {
                    Part::Key(text) => {
                        let galley = ui.painter().layout_no_wrap(
                            text.to_string(),
                            font.clone(),
                            theme::TEXT_BRIGHT,
                        );
                        let width = (galley.size().x + 2.0 * CAP_PADDING).max(CAP_MIN_WIDTH);
                        (width, Some(galley))
                    }
                    Part::Icon(_) => (CAP_MIN_WIDTH, None),
                    Part::Plus | Part::Or | Part::Then => {
                        let galley = ui.painter().layout_no_wrap(
                            joiner(part).to_owned(),
                            theme::body(),
                            theme::TEXT,
                        );
                        (galley.size().x, Some(galley))
                    }
                })
                .collect()
        })
        .collect();
    let keys_width = rows
        .iter()
        .zip(&laid)
        .map(|((parts, _), laid)| keys_width(parts, laid))
        .fold(0.0, f32::max);
    for ((parts, what), laid) in rows.iter().zip(laid) {
        let (rect, response) =
            ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::hover());
        response.widget_info(|| WidgetInfo::labeled(Role::Label, true, &**what));
        if ui.is_rect_visible(rect) {
            shortcut(ui, rect, keys_width, parts, laid, what);
        }
    }
}

fn keys_width(parts: &[Part], laid: &[(f32, Option<Arc<Galley>>)]) -> f32 {
    laid.iter().map(|(width, _)| width).sum::<f32>() + gaps(parts).iter().sum::<f32>()
}

fn shortcut(
    ui: &Ui,
    rect: Rect,
    keys_width: f32,
    parts: &[Part],
    laid: Vec<(f32, Option<Arc<Galley>>)>,
    what: &str,
) {
    let painter = ui.painter();
    let y = rect.center().y;
    let gaps = gaps(parts);
    let mut x = rect.left();
    for (i, (part, (width, galley))) in parts.iter().zip(laid).enumerate() {
        let slot = Rect::from_min_size(pos2(x, y - CAP_HEIGHT / 2.0), vec2(width, CAP_HEIGHT));
        if matches!(part, Part::Key(_) | Part::Icon(_)) {
            cap(painter, slot);
        }
        if let Part::Icon(icon) = part {
            paint_icon(painter, *icon, slot.center(), 13.0, theme::TEXT_BRIGHT);
        }
        if let Some(galley) = galley {
            let color = if matches!(part, Part::Key(_)) {
                theme::TEXT_BRIGHT
            } else {
                theme::TEXT
            };
            painter.galley(slot.center() - galley.size() / 2.0, galley, color);
        }
        x += width + gaps.get(i).copied().unwrap_or(0.0);
    }
    painter.text(
        pos2(rect.left() + keys_width + COLUMN_GAP, y),
        Align2::LEFT_CENTER,
        what,
        theme::body(),
        theme::TEXT_DIM,
    );
}

fn cap(painter: &egui::Painter, rect: Rect) {
    let radius = CornerRadius::same(theme::RADIUS);
    painter.rect_filled(rect.translate(vec2(0.0, 1.5)), radius, theme::SIDEBAR);
    painter.rect_filled(rect, radius, theme::SURFACE);
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(1.0, theme::SURFACE_HOVER),
        egui::StrokeKind::Inside,
    );
}

fn joiner(part: &Part) -> &'static str {
    match part {
        Part::Plus => "+",
        Part::Then => "·",
        _ => "/",
    }
}

fn gaps(parts: &[Part]) -> Vec<f32> {
    parts
        .windows(2)
        .map(|pair| match pair {
            [Part::Key(_) | Part::Icon(_), Part::Key(_) | Part::Icon(_)] => CAP_GAP,
            _ => JOINER_GAP,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_sit_closer_together_than_joiners() {
        assert_eq!(
            gaps(&[key("Ctrl"), Plus, key("F"), Or, key("/")]),
            [JOINER_GAP; 4]
        );
        assert_eq!(
            gaps(&[key("←"), key("→"), Then, key("M")]),
            [CAP_GAP, JOINER_GAP, JOINER_GAP]
        );
        assert!(gaps(&[key("M")]).is_empty());
    }
}
