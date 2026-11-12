use cineo_core::app::TorrentStatus;
use cineo_player::embedded::Status;
use cineo_player::plain_text;
use eframe::egui::{self, Color32, CornerRadius, Rect, Ui, pos2, vec2};

use crate::i18n::{Text, t};
use crate::theme;

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

pub(super) fn stat_rows(status: &Status, torrent: Option<&TorrentStatus>) -> Vec<(Text, String)> {
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
        rows.push((t!("Download"), format!("{}/s", bytes(rate))));
        rows.push((t!("Peers"), peers.to_string()));
        rows.push((
            t!("Downloaded"),
            t!(
                "{done} of {total}",
                done = bytes(done),
                total = bytes(total)
            )
            .into_owned(),
        ));
    }
    let stats = &status.stats;
    if let Some(speed) = stats.cache_speed {
        rows.push((t!("Network"), format!("{}/s", bytes(speed))));
    }
    if let Some(seconds) = stats.cache_s {
        rows.push((t!("Buffered"), format!("{seconds:.0} s")));
    }
    if let (Some(width), Some(height)) = (stats.width, stats.height) {
        rows.push((t!("Resolution"), format!("{width:.0}×{height:.0}")));
    }
    let codec = status.video_codec();
    let bitrate = stats
        .video_bitrate
        .filter(|b| *b > 0.0)
        .map(|b| format!("{:.1} Mb/s", b / 1_000_000.0));
    let video: Vec<String> = codec.into_iter().chain(bitrate).collect();
    if !video.is_empty() {
        rows.push((t!("Video"), video.join(" · ")));
    }
    if let Some(hwdec) = &stats.hwdec {
        let decoder = if hwdec == "no" {
            t!("Software").into_owned()
        } else {
            plain_text(hwdec)
        };
        rows.push((t!("Decoder"), decoder));
    }
    if let Some(dropped) = stats.dropped_frames {
        rows.push((t!("Dropped frames"), format!("{dropped:.0}")));
    }
    rows
}

fn bytes(count: f64) -> String {
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

pub(super) fn stats_overlay(ui: &Ui, rect: Rect, rows: &[(Text, String)]) {
    if rows.is_empty() {
        return;
    }
    let painter = ui.painter();
    let font = theme::body();
    let row_height = 20.0;
    let pad = 12.0;
    let layout = |text: &str, color| painter.layout_no_wrap(text.to_owned(), font.clone(), color);
    let labels: Vec<_> = rows
        .iter()
        .map(|(l, _)| layout(l, theme::TEXT_DIM))
        .collect();
    let values: Vec<_> = rows
        .iter()
        .map(|(_, v)| layout(v, theme::TEXT_BRIGHT))
        .collect();
    let label_width = labels.iter().map(|g| g.size().x).fold(0.0, f32::max);
    let value_width = values.iter().map(|g| g.size().x).fold(0.0, f32::max);
    #[expect(clippy::cast_precision_loss, reason = "a handful of rows")]
    let size = vec2(
        label_width + 16.0 + value_width + 2.0 * pad,
        rows.len() as f32 * row_height + 2.0 * pad,
    );
    let panel = Rect::from_min_size(rect.min + vec2(24.0, 80.0), size);
    painter.rect_filled(
        panel,
        CornerRadius::same(theme::RADIUS),
        Color32::from_black_alpha(170),
    );
    let mut y = panel.top() + pad + row_height / 2.0;
    for (label, value) in labels.into_iter().zip(values) {
        let x = panel.left() + pad;
        painter.galley(pos2(x, y - label.size().y / 2.0), label, theme::TEXT_DIM);
        painter.galley(
            pos2(x + label_width + 16.0, y - value.size().y / 2.0),
            value,
            theme::TEXT_BRIGHT,
        );
        y += row_height;
    }
}

#[cfg(test)]
mod tests {
    use cineo_player::embedded::{Track, TrackKind};

    use super::*;

    #[test]
    fn byte_counts_scale_to_readable_units() {
        assert_eq!(bytes(512.0), "512 B");
        assert_eq!(bytes(1536.0), "1.5 KB");
        assert_eq!(bytes(2.5 * 1024.0 * 1024.0), "2.5 MB");
        assert_eq!(bytes(3.0 * 1024.0 * 1024.0 * 1024.0 * 1024.0), "3072.0 GB");
    }

    #[test]
    fn stats_show_torrent_and_player_figures_that_exist() {
        let status = Status {
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
                width: Some(1920.0),
                height: Some(1080.0),
                video_bitrate: Some(5_200_000.0),
                hwdec: Some("no".into()),
                ..Default::default()
            },
            ..Status::default()
        };
        let torrent = TorrentStatus::Streaming {
            peers: 12,
            download_bytes_per_sec: 2 * 1024 * 1024,
            downloaded: 512 * 1024 * 1024,
            size: 1024 * 1024 * 1024,
        };
        let rows = stat_rows(&status, Some(&torrent));
        let values: Vec<&str> = rows.iter().map(|(_, v)| v.as_str()).collect();
        assert_eq!(
            values,
            [
                "2.0 MB/s",
                "12",
                &t!("{done} of {total}", done = "512.0 MB", total = "1.0 GB"),
                "1.0 MB/s",
                "1920×1080",
                "hevc · 5.2 Mb/s",
                &t!("Software"),
            ]
        );
        assert!(stat_rows(&Status::default(), Some(&TorrentStatus::Starting)).is_empty());
    }
}
