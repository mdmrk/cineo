#![allow(clippy::unwrap_used, reason = "test helpers panic on purpose")]

use cineo_core::sync::protocol::{
    MAX_BATCH_RECORDS, MAX_BLOB_BYTES, MAX_SPACE_BYTES, MAX_SPACE_RECORDS, PullPage, PushRequest,
    PushResponse, Write, encode,
};

use super::*;
use crate::native::SqliteDb;

const NOW: u64 = 0;

fn token(n: u8) -> String {
    encode(&[n; 32])
}

fn ctx() -> Context {
    Context {
        now_ms: NOW,
        nonce: 7,
        born: 41,
        may_create: true,
    }
}

fn request(method: Method, path: &str, bearer: Option<String>, body: Vec<u8>) -> Request {
    let (path, query) = match path.split_once('?') {
        Some((path, query)) => (path.to_owned(), Some(query.to_owned())),
        None => (path.to_owned(), None),
    };
    Request {
        method,
        path,
        query,
        bearer,
        body,
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
        &request(Method::Post, "/v1/records", Some(token(who)), body),
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
            Some(token(1)),
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
    let id = space_id(Some(&token(who))).unwrap();
    db.rows(Statement::new(
        "SELECT seq, records, bytes, deleted FROM spaces WHERE id = ?1",
        vec![SqlValue::Blob(id)],
    ))
    .await
    .unwrap()
    .remove(0)
}

#[tokio::test]
async fn info_needs_no_token_and_states_the_limits() {
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
async fn records_need_a_valid_token() {
    let db = SqliteDb::open_in_memory().unwrap();
    for bearer in [None, Some("not base64!".to_owned()), Some(encode(&[1; 31]))] {
        let response = handle(
            &db,
            &request(Method::Get, "/v1/records", bearer.clone(), Vec::new()),
            ctx(),
        )
        .await
        .response;
        assert_eq!(response.status, 401, "{bearer:?}");
    }
}

#[tokio::test]
async fn unknown_paths_and_methods_are_refused_and_a_prefix_is_allowed() {
    let db = SqliteDb::open_in_memory().unwrap();
    let status = |method, path: &'static str| {
        let db = &db;
        async move {
            handle(
                db,
                &request(method, path, Some(token(1)), Vec::new()),
                ctx(),
            )
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
        &request(Method::Get, "/v1/records", Some(token(2)), Vec::new()),
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
            &request(Method::Post, "/v1/records", Some(token(1)), body.clone()),
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
    let id = space_id(Some(&token(1))).unwrap();
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
            &request(Method::Delete, "/v1/space", Some(token(who)), Vec::new()),
            ctx(),
        )
        .await
        .response;
        assert_eq!(deleted.status, 204);
        let pulled = handle(
            &db,
            &request(Method::Get, "/v1/records", Some(token(who)), Vec::new()),
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

#[tokio::test]
async fn new_spaces_need_permission_but_existing_ones_do_not() {
    let db = SqliteDb::open_in_memory().unwrap();
    let refused = Context {
        may_create: false,
        ..ctx()
    };
    let response = push_as(&db, 1, vec![write(1, 0, b"a")], refused).await;
    assert_eq!(response.response.status, 429);
    assert_eq!(response.response.retry_after_s, Some(CREATE_RETRY_S));
    let created = push_as(&db, 1, vec![write(1, 0, b"a")], ctx()).await;
    assert!(created.created);
    let again = push_as(&db, 1, vec![write(2, 0, b"b")], refused).await;
    assert_eq!(again.response.status, 200);
    assert!(!again.created);
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
        &request(Method::Delete, "/v1/space", Some(token(2)), Vec::new()),
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
