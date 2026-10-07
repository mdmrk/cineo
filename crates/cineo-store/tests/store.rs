//! `cineo-store` against real SQLite files in a temporary directory.

// Test helpers panic on purpose: a panic is a failed assertion.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use cineo_core::addon::{
    ContentType, TransportUrl, parse_manifest, parse_meta_response, parse_stream_response,
};
use cineo_core::app::{Action, Effect, LibraryItem, Settings, State, continue_watching, update};
use cineo_store::{DB_FILE, SCHEMA_VERSION, Store, StoreError, diagnose};
use url::Url;

fn fixture(rel: &str) -> Vec<u8> {
    let full = format!("{}/../../tests/fixtures/{rel}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&full).unwrap_or_else(|err| panic!("reading {full}: {err}"))
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("store")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn addon(s: &str) -> TransportUrl {
    TransportUrl::parse(s).unwrap()
}

fn item(id: &str, time_offset_ms: u64, updated_ms: u64) -> LibraryItem {
    LibraryItem {
        id: id.into(),
        content_type: ContentType::new("movie").unwrap(),
        name: format!("Name of {id}"),
        poster: Some(Url::parse("https://img.example/p.jpg").unwrap()),
        video_id: id.into(),
        time_offset_ms,
        duration_ms: 6_000_000,
        updated_ms,
    }
}

fn user_version(path: &Path) -> i64 {
    rusqlite::Connection::open(path)
        .unwrap()
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap()
}

#[test]
fn fresh_database_is_created_at_the_latest_schema_version() {
    let dir = temp_dir("fresh").join("nested");
    let store = Store::open_in(&dir).unwrap();
    assert!(store.addons().unwrap().is_empty());
    assert!(store.library().unwrap().is_empty());
    assert_eq!(user_version(&dir.join(DB_FILE)), SCHEMA_VERSION);
}

#[cfg(unix)]
#[test]
fn data_directory_is_private_to_the_user() {
    use std::os::unix::fs::PermissionsExt;
    let dir = temp_dir("private").join("cineo");
    Store::open_in(&dir).unwrap();
    let mode = std::fs::metadata(&dir).unwrap().permissions().mode();
    assert_eq!(mode & 0o077, 0, "mode {mode:o}");
}

#[test]
fn addons_round_trip_in_order_and_replace_the_previous_list() {
    let mut store = Store::open_in_memory().unwrap();
    let a = addon("https://a.example/manifest.json");
    let b = addon("https://b.example/cfg%7B%7D/manifest.json?key=1");
    let c = addon("https://c.example/manifest.json");
    store
        .save_addons(&[b.clone(), a.clone(), c.clone()])
        .unwrap();
    assert_eq!(store.addons().unwrap(), vec![b.clone(), a.clone(), c]);

    store
        .save_addons(&[a.clone(), b.clone(), a.clone()])
        .unwrap();
    assert_eq!(store.addons().unwrap(), vec![a, b], "duplicates keep first");
}

#[test]
fn library_items_upsert_and_delete() {
    let store = Store::open_in_memory().unwrap();
    store.save_library_item(&item("tt1", 0, 1)).unwrap();
    store.save_library_item(&item("tt2", 5, 2)).unwrap();
    let updated = LibraryItem {
        poster: None,
        ..item("tt1", 42_000, 3)
    };
    store.save_library_item(&updated).unwrap();
    assert_eq!(store.library().unwrap(), vec![updated, item("tt2", 5, 2)]);

    store.delete_library_item("tt2").unwrap();
    store.delete_library_item("missing").unwrap();
    assert_eq!(store.library().unwrap().len(), 1);
}

#[test]
fn hostile_ids_are_stored_as_data() {
    let store = Store::open_in_memory().unwrap();
    let id = "x'); DROP TABLE library_items; --/../../etc";
    store.save_library_item(&item(id, 1, 1)).unwrap();
    assert_eq!(store.library().unwrap()[0].id, id);
}

#[test]
fn apply_runs_persistence_effects_and_ignores_others() {
    let mut store = Store::open_in_memory().unwrap();
    let a = addon("https://a.example/manifest.json");
    assert!(store.apply(&Effect::SaveAddons(vec![a.clone()])).unwrap());
    assert!(
        store
            .apply(&Effect::SaveLibraryItem(item("tt1", 1, 1)))
            .unwrap()
    );
    assert!(
        !store
            .apply(&Effect::FetchManifest {
                transport: a.clone(),
                install: false
            })
            .unwrap()
    );
    assert!(
        store
            .apply(&Effect::DeleteLibraryItem("tt1".into()))
            .unwrap()
    );
    assert_eq!(store.addons().unwrap(), vec![a]);
    assert!(store.library().unwrap().is_empty());
}

#[test]
fn v1_fixture_database_opens_and_invalid_rows_are_skipped() {
    let dir = temp_dir("v1");
    let path = dir.join(DB_FILE);
    let sql = String::from_utf8(fixture("store/v1.sql")).unwrap();
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute_batch(&sql)
        .unwrap();

    let store = Store::open(&path).unwrap();
    assert_eq!(
        store.addons().unwrap(),
        vec![
            addon("https://first.example/path/manifest.json?token=x"),
            addon("https://second.example/manifest.json"),
        ]
    );
    let library = store.library().unwrap();
    let ids: Vec<_> = library.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(ids, vec!["tt0000002", "tt0000001"], "most recent first");
    assert_eq!(library[0].poster, None, "non-http poster dropped");
    assert_eq!(library[0].video_id, "tt0000002:1:2");
    assert_eq!(library[1].time_offset_ms, 600_000);
    assert_eq!(user_version(&path), SCHEMA_VERSION);
    assert_eq!(store.settings().unwrap(), Settings::default());
}

#[test]
fn settings_default_until_saved_and_survive_a_reopen() {
    let dir = temp_dir("settings");
    let mut store = Store::open_in(&dir).unwrap();
    assert_eq!(store.settings().unwrap(), Settings::default());
    let changed = Settings {
        p2p_enabled: false,
        p2p_acknowledged: true,
    };
    assert!(store.apply(&Effect::SaveSettings(changed)).unwrap());
    drop(store);
    assert_eq!(Store::open_in(&dir).unwrap().settings().unwrap(), changed);
}

#[test]
fn unknown_or_unreadable_settings_are_ignored() {
    let dir = temp_dir("settings-junk");
    let store = Store::open_in(&dir).unwrap();
    drop(store);
    rusqlite::Connection::open(dir.join(DB_FILE))
        .unwrap()
        .execute_batch(
            "INSERT INTO settings VALUES ('p2p_enabled', 'maybe');
             INSERT INTO settings VALUES ('from_the_future', 'true');
             INSERT INTO settings VALUES ('p2p_acknowledged', 'true');",
        )
        .unwrap();
    let settings = Store::open_in(&dir).unwrap().settings().unwrap();
    assert_eq!(
        settings,
        Settings {
            p2p_enabled: true,
            p2p_acknowledged: true,
        }
    );
}

#[test]
fn newer_schema_is_refused_and_left_untouched() {
    let dir = temp_dir("newer");
    let path = dir.join(DB_FILE);
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute_batch("CREATE TABLE future (x); PRAGMA user_version = 99;")
        .unwrap();
    let before = std::fs::read(&path).unwrap();

    let err = Store::open(&path).unwrap_err();
    assert!(
        matches!(err, StoreError::UnsupportedVersion { found: 99, .. }),
        "{err:?}"
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn corrupt_file_is_reported_and_left_untouched() {
    let dir = temp_dir("corrupt");
    let path = dir.join(DB_FILE);
    let garbage = b"this is not a sqlite database, just some bytes \x00\x01\x02".repeat(100);
    std::fs::write(&path, &garbage).unwrap();

    let err = Store::open(&path).unwrap_err();
    assert!(matches!(err, StoreError::Corrupt(_)), "{err:?}");
    assert_eq!(std::fs::read(&path).unwrap(), garbage);

    let report = diagnose(&path);
    assert!(!report.is_healthy());
    assert!(report.error.is_some(), "{report:?}");
}

#[test]
fn doctor_reports_missing_and_healthy_databases_without_creating_one() {
    let dir = temp_dir("doctor");
    let path = dir.join(DB_FILE);
    let missing = diagnose(&path);
    assert!(!missing.exists && missing.is_healthy());
    assert!(!path.exists(), "doctor never creates the database");

    let mut store = Store::open(&path).unwrap();
    store
        .save_addons(&[addon("https://a.example/manifest.json")])
        .unwrap();
    store.save_library_item(&item("tt1", 1, 1)).unwrap();
    drop(store);

    let report = diagnose(&path);
    assert!(report.is_healthy(), "{report:?}");
    assert_eq!(report.schema_version, Some(SCHEMA_VERSION));
    assert_eq!(report.addons, Some(1));
    assert_eq!(report.library_items, Some(1));
    assert_eq!(report.integrity, vec!["ok".to_owned()]);
}

const BASIC: &str = "https://basic.example/manifest.json";

fn start_app(store: &Store) -> State {
    let mut state = State::default();
    let effects = update(
        &mut state,
        Action::Restore {
            addons: store.addons().unwrap(),
            library: store.library().unwrap(),
        },
    );
    for effect in effects {
        if let Effect::FetchManifest { transport, install } = effect {
            let manifest = parse_manifest(&fixture("addons/basic/manifest.json")).unwrap();
            update(
                &mut state,
                Action::ManifestLoaded {
                    transport,
                    result: Ok(Box::new(manifest.value)),
                    install,
                },
            );
        }
    }
    state
}

fn play_movie(state: &mut State, store: &mut Store) -> u64 {
    let mut run = |state: &mut State, action| -> Vec<Effect> {
        let effects = update(state, action);
        for effect in &effects {
            store.apply(effect).unwrap();
        }
        effects
    };
    let fetch = run(
        state,
        Action::OpenDetail {
            content_type: ContentType::new("movie").unwrap(),
            id: "tt0000001".into(),
            preview: None,
        },
    );
    let [Effect::FetchMeta { addon, path }] = fetch.as_slice() else {
        panic!("{fetch:?}")
    };
    let meta = parse_meta_response(&fixture("addons/basic/meta-movie.json")).unwrap();
    let fetch = run(
        state,
        Action::MetaLoaded {
            addon: addon.clone(),
            path: path.clone(),
            result: Ok(Box::new(meta.value)),
        },
    );
    for effect in fetch {
        if let Effect::FetchStreams { addon, path } = effect {
            let streams = parse_stream_response(&fixture("addons/basic/streams-movie.json"));
            run(
                state,
                Action::StreamsLoaded {
                    addon,
                    path,
                    result: Ok(streams.unwrap().value),
                },
            );
        }
    }
    let effects = run(
        state,
        Action::Play {
            group: 0,
            stream: 0,
        },
    );
    let Some(play) = effects.iter().find_map(|e| match e {
        Effect::Play(play) => Some(play),
        _ => None,
    }) else {
        panic!("{effects:?}")
    };
    play.start_ms
}

#[test]
fn continue_watching_resumes_at_the_saved_position_after_restart() {
    let dir = temp_dir("acceptance");

    {
        let mut store = Store::open_in(&dir).unwrap();
        store
            .apply(&Effect::SaveAddons(vec![addon(BASIC)]))
            .unwrap();
        let mut state = start_app(&store);
        assert_eq!(play_movie(&mut state, &mut store), 0);
        for effect in update(
            &mut state,
            Action::PlaybackProgress {
                meta_id: "tt0000001".into(),
                video_id: "tt0000001".into(),
                time_ms: 754_000,
                duration_ms: 6_000_000,
                now_ms: 1_700_000_000_000,
            },
        ) {
            store.apply(&effect).unwrap();
        }
    }

    let mut store = Store::open_in(&dir).unwrap();
    let mut state = start_app(&store);
    assert_eq!(state.addons.len(), 1);
    let resume: Vec<_> = continue_watching(&state.library)
        .iter()
        .map(|i| (i.id.as_str(), i.time_offset_ms))
        .collect();
    assert_eq!(resume, vec![("tt0000001", 754_000)]);
    assert_eq!(play_movie(&mut state, &mut store), 754_000);
}
