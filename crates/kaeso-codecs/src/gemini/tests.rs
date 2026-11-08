use kaeso_kpp::{
    Block, DialectId, ErrorKind, Extras, GenerateRequest, Message, OpaqueKind, Origin, Reply,
    Request, RequestKind, StopReason, StreamEvent, StreamMode,
};
use serde_json::{Value, json};

use super::*;
use crate::test_support::{assert_json_eq, assert_sse_eq, encoded_error, fixtures, sidecar};

const DIR: &str = "gemini";
const MODEL: &str = "gemini-3.5-flash";

fn call_of(folder: &str, name: &str) -> (String, GeminiAction) {
    let meta = sidecar(DIR, folder, name, "meta").unwrap_or(Value::Null);
    let model = meta["model"].as_str().unwrap_or(MODEL).to_owned();
    let default = if folder.ends_with("count_tokens") {
        GeminiAction::CountTokens
    } else {
        GeminiAction::GenerateContent
    };
    let action = meta["action"]
        .as_str()
        .map_or(default, |a| GeminiAction::from_wire(a).unwrap());
    (model, action)
}

fn decode_fixture(folder: &str, name: &str, body: &[u8]) -> Request {
    let (model, action) = call_of(folder, name);
    crate::dispatch::decode_gemini_request(&model, action, body, &HeaderList::new())
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn generate_ctx() -> EgressCtx {
    EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off)
}

fn stream_ctx() -> EgressCtx {
    EgressCtx::for_kind(RequestKind::Generate, StreamMode::on(true))
}

fn decode_stream(bytes: &[u8], size: usize) -> Vec<StreamEvent> {
    let mut dec = Gemini::stream_decoder(&stream_ctx());
    let mut events = Vec::new();
    if size == 0 {
        events.extend(dec.feed(bytes));
    } else {
        for chunk in bytes.chunks(size) {
            events.extend(dec.feed(chunk));
        }
    }
    events.extend(dec.finish());
    events
}

fn encode_stream(events: &[StreamEvent]) -> Vec<u8> {
    let mut enc = Gemini::stream_encoder(&EncodeCtx::new(
        RequestKind::Generate,
        StreamMode::on(true),
    ));
    let mut out = Vec::new();
    for e in events {
        out.extend(enc.encode(e).unwrap());
    }
    out.extend(enc.finish().unwrap());
    out
}

#[test]
fn every_request_fixture_round_trips_and_names_its_path() {
    for folder in ["requests", "requests/count_tokens"] {
        for (name, body) in fixtures(DIR, folder, "json") {
            let req = decode_fixture(folder, &name, &body);
            let (model, action) = call_of(folder, &name);
            let mut ctx = EgressCtx::new(&req);
            let enc = Gemini::encode_request(&req, &mut ctx)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(
                enc.path,
                GeminiProfile::GOOGLE_AI_STUDIO.path(&model, action).unwrap(),
                "{name}"
            );
            assert_json_eq(&name, &body, &enc.body);
            assert!(ctx.adapted.is_empty(), "{name}: {:?}", ctx.adapted);
        }
    }
}

#[test]
fn every_response_fixture_round_trips() {
    for (folder, kind) in [
        ("responses", RequestKind::Generate),
        ("responses/count_tokens", RequestKind::CountTokens),
    ] {
        for (name, body) in fixtures(DIR, folder, "json") {
            let ctx = EgressCtx::for_kind(kind, StreamMode::Off);
            let reply = Gemini::decode_response(200, &HeaderList::new(), &body, &ctx)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            let out = Gemini::encode_response(&reply, &EncodeCtx::new(kind, StreamMode::Off))
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_json_eq(&name, &body, &out);
        }
    }
}

#[test]
fn every_stream_fixture_round_trips_under_every_chunking() {
    for (name, bytes) in fixtures(DIR, "streams", "sse") {
        let whole = decode_stream(&bytes, 0);
        for size in [1, 7, 64] {
            assert_eq!(decode_stream(&bytes, size), whole, "{name}: chunks of {size}");
        }
        assert!(
            !whole.iter().any(|e| matches!(e, StreamEvent::Error(_))),
            "{name}: {whole:?}"
        );
        assert_sse_eq(&name, &bytes, &encode_stream(&whole));
    }
}

#[test]
fn every_error_fixture_round_trips() {
    for (name, body) in fixtures(DIR, "errors", "json") {
        let meta = sidecar(DIR, "errors", &name, "meta").unwrap();
        let status = u16::try_from(meta["status"].as_u64().unwrap()).unwrap();
        let err = Gemini::decode_error(status, &HeaderList::new(), &body);
        assert_eq!(err.status, Some(status), "{name}");
        let out = Gemini::encode_error(&err);
        assert_eq!(out.status, status, "{name}");
        assert_json_eq(&name, &encoded_error(&meta, &body), &out.body);
    }
}

#[test]
fn every_model_fixture_round_trips() {
    for (name, body) in fixtures(DIR, "models", "json") {
        let v: Value = serde_json::from_slice(&body).unwrap();
        let out = if v.get("models").is_some() {
            let list = Gemini::decode_models(200, &HeaderList::new(), &body).unwrap();
            Gemini::encode_models(&list).unwrap()
        } else {
            let model = Gemini::decode_model(200, &HeaderList::new(), &body).unwrap();
            Gemini::encode_model(&model).unwrap()
        };
        assert_json_eq(&name, &body, &out);
    }
}
