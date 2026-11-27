#![allow(clippy::unwrap_used, reason = "test helpers panic on purpose")]

use cineo_core::sync::SyncKey;
use cineo_core::sync::protocol::{
    MAX_BATCH_RECORDS, MAX_BLOB_BYTES, MAX_SPACE_BYTES, MAX_SPACE_RECORDS, PullPage, PushRequest,
    PushResponse, Write, parse_work_required,
};
use cineo_net::sync::{SpaceKeys, find_work};

use super::*;
use crate::native::SqliteDb;

const NOW: u64 = 0;
const TEST_WORK_BITS: u32 = 4;
const WORK_BITS_FOR_TEST: u32 = cineo_core::sync::protocol::WORK_BITS;

fn keys(who: u8) -> SpaceKeys {
    SpaceKeys::derive(&SyncKey::from_bytes([who; 16])).unwrap()
}

fn ctx() -> Context {
    Context {
        now_ms: NOW,
        nonce: 7,
        born: 41,
        work_bits: TEST_WORK_BITS,
    }
}

async fn handle(db: &impl Db, request: &Request, ctx: Context) -> Outcome {
    super::handle(db, &true, request, ctx).await
}

fn unsigned(method: Method, path: &str, body: Vec<u8>) -> Request {
    let (path, query) = match path.split_once('?') {
        Some((path, query)) => (path.to_owned(), Some(query.to_owned())),
        None => (path.to_owned(), None),
    };
    Request {
        method,
        path,
        query,
        authorization: None,
        work: None,
        body,
    }
}

fn signed_at(method: Method, path: &str, who: u8, body: Vec<u8>, time_ms: u64) -> Request {
    let mut request = unsigned(method, path, body);
    let target = request_target(&request.path, request.query.as_deref()).unwrap_or_default();
    request.authorization =
        Some(keys(who).authorization(method_name(method), &target, time_ms, &request.body));
    request.work = Some(find_work(&keys(who).public_key(), TEST_WORK_BITS).to_string());
    request
}

fn request(method: Method, path: &str, who: Option<u8>, body: Vec<u8>) -> Request {
    match who {
        Some(who) => signed_at(method, path, who, body, NOW),
        None => unsigned(method, path, body),
    }
}

fn write(key: u8, base_seq: u64, blob: &[u8]) -> Write {
    Write {
        key: vec![key; 16],
        base_seq,
        blob: blob.to_vec(),
    }
}

async fn push_as(db: &impl Db, who: u8, writes: Vec<Write>, ctx: Context) -> Outcome {
    let body = PushRequest { writes }.to_json().into_bytes();
    handle(
        db,
        &request(Method::Post, "/v1/records", Some(who), body),
        ctx,
    )
    .await
}

async fn push(db: &impl Db, writes: Vec<Write>) -> Response {
    push_as(db, 1, writes, ctx()).await.response
}

async fn pull(db: &impl Db, query: &str) -> Response {
    handle(
        db,
        &request(
            Method::Get,
            &format!("/v1/records?{query}"),
            Some(1),
            Vec::new(),
        ),
        ctx(),
    )
    .await
    .response
}

fn page(response: &Response) -> PullPage {
    assert_eq!(response.status, 200, "{response:?}");
    PullPage::parse(response.body.as_bytes()).unwrap()
}

fn seqs(response: &Response) -> Vec<u64> {
    assert_eq!(response.status, 200, "{response:?}");
    PushResponse::parse(response.body.as_bytes()).unwrap().seqs
}

async fn space_row(db: &impl Db, who: u8) -> Vec<SqlValue> {
    let id = space_id(&keys(who).public_key());
    db.rows(Statement::new(
        "SELECT seq, records, bytes, deleted FROM spaces WHERE id = ?1",
        vec![SqlValue::Blob(id)],
    ))
    .await
    .unwrap()
    .remove(0)
}

#[tokio::test]
async fn info_needs_no_signature_and_states_the_limits() {
    let db = SqliteDb::open_in_memory().unwrap();
    let response = handle(
        &db,
        &request(Method::Get, "/v1/info", None, Vec::new()),
        ctx(),
    )
    .await
    .response;
    assert_eq!(response.status, 200);
    assert!(response.body.contains("\"max_batch_records\":64"));
}

#[tokio::test]
async fn records_need_a_valid_signature() {
    let db = SqliteDb::open_in_memory().unwrap();
    let good = signed_at(Method::Get, "/v1/records?since=0", 1, Vec::new(), NOW);
    let mut other_query = good.clone();
    other_query.query = Some("since=5".into());
    let mut other_body = good.clone();
    other_body.body = b"{}".to_vec();
    let mut other_method = good.clone();
    other_method.method = Method::Post;
    let mut other_key = good.clone();
    let stranger = Authorization::parse(
        keys(2)
            .authorization("GET", "v1/records?since=0", NOW, &[])
            .as_str(),
    )
    .unwrap();
    let mut forged = Authorization::parse(good.authorization.as_deref().unwrap()).unwrap();
    forged.key = stranger.key;
    other_key.authorization = Some(forged.to_header());
    let mut garbage = good.clone();
    garbage.authorization = Some("Bearer abc".into());
    let unsigned = unsigned(Method::Get, "/v1/records", Vec::new());
    for (name, request) in [
        ("no header", unsigned),
        ("another scheme", garbage),
        ("another query", other_query),
        ("another body", other_body),
        ("another method", other_method),
        ("another key", other_key),
    ] {
        let response = handle(&db, &request, ctx()).await.response;
        assert_eq!(response.status, 401, "{name}");
    }
    assert_eq!(handle(&db, &good, ctx()).await.response.status, 200);
}

#[tokio::test]
async fn a_signed_request_expires_after_ten_minutes() {
    let db = SqliteDb::open_in_memory().unwrap();
    let request = signed_at(Method::Get, "/v1/records", 1, Vec::new(), NOW);
    for (later_ms, status) in [
        (0, 200),
        (MAX_CLOCK_SKEW_MS, 200),
        (MAX_CLOCK_SKEW_MS + 1, 401),
    ] {
        let at = Context {
            now_ms: NOW + later_ms,
            ..ctx()
        };
        assert_eq!(
            handle(&db, &request, at).await.response.status,
            status,
            "{later_ms}"
        );
    }
    let future = signed_at(
        Method::Get,
        "/v1/records",
        1,
        Vec::new(),
        NOW + MAX_CLOCK_SKEW_MS + 1,
    );
    assert_eq!(handle(&db, &future, ctx()).await.response.status, 401);
}

#[tokio::test]
async fn only_a_new_space_needs_proof_of_work() {
    let db = SqliteDb::open_in_memory().unwrap();
    let body = PushRequest {
        writes: vec![write(1, 0, b"a")],
    }
    .to_json()
    .into_bytes();
    let strict = Context {
        work_bits: WORK_BITS_FOR_TEST,
        ..ctx()
    };
    let mut lazy = signed_at(Method::Post, "/v1/records", 1, body.clone(), NOW);
    lazy.work = None;
    let refused = handle(&db, &lazy, strict).await.response;
    assert_eq!(refused.status, 428);
    assert_eq!(
        parse_work_required(refused.body.as_bytes()),
        Some(WORK_BITS_FOR_TEST)
    );
    let key = keys(1).public_key();
    let short = (0..)
        .find(|n| !work_done(&key, Some(*n), WORK_BITS_FOR_TEST))
        .unwrap();
    let mut weak = lazy.clone();
    weak.work = Some(short.to_string());
    assert_eq!(handle(&db, &weak, strict).await.response.status, 428);
    let mut done = lazy.clone();
    done.work = Some(find_work(&keys(1).public_key(), WORK_BITS_FOR_TEST).to_string());
    assert_eq!(handle(&db, &done, strict).await.response.status, 200);
    let next = PushRequest {
        writes: vec![write(2, 0, b"b")],
    }
    .to_json()
    .into_bytes();
    let mut again = signed_at(Method::Post, "/v1/records", 1, next, NOW);
    again.work = None;
    assert_eq!(handle(&db, &again, strict).await.response.status, 200);
}

#[tokio::test]
async fn unknown_paths_and_methods_are_refused_and_a_prefix_is_allowed() {
    let db = SqliteDb::open_in_memory().unwrap();
    let status = |method, path: &'static str| {
        let db = &db;
        async move {
            handle(db, &request(method, path, Some(1), Vec::new()), ctx())
                .await
                .response
                .status
        }
    };
    assert_eq!(status(Method::Get, "/v2/records").await, 404);
    assert_eq!(status(Method::Get, "/").await, 404);
    assert_eq!(status(Method::Delete, "/v1/records").await, 405);
    assert_eq!(status(Method::Other, "/v1/info").await, 405);
    assert_eq!(status(Method::Get, "/cineo/v1/info").await, 200);
}

#[tokio::test]
async fn a_space_nobody_wrote_to_is_empty() {
    let db = SqliteDb::open_in_memory().unwrap();
    assert_eq!(page(&pull(&db, "since=0").await), PullPage::default());
}

#[tokio::test]
async fn pushed_records_come_back_in_order_with_their_versions() {
    let db = SqliteDb::open_in_memory().unwrap();
    assert_eq!(
        seqs(&push(&db, vec![write(1, 0, b"a"), write(2, 0, b"b")]).await),
        [1, 2]
    );
    assert_eq!(seqs(&push(&db, vec![write(1, 1, b"a2")]).await), [3]);
    let records = page(&pull(&db, "since=0").await).records;
    let got: Vec<(u8, u64, &[u8])> = records
        .iter()
        .map(|r| (r.key[0], r.seq, r.blob.as_slice()))
        .collect();
    assert_eq!(got, [(2, 2, &b"b"[..]), (1, 3, &b"a2"[..])]);
    assert_eq!(page(&pull(&db, "since=2").await).records.len(), 1);
    assert_eq!(
        space_row(&db, 1).await,
        [
            SqlValue::Int(3),
            SqlValue::Int(2),
            SqlValue::Int(3),
            SqlValue::Int(0)
        ]
    );
}

#[tokio::test]
async fn a_page_tells_the_space_version_and_birth() {
    let db = SqliteDb::open_in_memory().unwrap();
    seqs(&push(&db, vec![write(1, 0, b"a"), write(2, 0, b"b")]).await);
    let born = Context {
        born: 1234,
        ..ctx()
    };
    seqs(
        &push_as(&db, 1, vec![write(3, 0, b"c")], born)
            .await
            .response,
    );
    let page = page(&pull(&db, "since=3").await);
    assert!(page.records.is_empty());
    assert_eq!((page.space_seq, page.born), (3, 41), "born is set once");
}

#[tokio::test]
async fn versions_follow_the_clock_so_a_restored_server_never_reuses_them() {
    let db = SqliteDb::open_in_memory().unwrap();
    let at = |now_ms| Context { now_ms, ..ctx() };
    let early = push_as(
        &db,
        1,
        vec![write(1, 0, b"a"), write(2, 0, b"b")],
        at(1_000),
    )
    .await;
    assert_eq!(
        seqs(&early.response),
        [1_000 * SEQS_PER_MS + 1, 1_000 * SEQS_PER_MS + 2]
    );
    let same_ms = push_as(&db, 1, vec![write(3, 0, b"c")], at(1_000)).await;
    assert_eq!(seqs(&same_ms.response), [1_000 * SEQS_PER_MS + 3]);
    let clock_back = push_as(&db, 1, vec![write(4, 0, b"d")], at(10)).await;
    assert_eq!(
        seqs(&clock_back.response),
        [1_000 * SEQS_PER_MS + 4],
        "never below a version already given"
    );
}

#[tokio::test]
async fn spaces_are_separate() {
    let db = SqliteDb::open_in_memory().unwrap();
    seqs(&push(&db, vec![write(1, 0, b"a")]).await);
    let other = handle(
        &db,
        &request(Method::Get, "/v1/records", Some(2), Vec::new()),
        ctx(),
    )
    .await
    .response;
    assert_eq!(page(&other), PullPage::default());
}

#[tokio::test]
async fn a_stale_base_version_is_refused_with_the_conflicting_keys() {
    let db = SqliteDb::open_in_memory().unwrap();
    seqs(&push(&db, vec![write(1, 0, b"a")]).await);
    let response = push(&db, vec![write(1, 0, b"x"), write(2, 0, b"y")]).await;
    assert_eq!(response.status, 409);
    assert_eq!(
        cineo_core::sync::protocol::parse_conflicts(response.body.as_bytes()),
        [(vec![1; 16], 1)],
        "only the stale key, with the server's version"
    );
    assert_eq!(
        page(&pull(&db, "since=0").await).records.len(),
        1,
        "nothing of the refused batch was written"
    );
}

#[tokio::test]
async fn malformed_pushes_are_refused() {
    let db = SqliteDb::open_in_memory().unwrap();
    let too_many = PushRequest {
        writes: (0..=MAX_BATCH_RECORDS)
            .map(|i| Write {
                key: vec![u8::try_from(i).unwrap(); 16],
                base_seq: 0,
                blob: b"x".to_vec(),
            })
            .collect(),
    };
    let short_key = Write {
        key: vec![1; 15],
        base_seq: 0,
        blob: b"x".to_vec(),
    };
    for body in [
        b"not json".to_vec(),
        b"{\"writes\":[{\"key\":\"AAAA\"}]}".to_vec(),
        too_many.to_json().into_bytes(),
        PushRequest {
            writes: vec![short_key],
        }
        .to_json()
        .into_bytes(),
        PushRequest {
            writes: vec![write(1, 0, b"")],
        }
        .to_json()
        .into_bytes(),
        PushRequest {
            writes: vec![write(1, 0, &vec![0; MAX_BLOB_BYTES + 1])],
        }
        .to_json()
        .into_bytes(),
        PushRequest {
            writes: vec![write(1, 0, b"a"), write(1, 0, b"b")],
        }
        .to_json()
        .into_bytes(),
    ] {
        let response = handle(
            &db,
            &request(Method::Post, "/v1/records", Some(1), body.clone()),
            ctx(),
        )
        .await
        .response;
        assert_eq!(
            response.status,
            400,
            "{}",
            String::from_utf8_lossy(&body[..body.len().min(80)])
        );
    }
}

#[tokio::test]
async fn a_full_space_refuses_more_but_takes_replacements() {
    let db = SqliteDb::open_in_memory().unwrap();
    seqs(&push(&db, vec![write(1, 0, b"aa")]).await);
    let id = space_id(&keys(1).public_key());
    db.batch(vec![Statement::new(
        "UPDATE spaces SET records = ?2, bytes = ?3 WHERE id = ?1",
        vec![
            SqlValue::Blob(id),
            SqlValue::from_u64(MAX_SPACE_RECORDS),
            SqlValue::from_u64(MAX_SPACE_BYTES),
        ],
    )])
    .await
    .unwrap();
    assert_eq!(push(&db, vec![write(2, 0, b"b")]).await.status, 413);
    assert_eq!(push(&db, vec![write(1, 1, b"aaa")]).await.status, 413);
    assert_eq!(seqs(&push(&db, vec![write(1, 1, b"bb")]).await), [2]);
}

#[tokio::test]
async fn a_deleted_space_is_gone_and_cannot_come_back() {
    let db = SqliteDb::open_in_memory().unwrap();
    seqs(&push(&db, vec![write(1, 0, b"a")]).await);
    for who in [1, 2] {
        let deleted = handle(
            &db,
            &request(Method::Delete, "/v1/space", Some(who), Vec::new()),
            ctx(),
        )
        .await
        .response;
        assert_eq!(deleted.status, 204);
        let pulled = handle(
            &db,
            &request(Method::Get, "/v1/records", Some(who), Vec::new()),
            ctx(),
        )
        .await
        .response;
        assert_eq!(pulled.status, 410);
        let pushed = push_as(&db, who, vec![write(9, 0, b"z")], ctx()).await;
        assert_eq!(pushed.response.status, 410, "space {who}");
    }
    assert_eq!(space_row(&db, 1).await[1], SqlValue::Int(0));
}

struct CountingGate {
    allow: bool,
    asked: std::cell::Cell<u32>,
}

impl Gate for CountingGate {
    async fn may_create(&self) -> bool {
        self.asked.set(self.asked.get() + 1);
        self.allow
    }
}

async fn push_gated(db: &impl Db, gate: &CountingGate, key: u8) -> Outcome {
    let body = PushRequest {
        writes: vec![write(key, 0, b"x")],
    }
    .to_json()
    .into_bytes();
    super::handle(
        db,
        gate,
        &request(Method::Post, "/v1/records", Some(1), body),
        ctx(),
    )
    .await
}

#[tokio::test]
async fn only_new_spaces_ask_for_permission() {
    let db = SqliteDb::open_in_memory().unwrap();
    let refuse = CountingGate {
        allow: false,
        asked: 0.into(),
    };
    let refused = push_gated(&db, &refuse, 1).await;
    assert_eq!(refused.response.status, 429);
    assert_eq!(refused.response.retry_after_s, Some(CREATE_RETRY_S));
    let allow = CountingGate {
        allow: true,
        asked: 0.into(),
    };
    let created = push_gated(&db, &allow, 1).await;
    assert!(created.created);
    let again = push_gated(&db, &refuse, 2).await;
    assert_eq!(again.response.status, 200);
    assert!(!again.created);
    assert_eq!(
        (refuse.asked.get(), allow.asked.get()),
        (1, 1),
        "an existing space never asks"
    );
}

struct RacingDb {
    inner: SqliteDb,
}

impl Db for RacingDb {
    async fn rows(&self, statement: Statement) -> Result<Vec<Vec<SqlValue>>, DbError> {
        self.inner.rows(statement).await
    }

    async fn batch(&self, statements: Vec<Statement>) -> Result<Vec<u64>, DbError> {
        let rival = Context { nonce: 99, ..ctx() };
        let raced = push_as(&self.inner, 1, vec![write(5, 0, b"rival")], rival).await;
        assert_eq!(raced.response.status, 200);
        self.inner.batch(statements).await
    }
}

#[tokio::test]
async fn a_write_that_loses_a_race_is_refused_whole() {
    let db = RacingDb {
        inner: SqliteDb::open_in_memory().unwrap(),
    };
    seqs(&push(&db.inner, vec![write(1, 0, b"a")]).await);
    let response = push(&db, vec![write(1, 1, b"mine"), write(2, 0, b"mine")]).await;
    assert_eq!(response.status, 409);
    let records = page(&pull(&db.inner, "since=0").await).records;
    let blobs: Vec<&[u8]> = records.iter().map(|r| r.blob.as_slice()).collect();
    assert_eq!(blobs, [&b"a"[..], &b"rival"[..]]);
}

#[tokio::test]
async fn pages_stop_at_the_record_limit_and_resume() {
    let db = SqliteDb::open_in_memory().unwrap();
    seqs(
        &push(
            &db,
            vec![write(1, 0, b"a"), write(2, 0, b"b"), write(3, 0, b"c")],
        )
        .await,
    );
    let first = page(&pull(&db, "since=0&limit=2").await);
    assert_eq!(first.records.len(), 2);
    assert!(first.more);
    let second = page(&pull(&db, "since=2&limit=2").await);
    assert_eq!(second.records.len(), 1);
    assert!(!second.more);
}

#[tokio::test]
async fn pages_stop_before_the_byte_limit() {
    let db = SqliteDb::open_in_memory().unwrap();
    let big = vec![7; MAX_BLOB_BYTES];
    for start in [0_u8, 35] {
        seqs(
            &push(
                &db,
                (start..start + 35).map(|k| write(k, 0, &big)).collect(),
            )
            .await,
        );
    }
    let first = page(&pull(&db, "since=0").await);
    assert!(first.more);
    assert!(first.records.len() < 70);
    assert!(first.to_json().len() <= MAX_PULL_BYTES);
    let last = first.records.last().unwrap().seq;
    let rest = page(&pull(&db, &format!("since={last}")).await);
    assert_eq!(first.records.len() + rest.records.len(), 70);
}

#[tokio::test]
async fn idle_spaces_and_deletion_markers_are_swept() {
    let db = SqliteDb::open_in_memory().unwrap();
    seqs(&push(&db, vec![write(1, 0, b"a")]).await);
    handle(
        &db,
        &request(Method::Delete, "/v1/space", Some(2), Vec::new()),
        ctx(),
    )
    .await;
    sweep(&db, NOW + IDLE_MS).await.unwrap();
    assert_eq!(page(&pull(&db, "since=0").await).records.len(), 1);
    sweep(&db, NOW + IDLE_MS + 1).await.unwrap();
    assert_eq!(page(&pull(&db, "since=0").await), PullPage::default());
    let count = db
        .rows(Statement::new(
            "SELECT (SELECT count(*) FROM spaces) + (SELECT count(*) FROM records)",
            Vec::new(),
        ))
        .await
        .unwrap();
    assert_eq!(count, [[SqlValue::Int(0)]]);
}

struct FailingDb;

impl Db for FailingDb {
    async fn rows(&self, _: Statement) -> Result<Vec<Vec<SqlValue>>, DbError> {
        Err(DbError("daily limit reached".into()))
    }

    async fn batch(&self, _: Vec<Statement>) -> Result<Vec<u64>, DbError> {
        Err(DbError("daily limit reached".into()))
    }
}

#[tokio::test]
async fn a_storage_failure_asks_the_client_to_come_back_later() {
    let response = pull(&FailingDb, "since=0").await;
    assert_eq!(response.status, 503);
    assert_eq!(response.retry_after_s, Some(BUSY_RETRY_S));
}
