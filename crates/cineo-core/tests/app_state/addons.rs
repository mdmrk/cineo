use super::*;

#[test]
fn restore_loads_addons_in_order_then_the_board() {
    let (state, board) = restored();
    let names: Vec<_> = state
        .addons
        .iter()
        .map(|a| a.manifest.name.as_str())
        .collect();
    assert_eq!(names, vec!["Basic Fixture", "Streams Fixture"]);
    let catalogs: Vec<_> = state
        .board
        .iter()
        .map(|r| (r.target.name.as_str(), r.target.path.content_type.as_str()))
        .collect();
    assert_eq!(
        catalogs,
        vec![("Top Movies", "movie"), ("Multi-genre", "series")]
    );
    assert_eq!(board.len(), 2);
    assert!(
        board
            .iter()
            .all(|e| matches!(e, Effect::FetchCatalog { .. }))
    );
}

#[test]
fn board_rows_load_as_soon_as_their_addon_does() {
    let mut state = restore(&[STREAMS, BASIC]);
    let effects = update(&mut state, loaded(BASIC, "basic/manifest.json"));
    assert_eq!(state.board.len(), 2, "no wait for the streams addon");
    assert_eq!(effects.len(), 2);
    let effects = update(&mut state, loaded(STREAMS, "basic/manifest-streams.json"));
    assert!(effects.is_empty(), "loaded rows are not fetched again");
}

#[test]
fn addons_keep_the_saved_order_whatever_order_they_load_in() {
    let mut state = restore(&[BASIC, STREAMS]);
    update(&mut state, loaded(STREAMS, "basic/manifest-streams.json"));
    update(&mut state, loaded(BASIC, "basic/manifest.json"));
    assert_eq!(
        addon_names(&state),
        vec!["Basic Fixture", "Streams Fixture"]
    );
}

#[test]
fn an_addon_that_fails_to_load_stays_installed() {
    const OTHER: &str = "https://other.example/manifest.json";
    let mut state = restore(&[OTHER, BASIC, STREAMS]);
    update(&mut state, load_failed(OTHER));
    update(&mut state, loaded(BASIC, "basic/manifest.json"));
    update(&mut state, loaded(STREAMS, "basic/manifest-streams.json"));
    assert!(state.notice.is_some());

    let effects = update(&mut state, Action::MoveAddon { from: 1, to: 0 });
    assert_eq!(
        effects[0],
        Effect::SaveAddons(vec![url(OTHER), url(STREAMS), url(BASIC)])
    );
    let effects = update(&mut state, Action::RemoveAddon(url(BASIC)));
    assert_eq!(
        effects[0],
        Effect::SaveAddons(vec![url(OTHER), url(STREAMS)])
    );
    assert_eq!(state.unavailable_addons(), vec![&url(OTHER)]);

    let effects = update(&mut state, Action::InstallAddon(OTHER.into()));
    assert!(
        matches!(
            effects.as_slice(),
            [Effect::FetchManifest {
                origin: ManifestOrigin::Install,
                ..
            }]
        ),
        "an unavailable addon can be retried"
    );
    update(
        &mut state,
        Action::ManifestLoaded {
            transport: url(OTHER),
            result: Ok(manifest("basic/manifest.json")),
            origin: ManifestOrigin::Install,
        },
    );
    assert_eq!(state.installed, vec![url(OTHER), url(STREAMS)]);
    assert_eq!(state.addons.len(), 2);
    assert!(state.unavailable_addons().is_empty());
}

#[test]
fn a_manifest_that_arrives_after_removal_is_ignored() {
    let mut state = restore(&[BASIC, STREAMS]);
    update(&mut state, Action::RemoveAddon(url(BASIC)));
    let effects = update(&mut state, loaded(BASIC, "basic/manifest.json"));
    assert!(effects.is_empty(), "{effects:?}");
    assert!(state.addons.is_empty());
    assert_eq!(state.installed, vec![url(STREAMS)]);
}

#[test]
fn reordering_and_removing_addons_reuse_loaded_rows() {
    let (mut state, _) = restored();
    let effects = update(&mut state, Action::MoveAddon { from: 0, to: 1 });
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::FetchCatalog { .. })),
        "{effects:?}"
    );
    assert_eq!(state.board.len(), 2);
}

fn addon_list_path(id: &str) -> ResourcePath {
    ResourcePath {
        resource: ResourceName::AddonCatalog,
        ..ResourcePath::catalog(ty("all"), id)
    }
}

fn fetched_addon_lists(effects: &[Effect]) -> Vec<&str> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::FetchAddonList { addon, path } if *addon == url(LISTS) => {
                Some(path.id.as_str())
            }
            _ => None,
        })
        .collect()
}

#[test]
fn addon_lists_load_when_asked_and_only_failed_ones_retry() {
    let mut state = restore(&[BASIC, LISTS]);
    update(&mut state, loaded(BASIC, "basic/manifest.json"));
    let effects = update(&mut state, loaded(LISTS, "basic/manifest-addon-lists.json"));
    assert!(
        fetched_addon_lists(&effects).is_empty(),
        "not asked for yet"
    );
    assert_eq!(state.addon_lists, None);

    let effects = update(&mut state, Action::LoadAddonLists);
    assert_eq!(
        fetched_addon_lists(&effects),
        vec!["official", "community"],
        "type `all` without required extras, in manifest order"
    );
    let lists = state.addon_lists.as_ref().unwrap();
    assert_eq!(lists[0].target.name, "Official");
    assert_eq!(lists[0].target.addon_name, "Lists Fixture");
    assert!(lists.iter().all(|l| l.addons.is_loading()));

    let addons = parse_addon_catalog_response(&fixture("quirks/addon-catalog-mixed.json"))
        .unwrap()
        .value;
    update(
        &mut state,
        Action::AddonListLoaded {
            addon: url(LISTS),
            path: addon_list_path("official"),
            result: Ok(addons.clone()),
        },
    );
    update(
        &mut state,
        Action::AddonListLoaded {
            addon: url(LISTS),
            path: addon_list_path("community"),
            result: Err("HTTP 500".into()),
        },
    );
    let lists = state.addon_lists.as_ref().unwrap();
    assert_eq!(lists[0].addons, Loadable::Ready(addons));
    assert_eq!(
        lists[1].addons,
        Loadable::Failed(Problem::Request("HTTP 500".into()))
    );

    let effects = update(&mut state, Action::LoadAddonLists);
    assert_eq!(fetched_addon_lists(&effects), vec!["community"]);
    assert!(
        state.addon_lists.as_ref().unwrap()[0]
            .addons
            .ready()
            .is_some()
    );
}

#[test]
fn listed_addons_are_sorted_by_name() {
    let mut state = restore(&[LISTS]);
    update(&mut state, loaded(LISTS, "basic/manifest-addon-lists.json"));
    update(&mut state, Action::LoadAddonLists);
    let mut addons = parse_addon_catalog_response(&fixture("quirks/addon-catalog-mixed.json"))
        .unwrap()
        .value;
    addons.reverse();
    addons[1].manifest.name = "catalog Fixture".into();
    update(
        &mut state,
        Action::AddonListLoaded {
            addon: url(LISTS),
            path: addon_list_path("official"),
            result: Ok(addons),
        },
    );
    let names: Vec<_> = state.addon_lists.as_ref().unwrap()[0]
        .addons
        .ready()
        .unwrap()
        .iter()
        .map(|a| a.manifest.name.as_str())
        .collect();
    assert_eq!(names, vec!["catalog Fixture", "Subtitles Fixture"]);
}

#[test]
fn retry_asks_again_for_addon_lists_that_failed() {
    let mut state = restore(&[LISTS]);
    update(&mut state, loaded(LISTS, "basic/manifest-addon-lists.json"));
    update(&mut state, Action::LoadAddonLists);
    update(
        &mut state,
        Action::AddonListLoaded {
            addon: url(LISTS),
            path: addon_list_path("official"),
            result: Ok(Vec::new()),
        },
    );
    update(
        &mut state,
        Action::AddonListLoaded {
            addon: url(LISTS),
            path: addon_list_path("community"),
            result: Err("HTTP 500".into()),
        },
    );

    let effects = update(&mut state, Action::RetryFailed);

    assert_eq!(fetched_addon_lists(&effects), vec!["community"]);
}

#[test]
fn addon_lists_follow_the_installed_addons() {
    let mut state = restore(&[BASIC]);
    update(&mut state, Action::LoadAddonLists);
    assert_eq!(state.addon_lists, Some(Vec::new()));
    update(&mut state, Action::InstallAddon(LISTS.into()));
    let effects = update(
        &mut state,
        Action::ManifestLoaded {
            transport: url(LISTS),
            result: Ok(manifest("basic/manifest-addon-lists.json")),
            origin: ManifestOrigin::Install,
        },
    );
    assert_eq!(fetched_addon_lists(&effects), vec!["official", "community"]);
    update(&mut state, Action::RemoveAddon(url(LISTS)));
    assert_eq!(state.addon_lists, Some(Vec::new()));
    let stale = update(
        &mut state,
        Action::AddonListLoaded {
            addon: url(LISTS),
            path: addon_list_path("official"),
            result: Ok(Vec::new()),
        },
    );
    assert!(stale.is_empty());
    assert_eq!(
        state.addon_lists,
        Some(Vec::new()),
        "late results are dropped"
    );
}

#[test]
fn install_validates_and_rejects_duplicates() {
    let (mut state, _) = restored();
    assert!(
        update(
            &mut state,
            Action::InstallAddon("ftp://x/manifest.json".into())
        )
        .is_empty()
    );
    assert!(matches!(state.install, Some(Loadable::Failed(_))));

    assert!(update(&mut state, Action::InstallAddon(BASIC.into())).is_empty());
    assert_eq!(
        state.install,
        Some(Loadable::Failed(Problem::AlreadyInstalled))
    );

    let effects = update(
        &mut state,
        Action::InstallAddon("https://new.example/manifest.json".into()),
    );
    assert!(matches!(
        effects.as_slice(),
        [Effect::FetchManifest {
            origin: ManifestOrigin::Install,
            ..
        }]
    ));
    let effects = update(
        &mut state,
        Action::ManifestLoaded {
            transport: url("https://new.example/manifest.json"),
            result: Err("HTTP 404".into()),
            origin: ManifestOrigin::Install,
        },
    );
    assert!(effects.is_empty());
    assert_eq!(
        state.install,
        Some(Loadable::Failed(Problem::Request("HTTP 404".into())))
    );
    assert_eq!(state.addons.len(), 2, "failed install adds nothing");
}

#[test]
fn removing_an_addon_saves_and_reloads_the_board() {
    let (mut state, _) = restored();
    let effects = update(&mut state, Action::RemoveAddon(url(BASIC)));
    assert_eq!(effects[0], Effect::SaveAddons(vec![url(STREAMS)]));
    assert!(state.board.is_empty(), "the streams addon has no catalogs");
}

#[test]
fn catalog_results_fill_matching_rows_and_stale_ones_are_dropped() {
    let (mut state, _) = restored();
    let path = state.board[0].target.path.clone();
    update(
        &mut state,
        Action::CatalogLoaded {
            addon: url(BASIC),
            path: path.clone(),
            result: Ok(catalog_items()),
        },
    );
    assert_eq!(state.board[0].items.ready().map(Vec::len), Some(2));
    assert!(state.board[1].items.is_loading(), "other rows untouched");

    let before = state.clone();
    update(
        &mut state,
        Action::CatalogLoaded {
            addon: url(STREAMS),
            path,
            result: Ok(Vec::new()),
        },
    );
    assert_eq!(state, before, "a result nobody asked for changes nothing");
}

#[test]
fn retry_asks_again_only_for_the_rows_that_failed() {
    let (mut state, _) = restored();
    let failed = state.board[0].target.clone();
    let ready = state.board[1].target.clone();
    update(
        &mut state,
        Action::CatalogLoaded {
            addon: failed.addon.clone(),
            path: failed.path.clone(),
            result: Err("request timed out".into()),
        },
    );
    update(
        &mut state,
        Action::CatalogLoaded {
            addon: ready.addon,
            path: ready.path,
            result: Ok(catalog_items()),
        },
    );

    let effects = update(&mut state, Action::RetryFailed);

    assert_eq!(
        effects,
        vec![Effect::FetchCatalog {
            addon: failed.addon,
            path: failed.path,
        }]
    );
    assert!(state.board[0].items.is_loading());
    assert_eq!(state.board[1].items.ready().map(Vec::len), Some(2));
    assert!(
        update(&mut state, Action::RetryFailed).is_empty(),
        "a row already on its way is not asked for twice"
    );
}

#[test]
fn retry_asks_again_for_search_rows_that_failed() {
    let (mut state, _) = restored();
    let effects = update(&mut state, Action::Search("matrix".into()));
    let [Effect::FetchCatalog { addon, path }] = effects.as_slice() else {
        panic!("{effects:?}");
    };
    let (addon, path) = (addon.clone(), path.clone());
    update(
        &mut state,
        Action::CatalogLoaded {
            addon: addon.clone(),
            path: path.clone(),
            result: Err("request timed out".into()),
        },
    );
    let board: Vec<_> = state.board.iter().map(|r| r.target.clone()).collect();
    for target in board {
        update(
            &mut state,
            Action::CatalogLoaded {
                addon: target.addon,
                path: target.path,
                result: Ok(catalog_items()),
            },
        );
    }

    let effects = update(&mut state, Action::RetryFailed);

    assert_eq!(effects, vec![Effect::FetchCatalog { addon, path }]);
    assert!(state.search[0].items.is_loading());
}

#[test]
fn retry_asks_again_for_the_addons_whose_manifest_failed() {
    let mut state = restore(&[BASIC, STREAMS]);
    update(&mut state, load_failed(BASIC));

    let effects = update(&mut state, Action::RetryFailed);

    assert_eq!(
        effects,
        vec![Effect::FetchManifest {
            transport: url(BASIC),
            origin: ManifestOrigin::Retry,
        }],
        "the addon still loading is not asked for twice"
    );
    assert!(state.addons_loading.contains(&url(BASIC)));
    update(&mut state, retried(loaded(BASIC, "basic/manifest.json")));
    assert_eq!(state.addons.len(), 1);
}

#[test]
fn retry_asks_again_for_a_failed_detail_and_its_failed_streams() {
    let (mut state, _) = restored();
    let mut effects = ask_movie(&mut state);
    let first = effects.clone();
    while let [Effect::FetchMeta { addon, path }] = effects.as_slice() {
        effects = update(
            &mut state,
            Action::MetaLoaded {
                addon: addon.clone(),
                path: path.clone(),
                result: Err("request timed out".into()),
            },
        );
    }
    assert!(matches!(
        state.detail.as_ref().unwrap().meta,
        Loadable::Failed(Problem::Request(_))
    ));

    assert_eq!(update(&mut state, Action::RetryFailed), first);
    assert!(state.detail.as_ref().unwrap().meta.is_loading());
    assert!(
        update(&mut state, Action::RetryFailed).is_empty(),
        "a detail already on its way is not asked for twice"
    );

    let [Effect::FetchMeta { addon, path }] = first.as_slice() else {
        panic!("{first:?}");
    };
    let meta = movie_meta();
    let streams = update(
        &mut state,
        Action::MetaLoaded {
            addon: addon.clone(),
            path: path.clone(),
            result: Ok(Box::new(meta)),
        },
    );
    let [
        Effect::FetchStreams {
            addon: failed_addon,
            path: failed_path,
        },
        Effect::FetchStreams {
            addon: ready_addon,
            path: ready_path,
        },
    ] = streams.as_slice()
    else {
        panic!("{streams:?}");
    };
    update(
        &mut state,
        Action::StreamsLoaded {
            addon: failed_addon.clone(),
            path: failed_path.clone(),
            result: Err("request timed out".into()),
        },
    );
    update(
        &mut state,
        Action::StreamsLoaded {
            addon: ready_addon.clone(),
            path: ready_path.clone(),
            result: Ok(Vec::new()),
        },
    );

    assert_eq!(
        update(&mut state, Action::RetryFailed),
        vec![Effect::FetchStreams {
            addon: failed_addon.clone(),
            path: failed_path.clone(),
        }]
    );
}

#[test]
fn retry_leaves_a_detail_no_addon_can_answer() {
    let mut state = State::default();
    ask_movie(&mut state);

    assert!(update(&mut state, Action::RetryFailed).is_empty());
    assert!(matches!(
        state.detail.as_ref().unwrap().meta,
        Loadable::Failed(Problem::NoMetaAddon)
    ));
}

#[test]
fn an_automatic_retry_that_fails_again_raises_no_notice() {
    let mut state = restore(&[BASIC]);
    update(&mut state, load_failed(BASIC));
    assert!(state.notice.is_some(), "the first failure is reported");
    update(&mut state, Action::DismissNotice);

    update(&mut state, Action::RetryFailed);
    update(&mut state, retried(load_failed(BASIC)));

    assert_eq!(state.notice, None);
}

#[test]
fn a_recovered_addon_joins_the_streams_of_the_open_detail() {
    let mut state = restore(&[BASIC, STREAMS]);
    update(&mut state, loaded(BASIC, "basic/manifest.json"));
    update(&mut state, load_failed(STREAMS));
    let before = open_movie(&mut state);
    assert_eq!(before.len(), 1, "only the addon that loaded is asked");

    update(&mut state, Action::RetryFailed);
    let effects = update(
        &mut state,
        retried(loaded(STREAMS, "basic/manifest-streams.json")),
    );

    let asked: Vec<_> = effects
        .iter()
        .filter(|e| matches!(e, Effect::FetchStreams { .. }))
        .collect();
    assert_eq!(asked.len(), 1, "{effects:?}");
    assert!(matches!(asked[0], Effect::FetchStreams { addon, .. } if *addon == url(STREAMS)));
    assert_eq!(state.detail.as_ref().unwrap().streams.len(), 2);
}

#[test]
fn a_recovered_addon_joins_the_current_search() {
    let mut state = restore(&[BASIC, STREAMS]);
    update(&mut state, load_failed(BASIC));
    update(&mut state, loaded(STREAMS, "basic/manifest-streams.json"));
    assert!(update(&mut state, Action::Search("matrix".into())).is_empty());

    update(&mut state, Action::RetryFailed);
    let effects = update(&mut state, retried(loaded(BASIC, "basic/manifest.json")));

    assert_eq!(state.search.len(), 1);
    assert!(state.search[0].items.is_loading());
    assert!(effects.iter().any(
        |e| matches!(e, Effect::FetchCatalog { path, .. } if *path == state.search[0].target.path)
    ));
}

#[test]
fn a_detail_without_addons_is_asked_for_once_they_recover() {
    let mut state = restore(&[BASIC]);
    update(&mut state, load_failed(BASIC));
    ask_movie(&mut state);
    assert!(matches!(
        state.detail.as_ref().unwrap().meta,
        Loadable::Failed(Problem::NoMetaAddon)
    ));

    update(&mut state, Action::RetryFailed);
    let effects = update(&mut state, retried(loaded(BASIC, "basic/manifest.json")));

    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::FetchMeta { .. })),
        "{effects:?}"
    );
    assert!(state.detail.as_ref().unwrap().meta.is_loading());
}

#[test]
fn an_addon_restored_late_answers_a_detail_opened_before_it() {
    let mut state = restore(&[BASIC]);
    ask_movie(&mut state);

    let effects = update(&mut state, loaded(BASIC, "basic/manifest.json"));

    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::FetchMeta { .. })),
        "{effects:?}"
    );
    assert!(state.detail.as_ref().unwrap().meta.is_loading());
}

#[test]
fn an_addon_restored_late_joins_the_current_search() {
    let mut state = restore(&[BASIC, STREAMS]);
    update(&mut state, loaded(STREAMS, "basic/manifest-streams.json"));
    assert!(update(&mut state, Action::Search("matrix".into())).is_empty());

    let effects = update(&mut state, loaded(BASIC, "basic/manifest.json"));

    assert_eq!(state.search.len(), 1);
    assert!(effects.iter().any(
        |e| matches!(e, Effect::FetchCatalog { path, .. } if *path == state.search[0].target.path)
    ));
}

#[test]
fn a_removed_addon_leaves_the_current_search() {
    let (mut state, _) = restored();
    update(&mut state, Action::Search("matrix".into()));
    assert_eq!(state.search.len(), 1);

    update(&mut state, Action::RemoveAddon(url(BASIC)));

    assert!(state.search.is_empty());
}

#[test]
fn a_removed_addon_leaves_the_streams_of_the_open_detail() {
    let (mut state, _) = restored();
    open_movie(&mut state);
    assert_eq!(state.detail.as_ref().unwrap().streams.len(), 2);

    update(&mut state, Action::RemoveAddon(url(STREAMS)));

    let streams = &state.detail.as_ref().unwrap().streams;
    assert_eq!(streams.len(), 1);
    assert_eq!(streams[0].addon, url(BASIC));
}

#[test]
fn moving_an_addon_reorders_the_open_streams_without_asking_again() {
    let (mut state, _) = restored();
    open_movie(&mut state);

    let effects = update(&mut state, Action::MoveAddon { from: 0, to: 1 });

    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::FetchStreams { .. })),
        "{effects:?}"
    );
    assert_eq!(
        state.detail.as_ref().unwrap().streams[0].addon,
        url(STREAMS)
    );
}

#[test]
fn search_asks_only_catalogs_that_support_search() {
    let (mut state, _) = restored();
    let effects = update(&mut state, Action::Search("  matrix ".into()));
    assert_eq!(state.search_query, "matrix");
    let [Effect::FetchCatalog { path, .. }] = effects.as_slice() else {
        panic!("expected one catalog fetch, got {effects:?}");
    };
    assert_eq!(
        path.to_url_path(),
        "catalog/movie/search/search=matrix.json"
    );
}

#[test]
fn meta_falls_back_to_the_next_addon_then_movie_streams_load_from_all() {
    let (mut state, _) = restored();
    let effects = ask_movie(&mut state);
    let [Effect::FetchMeta { addon, path }] = effects.as_slice() else {
        panic!("{effects:?}");
    };
    assert_eq!(addon, &url(BASIC));
    let meta = movie_meta();
    let effects = update(
        &mut state,
        Action::MetaLoaded {
            addon: addon.clone(),
            path: path.clone(),
            result: Ok(Box::new(meta)),
        },
    );
    assert_eq!(effects.len(), 2);
    let detail = state.detail.as_ref().unwrap();
    assert_eq!(detail.selected_video.as_deref(), Some("tt0000001"));

    let groups: Vec<(TransportUrl, ResourcePath)> = detail
        .streams
        .iter()
        .map(|g| (g.addon.clone(), g.path.clone()))
        .collect();
    update(
        &mut state,
        Action::StreamsLoaded {
            addon: groups[0].0.clone(),
            path: groups[0].1.clone(),
            result: Err("timed out".into()),
        },
    );
    let streams = parse_stream_response(&fixture("basic/streams-movie.json"))
        .unwrap()
        .value;
    update(
        &mut state,
        Action::StreamsLoaded {
            addon: groups[1].0.clone(),
            path: groups[1].1.clone(),
            result: Ok(streams),
        },
    );
    let detail = state.detail.as_ref().unwrap();
    assert_eq!(
        detail.streams[0].streams,
        Loadable::Failed(Problem::Request("timed out".into()))
    );
    assert_eq!(detail.streams[1].streams.ready().map(Vec::len), Some(6));
}

#[test]
fn meta_failure_tries_the_next_candidate() {
    let mut state = State::default();
    update(
        &mut state,
        Action::Restore {
            addons: vec![url(BASIC), url("https://basic2.example/manifest.json")],
            library: Vec::new(),
        },
    );
    for t in [BASIC, "https://basic2.example/manifest.json"] {
        update(
            &mut state,
            Action::ManifestLoaded {
                transport: url(t),
                result: Ok(manifest("basic/manifest.json")),
                origin: ManifestOrigin::Restore,
            },
        );
    }
    let effects = update(
        &mut state,
        Action::OpenDetail {
            content_type: ty("series"),
            id: "tt0000010".into(),
            preview: None,
        },
    );
    let [Effect::FetchMeta { addon, path }] = effects.as_slice() else {
        panic!()
    };
    let effects = update(
        &mut state,
        Action::MetaLoaded {
            addon: addon.clone(),
            path: path.clone(),
            result: Err("HTTP 500".into()),
        },
    );
    let [Effect::FetchMeta { addon: next, .. }] = effects.as_slice() else {
        panic!("{effects:?}")
    };
    assert_eq!(next, &url("https://basic2.example/manifest.json"));
    assert!(state.detail.as_ref().unwrap().meta.is_loading());
}

#[test]
fn discover_pages_with_skip_and_deduplicates() {
    let (mut state, _) = restored();
    let target = state.board[0].target.clone();
    let effects = update(
        &mut state,
        Action::OpenDiscover {
            addon: target.addon.clone(),
            path: target.path.clone(),
        },
    );
    let [Effect::FetchCatalog { path, .. }] = effects.as_slice() else {
        panic!()
    };
    update(
        &mut state,
        Action::CatalogLoaded {
            addon: target.addon.clone(),
            path: path.clone(),
            result: Ok(catalog_items()),
        },
    );
    assert_eq!(state.discover.items.len(), 2);
    assert_eq!(state.discover.next_skip, Some(2));

    let effects = update(&mut state, Action::LoadMoreDiscover);
    let [Effect::FetchCatalog { path, .. }] = effects.as_slice() else {
        panic!()
    };
    assert_eq!(path.to_url_path(), "catalog/movie/top/skip=2.json");
    let mut page = catalog_items();
    page.truncate(1);
    update(
        &mut state,
        Action::CatalogLoaded {
            addon: target.addon,
            path: path.clone(),
            result: Ok(page),
        },
    );
    assert_eq!(state.discover.items.len(), 2, "duplicates dropped");

    let effects = update(&mut state, Action::SetDiscoverGenre(Some("Drama".into())));
    let [Effect::FetchCatalog { path, .. }] = effects.as_slice() else {
        panic!()
    };
    assert_eq!(path.to_url_path(), "catalog/movie/top/genre=Drama.json");
    assert!(state.discover.items.is_empty());
}

#[test]
fn an_addon_that_needs_configuration_is_configured_not_installed() {
    let configured = url("https://addon.example/abc%20def/manifest.json?lang=es");
    assert_eq!(
        configured.configure_url().as_str(),
        "https://addon.example/abc%20def/configure?lang=es"
    );
    let manifest = |required: bool| {
        let json = format!(
            r#"{{"id":"org.example","version":"1.0.0","name":"Example","resources":["stream"],"types":["movie"],"catalogs":[],"behaviorHints":{{"configurable":true,"configurationRequired":{required}}}}}"#
        );
        Box::new(parse_manifest(json.as_bytes()).unwrap().value)
    };
    let mut state = State::default();
    let transport = url("https://addon.example/manifest.json");
    update(&mut state, Action::InstallAddon(transport.to_string()));
    let effects = update(
        &mut state,
        Action::ManifestLoaded {
            transport: transport.clone(),
            result: Ok(manifest(true)),
            origin: ManifestOrigin::Install,
        },
    );
    assert!(effects.is_empty());
    assert!(state.addons.is_empty() && state.installed.is_empty());
    assert_eq!(
        state.install,
        Some(Loadable::Failed(Problem::ConfigurationRequired(
            transport.clone()
        )))
    );

    update(&mut state, Action::InstallAddon(transport.to_string()));
    update(
        &mut state,
        Action::ManifestLoaded {
            transport: transport.clone(),
            result: Ok(manifest(false)),
            origin: ManifestOrigin::Install,
        },
    );
    assert_eq!(state.installed, vec![transport]);
}
