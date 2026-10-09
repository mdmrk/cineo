use super::state::InstalledAddon;
use crate::addon::{CatalogDef, ContentType, ExtraValue, ResourceName, ResourcePath, TransportUrl};

/// A catalog request; `name` falls back to the catalog id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogTarget {
    pub addon: TransportUrl,
    pub addon_name: String,
    pub path: ResourcePath,
    pub name: String,
}

/// Board rows: every catalog that needs no user input (no required extra).
pub fn board_targets(addons: &[InstalledAddon]) -> Vec<CatalogTarget> {
    catalogs(addons, CatalogDef::is_browsable)
        .map(|(addon, catalog)| catalog_target(addon, catalog))
        .collect()
}

/// Search rows: catalogs declaring `search` whose other required extras are
/// none (so `search` alone is a valid request).
pub fn search_targets(addons: &[InstalledAddon], query: &str) -> Vec<CatalogTarget> {
    catalogs(addons, |c| {
        c.extra.iter().any(|e| e.name == "search")
            && c.extra.iter().all(|e| !e.is_required || e.name == "search")
    })
    .map(|(addon, catalog)| {
        let mut target = catalog_target(addon, catalog);
        target.path.extra = vec![ExtraValue::new("search", query)];
        target
    })
    .collect()
}

pub(super) fn catalogs(
    addons: &[InstalledAddon],
    keep: impl Fn(&CatalogDef) -> bool + Copy,
) -> impl Iterator<Item = (&InstalledAddon, &CatalogDef)> {
    addons.iter().flat_map(move |addon| {
        addon
            .manifest
            .catalogs
            .iter()
            .filter(move |c| keep(c))
            .map(move |c| (addon, c))
    })
}

/// Addons that serve `meta` for this item, in user order. The first that
/// answers successfully wins.
pub fn meta_candidates(
    addons: &[InstalledAddon],
    content_type: &ContentType,
    id: &str,
) -> Vec<(TransportUrl, ResourcePath)> {
    targets(addons, ResourceName::Meta, content_type, id)
}

/// Addons that serve `stream` for this video, in user order. All are asked.
pub fn stream_targets(
    addons: &[InstalledAddon],
    content_type: &ContentType,
    video_id: &str,
) -> Vec<(TransportUrl, ResourcePath)> {
    targets(addons, ResourceName::Stream, content_type, video_id)
}

/// Addons that serve `subtitles` for this video, in user order. All are
/// asked.
pub fn subtitle_targets(
    addons: &[InstalledAddon],
    content_type: &ContentType,
    video_id: &str,
) -> Vec<(TransportUrl, ResourcePath)> {
    targets(addons, ResourceName::Subtitles, content_type, video_id)
}

fn targets(
    addons: &[InstalledAddon],
    resource: ResourceName,
    content_type: &ContentType,
    id: &str,
) -> Vec<(TransportUrl, ResourcePath)> {
    let path = ResourcePath {
        resource,
        content_type: content_type.clone(),
        id: id.to_owned(),
        extra: Vec::new(),
    };
    addons
        .iter()
        .filter(|a| a.manifest.supports(&path))
        .map(|a| (a.transport.clone(), path.clone()))
        .collect()
}

pub(super) fn catalog_target(addon: &InstalledAddon, catalog: &CatalogDef) -> CatalogTarget {
    CatalogTarget {
        addon: addon.transport.clone(),
        addon_name: addon.manifest.name.clone(),
        path: ResourcePath::catalog(catalog.content_type.clone(), catalog.id.clone()),
        name: catalog.name.clone().unwrap_or_else(|| catalog.id.clone()),
    }
}
