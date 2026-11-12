use std::collections::HashSet;

use super::message::{Notice, Problem};
use super::plan::{self, CatalogTarget};
use super::playback::addon_name;
use super::state::{
    AddonList, Detail, Effect, InstalledAddon, Loadable, ManifestOrigin, Row, State, StreamGroup,
};
use crate::addon::{
    ContentType, ExtraValue, Manifest, Meta, MetaPreview, ResourcePath, TransportUrl,
};

pub(super) fn addons_changed(state: &mut State, save: bool) -> Vec<Effect> {
    let mut effects = Vec::new();
    if save {
        effects.push(Effect::SaveAddons(state.installed.clone()));
    }
    effects.extend(refresh_views(state, false));
    effects
}

fn refresh_views(state: &mut State, retry: bool) -> Vec<Effect> {
    let mut effects = refresh_board(state, retry);
    effects.extend(refresh_addon_lists(state, retry));
    effects.extend(refresh_search(state, retry));
    effects.extend(refresh_detail(state, retry));
    effects
}

pub(super) fn fetch_streams(addon: &TransportUrl, path: &ResourcePath) -> Effect {
    Effect::FetchStreams {
        addon: addon.clone(),
        path: path.clone(),
    }
}

fn fetch_catalog(target: &CatalogTarget) -> Effect {
    Effect::FetchCatalog {
        addon: target.addon.clone(),
        path: target.path.clone(),
    }
}

pub(super) fn loading_rows(targets: Vec<CatalogTarget>) -> (Vec<Row>, Vec<Effect>) {
    let effects = targets.iter().map(fetch_catalog).collect();
    let rows = targets
        .into_iter()
        .map(|target| Row {
            target,
            items: Loadable::Loading,
        })
        .collect();
    (rows, effects)
}

pub(super) fn load_board(state: &mut State) -> Vec<Effect> {
    let effects;
    (state.board, effects) = loading_rows(plan::board_targets(&state.addons));
    effects
}

pub(super) fn retry_failed(state: &mut State) -> Vec<Effect> {
    let addons: Vec<TransportUrl> = state.unavailable_addons().into_iter().cloned().collect();
    state.addons_loading.extend(addons.iter().cloned());
    let mut effects: Vec<Effect> = addons
        .into_iter()
        .map(|transport| Effect::FetchManifest {
            transport,
            origin: ManifestOrigin::Retry,
        })
        .collect();
    effects.extend(refresh_views(state, true));
    effects
}

fn request_meta(addons: &[InstalledAddon], detail: &mut Detail) -> Option<Effect> {
    if !detail.meta.is_failed() {
        return None;
    }
    detail.meta_fallbacks = plan::meta_candidates(addons, &detail.content_type, &detail.id);
    let effect = next_meta(detail)?;
    detail.meta = Loadable::Loading;
    Some(effect)
}

fn next_meta(detail: &mut Detail) -> Option<Effect> {
    if detail.meta_fallbacks.is_empty() {
        return None;
    }
    let (addon, path) = detail.meta_fallbacks.remove(0);
    detail.meta_request = Some((addon.clone(), path.clone()));
    Some(Effect::FetchMeta { addon, path })
}

fn refresh_detail(state: &mut State, retry: bool) -> Vec<Effect> {
    let addons = &state.addons;
    let Some(detail) = state.detail.as_mut() else {
        return Vec::new();
    };
    let mut effects: Vec<Effect> = request_meta(addons, detail).into_iter().collect();
    if let Some(video_id) = detail.selected_video.clone() {
        effects.extend(refresh_streams(addons, detail, &video_id, retry));
    }
    effects
}

fn refresh_streams(
    addons: &[InstalledAddon],
    detail: &mut Detail,
    video_id: &str,
    retry: bool,
) -> Vec<Effect> {
    let targets = plan::stream_targets(addons, &detail.content_type, video_id);
    let mut effects = Vec::new();
    detail.streams = reconcile(
        std::mem::take(&mut detail.streams),
        targets,
        |group, (addon, path)| {
            group.addon == *addon && group.path == *path && !(retry && group.streams.is_failed())
        },
        |(addon, path)| {
            effects.push(fetch_streams(&addon, &path));
            StreamGroup {
                addon_name: addon_name(addons, &addon),
                addon,
                path,
                streams: Loadable::Loading,
            }
        },
    );
    effects
}

fn refresh_search(state: &mut State, retry: bool) -> Vec<Effect> {
    if state.search_query.is_empty() {
        return Vec::new();
    }
    let targets = plan::search_targets(&state.addons, &state.search_query);
    kept_rows(&mut state.search, targets, retry)
}

fn refresh_board(state: &mut State, retry: bool) -> Vec<Effect> {
    kept_rows(&mut state.board, plan::board_targets(&state.addons), retry)
}

fn kept_rows(rows: &mut Vec<Row>, targets: Vec<CatalogTarget>, retry: bool) -> Vec<Effect> {
    let mut effects = Vec::new();
    *rows = reconcile(
        std::mem::take(rows),
        targets,
        |row, target| row.target == *target && !(retry && row.items.is_failed()),
        |target| {
            effects.push(fetch_catalog(&target));
            Row {
                target,
                items: Loadable::Loading,
            }
        },
    );
    effects
}

fn reconcile<T, K>(
    mut old: Vec<T>,
    targets: Vec<K>,
    same: impl Fn(&T, &K) -> bool,
    mut create: impl FnMut(K) -> T,
) -> Vec<T> {
    targets
        .into_iter()
        .map(
            |target| match old.iter().position(|item| same(item, &target)) {
                Some(index) => old.swap_remove(index),
                None => create(target),
            },
        )
        .collect()
}

pub(super) fn refresh_addon_lists(state: &mut State, retry: bool) -> Vec<Effect> {
    let Some(old) = state.addon_lists.take() else {
        return Vec::new();
    };
    let mut effects = Vec::new();
    let lists = reconcile(
        old,
        plan::addon_list_targets(&state.addons),
        |list, target| list.target == *target && !(retry && list.addons.is_failed()),
        |target| {
            effects.push(Effect::FetchAddonList {
                addon: target.addon.clone(),
                path: target.path.clone(),
            });
            AddonList {
                target,
                addons: Loadable::Loading,
            }
        },
    );
    state.addon_lists = Some(lists);
    effects
}

pub(super) fn default_genre(
    state: &State,
    addon: &TransportUrl,
    path: &ResourcePath,
) -> Option<String> {
    let catalog = state
        .addons
        .iter()
        .find(|a| &a.transport == addon)?
        .manifest
        .catalog(&path.content_type, &path.id)?;
    let genre = catalog.extra.iter().find(|e| e.name == "genre")?;
    if genre.is_required {
        genre.options.first().cloned()
    } else {
        None
    }
}

pub(super) fn discover_page(state: &mut State, skip: usize) -> Vec<Effect> {
    let Some(target) = state.discover.target.clone() else {
        return Vec::new();
    };
    let mut extra = Vec::new();
    if let Some(genre) = &state.discover.genre {
        extra.push(ExtraValue::new("genre", genre.clone()));
    }
    if skip > 0 {
        extra.push(ExtraValue::new("skip", skip.to_string()));
    }
    let path = target.path.clone().with_extra(extra);
    let supported = state
        .addons
        .iter()
        .find(|a| a.transport == target.addon)
        .is_some_and(|a| a.manifest.supports(&path));
    if !supported {
        state.discover.error = Some(Problem::UnsupportedFilter);
        return Vec::new();
    }
    state.discover.error = None;
    state.discover.pending = Some(path.clone());
    vec![Effect::FetchCatalog {
        addon: target.addon,
        path,
    }]
}

pub(super) fn manifest_loaded(
    state: &mut State,
    transport: &TransportUrl,
    result: Result<Box<Manifest>, String>,
    origin: ManifestOrigin,
) -> Vec<Effect> {
    let install = origin == ManifestOrigin::Install;
    let was_loading = state.addons_loading.contains(transport);
    state.addons_loading.retain(|t| t != transport);
    if !install && !was_loading {
        return Vec::new();
    }
    match result {
        Ok(manifest) if install && manifest.behavior_hints.configuration_required => {
            state.install = Some(Loadable::Failed(Problem::ConfigurationRequired(
                transport.clone(),
            )));
            Vec::new()
        }
        Ok(manifest) => {
            if install {
                state.install = Some(Loadable::Ready(manifest.name.clone()));
            }
            let addon = InstalledAddon {
                transport: transport.clone(),
                manifest: *manifest,
            };
            if let Some(existing) = state.addons.iter_mut().find(|a| &a.transport == transport) {
                *existing = addon;
            } else {
                if !state.installed.contains(transport) {
                    state.installed.push(transport.clone());
                }
                let rank = |t: &TransportUrl| state.installed.iter().position(|i| i == t);
                let at = state
                    .addons
                    .iter()
                    .take_while(|a| rank(&a.transport) < rank(transport))
                    .count();
                state.addons.insert(at, addon);
            }
            addons_changed(state, install)
        }
        Err(err) => {
            match origin {
                ManifestOrigin::Install => {
                    state.install = Some(Loadable::Failed(Problem::Request(err)));
                }
                ManifestOrigin::Restore => {
                    state.notice = Some(Notice::AddonUnavailable(err));
                }
                ManifestOrigin::Retry => {}
            }
            Vec::new()
        }
    }
}

pub(super) fn catalog_loaded(
    state: &mut State,
    addon: &TransportUrl,
    path: &ResourcePath,
    result: Result<Vec<MetaPreview>, String>,
) {
    for row in state.board.iter_mut().chain(state.search.iter_mut()) {
        if &row.target.addon == addon && &row.target.path == path {
            row.items = result.clone().into();
        }
    }
    let discover = &mut state.discover;
    let is_current = discover.pending.as_ref() == Some(path)
        && discover.target.as_ref().is_some_and(|t| &t.addon == addon);
    if is_current {
        discover.pending = None;
        match result {
            Ok(items) => {
                let skip = path
                    .extra
                    .iter()
                    .find(|e| e.name == "skip")
                    .and_then(|e| e.value.parse::<usize>().ok())
                    .unwrap_or(0);
                let pages = state
                    .addons
                    .iter()
                    .find(|a| &a.transport == addon)
                    .and_then(|a| a.manifest.catalog(&path.content_type, &path.id))
                    .is_some_and(|c| c.extra.iter().any(|e| e.name == "skip"));
                discover.next_skip = (pages && !items.is_empty()).then(|| skip + items.len());
                if skip == 0 {
                    discover.items = items;
                } else {
                    let mut seen: HashSet<String> =
                        discover.items.iter().map(|i| i.id.clone()).collect();
                    discover
                        .items
                        .extend(items.into_iter().filter(|i| seen.insert(i.id.clone())));
                }
            }
            Err(err) => discover.error = Some(Problem::Request(err)),
        }
    }
}

pub(super) fn meta_loaded(
    state: &mut State,
    addon: &TransportUrl,
    path: &ResourcePath,
    result: Result<Box<Meta>, String>,
) -> Vec<Effect> {
    let request = Some((addon.clone(), path.clone()));
    if let Some(playing) = state.playing.as_mut().filter(|p| p.meta_request == request) {
        playing.meta_request = None;
        playing.meta = result.as_ref().ok().map(|m| (**m).clone());
    }
    let Some(detail) = state.detail.as_mut() else {
        return Vec::new();
    };
    if detail.meta_request != request {
        return Vec::new();
    }
    match result {
        Ok(meta) => {
            let auto_video = if meta.videos.is_empty() {
                Some(meta.preview.id.clone())
            } else {
                meta.default_video_id
                    .clone()
                    .filter(|id| meta.videos.iter().any(|v| &v.id == id))
            };
            detail.meta = Loadable::Ready(*meta);
            detail.meta_request = None;
            match auto_video {
                Some(video_id) => select_video(state, video_id),
                None => Vec::new(),
            }
        }
        Err(err) => match next_meta(detail) {
            Some(effect) => vec![effect],
            None => {
                detail.meta = Loadable::Failed(Problem::Request(err));
                detail.meta_request = None;
                Vec::new()
            }
        },
    }
}

pub(super) fn select_video(state: &mut State, video_id: String) -> Vec<Effect> {
    let addons = &state.addons;
    let Some(detail) = state.detail.as_mut() else {
        return Vec::new();
    };
    detail.streams.clear();
    let effects = refresh_streams(addons, detail, &video_id, false);
    detail.selected_video = Some(video_id);
    effects
}

pub(super) fn open_detail(
    state: &mut State,
    content_type: ContentType,
    id: String,
    preview: Option<MetaPreview>,
) -> Vec<Effect> {
    let detail = state.detail.insert(Detail {
        meta: Loadable::Failed(Problem::NoMetaAddon),
        content_type,
        id,
        preview,
        meta_fallbacks: Vec::new(),
        meta_request: None,
        selected_video: None,
        streams: Vec::new(),
    });
    request_meta(&state.addons, detail).into_iter().collect()
}
