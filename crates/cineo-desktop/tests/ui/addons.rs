use super::*;

#[test]
fn addons_page_installs_from_the_typed_url_and_lists_installed_addons() {
    let view = ViewState {
        page: Page::Addons,
        ..ViewState::default()
    };
    let mut harness = harness(board_state(), view);
    harness.get_by_label_contains("Basic Fixture 1.2.0");
    harness.get_by_label("basic.example");
    harness
        .get_by_role(eframe::egui::accesskit::Role::TextInput)
        .focus();
    harness.run();
    harness
        .get_by_role(eframe::egui::accesskit::Role::TextInput)
        .type_text("https://new.example/manifest.json");
    harness.run();
    harness.get_by_label("Install").click();
    harness.run();
    assert_eq!(
        harness.state().2,
        vec![Action::InstallAddon(
            "https://new.example/manifest.json".into()
        )]
    );
}

#[test]
fn addons_that_failed_to_load_can_be_retried_or_removed() {
    const DOWN: &str = "https://down.example/manifest.json";
    let mut state = State::default();
    let down = TransportUrl::parse(DOWN).unwrap();
    update(
        &mut state,
        Action::Restore {
            addons: vec![down.clone()],
            library: Vec::new(),
        },
    );
    update(
        &mut state,
        Action::ManifestLoaded {
            transport: down.clone(),
            result: Err("HTTP 503".into()),
            origin: ManifestOrigin::Restore,
        },
    );
    let view = ViewState {
        page: Page::Addons,
        ..ViewState::default()
    };
    let mut harness = harness(state, view);
    harness.get_by_label("down.example");
    harness.get_by_label("Retry").click();
    harness.run();
    harness.get_by_label("Remove").click();
    harness.run();
    assert_eq!(
        harness.state().2,
        vec![Action::InstallAddon(DOWN.into()), Action::RemoveAddon(down)]
    );
}

#[test]
fn addon_lists_install_listed_addons_and_mark_installed_ones() {
    let lists = TransportUrl::parse("https://lists.example/manifest.json").unwrap();
    let subs = TransportUrl::parse("https://subs.example.org/manifest.json").unwrap();
    let mut state = State::default();
    update(
        &mut state,
        Action::Restore {
            addons: vec![lists.clone(), subs.clone()],
            library: Vec::new(),
        },
    );
    update(
        &mut state,
        Action::ManifestLoaded {
            transport: lists.clone(),
            result: Ok(Box::new(
                parse_manifest(&fixture("basic/manifest-addon-lists.json"))
                    .unwrap()
                    .value,
            )),
            origin: ManifestOrigin::Restore,
        },
    );
    update(
        &mut state,
        Action::ManifestLoaded {
            transport: subs,
            result: Err("HTTP 503".into()),
            origin: ManifestOrigin::Restore,
        },
    );
    update(&mut state, Action::LoadAddonLists);
    let path = |id: &str| ResourcePath {
        resource: ResourceName::AddonCatalog,
        ..ResourcePath::catalog(ContentType::new("all").unwrap(), id)
    };
    let addons = parse_addon_catalog_response(&fixture("quirks/addon-catalog-mixed.json"))
        .unwrap()
        .value;
    update(
        &mut state,
        Action::AddonListLoaded {
            addon: lists.clone(),
            path: path("official"),
            result: Ok(addons),
        },
    );
    update(
        &mut state,
        Action::AddonListLoaded {
            addon: lists,
            path: path("community"),
            result: Err("HTTP 500".into()),
        },
    );
    let view = ViewState {
        page: Page::Addons,
        ..ViewState::default()
    };
    let mut harness = harness(state, view);
    harness.get_by_label_contains("made by third parties");
    harness.get_by_label_contains("Catalog Fixture 1.0.0");
    harness.get_by_label_contains("Subtitles Fixture 0.3.0");
    assert_eq!(
        harness.get_all_by_label("Install").count(),
        2,
        "the URL field and the one addon not installed yet"
    );
    harness.get_all_by_label("Install").last().unwrap().click();
    harness.run();
    harness.get_by_label("Community").click();
    harness.run();
    harness.get_by_label_contains("HTTP 500");
    assert!(harness.query_by_label_contains("Catalog Fixture").is_none());
    harness.get_all_by_label("Retry").last().unwrap().click();
    harness.run();
    assert_eq!(
        harness.state().2,
        vec![
            Action::InstallAddon("https://catalog.example.org/manifest.json".into()),
            Action::LoadAddonLists,
        ]
    );
}

#[test]
fn opening_the_addons_page_loads_the_addon_lists() {
    let mut harness = harness(State::default(), ViewState::default());
    harness.get_by_label("Addons").click();
    harness.run();
    assert_eq!(harness.state().1.page, Page::Addons);
    assert_eq!(harness.state().2, vec![Action::LoadAddonLists]);
}
