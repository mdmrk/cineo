use std::future::Future;

use cineo_core::addon::{ResourcePath, TransportUrl};
use cineo_core::app::{Action, Effect, ManifestOrigin};
use cineo_core::diagnostics::Parsed;
use tracing::debug;
use url::Url;

use crate::{AddonClient, FetchError};

enum Fetch {
    Manifest {
        transport: TransportUrl,
        origin: ManifestOrigin,
    },
    Catalog(TransportUrl, ResourcePath),
    AddonList(TransportUrl, ResourcePath),
    Meta(TransportUrl, ResourcePath),
    Streams(TransportUrl, ResourcePath),
    Subtitles(TransportUrl, ResourcePath),
    LatestRelease(Url),
}

pub fn fetch_task(
    client: &AddonClient,
    effect: Effect,
) -> Option<impl Future<Output = Action> + Send + 'static> {
    let fetch = match effect {
        Effect::FetchManifest { transport, origin } => Fetch::Manifest { transport, origin },
        Effect::FetchCatalog { addon, path } => Fetch::Catalog(addon, path),
        Effect::FetchAddonList { addon, path } => Fetch::AddonList(addon, path),
        Effect::FetchMeta { addon, path } => Fetch::Meta(addon, path),
        Effect::FetchStreams { addon, path } => Fetch::Streams(addon, path),
        Effect::FetchSubtitles { addon, path } => Fetch::Subtitles(addon, path),
        Effect::CheckForUpdate(url) => Fetch::LatestRelease(url),
        Effect::SaveAddons(_)
        | Effect::SaveLibraryItem(_)
        | Effect::DeleteLibraryItem(_)
        | Effect::ClearLibrary
        | Effect::SaveSettings(_)
        | Effect::Play(_)
        | Effect::StartTorrent(_)
        | Effect::StopTorrent => return None,
    };
    let client = client.clone();
    Some(async move { run(&client, fetch).await })
}

async fn run(client: &AddonClient, fetch: Fetch) -> Action {
    match fetch {
        Fetch::Manifest { transport, origin } => {
            let result = fetched(client.fetch_manifest(&transport).await).map(Box::new);
            Action::ManifestLoaded {
                transport,
                result,
                origin,
            }
        }
        Fetch::Catalog(addon, path) => {
            let result = fetched(client.fetch_catalog(&addon, &path).await).map(|c| c.metas);
            Action::CatalogLoaded {
                addon,
                path,
                result,
            }
        }
        Fetch::AddonList(addon, path) => {
            let result = fetched(client.fetch_addon_list(&addon, &path).await);
            Action::AddonListLoaded {
                addon,
                path,
                result,
            }
        }
        Fetch::Meta(addon, path) => {
            let result = fetched(client.fetch_meta(&addon, &path).await).map(Box::new);
            Action::MetaLoaded {
                addon,
                path,
                result,
            }
        }
        Fetch::Streams(addon, path) => {
            let result = fetched(client.fetch_streams(&addon, &path).await);
            Action::StreamsLoaded {
                addon,
                path,
                result,
            }
        }
        Fetch::Subtitles(addon, path) => {
            let result = fetched(client.fetch_subtitles(&addon, &path).await);
            Action::SubtitlesLoaded {
                addon,
                path,
                result,
            }
        }
        Fetch::LatestRelease(url) => {
            let result = client.fetch_latest_release(&url).await.map_err(|err| {
                debug!(%err, "update check failed");
                err.to_string()
            });
            Action::LatestReleaseLoaded(result)
        }
    }
}

fn fetched<T>(result: Result<Parsed<T>, FetchError>) -> Result<T, String> {
    result
        .map(|parsed| parsed.value)
        .map_err(|err| err.to_string())
}
