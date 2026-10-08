use percent_encoding::percent_decode_str;
use url::Url;

use crate::addon::{ContentType, ResourcePath, TransportUrl};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    InstallAddon(TransportUrl),
    Board,
    Discover(Option<DiscoverLink>),
    Library,
    Search(String),
    Detail {
        content_type: ContentType,
        id: String,
        video_id: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoverLink {
    pub addon: TransportUrl,
    pub path: ResourcePath,
    pub genre: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum LinkError {
    #[error("not a stremio:// or cineo:// link")]
    Scheme,
    #[error("not a known link")]
    Unknown,
}

const SCHEMES: [&str; 2] = ["stremio://", "cineo://"];

pub fn parse_link(link: &str) -> Result<Route, LinkError> {
    let link = link.trim();
    let rest = SCHEMES
        .iter()
        .find_map(|scheme| {
            link.get(..scheme.len())
                .filter(|head| head.eq_ignore_ascii_case(scheme))
                .map(|_| &link[scheme.len()..])
        })
        .ok_or(LinkError::Scheme)?;
    if !rest.starts_with('/') {
        return TransportUrl::parse(&format!("https://{rest}"))
            .map(Route::InstallAddon)
            .map_err(|_| LinkError::Unknown);
    }
    let url = Url::parse(&format!("https://link.invalid{rest}")).map_err(|_| LinkError::Unknown)?;
    let query = |name: &str| {
        url.query_pairs()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.into_owned())
    };
    let segments: Vec<String> = url
        .path_segments()
        .into_iter()
        .flatten()
        .filter(|s| !s.is_empty())
        .map(|s| percent_decode_str(s).decode_utf8_lossy().into_owned())
        .collect();
    let segments: Vec<&str> = segments.iter().map(String::as_str).collect();
    let content_type = |s: &str| ContentType::new(s).ok_or(LinkError::Unknown);
    match segments.as_slice() {
        [] | ["board"] => Ok(Route::Board),
        ["library"] => Ok(Route::Library),
        ["search"] => Ok(Route::Search(
            query("search").unwrap_or_default().trim().to_owned(),
        )),
        ["discover"] => Ok(Route::Discover(None)),
        ["discover", addon, ty, catalog] => Ok(Route::Discover(Some(DiscoverLink {
            addon: TransportUrl::parse(addon).map_err(|_| LinkError::Unknown)?,
            path: ResourcePath::catalog(content_type(ty)?, *catalog),
            genre: query("genre").filter(|g| !g.is_empty()),
        }))),
        ["detail", ty, id, video @ ..] if video.len() <= 1 => Ok(Route::Detail {
            content_type: content_type(ty)?,
            id: (*id).to_owned(),
            video_id: video.first().map(|v| (*v).to_owned()),
        }),
        _ => Err(LinkError::Unknown),
    }
}
