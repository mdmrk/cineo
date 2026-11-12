use super::*;

#[test]
fn the_app_reports_the_workspace_version() {
    let manifest =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml"))
            .unwrap();
    let workspace = manifest
        .split("[workspace.package]")
        .nth(1)
        .and_then(|section| section.lines().find_map(|l| l.strip_prefix("version = ")))
        .map(|v| v.trim_matches('"'))
        .unwrap();
    assert_eq!(version(), workspace);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_installed_addon_fills_the_board_and_is_back_after_a_restart() {
    let server = MockServer::start().await;
    serve_manifest(&server).await;
    serve(
        &server,
        "/cfg/catalog/movie/top.json",
        fixture("addons/basic/catalog-movie-top.json"),
    )
    .await;
    let manifest = format!("{}/cfg/manifest.json", server.uri());
    let dir = local_data_dir();

    tokio::task::block_in_place(|| {
        let (cineo, views) = start(dir.path());
        let empty = wait_for(&views, |_| true);
        assert!(empty.addons.is_empty() && empty.board.is_empty());

        cineo.install_addon(manifest.clone());
        let board = wait_for(&views, |v| {
            !v.board.is_empty() && v.board.iter().all(|r| r.items != RowItems::Loading)
        });
        assert_eq!(
            board.install,
            Some(InstallView::Installed {
                name: "Basic Fixture".into()
            })
        );
        assert_eq!(board.addons.len(), 1);
        assert_eq!(board.addons[0].transport, manifest);
        assert_eq!(board.addons[0].version, "1.2.0");
        let names: Vec<_> = board.board.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["Top Movies", "Multi-genre"]);
        let RowItems::Ready { items } = &board.board[0].items else {
            panic!("the movie row did not load: {:?}", board.board[0].items);
        };
        assert_eq!(items[0].id, "tt0000001");
        assert_eq!(items[0].name, "First Example Film");
        assert!(matches!(
            board.board[1].items,
            RowItems::Failed {
                problem: ProblemView::Request { .. }
            }
        ));
        drop(cineo);

        let (_cineo, views) = start(dir.path());
        let restored = wait_for(&views, |v| v.addons.len() == 1 && v.addons_loading == 0);
        assert_eq!(restored.addons[0].name, "Basic Fixture");
        assert_eq!(restored.install, None);
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn a_second_install_of_the_same_addon_is_refused() {
    let server = MockServer::start().await;
    serve_manifest(&server).await;
    let manifest = format!("{}/cfg/manifest.json", server.uri());
    let dir = local_data_dir();

    tokio::task::block_in_place(|| {
        let (cineo, views) = start(dir.path());
        cineo.install_addon(manifest.clone());
        wait_for(&views, |v| v.addons.len() == 1);
        cineo.install_addon(manifest);
        wait_for(&views, |v| {
            v.install
                == Some(InstallView::Failed {
                    problem: ProblemView::AlreadyInstalled,
                })
        });
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn images_come_through_the_addon_client_and_its_limits() {
    let server = MockServer::start().await;
    let png = vec![0x89, b'P', b'N', b'G', 1, 2, 3];
    Mock::given(method("GET"))
        .and(path("/poster.png"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(png.clone(), "image/png"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/huge.png"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(vec![0; 5 * 1024 * 1024], "image/png"),
        )
        .mount(&server)
        .await;
    let dir = local_data_dir();
    let (cineo, _views) = tokio::task::block_in_place(|| start(dir.path()));

    let bytes = cineo
        .fetch_image(format!("{}/poster.png", server.uri()))
        .await
        .unwrap();
    assert_eq!(bytes, png);
    let huge = cineo
        .fetch_image(format!("{}/huge.png", server.uri()))
        .await;
    assert!(matches!(huge, Err(ImageError::Fetch { .. })), "{huge:?}");
    let bad = cineo.fetch_image("not a url".into()).await;
    assert!(matches!(bad, Err(ImageError::InvalidUrl { .. })), "{bad:?}");
    tokio::task::block_in_place(|| drop(cineo));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_catalog_that_repeats_an_item_shows_it_once() {
    let server = MockServer::start().await;
    serve_manifest(&server).await;
    let metas = serde_json::json!({ "metas": [
        { "id": "tt1", "type": "movie", "name": "First" },
        { "id": "tt2", "type": "movie", "name": "Second" },
        { "id": "tt1", "type": "movie", "name": "First again" },
    ]});
    serve(
        &server,
        "/cfg/catalog/movie/top.json",
        serde_json::to_vec(&metas).unwrap(),
    )
    .await;
    let manifest = format!("{}/cfg/manifest.json", server.uri());
    let dir = local_data_dir();

    tokio::task::block_in_place(|| {
        let (cineo, views) = start(dir.path());
        cineo.install_addon(manifest);
        let board = wait_for(&views, |v| {
            v.board
                .first()
                .is_some_and(|r| matches!(r.items, RowItems::Ready { .. }))
        });
        let RowItems::Ready { items } = &board.board[0].items else {
            panic!("the row did not load");
        };
        let names: Vec<_> = items.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["First", "Second"]);
        drop(cineo);
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn an_image_nobody_waits_for_stops_downloading() {
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let url = format!("http://{}/poster.png", listener.local_addr().unwrap());
    let dir = local_data_dir();
    let (cineo, _views) = tokio::task::block_in_place(|| start(dir.path()));

    let (mut socket, _) = tokio::select! {
        result = cineo.fetch_image(url) => panic!("the image finished: {result:?}"),
        accepted = listener.accept() => accepted.unwrap(),
    };
    let closed = tokio::time::timeout(Duration::from_secs(5), async {
        let mut buffer = [0; 1024];
        while matches!(socket.read(&mut buffer).await, Ok(read) if read > 0) {}
    })
    .await;
    assert!(closed.is_ok(), "the request was still open after 5 s");
    tokio::task::block_in_place(|| drop(cineo));
}

#[tokio::test(flavor = "multi_thread")]
async fn addons_can_be_reordered_and_removed_with_their_rows() {
    let server = MockServer::start().await;
    serve(
        &server,
        "/a/manifest.json",
        fixture("addons/basic/manifest.json"),
    )
    .await;
    serve(
        &server,
        "/b/manifest.json",
        fixture("addons/quirks/manifest.json"),
    )
    .await;
    let basic = format!("{}/a/manifest.json", server.uri());
    let quirks = format!("{}/b/manifest.json", server.uri());
    let dir = local_data_dir();
    let names = |v: &AppView| v.addons.iter().map(|a| a.name.clone()).collect::<Vec<_>>();

    tokio::task::block_in_place(|| {
        let (cineo, views) = start(dir.path());
        cineo.install_addon(basic.clone());
        wait_for(&views, |v| v.addons.len() == 1);
        cineo.install_addon(quirks.clone());
        let both = wait_for(&views, |v| v.addons.len() == 2);
        assert_eq!(names(&both), ["Basic Fixture", "Quirks Fixture"]);
        assert!(both.board.iter().any(|r| r.addon_name == "Basic Fixture"));
        cineo.move_addon(1, 0);
        wait_for(&views, |v| names(v) == ["Quirks Fixture", "Basic Fixture"]);
        cineo.move_addon(5, 0);
        cineo.remove_addon("not a url");
        cineo.remove_addon(&basic);
        let removed = wait_for(&views, |v| v.addons.len() == 1);
        assert_eq!(names(&removed), ["Quirks Fixture"]);
        assert!(
            removed
                .board
                .iter()
                .all(|r| r.addon_name != "Basic Fixture")
        );
        drop(cineo);

        let (_cineo, views) = start(dir.path());
        let restored = wait_for(&views, |v| v.addons.len() == 1 && v.addons_loading == 0);
        assert_eq!(restored.addons[0].transport, quirks);
        assert!(restored.unavailable_addons.is_empty());
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn an_addon_that_no_longer_loads_is_listed_and_can_be_removed() {
    let server = MockServer::start().await;
    let manifest = movie_addon(&server).await;
    let dir = local_data_dir();
    tokio::task::block_in_place(|| {
        let (cineo, views) = start(dir.path());
        cineo.install_addon(manifest.clone());
        wait_for(&views, |v| v.addons.len() == 1);
    });
    server.reset().await;

    tokio::task::block_in_place(|| {
        let (cineo, views) = start(dir.path());
        let broken = wait_for(&views, |v| {
            v.addons_loading == 0 && !v.unavailable_addons.is_empty()
        });
        assert!(broken.addons.is_empty());
        assert_eq!(broken.unavailable_addons, std::slice::from_ref(&manifest));
        cineo.remove_addon(&manifest);
        wait_for(&views, |v| v.unavailable_addons.is_empty());
        drop(cineo);
    });
    assert!(
        Store::open_in(dir.path())
            .unwrap()
            .addons()
            .unwrap()
            .is_empty()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_usable_network_asks_again_for_what_failed() {
    asks_again_for_what_failed(true).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_usable_network_that_keeps_its_connections_asks_again_too() {
    asks_again_for_what_failed(false).await;
}

async fn asks_again_for_what_failed(renew: bool) {
    let server = MockServer::start().await;
    serve_manifest(&server).await;
    let manifest = format!("{}/cfg/manifest.json", server.uri());
    let dir = local_data_dir();

    tokio::task::block_in_place(|| {
        let (cineo, views) = start(dir.path());
        cineo.install_addon(manifest);
        wait_for(&views, |v| {
            !v.board.is_empty()
                && v.board
                    .iter()
                    .all(|r| matches!(r.items, RowItems::Failed { .. }))
        });

        tokio::runtime::Handle::current().block_on(serve(
            &server,
            "/cfg/catalog/movie/top.json",
            fixture("addons/basic/catalog-movie-top.json"),
        ));
        cineo.network_usable(renew);

        wait_for(&views, |v| {
            v.board
                .iter()
                .any(|r| matches!(r.items, RowItems::Ready { .. }))
        });
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn a_search_fills_rows_from_search_catalogs_and_opens_its_results() {
    let server = MockServer::start().await;
    let manifest = movie_addon(&server).await;
    serve(
        &server,
        "/cfg/catalog/movie/search/search=first.json",
        fixture("addons/basic/catalog-movie-top.json"),
    )
    .await;
    let dir = local_data_dir();

    tokio::task::block_in_place(|| {
        let (cineo, views) = start(dir.path());
        cineo.install_addon(manifest);
        wait_for(&views, |v| v.addons.len() == 1);
        cineo.search("  first ".into());
        let found = wait_for(&views, |v| {
            !v.search.is_empty() && v.search.iter().all(|r| r.items != RowItems::Loading)
        });
        assert_eq!(found.search_query, "first");
        assert_eq!(found.search.len(), 1);
        let RowItems::Ready { items } = &found.search[0].items else {
            panic!("the search did not load: {:?}", found.search[0].items);
        };
        assert_eq!(items[0].name, "First Example Film");
        cineo.open_detail("movie".into(), items[0].id.clone());
        let opened = wait_for(&views, |v| v.detail.is_some());
        assert_eq!(
            opened.detail.unwrap().name.as_deref(),
            Some("First Example Film")
        );
        cineo.search(String::new());
        wait_for(&views, |v| v.search.is_empty() && v.search_query.is_empty());
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn addon_lists_offer_installs_and_configuration_links() {
    let server = MockServer::start().await;
    serve(
        &server,
        "/lists/manifest.json",
        fixture("addons/basic/manifest-addon-lists.json"),
    )
    .await;
    serve_manifest(&server).await;
    let base = server.uri();
    let basic: serde_json::Value =
        serde_json::from_slice(&fixture("addons/basic/manifest.json")).unwrap();
    let mut setup = basic.clone();
    setup["id"] = "org.cineo.fixture.setup".into();
    setup["name"] = "Setup Fixture".into();
    setup["behaviorHints"] =
        serde_json::json!({ "configurable": true, "configurationRequired": true });
    let official = serde_json::json!({ "addons": [
        { "transportUrl": format!("{base}/cfg/manifest.json"), "manifest": basic },
        { "transportUrl": format!("{base}/setup/manifest.json"), "manifest": setup },
    ]});
    serve(
        &server,
        "/lists/addon_catalog/all/official.json",
        official.to_string().into_bytes(),
    )
    .await;
    serve(
        &server,
        "/lists/addon_catalog/all/community.json",
        br#"{"addons":[]}"#.to_vec(),
    )
    .await;
    let dir = local_data_dir();

    tokio::task::block_in_place(|| {
        let (cineo, views) = start(dir.path());
        cineo.install_addon(format!("{base}/lists/manifest.json"));
        wait_for(&views, |v| v.addons.len() == 1);
        cineo.load_addon_lists();
        let loaded = wait_for(&views, |v| {
            v.addon_lists.len() == 2
                && v.addon_lists
                    .iter()
                    .all(|l| l.items != AddonListItems::Loading)
        });
        let names: Vec<_> = loaded.addon_lists.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, ["Official", "Community"]);
        let AddonListItems::Ready { addons } = &loaded.addon_lists[0].items else {
            panic!("the list did not load: {:?}", loaded.addon_lists[0].items);
        };
        let listed: Vec<_> = addons.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(listed, ["Basic Fixture", "Setup Fixture"]);
        assert!(!addons[0].installed && !addons[0].needs_setup);
        assert!(addons[1].needs_setup);
        assert_eq!(addons[1].configure_url, format!("{base}/setup/configure"));

        cineo.install_addon(addons[0].transport.clone());
        let installed = wait_for(&views, |v| v.addons.len() == 2);
        let AddonListItems::Ready { addons } = &installed.addon_lists[0].items else {
            panic!("the list is gone: {:?}", installed.addon_lists[0].items);
        };
        assert!(addons[0].installed);
        assert_eq!(
            installed.addons[1].configure_url.as_deref(),
            Some(format!("{base}/cfg/configure").as_str())
        );
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn streams_can_be_filtered_and_sorted_like_on_the_desktop() {
    let server = MockServer::start().await;
    serve_movie(&server).await;
    let streams = r#"{"streams":[
        {"name":"Fixture\n720p","description":"Film.720p.WEB 👤 5 💾 700 MB","url":"https://media.example/720.mp4"},
        {"name":"Fixture\n4k HDR","description":"Film.2160p.REMUX 👤 2 💾 15 GB","url":"https://media.example/4k.mkv"},
        {"name":"Fixture\n1080p","description":"Film.1080p.WEB 👤 40 💾 2 GB","url":"https://media.example/1080.mp4"},
        {"name":"Fixture\n1080p","description":"Film.1080p.BluRay 👤 90","infoHash":"0123456789abcdef0123456789abcdef01234567"}
    ]}"#;
    serve(
        &server,
        "/cfg/stream/movie/tt0000001.json",
        streams.as_bytes().to_vec(),
    )
    .await;
    let manifest = format!("{}/cfg/manifest.json", server.uri());
    let dir = local_data_dir();
    let order = |v: &AppView| -> (Vec<u32>, u32) {
        let detail = v.detail.as_ref().unwrap();
        match &detail.streams[0].items {
            StreamItems::Ready { order, hidden, .. } => (order.clone(), *hidden),
            other => panic!("streams are not ready: {other:?}"),
        }
    };

    tokio::task::block_in_place(|| {
        let (cineo, views) = start(dir.path());
        cineo.install_addon(manifest);
        wait_for(&views, |v| v.addons.len() == 1);
        cineo.open_detail("movie".into(), "tt0000001".into());
        let ready = wait_for(&views, streams_ready);
        assert_eq!(order(&ready), (vec![0, 1, 2, 3], 0));
        let detail = ready.detail.as_ref().unwrap();
        let shown = [Resolution::Uhd, Resolution::FullHd, Resolution::Hd];
        assert_eq!(detail.qualities, shown);
        let StreamItems::Ready { streams, .. } = &detail.streams[0].items else {
            unreachable!()
        };
        let four_k = streams[1].quality.as_ref().unwrap();
        assert_eq!(
            (four_k.resolution, four_k.tier, four_k.flags.clone()),
            (Resolution::Uhd, Tier::Ultra, vec!["HDR".to_owned()])
        );

        cineo.set_stream_sort(StreamSort::Seeders);
        wait_for(&views, |v| order(v).0 == [3, 2, 0, 1]);
        cineo.set_stream_quality(Some(Resolution::FullHd));
        wait_for(&views, |v| order(v).0 == [3, 2]);
        cineo.set_stream_query("web".into());
        wait_for(&views, |v| order(v).0 == [2]);
        cineo.set_stream_addon(Some(1));
        wait_for(&views, |v| order(v).0.is_empty());
        cineo.set_stream_addon(None);
        cineo.set_stream_query(String::new());
        cineo.set_stream_quality(Some(Resolution::Sd));
        wait_for(&views, |v| order(v).0.is_empty());
        cineo.set_stream_quality(None);
        cineo.change_setting(SettingChange::P2pEnabled { on: false });
        let off = wait_for(&views, |v| order(v) == (vec![2, 0, 1], 1));
        assert_eq!(off.detail.unwrap().qualities, shown);
        cineo.change_setting(SettingChange::P2pEnabled { on: true });
        cineo.set_stream_query("bluray".into());
        wait_for(&views, |v| order(v).0 == [3]);

        cineo.close_detail();
        wait_for(&views, |v| v.detail.is_none());
        cineo.open_detail("movie".into(), "tt0000001".into());
        let reopened = wait_for(&views, streams_ready);
        let filter = &reopened.detail.as_ref().unwrap().filter;
        assert_eq!(filter.query, "");
        assert_eq!(filter.sort, StreamSort::Seeders);
        assert_eq!(order(&reopened).0, [3, 2, 0, 1]);
    });
}
