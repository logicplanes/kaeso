#![cfg(test)]

use std::collections::BTreeMap;
use std::path::Path;

use kaeso_codecs::sse::{SseItem, parse_all};
use kaeso_codecs::{EgressCtx, EncodeCtx, HeaderList, dispatch};
use kaeso_kpp::{DialectId, Request, RequestKind, StreamEvent, StreamMode};
use serde_json::{Value, json};

const A: DialectId = DialectId::AnthropicMessages;
const R: DialectId = DialectId::OpenAiResponses;

fn fixture(dialect: DialectId, folder: &str, name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(dialect.as_str())
        .join(folder)
        .join(name);
    std::fs::read(path).unwrap()
}

fn decode(dialect: DialectId, kind: RequestKind, body: &[u8]) -> Request {
    dispatch::decode_request(dialect, kind, body, &HeaderList::new()).unwrap()
}

#[derive(Clone, Copy)]
struct Model {
    adaptive: Option<bool>,
    reasons: Option<bool>,
    output_limit: u32,
}

const ADAPTIVE: Model = Model {
    adaptive: Some(true),
    reasons: Some(true),
    output_limit: 64_000,
};
const BUDGET: Model = Model {
    adaptive: Some(false),
    reasons: Some(true),
    output_limit: 64_000,
};
const UNKNOWN: Model = Model {
    adaptive: None,
    reasons: None,
    output_limit: 64_000,
};

fn to_anthropic(kind: RequestKind, body: &[u8], model: Model) -> Value {
    try_to_anthropic(kind, body, model).unwrap()
}

fn try_to_anthropic(
    kind: RequestKind,
    body: &[u8],
    model: Model,
) -> Result<Value, kaeso_kpp::KppError> {
    let mut req = decode(R, kind, body);
    if let Request::Generate(g) | Request::CountTokens(g) = &mut req
        && kind == RequestKind::Generate
    {
        g.sampling
            .max_output_tokens
            .get_or_insert(model.output_limit);
    }
    let mut ctx = EgressCtx::new(&req).with_provider("anthropic");
    ctx.adaptive_thinking = model.adaptive;
    ctx.reasoning = model.reasons;
    dispatch::check(A, &req, &EgressCtx::new(&req))?;
    let enc = dispatch::encode_request(A, &req, &mut ctx)?;
    Ok(serde_json::from_slice(&enc.body).unwrap())
}

fn generate(body: Value, model: Model) -> Value {
    to_anthropic(
        RequestKind::Generate,
        &serde_json::to_vec(&body).unwrap(),
        model,
    )
}

fn ask(reasoning: Value) -> Value {
    json!({"model": "claude", "input": "Is 1071 divisible by 7?", "reasoning": reasoning, "stream": true})
}

#[test]
fn effort_and_summary_become_thinking_the_model_accepts() {
    let cases = [
        (
            json!({"effort": "medium", "summary": "auto"}),
            ADAPTIVE,
            json!({"type": "adaptive", "display": "summarized"}),
            json!("medium"),
        ),
        (
            json!({"effort": "xhigh", "summary": "concise"}),
            ADAPTIVE,
            json!({"type": "adaptive", "display": "summarized"}),
            json!("xhigh"),
        ),
        (
            json!({"effort": "high"}),
            ADAPTIVE,
            json!({"type": "adaptive"}),
            json!("high"),
        ),
        (
            json!({"effort": "low", "summary": "auto"}),
            UNKNOWN,
            json!({"type": "adaptive", "display": "summarized"}),
            json!("low"),
        ),
        (
            json!({"effort": "low", "summary": "auto"}),
            BUDGET,
            json!({"type": "enabled", "budget_tokens": 2048, "display": "summarized"}),
            Value::Null,
        ),
        (
            json!({"effort": "medium", "summary": "detailed"}),
            BUDGET,
            json!({"type": "enabled", "budget_tokens": 8192, "display": "summarized"}),
            Value::Null,
        ),
        (
            json!({"effort": "max"}),
            BUDGET,
            json!({"type": "enabled", "budget_tokens": 24576}),
            Value::Null,
        ),
        (
            json!({"summary": "auto"}),
            ADAPTIVE,
            json!({"type": "adaptive", "display": "summarized"}),
            Value::Null,
        ),
        (
            json!({"summary": "auto"}),
            BUDGET,
            json!({"type": "enabled", "budget_tokens": 8192, "display": "summarized"}),
            Value::Null,
        ),
    ];
    for (reasoning, model, thinking, effort) in cases {
        let body = generate(ask(reasoning.clone()), model);
        assert_eq!(body["thinking"], thinking, "{reasoning} → {body}");
        assert_eq!(
            body["output_config"]["effort"], effort,
            "{reasoning} → {body}"
        );
        assert_eq!(body["max_tokens"], 64_000, "{reasoning} → {body}");
    }
}

#[test]
fn no_thinking_is_turned_on_that_the_app_did_not_ask_for() {
    for model in [ADAPTIVE, BUDGET, UNKNOWN] {
        let body = generate(
            json!({"model": "claude", "input": "hi", "stream": true}),
            model,
        );
        assert!(body.get("thinking").is_none(), "{body}");
        assert!(body.get("output_config").is_none(), "{body}");
        let body = generate(ask(json!({"effort": "none", "summary": "auto"})), model);
        assert!(body.get("thinking").is_none(), "{body}");
        assert!(body.get("output_config").is_none(), "{body}");
    }
    let body = generate(ask(json!({"summary": "auto"})), UNKNOWN);
    assert!(body.get("thinking").is_none(), "{body}");
    let not_reasoning = Model {
        reasons: Some(false),
        ..ADAPTIVE
    };
    let body = generate(ask(json!({"summary": "auto"})), not_reasoning);
    assert!(body.get("thinking").is_none(), "{body}");
}

#[test]
fn what_anthropic_forbids_next_to_thinking_is_adjusted_not_refused() {
    let request = fixture(R, "requests", "reasoning-summary-sampling-forced-tool.json");
    let body = to_anthropic(RequestKind::Generate, &request, ADAPTIVE);
    assert_eq!(
        body["thinking"],
        json!({"type": "adaptive", "display": "summarized"})
    );
    assert_eq!(body["output_config"]["effort"], "low");
    assert_eq!(body["tool_choice"], json!({"type": "any"}));
    for key in ["temperature", "top_p", "top_k"] {
        assert!(body.get(key).is_none(), "{key}: {body}");
    }
    assert_eq!(body["max_tokens"], 800);
    let body = to_anthropic(RequestKind::Generate, &request, BUDGET);
    assert!(body.get("thinking").is_none(), "{body}");
    assert_eq!(body["tool_choice"], json!({"type": "any"}));
    assert_eq!(body["temperature"], 0.2);
    assert_eq!(body["top_p"], 0.9);
    assert_eq!(body["max_tokens"], 800);
    let mut v: Value = serde_json::from_slice(&request).unwrap();
    v.as_object_mut().unwrap().remove("tool_choice");
    let body = generate(v, BUDGET);
    assert_eq!(
        body["thinking"],
        json!({"type": "enabled", "budget_tokens": 1024, "display": "summarized"})
    );
    assert_eq!(body["max_tokens"], 1_824);
    assert!(body.get("temperature").is_none(), "{body}");
}

#[test]
fn token_counts_carry_thinking_without_display() {
    let body = to_anthropic(
        RequestKind::CountTokens,
        br#"{"model":"claude","input":"hi","reasoning":{"effort":"low","summary":"auto"}}"#,
        ADAPTIVE,
    );
    assert_eq!(body["thinking"], json!({"type": "adaptive"}));
    assert!(body.get("max_tokens").is_none(), "{body}");
}

#[test]
fn anthropic_apps_on_their_own_dialect_are_unchanged() {
    let own = br#"{"model":"claude-haiku-4-5","max_tokens":2000,"temperature":0.5,"thinking":{"type":"enabled","budget_tokens":1500},"tool_choice":{"type":"any"},"tools":[{"name":"f","input_schema":{"type":"object"}}],"messages":[{"role":"user","content":"hi"}]}"#;
    let req = decode(A, RequestKind::Generate, own);
    let mut ctx = EgressCtx::new(&req)
        .with_adaptive_thinking(false)
        .with_reasoning(true);
    let enc = dispatch::encode_request(A, &req, &mut ctx).unwrap();
    let sent: Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(sent, serde_json::from_slice::<Value>(own).unwrap());
}

fn anthropic_to_responses(thinking: Value, effort: Option<&str>, reasons: Option<bool>) -> Value {
    let mut body = json!({"model": "gpt-5.5", "max_tokens": 4000, "thinking": thinking,
        "messages": [{"role": "user", "content": "hi"}]});
    if let Some(e) = effort {
        body["output_config"] = json!({"effort": e});
    }
    let req = decode(
        A,
        RequestKind::Generate,
        &serde_json::to_vec(&body).unwrap(),
    );
    let mut ctx = EgressCtx::new(&req).with_provider("openai");
    ctx.reasoning = reasons;
    let enc = dispatch::encode_request(R, &req, &mut ctx).unwrap();
    serde_json::from_slice(&enc.body).unwrap()
}

#[test]
fn anthropic_apps_on_openai_ask_for_reasoning_summaries() {
    let b = anthropic_to_responses(json!({"type": "adaptive"}), Some("high"), None);
    assert_eq!(b["reasoning"], json!({"effort": "high", "summary": "auto"}));
    assert_eq!(b["include"], json!(["reasoning.encrypted_content"]));
    let b = anthropic_to_responses(
        json!({"type": "enabled", "budget_tokens": 10_000}),
        None,
        None,
    );
    assert_eq!(
        b["reasoning"],
        json!({"effort": "medium", "summary": "auto"})
    );
    let b = anthropic_to_responses(
        json!({"type": "adaptive", "display": "summarized"}),
        None,
        Some(true),
    );
    assert_eq!(b["reasoning"], json!({"summary": "auto"}));
    assert_eq!(b["include"], json!(["reasoning.encrypted_content"]));
    let b = anthropic_to_responses(
        json!({"type": "adaptive", "display": "omitted"}),
        Some("low"),
        None,
    );
    assert_eq!(b["reasoning"], json!({"effort": "low"}));
    assert_eq!(b["include"], json!(["reasoning.encrypted_content"]));
    let b = anthropic_to_responses(json!({"type": "adaptive"}), None, None);
    assert!(b.get("reasoning").is_none(), "{b}");
    assert!(b.get("include").is_none(), "{b}");
    let b = anthropic_to_responses(json!({"type": "disabled"}), None, Some(true));
    assert_eq!(b["reasoning"], json!({"effort": "none"}));
    assert!(b.get("include").is_none(), "{b}");
}

fn responses_stream(name: &str) -> (Vec<Value>, usize) {
    let mode = StreamMode::on(true);
    let ctx = EgressCtx::for_kind(RequestKind::Generate, mode).with_provider("anthropic");
    let mut dec = dispatch::stream_decoder(A, &ctx).unwrap();
    let mut events: Vec<StreamEvent> = dec.feed(&fixture(A, "streams", name));
    events.extend(dec.finish());
    let app = decode(
        R,
        RequestKind::Generate,
        br#"{"model":"claude","input":"q","reasoning":{"effort":"medium","summary":"auto"},"stream":true}"#,
    );
    let mut enc =
        dispatch::stream_encoder(R, &EncodeCtx::for_request(app).with_created_at(1)).unwrap();
    let mut out = Vec::new();
    for e in &events {
        out.extend(enc.encode(e).unwrap());
    }
    out.extend(enc.finish().unwrap());
    let mut datas = Vec::new();
    let mut keepalives = 0;
    for item in parse_all(&out).unwrap() {
        match item {
            SseItem::Event(e) => {
                let v: Value = serde_json::from_str(&e.data).unwrap();
                assert_eq!(e.event.as_deref(), v["type"].as_str(), "{v}");
                datas.push(v);
            }
            _ => keepalives += 1,
        }
    }
    (datas, keepalives)
}

fn check_reasoning_events(events: &[Value]) -> Vec<Value> {
    let types: Vec<&str> = events.iter().map(|e| e["type"].as_str().unwrap()).collect();
    assert_eq!(types.first(), Some(&"response.created"));
    assert_eq!(types.get(1), Some(&"response.in_progress"));
    assert_eq!(types.last(), Some(&"response.completed"));
    for (i, e) in events.iter().enumerate() {
        assert_eq!(e["sequence_number"], i, "{e}");
    }
    let mut open: BTreeMap<u64, Value> = BTreeMap::new();
    let mut closed: Vec<Value> = Vec::new();
    let mut summary: BTreeMap<(u64, u64), (String, &str)> = BTreeMap::new();
    for e in events {
        let ty = e["type"].as_str().unwrap();
        let oi = e["output_index"].as_u64();
        match ty {
            "response.output_item.added" => {
                let oi = oi.unwrap();
                assert_eq!(oi as usize, closed.len() + open.len(), "{e}");
                if e["item"]["type"] == "reasoning" {
                    assert_eq!(e["item"]["summary"], json!([]), "{e}");
                    assert!(e["item"]["id"].as_str().unwrap().starts_with("rs_"));
                }
                open.insert(oi, e["item"].clone());
            }
            t if t.starts_with("response.reasoning_summary_") => {
                let oi = oi.unwrap();
                let item = open.get(&oi).unwrap_or_else(|| panic!("closed item: {e}"));
                assert_eq!(item["type"], "reasoning");
                assert_eq!(e["item_id"], item["id"], "{e}");
                let si = e["summary_index"].as_u64().unwrap();
                match t {
                    "response.reasoning_summary_part.added" => {
                        let before = summary.keys().filter(|(o, _)| *o == oi).count() as u64;
                        assert_eq!(si, before, "summary indices are consecutive: {e}");
                        assert_eq!(e["part"], json!({"type": "summary_text", "text": ""}));
                        summary.insert((oi, si), (String::new(), "open"));
                    }
                    "response.reasoning_summary_text.delta" => {
                        let (text, state) = summary.get_mut(&(oi, si)).unwrap();
                        assert_eq!(*state, "open", "{e}");
                        text.push_str(e["delta"].as_str().unwrap());
                    }
                    "response.reasoning_summary_text.done" => {
                        let (text, state) = summary.get_mut(&(oi, si)).unwrap();
                        assert_eq!(*state, "open", "{e}");
                        assert_eq!(e["text"], json!(text), "{e}");
                        *state = "text done";
                    }
                    "response.reasoning_summary_part.done" => {
                        let (text, state) = summary.get_mut(&(oi, si)).unwrap();
                        assert_eq!(*state, "text done", "{e}");
                        assert_eq!(
                            e["part"],
                            json!({"type": "summary_text", "text": text}),
                            "{e}"
                        );
                        *state = "done";
                    }
                    other => panic!("unexpected event {other}"),
                }
            }
            "response.output_item.done" => {
                let oi = oi.unwrap();
                let added = open.remove(&oi).unwrap_or_else(|| panic!("not open: {e}"));
                assert_eq!(e["item"]["id"], added["id"]);
                assert_eq!(oi as usize, closed.len(), "items close in order: {e}");
                if e["item"]["type"] == "reasoning" {
                    let parts: Vec<Value> = summary
                        .iter()
                        .filter(|((o, _), _)| *o == oi)
                        .map(|(_, (text, state))| {
                            assert_eq!(*state, "done");
                            json!({"type": "summary_text", "text": text})
                        })
                        .collect();
                    assert_eq!(e["item"]["summary"], json!(parts), "{e}");
                    assert!(
                        e["item"]["encrypted_content"]
                            .as_str()
                            .unwrap()
                            .starts_with("kpp:reasoning:v1:")
                    );
                }
                closed.push(e["item"].clone());
            }
            _ => {}
        }
    }
    assert!(open.is_empty(), "unclosed items: {open:?}");
    let completed = &events.last().unwrap()["response"];
    assert_eq!(completed["status"], "completed");
    assert_eq!(completed["output"], json!(closed));
    closed
        .into_iter()
        .filter(|i| i["type"] == "reasoning")
        .collect()
}

#[test]
fn every_anthropic_thinking_stream_streams_like_openai_reasoning() {
    let streams = [
        ("thinking-enabled-budget.sse", 1, 1),
        ("thinking-display-omitted.sse", 1, 0),
        ("recorded-thinking-display-omitted.sse", 1, 0),
        ("thinking-redacted.sse", 2, 1),
        ("thinking-interleaved-tools.sse", 1, 1),
        ("thinking-long-tool-loop.sse", 1, 1),
    ];
    for (name, items, readable) in streams {
        let (events, _) = responses_stream(name);
        let reasoning = check_reasoning_events(&events);
        assert_eq!(reasoning.len(), items, "{name}");
        let with_text = reasoning
            .iter()
            .filter(|r| r["summary"].as_array().is_some_and(|s| !s.is_empty()))
            .count();
        assert_eq!(with_text, readable, "{name}");
    }
    let own = fixture(R, "streams", "reasoning-summary-stream.sse");
    let datas: Vec<Value> = parse_all(&own)
        .unwrap()
        .into_iter()
        .filter_map(|i| match i {
            SseItem::Event(e) => serde_json::from_str(&e.data).ok(),
            _ => None,
        })
        .collect();
    let encrypted_is_openai = |v: &Value| {
        let mut v = v.clone();
        for item in v["response"]["output"].as_array_mut().into_iter().flatten() {
            if item["type"] == "reasoning" {
                item["encrypted_content"] = json!("kpp:reasoning:v1:x");
            }
        }
        if v["item"]["type"] == "reasoning" && v["type"] == "response.output_item.done" {
            v["item"]["encrypted_content"] = json!("kpp:reasoning:v1:x");
        }
        v
    };
    let datas: Vec<Value> = datas.iter().map(encrypted_is_openai).collect();
    check_reasoning_events(&datas);
}

#[test]
fn long_thinking_streams_reasoning_deltas_as_it_arrives() {
    let (events, _) = responses_stream("thinking-long-tool-loop.sse");
    let deltas = events
        .iter()
        .filter(|e| e["type"] == "response.reasoning_summary_text.delta")
        .count();
    assert_eq!(deltas, 40);
}

#[test]
fn complete_replies_show_thinking_as_reasoning_items() {
    let ctx =
        EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off).with_provider("anthropic");
    for (name, summaries) in [
        ("thinking-enabled-budget.json", vec![true]),
        ("thinking-display-omitted.json", vec![false]),
        ("thinking-redacted.json", vec![true, false]),
        ("thinking-adaptive-interleaved-tools.json", vec![false]),
    ] {
        let reply = dispatch::decode_response(
            A,
            200,
            &HeaderList::new(),
            &fixture(A, "responses", name),
            &ctx,
        )
        .unwrap();
        let out: Value = serde_json::from_slice(
            &dispatch::encode_response(
                R,
                &reply,
                &EncodeCtx::new(RequestKind::Generate, StreamMode::Off).with_created_at(1),
            )
            .unwrap(),
        )
        .unwrap();
        let reasoning: Vec<&Value> = out["output"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| i["type"] == "reasoning")
            .collect();
        let shown: Vec<bool> = reasoning
            .iter()
            .map(|r| !r["summary"].as_array().unwrap().is_empty())
            .collect();
        assert_eq!(shown, summaries, "{name}: {out}");
        for r in reasoning {
            assert!(r["id"].as_str().unwrap().starts_with("rs_kaeso_"));
            assert!(
                r["encrypted_content"]
                    .as_str()
                    .unwrap()
                    .starts_with("kpp:reasoning:v1:")
            );
        }
        if name == "thinking-enabled-budget.json" {
            assert!(
                out["usage"]["output_tokens_details"]["reasoning_tokens"]
                    .as_u64()
                    .unwrap()
                    > 0
            );
        }
    }
}

#[test]
fn a_streamed_tool_loop_sends_signed_and_redacted_thinking_back_in_order() {
    let (events, _) = responses_stream("thinking-redacted.sse");
    let output = events.last().unwrap()["response"]["output"].clone();
    let mut input = vec![json!({"role": "user", "content": "Can I see the aurora tonight?"})];
    for item in output.as_array().unwrap() {
        input.push(item.clone());
        if item["type"] == "function_call" {
            input.push(json!({"type": "function_call_output", "call_id": item["call_id"], "output": "{\"cloud_cover\":0.1}"}));
        }
    }
    let next = json!({"model": "claude", "input": input, "stream": true,
        "reasoning": {"effort": "medium", "summary": "auto"},
        "include": ["reasoning.encrypted_content"], "store": false,
        "tools": [{"type": "function", "name": "get_weather", "parameters": {"type": "object", "properties": {}}}]});
    let body = generate(next, BUDGET);
    assert_eq!(
        body["thinking"],
        json!({"type": "enabled", "budget_tokens": 8192, "display": "summarized"})
    );
    let original: Value =
        serde_json::from_slice(&fixture(A, "responses", "thinking-redacted.json")).unwrap();
    let sent = &body["messages"][1];
    assert_eq!(sent["role"], "assistant");
    let blocks = sent["content"].as_array().unwrap();
    let kinds: Vec<&str> = blocks.iter().map(|b| b["type"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["thinking", "redacted_thinking", "tool_use"]);
    for (b, o) in blocks.iter().zip(original["content"].as_array().unwrap()) {
        match o["type"].as_str().unwrap() {
            "thinking" => {
                assert_eq!(b["thinking"], o["thinking"]);
                assert_eq!(b["signature"], o["signature"]);
            }
            "redacted_thinking" => assert_eq!(b["data"], o["data"]),
            "tool_use" => {
                assert_eq!(b["id"], o["id"]);
                assert_eq!(b["input"], o["input"]);
            }
            other => panic!("unexpected {other}"),
        }
    }
    assert_eq!(body["messages"][2]["content"][0]["type"], "tool_result");
}

#[test]
fn a_tool_loop_without_the_reasoning_item_keeps_adaptive_and_drops_budget_thinking() {
    let input = json!([
        {"role": "user", "content": "Weather in Oslo?"},
        {"type": "function_call", "call_id": "toolu_01", "name": "get_weather", "arguments": "{\"city\":\"Oslo\"}"},
        {"type": "function_call_output", "call_id": "toolu_01", "output": "rain"}
    ]);
    let req = |model| {
        generate(
            json!({"model": "claude", "input": input, "reasoning": {"effort": "high", "summary": "auto"},
                "tools": [{"type": "function", "name": "get_weather", "parameters": {"type": "object", "properties": {}}}]}),
            model,
        )
    };
    let body = req(BUDGET);
    assert!(body.get("thinking").is_none(), "{body}");
    let body = req(ADAPTIVE);
    assert_eq!(
        body["thinking"],
        json!({"type": "adaptive", "display": "summarized"})
    );
}
