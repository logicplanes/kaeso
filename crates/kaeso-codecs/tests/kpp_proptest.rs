#![cfg(test)]

use kaeso_codecs::{EgressCtx, EncodeCtx, HeaderList, dispatch};
use kaeso_kpp::{
    Block, DialectId, FunctionTool, GenerateRequest, Message, RawJson, Reply, Request, RequestKind,
    Response, Role, StopReason, StreamMode, ToolDef, Usage,
};
use proptest::prelude::*;

const GENERATION: [DialectId; 3] = [
    DialectId::AnthropicMessages,
    DialectId::OpenAiChat,
    DialectId::OpenAiResponses,
];

fn text() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9 .,!?'\"\\\\é✓\n]{1,40}"
}

fn args() -> impl Strategy<Value = RawJson> {
    (text(), any::<i32>(), any::<bool>()).prop_map(|(s, n, b)| {
        RawJson::from_serialize(&serde_json::json!({"q": s, "n": n, "flag": b})).unwrap()
    })
}

#[derive(Clone, Debug)]
enum Turn {
    Text(String, String),
    Tool(String, RawJson, String),
}

fn turn() -> impl Strategy<Value = Turn> {
    prop_oneof![
        (text(), text()).prop_map(|(q, a)| Turn::Text(q, a)),
        (text(), args(), text()).prop_map(|(q, a, r)| Turn::Tool(q, a, r)),
    ]
}

prop_compose! {
    fn request()(
        system in proptest::option::of(text()),
        turns in proptest::collection::vec(turn(), 1..6),
        last in text(),
        max_tokens in 1u32..4096,
        temperature in proptest::option::of(0.0f64..1.0),
    ) -> GenerateRequest {
        let schema = RawJson::from_string(r#"{"type":"object","properties":{"q":{"type":"string"}}}"#.into()).unwrap();
        let mut req = GenerateRequest::new("model-x").with_tool(ToolDef::Function(FunctionTool::new("lookup", Some(schema))));
        if let Some(s) = system {
            req = req.with_system(s);
        }
        for (i, t) in turns.into_iter().enumerate() {
            match t {
                Turn::Text(q, a) => {
                    req.messages.push(Message::user(q));
                    req.messages.push(Message::assistant(a));
                }
                Turn::Tool(q, a, r) => {
                    let id = format!("call_{i}");
                    req.messages.push(Message::user(q));
                    req.messages.push(Message::new(Role::Assistant, vec![Block::tool_call(&id, "lookup", a)]));
                    req.messages.push(Message::new(Role::User, vec![Block::tool_result(&id, vec![Block::text(r)], false)]));
                }
            }
        }
        req.messages.push(Message::user(last));
        req.sampling.max_output_tokens = Some(max_tokens);
        req.sampling.temperature = temperature;
        req
    }
}

fn essence(req: &GenerateRequest) -> serde_json::Value {
    let mut turns = Vec::new();
    for m in &req.messages {
        for b in &m.blocks {
            match b {
                Block::Text(t) => turns.push(serde_json::json!([m.role.as_str(), "text", t.text])),
                Block::ToolCall(c) => turns.push(serde_json::json!([
                    m.role.as_str(),
                    "call",
                    c.id,
                    c.name,
                    c.arguments.to_value().unwrap()
                ])),
                Block::ToolResult(r) => {
                    let text: String = r
                        .content
                        .iter()
                        .filter_map(|b| match b {
                            Block::Text(t) => Some(t.text.as_str()),
                            _ => None,
                        })
                        .collect();
                    turns.push(serde_json::json!([
                        m.role.as_str(),
                        "result",
                        r.call_id,
                        text
                    ]));
                }
                other => turns.push(serde_json::json!([m.role.as_str(), other.kind()])),
            }
        }
    }
    let system: String = req
        .system
        .iter()
        .filter_map(|b| match b {
            Block::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect();
    serde_json::json!({
        "model": req.model,
        "system": system,
        "turns": turns,
        "max": req.sampling.max_output_tokens,
        "temperature": req.sampling.temperature,
        "tools": req.tools.iter().map(|t| match t { ToolDef::Function(f) => f.name.clone(), _ => String::new() }).collect::<Vec<_>>(),
    })
}

fn round_trip(dialect: DialectId, req: &GenerateRequest) -> GenerateRequest {
    let request = Request::Generate(req.clone());
    dispatch::check(dialect, &request).unwrap_or_else(|e| panic!("{dialect}: {e}"));
    let mut ctx = EgressCtx::new(&request);
    let enc = dispatch::encode_request(dialect, &request, &mut ctx).unwrap();
    let back = dispatch::decode_request(
        dialect,
        RequestKind::Generate,
        &enc.body,
        &HeaderList::new(),
    )
    .unwrap();
    let Request::Generate(back) = back else {
        panic!("kind changed")
    };
    back
}

#[test]
fn a_temperature_with_seventeen_significant_digits_survives_every_dialect() {
    let schema =
        RawJson::from_string(r#"{"type":"object","properties":{"q":{"type":"string"}}}"#.into())
            .unwrap();
    let mut req = GenerateRequest::new("model-x")
        .with_tool(ToolDef::Function(FunctionTool::new("lookup", Some(schema))));
    req.messages.push(Message::user(" "));
    req.messages.push(Message::assistant("é"));
    req.messages.push(Message::user("?"));
    req.sampling.max_output_tokens = Some(1);
    req.sampling.temperature = Some(0.950_729_014_766_991_7);
    for d in GENERATION {
        let back = round_trip(d, &req);
        assert_eq!(back.sampling.temperature, req.sampling.temperature, "{d}");
        assert_eq!(essence(&back), essence(&req), "{d}");
    }
}

proptest! {
    #![proptest_config(ProptestConfig::default())]

    #[test]
    fn requests_survive_every_dialect(req in request()) {
        for d in GENERATION {
            prop_assert_eq!(essence(&round_trip(d, &req)), essence(&req), "dialect {}", d);
        }
    }

    #[test]
    fn replies_survive_every_dialect(
        texts in proptest::collection::vec(text(), 1..3),
        call in proptest::option::of(args()),
        input in 0u64..100_000,
        output in 0u64..10_000,
    ) {
        let mut blocks: Vec<Block> = texts.iter().map(Block::text).collect();
        if let Some(a) = call.clone() {
            blocks.push(Block::tool_call("call_1", "lookup", a));
        }
        let mut resp = Response::new("resp_1", "model-x", blocks);
        resp.stop = if call.is_some() { StopReason::ToolUse } else { StopReason::EndTurn };
        resp.usage = Usage::new(input, output);
        let reply = Reply::Generate(resp);
        for d in GENERATION {
            let enc_ctx = EncodeCtx::new(RequestKind::Generate, StreamMode::Off).with_created_at(1);
            let out = dispatch::encode_response(d, &reply, &enc_ctx).unwrap();
            let ctx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off);
            let Reply::Generate(back) = dispatch::decode_response(d, 200, &HeaderList::new(), &out, &ctx).unwrap() else {
                panic!("kind changed")
            };
            let text: String = back.blocks.iter().filter_map(|b| match b { Block::Text(t) => Some(t.text.as_str()), _ => None }).collect();
            prop_assert_eq!(text, texts.concat(), "dialect {}", d);
            let calls: Vec<_> = back.blocks.iter().filter_map(|b| match b { Block::ToolCall(c) => Some(c.arguments.to_value().unwrap()), _ => None }).collect();
            prop_assert_eq!(calls, call.iter().map(|a| a.to_value().unwrap()).collect::<Vec<_>>(), "dialect {}", d);
            prop_assert_eq!(back.usage.input_tokens, input, "dialect {}", d);
            prop_assert_eq!(back.usage.output_tokens, output, "dialect {}", d);
        }
    }
}
