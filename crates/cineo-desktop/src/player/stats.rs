use std::collections::VecDeque;

use cineo_core::app::TorrentStatus;
use cineo_player::embedded::Status;
use cineo_player::plain_text;
use eframe::egui::text::LayoutJob;
use eframe::egui::{
    self, Color32, CornerRadius, CursorIcon, FontId, Painter, Pos2, Rect, Sense, Shape, Stroke, Ui,
    pos2, vec2,
};

use crate::i18n::{Text, t};
use crate::theme;

const SAMPLE_S: f64 = 1.0;
const SAMPLES: usize = 60;

fn stats_id() -> egui::Id {
    egui::Id::new("player-stats")
}

pub(super) fn stats_shown(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp(stats_id())).unwrap_or(false)
}

pub(super) fn toggle_stats_shown(ctx: &egui::Context) {
    let shown = !stats_shown(ctx);
    ctx.data_mut(|d| d.insert_temp(stats_id(), shown));
}

#[derive(Debug, Clone, Default)]
pub(super) struct History {
    samples: VecDeque<f64>,
    last: Option<f64>,
}

impl History {
    fn sample(&mut self, value: Option<f64>, now: f64) {
        let Some(value) = value else {
            return;
        };
        if self.last.is_some_and(|last| now - last < SAMPLE_S) {
            return;
        }
        self.last = Some(now);
        if self.samples.len() == SAMPLES {
            self.samples.pop_front();
        }
        self.samples.push_back(value.max(0.0));
    }

    fn peak(&self) -> f64 {
        self.samples.iter().copied().fold(0.0, f64::max)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Metric {
    Download,
    Network,
    Buffer,
}

#[derive(Debug, Clone, Default)]
pub(super) struct Trends {
    download: History,
    network: History,
    buffer: History,
}

impl Trends {
    pub(super) fn sample(&mut self, status: &Status, torrent: Option<&TorrentStatus>, now: f64) {
        #[expect(clippy::cast_precision_loss, reason = "display only")]
        let download = match torrent {
            Some(TorrentStatus::Streaming {
                download_bytes_per_sec,
                ..
            }) => Some(*download_bytes_per_sec as f64),
            _ => None,
        };
        self.download.sample(download, now);
        self.network.sample(status.stats.cache_speed, now);
        self.buffer.sample(status.stats.cache_s, now);
    }

    fn of(&self, metric: Metric) -> &History {
        match metric {
            Metric::Download => &self.download,
            Metric::Network => &self.network,
            Metric::Buffer => &self.buffer,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Row {
    pub(super) label: Text,
    pub(super) value: String,
    pub(super) warn: bool,
    pub(super) trend: Option<Metric>,
}

impl Row {
    fn new(label: Text, value: String) -> Self {
        Self {
            label,
            value,
            warn: false,
            trend: None,
        }
    }

    fn warn(mut self, warn: bool) -> Self {
        self.warn = warn;
        self
    }

    fn trend(mut self, metric: Metric) -> Self {
        self.trend = Some(metric);
        self
    }
}

pub(super) fn stat_rows(status: &Status, torrent: Option<&TorrentStatus>) -> Vec<Row> {
    let mut rows = Vec::new();
    if let Some(TorrentStatus::Streaming {
        peers,
        download_bytes_per_sec,
        downloaded,
        size,
    }) = torrent
    {
        #[expect(clippy::cast_precision_loss, reason = "display only")]
        let (rate, done, total) = (
            *download_bytes_per_sec as f64,
            *downloaded as f64,
            *size as f64,
        );
        rows.push(Row::new(t!("Download"), format!("{}/s", bytes(rate))).trend(Metric::Download));
        rows.push(Row::new(t!("Peers"), peers.to_string()).warn(*peers == 0));
        let mut progress = t!(
            "{done} of {total}",
            done = bytes(done),
            total = bytes(total)
        )
        .into_owned();
        if total > 0.0 {
            progress = format!(
                "{progress} ({:.0}%)",
                (done / total).clamp(0.0, 1.0) * 100.0
            );
        }
        rows.push(Row::new(t!("Downloaded"), progress));
    }
    let stats = &status.stats;
    if let Some(speed) = stats.cache_speed {
        rows.push(Row::new(t!("Network"), format!("{}/s", bytes(speed))).trend(Metric::Network));
    }
    if let Some(seconds) = stats.cache_s {
        rows.push(Row::new(t!("Buffered"), format!("{seconds:.2} s")).trend(Metric::Buffer));
    }
    if let (Some(width), Some(height)) = (stats.width, stats.height) {
        rows.push(Row::new(
            t!("Resolution"),
            format!("{width:.0}×{height:.0}"),
        ));
    }
    let codec = status.video_codec();
    let bitrate = stats
        .video_bitrate
        .filter(|b| *b > 0.0)
        .map(|b| format!("{:.1} Mb/s", b / 1_000_000.0));
    let format: Vec<String> = codec.into_iter().chain(bitrate).collect();
    if !format.is_empty() {
        rows.push(Row::new(t!("Codec"), format.join(" · ")));
    }
    if let Some(hwdec) = &stats.hwdec {
        let decoder = if hwdec == "no" {
            t!("Software").into_owned()
        } else {
            plain_text(hwdec)
        };
        rows.push(Row::new(t!("Decoder"), decoder));
    }
    if let Some(dropped) = stats.dropped_frames {
        rows.push(Row::new(t!("Dropped frames"), format!("{dropped:.0}")).warn(dropped > 0.0));
    }
    rows
}

pub(crate) fn bytes(count: f64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = count.max(0.0);
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

const FONT_SIZE: f32 = 12.0;
const PAD: f32 = 10.0;
const WIDTH: f32 = 340.0;
const LABEL_WIDTH: f32 = 116.0;
const ROW_HEIGHT: f32 = 16.0;
const GAP: f32 = 8.0;
const TREND_WIDTH: f32 = 72.0;
const TREND_HEIGHT: f32 = 10.0;
const CLOSE: f32 = 14.0;
const BACKGROUND: Color32 = Color32::from_rgba_premultiplied(20, 20, 20, 204);

fn one_line(text: &str, font: FontId, max_width: f32, color: Color32) -> LayoutJob {
    let mut job = LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap.max_width = max_width.max(0.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job
}

pub(super) fn stats_overlay(ui: &Ui, rect: Rect, rows: &[Row], trends: &Trends) {
    if rows.is_empty() {
        return;
    }
    let painter = ui.painter();
    let label_font = FontId::new(FONT_SIZE, theme::strong_family());
    let value_font = FontId::new(FONT_SIZE, egui::FontFamily::Proportional);
    #[expect(clippy::cast_precision_loss, reason = "a handful of rows")]
    let height = 2.0 * PAD + rows.len() as f32 * ROW_HEIGHT;
    let panel = Rect::from_min_size(rect.min + vec2(24.0, 80.0), vec2(WIDTH, height));
    painter.rect_filled(panel, CornerRadius::same(theme::RADIUS), BACKGROUND);
    let left = panel.left() + PAD;
    let values_left = left + LABEL_WIDTH;
    let right = panel.right() - PAD - CLOSE - GAP;
    let mut y = panel.top() + PAD;
    for row in rows {
        let mid = y + ROW_HEIGHT / 2.0;
        let label = painter.layout_job(one_line(
            &row.label,
            label_font.clone(),
            LABEL_WIDTH - GAP,
            theme::TEXT_BRIGHT,
        ));
        painter.galley(
            pos2(left, mid - label.size().y / 2.0),
            label,
            theme::TEXT_BRIGHT,
        );
        let mut x = values_left;
        if let Some(metric) = row.trend {
            trend(
                painter,
                Rect::from_min_size(
                    pos2(x, mid - TREND_HEIGHT / 2.0),
                    vec2(TREND_WIDTH, TREND_HEIGHT),
                ),
                trends.of(metric),
            );
            x += TREND_WIDTH + GAP;
        }
        let color = if row.warn {
            theme::WARNING
        } else {
            theme::TEXT
        };
        let value = painter.layout_job(one_line(&row.value, value_font.clone(), right - x, color));
        painter.galley(pos2(x, mid - value.size().y / 2.0), value, color);
        y += ROW_HEIGHT;
    }
    close_button(ui, panel);
}

fn close_button(ui: &Ui, panel: Rect) {
    let area = Rect::from_min_size(
        pos2(panel.right() - PAD - CLOSE, panel.top() + PAD),
        vec2(CLOSE, CLOSE),
    );
    let response = ui
        .interact(area, stats_id().with("close"), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    if response.clicked() {
        toggle_stats_shown(ui.ctx());
    }
    let color = if response.hovered() {
        theme::TEXT_BRIGHT
    } else {
        theme::TEXT_DIM
    };
    let cross = area.shrink(3.5);
    let stroke = Stroke::new(1.4, color);
    ui.painter()
        .line_segment([cross.left_top(), cross.right_bottom()], stroke);
    ui.painter()
        .line_segment([cross.right_top(), cross.left_bottom()], stroke);
}

fn trend(painter: &Painter, area: Rect, history: &History) {
    painter.rect_filled(area, CornerRadius::ZERO, Color32::from_white_alpha(10));
    let peak = history.peak();
    if history.samples.len() < 2 || peak <= 0.0 {
        return;
    }
    #[expect(clippy::cast_precision_loss, reason = "at most a minute of samples")]
    let step = area.width() / (SAMPLES - 1) as f32;
    #[expect(clippy::cast_precision_loss, reason = "at most a minute of samples")]
    let start = area.right() - step * (history.samples.len() - 1) as f32;
    #[expect(clippy::cast_possible_truncation, reason = "a fraction")]
    let points: Vec<Pos2> = history
        .samples
        .iter()
        .enumerate()
        .map(|(i, value)| {
            #[expect(clippy::cast_precision_loss, reason = "at most a minute of samples")]
            let x = start + step * i as f32;
            pos2(x, area.bottom() - area.height() * (value / peak) as f32)
        })
        .collect();
    let fill = Color32::from_rgba_unmultiplied(
        theme::ACCENT.r(),
        theme::ACCENT.g(),
        theme::ACCENT.b(),
        40,
    );
    for pair in points.windows(2) {
        painter.add(Shape::convex_polygon(
            vec![
                pair[0],
                pair[1],
                pos2(pair[1].x, area.bottom()),
                pos2(pair[0].x, area.bottom()),
            ],
            fill,
            Stroke::NONE,
        ));
    }
    painter.add(Shape::line(points, Stroke::new(1.0, theme::ACCENT)));
}

#[cfg(test)]
mod tests {
    use cineo_player::embedded::{Track, TrackKind};

    use super::*;
    use crate::i18n::t as tr;

    #[test]
    fn byte_counts_scale_to_readable_units() {
        assert_eq!(bytes(512.0), "512 B");
        assert_eq!(bytes(1536.0), "1.5 KB");
        assert_eq!(bytes(2.5 * 1024.0 * 1024.0), "2.5 MB");
        assert_eq!(bytes(3.0 * 1024.0 * 1024.0 * 1024.0 * 1024.0), "3072.0 GB");
    }

    fn playing() -> Status {
        Status {
            tracks: [Track {
                id: 1,
                kind: TrackKind::Video,
                title: None,
                lang: None,
                selected: true,
                external_file: None,
                codec: Some("hevc".into()),
            }]
            .into(),
            stats: cineo_player::embedded::Stats {
                cache_speed: Some(1024.0 * 1024.0),
                cache_s: Some(12.5),
                width: Some(1920.0),
                height: Some(1080.0),
                video_bitrate: Some(5_200_000.0),
                hwdec: Some("no".into()),
                ..Default::default()
            },
            ..Status::default()
        }
    }

    fn streaming(peers: u32, downloaded: u64, size: u64) -> TorrentStatus {
        TorrentStatus::Streaming {
            peers,
            download_bytes_per_sec: 2 * 1024 * 1024,
            downloaded,
            size,
        }
    }

    #[test]
    fn stats_show_torrent_and_player_figures_that_exist() {
        let torrent = streaming(12, 256 * 1024 * 1024, 1024 * 1024 * 1024);
        let rows = stat_rows(&playing(), Some(&torrent));
        let values: Vec<&str> = rows.iter().map(|r| r.value.as_str()).collect();
        let progress = format!(
            "{} (25%)",
            tr!("{done} of {total}", done = "256.0 MB", total = "1.0 GB")
        );
        assert_eq!(
            values,
            [
                "2.0 MB/s",
                "12",
                progress.as_str(),
                "1.0 MB/s",
                "12.50 s",
                "1920×1080",
                "hevc · 5.2 Mb/s",
                &tr!("Software"),
            ]
        );
        let trends: Vec<Option<Metric>> = rows.iter().map(|r| r.trend).collect();
        assert_eq!(
            trends,
            [
                Some(Metric::Download),
                None,
                None,
                Some(Metric::Network),
                Some(Metric::Buffer),
                None,
                None,
                None,
            ]
        );
        assert!(stat_rows(&Status::default(), Some(&TorrentStatus::Starting)).is_empty());
    }

    #[test]
    fn stalled_peers_and_dropped_frames_are_flagged() {
        let mut status = playing();
        status.stats.dropped_frames = Some(3.0);
        let rows = stat_rows(&status, Some(&streaming(0, 0, 0)));
        let warned: Vec<&str> = rows
            .iter()
            .filter(|r| r.warn)
            .map(|r| r.value.as_str())
            .collect();
        assert_eq!(warned, ["0", "3"]);
        assert_eq!(
            rows[2].value,
            tr!("{done} of {total}", done = "0 B", total = "0 B")
        );
    }

    #[test]
    fn trends_follow_each_figure_once_a_second() {
        let mut trends = Trends::default();
        let torrent = streaming(1, 0, 0);
        trends.sample(&playing(), Some(&torrent), 0.0);
        trends.sample(&playing(), None, 0.5);
        trends.sample(&Status::default(), None, 1.0);
        assert_eq!(
            Vec::from(trends.download.samples.clone()),
            [2.0 * 1024.0 * 1024.0]
        );
        assert_eq!(Vec::from(trends.network.samples.clone()), [1024.0 * 1024.0]);
        assert_eq!(Vec::from(trends.buffer.samples.clone()), [12.5]);
    }

    #[test]
    fn a_history_keeps_one_sample_a_second_for_a_minute() {
        let mut history = History::default();
        history.sample(Some(10.0), 0.0);
        history.sample(Some(99.0), 0.5);
        history.sample(None, 1.0);
        history.sample(Some(20.0), 1.0);
        assert_eq!(Vec::from(history.samples.clone()), [10.0, 20.0]);
        for second in 2..100 {
            history.sample(Some(f64::from(second)), f64::from(second));
        }
        assert_eq!(history.samples.len(), SAMPLES);
        assert_eq!(history.samples.front(), Some(&40.0));
        assert!((history.peak() - 99.0).abs() < f64::EPSILON);
    }
}
