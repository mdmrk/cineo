use super::*;

fn half_watched(id: &str, name: &str, favorited: Option<u64>) -> LibraryItem {
    LibraryItem {
        id: id.into(),
        content_type: ContentType::new("movie").unwrap(),
        name: name.into(),
        poster: None,
        video_id: id.into(),
        time_offset_ms: 60_000,
        duration_ms: 600_000,
        updated_ms: 1,
        favorited,
        stream: None,
    }
}

#[test]
fn a_watched_favourite_is_also_in_continue_watching_and_recently_played() {
    let mut state = board_state();
    state.library.push(half_watched("a", "Liked", Some(1)));
    let mut harness = harness(state, ViewState::default());
    harness.get_by_label("Continue watching");
    harness.get_by_label("Liked");
    harness.state_mut().1.page = Page::Library;
    harness.run();
    assert_eq!(
        harness.get_all_by_label("Liked").count(),
        2,
        "Favourites and Recently played"
    );
}

#[test]
fn a_continue_watching_card_resumes_favorites_or_removes_the_item() {
    let mut state = board_state();
    state
        .library
        .push(half_watched("tt0000001", "Half Watched", None));
    let mut harness = harness(state, ViewState::default());
    harness.get_by_label("Continue watching");
    for label in [
        "Half Watched",
        "Add to favourites",
        "Remove from continue watching",
    ] {
        harness.get_by_label(label).click();
        harness.run();
    }
    assert_eq!(
        harness.state().2,
        vec![
            Action::Resume("tt0000001".into()),
            Action::SetFavorite {
                id: "tt0000001".into(),
                favorite: true,
            },
            Action::RemoveFromLibrary("tt0000001".into()),
        ]
    );
}

#[test]
fn the_library_lists_newest_favorites_first_and_unfavorites_them() {
    let state = State {
        library: vec![
            half_watched("a", "Liked", Some(1)),
            half_watched("b", "Plain", None),
            half_watched("c", "Loved", Some(2)),
        ],
        ..State::default()
    };
    let view = ViewState {
        page: Page::Library,
        ..ViewState::default()
    };
    let mut harness = harness(state, view);
    let first =
        |h: &Harness<'_, Ui>, label: &str| h.get_all_by_label(label).next().expect(label).rect();
    let top = |h: &Harness<'_, Ui>, label: &str| first(h, label).top();
    let left = |h: &Harness<'_, Ui>, label: &str| first(h, label).left();
    assert!(top(&harness, "Favourites") < top(&harness, "Liked"));
    assert!(
        left(&harness, "Loved") < left(&harness, "Liked"),
        "newest first"
    );
    assert!(top(&harness, "Liked") < top(&harness, "Recently played"));
    assert!(top(&harness, "Recently played") < top(&harness, "Plain"));
    harness
        .get_all_by_label("Remove from favourites")
        .nth(1)
        .unwrap()
        .click();
    harness.run();
    assert_eq!(
        harness.state().2,
        vec![Action::SetFavorite {
            id: "a".into(),
            favorite: false,
        }]
    );
    assert_eq!(
        harness.get_all_by_label("Remove from library").count(),
        3,
        "only recently played cards can be removed, not favourites"
    );
}

#[test]
fn favourites_open_their_page_and_recently_played_resumes() {
    let state = State {
        library: vec![
            half_watched("a", "Liked", Some(1)),
            half_watched("b", "Plain", None),
        ],
        ..State::default()
    };
    let view = ViewState {
        page: Page::Library,
        ..ViewState::default()
    };
    let mut harness = harness(state, view);
    harness.get_all_by_label("Liked").next().unwrap().click();
    harness.run();
    harness.get_all_by_label("Liked").nth(1).unwrap().click();
    harness.run();
    harness.get_by_label("Plain").click();
    harness.run();
    let actions = &harness.state().2;
    assert!(
        matches!(&actions[0], Action::OpenDetail { id, preview: Some(p), .. } if id == "a" && p.name == "Liked"),
        "{actions:?}"
    );
    assert_eq!(
        actions[1..],
        [Action::Resume("a".into()), Action::Resume("b".into())]
    );
}

#[test]
fn a_favourite_never_played_has_no_remove_button() {
    let mut unplayed = half_watched("a", "Liked", Some(1));
    unplayed.time_offset_ms = 0;
    unplayed.updated_ms = 0;
    let state = State {
        library: vec![unplayed],
        ..State::default()
    };
    let view = ViewState {
        page: Page::Library,
        ..ViewState::default()
    };
    let harness = harness(state, view);
    harness.get_by_label("Remove from favourites");
    assert!(harness.query_by_label("Remove from library").is_none());
}

#[test]
fn the_detail_page_adds_and_removes_a_favourite() {
    let mut harness = harness(detail_state(), ViewState::default());
    harness.get_by_label("Add to favourites").click();
    harness.run();
    let favorite = harness.state().2.last().cloned().unwrap();
    assert_eq!(
        favorite,
        Action::SetFavorite {
            id: "tt0000001".into(),
            favorite: true,
        }
    );
    let state = &mut harness.state_mut().0;
    update(state, favorite);
    harness.run();
    harness.get_by_label("Remove from favourites").click();
    harness.run();
    assert_eq!(
        harness.state().2.last(),
        Some(&Action::SetFavorite {
            id: "tt0000001".into(),
            favorite: false,
        })
    );
}
