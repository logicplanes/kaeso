#![cfg(test)]

use kaeso_codecs::limits::{MAX_DEPTH, request_shape};
use kaeso_codecs::{EgressCtx, EncodeCtx, HeaderList, dispatch};
use kaeso_kpp::{
    Block, DialectId, ErrorKind, Reply, Request, RequestKind, StreamEvent, StreamMode,
};
use serde_json::Value;

const A: DialectId = DialectId::AnthropicMessages;
const C: DialectId = DialectId::OpenAiChat;
const R: DialectId = DialectId::OpenAiResponses;
const E: DialectId = DialectId::OpenAiEmbeddings;

const DEEP: usize = 5_000;

fn nested_objects(key: &str, levels: usize) -> String {
    let mut out = String::with_capacity(levels * (key.len() + 5) + 2);
    for _ in 0..levels {
        out.push_str(&format!("{{\"{key}\":"));
    }
    out.push_str("{}");
    out.push_str(&"}".repeat(levels));
    out
}

fn nested_arrays(levels: usize) -> String {
    format!("{}1{}", "[".repeat(levels), "]".repeat(levels))
}

fn nested_content(levels: usize) -> String {
    let mut out = String::new();
    for _ in 0..levels {
        out.push_str(r#"{"role":"user","content":["#);
    }
    out.push_str(r#"{"type":"text","text":"x"}"#);
    out.push_str(&"]}".repeat(levels));
    format!("[{out}]")
}

fn base(dialect: DialectId) -> (RequestKind, Vec<(&'static str, String)>) {
    let messages = || r#"[{"role":"user","content":"Hi"}]"#.to_owned();
    match dialect {
        A => (
            RequestKind::Generate,
            vec![
                ("model", r#""claude-sonnet-4-6""#.into()),
                ("max_tokens", "16".into()),
                ("messages", messages()),
            ],
        ),
        C => (
            RequestKind::Generate,
            vec![("model", r#""gpt-5""#.into()), ("messages", messages())],
        ),
        E => (
            RequestKind::Embed,
            vec![
                ("model", r#""text-embedding-3-small""#.into()),
                ("input", r#""Hi""#.into()),
            ],
        ),
        _ => (
            RequestKind::Generate,
            vec![("model", r#""gpt-5""#.into()), ("input", r#""Hi""#.into())],
        ),
    }
}

fn body_with(dialect: DialectId, key: &str, value: &str) -> Vec<u8> {
    let (_, fields) = base(dialect);
    let mut out: Vec<String> = fields
        .iter()
        .filter(|(k, _)| *k != key)
        .map(|(k, v)| format!("\"{k}\":{v}"))
        .collect();
    out.push(format!("\"{key}\":{value}"));
    format!("{{{}}}", out.join(",")).into_bytes()
}

fn deep_placements(dialect: DialectId) -> Vec<(String, Vec<u8>)> {
    let keys: &[&str] = match dialect {
        A => &[
            "metadata",
            "thinking",
            "tool_choice",
            "system",
            "zzz_unknown",
        ],
        C => &[
            "response_format",
            "stream_options",
            "metadata",
            "zzz_unknown",
        ],
        E => &["zzz_unknown"],
        _ => &[
            "text",
            "reasoning",
            "stream_options",
            "metadata",
            "zzz_unknown",
        ],
    };
    let mut out = Vec::new();
    for key in keys {
        out.push((
            key.to_string(),
            body_with(dialect, key, &nested_objects(key, DEEP)),
        ));
        out.push((
            format!("{key}[]"),
            body_with(dialect, key, &nested_arrays(DEEP)),
        ));
    }
    let list = if dialect == R || dialect == E {
        "input"
    } else {
        "messages"
    };
    out.push((
        format!("{list} content"),
        body_with(dialect, list, &nested_content(DEEP)),
    ));
    let tool = match dialect {
        A => format!(
            r#"[{{"name":"f","input_schema":{}}}]"#,
            nested_objects("properties", DEEP)
        ),
        C => format!(
            r#"[{{"type":"function","function":{{"name":"f","parameters":{}}}}}]"#,
            nested_objects("properties", DEEP)
        ),
        _ => format!(
            r#"[{{"type":"function","name":"f","parameters":{}}}]"#,
            nested_objects("properties", DEEP)
        ),
    };
    if dialect != E {
        out.push(("tools".into(), body_with(dialect, "tools", &tool)));
    }
    out
}

const TOKIO_WORKER_STACK: usize = 2 * 1024 * 1024;

fn nested_tool_results(levels: usize) -> String {
    let open = r#"{"type":"tool_result","tool_use_id":"toolu_1","content":["#;
    let mut out = String::with_capacity(levels * (open.len() + 2) + 32);
    for _ in 0..levels {
        out.push_str(open);
    }
    out.push_str(r#"{"type":"text","text":"x"}"#);
    for _ in 0..levels {
        out.push_str("]}");
    }
    out
}

fn request_body(levels: usize) -> Vec<u8> {
    format!(
        r#"{{"model":"claude-sonnet-4-6","max_tokens":16,"messages":[{{"role":"user","content":[{}]}}]}}"#,
        nested_tool_results(levels)
    )
    .into_bytes()
}

fn response_body(levels: usize) -> Vec<u8> {
    format!(
        r#"{{"id":"msg_1","type":"message","role":"assistant","model":"claude-sonnet-4-6","content":[{}],"stop_reason":"end_turn","stop_sequence":null,"usage":{{"input_tokens":1,"output_tokens":1}}}}"#,
        nested_tool_results(levels)
    )
    .into_bytes()
}

fn stream_body(levels: usize) -> Vec<u8> {
    format!(
        "event: message_start\ndata: {{\"type\":\"message_start\",\"message\":{{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"claude-sonnet-4-6\",\"content\":[],\"stop_reason\":null,\"stop_sequence\":null,\"usage\":{{\"input_tokens\":1,\"output_tokens\":1}}}}}}\n\nevent: content_block_start\ndata: {{\"type\":\"content_block_start\",\"index\":0,\"content_block\":{}}}\n\n",
        nested_tool_results(levels)
    )
    .into_bytes()
}

fn on_worker_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(TOKIO_WORKER_STACK)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap()
}

fn decode_request(levels: usize) -> Result<Request, kaeso_kpp::KppError> {
    let body = request_body(levels);
    on_worker_stack(move || {
        dispatch::decode_request(A, RequestKind::Generate, &body, &HeaderList::new())
    })
}

fn decode_response(levels: usize) -> Result<Reply, kaeso_kpp::KppError> {
    let body = response_body(levels);
    on_worker_stack(move || {
        let ctx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off);
        dispatch::decode_response(A, 200, &HeaderList::new(), &body, &ctx)
    })
}

fn decode_stream(levels: usize) -> Vec<StreamEvent> {
    let body = stream_body(levels);
    on_worker_stack(move || {
        let ctx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::on(true));
        let mut decoder = dispatch::stream_decoder(A, &ctx).unwrap();
        let mut events = decoder.feed(&body);
        events.extend(decoder.finish());
        events
    })
}

fn depth(blocks: &[Block]) -> usize {
    match blocks.first() {
        Some(Block::ToolResult(r)) => 1 + depth(&r.content),
        _ => 0,
    }
}

#[test]
fn deeply_nested_tool_results_in_a_request_are_refused() {
    let err = decode_request(50).unwrap_err();
    assert_eq!(err.kind, ErrorKind::InvalidRequest);
    assert_eq!(err.param.as_deref(), Some("messages[0].content"));
    assert!(
        err.message
            .contains("`tool_result` blocks nest more than 4 levels deep"),
        "{}",
        err.message
    );
}

#[test]
fn deeply_nested_tool_results_in_a_response_are_a_provider_error() {
    let err = decode_response(10_000).unwrap_err();
    assert_eq!(err.kind, ErrorKind::ProviderError);
    assert!(
        err.message
            .contains("`tool_result` blocks nest more than 4 levels deep"),
        "{}",
        err.message
    );
}

#[test]
fn deeply_nested_tool_results_in_a_stream_end_it_with_an_error() {
    let events = decode_stream(10_000);
    let errors: Vec<&kaeso_kpp::KppError> = events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::Error(err) => Some(err),
            _ => None,
        })
        .collect();
    assert_eq!(errors.len(), 1, "{events:?}");
    assert!(
        errors[0]
            .message
            .contains("`tool_result` blocks nest more than 4 levels deep"),
        "{}",
        errors[0].message
    );
}

#[test]
fn tool_results_nested_up_to_the_limit_still_decode() {
    let Request::Generate(req) = decode_request(4).unwrap() else {
        panic!("not a generate request");
    };
    assert_eq!(depth(&req.messages[0].blocks), 4);
    let Reply::Generate(reply) = decode_response(4).unwrap() else {
        panic!("not a generate reply");
    };
    assert_eq!(depth(&reply.blocks), 4);
    assert!(
        decode_request(5).is_err(),
        "five levels exceed the limit of four"
    );
}

fn too_deep(err: &kaeso_kpp::KppError) {
    assert_eq!(err.kind, ErrorKind::InvalidRequest);
    assert_eq!(err.status, Some(400));
    assert_eq!(
        err.message,
        "The body nests more than 127 levels deep; Kaeso takes at most 127."
    );
}

#[test]
fn a_body_nested_thousands_deep_is_refused_in_every_dialect_on_a_worker_stack() {
    for dialect in [A, C, R, E] {
        let (kind, _) = base(dialect);
        for (place, body) in deep_placements(dialect) {
            too_deep(&request_shape(dialect, &body).unwrap_err());
            let decoded = on_worker_stack(move || {
                dispatch::decode_request(dialect, kind, &body, &HeaderList::new())
            });
            let err = decoded.expect_err(&format!("{dialect:?} {place}"));
            too_deep(&err);
        }
    }
}

#[test]
fn depth_is_counted_outside_strings_only() {
    let quoted = format!(r#""{}""#, "[{".repeat(DEEP));
    let escaped = format!(r#""\"{}""#, "[".repeat(DEEP));
    for value in [quoted, escaped] {
        let body = body_with(R, "input", &value);
        assert!(request_shape(R, &body).is_ok());
    }
    let after_string = body_with(
        R,
        "zzz_unknown",
        &format!(r#"["\\",{}]"#, nested_arrays(DEEP)),
    );
    too_deep(&request_shape(R, &after_string).unwrap_err());
}

#[test]
fn a_body_at_the_depth_limit_decodes_and_encodes_unchanged() {
    for (dialect, key) in [
        (R, "text"),
        (R, "reasoning"),
        (R, "zzz_unknown"),
        (C, "zzz_unknown"),
        (A, "zzz_unknown"),
        (E, "zzz_unknown"),
    ] {
        let (kind, _) = base(dialect);
        let at_limit = body_with(dialect, key, &nested_objects(key, MAX_DEPTH - 2));
        let over = body_with(dialect, key, &nested_objects(key, MAX_DEPTH - 1));
        too_deep(&request_shape(dialect, &over).unwrap_err());
        let sent = at_limit.clone();
        let encoded = on_worker_stack(move || {
            let req = dispatch::decode_request(dialect, kind, &sent, &HeaderList::new())?;
            let mut ctx = EgressCtx::new(&req);
            dispatch::encode_request(dialect, &req, &mut ctx).map(|e| e.body)
        })
        .unwrap_or_else(|e| panic!("{dialect:?} {key}: {}", e.message));
        let expected: Value = serde_json::from_slice(&at_limit).unwrap();
        let actual: Value = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(
            expected.get(key),
            actual.get(key),
            "{dialect:?} {key} keeps the nested value"
        );
    }
}

#[test]
fn a_provider_response_nested_thousands_deep_is_carried_whole_on_a_worker_stack() {
    let deep = nested_objects("properties", DEEP);
    let body = format!(
        r#"{{"id":"resp_1","object":"response","created_at":1,"status":"completed","model":"gpt-5","output":[{{"id":"msg_1","type":"message","status":"completed","role":"assistant","content":[{{"type":"output_text","annotations":[{{"type":"url_citation","url":"https://example.com","title":"x","start_index":0,"end_index":1,"zzz":{deep}}}],"text":"Hi"}}]}}],"tools":[{{"type":"function","name":"f","parameters":{deep}}}],"zzz_unknown":{deep},"usage":{{"input_tokens":1,"output_tokens":1,"total_tokens":2}}}}"#
    )
    .into_bytes();
    let encoded = on_worker_stack(move || {
        let ctx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off);
        let reply = dispatch::decode_response(R, 200, &HeaderList::new(), &body, &ctx)?;
        let ctx = EncodeCtx::new(RequestKind::Generate, StreamMode::Off);
        dispatch::encode_response(R, &reply, &ctx)
    })
    .unwrap_or_else(|e| panic!("{}", e.message));
    let text = String::from_utf8(encoded).unwrap();
    assert!(
        text.contains(&format!(r#""zzz_unknown":{deep}"#)),
        "the unknown field keeps every level"
    );
    assert!(
        text.contains(&format!(r#""zzz":{deep}"#)),
        "the annotation keeps every level"
    );
}
