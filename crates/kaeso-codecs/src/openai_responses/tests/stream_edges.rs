use kaeso_kpp::{ErrorKind, ResponseStop};
use serde_json::json;

use super::streams::{decode_stream_chunked, encode_stream, sse_ctx, start, typed};
use super::*;

fn sse_of(events: &[Value]) -> Vec<u8> {
    let mut out = Vec::new();
    for e in events {
        crate::sse::write_event(&mut out, e["type"].as_str(), &e.to_string());
    }
    out
}

fn msg_events(seq0: u64) -> Vec<Value> {
    let item = json!({"id": "msg_1", "type": "message", "status": "in_progress", "content": [], "role": "assistant"});
    let part = json!({"type": "output_text", "annotations": [], "logprobs": [], "text": ""});
    let done_part = json!({"type": "output_text", "annotations": [], "logprobs": [], "text": "Hi"});
    let base = json!({"item_id": "msg_1", "output_index": 0, "content_index": 0});
    let with = |ty: &str, seq: u64, extra: Value| {
        let mut v = base.clone();
        v["type"] = json!(ty);
        v["sequence_number"] = json!(seq);
        for (k, x) in extra.as_object().unwrap() {
            v[k] = x.clone();
        }
        v
    };
    vec![
        json!({"type": "response.output_item.added", "sequence_number": seq0, "output_index": 0, "item": item}),
        with(
            "response.content_part.added",
            seq0 + 1,
            json!({"part": part}),
        ),
        with(
            "response.output_text.delta",
            seq0 + 2,
            json!({"delta": "Hi", "logprobs": []}),
        ),
        with(
            "response.output_text.done",
            seq0 + 3,
            json!({"text": "Hi", "logprobs": []}),
        ),
        with(
            "response.content_part.done",
            seq0 + 4,
            json!({"part": done_part}),
        ),
    ]
}

#[test]
fn a_stream_without_response_created_still_decodes_its_content() {
    let mut events = msg_events(0);
    let done_item = json!({"id": "msg_1", "type": "message", "status": "completed",
        "content": [{"type": "output_text", "annotations": [], "logprobs": [], "text": "Hi"}], "role": "assistant"});
    events.push(json!({"type": "response.output_item.done", "sequence_number": 5, "output_index": 0, "item": done_item}));
    events.push(json!({"type": "response.completed", "sequence_number": 6, "response": {"id": "resp_9", "object": "response",
        "status": "completed", "model": "m", "output": [done_item],
        "usage": {"input_tokens": 1, "input_tokens_details": {"cached_tokens": 0, "cache_write_tokens": 0},
                  "output_tokens": 1, "output_tokens_details": {"reasoning_tokens": 0}, "total_tokens": 2}}}));
    let sse = sse_of(&events);
    let decoded = decode_stream_chunked(&sse, 17);
    assert!(matches!(&decoded[0], StreamEvent::ResponseStart(s) if s.id.is_empty()));
    assert!(
        typed(&decoded).contains(&"delta 0 Hi".to_owned()),
        "{:?}",
        typed(&decoded)
    );
    assert!(matches!(decoded.last(), Some(StreamEvent::ResponseStop(_))));
    assert!(!decoded.iter().any(|e| matches!(e, StreamEvent::Error(_))));
    assert_sse_eq("no created", &sse, &encode_stream(&decoded, &sse_ctx()));
}

#[test]
fn events_for_unknown_items_are_provider_errors() {
    let mut events = vec![json!({"type": "response.created", "sequence_number": 0,
        "response": {"id": "r", "object": "response", "status": "in_progress", "model": "m", "output": []}})];
    events.push(
        json!({"type": "response.output_text.delta", "sequence_number": 1, "item_id": "msg_x",
        "output_index": 3, "content_index": 0, "delta": "lost", "logprobs": []}),
    );
    events.push(
        json!({"type": "response.output_text.delta", "sequence_number": 2, "item_id": "msg_x",
        "output_index": 3, "content_index": 0, "delta": "more", "logprobs": []}),
    );
    let decoded = decode_stream_chunked(&sse_of(&events), 5);
    assert!(matches!(&decoded[0], StreamEvent::ResponseStart(_)));
    assert_eq!(decoded.len(), 2, "{decoded:?}");
    assert!(
        matches!(&decoded[1], StreamEvent::Error(e) if e.kind == ErrorKind::ProviderError && e.message.contains("response.output_text.delta"))
    );
    for bad in [
        json!({"type": "response.function_call_arguments.delta", "sequence_number": 1, "item_id": "fc", "output_index": 9, "delta": "{"}),
        json!({"type": "response.content_part.done", "sequence_number": 1, "item_id": "m", "output_index": 9, "content_index": 0, "part": {}}),
        json!({"type": "response.output_item.done", "sequence_number": 1, "output_index": 9, "item": {"type": "function_call"}}),
        json!({"type": "response.reasoning_summary_text.delta", "sequence_number": 1, "item_id": "rs", "output_index": 9, "summary_index": 0, "delta": "x"}),
    ] {
        let decoded = decode_stream_chunked(&sse_of(&[events[0].clone(), bad.clone()]), 64);
        assert!(
            matches!(decoded.last(), Some(StreamEvent::Error(_))),
            "{bad}"
        );
    }
}

#[test]
fn nothing_is_decoded_or_encoded_after_the_response_ends() {
    let created = json!({"type": "response.created", "sequence_number": 0,
        "response": {"id": "r", "object": "response", "status": "in_progress", "model": "m", "output": []}});
    let error = json!({"type": "error", "sequence_number": 1, "code": "server_error", "message": "boom", "param": null});
    let failed = json!({"type": "response.failed", "sequence_number": 2, "response": {"id": "r", "object": "response",
        "status": "failed", "model": "m", "output": [], "error": {"code": "server_error", "message": "boom"}}});
    let sse = sse_of(&[created.clone(), error, failed]);
    let decoded = decode_stream_chunked(&sse, 9);
    assert_eq!(
        decoded
            .iter()
            .filter(|e| matches!(e, StreamEvent::Error(_)))
            .count(),
        1
    );
    assert_sse_eq("error+failed", &sse, &encode_stream(&decoded, &sse_ctx()));
    let completed = json!({"type": "response.completed", "sequence_number": 1, "response": {"id": "r", "object": "response",
        "status": "completed", "model": "m", "output": [], "usage": null}});
    let mut late = msg_events(2);
    late.insert(0, completed);
    late.insert(0, created);
    let decoded = decode_stream_chunked(&sse_of(&late), 11);
    assert!(
        matches!(decoded.last(), Some(StreamEvent::ResponseStop(_))),
        "{decoded:?}"
    );
    let mut enc = OpenAiResponses::stream_encoder(&sse_ctx());
    enc.encode(&start("r", "m")).unwrap();
    enc.encode(&StreamEvent::Error(KppError::provider("x")))
        .unwrap();
    assert!(
        enc.encode(&StreamEvent::ResponseStop(ResponseStop::new()))
            .is_err()
    );
}
