use kaeso_kpp::{
    Block, CacheTtl, DialectId, DocumentBlock, FunctionTool, GenerateRequest, JsonSchemaSpec,
    Message, Opaque, OpaqueKind, Origin, OutputSpec, ReasoningBlock, ReasoningEffort, Reply,
    Request, RequestKind, Response, Role, Source, StopReason, StreamMode, TextBlock, ToolChoice,
    ToolDef, Usage,
};
use serde_json::json;

use super::OpenAiChat;
use super::tests::{decode, encode, encode_reply, raw};
use crate::codec::{Codec, EgressCtx, EncodeCtx, HeaderList};
use crate::test_support::fixtures;

fn anthropic() -> Origin {
    Origin::new(DialectId::AnthropicMessages)
}

#[test]
fn a_foreign_request_becomes_idiomatic_chat() {
    let mut req = GenerateRequest::new("gpt-5.5");
    let mut sys = TextBlock::new("Be brief.");
    sys.cache = Some(kaeso_kpp::CacheHint::new(Some(CacheTtl::OneHour)));
    req.system = vec![Block::Text(sys), Block::text("Use tools.")];
    let mut reasoning = ReasoningBlock::new(anthropic());
    reasoning.text.push("thinking".into());
    reasoning.opaque.push(Opaque::new(
        anthropic(),
        OpaqueKind::AnthropicThinkingSignature,
        "sig",
    ));
    req.messages = vec![
        Message::user("Weather in Paris?"),
        Message::new(
            Role::Assistant,
            vec![
                Block::Reasoning(reasoning),
                Block::text("Let me check."),
                Block::tool_call("toolu_01", "get_weather", raw(r#"{"city":"Paris"}"#)),
            ],
        ),
        Message::new(
            Role::User,
            vec![
                Block::tool_result("toolu_01", vec![Block::text("18C")], true),
                Block::text("Thanks!"),
            ],
        ),
    ];
    let mut tool = FunctionTool::new("get_weather", Some(raw(r#"{"type":"object"}"#)));
    tool.description = Some("Weather".into());
    tool.cache = Some(kaeso_kpp::CacheHint::default());
    req.tools.push(ToolDef::Function(tool));
    req.tool_choice = Some(ToolChoice::Tool("get_weather".into()));
    req.sampling.max_output_tokens = Some(1024);
    req.sampling.stop = vec!["END".into()];
    req.user = Some("user-42".into());
    req.stream = StreamMode::on(false);
    req.output = OutputSpec::JsonSchema(JsonSchemaSpec::new(raw(r#"{"type":"object"}"#)));
    let mut config = kaeso_kpp::ReasoningConfig::default();
    config.effort = Some(ReasoningEffort::High);
    config.mode = Some(kaeso_kpp::ReasoningMode::Adaptive);
    req.reasoning = Some(config);

    let (out, _) = encode(&req);
    assert_eq!(
        out,
        json!({
            "model": "gpt-5.5",
            "messages": [
                {"role": "system", "content": [
                    {"type": "text", "text": "Be brief."},
                    {"type": "text", "text": "Use tools."}
                ]},
                {"role": "user", "content": "Weather in Paris?"},
                {"role": "assistant", "content": "Let me check.", "tool_calls": [
                    {"id": "toolu_01", "type": "function",
                     "function": {"name": "get_weather", "arguments": "{\"city\":\"Paris\"}"}}
                ]},
                {"role": "tool", "tool_call_id": "toolu_01", "content": "18C"},
                {"role": "user", "content": "Thanks!"}
            ],
            "max_completion_tokens": 1024,
            "stop": ["END"],
            "safety_identifier": "user-42",
            "stream": true,
            "stream_options": {"include_usage": true},
            "reasoning_effort": "high",
            "response_format": {"type": "json_schema", "json_schema": {
                "name": "response", "schema": {"type": "object"}}},
            "tools": [{"type": "function", "function": {
                "name": "get_weather", "description": "Weather",
                "parameters": {"type": "object"}}}],
            "tool_choice": {"type": "function", "function": {"name": "get_weather"}}
        })
    );
}

#[test]
fn tool_names_chat_rejects_are_rewritten_and_mapped_back() {
    let long = format!("mcp.files/{}", "x".repeat(80));
    let req = GenerateRequest::new("m")
        .with_message(Message::user("go"))
        .with_tool(ToolDef::Function(FunctionTool::new(long.clone(), None)));
    let (out, ctx) = encode(&req);
    let sent = out["tools"][0]["function"]["name"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(super::convert::valid_tool_name(&sent), "{sent}");
    assert_eq!(ctx.tool_names.get(&sent), Some(&long));

    let body = json!({
        "id": "c", "object": "chat.completion", "created": 1, "model": "m",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": null,
            "tool_calls": [{"id": "call_1", "type": "function",
                "function": {"name": sent, "arguments": "{}"}}]},
            "logprobs": null, "finish_reason": "tool_calls"}]
    });
    let reply = OpenAiChat::decode_response(
        200,
        &HeaderList::new(),
        &serde_json::to_vec(&body).unwrap(),
        &ctx,
    )
    .unwrap();
    let Reply::Generate(resp) = reply else {
        panic!()
    };
    let Block::ToolCall(call) = &resp.blocks[0] else {
        panic!()
    };
    assert_eq!(call.name, long);
}

#[test]
fn chat_documents_encode_as_file_parts() {
    let mut doc = DocumentBlock::new(Source::base64("application/pdf", "JVBE"));
    doc.cache = Some(kaeso_kpp::CacheHint::default());
    let req = GenerateRequest::new("m").with_message(Message::new(
        Role::User,
        vec![
            Block::Document(doc),
            Block::image(Source::base64("image/png", "iVBO")),
        ],
    ));
    let (out, _) = encode(&req);
    assert_eq!(
        out["messages"][0]["content"],
        json!([
            {"type": "file", "file": {"file_data": "data:application/pdf;base64,JVBE"}},
            {"type": "image_url", "image_url": {"url": "data:image/png;base64,iVBO"}}
        ])
    );
}

#[test]
fn a_foreign_reply_becomes_a_chat_completion() {
    let mut resp = Response::new("msg_01", "claude-x", Vec::new());
    let mut reasoning = ReasoningBlock::new(anthropic());
    reasoning.text.push("hmm".into());
    let mut cited = TextBlock::new(" world");
    cited.citations.push(kaeso_kpp::Citation::new(
        anthropic(),
        raw(r#"{"type":"char_location"}"#),
    ));
    resp.blocks = vec![
        Block::Reasoning(reasoning),
        Block::text("Hello"),
        Block::Text(cited),
        Block::tool_call("toolu_1", "f", raw(r#"{"a":1}"#)),
        Block::Unknown(kaeso_kpp::RawItem::new(
            anthropic(),
            raw(r#"{"type":"server_tool_use"}"#),
        )),
    ];
    resp.stop = StopReason::ToolUse;
    let mut usage = Usage::new(10, 5);
    usage.cache_read_tokens = 100;
    usage.cache_write_tokens = 3;
    resp.usage = usage;
    resp.extras = kaeso_kpp::Extras::new(anthropic()).with_field("stop_sequence", raw("null"));
    assert_eq!(
        encode_reply(&resp),
        json!({
            "id": "msg_01", "object": "chat.completion", "created": 1_700_000_000, "model": "claude-x",
            "choices": [{"index": 0, "message": {
                "role": "assistant", "content": "Hello world", "refusal": null, "annotations": [],
                "tool_calls": [{"id": "toolu_1", "type": "function",
                    "function": {"name": "f", "arguments": "{\"a\":1}"}}]},
                "logprobs": null, "finish_reason": "tool_calls"}],
            "usage": {"prompt_tokens": 113, "completion_tokens": 5, "total_tokens": 118,
                "prompt_tokens_details": {"cached_tokens": 100, "cache_write_tokens": 3}}
        })
    );
    resp.stop = StopReason::StopSequence("END".into());
    assert_eq!(encode_reply(&resp)["choices"][0]["finish_reason"], "stop");
    resp.stop = StopReason::Refusal;
    assert_eq!(
        encode_reply(&resp)["choices"][0]["finish_reason"],
        "content_filter"
    );
}

#[test]
fn anthropic_fixture_requests_encode_or_name_the_feature() {
    use crate::anthropic_messages::AnthropicMessages;
    let (mut encoded, mut refused) = (Vec::new(), Vec::new());
    for folder in ["requests", "requests/count_tokens"] {
        let kind = if folder.ends_with("count_tokens") {
            RequestKind::CountTokens
        } else {
            RequestKind::Generate
        };
        for (name, body) in fixtures("anthropic_messages", folder, "json") {
            let Ok(req) = AnthropicMessages::decode_request(kind, &body, &HeaderList::new()) else {
                continue;
            };
            let mut ctx = EgressCtx::new(&req);
            match OpenAiChat::encode_request(&req, &mut ctx) {
                Ok(out) => {
                    let back = decode(&out.body).unwrap_or_else(|e| panic!("{name}: {e:?}"));
                    assert!(matches!(back, Request::Generate(_)), "{name}");
                    encoded.push(name);
                }
                Err(err) => {
                    let feature = err.unsupported_feature().map(str::to_owned);
                    assert!(feature.is_some(), "{folder}/{name}: {err:?}");
                    refused.push((name, feature));
                }
            }
        }
    }
    assert!(
        encoded.len() >= 10,
        "encoded {encoded:?}, refused {refused:?}"
    );
    assert!(
        encoded.contains(&"tools-parallel".to_owned()),
        "{refused:?}"
    );
    assert!(
        refused
            .iter()
            .any(|(_, f)| f.as_deref() == Some("top_k") || f.as_deref() == Some("token counting"))
    );
}

#[test]
fn anthropic_fixture_responses_become_chat_completions() {
    use crate::anthropic_messages::AnthropicMessages;
    for (name, body) in fixtures("anthropic_messages", "responses", "json") {
        let ctx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off);
        let Ok(reply) = AnthropicMessages::decode_response(200, &HeaderList::new(), &body, &ctx)
        else {
            continue;
        };
        let out = OpenAiChat::encode_response(
            &reply,
            &EncodeCtx::new(RequestKind::Generate, StreamMode::Off),
        )
        .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let back = OpenAiChat::decode_response(200, &HeaderList::new(), &out, &ctx);
        assert!(back.is_ok(), "{name}: {back:?}");
    }
}
