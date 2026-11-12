use std::path::Path;

use cineo_core::app::{AudioOutput, Language, Settings, SubtitleFont};

use super::PlayerCommand;

pub(super) fn start_option(start_ms: u64) -> String {
    format!("{}.{:03}", start_ms / 1000, start_ms % 1000)
}

pub(super) fn settings_options(settings: &Settings) -> Vec<(&'static str, String)> {
    let mut options = Vec::new();
    if !settings.hardware_decoding {
        options.push(("hwdec", "no".to_owned()));
    }
    let alang = language_list(settings.audio_languages());
    if !alang.is_empty() {
        options.push(("alang", alang));
    }
    let slang = language_list(settings.subtitle_languages());
    if !slang.is_empty() {
        options.push(("slang", slang));
    }
    if settings.remember_volume {
        options.push(("volume", settings.volume.get().to_string()));
    }
    if settings.audio_output == AudioOutput::Stereo {
        options.push(("audio-channels", "stereo".to_owned()));
    }
    if settings.audio_passthrough {
        options.push(("audio-spdif", "ac3,eac3,dts,dts-hd,truehd".to_owned()));
    }
    options
}

pub(super) fn style_options(settings: &Settings) -> Vec<(&'static str, String)> {
    let defaults = style_properties(&Settings::default());
    style_properties(settings)
        .into_iter()
        .zip(defaults)
        .filter(|(set, default)| set != default)
        .map(|(set, _)| set)
        .collect()
}

pub(super) fn style_properties(settings: &Settings) -> Vec<(&'static str, String)> {
    let scale = f64::from(settings.subtitle_size.get()) / 100.0;
    let font = match settings.subtitle_font {
        SubtitleFont::Sans => "sans-serif",
        SubtitleFont::Serif => "serif",
        SubtitleFont::Mono => "monospace",
    };
    let alpha = percent_alpha(settings.subtitle_opacity.get());
    let (style, border, size) = match settings.subtitle_background.rgb() {
        Some(rgb) => ("opaque-box", argb(0xCC, rgb), "1.65"),
        None => match settings.subtitle_outline.rgb() {
            Some(rgb) => ("outline-and-shadow", argb(0xFF, rgb), "1.65"),
            None => ("outline-and-shadow", argb(0xFF, [0, 0, 0]), "0"),
        },
    };
    let bold = if settings.subtitle_bold { "yes" } else { "no" };
    let ass = if settings.keep_subtitle_styles {
        "scale"
    } else {
        "force"
    };
    vec![
        ("sub-scale", format!("{scale:.2}")),
        ("sub-font", font.to_owned()),
        ("sub-bold", bold.to_owned()),
        (
            "sub-pos",
            (100 - settings.subtitle_position.get()).to_string(),
        ),
        ("sub-color", argb(alpha, settings.subtitle_color.rgb())),
        ("sub-border-style", style.to_owned()),
        ("sub-border-color", border),
        ("sub-border-size", size.to_owned()),
        ("sub-ass-override", ass.to_owned()),
    ]
}

pub(super) fn percent_alpha(percent: u32) -> u8 {
    u8::try_from(percent.min(100) * 255 / 100).unwrap_or(u8::MAX)
}

pub(super) fn argb(alpha: u8, [r, g, b]: [u8; 3]) -> String {
    format!("#{alpha:02X}{r:02X}{g:02X}{b:02X}")
}

pub(super) fn language_list(languages: impl Iterator<Item = Language>) -> String {
    languages
        .flat_map(Language::codes)
        .collect::<Vec<_>>()
        .join(",")
}

pub(super) fn sub_add_args<'a>(
    path: &'a Path,
    title: &'a str,
    lang: &'a str,
) -> Option<[&'a str; 5]> {
    if !path.is_absolute() {
        return None;
    }
    Some(["sub-add", path.to_str()?, "cached", title, lang])
}

pub(super) fn command_args(command: PlayerCommand) -> Option<Vec<String>> {
    let track = |id: Option<i64>| id.map_or_else(|| "no".to_owned(), |id| id.to_string());
    let args = match command {
        PlayerCommand::TogglePause => vec!["cycle".into(), "pause".into()],
        PlayerCommand::SetPause(paused) => vec![
            "set".into(),
            "pause".into(),
            if paused { "yes" } else { "no" }.into(),
        ],
        PlayerCommand::SeekTo(seconds) if seconds.is_finite() => vec![
            "seek".into(),
            format!("{:.3}", seconds.max(0.0)),
            "absolute".into(),
        ],
        PlayerCommand::SeekBy(seconds) if seconds.is_finite() => {
            vec!["seek".into(), format!("{seconds:.3}"), "relative".into()]
        }
        PlayerCommand::SetVolume(percent) if percent.is_finite() => vec![
            "set".into(),
            "volume".into(),
            format!("{:.0}", percent.clamp(0.0, 100.0)),
        ],
        PlayerCommand::SetSubtitleDelay(seconds) if seconds.is_finite() => {
            vec!["set".into(), "sub-delay".into(), format!("{seconds:.3}")]
        }
        PlayerCommand::SeekTo(_)
        | PlayerCommand::SeekBy(_)
        | PlayerCommand::SetVolume(_)
        | PlayerCommand::SetSubtitleDelay(_) => return None,
        PlayerCommand::ToggleMute => vec!["cycle".into(), "mute".into()],
        PlayerCommand::SetMute(muted) => vec![
            "set".into(),
            "mute".into(),
            if muted { "yes" } else { "no" }.into(),
        ],
        PlayerCommand::SetAudio(id) => vec!["set".into(), "aid".into(), track(id)],
        PlayerCommand::SetSubtitle(id) => vec!["set".into(), "sid".into(), track(id)],
        PlayerCommand::Stop => vec!["quit".into()],
    };
    Some(args)
}
