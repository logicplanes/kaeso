#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use crate::common;

use axum::http::StatusCode;
use base64::Engine as _;
use common::v1::Env;
use futures::{SinkExt, StreamExt};
use kaeso_server::model_api::hooks::Outcome;
use kaeso_testkit::mock_realtime::{REALTIME_AUDIO, REALTIME_TRANSCRIPT};
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::{Error as WsError, Message};

const ELEVENLABS_SECRET: &str = "sk_test-elevenlabs-realtime-0000000000000";
const VOICE: &str = "hpp4J3VqNfWAUOO0d1Us";

type Client =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn env() -> (Env, String) {
    let capture = std::sync::Arc::new(common::v1::Capture::default());
    let env = common::v1::env(Some(capture), None).await;
    let org = &env.owner.org_id;
    let r = env
        .app
        .post(
            &format!("/orgs/{org}/credentials"),
            json!({ "provider": "elevenlabs", "name": "ElevenLabs", "secret": ELEVENLABS_SECRET }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    for (provider, model) in [
        ("elevenlabs", "eleven_flash_v2_5"),
        ("elevenlabs", "eleven_v3"),
        ("elevenlabs", "scribe_v2_realtime"),
        ("anthropic", "claude-haiku-4-5"),
    ] {
        let r = env
            .app
            .post(
                &format!("/orgs/{org}/projects/{}/models", env.project),
                json!({ "provider": provider, "model_id": model }),
                Some(&env.owner.cookie),
            )
            .await;
        assert!(
            matches!(r.status, StatusCode::CREATED | StatusCode::CONFLICT),
            "{model}: {:?}",
            r.body
        );
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let router = env.app.router.clone();
    tokio::spawn(async move { axum::serve(listener, router).await });
    (env, format!("ws://{addr}"))
}

async fn open(base: &str, path: &str, key: Option<&str>) -> Result<(Client, String), WsError> {
    let mut req = format!("{base}{path}").into_client_request().unwrap();
    if let Some(k) = key {
        req.headers_mut().insert("xi-api-key", k.parse().unwrap());
    }
    let (ws, resp) = tokio_tungstenite::connect_async(req).await?;
    let id = resp
        .headers()
        .get("kaeso-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    Ok((ws, id))
}

fn refusal(err: WsError) -> (u16, Value) {
    let WsError::Http(resp) = err else {
        panic!("{err:?}")
    };
    let status = resp.status().as_u16();
    let body = resp.into_body().unwrap_or_default();
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}

async fn send(ws: &mut Client, v: Value) {
    ws.send(Message::text(v.to_string())).await.unwrap();
}

async fn until_closed(ws: &mut Client) -> (Vec<Value>, Option<u16>) {
    let mut got = Vec::new();
    let mut code = None;
    while let Some(Ok(msg)) = ws.next().await {
        match msg {
            Message::Text(t) => got.push(serde_json::from_str(t.as_str()).unwrap()),
            Message::Close(frame) => {
                code = frame.map(|f| u16::from(f.code));
                break;
            }
            _ => {}
        }
    }
    (got, code)
}

async fn outcome(env: &Env, id: &str) -> Outcome {
    tokio::time::timeout(std::time::Duration::from_secs(10), env.capture.recorded(id))
        .await
        .expect("the session was logged")
}

fn sent_messages(env: &Env, path: &str) -> Vec<Value> {
    env.mock
        .received()
        .into_iter()
        .filter(|r| r.method == "WS" && r.path == path)
        .map(|r| r.json())
        .collect()
}

#[tokio::test]
async fn text_to_speech_streams_through_with_the_projects_key_and_is_billed_per_character() {
    let (env, base) = env().await;
    let path = format!("/v1/text-to-speech/{VOICE}/stream-input");
    let (mut ws, id) = open(
        &base,
        &format!("{path}?model_id=eleven_flash_v2_5&output_format=mp3_44100_128"),
        Some(&env.key),
    )
    .await
    .unwrap();
    send(
        &mut ws,
        json!({"text": " ", "voice_settings": {"stability": 0.5}}),
    )
    .await;
    send(&mut ws, json!({"text": "Hello there. "})).await;
    send(&mut ws, json!({"text": ""})).await;
    let (got, code) = until_closed(&mut ws).await;
    assert_eq!(code, Some(1000));
    let audio = base64::engine::general_purpose::STANDARD
        .decode(got[0]["audio"].as_str().unwrap())
        .unwrap();
    assert_eq!(audio, REALTIME_AUDIO);
    assert_eq!(got.last().unwrap()["isFinal"], true);

    let handshake = env
        .mock
        .received()
        .into_iter()
        .find(|r| r.method == "GET" && r.path == path)
        .unwrap();
    assert_eq!(handshake.header("xi-api-key"), Some(ELEVENLABS_SECRET));
    assert_eq!(
        handshake.query.as_deref(),
        Some("model_id=eleven_flash_v2_5&output_format=mp3_44100_128")
    );
    assert_eq!(sent_messages(&env, &path).len(), 3);

    let o = outcome(&env, &id).await;
    assert_eq!(
        kaeso_server::services::logs::endpoint(&o),
        "text-to-speech/stream-input"
    );
    assert_eq!(o.status, 101);
    assert_eq!(o.error_kind, None);
    let media = o.media.clone().unwrap();
    assert_eq!(media.characters, 14);
    assert_eq!(media.format.as_deref(), Some("mp3_44100_128"));
    assert_eq!(
        kaeso_server::services::usage::costed(&o).1,
        "0.00056".parse::<rust_decimal::Decimal>().unwrap(),
        "14 characters at 40 per million"
    );
}

#[tokio::test]
async fn a_key_in_the_first_message_is_checked_by_kaeso_and_never_reaches_elevenlabs() {
    let (env, base) = env().await;
    let path = format!("/v1/text-to-speech/{VOICE}/multi-stream-input");
    let (mut ws, _) = open(&base, &format!("{path}?model_id=eleven_flash_v2_5"), None)
        .await
        .unwrap();
    send(
        &mut ws,
        json!({"text": " ", "context_id": "a", "xi_api_key": env.key}),
    )
    .await;
    send(&mut ws, json!({"text": "Hi. ", "context_id": "a"})).await;
    send(&mut ws, json!({"close_socket": true})).await;
    let (got, code) = until_closed(&mut ws).await;
    assert_eq!(code, Some(1000));
    assert_eq!(got[0]["contextId"], "a");
    let sent = sent_messages(&env, &path);
    assert_eq!(sent[0], json!({"text": " ", "context_id": "a"}));
    assert!(!sent.iter().any(|m| m.to_string().contains(&env.key)));

    let (mut ws, _) = open(&base, &format!("{path}?model_id=eleven_flash_v2_5"), None)
        .await
        .unwrap();
    send(
        &mut ws,
        json!({"text": " ", "xi_api_key": "sk-kso-not-a-key"}),
    )
    .await;
    let (got, code) = until_closed(&mut ws).await;
    assert_eq!(code, Some(1008));
    assert_eq!(got[0]["error"], "invalid_api_key");
    assert!(got[0]["message"].is_string());
}

#[tokio::test]
async fn realtime_speech_to_text_is_billed_by_the_audio_it_hears() {
    let (env, base) = env().await;
    let path = "/v1/speech-to-text/realtime";
    let (mut ws, id) = open(
        &base,
        &format!(
            "{path}?model_id=scribe_v2_realtime&audio_format=pcm_16000&commit_strategy=manual"
        ),
        Some(&env.key),
    )
    .await
    .unwrap();
    let first = ws.next().await.unwrap().unwrap();
    assert!(first.to_text().unwrap().contains("session_started"));
    let second = base64::engine::general_purpose::STANDARD.encode(vec![0u8; 32_000]);
    send(&mut ws, json!({"message_type": "input_audio_chunk", "audio_base_64": second, "commit": false, "sample_rate": 16000})).await;
    send(&mut ws, json!({"message_type": "input_audio_chunk", "audio_base_64": "", "commit": true, "sample_rate": 16000})).await;
    let mut committed = None;
    while let Some(Ok(Message::Text(t))) = ws.next().await {
        let v: Value = serde_json::from_str(t.as_str()).unwrap();
        if v["message_type"] == "committed_transcript" {
            committed = Some(v);
            break;
        }
    }
    assert_eq!(committed.unwrap()["text"], REALTIME_TRANSCRIPT);
    ws.close(None).await.unwrap();
    let o = outcome(&env, &id).await;
    assert_eq!(
        kaeso_server::services::logs::endpoint(&o),
        "speech-to-text/realtime"
    );
    assert_eq!(o.media.clone().unwrap().input_ms, 1000);
    assert!(kaeso_server::services::usage::costed(&o).1 > rust_decimal::Decimal::ZERO);
}

#[tokio::test]
async fn dialogue_sockets_count_every_lines_characters() {
    let (env, base) = env().await;
    let path = "/v1/text-to-dialogue/stream-input";
    let (mut ws, id) = open(&base, &format!("{path}?model_id=eleven_v3"), Some(&env.key))
        .await
        .unwrap();
    send(&mut ws, json!({"voices": [VOICE]})).await;
    send(&mut ws, json!({"inputs": [{"text": "Hi.", "voice_id": VOICE}, {"text": "Yes?", "voice_id": VOICE}]})).await;
    send(&mut ws, json!({"close_socket": true})).await;
    let (got, code) = until_closed(&mut ws).await;
    assert_eq!(code, Some(1000));
    assert_eq!(got.last().unwrap()["is_final"], true);
    assert_eq!(outcome(&env, &id).await.media.unwrap().characters, 7);
}

#[tokio::test]
async fn what_kaeso_cannot_open_is_refused_in_elevenlabs_shape_before_the_upgrade() {
    let (env, base) = env().await;
    let tts = format!("/v1/text-to-speech/{VOICE}/stream-input");
    let stt = "/v1/speech-to-text/realtime?model_id=scribe_v2_realtime";
    for (path, key, status) in [
        (format!("{stt}&audio_format=pcm_16000"), None, 401),
        (format!("{stt}&token=abc"), Some(env.key.as_str()), 401),
        (tts.clone(), Some(env.key.as_str()), 400),
        (
            format!("{tts}?model_id=claude-haiku-4-5"),
            Some(env.key.as_str()),
            400,
        ),
        (format!("{tts}?model_id=nope"), Some(env.key.as_str()), 404),
        (
            "/v1/text-to-speech/x..y/stream-input?model_id=eleven_v3".to_owned(),
            Some(env.key.as_str()),
            400,
        ),
        (
            format!("{tts}?model_id=eleven_flash_v2_5"),
            Some("sk-kso-wrong"),
            401,
        ),
    ] {
        let err = open(&base, &path, key).await.err().unwrap();
        let (got, body) = refusal(err);
        assert_eq!(got, status, "{path}: {body}");
        assert!(body["detail"]["message"].is_string(), "{path}: {body}");
    }
    assert!(
        !env.mock
            .received()
            .iter()
            .any(|r| r.method == "GET" && r.path.contains("stream-input")),
        "ElevenLabs was never asked"
    );
}

#[tokio::test]
async fn a_session_counts_against_the_keys_request_rate() {
    let (env, base) = env().await;
    let org = &env.owner.org_id;
    let projects = format!("/orgs/{org}/projects/{}", env.project);
    let key = env
        .app
        .post(
            &format!("{projects}/keys"),
            json!({ "name": "rated" }),
            Some(&env.owner.cookie),
        )
        .await
        .body;
    let r = env
        .app
        .post(
            &format!("{projects}/limits"),
            json!({ "level": "key", "key": key["id"], "kind": "request_rate", "period": "minute", "amount": 1 }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let rated = key["secret"].as_str().unwrap();
    let path = format!("/v1/text-to-speech/{VOICE}/stream-input?model_id=eleven_flash_v2_5");
    let (mut ws, _) = open(&base, &path, Some(rated)).await.unwrap();
    send(&mut ws, json!({"text": ""})).await;
    until_closed(&mut ws).await;
    let (status, body) = refusal(open(&base, &path, Some(rated)).await.err().unwrap());
    assert_eq!(status, 429, "{body}");
    assert_eq!(body["detail"]["type"], "rate_limit_error");
}

#[tokio::test]
async fn a_message_elevenlabs_refuses_closes_the_session_and_is_logged_as_failed() {
    let (env, base) = env().await;
    let path = "/v1/speech-to-text/realtime?model_id=scribe_v2_realtime&audio_format=pcm_16000";
    let (mut ws, id) = open(&base, path, Some(&env.key)).await.unwrap();
    ws.next().await.unwrap().unwrap();
    send(&mut ws, json!({"message_type": "input_audio_chunk", "audio_base_64": "", "previous_text": "[mock:error]"})).await;
    let (got, code) = until_closed(&mut ws).await;
    assert_eq!(code, Some(1008));
    assert_eq!(got[0]["message_type"], "input_error");
    let o = outcome(&env, &id).await;
    assert_eq!(o.status, 101);
    assert_eq!(o.error_kind.as_deref(), Some("provider_error"));
    assert!(o.error_message.unwrap().contains("1008"));

    let (mut ws, _) = open(&base, path, Some(&env.key)).await.unwrap();
    ws.next().await.unwrap().unwrap();
    ws.send(Message::text("not json")).await.unwrap();
    let (got, code) = until_closed(&mut ws).await;
    assert_eq!(code, Some(1008));
    assert_eq!(got[0]["message_type"], "invalid_request");
}
