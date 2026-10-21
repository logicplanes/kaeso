#![cfg(test)]

use kaeso_codecs::limits::{
    MAX_BLOCKS_PER_MESSAGE, MAX_EMBEDDING_INPUTS, MAX_MESSAGES, MAX_TOOLS, RequestShape,
    request_shape,
};
use kaeso_codecs::{HeaderList, dispatch};
use kaeso_kpp::{DialectId, ErrorKind, KppError, RequestKind};

const A: DialectId = DialectId::AnthropicMessages;
const C: DialectId = DialectId::OpenAiChat;
const R: DialectId = DialectId::OpenAiResponses;
const E: DialectId = DialectId::OpenAiEmbeddings;

fn repeated(item: &str, n: usize) -> String {
    let mut out = String::with_capacity(n * (item.len() + 1));
    for i in 0..n {
        if i > 0 {
            out.push(',');
        }
        out.push_str(item);
    }
    out
}

fn shape(dialect: DialectId, body: &str) -> RequestShape {
    request_shape(dialect, body.as_bytes()).unwrap()
}

fn refused(dialect: DialectId, body: &str) -> KppError {
    let err = request_shape(dialect, body.as_bytes()).unwrap_err();
    let decoded = dispatch::decode_request(
        dialect,
        RequestKind::Generate,
        body.as_bytes(),
        &HeaderList::new(),
    )
    .unwrap_err();
    assert_eq!(decoded, err, "every decoder refuses what the count refuses");
    assert_eq!(err.kind, ErrorKind::InvalidRequest);
    assert_eq!(err.status, Some(400));
    err
}

fn counted(items: usize, blocks: usize, tools: usize) -> RequestShape {
    let mut s = RequestShape::default();
    s.items = items;
    s.blocks = blocks;
    s.tools = tools;
    s
}

#[test]
fn messages_blocks_nested_tool_results_system_and_tools_are_counted() {
    let body = r#"{
        "model": "claude-haiku-4-5", "max_tokens": 16,
        "system": [{"type": "text", "text": "a"}, {"type": "text", "text": "b"}],
        "messages": [
            {"role": "user", "content": "Hi"},
            {"role": "assistant", "content": [{"type": "tool_use", "id": "t", "name": "f", "input": {"content": [1, 2, 3]}}]},
            {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "t", "content": [{"type": "text", "text": "x"}, {"type": "text", "text": "y"}]}]}
        ],
        "tools": [{"name": "f", "input_schema": {"type": "object"}}]
    }"#;
    assert_eq!(shape(A, body), counted(3, 7, 1));

    let chat = r#"{"model": "m", "messages": [{"role": "user", "content": [{"type": "text", "text": "a"}]}], "functions": [{"name": "f"}], "tools": [{"type": "function", "function": {"name": "g"}}]}"#;
    assert_eq!(shape(C, chat), counted(1, 1, 2));

    let responses = r#"{"model": "m", "input": [
        {"role": "user", "content": [{"type": "input_text", "text": "a"}, {"type": "input_image", "image_url": "data:"}]},
        {"type": "function_call_output", "call_id": "c", "output": [{"type": "input_text", "text": "b"}]}
    ], "tools": [{"type": "web_search"}]}"#;
    assert_eq!(shape(R, responses), counted(2, 3, 1));
    assert_eq!(
        shape(R, r#"{"model": "m", "input": "Hi"}"#),
        counted(1, 0, 0)
    );

    assert_eq!(
        shape(E, r#"{"model": "m", "input": ["a", "b", [1, 2]]}"#),
        counted(3, 0, 0)
    );
    assert_eq!(
        shape(E, r#"{"model": "m", "input": [1, 2, 3]}"#),
        counted(0, 0, 0)
    );
}

#[test]
fn a_body_kaeso_cannot_read_is_left_to_its_decoder() {
    for body in [
        "",
        "not json",
        "[1, 2]",
        r#"{"messages": 5}"#,
        r#"{"messages": [1, "x", null, true, -1, 1.5, {"content": 3}]}"#,
    ] {
        assert!(request_shape(A, body.as_bytes()).is_ok(), "{body}");
    }
}

#[test]
fn more_messages_than_the_limit_are_refused_naming_the_field() {
    let at_limit = format!(
        r#"{{"model": "m", "messages": [{}]}}"#,
        repeated(r#"{"role":"user","content":"a"}"#, MAX_MESSAGES)
    );
    assert_eq!(shape(C, &at_limit).items, MAX_MESSAGES);
    let over = format!(
        r#"{{"model": "m", "max_tokens": 1, "messages": [{}]}}"#,
        repeated(r#"{"role":"user","content":"a"}"#, MAX_MESSAGES + 1)
    );
    for dialect in [A, C] {
        let err = refused(dialect, &over);
        assert_eq!(
            err.message,
            "`messages` holds more than 100000 messages; Kaeso takes at most 100000."
        );
        assert_eq!(err.param.as_deref(), Some("messages"));
    }
    let input = format!(
        r#"{{"model": "m", "input": [{}]}}"#,
        repeated(r#"{"role":"user","content":"a"}"#, MAX_MESSAGES + 1)
    );
    let err = refused(R, &input);
    assert_eq!(
        err.message,
        "`input` holds more than 100000 items; Kaeso takes at most 100000."
    );
    assert_eq!(err.param.as_deref(), Some("input"));
}

#[test]
fn a_message_with_too_many_blocks_is_refused_and_each_message_counts_on_its_own() {
    let blocks = |n| repeated(r#"{"type":"text","text":"a"}"#, n);
    let full = format!(
        r#"{{"role":"user","content":[{}]}}"#,
        blocks(MAX_BLOCKS_PER_MESSAGE)
    );
    let two_full = format!(
        r#"{{"model": "m", "max_tokens": 1, "system": [{}], "messages": [{full},{full}]}}"#,
        blocks(MAX_BLOCKS_PER_MESSAGE)
    );
    assert_eq!(shape(A, &two_full).blocks, 3 * MAX_BLOCKS_PER_MESSAGE);
    let nested = format!(
        r#"{{"model": "m", "max_tokens": 1, "messages": [{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"t","content":[{}]}}]}}]}}"#,
        blocks(MAX_BLOCKS_PER_MESSAGE)
    );
    let err = refused(A, &nested);
    assert_eq!(
        err.message,
        "A message holds more than 10000 content blocks; Kaeso takes at most 10000."
    );
    assert_eq!(err.param.as_deref(), Some("messages"));
    let parts = format!(
        r#"{{"model": "m", "input": [{{"role":"user","content":[{}]}}]}}"#,
        repeated(
            r#"{"type":"input_text","text":"a"}"#,
            MAX_BLOCKS_PER_MESSAGE + 1
        )
    );
    assert_eq!(refused(R, &parts).param.as_deref(), Some("input"));
}

#[test]
fn too_many_tools_or_embedding_inputs_are_refused() {
    let tools = format!(
        r#"{{"model": "m", "input": "Hi", "tools": [{}]}}"#,
        repeated(r#"{"type":"function","name":"f"}"#, MAX_TOOLS + 1)
    );
    let err = refused(R, &tools);
    assert_eq!(
        err.message,
        "`tools` holds more than 10000 tools; Kaeso takes at most 10000."
    );
    assert_eq!(err.param.as_deref(), Some("tools"));
    let inputs = format!(
        r#"{{"model": "m", "input": [{}]}}"#,
        repeated(r#""a""#, MAX_EMBEDDING_INPUTS + 1)
    );
    let err = request_shape(E, inputs.as_bytes()).unwrap_err();
    assert_eq!(
        err.message,
        "`input` holds more than 2048 inputs; Kaeso takes at most 2048."
    );
    let decoded =
        dispatch::decode_request(E, RequestKind::Embed, inputs.as_bytes(), &HeaderList::new())
            .unwrap_err();
    assert_eq!(decoded, err);
}

#[test]
fn an_escaped_key_is_counted_like_the_key_it_spells() {
    let body = format!(
        r#"{{"model": "m", "messages": [{}]}}"#,
        repeated(r#"{"role":"user","content":"a"}"#, MAX_MESSAGES + 1)
    );
    assert_eq!(refused(C, &body).param.as_deref(), Some("messages"));
}
