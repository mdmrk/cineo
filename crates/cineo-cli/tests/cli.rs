//! End-to-end: the `cineo` binary against a local mock addon.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers panic on purpose"
)]

use std::process::Command;

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn fixture(rel: &str) -> Vec<u8> {
    let full = format!("{}/../../tests/{rel}", env!("CARGO_MANIFEST_DIR"));
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

fn data_dir(name: &str) -> String {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("cli")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    dir.to_str().unwrap().to_owned()
}

fn stdout_of(out: &std::process::Output) -> String {
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout.clone()).unwrap()
}

#[tokio::test]
async fn addons_are_added_listed_and_removed_across_runs() {
    let server = mock_addon().await;
    let manifest = format!("{}/manifest.json", server.uri());
    let dir = data_dir("addons");
    let run = |args: &[&str]| {
        let mut all = vec!["--allow-private-network", "--data-dir", &dir];
        all.extend_from_slice(args);
        cineo(&all)
    };

    let added = stdout_of(&run(&["addon", "add", &manifest]));
    assert_eq!(
        added,
        "installed Basic Fixture 1.2.0 (org.cineo.fixture.basic)\n"
    );
    assert_eq!(
        stdout_of(&run(&["addon", "list"])),
        format!("1\t{manifest}\n")
    );

    let again = run(&["addon", "add", &manifest]);
    assert!(!again.status.success());
    let stderr = String::from_utf8(again.stderr).unwrap();
    assert!(stderr.contains("already installed"), "{stderr}");
    assert_eq!(
        server.received_requests().await.unwrap().len(),
        1,
        "a duplicate is refused before any request"
    );

    assert_eq!(
        stdout_of(&run(&["addon", "remove", &manifest])),
        "removed\n"
    );
    assert_eq!(stdout_of(&run(&["addon", "list"])), "");
}

#[tokio::test]
async fn an_addon_with_an_invalid_manifest_is_not_installed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/manifest.json"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            fixture("addons/invalid/manifest-empty-id.json"),
            "application/json",
        ))
        .mount(&server)
        .await;
    let manifest = format!("{}/manifest.json", server.uri());
    let dir = data_dir("invalid");
    let base = ["--allow-private-network", "--data-dir", &dir];

    let out = cineo(&[&base[..], &["addon", "add", &manifest]].concat());
    assert!(!out.status.success());
    assert_eq!(
        stdout_of(&cineo(&[&base[..], &["addon", "list"]].concat())),
        ""
    );
}

#[test]
fn doctor_reports_on_the_database() {
    let dir = data_dir("doctor");
    let missing = stdout_of(&cineo(&["--data-dir", &dir, "doctor"]));
    assert!(
        missing.ends_with("status: not created yet (created on first use)\n"),
        "{missing}"
    );

    assert_eq!(stdout_of(&cineo(&["--data-dir", &dir, "library"])), "");
    let healthy = stdout_of(&cineo(&["--data-dir", &dir, "doctor"]));
    assert!(
        healthy.contains(
            "schema version: 4 (supported: 4)\naddons: 0\nlibrary items: 0\nintegrity: ok\n"
        ),
        "{healthy}"
    );

    std::fs::write(format!("{dir}/cineo.db"), vec![b'x'; 4096]).unwrap();
    let corrupt = cineo(&["--data-dir", &dir, "doctor"]);
    assert!(!corrupt.status.success());
    let out = String::from_utf8(corrupt.stdout).unwrap();
    assert!(out.contains("error: "), "{out}");
    let library = cineo(&["--data-dir", &dir, "library"]);
    let stderr = String::from_utf8(library.stderr).unwrap();
    assert!(
        stderr.contains("corrupt or not a SQLite database"),
        "{stderr}"
    );
}
