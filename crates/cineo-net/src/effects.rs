use std::future::Future;

use cineo_core::addon::{ResourcePath, TransportUrl};
use cineo_core::app::{Action, Effect, ManifestOrigin, PulledPage, PushedBatch, SyncFailure};
use cineo_core::diagnostics::Parsed;
use cineo_core::sync::{Outgoing, Pairing};
use tracing::debug;
use url::Url;

use crate::sync::{SyncClient, SyncError};
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
    SyncPull(Pairing, u64),
    SyncPush(Pairing, Vec<Outgoing>),
    DeleteSyncSpace(Pairing),
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
        Effect::SyncPull { pairing, since } => Fetch::SyncPull(pairing, since),
        Effect::SyncPush { pairing, outgoing } => Fetch::SyncPush(pairing, outgoing),
        Effect::DeleteSyncSpace(pairing) => Fetch::DeleteSyncSpace(pairing),
        Effect::SavePairing { .. }
        | Effect::SaveSync(_)
        | Effect::ForgetSync
        | Effect::SaveRemoteHost(_)
        | Effect::ServeRemote(_)
        | Effect::SaveRemotePeer(_)
        | Effect::SaveRemoteChoice(_)
        | Effect::SaveAddons(_)
        | Effect::SaveLibraryItem(_)
        | Effect::DeleteLibraryItem(_)
        | Effect::ClearLibrary
        | Effect::SaveSettings(_)
        | Effect::Play(_)
        | Effect::StartTorrent(_)
        | Effect::StopTorrent
        | Effect::InstallUpdate(_)
        | Effect::RestartToUpdate => return None,
    };
    let client = client.clone();
    Some(async move { run(&client, fetch).await })
}

fn failure(err: SyncError) -> SyncFailure {
    match err {
        SyncError::InsecureServer => SyncFailure::Insecure,
        SyncError::Fetch(err) => SyncFailure::Network(err.to_string()),
        SyncError::Conflict(server) => SyncFailure::Conflict(server),
        SyncError::SpaceGone => SyncFailure::SpaceGone,
        SyncError::Unavailable { retry_after_s } => SyncFailure::Unavailable { retry_after_s },
        SyncError::Rejected(status) => SyncFailure::Rejected(status.as_u16()),
        SyncError::Malformed | SyncError::Crypto => SyncFailure::Malformed,
    }
}

async fn sync_pull(
    client: &AddonClient,
    pairing: &Pairing,
    since: u64,
) -> Result<PulledPage, SyncFailure> {
    let sync = SyncClient::new(client.clone(), pairing).map_err(failure)?;
    let pulled = sync.pull(since).await.map_err(failure)?;
    Ok(PulledPage {
        records: pulled.records,
        more: pulled.more,
        space_seq: pulled.space_seq,
        born: pulled.born,
    })
}

async fn sync_push(client: &AddonClient, pairing: &Pairing, outgoing: &[Outgoing]) -> PushedBatch {
    let sync = match SyncClient::new(client.clone(), pairing) {
        Ok(sync) => sync,
        Err(err) => {
            return PushedBatch {
                failure: Some(failure(err)),
                ..PushedBatch::default()
            };
        }
    };
    let outcome = sync.push(outgoing).await;
    PushedBatch {
        pushed: outcome
            .pushed
            .into_iter()
            .map(|p| (p.name, p.stamp, p.seq))
            .collect(),
        oversized: outcome
            .oversized
            .into_iter()
            .filter_map(|name| {
                let out = outgoing.iter().find(|o| o.record.name == name)?;
                Some((name, out.record.stamp))
            })
            .collect(),
        failure: outcome.error.map(failure),
    }
}

async fn run(client: &AddonClient, fetch: Fetch) -> Action {
    match fetch {
        Fetch::SyncPull(pairing, since) => {
            let result = sync_pull(client, &pairing, since).await;
            Action::SyncPulled { pairing, result }
        }
        Fetch::SyncPush(pairing, outgoing) => {
            let batch = sync_push(client, &pairing, &outgoing).await;
            Action::SyncPushed { pairing, batch }
        }
        Fetch::DeleteSyncSpace(pairing) => {
            let result = match SyncClient::new(client.clone(), &pairing) {
                Ok(sync) => sync.delete_space().await.map_err(failure),
                Err(err) => Err(failure(err)),
            };
            Action::SyncSpaceDeleted(result)
        }
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
