//! `AddonClient` against a local mock addon. The mock listens on loopback, so
//! most tests opt into `allow_private_networks`; one test proves the default
//! policy refuses it.

// Test helpers panic on purpose: a panic is a failed assertion.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use cineo_core::addon::{ContentType, ExtraValue, ResourcePath, TransportUrl};
use cineo_net::{AddonClient, BlockReason, FetchError, NetPolicy};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn fixture(rel: &str) -> Vec<u8> {
    let full = format!("{}/../../tests/fixtures/{rel}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&full).unwrap_or_else(|err| panic!("reading {full}: {err}"))
}

fn local_policy() -> NetPolicy {
    NetPolicy {
        allow_private_networks: true,
        ..NetPolicy::default()
    }
}

fn transport(server: &MockServer) -> TransportUrl {
    TransportUrl::parse(&format!("{}/cfg/manifest.json", server.uri())).unwrap()
}

async fn serve(server: &MockServer, at: &str, body: Vec<u8>) {
    Mock::given(method("GET"))
        .and(path(at))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(server)
        .await;
}

#[tokio::test]
async fn fetches_manifest_and_catalog_page() {
    let server = MockServer::start().await;
    serve(
        &server,
        "/cfg/manifest.json",
        fixture("addons/basic/manifest.json"),
    )
    .await;
    serve(
        &server,
        "/cfg/catalog/movie/top/genre=Action&skip=100.json",
        fixture("addons/basic/catalog-movie-top.json"),
    )
    .await;

    let client = AddonClient::new(local_policy()).unwrap();
    let addon = transport(&server);
    let manifest = client.fetch_manifest(&addon).await.unwrap().value;
    assert_eq!(manifest.id, "org.cineo.fixture.basic");

    let page = ResourcePath::catalog(ContentType::new("movie").unwrap(), "top").with_extra(vec![
        ExtraValue::new("genre", "Action"),
        ExtraValue::new("skip", "100"),
    ]);
    assert!(manifest.supports(&page));
    let catalog = client.fetch_catalog(&addon, &page).await.unwrap().value;
    assert_eq!(catalog.metas.len(), 2);
}

#[tokio::test]
async fn default_policy_blocks_loopback_addons() {
    let server = MockServer::start().await;
    serve(
        &server,
        "/cfg/manifest.json",
        fixture("addons/basic/manifest.json"),
    )
    .await;

    let client = AddonClient::new(NetPolicy::default()).unwrap();
    let err = client
        .fetch_manifest(&transport(&server))
        .await
        .unwrap_err();
    assert!(
        matches!(err, FetchError::Blocked(BlockReason::NonPublicAddress(_))),
        "got {err:?}"
    );
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn non_success_status_is_an_error() {
    let server = MockServer::start().await;
    let client = AddonClient::new(local_policy()).unwrap();
    let err = client
        .fetch_manifest(&transport(&server))
        .await
        .unwrap_err();
    assert!(
        matches!(err, FetchError::Status(s) if s.as_u16() == 404),
        "got {err:?}"
    );
}

#[tokio::test]
async fn oversized_body_is_rejected() {
    let server = MockServer::start().await;
    serve(&server, "/cfg/manifest.json", vec![b' '; 4096]).await;
    let client = AddonClient::new(NetPolicy {
        max_body_bytes: 1024,
        ..local_policy()
    })
    .unwrap();
    let err = client
        .fetch_manifest(&transport(&server))
        .await
        .unwrap_err();
    assert!(
        matches!(err, FetchError::TooLarge { limit: 1024 }),
        "got {err:?}"
    );
}

#[tokio::test]
async fn limit_applies_to_decompressed_bytes() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cfg/manifest.json"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-encoding", "gzip")
                .set_body_raw(fixture("net/zeros-1mib.json.gz"), "application/json"),
        )
        .mount(&server)
        .await;
    let client = AddonClient::new(NetPolicy {
        max_body_bytes: 64 * 1024,
        ..local_policy()
    })
    .unwrap();
    let err = client
        .fetch_manifest(&transport(&server))
        .await
        .unwrap_err();
    assert!(matches!(err, FetchError::TooLarge { .. }), "got {err:?}");
}

#[tokio::test]
async fn slow_addon_times_out() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(5)))
        .mount(&server)
        .await;
    let client = AddonClient::new(NetPolicy {
        timeout: Duration::from_millis(200),
        ..local_policy()
    })
    .unwrap();
    let err = client
        .fetch_manifest(&transport(&server))
        .await
        .unwrap_err();
    assert!(matches!(err, FetchError::Timeout), "got {err:?}");
}

#[tokio::test]
async fn redirect_loops_are_cut_off() {
    let server = MockServer::start().await;
    let target = format!("{}/cfg/manifest.json", server.uri());
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(302).insert_header("location", target.as_str()))
        .mount(&server)
        .await;
    let client = AddonClient::new(NetPolicy {
        max_redirects: 3,
        ..local_policy()
    })
    .unwrap();
    let err = client
        .fetch_manifest(&transport(&server))
        .await
        .unwrap_err();
    assert!(
        matches!(err, FetchError::Blocked(BlockReason::TooManyRedirects(3))),
        "got {err:?}"
    );
    assert_eq!(server.received_requests().await.unwrap().len(), 4);
}

#[tokio::test]
async fn errors_do_not_leak_addon_configuration() {
    let client = AddonClient::new(local_policy()).unwrap();
    let secret = TransportUrl::parse("http://127.0.0.1:1/SECRET-TOKEN/manifest.json").unwrap();
    let err = client.fetch_manifest(&secret).await.unwrap_err();
    assert!(matches!(err, FetchError::Connect(_)), "got {err:?}");
    assert!(!err.to_string().contains("SECRET-TOKEN"), "{err}");
}

#[tokio::test]
async fn fetches_meta_and_streams() {
    let server = MockServer::start().await;
    serve(
        &server,
        "/cfg/meta/series/tt0000010.json",
        fixture("addons/basic/meta-series.json"),
    )
    .await;
    serve(
        &server,
        "/cfg/stream/series/tt0000010%3A1%3A1.json",
        fixture("addons/basic/streams-movie.json"),
    )
    .await;
    let client = AddonClient::new(local_policy()).unwrap();
    let addon = transport(&server);
    let series = ContentType::new("series").unwrap();
    let meta_path = ResourcePath {
        resource: cineo_core::addon::ResourceName::Meta,
        content_type: series.clone(),
        id: "tt0000010".into(),
        extra: Vec::new(),
    };
    let meta = client.fetch_meta(&addon, &meta_path).await.unwrap().value;
    assert_eq!(meta.videos.len(), 4);
    let stream_path = ResourcePath {
        resource: cineo_core::addon::ResourceName::Stream,
        content_type: series,
        id: "tt0000010:1:1".into(),
        extra: Vec::new(),
    };
    let streams = client
        .fetch_streams(&addon, &stream_path)
        .await
        .unwrap()
        .value;
    assert_eq!(streams.len(), 6);
}

#[tokio::test]
async fn images_use_their_own_size_limit() {
    let server = MockServer::start().await;
    serve(&server, "/poster.jpg", vec![0_u8; 2048]).await;
    let client = AddonClient::new(local_policy()).unwrap();
    let url = url::Url::parse(&format!("{}/poster.jpg", server.uri())).unwrap();
    assert_eq!(client.fetch_image(&url, 4096).await.unwrap().len(), 2048);
    let err = client.fetch_image(&url, 1024).await.unwrap_err();
    assert!(
        matches!(err, FetchError::TooLarge { limit: 1024 }),
        "got {err:?}"
    );

    let blocked = AddonClient::new(NetPolicy::default()).unwrap();
    assert!(matches!(
        blocked.fetch_image(&url, 4096).await,
        Err(FetchError::Blocked(_))
    ));
}
