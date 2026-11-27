use cineo_android::{SyncInputError, SyncStatusView, SyncView, sync_link};
use cineo_core::addon::ContentType;
use cineo_core::app::LibraryItem;
use cineo_sync_server::{ServeOptions, SqliteDb, serve as serve_sync};
use tokio::net::TcpListener;

use super::*;

pub(super) async fn sync_server() -> String {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    let db = Arc::new(SqliteDb::open_in_memory().unwrap());
    tokio::spawn(serve_sync(listener, db, ServeOptions::default()));
    format!("http://{address}/")
}

fn favorite(id: &str) -> LibraryItem {
    LibraryItem {
        id: id.to_owned(),
        content_type: ContentType::new("movie").unwrap(),
        name: format!("Movie {id}"),
        poster: None,
        video_id: id.to_owned(),
        time_offset_ms: 0,
        duration_ms: 0,
        updated_ms: 1,
        favorited: Some(1),
        stream: None,
    }
}

fn synced(view: &AppView) -> Option<&SyncView> {
    view.sync
        .as_ref()
        .filter(|s| matches!(s.status, SyncStatusView::Synced { .. }))
}

fn favorites(view: &AppView) -> Vec<&str> {
    view.favorites.iter().map(|i| i.id.as_str()).collect()
}

fn until(
    views: &Receiver<AppView>,
    mut nudge: impl FnMut(),
    done: impl Fn(&AppView) -> bool,
) -> AppView {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        nudge();
        let step = Instant::now() + Duration::from_millis(500);
        while let Some(left) = step.checked_duration_since(Instant::now()) {
            match views.recv_timeout(left) {
                Ok(view) if done(&view) => return view,
                Ok(_) => {}
                Err(_) => break,
            }
        }
    }
    panic!("the view never got there");
}

#[tokio::test(flavor = "multi_thread")]
async fn two_phones_share_their_library_and_settings_through_the_sync_server() {
    let server = sync_server().await;
    let first_dir = local_data_dir();
    Store::open_in(first_dir.path())
        .unwrap()
        .save_library_item(&favorite("tt1"))
        .unwrap();
    let second_dir = local_data_dir();

    tokio::task::block_in_place(|| {
        let (first, first_views) = start(first_dir.path());
        wait_for(&first_views, |v| v.sync.is_none());
        first.change_setting(SettingChange::SeekStep { seconds: 30 });
        first.sync_turn_on(&server).unwrap();
        let on = wait_for(&first_views, |v| synced(v).is_some());
        let code = on.sync.unwrap().code;

        let (second, second_views) = start(second_dir.path());
        wait_for(&second_views, |v| v.sync.is_none());
        second.sync_pair(&code.to_lowercase(), &server).unwrap();
        let paired = wait_for(&second_views, |v| {
            synced(v).is_some() && favorites(v) == ["tt1"]
        });
        assert_eq!(paired.settings.seek_step_s, 30);
        assert_eq!(paired.sync.unwrap().code, code);

        second.set_favorite("tt1".into(), false);
        second.sync_now();
        until(
            &first_views,
            || first.app_visible(true),
            |v| favorites(v).is_empty(),
        );
        drop(first);

        let (_first, first_views) = start(first_dir.path());
        let restored = wait_for(&first_views, |v| v.sync.is_some());
        assert!(favorites(&restored).is_empty());
        assert_eq!(restored.sync.unwrap().code, code);
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn turning_off_and_deleting_stops_the_other_phone_until_it_pairs_again() {
    let server = sync_server().await;
    let first_dir = local_data_dir();
    let second_dir = local_data_dir();

    tokio::task::block_in_place(|| {
        let (first, first_views) = start(first_dir.path());
        wait_for(&first_views, |v| v.sync.is_none());
        first.sync_turn_on(&server).unwrap();
        let code = wait_for(&first_views, |v| synced(v).is_some())
            .sync
            .unwrap()
            .code;
        let (second, second_views) = start(second_dir.path());
        wait_for(&second_views, |v| v.sync.is_none());
        second.sync_pair(&code, &server).unwrap();
        wait_for(&second_views, |v| synced(v).is_some());

        second.sync_turn_off(true);
        wait_for(&second_views, |v| v.sync.is_none());
        let stopped = until(
            &first_views,
            || first.sync_now(),
            |v| {
                v.sync
                    .as_ref()
                    .is_some_and(|s| s.status == SyncStatusView::KeyChanged)
            },
        );
        assert!(!stopped.sync.unwrap().can_sync_now);

        first.sync_change_key().unwrap();
        let moved = wait_for(&first_views, |v| synced(v).is_some_and(|s| s.code != code));
        let new_code = moved.sync.unwrap().code;
        second.sync_pair(&new_code, &server).unwrap();
        wait_for(&second_views, |v| {
            synced(v).is_some_and(|s| s.code == new_code)
        });
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn a_bad_code_or_an_insecure_server_turns_nothing_on() {
    let dir = local_data_dir();
    tokio::task::block_in_place(|| {
        let (cineo, views) = start(dir.path());
        wait_for(&views, |v| v.sync.is_none());
        let code = "AAAQ-EAYE-AUDA-OCAJ-BIFQ-YDIO-B4KA-47NG";
        let typo = "BAAQ-EAYE-AUDA-OCAJ-BIFQ-YDIO-B4KA-47NG";
        assert_eq!(cineo.sync_pair(typo, ""), Err(SyncInputError::CodeTypo));
        assert_eq!(cineo.sync_pair("ABCD", ""), Err(SyncInputError::CodeLength));
        assert_eq!(
            cineo.sync_pair(code, "http://sync.example/"),
            Err(SyncInputError::Server)
        );
        assert_eq!(
            cineo.sync_turn_on("http://sync.example/"),
            Err(SyncInputError::Server)
        );
        cineo.sync_now();
        cineo.sync_turn_off(true);
        std::thread::sleep(Duration::from_millis(500));
        assert!(views.try_iter().all(|v| v.sync.is_none()));
    });
}

#[test]
fn a_sync_link_gives_its_code_and_server() {
    let code = "AAAQ-EAYE-AUDA-OCAJ-BIFQ-YDIO-B4KA-47NG";
    let plain = sync_link(&format!("cineo:///sync/{code}")).unwrap();
    assert_eq!((plain.code.as_str(), plain.server.as_str()), (code, ""));
    let own = sync_link(&format!(
        "cineo:///sync/{code}?server=https%3A%2F%2Fsync.example%2Fcineo%2F"
    ))
    .unwrap();
    assert_eq!(own.server, "https://sync.example/cineo/");
    assert_eq!(sync_link("cineo:///library/"), None);
    assert_eq!(sync_link("https://example.com/"), None);
}
