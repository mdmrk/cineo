use crate::addon::Stream;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Resolution {
    Uhd,
    Qhd,
    FullHd,
    Hd,
    Sd,
    Cam,
}

impl Resolution {
    pub const ALL: [Self; 6] = [
        Self::Uhd,
        Self::Qhd,
        Self::FullHd,
        Self::Hd,
        Self::Sd,
        Self::Cam,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Uhd => "4K",
            Self::Qhd => "1440p",
            Self::FullHd => "1080p",
            Self::Hd => "720p",
            Self::Sd => "SD",
            Self::Cam => "CAM",
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|r| r.label() == label)
    }

    pub const fn tier(self) -> Tier {
        match self {
            Self::Uhd => Tier::Ultra,
            Self::Qhd | Self::FullHd => Tier::High,
            Self::Hd => Tier::Standard,
            Self::Sd => Tier::Low,
            Self::Cam => Tier::Cam,
        }
    }

    const fn names(self) -> &'static [&'static str] {
        match self {
            Self::Uhd => &["2160p", "4k", "uhd"],
            Self::Qhd => &["1440p", "2k"],
            Self::FullHd => &["1080p", "fhd"],
            Self::Hd => &["720p"],
            Self::Sd => &["576p", "480p", "360p", "sd"],
            Self::Cam => &["cam", "hdcam", "camrip", "telesync", "hdts"],
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StreamSort {
    #[default]
    AddonOrder,
    Quality,
    Seeders,
    Size,
}

impl StreamSort {
    pub const ALL: [Self; 4] = [Self::AddonOrder, Self::Quality, Self::Seeders, Self::Size];

    pub fn order(self, facts: &[Facts]) -> Vec<usize> {
        let mut order: Vec<usize> = (0..facts.len()).collect();
        let key: fn(&Facts) -> Option<u64> = match self {
            Self::AddonOrder => return order,
            Self::Quality => Facts::quality_rank,
            Self::Seeders => |facts| facts.seeders,
            Self::Size => |facts| facts.size,
        };
        order.sort_by_key(|&i| std::cmp::Reverse(key(&facts[i])));
        order
    }
}

#[derive(Debug, Clone)]
pub struct Facts {
    pub quality: Option<Quality>,
    pub seeders: Option<u64>,
    pub size: Option<u64>,
    pub text: String,
}

impl Facts {
    pub fn of(stream: &Stream) -> Self {
        Self {
            quality: Quality::of(stream),
            seeders: seeders(stream),
            size: size_bytes(stream),
            text: stream_text(stream)
                .map(str::to_lowercase)
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }

    pub fn quality_rank(&self) -> Option<u64> {
        let worse = Resolution::ALL.len() - self.quality?.resolution as usize;
        u64::try_from(worse).ok()
    }
}

fn stream_text(stream: &Stream) -> impl Iterator<Item = &str> {
    [stream.name.as_deref(), stream.description.as_deref()]
        .into_iter()
        .flatten()
}

fn seeders(stream: &Stream) -> Option<u64> {
    stream_text(stream).find_map(|text| {
        let (_, after) = text.split_once('👤')?;
        let after = after.trim_start();
        let end = after
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(after.len());
        after[..end].parse().ok()
    })
}

fn size_bytes(stream: &Stream) -> Option<u64> {
    stream
        .video_size
        .or_else(|| stream_text(stream).find_map(size_in_text))
}

fn size_in_text(text: &str) -> Option<u64> {
    let tokens = text.split_whitespace();
    tokens
        .clone()
        .zip(tokens.skip(1))
        .find_map(|(value, unit)| {
            let value: f64 = value.parse().ok()?;
            let unit: f64 = match unit.trim_end_matches(|c: char| !c.is_ascii_alphabetic()) {
                "KB" | "KiB" => 1024.0,
                "MB" | "MiB" => 1024.0 * 1024.0,
                "GB" | "GiB" => 1024.0 * 1024.0 * 1024.0,
                "TB" | "TiB" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
                _ => return None,
            };
            let bytes = value * unit;
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a finite, non-negative size"
            )]
            (bytes.is_finite() && bytes >= 0.0).then_some(bytes as u64)
        })
}

pub fn matcher(query: &str, quality: Option<Resolution>) -> impl Fn(&Facts) -> bool {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    move |facts| {
        quality.is_none_or(|quality| facts.quality.map(|q| q.resolution) == Some(quality))
            && words.iter().all(|word| facts.text.contains(word.as_str()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Ultra,
    High,
    Standard,
    Low,
    Cam,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quality {
    pub resolution: Resolution,
    pub flags: &'static [&'static str],
}

impl Quality {
    pub fn of(stream: &Stream) -> Option<Self> {
        let name = stream.name.as_deref().unwrap_or_default();
        let description = stream.description.as_deref().unwrap_or_default();
        let resolution = resolution(name).or_else(|| resolution(description))?;
        let (mut hdr, mut dv) = (false, false);
        for word in words(name).chain(words(description)) {
            hdr |= word.get(..3).is_some_and(|p| p.eq_ignore_ascii_case("hdr"));
            dv |= word.eq_ignore_ascii_case("dv") || word.eq_ignore_ascii_case("dovi");
        }
        let flags: &[&str] = match (hdr, dv) {
            (true, true) => &["HDR", "DV"],
            (true, false) => &["HDR"],
            (false, true) => &["DV"],
            (false, false) => &[],
        };
        Some(Self { resolution, flags })
    }

    pub const fn label(self) -> &'static str {
        self.resolution.label()
    }

    pub const fn tier(self) -> Tier {
        self.resolution.tier()
    }
}

fn words(text: &str) -> impl Iterator<Item = &str> + Clone {
    text.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
}

fn resolution(text: &str) -> Option<Resolution> {
    words(text).find_map(|word| {
        Resolution::ALL
            .into_iter()
            .find(|r| r.names().iter().any(|n| word.eq_ignore_ascii_case(n)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stream(name: Option<&str>, description: Option<&str>) -> Stream {
        let field = |key: &str, value: Option<&str>| {
            value.map_or(String::new(), |v| {
                format!(r#","{key}":"{}""#, v.replace('\n', "\\n"))
            })
        };
        let json = format!(
            r#"{{"streams":[{{"infoHash":"0123456789abcdef0123456789abcdef01234567"{}{}}}]}}"#,
            field("name", name),
            field("description", description),
        );
        crate::addon::parse_stream_response(json.as_bytes())
            .unwrap()
            .value
            .remove(0)
    }

    #[test]
    fn quality_comes_from_the_name_then_the_description() {
        let q = Quality::of(&stream(Some("Torrentio\n4k DV | HDR"), None)).unwrap();
        assert_eq!(
            q,
            Quality {
                resolution: Resolution::Uhd,
                flags: &["HDR", "DV"]
            }
        );
        let q = Quality::of(&stream(
            Some("Torrentio"),
            Some("Film.2023.1080p.WEB-DL.x264\n👤 12 💾 1.2 GB"),
        ))
        .unwrap();
        assert_eq!((q.label(), q.tier()), ("1080p", Tier::High));
        assert!(q.flags.is_empty());
        let q = Quality::of(&stream(Some("HDCAM"), None)).unwrap();
        assert_eq!(q.tier(), Tier::Cam);
    }

    #[test]
    fn seeders_and_size_come_from_the_description() {
        let s = stream(
            Some("Example\n1080p"),
            Some("Night of the Living Dead 1968 1080p BluRay\n👤 100 💾 1.51 GB ⚙️ YTS"),
        );
        assert_eq!(seeders(&s), Some(100));
        assert_eq!(size_bytes(&s), Some(1_621_350_154));
        let bare = stream(Some("1080p"), Some("No numbers here, 12 seeds"));
        assert_eq!(seeders(&bare), None);
        assert_eq!(size_bytes(&bare), None);
        assert_eq!(size_in_text("700 MB"), Some(734_003_200));
        assert_eq!(size_in_text("2 TiB,"), Some(2_199_023_255_552));
        let hinted = Stream {
            video_size: Some(42),
            ..s
        };
        assert_eq!(size_bytes(&hinted), Some(42), "the addon's hint wins");
    }

    #[test]
    fn sorting_puts_larger_values_first_and_unknown_last_keeping_ties_in_order() {
        let streams = [
            stream(Some("720p"), Some("👤 5 💾 700 MB")),
            stream(Some("plain"), None),
            stream(Some("4k"), Some("👤 5 💾 15.77 GB")),
            stream(Some("1080p"), Some("👤 39 💾 1.5 GB")),
        ];
        let facts: Vec<Facts> = streams.iter().map(Facts::of).collect();
        assert_eq!(StreamSort::AddonOrder.order(&facts), [0, 1, 2, 3]);
        assert_eq!(StreamSort::Quality.order(&facts), [2, 3, 0, 1]);
        assert_eq!(StreamSort::Seeders.order(&facts), [3, 0, 2, 1]);
        assert_eq!(StreamSort::Size.order(&facts), [2, 3, 0, 1]);
    }

    #[test]
    fn stream_filter_needs_every_word_and_the_quality() {
        let s = stream(Some("Torrentio\n1080p"), Some("Film.2024.WEB-DL 👤 12"));
        let facts = Facts::of(&s);
        let matches = |query: &str, quality| matcher(query, quality)(&facts);
        assert!(matches("", None));
        assert!(matches("web-dl  TORRENTIO", None));
        assert!(!matches("web-dl hevc", None));
        assert!(matches("film", Some(Resolution::FullHd)));
        assert!(!matches("", Some(Resolution::Uhd)));
    }

    #[test]
    fn resolutions_are_read_back_from_their_labels() {
        for resolution in Resolution::ALL {
            assert_eq!(Resolution::from_label(resolution.label()), Some(resolution));
        }
        assert_eq!(Resolution::from_label("8K"), None);
    }

    #[test]
    fn quality_needs_a_whole_word() {
        assert_eq!(
            Quality::of(&stream(Some("Torrent"), Some("Scam 4kHz"))),
            None
        );
        assert_eq!(Quality::of(&stream(None, None)), None);
    }
}
