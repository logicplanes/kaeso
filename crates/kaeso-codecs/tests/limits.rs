#![cfg(test)]

use kaeso_codecs::limits::{
    MAX_BLOCKS_PER_MESSAGE, MAX_EMBEDDING_INPUTS, MAX_MESSAGES, MAX_TOOLS, RequestShape,
    request_shape,
};
use kaeso_codecs::{HeaderList, dispatch};
use kaeso_kpp::{DialectId, ErrorKind, KppError, Request, RequestKind, ToolDef};

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
    let mut shape = request_shape(dialect, body.as_bytes()).unwrap();
    shape.bytes = 0;
    shape
}

#[test]
fn the_bytes_left_without_whitespace_between_tokens_are_counted() {
    let tight = r#"{"model":"m","input":"a  b\" {"}"#;
    let loose = " {\n\t\"model\" : \"m\",\r\n  \"input\":   \"a  b\\\" {\"  } ";
    for body in [tight, loose] {
        let s = request_shape(R, body.as_bytes()).unwrap();
        assert_eq!(s.bytes, tight.len(), "{body}");
    }
}

#[test]
fn whitespace_inside_a_value_kept_raw_is_counted() {
    let p = " ".repeat(1 << 20);
    let tight = r#"{"model":"m","messages":[{"role":"user","content":"hi"}],"tools":[{"type":"function","function":{"name":"f","parameters":{"type":"object"}}}]}"#;
    let schema = format!(r#"{{{p}"type"{p}:{p}"object"{p}}}"#);
    let padded = tight.replace(r#"{"type":"object"}"#, &schema);
    assert_eq!(
        request_shape(C, padded.as_bytes()).unwrap().bytes,
        tight.len() + 4 * p.len()
    );
    let Request::Generate(decoded) = dispatch::decode_request(
        C,
        RequestKind::Generate,
        padded.as_bytes(),
        &HeaderList::new(),
    )
    .unwrap() else {
        panic!("a generate request");
    };
    let [ToolDef::Function(tool)] = decoded.tools.as_slice() else {
        panic!("one function tool");
    };
    assert_eq!(
        tool.parameters.as_ref().unwrap().get(),
        schema,
        "the decoded request keeps the padding, so the count does too"
    );
    let between = format!(
        r#"{{{p}"model":"m",{p}"messages":[{p}{{"role":{p}"user","content":"hi"}}{p}],{p}"tools":[{p}{{"type":"function","function":{{"name":"f","parameters":{{"type":"object"}}}}}}{p}]{p}}}"#
    );
    assert_eq!(
        request_shape(C, between.as_bytes()).unwrap().bytes,
        tight.len(),
        "whitespace that decoding drops is not counted"
    );
}

const PAD: usize = 4096;

fn padding_kept_and_counted(dialect: DialectId, template: &str) -> Option<(bool, usize)> {
    let pad = " ".repeat(PAD);
    let tight = template.replace('~', "");
    let padded = template.replace('~', &pad);
    let kind = if dialect == E {
        RequestKind::Embed
    } else {
        RequestKind::Generate
    };
    let decoded =
        dispatch::decode_request(dialect, kind, padded.as_bytes(), &HeaderList::new()).ok()?;
    let kept = format!("{decoded:?}").contains(&pad);
    let counted = request_shape(dialect, padded.as_bytes()).unwrap().bytes
        - request_shape(dialect, tight.as_bytes()).unwrap().bytes;
    Some((kept, counted))
}

fn assert_counted_exactly_when_kept(dialect: DialectId, templates: &[String]) -> usize {
    let mut decoded = 0;
    for template in templates {
        let Some((kept, counted)) = padding_kept_and_counted(dialect, template) else {
            continue;
        };
        decoded += 1;
        let expected = if kept { PAD } else { 0 };
        assert_eq!(counted, expected, "kept: {kept}, {template}");
    }
    decoded
}

#[test]
fn chat_counts_whitespace_exactly_where_its_decoder_keeps_it() {
    let parts = [
        r#"{~"type":"text","text":"a"}"#,
        r#"{~"type":"refusal","refusal":"no"}"#,
        r#"{~"type":"image_url","image_url":{"url":"https://example.com/a.png"}}"#,
        r#"{~"type":"file","file":{"file_id":"file-1"}}"#,
        r#"{~"type":"input_audio","input_audio":{"data":"AA==","format":"wav"}}"#,
        r#"{~"type":"novel","novel":1}"#,
        r#"{~"text":"a"}"#,
        r#"{"type":"text","text":"a","content":[~1]}"#,
        r#"{"type":"text","text":"a","output":{~}}"#,
    ];
    let messages = [
        r#"{"role":"system","content":[PART]}"#,
        r#"{"role":"developer","content":[PART]}"#,
        r#"{"role":"user","content":[PART]}"#,
        r#"{"role":"assistant","content":[PART]}"#,
        r#"{"role":"tool","tool_call_id":"t","content":[PART]}"#,
    ];
    let mut templates: Vec<String> = messages
        .iter()
        .flat_map(|m| parts.iter().map(move |p| m.replace("PART", p)))
        .collect();
    templates.extend(
        [
            r#"{~"role":"user","content":"a"}"#,
            r#"{"role":"user","content":"a","output":[~1]}"#,
            r#"{~"role":"function","name":"f","content":"a"}"#,
            r#"{"role":"function","name":"f","content":[~{"type":"text","text":"a"}]}"#,
            r#"{"role":"assistant","content":null,"tool_calls":[{~"id":"c","type":"novel"}]}"#,
        ]
        .map(str::to_owned),
    );
    let bodies: Vec<String> = templates
        .iter()
        .map(|m| format!(r#"{{"model":"m","messages":[{m}]}}"#))
        .collect();
    let decoded = assert_counted_exactly_when_kept(C, &bodies);
    assert!(decoded > 40, "{decoded} of {} bodies decoded", bodies.len());
    let unknown = r#"{"model":"m","messages":[{"role":"user","content":[{~"type":"novel"}]}]}"#;
    assert_eq!(padding_kept_and_counted(C, unknown), Some((true, PAD)));
}

#[test]
fn anthropic_counts_whitespace_exactly_where_its_decoder_keeps_it() {
    let blocks = [
        r#"{~"type":"text","text":"a"}"#,
        r#"{"type":"text","text":"a","content":[~1]}"#,
        r#"{"type":"text","text":"a","output":[~1]}"#,
        r#"{~"type":"image","source":{"type":"base64","media_type":"image/png","data":"AA=="}}"#,
        r#"{~"type":"image","source":{"type":"url","url":"https://example.com/a.png"}}"#,
        r#"{~"type":"image","source":{"type":"file","file_id":"f"}}"#,
        r#"{~"type":"image","source":{"type":"text","media_type":"text/plain","data":"a"}}"#,
        r#"{~"type":"image","source":{"type":"novel"}}"#,
        r#"{"type":"image","source":{~"type":"novel"}}"#,
        r#"{~"type":"document","source":{"type":"text","media_type":"text/plain","data":"a"}}"#,
        r#"{~"type":"document","source":{"type":"base64","media_type":"application/pdf","data":"AA=="}}"#,
        r#"{~"type":"document","source":{"type":"novel"}}"#,
        r#"{~"type":"tool_use","id":"t","name":"f","input":{}}"#,
        r#"{~"type":"tool_result","tool_use_id":"t","content":"a"}"#,
        r#"{"type":"tool_result","tool_use_id":"t","content":[{~"type":"text","text":"a"}]}"#,
        r#"{"type":"tool_result","tool_use_id":"t","content":[{~"type":"novel"}]}"#,
        r#"{"type":"tool_result","tool_use_id":"t","output":[~1]}"#,
        r#"{~"type":"thinking","thinking":"a","signature":"s"}"#,
        r#"{~"type":"redacted_thinking","data":"d"}"#,
        r#"{~"type":"server_tool_use","id":"s","name":"web_search","input":{}}"#,
        r#"{"type":"web_search_tool_result","tool_use_id":"s","content":[{~"type":"web_search_result","url":"u"}]}"#,
        r#"{~"type":"novel"}"#,
    ];
    let nest = |levels: usize, inner: &str| {
        let open = r#"{"type":"tool_result","tool_use_id":"t","content":["#.repeat(levels);
        format!("{open}{inner}{}", "]}".repeat(levels))
    };
    let mut bodies: Vec<String> = blocks
        .iter()
        .flat_map(|b| {
            [
                format!(r#"{{"model":"m","max_tokens":1,"messages":[{{"role":"user","content":[{b}]}}]}}"#),
                format!(r#"{{"model":"m","max_tokens":1,"system":[{b}],"messages":[]}}"#),
            ]
        })
        .collect();
    for levels in [4, 5] {
        for inner in [r#"{~"type":"text","text":"a"}"#, r#"{~"type":"novel"}"#] {
            let b = nest(levels, inner);
            bodies.push(format!(
                r#"{{"model":"m","max_tokens":1,"messages":[{{"role":"user","content":[{b}]}}]}}"#
            ));
        }
    }
    bodies.extend([
        r#"{"model":"m","max_tokens":1,"messages":[{~"role":"user","content":"a"}]}"#.to_owned(),
        r#"{"model":"m","max_tokens":1,"messages":[{"role":"user","content":"a","output":[~1]}]}"#
            .to_owned(),
    ]);
    let decoded = assert_counted_exactly_when_kept(A, &bodies);
    assert!(decoded > 40, "{decoded} of {} bodies decoded", bodies.len());
    let unknown = r#"{"model":"m","max_tokens":1,"messages":[{"role":"user","content":[{~"type":"novel"}]}]}"#;
    assert_eq!(padding_kept_and_counted(A, unknown), Some((true, PAD)));
}

#[test]
fn responses_counts_all_whitespace_inside_its_input_items() {
    let items = [
        r#"{~"role":"user","content":"a"}"#,
        r#"{~"type":"message","role":"user","content":[{"type":"input_text","text":"a"}]}"#,
        r#"{"role":"user","content":[{~"type":"input_text","text":"a"}]}"#,
        r#"{"role":"user","content":[{~"type":"input_image","image_url":"a","file_id":"b"}]}"#,
        r#"{~"type":"function_call","call_id":"c","name":"f","arguments":"{}"}"#,
        r#"{~"type":"function_call","call_id":1,"name":"f","arguments":"{}"}"#,
        r#"{~"type":"function_call_output","call_id":"c","output":"a"}"#,
        r#"{~"type":"reasoning","summary":[]}"#,
        r#"{~"type":"item_reference","id":"r"}"#,
        r#"{~"role":"critic","content":"a"}"#,
    ];
    let mut kept_raw = 0;
    for item in items {
        let body = format!(r#"{{"model":"m","input":[{item}]}}"#);
        let (kept, counted) = padding_kept_and_counted(R, &body).unwrap();
        assert_eq!(counted, PAD, "{item}");
        kept_raw += usize::from(kept);
    }
    assert!(kept_raw >= 4, "{kept_raw} items kept raw");
    for between in [
        r#"{~"model":"m","input":[{"role":"user","content":"a"}]}"#,
        r#"{"model":"m",~"input":[{"role":"user","content":"a"}]}"#,
        r#"{"model":"m","input":[~{"role":"user","content":"a"}]}"#,
        r#"{"model":"m","input":[{"role":"user","content":"a"}~]}"#,
    ] {
        assert_eq!(
            padding_kept_and_counted(R, between),
            Some((false, 0)),
            "{between}"
        );
    }
}

#[test]
fn token_ids_of_embedding_inputs_are_counted_without_the_whitespace_between_them() {
    let tight = r#"{"model":"m","input":[[1,2],[3]]}"#;
    let loose = r#"{"model":"m","input":[ [ 1 , 2 ] ,
        [3] ]}"#;
    for body in [tight, loose] {
        let s = request_shape(E, body.as_bytes()).unwrap();
        assert_eq!((s.items, s.tokens, s.bytes), (2, 3, tight.len()), "{body}");
    }
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

    let tokens = |items, tokens| {
        let mut s = counted(items, 0, 0);
        s.tokens = tokens;
        s
    };
    assert_eq!(
        shape(E, r#"{"model": "m", "input": ["a", "b", [1, 2]]}"#),
        tokens(3, 2)
    );
    assert_eq!(
        shape(E, r#"{"model": "m", "input": [1, 2, 3]}"#),
        tokens(0, 3)
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
