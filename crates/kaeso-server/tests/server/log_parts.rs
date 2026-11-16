#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use crate::common;

use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use clickhouse::Row;
use common::v1::Env;
use kaeso_core::clock::Clock;
use kaeso_store::clickhouse::logs as ch;
use kaeso_store::clickhouse::parts::{self as ch_parts, PartRow};
use serde::Deserialize;
use serde_json::json;
use time::OffsetDateTime;
use uuid::Uuid;

const HOUR: time::Duration = time::Duration::HOUR;

async fn env() -> Env {
    let e = common::v1::env(None, None).await;
    e.app
        .state
        .clickhouse
        .as_ref()
        .unwrap()
        .migrate(&e.app.state.pg)
        .await
        .unwrap();
    e
}

fn clickhouse(env: &Env) -> &kaeso_store::clickhouse::ClickHouse {
    env.app.state.clickhouse.as_ref().unwrap()
}

async fn flush(env: &Env) {
    env.app.state.in_flight.settled().await;
    kaeso_server::services::logs::flush_all(&env.app.state, Duration::from_secs(10)).await;
    assert_eq!(env.app.state.logs.len(), 0, "the buffer drained");
}

// Text that doesn't compress, as real conversations mostly don't between turns' new messages.
fn noise(seed: u64, len: usize) -> String {
    let mut x = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
    (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            char::from(b'a' + (x % 26) as u8)
        })
        .collect()
}

#[derive(Clone, Copy, Debug)]
enum Format {
    Anthropic,
    Chat,
    Responses,
    Gemini,
}

impl Format {
    fn path(self) -> &'static str {
        match self {
            Format::Anthropic => "/v1/messages",
            Format::Chat => "/v1/chat/completions",
            Format::Responses => "/v1/responses",
            Format::Gemini => "/v1beta/models/claude-haiku-4-5:generateContent",
        }
    }

    // A conversation of `turns` long messages and one short one, written with the spacing and
    // escapes an app might send, so a read that isn't byte for byte shows.
    fn body(self, turns: u64, seed: u64) -> String {
        let long = |i: u64| format!("{} \\u00e9\\n", noise(seed + i, 700));
        let instructions = format!("{} \\\"rules\\\"", noise(seed, 900));
        match self {
            Format::Anthropic => {
                let mut msgs: Vec<String> = (0..turns)
                    .map(|i| {
                        let role = if i % 2 == 0 { "user" } else { "assistant" };
                        format!("{{\"role\": \"{role}\", \"content\": \"{}\"}}", long(i))
                    })
                    .collect();
                msgs.push(r#"{"role":"user","content":"short and inline"}"#.to_owned());
                format!(
                    "{{\"model\":\"claude-haiku-4-5\",\"max_tokens\":64,\n \"system\": \"{instructions}\",\n \"messages\": [\n  {}\n ]}}",
                    msgs.join(",\n  ")
                )
            }
            Format::Chat => {
                let mut msgs = vec![format!(
                    "{{\"role\":\"system\",\"content\":\"{instructions}\"}}"
                )];
                msgs.extend((0..turns).map(|i| {
                    let role = if i % 2 == 0 { "user" } else { "assistant" };
                    format!("{{\"role\":\"{role}\",\"content\":\"{}\"}}", long(i))
                }));
                msgs.push(r#"{"role":"user","content":"short and inline"}"#.to_owned());
                format!(
                    "{{\"model\": \"claude-haiku-4-5\", \"messages\": [{}]}}",
                    msgs.join(", ")
                )
            }
            Format::Responses => {
                let mut items: Vec<String> = (0..turns)
                    .map(|i| {
                        let role = if i % 2 == 0 { "user" } else { "assistant" };
                        format!(
                            "{{\"type\":\"message\",\"role\":\"{role}\",\"content\":\"{}\"}}",
                            long(i)
                        )
                    })
                    .collect();
                items.push(r#"{"role":"user","content":"short and inline"}"#.to_owned());
                format!(
                    "{{\"model\":\"claude-haiku-4-5\",\"max_output_tokens\":64,\"instructions\":\"{instructions}\",\"input\":[{}]}}",
                    items.join(",")
                )
            }
            Format::Gemini => {
                let mut contents: Vec<String> = (0..turns)
                    .map(|i| {
                        let role = if i % 2 == 0 { "user" } else { "model" };
                        format!(
                            "{{\"role\":\"{role}\",\"parts\":[{{\"text\":\"{}\"}}]}}",
                            long(i)
                        )
                    })
                    .collect();
                contents
                    .push(r#"{"role":"user","parts":[{"text":"short and inline"}]}"#.to_owned());
                format!(
                    "{{\"systemInstruction\":{{\"parts\":[{{\"text\":\"{instructions}\"}}]}},\"contents\":[{}]}}",
                    contents.join(",")
                )
            }
        }
    }
}

async fn send_to(env: &Env, key: &str, format: Format, body: &str) -> String {
    let mut req = Request::builder()
        .method("POST")
        .uri(format.path())
        .header("content-type", "application/json");
    req = match format {
        Format::Anthropic => req
            .header("x-api-key", key)
            .header("anthropic-version", "2023-06-01"),
        Format::Gemini => req.header("x-goog-api-key", key),
        _ => req.header("authorization", format!("Bearer {key}")),
    };
    let (status, headers, bytes) = env
        .app
        .raw_bytes(req.body(Body::from(body.to_owned())).unwrap())
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "{format:?}: {}",
        String::from_utf8_lossy(&bytes)
    );
    headers["kaeso-request-id"].to_str().unwrap().to_owned()
}

async fn send(env: &Env, format: Format, body: &str) -> String {
    send_to(env, &env.key, format, body).await
}

fn uuid_of(public: &str) -> Uuid {
    common::uuid(public)
}

#[derive(Debug, Row, Deserialize)]
struct Stored {
    #[serde(with = "clickhouse::serde::uuid")]
    request_id: Uuid,
    #[serde(with = "clickhouse::serde::uuid")]
    project_id: Uuid,
    #[serde(with = "clickhouse::serde::time::datetime64::millis")]
    started_at: OffsetDateTime,
    request_body: String,
    request_parts: u32,
    #[serde(with = "clickhouse::serde::time::datetime")]
    expires_at: OffsetDateTime,
}

async fn stored(env: &Env) -> Vec<Stored> {
    clickhouse(env)
        .client()
        .query("SELECT request_id, project_id, started_at, request_body, request_parts, expires_at FROM request_content ORDER BY started_at")
        .fetch_all::<Stored>()
        .await
        .unwrap()
}

async fn stored_one(env: &Env, id: &str) -> Stored {
    let id = uuid_of(id);
    stored(env)
        .await
        .into_iter()
        .find(|s| s.request_id == id)
        .unwrap()
}

async fn part_rows(env: &Env) -> Vec<PartRow> {
    clickhouse(env)
        .client()
        .query("SELECT ?fields FROM request_content_part ORDER BY first_at")
        .fetch_all::<PartRow>()
        .await
        .unwrap()
}

// Every body that names a part finds a row of it in its own project that spans the request,
// allowing for the rewrite interval, and outlives it: what deletion and TTL go by.
async fn every_body_keeps_its_parts(env: &Env) {
    let rows = part_rows(env).await;
    for s in stored(env).await.iter().filter(|s| s.request_parts > 0) {
        let hashes = ch_parts::hashes(&s.request_body);
        assert_eq!(hashes.len(), s.request_parts as usize);
        for h in hashes {
            assert!(
                rows.iter().any(|p| p.project_id == s.project_id
                    && p.hash == h
                    && p.first_at <= s.started_at
                    && s.started_at <= p.last_at + HOUR
                    && p.expires_at >= s.expires_at),
                "request {} started {} names a part no row keeps for it",
                s.request_id,
                s.started_at
            );
        }
    }
}

async fn entry_text(env: &Env, project: &str, id: &str) -> String {
    let req = Request::builder()
        .uri(format!(
            "/platform/v1/orgs/{}/projects/{project}/logs/{id}",
            env.owner.org_id
        ))
        .header(
            "cookie",
            format!("__Host-kaeso_session={}", env.owner.cookie),
        )
        .body(Body::empty())
        .unwrap();
    let (status, _, bytes) = env.app.raw_bytes(req).await;
    assert_eq!(status, StatusCode::OK);
    String::from_utf8(bytes).unwrap()
}

async fn export_text(env: &Env) -> String {
    let req = Request::builder()
        .uri(format!(
            "/platform/v1/orgs/{}/projects/{}/logs/export?include_content=true",
            env.owner.org_id, env.project
        ))
        .header(
            "cookie",
            format!("__Host-kaeso_session={}", env.owner.cookie),
        )
        .body(Body::empty())
        .unwrap();
    let (status, _, bytes) = env.app.raw_bytes(req).await;
    assert_eq!(status, StatusCode::OK);
    String::from_utf8(bytes).unwrap()
}

async fn reads_back(env: &Env, id: &str, body: &str) {
    let text = entry_text(env, &env.project, id).await;
    assert!(
        text.contains(&format!("\"request\":{body},\"reply\":")),
        "the entry holds the body byte for byte: {:.300}",
        text
    );
}

#[tokio::test]
async fn every_named_format_stores_long_history_once_and_reads_it_back_byte_for_byte() {
    let env = env().await;
    let mut sent = Vec::new();
    for (i, format) in [
        Format::Anthropic,
        Format::Chat,
        Format::Responses,
        Format::Gemini,
    ]
    .into_iter()
    .enumerate()
    {
        let seed = 1000 * (i as u64 + 1);
        let before = part_rows(&env).await.len();
        let first = format.body(3, seed);
        let a = send(&env, format, &first).await;
        flush(&env).await;
        let s = stored_one(&env, &a).await;
        assert_eq!(
            s.request_parts, 4,
            "{format:?}: the system prompt and three messages"
        );
        assert!(
            s.request_body.contains("short and inline"),
            "{format:?}: a short message stays in the body"
        );
        assert!(s.request_body.len() < first.len() / 3, "{format:?}");
        assert_eq!(part_rows(&env).await.len(), before + 4, "{format:?}");

        env.app.clock.advance(time::Duration::minutes(5));
        let second = format.body(5, seed);
        let b = send(&env, format, &second).await;
        flush(&env).await;
        assert_eq!(stored_one(&env, &b).await.request_parts, 6, "{format:?}");
        assert_eq!(
            part_rows(&env).await.len(),
            before + 6,
            "{format:?}: the next turn writes only its two new messages"
        );
        reads_back(&env, &a, &first).await;
        reads_back(&env, &b, &second).await;
        sent.push(first);
        sent.push(second);
    }
    let export = export_text(&env).await;
    for body in &sent {
        assert!(
            export.contains(&format!("\"request\":{body},\"reply\":")),
            "the export holds every body byte for byte"
        );
    }
    let logged = ch::search(
        clickhouse(&env),
        uuid_of(&env.owner.org_id),
        uuid_of(&env.project),
        &ch::Filter::default(),
        None,
        100,
    )
    .await
    .unwrap();
    let contents = ch::contents(
        clickhouse(&env),
        uuid_of(&env.owner.org_id),
        uuid_of(&env.project),
        &logged.iter().map(|l| l.request_id).collect::<Vec<_>>(),
    )
    .await
    .unwrap();
    for l in &logged {
        let c = contents
            .iter()
            .find(|c| c.request_id == l.request_id)
            .unwrap();
        assert_eq!(
            l.logical_bytes,
            (c.request_body.len() + c.reply.len()) as u64,
            "logical bytes count the body as received"
        );
        assert!(sent.contains(&c.request_body));
    }
    every_body_keeps_its_parts(&env).await;
}

#[derive(Debug, Row, Deserialize)]
struct Size {
    compressed: u64,
}

async fn content_on_disk(env: &Env) -> u64 {
    let ch = clickhouse(env);
    for table in ["request_content", "request_content_part"] {
        ch.client()
            .query("OPTIMIZE TABLE ? FINAL")
            .bind(clickhouse::sql::Identifier(table))
            .execute()
            .await
            .unwrap();
    }
    ch.client()
        .query(
            "SELECT toUInt64(sum(data_compressed_bytes)) AS compressed FROM system.parts \
             WHERE active AND database = currentDatabase() AND table IN ('request_content', 'request_content_part')",
        )
        .fetch_one::<Size>()
        .await
        .unwrap()
        .compressed
}

#[tokio::test]
async fn a_long_session_stores_its_content_linearly() {
    let env = env().await;
    let mut logical = 0u64;
    let mut at = Vec::new();
    for turn in 1..=80u64 {
        let body = Format::Chat.body(turn, 7);
        logical += body.len() as u64;
        send(&env, Format::Chat, &body).await;
        env.app.clock.advance(time::Duration::seconds(20));
        if turn % 40 == 0 {
            flush(&env).await;
            at.push((logical, content_on_disk(&env).await));
        }
    }
    let [(logical_40, disk_40), (logical_80, disk_80)] = at[..] else {
        panic!()
    };
    assert!(
        logical_80 > 3 * logical_40,
        "as received, a session grows with the square of its turns: {logical_40} then {logical_80}"
    );
    assert!(
        disk_80 * 10 < logical_80,
        "stored, it is a fraction of that: {disk_80} of {logical_80}"
    );
    assert!(
        disk_80 < disk_40 * 5 / 2,
        "and twice the turns take about twice the space: {disk_40} then {disk_80}"
    );
    let parts: usize = part_rows(&env).await.iter().map(|p| p.body.len()).sum();
    assert_eq!(
        part_rows(&env).await.len(),
        81,
        "each message and the system prompt once"
    );
    assert!(parts < 81 * 1000, "{parts}");
    let storage = env
        .app
        .get(
            &format!("/orgs/{}/logs/storage", env.owner.org_id),
            &env.owner.cookie,
        )
        .await;
    assert_eq!(storage.status, StatusCode::OK);
    let logical_bytes = storage.body["logical_bytes"].as_u64().unwrap();
    let estimated = storage.body["estimated_disk_bytes"].as_u64().unwrap();
    assert!(
        logical_bytes >= logical_80,
        "logical bytes stay as received"
    );
    assert!(
        estimated < logical_bytes / 5,
        "the estimate counts parts once: {estimated} of {logical_bytes}"
    );
    every_body_keeps_its_parts(&env).await;
}

#[tokio::test]
async fn a_part_is_written_again_only_after_the_interval_and_never_for_another_project() {
    let env = env().await;
    let body = Format::Chat.body(2, 50);
    send(&env, Format::Chat, &body).await;
    flush(&env).await;
    assert_eq!(part_rows(&env).await.len(), 3);

    let r = env
        .app
        .call(
            axum::http::Method::PATCH,
            &format!("/orgs/{}/logs/settings", env.owner.org_id),
            Some(json!({ "content_retention_days": 60 })),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    env.app.clock.advance(time::Duration::minutes(1));
    let longer = send(&env, Format::Chat, &body).await;
    flush(&env).await;
    let rows = part_rows(&env).await;
    assert_eq!(
        rows.len(),
        6,
        "a longer retention moves the parts' expiry at once"
    );
    let expires = stored_one(&env, &longer).await.expires_at;
    assert_eq!(
        rows.iter()
            .filter(|p| p.expires_at == expires + HOUR)
            .count(),
        3
    );

    env.app.clock.advance(time::Duration::minutes(30));
    send(&env, Format::Chat, &body).await;
    flush(&env).await;
    assert_eq!(part_rows(&env).await.len(), 6, "the cache knows them");

    env.app.clock.advance(time::Duration::minutes(31));
    let late = send(&env, Format::Chat, &body).await;
    flush(&env).await;
    let rows = part_rows(&env).await;
    assert_eq!(
        rows.len(),
        9,
        "after the interval each part is written once more"
    );
    let started = stored_one(&env, &late).await.started_at;
    assert_eq!(rows.iter().filter(|p| p.last_at == started).count(), 3);

    let other =
        common::create_project(&env.app, &env.owner.org_id, &env.owner.cookie, "Other").await;
    let base = format!("/orgs/{}/projects/{other}", env.owner.org_id);
    let r = env
        .app
        .post(
            &format!("{base}/models"),
            json!({ "provider": "anthropic", "model_id": "claude-haiku-4-5" }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let key = env
        .app
        .post(
            &format!("{base}/keys"),
            json!({ "name": "other" }),
            Some(&env.owner.cookie),
        )
        .await
        .body["secret"]
        .as_str()
        .unwrap()
        .to_owned();
    let theirs = send_to(&env, &key, Format::Chat, &body).await;
    flush(&env).await;
    let rows = part_rows(&env).await;
    assert_eq!(
        rows.iter()
            .filter(|p| p.project_id == uuid_of(&other))
            .count(),
        3,
        "another project writes its own parts"
    );
    let text = entry_text(&env, &other, &theirs).await;
    assert!(text.contains(&format!("\"request\":{body},\"reply\":")));
    every_body_keeps_its_parts(&env).await;
}

async fn eventually_parts(env: &Env, want: usize) {
    let mut n = usize::MAX;
    for _ in 0..100 {
        n = part_rows(env).await.len();
        if n == want {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(n, want, "parts left");
}

fn rfc3339(t: OffsetDateTime) -> String {
    t.format(&time::format_description::well_known::Rfc3339)
        .unwrap()
}

#[tokio::test]
async fn deleting_a_range_keeps_parts_that_requests_outside_it_still_need() {
    let env = env().await;
    let body = Format::Anthropic.body(2, 70);
    let t = env.app.clock.now();
    let first = send(&env, Format::Anthropic, &body).await;
    env.app.clock.advance(time::Duration::minutes(12));
    let second = send(&env, Format::Anthropic, &body).await;
    flush(&env).await;
    assert_eq!(
        part_rows(&env).await.len(),
        3,
        "the second turn relies on the first's parts"
    );

    let r = env
        .app
        .post(
            &format!("/orgs/{}/logs/deletions", env.owner.org_id),
            json!({ "from": rfc3339(t - time::Duration::minutes(1)), "to": rfc3339(t + time::Duration::minutes(10)) }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::ACCEPTED, "{:?}", r.body);
    kaeso_server::services::logs::run_jobs(&env.app.state)
        .await
        .unwrap();
    let mut left = Vec::new();
    for _ in 0..100 {
        left = stored(&env).await;
        if left.len() == 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].request_id, uuid_of(&second));
    assert_eq!(
        part_rows(&env).await.len(),
        3,
        "the turn outside the range still needs them"
    );
    reads_back(&env, &second, &body).await;
    every_body_keeps_its_parts(&env).await;
    let _ = first;

    let org = uuid_of(&env.owner.org_id);
    let project = uuid_of(&env.project);
    ch::delete_range(
        clickhouse(&env),
        org,
        Some(project),
        t - time::Duration::minutes(1),
        t + time::Duration::minutes(40),
        true,
    )
    .await
    .unwrap();
    for _ in 0..100 {
        if stored(&env).await.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(stored(&env).await.is_empty(), "the content is gone");
    assert_eq!(
        part_rows(&env).await.len(),
        3,
        "a request up to an hour after the part's last write may still name it"
    );
    ch::delete_range(
        clickhouse(&env),
        org,
        Some(project),
        t - time::Duration::minutes(1),
        t + 2 * HOUR,
        true,
    )
    .await
    .unwrap();
    eventually_parts(&env, 0).await;
    let logs = ch::search(
        clickhouse(&env),
        org,
        project,
        &ch::Filter::default(),
        None,
        10,
    )
    .await
    .unwrap();
    assert_eq!(logs.len(), 1, "deleting content keeps the metadata");
}

#[tokio::test]
async fn deleting_the_org_and_ttl_remove_parts() {
    let env = env().await;
    send(&env, Format::Responses, &Format::Responses.body(2, 90)).await;
    flush(&env).await;
    assert_eq!(part_rows(&env).await.len(), 3);
    let org = uuid_of(&env.owner.org_id);
    let project = uuid_of(&env.project);
    let past = env.app.clock.now() - time::Duration::days(400);
    ch_parts::insert(
        clickhouse(&env),
        &[PartRow {
            org_id: org,
            project_id: project,
            hash: [7; 16],
            body: "\"expired\"".into(),
            first_at: past,
            last_at: past,
            expires_at: past + time::Duration::days(1),
        }],
    )
    .await
    .unwrap();
    clickhouse(&env)
        .client()
        .query("OPTIMIZE TABLE request_content_part FINAL")
        .execute()
        .await
        .unwrap();
    assert!(
        part_rows(&env).await.iter().all(|p| p.hash != [7; 16]),
        "a part past its expiry is dropped"
    );
    ch::delete_org(clickhouse(&env), org).await.unwrap();
    eventually_parts(&env, 0).await;
}

#[tokio::test]
async fn bodies_stored_whole_read_as_they_were() {
    let env = env().await;
    let org = uuid_of(&env.owner.org_id);
    let project = uuid_of(&env.project);
    let at = env.app.clock.now().replace_nanosecond(0).unwrap();
    let (log, mut content) = crate::logs::row(org, project, at, 1);
    let lookalike = format!(
        r#"{{"messages":[{}], "n": 1}}"#,
        ch_parts::placeholder(&[1; 16])
    );
    content.request_body = lookalike.clone();
    let mut with_parts = log.clone();
    with_parts.has_content = true;
    ch::insert_logs(clickhouse(&env), [&with_parts])
        .await
        .unwrap();
    ch::insert_contents(clickhouse(&env), [&content])
        .await
        .unwrap();
    let read = ch::contents(clickhouse(&env), org, project, &[log.request_id])
        .await
        .unwrap();
    assert_eq!(
        read[0].request_body, lookalike,
        "a whole body is never spliced"
    );

    let body = format!(
        r#"{{"model":"claude-haiku-4-5","max_tokens":64,"messages":[{{"role":"user","content":"{}","x":{{"$kaeso_part":"not ours"}}}}]}}"#,
        noise(3, 900)
    );
    let id = send(&env, Format::Anthropic, &body).await;
    flush(&env).await;
    assert_eq!(
        stored_one(&env, &id).await.request_parts,
        0,
        "a body that already names the key is stored whole"
    );
    reads_back(&env, &id, &body).await;
}
