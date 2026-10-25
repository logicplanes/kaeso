use kaeso_kpp::{Block, ErrorKind, Reply, Request, RequestKind, Response, StreamEvent, StreamMode};

use super::OpenAiChat;
use crate::codec::{Codec, EgressCtx, EncodeCtx, HeaderList};
use crate::test_support::{assert_json_eq, assert_sse_eq, fixtures};

const D: &str = "openai_chat";
const KEEP_ALIVE: &[u8] = b": keep-alive\n\n";

fn fixture(folder: &str, extension: &str, name: &str) -> Vec<u8> {
    let wanted = format!("providers/deepseek/{name}");
    fixtures(D, folder, extension)
        .into_iter()
        .find(|(n, _)| *n == wanted)
        .unwrap_or_else(|| panic!("no fixture {folder}/{wanted}"))
        .1
}

fn reply(body: &[u8]) -> Response {
    let ctx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off);
    match OpenAiChat::decode_response(200, &HeaderList::new(), body, &ctx).unwrap() {
        Reply::Generate(r) => r,
        other => panic!("{other:?}"),
    }
}

fn encode_reply(resp: &Response) -> Vec<u8> {
    let ctx = EncodeCtx::new(RequestKind::Generate, StreamMode::Off);
    OpenAiChat::encode_response(&Reply::Generate(resp.clone()), &ctx).unwrap()
}

fn decode_stream(bytes: &[u8]) -> Vec<StreamEvent> {
    let ctx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::on(true));
    let mut decoder = OpenAiChat::stream_decoder(&ctx);
    let mut events = decoder.feed(bytes);
    events.extend(decoder.finish());
    events
}

fn encode_stream(events: &[StreamEvent]) -> Vec<u8> {
    let ctx = EncodeCtx::new(RequestKind::Generate, StreamMode::on(true));
    let mut encoder = OpenAiChat::stream_encoder(&ctx);
    let mut out = Vec::new();
    for event in events {
        out.extend(encoder.encode(event).unwrap());
    }
    out.extend(encoder.finish().unwrap());
    out
}

fn is_extra(event: &StreamEvent) -> bool {
    matches!(event, StreamEvent::Extra(_))
}

#[test]
fn prompt_cache_hits_are_cache_reads() {
    let usage = reply(&fixture("responses", "json", "recorded-prompt-cache-hit")).usage;
    assert_eq!(
        (
            usage.input_tokens,
            usage.cache_read_tokens,
            usage.output_tokens
        ),
        (154, 128, 87)
    );
    assert_eq!(usage.total_input_tokens(), 282);
}

#[test]
fn reasoning_content_stays_an_extra_of_the_reply() {
    let resp = reply(&fixture(
        "responses",
        "json",
        "recorded-thinking-reasoning-content",
    ));
    assert!(!resp.blocks.iter().any(|b| matches!(b, Block::Reasoning(_))));
    let extras = serde_json::to_string(&resp.extras).unwrap();
    assert!(extras.contains("reasoning_content"), "{extras}");
    assert_eq!(resp.usage.reasoning_tokens, Some(18));
}

#[test]
fn reasoning_content_sent_back_is_readable_reasoning_of_its_message() {
    let body = fixture("requests", "json", "reasoning-content-sent-back");
    let Request::Generate(req) =
        OpenAiChat::decode_request(RequestKind::Generate, &body, &HeaderList::new()).unwrap()
    else {
        panic!()
    };
    let assistant = &req.messages[1];
    let Some(Block::Reasoning(thought)) = assistant.blocks.first() else {
        panic!("{assistant:?}")
    };
    assert!(!thought.text.concat().is_empty());
    assert!(!assistant.extras.fields.contains_key("reasoning_content"));
}

#[test]
fn empty_lines_sent_while_busy_do_not_change_a_reply() {
    let body = fixture("responses", "json", "recorded-thinking-tools-auto");
    let mut waited = b"\n\n\n".to_vec();
    waited.extend_from_slice(&body);
    let resp = reply(&waited);
    assert_eq!(resp, reply(&body));
    assert_json_eq("after empty lines", &body, &encode_reply(&resp));
}

#[test]
fn keep_alive_comments_sent_while_busy_round_trip_and_change_no_event() {
    let stream = fixture("streams", "sse", "recorded-thinking-tools-stream");
    let mut waited = [KEEP_ALIVE, KEEP_ALIVE].concat();
    waited.extend_from_slice(&stream);
    let events = decode_stream(&waited);
    let plain = decode_stream(&stream);
    let extras = |events: &[StreamEvent]| events.iter().filter(|e| is_extra(e)).count();
    assert_eq!(extras(&events), extras(&plain) + 2);
    let typed = |events: &[StreamEvent]| {
        events
            .iter()
            .filter(|e| !is_extra(e))
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(typed(&events), typed(&plain));
    assert_sse_eq("keep-alive", &waited, &encode_stream(&events));
}

#[test]
fn insufficient_balance_is_a_billing_error() {
    let err = OpenAiChat::decode_error(402, &HeaderList::new(), b"");
    assert_eq!((&err.kind, err.status), (&ErrorKind::Billing, Some(402)));
    assert_eq!(OpenAiChat::encode_error(&err).status, 402);
}

#[test]
fn recorded_errors_keep_their_kind() {
    let cases = [
        (
            "recorded-authentication-fails",
            401,
            ErrorKind::Authentication,
        ),
        ("recorded-missing-model", 400, ErrorKind::InvalidRequest),
        (
            "recorded-plain-developer-string",
            422,
            ErrorKind::InvalidRequest,
        ),
        (
            "recorded-thinking-tool-choice-required",
            400,
            ErrorKind::InvalidRequest,
        ),
    ];
    for (name, status, kind) in cases {
        let body = fixture("errors", "json", name);
        let err = OpenAiChat::decode_error(status, &HeaderList::new(), &body);
        assert_eq!(err.kind, kind, "{name}");
        let out = OpenAiChat::encode_error(&err);
        assert_eq!(out.status, status, "{name}");
        assert_json_eq(name, &body, &out.body);
    }
}
