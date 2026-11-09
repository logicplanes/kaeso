use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{OriginalUri, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use serde_json::{Value, json};

use crate::mock_provider::{Ctx, Received};

/// The bytes the mock's speech sockets send as audio.
pub const REALTIME_AUDIO: &[u8] = b"mock-audio";

/// The transcript the mock's realtime speech to text commits.
pub const REALTIME_TRANSCRIPT: &str = "Hello from the mock.";

// What the mock's sockets hear, for tests to read back: the handshake as a `GET` and each
// message from Kaeso as a `WS` entry.
fn record(
    ctx: &Ctx,
    method: &str,
    path: &str,
    query: Option<&str>,
    headers: &HeaderMap,
    body: &str,
) {
    let headers = headers
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or_default().to_owned()))
        .collect();
    ctx.record(Received {
        method: method.to_owned(),
        path: path.to_owned(),
        query: query.map(str::to_owned),
        headers,
        body: Bytes::from(body.to_owned()),
    });
}

/// ElevenLabs' realtime sockets: speech from text in one or several contexts, dialogue, and
/// speech to text. A key named `refuse` is refused at the handshake, and a message holding
/// `[mock:error]` gets ElevenLabs' error message and a close.
pub(crate) async fn socket(
    State(ctx): State<Ctx>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    let path = uri.path().to_owned();
    record(&ctx, "GET", &path, uri.query(), &headers, "");
    let key = headers
        .get("xi-api-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    if key.is_empty() || key == "refuse" {
        return (
            StatusCode::UNAUTHORIZED,
            axum::Json(
                json!({"detail": {"status": "invalid_api_key", "message": "Invalid API key"}}),
            ),
        )
            .into_response();
    }
    upgrade.on_upgrade(move |ws| serve(ws, ctx, path))
}

fn audio() -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(REALTIME_AUDIO)
}

async fn send(ws: &mut WebSocket, v: Value) -> bool {
    ws.send(Message::Text(v.to_string().into())).await.is_ok()
}

async fn close(ws: &mut WebSocket, code: u16, reason: &str) {
    let _ = ws
        .send(Message::Close(Some(CloseFrame {
            code,
            reason: reason.into(),
        })))
        .await;
}

async fn serve(mut ws: WebSocket, ctx: Ctx, path: String) {
    let transcribes = path.ends_with("/speech-to-text/realtime");
    let dialogue = path.contains("/text-to-dialogue/");
    let contexts = path.ends_with("/multi-stream-input");
    if transcribes
        && !send(
            &mut ws,
            json!({"message_type": "session_started", "session_id": "mock-session", "config": {}}),
        )
        .await
    {
        return;
    }
    while let Some(Ok(msg)) = ws.recv().await {
        let Message::Text(text) = msg else {
            if matches!(msg, Message::Close(_)) {
                return;
            }
            continue;
        };
        record(&ctx, "WS", &path, None, &HeaderMap::new(), text.as_str());
        let v: Value = serde_json::from_str(text.as_str()).unwrap_or(Value::Null);
        if text.as_str().contains("[mock:error]") {
            let body = if transcribes {
                json!({"message_type": "input_error", "error": "The mock refuses this message."})
            } else {
                json!({"message": "The mock refuses this message.", "error": "invalid_request", "code": 1008, "param": null})
            };
            send(&mut ws, body).await;
            close(&mut ws, 1008, "refused").await;
            return;
        }
        if transcribes {
            let reply = if v["commit"] == true {
                json!({"message_type": "committed_transcript", "text": REALTIME_TRANSCRIPT})
            } else {
                json!({"message_type": "partial_transcript", "text": "Hello"})
            };
            if !send(&mut ws, reply).await {
                return;
            }
            continue;
        }
        let ends = if contexts || dialogue {
            v["close_socket"] == true
        } else {
            v["text"] == ""
        };
        if ends {
            let last = if dialogue {
                json!({"is_final": true})
            } else {
                json!({"isFinal": true})
            };
            send(&mut ws, last).await;
            close(&mut ws, 1000, "").await;
            return;
        }
        let speaks = if dialogue {
            v["inputs"].as_array().is_some_and(|i| !i.is_empty())
        } else {
            v["text"].as_str().is_some_and(|t| !t.trim().is_empty())
        };
        if speaks {
            let mut chunk = if dialogue {
                json!({"audio": audio(), "alignment": null, "is_final_audio_for_turn": false})
            } else {
                json!({"audio": audio(), "isFinal": null, "normalizedAlignment": null, "alignment": null})
            };
            if contexts {
                chunk["contextId"] = v["context_id"].clone();
            }
            if !send(&mut ws, chunk).await {
                return;
            }
        }
    }
}
