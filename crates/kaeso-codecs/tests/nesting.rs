#![cfg(test)]

use kaeso_codecs::{EgressCtx, HeaderList, dispatch};
use kaeso_kpp::{
    Block, DialectId, ErrorKind, Reply, Request, RequestKind, StreamEvent, StreamMode,
};

const A: DialectId = DialectId::AnthropicMessages;

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
    let err = decode_request(10_000).unwrap_err();
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
