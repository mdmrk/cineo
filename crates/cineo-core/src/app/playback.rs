use std::collections::HashSet;

use url::Url;

use super::library::{LibraryItem, SavedStream};
use super::message::{Notice, PlaybackFailure};
use super::plan;
use super::refresh::{fetch_streams, open_detail, select_video};
use super::settings::Settings;
use super::state::{
    Detail, Effect, InstalledAddon, Loadable, NextVideo, PlayRequest, Playing, State,
    SubtitleGroup, stop_torrent,
};
use super::torrent::{TorrentPlayback, TorrentRequest, TorrentStatus};
use crate::addon::{
    ContentType, ExtraValue, Meta, MetaPreview, Stream, StreamSource, Subtitle, TransportUrl, Video,
};

enum Source {
    Play(Url, Option<TorrentRequest>),
    NeedsConsent,
    Refused(Notice),
    Unusable,
}

fn resolve(settings: &Settings, stream: &Stream) -> Source {
    match &stream.source {
        StreamSource::Url(url) if stream.source.is_playable() => Source::Play(url.clone(), None),
        StreamSource::Url(url) => {
            Source::Refused(Notice::UnsupportedScheme(url.scheme().to_owned()))
        }
        StreamSource::Torrent { .. } if !settings.p2p_enabled => {
            Source::Refused(Notice::TorrentsOff)
        }
        StreamSource::Torrent { .. } if !settings.p2p_acknowledged => Source::NeedsConsent,
        StreamSource::Torrent { .. } => match TorrentRequest::from_stream(stream) {
            Some(request) => Source::Play(request.magnet(), Some(request)),
            None => Source::Unusable,
        },
        _ => Source::Refused(Notice::UnsupportedSource(stream.source.kind())),
    }
}

struct Start {
    content_type: ContentType,
    meta_id: String,
    meta: Option<Meta>,
    preview: Option<MetaPreview>,
    video_id: String,
    addon: TransportUrl,
    addon_name: String,
    stream: Stream,
}

pub(super) fn play(state: &mut State, group: usize, stream_index: usize) -> Vec<Effect> {
    let Some(detail) = state.detail.as_ref() else {
        return Vec::new();
    };
    let Some(video_id) = detail.selected_video.clone() else {
        return Vec::new();
    };
    let Some(source_group) = detail.streams.get(group) else {
        return Vec::new();
    };
    let Some(stream) = source_group
        .streams
        .ready()
        .and_then(|s| s.get(stream_index))
    else {
        return Vec::new();
    };
    let (url, torrent) = match resolve(&state.settings, stream) {
        Source::Play(url, torrent) => (url, torrent),
        Source::NeedsConsent => {
            state.p2p_prompt = Some((group, stream_index));
            return Vec::new();
        }
        Source::Refused(notice) => {
            state.notice = Some(notice);
            return Vec::new();
        }
        Source::Unusable => return Vec::new(),
    };
    let start = Start {
        content_type: detail.content_type.clone(),
        meta_id: detail.id.clone(),
        meta: detail.meta.ready().cloned(),
        preview: detail
            .meta
            .ready()
            .map(|m| m.preview.clone())
            .or_else(|| detail.preview.clone()),
        video_id,
        addon: source_group.addon.clone(),
        addon_name: source_group.addon_name.clone(),
        stream: stream.clone(),
    };
    start_stream(state, start, url, torrent)
}

fn episode_title(video: &Video) -> String {
    match (video.season, video.episode) {
        (Some(s), Some(e)) => format!("S{s:02}E{e:02} {}", video.title),
        _ => video.title.clone(),
    }
}

fn video_title(name: &str, meta: Option<&Meta>, video_id: &str) -> String {
    let episode = meta.and_then(|m| {
        m.videos
            .iter()
            .find(|v| v.id == video_id)
            .map(episode_title)
    });
    match episode {
        Some(ep) => format!("{name} — {ep}"),
        None => name.to_owned(),
    }
}

fn start_stream(
    state: &mut State,
    start: Start,
    url: Url,
    torrent: Option<TorrentRequest>,
) -> Vec<Effect> {
    let Start {
        content_type,
        meta_id,
        meta,
        preview,
        video_id,
        addon,
        addon_name,
        stream,
    } = start;
    let name = preview.as_ref().map(|p| p.name.clone()).unwrap_or_default();
    let title = video_title(&name, meta.as_ref(), &video_id);
    let subtitles = subtitle_groups(
        state,
        &content_type,
        (&addon, &addon_name),
        &stream,
        &video_id,
    );
    let headers = if torrent.is_some() {
        Vec::new()
    } else {
        stream.request_headers.clone()
    };
    let binge_group = stream.binge_group.clone();
    let saved = SavedStream {
        addon: addon.clone(),
        title: title.clone(),
        logo: preview.as_ref().and_then(|p| p.logo.clone()),
        background: preview.as_ref().and_then(|p| p.background.clone()),
        stream,
    };
    let request = PlayRequest {
        url,
        title,
        logo: saved.logo.clone(),
        background: saved.background.clone(),
        headers,
        settings: state.settings,
        start_ms: 0,
        meta_id: meta_id.clone(),
        video_id: video_id.clone(),
    };

    let start_ms = match state.library.iter_mut().find(|i| i.id == meta_id) {
        Some(item) => {
            let start = item.resume_ms(&video_id, state.settings.watched_at);
            if item.video_id != video_id {
                item.video_id.clone_from(&video_id);
                item.time_offset_ms = 0;
                item.duration_ms = 0;
            }
            start
        }
        None => {
            state.library.push(LibraryItem {
                id: meta_id.clone(),
                content_type: content_type.clone(),
                name,
                poster: preview.and_then(|p| p.poster),
                video_id: video_id.clone(),
                time_offset_ms: 0,
                duration_ms: 0,
                updated_ms: 0,
                favorited: None,
                stream: None,
            });
            0
        }
    };
    let mut effects = Vec::new();
    if let Some(item) = state.library.iter().find(|i| i.id == meta_id) {
        effects.push(Effect::SaveLibraryItem(item.clone()));
    }
    state.playing = Some(Playing {
        meta_id,
        resumed: false,
        stream: Some(Box::new(saved)),
        content_type,
        video_id,
        addon,
        binge_group,
        meta,
        meta_request: None,
        next: None,
        next_checked: false,
    });
    effects.extend(start_playback(
        state,
        PlayRequest {
            start_ms,
            ..request
        },
        torrent,
        subtitles,
    ));
    effects
}

pub(super) fn resume(state: &mut State, meta_id: &str) -> Vec<Effect> {
    let Some(item) = state.library.iter().find(|i| i.id == meta_id) else {
        return Vec::new();
    };
    let source = item
        .stream
        .as_ref()
        .map(|saved| resolve(&state.settings, &saved.stream));
    let (Some(saved), Some(Source::Play(url, torrent))) = (item.stream.as_deref().cloned(), source)
    else {
        return open_detail(state, item.content_type.clone(), item.id.clone(), None);
    };
    let video_id = item.video_id.clone();
    let source_name = addon_name(&state.addons, &saved.addon);
    let subtitles = subtitle_groups(
        state,
        &item.content_type,
        (&saved.addon, &source_name),
        &saved.stream,
        &video_id,
    );
    let meta_request = plan::meta_candidates(&state.addons, &item.content_type, &item.id)
        .into_iter()
        .next();
    let request = PlayRequest {
        url,
        title: saved.title,
        logo: saved.logo,
        background: saved.background,
        headers: if torrent.is_some() {
            Vec::new()
        } else {
            saved.stream.request_headers
        },
        settings: state.settings,
        start_ms: item.resume_ms(&video_id, state.settings.watched_at),
        meta_id: item.id.clone(),
        video_id: video_id.clone(),
    };
    state.playing = Some(Playing {
        meta_id: item.id.clone(),
        resumed: true,
        stream: None,
        content_type: item.content_type.clone(),
        video_id,
        addon: saved.addon,
        binge_group: saved.stream.binge_group,
        meta: None,
        meta_request: meta_request.clone(),
        next: None,
        next_checked: false,
    });
    let mut effects = start_playback(state, request, torrent, subtitles);
    effects.extend(meta_request.map(|(addon, path)| Effect::FetchMeta { addon, path }));
    effects
}

pub(super) fn check_next(state: &mut State, now_ms: u64) -> Vec<Effect> {
    let Some(playing) = state.playing.as_mut().filter(|p| !p.next_checked) else {
        return Vec::new();
    };
    let Some(meta) = &playing.meta else {
        return Vec::new();
    };
    playing.next_checked = true;
    let Some(video) = meta.next_video(&playing.video_id, now_ms).cloned() else {
        return Vec::new();
    };
    let path = playing.binge_group.as_ref().and_then(|_| {
        plan::stream_targets(&state.addons, &playing.content_type, &video.id)
            .into_iter()
            .find(|(addon, _)| *addon == playing.addon)
            .map(|(_, path)| path)
    });
    let effects = path
        .iter()
        .map(|path| fetch_streams(&playing.addon, path))
        .collect();
    playing.next = Some(NextVideo {
        title: episode_title(&video),
        image: video
            .thumbnail
            .clone()
            .or_else(|| meta.preview.background.clone()),
        video,
        path,
        stream: None,
    });
    effects
}

pub(super) fn play_next(state: &mut State) -> Vec<Effect> {
    let next = state.playing.as_ref().and_then(|p| {
        Some((
            p.content_type.clone(),
            p.meta_id.clone(),
            p.meta.clone()?,
            p.next.clone()?,
            p.addon.clone(),
        ))
    });
    let mut effects = playback_ended(state, None);
    let Some((content_type, meta_id, meta, next, addon)) = next else {
        return effects;
    };
    state.detail = Some(Detail {
        content_type: content_type.clone(),
        id: meta_id.clone(),
        preview: Some(meta.preview.clone()),
        meta: Loadable::Ready(meta.clone()),
        meta_fallbacks: Vec::new(),
        meta_request: None,
        selected_video: None,
        streams: Vec::new(),
    });
    effects.extend(select_video(state, next.video.id.clone()));
    let Some(stream) = next.stream else {
        return effects;
    };
    match resolve(&state.settings, &stream) {
        Source::Play(url, torrent) => {
            let start = Start {
                content_type,
                meta_id,
                preview: Some(meta.preview.clone()),
                meta: Some(meta),
                video_id: next.video.id,
                addon_name: addon_name(&state.addons, &addon),
                addon,
                stream,
            };
            effects.extend(start_stream(state, start, url, torrent));
        }
        Source::Refused(notice) => state.notice = Some(notice),
        Source::NeedsConsent | Source::Unusable => {}
    }
    effects
}

fn advance_to_next(state: &mut State, playing: &Playing) -> Option<Effect> {
    let next = playing.next.as_ref()?;
    let watched_at = state.settings.watched_at;
    let item = state.library.iter_mut().find(|i| {
        i.id == playing.meta_id
            && i.video_id == playing.video_id
            && i.was_played()
            && i.is_finished(watched_at)
    })?;
    let meta = playing.meta.as_ref();
    let preview = meta.map(|m| &m.preview);
    item.stream = next
        .stream
        .clone()
        .filter(|s| s.source.is_playable())
        .map(|stream| {
            Box::new(SavedStream {
                addon: playing.addon.clone(),
                title: video_title(&item.name, meta, &next.video.id),
                logo: preview.and_then(|p| p.logo.clone()),
                background: preview.and_then(|p| p.background.clone()),
                stream,
            })
        });
    item.video_id.clone_from(&next.video.id);
    item.time_offset_ms = 1;
    item.duration_ms = 0;
    Some(Effect::SaveLibraryItem(item.clone()))
}

pub(super) fn playback_ended(state: &mut State, failure: Option<PlaybackFailure>) -> Vec<Effect> {
    let mut effects = stop_torrent(state);
    let playing = state.playing.take();
    if let Some(playing) = &playing
        && let Some(index) = state
            .library
            .iter()
            .position(|i| i.id == playing.meta_id && !i.was_played() && !i.is_favorite())
    {
        state.library.remove(index);
        effects.push(Effect::DeleteLibraryItem(playing.meta_id.clone()));
    }
    let Some(failure) = failure else {
        effects.extend(
            playing
                .as_ref()
                .and_then(|playing| advance_to_next(state, playing)),
        );
        return effects;
    };
    let resumed = playing
        .filter(|p| p.resumed)
        .and_then(|p| state.library.iter().find(|i| i.id == p.meta_id))
        .map(|i| (i.content_type.clone(), i.id.clone()));
    let Some((content_type, id)) = resumed else {
        state.notice = Some(Notice::PlaybackFailed {
            failure,
            pick_another: false,
        });
        return effects;
    };
    state.notice = Some(Notice::PlaybackFailed {
        failure,
        pick_another: true,
    });
    effects.extend(open_detail(state, content_type, id, None));
    effects
}

fn start_playback(
    state: &mut State,
    request: PlayRequest,
    torrent: Option<TorrentRequest>,
    subtitles: Vec<SubtitleGroup>,
) -> Vec<Effect> {
    let mut effects = Vec::new();
    let fetch_subtitles: Vec<Effect> = subtitles
        .iter()
        .filter_map(|g| {
            g.path.clone().map(|path| Effect::FetchSubtitles {
                addon: g.addon.clone(),
                path,
            })
        })
        .collect();
    state.subtitles = subtitles;
    match torrent {
        None => {
            effects.extend(stop_torrent(state));
            effects.push(Effect::Play(Box::new(request)));
        }
        Some(torrent) => {
            state.torrent = Some(TorrentPlayback {
                info_hash: torrent.info_hash.clone(),
                status: TorrentStatus::Starting,
                pending: Some(request),
            });
            effects.push(Effect::StartTorrent(torrent));
        }
    }
    effects.extend(fetch_subtitles);
    effects
}

fn subtitle_groups(
    state: &State,
    content_type: &ContentType,
    (addon, addon_name): (&TransportUrl, &str),
    stream: &Stream,
    video_id: &str,
) -> Vec<SubtitleGroup> {
    let mut groups = Vec::new();
    if !stream.subtitles.is_empty() {
        let mut subtitles = stream.subtitles.clone();
        dedup_by_url(&mut subtitles);
        groups.push(SubtitleGroup {
            addon: addon.clone(),
            addon_name: addon_name.to_owned(),
            path: None,
            subtitles: Loadable::Ready(subtitles),
        });
    }
    let extra: Vec<ExtraValue> = [
        ("videoHash", stream.video_hash.clone()),
        ("videoSize", stream.video_size.map(|s| s.to_string())),
        ("filename", stream.filename.clone()),
    ]
    .into_iter()
    .filter_map(|(name, value)| value.map(|v| ExtraValue::new(name, v)))
    .collect();
    groups.extend(
        plan::subtitle_targets(&state.addons, content_type, video_id)
            .into_iter()
            .map(|(addon, path)| SubtitleGroup {
                addon_name: self::addon_name(&state.addons, &addon),
                addon,
                path: Some(path.with_extra(extra.clone())),
                subtitles: Loadable::Loading,
            }),
    );
    groups
}

pub(super) fn addon_name(addons: &[InstalledAddon], addon: &TransportUrl) -> String {
    addons
        .iter()
        .find(|a| a.transport == *addon)
        .map(|a| a.manifest.name.clone())
        .unwrap_or_default()
}

pub(super) fn dedup_by_url(subtitles: &mut Vec<Subtitle>) {
    let mut seen = HashSet::new();
    subtitles.retain(|s| seen.insert(s.url.clone()));
}
