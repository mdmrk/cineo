#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers panic on purpose"
)]

use std::sync::Arc;

use cineo_core::addon::ContentType;
use cineo_core::app::{Effect, LibraryItem, SeekStep, Setting, Settings};
use cineo_core::sync::{DeviceId, Pairing, Replica, SyncKey};
use cineo_net::sync::{SyncClient, SyncError};
use cineo_net::{AddonClient, NetPolicy};
use cineo_sync_server::{ServeOptions, SqliteDb, serve, system_ms};
use tokio::net::TcpListener;

const KEY: [u8; 16] = [3; 16];

async fn start(db: SqliteDb) -> String {
    start_with(db, ServeOptions::default()).await
}

async fn start_with(db: SqliteDb, options: ServeOptions) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(serve(listener, Arc::new(db), options));
    format!("http://{address}/")
}

fn an_hour_later() -> u64 {
    system_ms() + 3_600_000
}

async fn start_later(db: SqliteDb) -> String {
    start_with(
        db,
        ServeOptions {
            clock: an_hour_later,
            ..ServeOptions::default()
        },
    )
    .await
}

fn client(server: &str, key: [u8; 16]) -> SyncClient {
    let http = AddonClient::new(NetPolicy {
        allow_private_networks: true,
        ..NetPolicy::default()
    })
    .unwrap();
    SyncClient::new(
        http,
        &Pairing {
            key: SyncKey::from_bytes(key),
            server: server.parse().unwrap(),
        },
    )
    .unwrap()
}

fn item(id: &str, name: &str, offset_ms: u64) -> LibraryItem {
    LibraryItem {
        id: id.to_owned(),
        content_type: ContentType::new("movie").unwrap(),
        name: name.to_owned(),
        poster: None,
        video_id: id.to_owned(),
        time_offset_ms: offset_ms,
        duration_ms: 6_000_000,
        updated_ms: 1,
        favorited: None,
        stream: None,
    }
}

async fn push(replica: &mut Replica, client: &SyncClient) -> Result<(), SyncError> {
    let outcome = client.push(&replica.outgoing()).await;
    for pushed in &outcome.pushed {
        replica.pushed(&pushed.name, pushed.stamp, pushed.seq);
    }
    outcome.error.map_or(Ok(()), Err)
}

async fn pull(replica: &mut Replica, client: &SyncClient, now_ms: u64) -> Result<(), SyncError> {
    loop {
        let pulled = client.pull(replica.since()).await?;
        for (record, seq) in pulled.records {
            replica.merge(record, seq, now_ms);
        }
        if !pulled.more {
            return Ok(());
        }
    }
}

fn ids(replica: &Replica) -> Vec<(String, u64)> {
    replica
        .library()
        .into_iter()
        .map(|i| (i.id, i.time_offset_ms))
        .collect()
}

#[tokio::test]
async fn two_devices_sync_through_the_server() {
    let server = start(SqliteDb::open_in_memory().unwrap()).await;
    let (phone, desktop) = (client(&server, KEY), client(&server, KEY));
    let mut a = Replica::new(DeviceId(1), Settings::default());
    let mut b = Replica::new(DeviceId(2), Settings::default());
    a.record_effect(&Effect::SaveLibraryItem(item("tt1", "One", 10)), 1_000);
    let mut settings = Settings::default();
    settings.set(Setting::SeekStep(SeekStep::S30));
    a.record_effect(&Effect::SaveSettings(settings), 1_000);
    push(&mut a, &phone).await.unwrap();
    pull(&mut b, &desktop, 2_000).await.unwrap();
    assert_eq!(ids(&b), [("tt1".to_owned(), 10)]);
    assert_eq!(
        b.settings_over(Settings::default()).seek_step,
        SeekStep::S30
    );
    b.record_effect(&Effect::DeleteLibraryItem("tt1".into()), 3_000);
    push(&mut b, &desktop).await.unwrap();
    pull(&mut a, &phone, 4_000).await.unwrap();
    assert!(a.library().is_empty());
}

#[tokio::test]
async fn a_conflict_is_resolved_by_pulling_and_merging() {
    let server = start(SqliteDb::open_in_memory().unwrap()).await;
    let (phone, desktop) = (client(&server, KEY), client(&server, KEY));
    let mut a = Replica::new(DeviceId(1), Settings::default());
    let mut b = Replica::new(DeviceId(2), Settings::default());
    a.record_effect(&Effect::SaveLibraryItem(item("tt1", "One", 10)), 1_000);
    b.record_effect(&Effect::SaveLibraryItem(item("tt1", "One", 20)), 2_000);
    push(&mut a, &phone).await.unwrap();
    let err = push(&mut b, &desktop).await.unwrap_err();
    assert!(matches!(err, SyncError::Conflict(_)), "{err:?}");
    pull(&mut b, &desktop, 2_000).await.unwrap();
    push(&mut b, &desktop).await.unwrap();
    pull(&mut a, &phone, 3_000).await.unwrap();
    assert_eq!(ids(&a), [("tt1".to_owned(), 20)]);
    assert_eq!(ids(&a), ids(&b));
}

#[tokio::test]
async fn a_rotated_space_cannot_be_used_with_the_old_code() {
    let server = start(SqliteDb::open_in_memory().unwrap()).await;
    let mut a = Replica::new(DeviceId(1), Settings::default());
    a.record_effect(&Effect::SaveLibraryItem(item("tt1", "One", 10)), 1_000);
    let old = client(&server, KEY);
    push(&mut a, &old).await.unwrap();
    old.delete_space().await.unwrap();
    let stale = client(&server, KEY);
    assert!(matches!(stale.pull(0).await, Err(SyncError::SpaceGone)));
    let mut b = Replica::new(DeviceId(2), Settings::default());
    b.record_effect(&Effect::SaveLibraryItem(item("tt2", "Two", 1)), 1_000);
    assert!(matches!(
        push(&mut b, &stale).await,
        Err(SyncError::SpaceGone)
    ));
}

#[tokio::test]
async fn another_key_sees_nothing() {
    let server = start(SqliteDb::open_in_memory().unwrap()).await;
    let mut a = Replica::new(DeviceId(1), Settings::default());
    a.record_effect(&Effect::SaveLibraryItem(item("tt1", "One", 10)), 1_000);
    push(&mut a, &client(&server, KEY)).await.unwrap();
    let pulled = client(&server, [4; 16]).pull(0).await.unwrap();
    assert!(pulled.records.is_empty() && pulled.unreadable == 0);
}

#[tokio::test]
async fn a_large_library_pages_through() {
    let server = start(SqliteDb::open_in_memory().unwrap()).await;
    let mut a = Replica::new(DeviceId(1), Settings::default());
    for i in 0..1_200 {
        a.record_effect(
            &Effect::SaveLibraryItem(item(&format!("tt{i:07}"), &"n".repeat(500), i)),
            1_000 + i,
        );
    }
    push(&mut a, &client(&server, KEY)).await.unwrap();
    assert!(a.outgoing().is_empty());
    let mut b = Replica::new(DeviceId(2), Settings::default());
    pull(&mut b, &client(&server, KEY), 10_000).await.unwrap();
    assert_eq!(b.library().len(), 1_200);
}

#[tokio::test]
async fn the_server_stores_nothing_readable() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sync.db");
    let server = start(SqliteDb::open(&path).unwrap()).await;
    let mut a = Replica::new(DeviceId(1), Settings::default());
    a.record_effect(
        &Effect::SaveLibraryItem(item("tt0123456", "A Very Secret Film", 10)),
        1_000,
    );
    let mut settings = Settings::default();
    settings.set(Setting::SeekStep(SeekStep::S30));
    a.record_effect(&Effect::SaveSettings(settings), 1_000);
    push(&mut a, &client(&server, KEY)).await.unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    let mut stored = Vec::new();
    let mut stmt = conn
        .prepare("SELECT space, key, blob FROM records UNION ALL SELECT id, id, id FROM spaces")
        .unwrap();
    let mut rows = stmt.query([]).unwrap();
    while let Some(row) = rows.next().unwrap() {
        for column in 0..3 {
            stored.extend(row.get::<_, Vec<u8>>(column).unwrap());
        }
    }
    let text = String::from_utf8_lossy(&stored);
    assert!(stored.len() > 100, "the push reached the database");
    for secret in ["tt0123456", "Secret Film", "seek_step", "library/"] {
        assert!(!text.contains(secret), "{secret} is stored in clear");
    }
}

async fn raw(server: &str, request: &str) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let address = server.trim_start_matches("http://").trim_end_matches('/');
    let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    response
}

#[tokio::test]
async fn an_oversized_request_is_refused_before_it_is_read() {
    let server = start(SqliteDb::open_in_memory().unwrap()).await;
    let response = raw(
        &server,
        "POST /v1/records HTTP/1.1\r\nHost: x\r\nContent-Length: 300000\r\nConnection: close\r\n\r\n",
    )
    .await;
    assert!(response.starts_with("HTTP/1.1 413"), "{response}");
}

#[tokio::test]
async fn too_many_requests_from_one_address_are_refused() {
    let server = start(SqliteDb::open_in_memory().unwrap()).await;
    let info = "GET /v1/info HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n";
    for _ in 0..cineo_sync_server::REQUESTS_PER_MINUTE {
        assert!(raw(&server, info).await.starts_with("HTTP/1.1 200"));
    }
    let refused = raw(&server, info).await;
    assert!(refused.starts_with("HTTP/1.1 429"), "{refused}");
    assert!(refused.to_lowercase().contains("retry-after:"), "{refused}");
}

#[tokio::test]
async fn behind_a_proxy_the_forwarded_address_is_limited() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(serve(
        listener,
        Arc::new(SqliteDb::open_in_memory().unwrap()),
        ServeOptions {
            trust_forwarded_for: true,
            ..ServeOptions::default()
        },
    ));
    let server = format!("http://{address}/");
    let from = |ip: &str| {
        format!(
            "GET /v1/info HTTP/1.1\r\nHost: x\r\nX-Forwarded-For: 198.51.100.9, {ip}\r\nConnection: close\r\n\r\n"
        )
    };
    for _ in 0..cineo_sync_server::REQUESTS_PER_MINUTE {
        raw(&server, &from("192.0.2.1")).await;
    }
    assert!(
        raw(&server, &from("192.0.2.1"))
            .await
            .starts_with("HTTP/1.1 429")
    );
    assert!(
        raw(&server, &from("192.0.2.2"))
            .await
            .starts_with("HTTP/1.1 200")
    );
}

fn snapshot(path: &std::path::Path, to: &std::path::Path) {
    rusqlite::Connection::open(path)
        .unwrap()
        .execute("VACUUM INTO ?1", [to.to_str().unwrap()])
        .unwrap();
}

#[tokio::test]
async fn devices_recover_when_the_server_restores_an_older_backup() {
    let dir = tempfile::tempdir().unwrap();
    let (live, backup) = (dir.path().join("sync.db"), dir.path().join("backup.db"));
    let first = start(SqliteDb::open(&live).unwrap()).await;
    let mut a = Replica::new(DeviceId(1), Settings::default());
    let mut b = Replica::new(DeviceId(2), Settings::default());
    a.record_effect(&Effect::SaveLibraryItem(item("tt1", "One", 1)), 1_000);
    client(&first, KEY).sync(&mut a, 1_000).await.unwrap();
    client(&first, KEY).sync(&mut b, 1_000).await.unwrap();
    snapshot(&live, &backup);
    a.record_effect(&Effect::SaveLibraryItem(item("tt2", "Two", 2)), 2_000);
    a.record_effect(&Effect::SaveLibraryItem(item("tt3", "Three", 3)), 2_000);
    client(&first, KEY).sync(&mut a, 2_000).await.unwrap();
    client(&first, KEY).sync(&mut b, 2_000).await.unwrap();
    assert_eq!(b.library().len(), 3);

    let restored = start_later(SqliteDb::open(&backup).unwrap()).await;
    b.record_effect(&Effect::SaveLibraryItem(item("tt4", "Four", 4)), 3_000);
    let report = client(&restored, KEY).sync(&mut b, 3_000).await.unwrap();
    assert!(report.started_over);
    client(&restored, KEY).sync(&mut a, 4_000).await.unwrap();
    client(&restored, KEY).sync(&mut b, 5_000).await.unwrap();
    assert_eq!(ids(&a), ids(&b));
    assert_eq!(a.library().len(), 4);
    assert!(a.outgoing().is_empty() && b.outgoing().is_empty());
    let mut fresh = Replica::new(DeviceId(3), Settings::default());
    client(&restored, KEY)
        .sync(&mut fresh, 6_000)
        .await
        .unwrap();
    assert_eq!(ids(&fresh), ids(&a), "the server has everything again");
}

#[tokio::test]
async fn devices_recover_when_the_server_starts_empty() {
    let first = start(SqliteDb::open_in_memory().unwrap()).await;
    let mut a = Replica::new(DeviceId(1), Settings::default());
    for i in 0..3 {
        a.record_effect(
            &Effect::SaveLibraryItem(item(&format!("tt{i}"), "x", i)),
            1_000,
        );
    }
    client(&first, KEY).sync(&mut a, 1_000).await.unwrap();
    let empty = start(SqliteDb::open_in_memory().unwrap()).await;
    let report = client(&empty, KEY).sync(&mut a, 2_000).await.unwrap();
    assert!(report.started_over);
    assert_eq!(report.pushed, 3);
    let mut b = Replica::new(DeviceId(2), Settings::default());
    client(&empty, KEY).sync(&mut b, 3_000).await.unwrap();
    assert_eq!(b.library().len(), 3);
}

#[tokio::test]
async fn a_device_that_missed_records_after_a_restore_recovers_when_it_pushes() {
    let dir = tempfile::tempdir().unwrap();
    let (live, backup) = (dir.path().join("sync.db"), dir.path().join("backup.db"));
    let first = start(SqliteDb::open(&live).unwrap()).await;
    let mut a = Replica::new(DeviceId(1), Settings::default());
    let mut b = Replica::new(DeviceId(2), Settings::default());
    a.record_effect(&Effect::SaveLibraryItem(item("tt1", "One", 1)), 1_000);
    client(&first, KEY).sync(&mut a, 1_000).await.unwrap();
    snapshot(&live, &backup);
    for i in 2..6 {
        a.record_effect(
            &Effect::SaveLibraryItem(item(&format!("tt{i}"), "x", i)),
            2_000,
        );
    }
    client(&first, KEY).sync(&mut a, 2_000).await.unwrap();
    client(&first, KEY).sync(&mut b, 2_000).await.unwrap();

    let restored = start_later(SqliteDb::open(&backup).unwrap()).await;
    for i in 6..12 {
        a.record_effect(
            &Effect::SaveLibraryItem(item(&format!("tt{i}"), "x", i)),
            3_000,
        );
    }
    client(&restored, KEY).sync(&mut a, 3_000).await.unwrap();
    b.record_effect(&Effect::SaveLibraryItem(item("tt1", "One", 99)), 4_000);
    client(&restored, KEY).sync(&mut b, 4_000).await.unwrap();
    assert_eq!(b.library().len(), 11, "new versions are above the old ones");
    client(&restored, KEY).sync(&mut a, 5_000).await.unwrap();
    assert_eq!(ids(&a), ids(&b));
}

#[tokio::test]
async fn a_record_only_one_device_had_comes_back_with_the_daily_check() {
    let dir = tempfile::tempdir().unwrap();
    let (live, backup) = (dir.path().join("sync.db"), dir.path().join("backup.db"));
    let first = start(SqliteDb::open(&live).unwrap()).await;
    let mut a = Replica::new(DeviceId(1), Settings::default());
    let mut b = Replica::new(DeviceId(2), Settings::default());
    a.record_effect(&Effect::SaveLibraryItem(item("tt1", "One", 1)), 1_000);
    client(&first, KEY).sync(&mut a, 1_000).await.unwrap();
    client(&first, KEY).sync(&mut b, 1_000).await.unwrap();
    snapshot(&live, &backup);
    b.record_effect(&Effect::SaveLibraryItem(item("only-b", "B", 2)), 2_000);
    client(&first, KEY).sync(&mut b, 2_000).await.unwrap();

    let restored = start_later(SqliteDb::open(&backup).unwrap()).await;
    a.record_effect(&Effect::SaveLibraryItem(item("tt2", "Two", 3)), 3_000);
    client(&restored, KEY).sync(&mut a, 3_000).await.unwrap();
    let report = client(&restored, KEY).sync(&mut b, 4_000).await.unwrap();
    assert!(
        !report.started_over,
        "nothing told b that the server went back"
    );
    assert!(
        a.get(&cineo_core::sync::RecordName::Item("only-b".into()))
            .is_none()
    );

    let day = cineo_core::sync::VERIFY_EVERY_MS;
    client(&restored, KEY)
        .sync(&mut b, 1_000 + day)
        .await
        .unwrap();
    client(&restored, KEY)
        .sync(&mut a, 2_000 + day)
        .await
        .unwrap();
    assert_eq!(ids(&a), ids(&b));
    assert_eq!(a.library().len(), 3);
}

#[tokio::test]
#[ignore = "needs a running server in CINEO_SYNC_SERVER, such as `wrangler dev`"]
async fn a_running_server_syncs_recovers_and_deletes() {
    let server = std::env::var("CINEO_SYNC_SERVER").expect("CINEO_SYNC_SERVER is set");
    let key: [u8; 16] = std::array::from_fn(|i| {
        u8::try_from(system_ms() % 251).unwrap_or(0) ^ u8::try_from(i).unwrap_or(0)
    });
    let mut a = Replica::new(DeviceId(1), Settings::default());
    let mut b = Replica::new(DeviceId(2), Settings::default());
    for i in 0..150 {
        a.record_effect(
            &Effect::SaveLibraryItem(item(&format!("tt{i:07}"), &"n".repeat(300), i)),
            1_000,
        );
    }
    let report = client(&server, key).sync(&mut a, 1_000).await.unwrap();
    assert_eq!(report.pushed, 150);
    client(&server, key).sync(&mut b, 1_000).await.unwrap();
    assert_eq!(b.library().len(), 150);

    a.record_effect(&Effect::SaveLibraryItem(item("tt0000001", "x", 7)), 2_000);
    b.record_effect(&Effect::SaveLibraryItem(item("tt0000001", "x", 9)), 3_000);
    client(&server, key).sync(&mut a, 2_000).await.unwrap();
    client(&server, key).sync(&mut b, 3_000).await.unwrap();
    client(&server, key).sync(&mut a, 4_000).await.unwrap();
    assert_eq!(ids(&a), ids(&b));

    client(&server, key).delete_space().await.unwrap();
    assert!(matches!(
        client(&server, key).pull(0).await,
        Err(SyncError::SpaceGone)
    ));
}
