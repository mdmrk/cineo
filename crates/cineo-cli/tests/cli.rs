//! End-to-end: the `cineo` binary against a local mock addon.

// Test helpers panic on purpose: a panic is a failed assertion.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::process::Command;

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn fixture(rel: &str) -> Vec<u8> {
    let full = format!("{}/../../tests/fixtures/{rel}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&full).unwrap_or_else(|err| panic!("reading {full}: {err}"))
}

async fn mock_addon() -> MockServer {
    let server = MockServer::start().await;
    for (at, file) in [
        ("/manifest.json", "addons/basic/manifest.json"),
        (
            "/catalog/movie/top.json",
            "addons/basic/catalog-movie-top.json",
        ),
    ] {
        Mock::given(method("GET"))
            .and(path(at))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(fixture(file), "application/json"),
            )
            .mount(&server)
            .await;
    }
    server
}

fn cineo(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_cineo"))
        .args(args)
        .env_remove("RUST_LOG")
        .output()
        .unwrap()
}

#[tokio::test]
async fn catalog_command_lists_items() {
    let server = mock_addon().await;
    let manifest = format!("{}/manifest.json", server.uri());
    let out = cineo(&[
        "--allow-private-network",
        "catalog",
        &manifest,
        "movie",
        "top",
    ]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(
        stdout,
        "tt0000001\tmovie\t1999\tFirst Example Film\ntt0000002\tmovie\t-\tSecond Example Film\n"
    );
}

#[tokio::test]
async fn inspect_command_summarizes_manifest() {
    let server = mock_addon().await;
    let manifest = format!("{}/manifest.json", server.uri());
    let out = cineo(&["--allow-private-network", "addon", "inspect", &manifest]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.starts_with("Basic Fixture 1.2.0 (org.cineo.fixture.basic)\n"),
        "{stdout}"
    );
    assert!(
        stdout.contains("  movie/search \"Search\" extra: [search*]\n"),
        "{stdout}"
    );
}

#[tokio::test]
async fn undeclared_catalog_is_refused_before_any_request() {
    let server = mock_addon().await;
    let manifest = format!("{}/manifest.json", server.uri());
    let out = cineo(&[
        "--allow-private-network",
        "catalog",
        &manifest,
        "movie",
        "nope",
    ]);
    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("does not declare catalog movie/nope"),
        "{stderr}"
    );
    assert_eq!(
        server.received_requests().await.unwrap().len(),
        1,
        "only the manifest"
    );
}

#[test]
fn private_network_is_refused_by_default() {
    let out = cineo(&["addon", "inspect", "http://127.0.0.1:9/manifest.json"]);
    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("blocked by network policy"), "{stderr}");
}
