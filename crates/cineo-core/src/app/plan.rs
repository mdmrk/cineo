use super::state::InstalledAddon;
use crate::addon::{CatalogDef, ContentType, ExtraValue, ResourceName, ResourcePath, TransportUrl};

/// A catalog request with a display title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogTarget {
    pub addon: TransportUrl,
    pub addon_name: String,
    pub path: ResourcePath,
    pub title: String,
}

/// Board rows: every catalog that needs no user input (no required extra).
pub fn board_targets(addons: &[InstalledAddon]) -> Vec<CatalogTarget> {
    addons
        .iter()
        .flat_map(|addon| {
            addon
                .manifest
                .catalogs
                .iter()
                .filter(|c| c.is_browsable())
                .map(move |c| catalog_target(addon, c, title(c)))
        })
        .collect()
}

/// Search rows: catalogs declaring `search` whose other required extras are
/// none (so `search` alone is a valid request).
pub fn search_targets(addons: &[InstalledAddon], query: &str) -> Vec<CatalogTarget> {
    addons
        .iter()
        .flat_map(|addon| {
            addon
                .manifest
                .catalogs
                .iter()
                .filter(|c| {
                    c.extra.iter().any(|e| e.name == "search")
                        && c.extra.iter().all(|e| !e.is_required || e.name == "search")
                })
                .map(move |c| {
                    let mut target = catalog_target(addon, c, title(c));
                    target.path.extra = vec![ExtraValue::new("search", query)];
                    target
                })
        })
        .collect()
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

pub(super) fn catalog_target(
    addon: &InstalledAddon,
    catalog: &CatalogDef,
    title: String,
) -> CatalogTarget {
    CatalogTarget {
        addon: addon.transport.clone(),
        addon_name: addon.manifest.name.clone(),
        path: ResourcePath::catalog(catalog.content_type.clone(), catalog.id.clone()),
        title,
    }
}

fn title(catalog: &CatalogDef) -> String {
    let type_label = match catalog.content_type.as_str() {
        ContentType::MOVIE => "Movies",
        ContentType::SERIES => "Series",
        ContentType::CHANNEL => "Channels",
        ContentType::TV => "TV",
        other => other,
    };
    format!(
        "{} {type_label}",
        catalog.name.as_deref().unwrap_or(&catalog.id)
    )
}
