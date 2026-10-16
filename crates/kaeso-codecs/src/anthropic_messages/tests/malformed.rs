use kaeso_kpp::{EmbedInput, EmbedRequest, ErrorKind, StreamEvent};
use serde_json::{Value, json};

use super::*;

fn decode_err(body: &Value) -> KppError {
    let bytes = serde_json::to_vec(body).unwrap();
    AnthropicMessages::decode_request(RequestKind::Generate, &bytes, &HeaderList::new())
        .unwrap_err()
}

#[test]
fn malformed_requests_name_the_field() {
    let user = json!({"role": "user", "content": "Hi"});
    let cases = [
        (
            json!({"model": "m", "max_tokens": 1, "system": 5, "messages": [user]}),
            "system",
        ),
        (
            json!({"model": "m", "max_tokens": 1, "stop_sequences": [1], "messages": [user]}),
            "stop_sequences",
        ),
        (
            json!({"model": "m", "max_tokens": 1, "messages": [{"content": "Hi"}]}),
            "messages[0]",
        ),
        (
            json!({"model": "m", "max_tokens": 1, "messages": [user, 5]}),
            "messages[1]",
        ),
        (
            json!({"model": "m", "max_tokens": 1, "messages": [{"role": "robot", "content": "Hi"}]}),
            "messages[0].role",
        ),
        (
            json!({"model": "m", "max_tokens": 1, "messages": [user], "tools": [{"name": 5}]}),
            "tools[0]",
        ),
    ];
    for (body, param) in cases {
        let err = decode_err(&body);
        assert_eq!(err.kind, ErrorKind::InvalidRequest, "{body}: {err:?}");
        assert_eq!(err.param.as_deref(), Some(param), "{body}: {err:?}");
    }
}

#[test]
fn a_body_that_is_not_an_object_is_an_invalid_request() {
    for body in [json!([1, 2]), json!("text"), json!({"model": 5})] {
        let err = decode_err(&body);
        assert_eq!(err.kind, ErrorKind::InvalidRequest, "{body}");
    }
}

#[test]
fn embeddings_are_not_an_anthropic_request() {
    let req = Request::Embed(EmbedRequest::new(
        "claude-sonnet-5-5",
        vec![EmbedInput::Text("hello".to_owned())],
    ));
    let mut ctx = EgressCtx::new(&req);
    let err = AnthropicMessages::encode_request(&req, &mut ctx).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::Unsupported { .. }), "{err:?}");
    assert!(
        err.message.contains("embeddings request"),
        "{}",
        err.message
    );
}

fn sse(events: &[Value]) -> String {
    events
        .iter()
        .map(|e| {
            format!(
                "event: {}\ndata: {e}\n\n",
                e["type"].as_str().unwrap_or_default()
            )
        })
        .collect()
}

fn message_start() -> Value {
    json!({"type": "message_start", "message": {"id": "msg_1", "type": "message", "role": "assistant", "model": "claude-sonnet-5-5", "content": [], "stop_reason": null, "stop_sequence": null, "usage": {"input_tokens": 3, "output_tokens": 0}}})
}

fn block_start(index: u32, block: Value) -> Value {
    json!({"type": "content_block_start", "index": index, "content_block": block})
}

fn text_delta(index: u32) -> Value {
    json!({"type": "content_block_delta", "index": index, "delta": {"type": "text_delta", "text": "x"}})
}

fn stop(index: u32) -> Value {
    json!({"type": "content_block_stop", "index": index})
}

#[test]
fn impossible_block_orders_end_the_stream_with_one_error() {
    let text = json!({"type": "text", "text": ""});
    let tool = json!({"type": "tool_use", "id": "toolu_1", "name": "lookup", "input": {}});
    let thinking = json!({"type": "thinking", "thinking": "", "signature": ""});
    let cases = [
        sse(&[
            message_start(),
            block_start(0, text.clone()),
            block_start(0, text.clone()),
        ]),
        sse(&[
            message_start(),
            block_start(0, text.clone()),
            stop(0),
            text_delta(0),
        ]),
        sse(&[message_start(), block_start(0, tool), text_delta(0)]),
        sse(&[message_start(), block_start(0, thinking), text_delta(0)]),
        sse(&[
            message_start(),
            block_start(0, text),
            json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": 5}}),
        ]),
        format!(
            "{}event: content_block_delta\ndata: {{\"index\": 0}}\n\n",
            sse(&[message_start()])
        ),
    ];
    for bytes in cases {
        let events = streams::decode_stream_chunked(bytes.as_bytes(), 0, &streams::stream_ctx());
        let errors: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                StreamEvent::Error(e) => Some(e),
                _ => None,
            })
            .collect();
        assert_eq!(errors.len(), 1, "{bytes}: {events:?}");
        assert_eq!(errors[0].kind, ErrorKind::ProviderError, "{bytes}");
        assert!(
            matches!(events.last(), Some(StreamEvent::Error(_))),
            "{bytes}"
        );
    }
}

#[test]
fn unknown_events_keep_their_framing() {
    let bytes = format!(
        "{}retry: 500\nx-trace: abc\nevent: x_future_event\ndata: {{\"type\":\"x_future_event\"}}\n\nevent: x_future_untyped\ndata: {{\"note\":1}}\n\n{}",
        sse(&[message_start()]),
        sse(&[
            json!({"type": "message_delta", "delta": {"stop_reason": "end_turn", "stop_sequence": null}, "usage": {"output_tokens": 0}}),
            json!({"type": "message_stop"})
        ])
    );
    let events = streams::decode_stream_chunked(bytes.as_bytes(), 0, &streams::stream_ctx());
    assert!(
        !events.iter().any(|e| matches!(e, StreamEvent::Error(_))),
        "{events:?}"
    );
    let out = streams::encode_stream(&events);
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("retry: 500\n"), "{text}");
    assert!(text.contains("x-trace: abc\n"), "{text}");
    assert!(text.contains("event: x_future_untyped\n"), "{text}");
}

#[test]
fn the_encoder_refuses_block_orders_that_cannot_happen() {
    use kaeso_kpp::{BlockDelta, BlockHeader, BlockStart, BlockStop, Delta, ResponseStart};
    let start = || StreamEvent::ResponseStart(ResponseStart::new("msg_1", "claude-sonnet-5-5"));
    let text = |i| StreamEvent::BlockStart(BlockStart::new(i, BlockHeader::Text));
    let delta = |i| StreamEvent::BlockDelta(BlockDelta::new(i, Delta::Text("x".to_owned())));
    let stop = |i| StreamEvent::BlockStop(BlockStop::new(i));
    let sequences = [
        vec![start(), text(0), text(0)],
        vec![start(), text(0), delta(0), stop(0), delta(0)],
        vec![start(), stop(4)],
    ];
    let ctx = EncodeCtx::new(
        RequestKind::Generate,
        StreamMode::On {
            include_usage: true,
        },
    );
    for events in sequences {
        let mut enc = AnthropicMessages::stream_encoder(&ctx);
        let results: Vec<_> = events.iter().map(|e| enc.encode(e)).collect();
        assert!(results.iter().any(Result::is_err), "{events:?}");
    }
}

#[test]
fn overlapping_blocks_are_written_in_order() {
    use kaeso_kpp::{BlockDelta, BlockHeader, BlockStart, BlockStop, Delta, ResponseStart};
    let events = vec![
        StreamEvent::ResponseStart(ResponseStart::new("msg_1", "claude-sonnet-5-5")),
        StreamEvent::BlockStart(BlockStart::new(0, BlockHeader::Text)),
        StreamEvent::BlockStart(BlockStart::new(1, BlockHeader::Text)),
        StreamEvent::BlockDelta(BlockDelta::new(1, Delta::Text("second".to_owned()))),
        StreamEvent::BlockStop(BlockStop::new(1)),
        StreamEvent::BlockDelta(BlockDelta::new(0, Delta::Text("first".to_owned()))),
        StreamEvent::BlockStop(BlockStop::new(0)),
    ];
    let text = String::from_utf8(streams::encode_stream(&events)).unwrap();
    let first = text.find("first").unwrap();
    let second = text.find("second").unwrap();
    assert!(first < second, "{text}");
}
