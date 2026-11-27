#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers panic on purpose"
)]

use cineo_core::addon::ContentType;
use cineo_core::app::LibraryItem;
use cineo_core::sync::{DeviceId, Outgoing, Pairing, Record, RecordName, Stamp, SyncKey, Value};
use cineo_net::sync::{
    CodeError, MAX_BATCH_RECORDS, MAX_PULL_BYTES, SpaceKeys, SyncClient, SyncError, format_code,
    parse_code,
};
use cineo_net::{AddonClient, BlockReason, FetchError, NetPolicy};
use data_encoding::BASE64URL_NOPAD;
use serde_json::{Value as Json, json};
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

const REFERENCE_KEY: [u8; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
const REFERENCE_CODE: &str = "AAAQ-EAYE-AUDA-OCAJ-BIFQ-YDIO-B4KA-47NG";
const REFERENCE_TOKEN: &str = "2aPNnOEGvveIUycd2wY60NlTE3pqP7G_Psu-7o846J8";

fn key() -> SyncKey {
    SyncKey::from_bytes(REFERENCE_KEY)
}

fn local_policy() -> NetPolicy {
    NetPolicy {
        allow_private_networks: true,
        ..NetPolicy::default()
    }
}

fn pairing(server: &str) -> Pairing {
    Pairing {
        key: key(),
        server: server.parse().unwrap(),
    }
}

fn client(server: &MockServer) -> SyncClient {
    SyncClient::new(
        AddonClient::new(local_policy()).unwrap(),
        &pairing(&format!("{}/base", server.uri())),
    )
    .unwrap()
}

fn item_record(id: &str, name: &str) -> Record {
    Record {
        name: RecordName::Item(id.to_owned()),
        stamp: Stamp {
            physical_ms: 1_000,
            counter: 0,
            device: DeviceId(7),
        },
        value: Value::Item(Box::new(LibraryItem {
            id: id.to_owned(),
            content_type: ContentType::new("movie").unwrap(),
            name: name.to_owned(),
            poster: None,
            video_id: id.to_owned(),
            time_offset_ms: 5,
            duration_ms: 10,
            updated_ms: 1,
            favorited: None,
            stream: None,
        })),
    }
}

fn outgoing(record: Record, base_seq: u64) -> Outgoing {
    Outgoing { record, base_seq }
}

fn sealed_entry(keys: &SpaceKeys, record: &Record, seq: u64) -> Json {
    let sealed = keys.seal(record).unwrap();
    json!({
        "key": BASE64URL_NOPAD.encode(&sealed.key),
        "seq": seq,
        "blob": BASE64URL_NOPAD.encode(&sealed.blob),
    })
}

#[test]
fn the_code_matches_the_reference_vector() {
    assert_eq!(format_code(&key()), REFERENCE_CODE);
}

#[test]
fn the_code_reads_back_whatever_the_case_dashes_spaces_and_lookalikes() {
    for typed in [
        REFERENCE_CODE.to_owned(),
        REFERENCE_CODE.to_lowercase(),
        REFERENCE_CODE.replace('-', " "),
        REFERENCE_CODE.replace('-', ""),
        REFERENCE_CODE.replace('O', "0").replace('I', "1"),
    ] {
        assert_eq!(parse_code(&typed), Ok(key()), "{typed}");
    }
}

#[test]
fn a_mistyped_code_is_rejected() {
    let swapped = REFERENCE_CODE.replacen("AAAQ", "AABQ", 1);
    assert_eq!(parse_code(&swapped), Err(CodeError::Typo));
    assert_eq!(parse_code(&REFERENCE_CODE[..30]), Err(CodeError::Length));
    let eight = REFERENCE_CODE.replacen('A', "8", 1);
    assert_eq!(parse_code(&eight), Err(CodeError::Character));
}

#[test]
fn derived_keys_match_the_reference_vectors() {
    let keys = SpaceKeys::derive(&key()).unwrap();
    assert_eq!(keys.token(), REFERENCE_TOKEN);
    assert_eq!(
        keys.record_key(&RecordName::Item("tt1".into())),
        [
            0x8f, 0x78, 0x2d, 0xb3, 0x06, 0xc2, 0x39, 0x80, 0x84, 0xc3, 0xa0, 0x7a, 0x7b, 0xd1,
            0xf1, 0xcc
        ]
    );
}

#[test]
fn the_keys_are_never_printed() {
    let printed = format!("{:?}", SpaceKeys::derive(&key()).unwrap());
    assert_eq!(printed, "SpaceKeys(..)");
}

#[test]
fn a_sealed_record_opens_only_with_its_key_in_its_place() {
    let keys = SpaceKeys::derive(&key()).unwrap();
    let record = item_record("tt1", "A film");
    let sealed = keys.seal(&record).unwrap();
    assert_eq!(keys.open(&sealed.key, &sealed.blob), Some(record.clone()));
    assert_ne!(
        keys.seal(&record).unwrap().blob,
        sealed.blob,
        "every seal uses a fresh nonce"
    );

    let other = SpaceKeys::derive(&SyncKey::from_bytes([9; 16])).unwrap();
    assert_eq!(other.open(&sealed.key, &sealed.blob), None, "another key");

    let moved = keys.record_key(&RecordName::Item("tt2".into()));
    assert_eq!(
        keys.open(&moved, &sealed.blob),
        None,
        "moved to another record"
    );

    let mut flipped = sealed.blob.clone();
    *flipped.last_mut().unwrap() ^= 1;
    assert_eq!(keys.open(&sealed.key, &flipped), None, "tampered");

    let mut newer = sealed.blob.clone();
    newer[0] = 2;
    assert_eq!(keys.open(&sealed.key, &newer), None, "unknown format");
}

#[test]
fn the_server_must_use_https_unless_it_is_this_machine() {
    let http = AddonClient::new(NetPolicy::default()).unwrap();
    for (server, allowed) in [
        ("https://sync.example/", true),
        ("http://sync.example/", false),
        ("http://127.0.0.1:8787/", true),
        ("http://[::1]:8787/", true),
        ("http://localhost:8787/", true),
        ("ftp://sync.example/", false),
    ] {
        let result = SyncClient::new(http.clone(), &pairing(server));
        assert_eq!(
            result.is_ok(),
            allowed,
            "{server}: {:?}",
            result.err().map(|e| e.to_string())
        );
    }
}

#[tokio::test]
async fn the_sync_server_follows_the_network_policy() {
    let server = MockServer::start().await;
    let client = SyncClient::new(
        AddonClient::new(NetPolicy::default()).unwrap(),
        &pairing(&server.uri()),
    )
    .unwrap();
    let err = client.pull(0).await.unwrap_err();
    assert!(
        matches!(
            err,
            SyncError::Fetch(FetchError::Blocked(BlockReason::NonPublicAddress(_)))
        ),
        "{err:?}"
    );
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn pull_opens_records_and_skips_unreadable_ones() {
    let server = MockServer::start().await;
    let keys = SpaceKeys::derive(&key()).unwrap();
    let stranger = SpaceKeys::derive(&SyncKey::from_bytes([9; 16])).unwrap();
    let record = item_record("tt1", "A film");
    Mock::given(method("GET"))
        .and(path("/base/v1/records"))
        .and(query_param("since", "41"))
        .and(query_param("limit", "500"))
        .and(header("authorization", format!("Bearer {REFERENCE_TOKEN}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "records": [
                sealed_entry(&keys, &record, 42),
                sealed_entry(&stranger, &item_record("tt2", "B"), 43),
                { "key": "!!", "seq": 44, "blob": "AA" },
                { "seq": 45 },
            ],
            "more": true,
            "seq": 45,
            "born": 9,
        })))
        .expect(1)
        .mount(&server)
        .await;
    let pulled = client(&server).pull(41).await.unwrap();
    assert_eq!(pulled.records, [(record, 42)]);
    assert_eq!(pulled.unreadable, 3);
    assert!(pulled.more);
    assert_eq!((pulled.space_seq, pulled.born), (45, 9));
}

#[tokio::test]
async fn a_wrong_server_address_is_an_error_not_an_empty_space() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    let err = client(&server).pull(0).await.unwrap_err();
    assert!(
        matches!(err, SyncError::Rejected(status) if status == 404),
        "{err:?}"
    );
}

#[tokio::test]
async fn server_answers_become_typed_errors() {
    for (template, expected) in [
        (ResponseTemplate::new(410), "SpaceGone"),
        (
            ResponseTemplate::new(503).insert_header("retry-after", "120"),
            "Unavailable { retry_after_s: Some(120) }",
        ),
        (
            ResponseTemplate::new(429),
            "Unavailable { retry_after_s: None }",
        ),
        (ResponseTemplate::new(401), "Rejected(401)"),
        (
            ResponseTemplate::new(200).set_body_string("<html>"),
            "Malformed",
        ),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(template)
            .mount(&server)
            .await;
        let err = client(&server).pull(0).await.unwrap_err();
        assert_eq!(format!("{err:?}"), expected);
    }
}

#[tokio::test]
async fn an_oversized_pull_page_is_refused() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![b' '; MAX_PULL_BYTES + 1]))
        .mount(&server)
        .await;
    let err = client(&server).pull(0).await.unwrap_err();
    assert!(
        matches!(err, SyncError::Fetch(FetchError::TooLarge { .. })),
        "{err:?}"
    );
}

struct EchoSeqs;

impl Respond for EchoSeqs {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body: Json = serde_json::from_slice(&request.body).unwrap();
        let writes = body["writes"].as_array().unwrap();
        let seqs: Vec<u64> = writes
            .iter()
            .map(|w| w["base_seq"].as_u64().unwrap() + 1000)
            .collect();
        ResponseTemplate::new(200).set_body_json(json!({ "seqs": seqs }))
    }
}

#[tokio::test]
async fn push_sends_sealed_writes_with_their_base_versions() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/base/v1/records"))
        .and(header("authorization", format!("Bearer {REFERENCE_TOKEN}")))
        .and(header("content-type", "application/json"))
        .respond_with(EchoSeqs)
        .expect(1)
        .mount(&server)
        .await;
    let records = [item_record("tt1", "A"), item_record("tt2", "B")];
    let outcome = client(&server)
        .push(&[
            outgoing(records[0].clone(), 0),
            outgoing(records[1].clone(), 7),
        ])
        .await;
    assert!(outcome.error.is_none(), "{:?}", outcome.error);
    let seqs: Vec<(RecordName, u64)> = outcome
        .pushed
        .iter()
        .map(|p| (p.name.clone(), p.seq))
        .collect();
    assert_eq!(
        seqs,
        [
            (RecordName::Item("tt1".into()), 1000),
            (RecordName::Item("tt2".into()), 1007)
        ]
    );

    let requests = server.received_requests().await.unwrap();
    let body: Json = serde_json::from_slice(&requests[0].body).unwrap();
    let keys = SpaceKeys::derive(&key()).unwrap();
    let write = &body["writes"][1];
    let opened = keys.open(
        &BASE64URL_NOPAD
            .decode(write["key"].as_str().unwrap().as_bytes())
            .unwrap(),
        &BASE64URL_NOPAD
            .decode(write["blob"].as_str().unwrap().as_bytes())
            .unwrap(),
    );
    assert_eq!(opened, Some(records[1].clone()));
    assert!(
        !String::from_utf8_lossy(&requests[0].body).contains("tt2"),
        "names and ids never leave the device in clear"
    );
}

#[tokio::test]
async fn pushes_are_split_into_batches() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(EchoSeqs)
        .mount(&server)
        .await;
    let out: Vec<Outgoing> = (0..250)
        .map(|i| outgoing(item_record(&format!("tt{i}"), "x"), 0))
        .collect();
    let outcome = client(&server).push(&out).await;
    assert!(outcome.error.is_none());
    assert_eq!(outcome.pushed.len(), 250);
    let sizes: Vec<usize> = server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .map(|r| {
            serde_json::from_slice::<Json>(&r.body).unwrap()["writes"]
                .as_array()
                .unwrap()
                .len()
        })
        .collect();
    assert_eq!(
        sizes,
        [
            MAX_BATCH_RECORDS,
            MAX_BATCH_RECORDS,
            MAX_BATCH_RECORDS,
            250 - 3 * MAX_BATCH_RECORDS
        ]
    );
}

#[tokio::test]
async fn a_conflict_stops_the_push_and_keeps_what_went_through() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(EchoSeqs)
        .up_to_n_times(1)
        .mount(&server)
        .await;
    let keys = SpaceKeys::derive(&key()).unwrap();
    let clash = format!("tt{MAX_BATCH_RECORDS}");
    let clash_key = keys.record_key(&RecordName::Item(clash.clone()));
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(409).set_body_json(json!({
            "conflicts": [
                { "key": BASE64URL_NOPAD.encode(&clash_key), "seq": 5 },
                { "key": BASE64URL_NOPAD.encode(&[0; 16]), "seq": 6 },
            ],
        })))
        .mount(&server)
        .await;
    let out: Vec<Outgoing> = (0..150)
        .map(|i| outgoing(item_record(&format!("tt{i}"), "x"), 0))
        .collect();
    let outcome = client(&server).push(&out).await;
    assert_eq!(outcome.pushed.len(), MAX_BATCH_RECORDS);
    let Some(SyncError::Conflict(conflicts)) = outcome.error else {
        panic!("{:?}", outcome.error);
    };
    assert_eq!(
        conflicts,
        [(RecordName::Item(clash), 5)],
        "keys the batch did not send are ignored"
    );
}

#[tokio::test]
async fn a_record_too_large_to_push_is_reported_and_skipped() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(EchoSeqs)
        .mount(&server)
        .await;
    let outcome = client(&server)
        .push(&[
            outgoing(item_record("big", &"x".repeat(40_000)), 0),
            outgoing(item_record("small", "x"), 0),
        ])
        .await;
    assert_eq!(outcome.oversized, [RecordName::Item("big".into())]);
    assert_eq!(outcome.pushed.len(), 1);
}

#[tokio::test]
async fn deleting_the_space_tolerates_one_already_gone() {
    for status in [204, 410] {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/base/v1/space"))
            .respond_with(ResponseTemplate::new(status))
            .expect(1)
            .mount(&server)
            .await;
        client(&server).delete_space().await.unwrap();
    }
}

#[test]
fn the_pairing_link_carries_the_code_and_a_server_other_than_the_default() {
    let default = pairing(cineo_core::sync::DEFAULT_SERVER);
    assert_eq!(
        cineo_net::sync::pairing_link(&default),
        format!("cineo:///sync/{REFERENCE_CODE}")
    );
    let own = pairing("https://sync.example/cineo/");
    let link = cineo_net::sync::pairing_link(&own);
    assert_eq!(
        cineo_core::app::parse_link(&link),
        Ok(cineo_core::app::Route::Sync {
            code: REFERENCE_CODE.into(),
            server: Some(own.server),
        })
    );
}
