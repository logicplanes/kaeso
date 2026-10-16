use kaeso_kpp::ErrorKind;
use serde_json::{Value, json};

use super::streams::{decode_stream_chunked, encode_stream, sse_ctx};
use super::*;

fn frame(event: &Value) -> String {
    let ty = event["type"].as_str().unwrap_or_default();
    format!("event: {ty}\ndata: {event}\n\n")
}

fn created() -> String {
    frame(&json!({
        "type": "response.created",
        "sequence_number": 0,
        "response": {"id": "resp_1", "model": "gpt-5.6-sol", "status": "in_progress", "output": []}
    }))
}

fn message_added() -> String {
    frame(&json!({
        "type": "response.output_item.added",
        "sequence_number": 1,
        "output_index": 0,
        "item": {"id": "msg_1", "type": "message", "status": "in_progress", "content": [], "role": "assistant"}
    }))
}

fn call_added() -> String {
    frame(&json!({
        "type": "response.output_item.added",
        "sequence_number": 1,
        "output_index": 0,
        "item": {"id": "fc_1", "type": "function_call", "status": "in_progress", "call_id": "call_1", "name": "lookup", "arguments": ""}
    }))
}

fn errors(sse: &str) -> Vec<KppError> {
    decode_stream_chunked(sse.as_bytes(), usize::MAX)
        .into_iter()
        .filter_map(|e| match e {
            StreamEvent::Error(err) => Some(err),
            _ => None,
        })
        .collect()
}

#[test]
fn known_events_without_what_they_need_end_the_stream_with_one_error() {
    let broken = [
        format!(
            "{}{}",
            created(),
            frame(
                &json!({"type": "response.output_item.added", "sequence_number": 1, "output_index": 0})
            )
        ),
        format!(
            "{}{}",
            created(),
            frame(
                &json!({"type": "response.output_item.added", "sequence_number": 1, "output_index": 0, "item": {"type": "function_call", "call_id": 5}})
            )
        ),
        format!(
            "{}{}{}",
            created(),
            message_added(),
            frame(
                &json!({"type": "response.content_part.added", "sequence_number": 2, "item_id": "msg_1", "output_index": 0, "content_index": 0})
            )
        ),
        format!(
            "{}{}{}",
            created(),
            call_added(),
            frame(
                &json!({"type": "response.content_part.added", "sequence_number": 2, "item_id": "fc_1", "output_index": 0, "content_index": 0, "part": {"type": "output_text", "text": ""}})
            )
        ),
        format!(
            "{}{}{}",
            created(),
            message_added(),
            frame(
                &json!({"type": "response.output_text.annotation.added", "sequence_number": 2, "item_id": "msg_1", "output_index": 0, "content_index": 3, "annotation_index": 0, "annotation": {"type": "url_citation"}})
            )
        ),
        format!(
            "{}{}{}",
            created(),
            call_added(),
            frame(
                &json!({"type": "response.reasoning_text.delta", "sequence_number": 2, "item_id": "fc_1", "output_index": 0, "content_index": 0, "delta": "thinking"})
            )
        ),
        format!(
            "{}{}{}",
            created(),
            message_added(),
            frame(
                &json!({"type": "response.output_item.done", "sequence_number": 2, "output_index": 0})
            )
        ),
        format!(
            "{}{}",
            created(),
            frame(&json!({"type": "response.completed", "sequence_number": 1, "response": 5}))
        ),
        format!(
            "{}{}",
            created(),
            frame(&json!({"type": "response.failed", "sequence_number": 1, "response": "gone"}))
        ),
    ];
    for sse in broken {
        let errors = errors(&sse);
        assert_eq!(errors.len(), 1, "{sse}: {errors:?}");
        assert_eq!(errors[0].kind, ErrorKind::ProviderError, "{sse}");
    }
}

#[test]
fn a_mid_stream_error_keeps_its_param() {
    let sse = format!(
        "{}{}",
        created(),
        frame(
            &json!({"type": "error", "sequence_number": 1, "code": "invalid_prompt", "message": "Bad prompt.", "param": "input"})
        )
    );
    let errors = errors(&sse);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].param.as_deref(), Some("input"));
}

#[test]
fn unknown_framing_on_unknown_events_is_replayed() {
    let sse = format!(
        "{}retry: 2000\nx-future-field: on\nevent: x_future_event\ndata: {{\"type\":\"x_future_event\"}}\n\n{}",
        created(),
        frame(
            &json!({"type": "response.completed", "sequence_number": 2, "response": {"id": "resp_1", "model": "gpt-5.6-sol", "status": "completed", "output": []}})
        )
    );
    let events = decode_stream_chunked(sse.as_bytes(), usize::MAX);
    let out = encode_stream(&events, &sse_ctx());
    assert_sse_eq("unknown framing", sse.as_bytes(), &out);
}

#[test]
fn a_created_event_without_a_readable_response_is_kept() {
    for created in [
        frame(&json!({"type": "response.created", "sequence_number": 0})),
        frame(&json!({"type": "response.created", "sequence_number": 0, "response": {"id": 5}})),
    ] {
        let events = decode_stream_chunked(created.as_bytes(), usize::MAX);
        assert!(
            events.iter().any(|e| matches!(e, StreamEvent::Extra(_))),
            "{created}: {events:?}"
        );
    }
}

#[test]
fn nothing_is_decoded_after_the_stream_failed() {
    let sse = format!("{}data: {{not json\n\n{}", created(), message_added());
    for size in [1, 4, usize::MAX] {
        let errors: Vec<_> = decode_stream_chunked(sse.as_bytes(), size)
            .into_iter()
            .filter(|e| matches!(e, StreamEvent::Error(_)))
            .collect();
        assert_eq!(errors.len(), 1, "{size}");
    }
}

#[test]
fn the_encoder_refuses_event_sequences_that_cannot_happen() {
    use kaeso_kpp::{BlockStop, ResponseStart};
    let start = || StreamEvent::ResponseStart(ResponseStart::new("resp_1", "gpt-5.6-sol"));
    let sequences = [
        vec![start(), start()],
        vec![start(), StreamEvent::BlockStop(BlockStop::new(5))],
    ];
    for events in sequences {
        let mut enc = OpenAiResponses::stream_encoder(&sse_ctx());
        let results: Vec<_> = events.iter().map(|e| enc.encode(e)).collect();
        assert!(results.iter().any(Result::is_err), "{events:?}");
    }
}

#[test]
fn a_stream_without_a_start_gets_one() {
    use kaeso_kpp::{BlockHeader, BlockStart};
    let mut enc = OpenAiResponses::stream_encoder(&sse_ctx());
    let out = enc
        .encode(&StreamEvent::BlockStart(BlockStart::new(
            0,
            BlockHeader::Text,
        )))
        .unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(text.starts_with("event: response.created\n"), "{text}");
}
