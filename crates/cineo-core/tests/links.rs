#![allow(clippy::unwrap_used, reason = "test helpers panic on purpose")]

use cineo_core::addon::{ContentType, ResourcePath, TransportUrl};
use cineo_core::app::{Action, DiscoverLink, Effect, LinkError, Route, State, parse_link, update};

fn addon(url: &str) -> TransportUrl {
    TransportUrl::parse(url).unwrap()
}

#[test]
fn links_parse_into_routes_and_anything_else_is_rejected() {
    let detail = |id: &str, video: Option<&str>| Route::Detail {
        content_type: ContentType::new("series").unwrap(),
        id: id.into(),
        video_id: video.map(Into::into),
    };
    let cases = [
        (
            "stremio://watchhub-us.strem.io/manifest.json",
            Ok(Route::InstallAddon(addon(
                "https://watchhub-us.strem.io/manifest.json",
            ))),
        ),
        ("STREMIO:///board", Ok(Route::Board)),
        ("cineo:///library/", Ok(Route::Library)),
        (
            "stremio:///search?search=the%20office",
            Ok(Route::Search("the office".into())),
        ),
        (
            "stremio:///discover/https%3A%2F%2Fv3-cinemeta.strem.io%2Fmanifest.json/movie/top?genre=Drama",
            Ok(Route::Discover(Some(DiscoverLink {
                addon: addon("https://v3-cinemeta.strem.io/manifest.json"),
                path: ResourcePath::catalog(ContentType::new("movie").unwrap(), "top"),
                genre: Some("Drama".into()),
            }))),
        ),
        (
            "stremio:///detail/series/tt0108778",
            Ok(detail("tt0108778", None)),
        ),
        (
            "stremio:///detail/series/tt0108778/tt0108778:1:1?autoPlay=true",
            Ok(detail("tt0108778", Some("tt0108778:1:1"))),
        ),
        ("https://example.com/manifest.json", Err(LinkError::Scheme)),
        ("stremio://example.com/addon.json", Err(LinkError::Unknown)),
        (
            "stremio://user:pw@example.com/manifest.json",
            Err(LinkError::Unknown),
        ),
        ("stremio:///settings", Err(LinkError::Unknown)),
        ("stremio:///detail/series", Err(LinkError::Unknown)),
        (
            "stremio:///discover/not-a-url/movie/top",
            Err(LinkError::Unknown),
        ),
    ];
    for (link, expected) in cases {
        assert_eq!(parse_link(link), expected, "{link}");
    }
}

#[test]
fn an_install_link_installs_nothing_until_accepted() {
    let mut state = State::default();
    let route = parse_link("stremio://addon.example/manifest.json").unwrap();
    assert!(update(&mut state, Action::OpenLink(route.clone())).is_empty());
    assert_eq!(
        state.link_prompt,
        Some(addon("https://addon.example/manifest.json"))
    );
    assert!(update(&mut state, Action::DeclineLink).is_empty());
    assert_eq!(state.link_prompt, None);

    update(&mut state, Action::OpenLink(route));
    let effects = update(&mut state, Action::AcceptLink);
    assert_eq!(
        effects,
        vec![Effect::FetchManifest {
            transport: addon("https://addon.example/manifest.json"),
            install: true,
        }]
    );
}
