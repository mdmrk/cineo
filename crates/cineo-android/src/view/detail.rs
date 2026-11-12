use cineo_core::addon::{Meta, MetaPreview, SourceKind, Stream, Video};
use cineo_core::app::{
    Detail, LibraryItem, Loadable, Resolution, StreamFacts, StreamGroup, StreamSort, Tier,
    stream_matcher,
};

use super::{ProblemView, problem_view};

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct DetailView {
    pub id: String,
    pub name: Option<String>,
    pub favorite: bool,
    pub poster: Option<String>,
    pub background: Option<String>,
    pub logo: Option<String>,
    pub description: Option<String>,
    pub release_info: Option<String>,
    pub runtime: Option<String>,
    pub genres: Vec<String>,
    pub imdb_rating: Option<String>,
    pub meta: MetaStatus,
    pub videos: Vec<VideoView>,
    pub selected_video: Option<String>,
    pub streams: Vec<StreamGroupView>,
    pub qualities: Vec<Resolution>,
    pub filter: StreamFilterView,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum MetaStatus {
    Loading,
    Ready,
    Failed { problem: ProblemView },
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct VideoView {
    pub id: String,
    pub title: String,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub released: Option<String>,
    pub thumbnail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct StreamGroupView {
    pub addon_name: String,
    pub items: StreamItems,
}

#[uniffi::remote(Enum)]
pub enum StreamSort {
    AddonOrder,
    Quality,
    Seeders,
    Size,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, uniffi::Record)]
pub struct StreamFilterView {
    pub query: String,
    pub quality: Option<Resolution>,
    pub addon: Option<u32>,
    pub sort: StreamSort,
}

#[uniffi::remote(Enum)]
pub enum Resolution {
    Uhd,
    Qhd,
    FullHd,
    Hd,
    Sd,
    Cam,
}

#[uniffi::remote(Enum)]
pub enum Tier {
    Ultra,
    High,
    Standard,
    Low,
    Cam,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct QualityView {
    pub resolution: Resolution,
    pub tier: Tier,
    pub flags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum StreamItems {
    Loading,
    Ready {
        streams: Vec<StreamView>,
        order: Vec<u32>,
        hidden: u32,
    },
    Failed {
        problem: ProblemView,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct StreamView {
    pub name: Option<String>,
    pub description: Option<String>,
    pub kind: SourceKind,
    pub quality: Option<QualityView>,
}

pub(super) fn detail(
    detail: &Detail,
    library: &[LibraryItem],
    filter: &StreamFilterView,
    p2p_enabled: bool,
) -> DetailView {
    let meta = detail.meta.ready();
    let facts: Vec<Option<Vec<StreamFacts>>> = detail
        .streams
        .iter()
        .map(|g| {
            g.streams
                .ready()
                .map(|s| s.iter().map(StreamFacts::of).collect())
        })
        .collect();
    let preview = meta.map(|m| &m.preview).or(detail.preview.as_ref());
    let text = |pick: fn(&MetaPreview) -> Option<&String>| preview.and_then(pick).cloned();
    let url = |pick: fn(&MetaPreview) -> Option<&url::Url>| {
        preview.and_then(pick).map(ToString::to_string)
    };
    DetailView {
        id: detail.id.clone(),
        name: preview.map(|p| p.name.clone()),
        favorite: library.iter().any(|i| i.id == detail.id && i.is_favorite()),
        poster: url(|p| p.poster.as_ref()),
        background: url(|p| p.background.as_ref()),
        logo: url(|p| p.logo.as_ref()),
        description: text(|p| p.description.as_ref()),
        release_info: text(|p| p.release_info.as_ref()),
        runtime: meta.and_then(|m: &Meta| m.runtime.clone()),
        genres: preview.map_or_default(|p| p.genres.clone()),
        imdb_rating: text(|p| p.imdb_rating.as_ref()),
        meta: match &detail.meta {
            Loadable::Loading => MetaStatus::Loading,
            Loadable::Ready(_) => MetaStatus::Ready,
            Loadable::Failed(problem) => MetaStatus::Failed {
                problem: problem_view(problem),
            },
        },
        videos: meta.map_or_default(|m| m.videos.iter().map(video).collect()),
        selected_video: detail.selected_video.clone(),
        streams: detail
            .streams
            .iter()
            .zip(&facts)
            .enumerate()
            .map(|(index, (group, facts))| {
                stream_group(index, group, facts.as_deref(), filter, p2p_enabled)
            })
            .collect(),
        qualities: qualities(&detail.streams, &facts, p2p_enabled),
        filter: filter.clone(),
    }
}

fn qualities(
    groups: &[StreamGroup],
    facts: &[Option<Vec<StreamFacts>>],
    p2p_enabled: bool,
) -> Vec<Resolution> {
    let mut shown: Vec<Resolution> = groups
        .iter()
        .zip(facts)
        .filter_map(|(group, facts)| group.streams.ready().zip(facts.as_ref()))
        .flat_map(|(streams, facts)| streams.iter().zip(facts))
        .filter(|(stream, _)| p2p_enabled || !stream.source.is_p2p())
        .filter_map(|(_, facts)| facts.quality.map(|q| q.resolution))
        .collect();
    shown.sort_unstable();
    shown.dedup();
    shown
}

fn video(video: &Video) -> VideoView {
    VideoView {
        id: video.id.clone(),
        title: video.title.clone(),
        season: video.season,
        episode: video.episode,
        released: video.released.as_deref().map(day),
        thumbnail: video.thumbnail.as_ref().map(ToString::to_string),
    }
}

fn day(released: &str) -> String {
    let date = released.split('T').next().unwrap_or(released);
    let is_day = date.len() == 10
        && date.bytes().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                b == b'-'
            } else {
                b.is_ascii_digit()
            }
        });
    if is_day { date } else { released }.to_owned()
}

fn stream_group(
    index: usize,
    group: &StreamGroup,
    facts: Option<&[StreamFacts]>,
    filter: &StreamFilterView,
    p2p_enabled: bool,
) -> StreamGroupView {
    StreamGroupView {
        addon_name: group.addon_name.clone(),
        items: match (&group.streams, facts) {
            (Loadable::Loading, _) | (Loadable::Ready(_), None) => StreamItems::Loading,
            (Loadable::Ready(streams), Some(facts)) => {
                let matches = stream_matcher(&filter.query, filter.quality);
                let other_addon = filter.addon.is_some_and(|a| a as usize != index);
                let mut hidden = 0;
                let order = filter
                    .sort
                    .order(facts)
                    .into_iter()
                    .filter(|&i| {
                        if !p2p_enabled && streams[i].source.is_p2p() {
                            hidden += 1;
                            return false;
                        }
                        !other_addon && matches(&facts[i])
                    })
                    .filter_map(|i| u32::try_from(i).ok())
                    .collect();
                StreamItems::Ready {
                    streams: streams.iter().zip(facts).map(stream).collect(),
                    order,
                    hidden,
                }
            }
            (Loadable::Failed(problem), _) => StreamItems::Failed {
                problem: problem_view(problem),
            },
        },
    }
}

fn stream((stream, facts): (&Stream, &StreamFacts)) -> StreamView {
    StreamView {
        name: stream.name.clone(),
        description: stream.description.clone(),
        kind: stream.source.kind(),
        quality: facts.quality.map(|q| QualityView {
            resolution: q.resolution,
            tier: q.tier(),
            flags: q.flags.iter().map(|f| (*f).to_owned()).collect(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::day;

    #[test]
    fn release_times_show_only_their_day() {
        assert_eq!(day("2025-07-14T04:00:00.000Z"), "2025-07-14");
        assert_eq!(day("2025-07-14"), "2025-07-14");
        assert_eq!(day("July 2025"), "July 2025");
        assert_eq!(day("2025T"), "2025T");
    }
}
