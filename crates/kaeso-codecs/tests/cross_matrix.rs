#![cfg(test)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use kaeso_codecs::{EgressCtx, EncodeCtx, HeaderList, dispatch};
use kaeso_kpp::{
    Block, Delta, DialectId, Reply, Request, RequestKind, Role, StreamEvent, StreamMode,
};
use serde_json::{Value, json};

const GENERATION: [DialectId; 3] = [
    DialectId::AnthropicMessages,
    DialectId::OpenAiChat,
    DialectId::OpenAiResponses,
];

const PROVIDERS: &str = "providers";

// Routes whose conversation changes on purpose, each change asserted in tests/codex.rs.
const CHANGED_TURNS: &[(&str, DialectId, &str)] = &[
    (
        "codex-model-switch",
        DialectId::AnthropicMessages,
        "custom and local shell calls become tool calls",
    ),
    (
        "codex-model-switch",
        DialectId::OpenAiChat,
        "custom and local shell calls become tool calls",
    ),
    (
        "providers/xai/codex-gpt-history",
        DialectId::AnthropicMessages,
        "custom and local shell calls become tool calls",
    ),
    (
        "providers/xai/codex-gpt-history",
        DialectId::OpenAiChat,
        "custom and local shell calls become tool calls",
    ),
];

fn fixtures_dir(dialect: DialectId) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(dialect.as_str())
}

fn fixtures(dialect: DialectId, folder: &str, ext: &str) -> Vec<(String, Vec<u8>)> {
    let mut out = files(&fixtures_dir(dialect).join(folder), ext, "");
    let providers = fixtures_dir(dialect).join(PROVIDERS);
    if providers.is_dir() {
        for provider in std::fs::read_dir(&providers).unwrap() {
            let provider = provider.unwrap().path();
            let own = provider.join(folder);
            if own.is_dir() {
                let id = provider.file_name().unwrap().to_str().unwrap();
                out.extend(files(&own, ext, &format!("{PROVIDERS}/{id}/")));
            }
        }
    }
    out.sort();
    out
}

fn files(dir: &Path, ext: &str, prefix: &str) -> Vec<(String, Vec<u8>)> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_file() && p.extension().and_then(|e| e.to_str()) == Some(ext))
        .filter(|p| {
            let n = p.file_name().unwrap().to_str().unwrap();
            !n.ends_with(".headers.json") && !n.ends_with(".meta.json")
        })
        .map(|p: PathBuf| {
            (
                format!("{prefix}{}", p.file_stem().unwrap().to_str().unwrap()),
                std::fs::read(&p).unwrap(),
            )
        })
        .collect()
}

fn headers(dialect: DialectId, name: &str) -> HeaderList {
    let path = match name
        .strip_prefix(PROVIDERS)
        .and_then(|rest| rest.strip_prefix('/'))
        .and_then(|rest| rest.split_once('/'))
    {
        Some((id, stem)) => fixtures_dir(dialect)
            .join(PROVIDERS)
            .join(id)
            .join("requests")
            .join(format!("{stem}.headers.json")),
        None => fixtures_dir(dialect)
            .join("requests")
            .join(format!("{name}.headers.json")),
    };
    match std::fs::read(path) {
        Ok(b) => {
            let v: Value = serde_json::from_slice(&b).unwrap();
            v.as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_owned()))
                .collect()
        }
        Err(_) => HeaderList::new(),
    }
}

fn called(call: &kaeso_kpp::ToolCallBlock) -> String {
    let namespace = call
        .extras
        .fields_for(DialectId::OpenAiResponses)
        .and_then(|f| f.get("namespace"))
        .and_then(|n| n.parse::<String>().ok());
    match namespace {
        Some(ns) => format!("{ns}__{}", call.name),
        None => call.name.clone(),
    }
}

// The cross-format rule for Anthropic: a system or developer message of another format
// joins the next user turn, after its tool results, or the last user turn.
fn as_anthropic_reads(req: &Request) -> Request {
    let mut req = req.clone();
    let Request::Generate(r) = &mut req else {
        return req;
    };
    if r.extras.is_from(DialectId::AnthropicMessages) {
        return req;
    }
    let mut held = Vec::new();
    let mut out: Vec<kaeso_kpp::Message> = Vec::new();
    for mut m in std::mem::take(&mut r.messages) {
        if m.role == Role::System {
            held.extend(m.blocks);
            continue;
        }
        if m.role == Role::User && !held.is_empty() {
            let at = m
                .blocks
                .iter()
                .take_while(|b| matches!(b, Block::ToolResult(_)))
                .count();
            m.blocks.splice(at..at, held.drain(..));
        }
        out.push(m);
    }
    if !held.is_empty() {
        match out.last_mut() {
            Some(last) if last.role == Role::User => last.blocks.append(&mut held),
            _ => out.push(kaeso_kpp::Message::new(Role::User, held)),
        }
    }
    r.messages = out;
    req
}

fn referenced_tool(item: &kaeso_kpp::RawItem) -> Option<String> {
    let v = item.raw.to_value().ok()?;
    (v["type"] == "tool_reference").then(|| v["tool_name"].as_str().map(str::to_owned))?
}

fn request_meaning(req: &Request) -> Value {
    let Request::Generate(r) = req else {
        return Value::Null;
    };
    let system: String = r
        .system
        .iter()
        .filter_map(|b| match b {
            Block::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("");
    let mut turns = Vec::new();
    for m in &r.messages {
        for b in &m.blocks {
            let item = match b {
                Block::Text(t) if !t.text.is_empty() => json!({"text": t.text}),
                Block::ToolCall(c) => {
                    json!({"call": called(c), "args": c.arguments.to_value().ok()})
                }
                Block::ToolResult(t) => json!({"result": t.content.iter().filter_map(|b| match b {
                    Block::Text(t) => Some(t.text.clone()),
                    Block::Unknown(item) => referenced_tool(item),
                    _ => None,
                }).collect::<Vec<_>>().join("")}),
                Block::Image(_) => json!("image"),
                Block::Document(_) => json!("document"),
                _ => continue,
            };
            turns.push(json!({"role": m.role.as_str(), "item": item}));
        }
    }
    json!({"system": system, "turns": turns, "tools": r.tools.len(), "model": r.model})
}

fn reply_meaning(reply: &Reply) -> Value {
    let Reply::Generate(r) = reply else {
        return Value::Null;
    };
    let mut text = String::new();
    let mut refusal = String::new();
    let mut calls = Vec::new();
    for b in &r.blocks {
        match b {
            Block::Text(t) => text.push_str(&t.text),
            Block::Refusal(t) => refusal.push_str(&t.text),
            Block::ToolCall(c) => {
                calls.push(json!({"name": c.name, "args": c.arguments.to_value().ok()}))
            }
            _ => {}
        }
    }
    json!({"text": text, "refusal": refusal, "calls": calls})
}

fn stream_meaning(events: &[StreamEvent]) -> Value {
    let mut text = String::new();
    let mut refusal = String::new();
    let mut calls: BTreeMap<u32, (String, String)> = BTreeMap::new();
    let mut error = None;
    for ev in events {
        match ev {
            StreamEvent::BlockStart(s) => {
                if let kaeso_kpp::BlockHeader::ToolCall(h) = &s.block {
                    calls.insert(s.index, (h.name.clone(), String::new()));
                }
            }
            StreamEvent::BlockDelta(d) => match &d.delta {
                Delta::Text(t) => text.push_str(t),
                Delta::Refusal(t) => refusal.push_str(t),
                Delta::ToolArguments(a) => {
                    if let Some(c) = calls.get_mut(&d.index) {
                        c.1.push_str(a);
                    }
                }
                _ => {}
            },
            StreamEvent::Error(e) => error = Some(format!("{:?}", e.kind)),
            _ => {}
        }
    }
    let calls: Vec<Value> = calls
        .into_values()
        .map(|(n, a)| json!({"name": n, "args": arguments_meaning(&a)}))
        .collect();
    json!({"text": text, "refusal": refusal, "calls": calls, "error": error})
}

fn arguments_meaning(text: &str) -> Option<Value> {
    if text.trim().is_empty() {
        return Some(json!({}));
    }
    serde_json::from_str(text).ok()
}

fn visible(meaning: &Value) -> String {
    format!(
        "{}{}",
        meaning["text"].as_str().unwrap_or_default(),
        meaning["refusal"].as_str().unwrap_or_default()
    )
}

fn pretty(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(bytes).into()))
}

fn snapshot_name(kind: &str, a: DialectId, b: DialectId) -> String {
    format!("{kind}__{}__to__{}", a.as_str(), b.as_str())
}

#[test]
fn requests_cross_every_dialect() {
    for a in GENERATION {
        let reqs = fixtures(a, "requests", "json");
        for b in GENERATION.into_iter().filter(|b| *b != a) {
            let mut outcomes = BTreeMap::new();
            for (name, body) in &reqs {
                let mut req = match dispatch::decode_request(
                    a,
                    RequestKind::Generate,
                    body,
                    &headers(a, name),
                ) {
                    Ok(r) => r,
                    Err(e) => {
                        outcomes.insert(name.clone(), json!({"rejected_by_source": e.unsupported_feature().unwrap_or("invalid")}));
                        continue;
                    }
                };
                if let Request::Generate(g) = &mut req
                    && b == DialectId::AnthropicMessages
                    && g.sampling.max_output_tokens.is_none()
                {
                    g.sampling.max_output_tokens = Some(4096);
                }
                let mut ctx = EgressCtx::new(&req);
                let outcome = match dispatch::check(b, &req, &EgressCtx::new(&req))
                    .and_then(|()| dispatch::encode_request(b, &req, &mut ctx))
                {
                    Ok(enc) => {
                        let back = dispatch::decode_request(
                            b,
                            RequestKind::Generate,
                            &enc.body,
                            &enc.headers,
                        )
                        .unwrap_or_else(|e| {
                            panic!("{a}->{b} {name}: target cannot decode its own output: {e}")
                        });
                        if !CHANGED_TURNS.iter().any(|(n, t, _)| n == name && *t == b) {
                            assert_eq!(
                                request_meaning(&back)["turns"],
                                request_meaning(&if b == DialectId::AnthropicMessages {
                                    as_anthropic_reads(&req)
                                } else {
                                    req.clone()
                                })["turns"],
                                "{a}->{b} {name}: conversation meaning changed",
                            );
                        }
                        json!({"path": enc.path, "body": pretty(&enc.body)})
                    }
                    Err(e) => {
                        let feature = e.unsupported_feature().map(str::to_owned);
                        assert!(
                            feature.is_some(),
                            "{a}->{b} {name}: refused without a feature name: {e:?}"
                        );
                        json!({"unsupported": feature, "message": e.message, "param": e.param})
                    }
                };
                outcomes.insert(name.clone(), outcome);
            }
            insta::assert_json_snapshot!(snapshot_name("request", a, b), outcomes);
        }
    }
}

#[test]
fn responses_cross_every_dialect() {
    for a in GENERATION {
        let bodies = fixtures(a, "responses", "json");
        for b in GENERATION.into_iter().filter(|b| *b != a) {
            let mut outcomes = BTreeMap::new();
            for (name, body) in &bodies {
                let ctx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off);
                let reply = match dispatch::decode_response(a, 200, &HeaderList::new(), body, &ctx)
                {
                    Ok(r) => r,
                    Err(e) => {
                        outcomes.insert(name.clone(), json!({"rejected_by_source": e.message}));
                        continue;
                    }
                };
                let enc_ctx = EncodeCtx::new(RequestKind::Generate, StreamMode::Off)
                    .with_created_at(1_760_000_000);
                let out = dispatch::encode_response(b, &reply, &enc_ctx)
                    .unwrap_or_else(|e| panic!("{a}->{b} {name}: cannot encode reply: {e}"));
                let back = dispatch::decode_response(b, 200, &HeaderList::new(), &out, &ctx)
                    .unwrap_or_else(|e| {
                        panic!("{a}->{b} {name}: target cannot decode its own reply: {e}")
                    });
                let (want, got) = (reply_meaning(&reply), reply_meaning(&back));
                assert_eq!(
                    visible(&got),
                    visible(&want),
                    "{a}->{b} {name}: visible text changed"
                );
                assert_eq!(
                    got["calls"], want["calls"],
                    "{a}->{b} {name}: tool calls changed"
                );
                outcomes.insert(name.clone(), pretty(&out));
            }
            insta::assert_json_snapshot!(snapshot_name("response", a, b), outcomes);
        }
    }
}

#[test]
fn streams_cross_every_dialect() {
    for a in GENERATION {
        let streams = fixtures(a, "streams", "sse");
        for b in GENERATION.into_iter().filter(|b| *b != a) {
            let mut outcomes = BTreeMap::new();
            for (name, bytes) in &streams {
                let mode = StreamMode::on(true);
                let ctx = EgressCtx::for_kind(RequestKind::Generate, mode);
                let mut dec = dispatch::stream_decoder(a, &ctx).unwrap();
                let mut events = dec.feed(bytes);
                events.extend(dec.finish());
                let enc_ctx =
                    EncodeCtx::new(RequestKind::Generate, mode).with_created_at(1_760_000_000);
                let mut enc = dispatch::stream_encoder(b, &enc_ctx).unwrap();
                let mut out = Vec::new();
                for ev in &events {
                    out.extend(
                        enc.encode(ev)
                            .unwrap_or_else(|e| panic!("{a}->{b} {name}: {e}")),
                    );
                }
                out.extend(
                    enc.finish()
                        .unwrap_or_else(|e| panic!("{a}->{b} {name}: {e}")),
                );
                let mut back = dispatch::stream_decoder(b, &ctx).unwrap();
                let mut again = back.feed(&out);
                again.extend(back.finish());
                let (want, got) = (stream_meaning(&events), stream_meaning(&again));
                assert_eq!(
                    visible(&got),
                    visible(&want),
                    "{a}->{b} {name}: visible text changed\n{}",
                    String::from_utf8_lossy(&out)
                );
                assert_eq!(
                    got["calls"],
                    want["calls"],
                    "{a}->{b} {name}: tool calls changed\n{}",
                    String::from_utf8_lossy(&out)
                );
                assert_eq!(
                    got["error"].is_null(),
                    want["error"].is_null(),
                    "{a}->{b} {name}: error presence changed\n{}",
                    String::from_utf8_lossy(&out)
                );
                outcomes.insert(
                    name.clone(),
                    Value::String(String::from_utf8_lossy(&out).into_owned()),
                );
            }
            insta::assert_json_snapshot!(snapshot_name("stream", a, b), outcomes);
        }
    }
}

#[test]
fn round_trips_through_another_dialect_keep_the_meaning() {
    for a in GENERATION {
        for (name, body) in fixtures(a, "responses", "json") {
            let ctx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off);
            let Ok(reply) = dispatch::decode_response(a, 200, &HeaderList::new(), &body, &ctx)
            else {
                continue;
            };
            for b in GENERATION.into_iter().filter(|b| *b != a) {
                let enc_ctx =
                    EncodeCtx::new(RequestKind::Generate, StreamMode::Off).with_created_at(1);
                let mid = dispatch::encode_response(b, &reply, &enc_ctx).unwrap();
                let in_b =
                    dispatch::decode_response(b, 200, &HeaderList::new(), &mid, &ctx).unwrap();
                let back = dispatch::encode_response(a, &in_b, &enc_ctx).unwrap();
                let in_a =
                    dispatch::decode_response(a, 200, &HeaderList::new(), &back, &ctx).unwrap();
                assert_eq!(
                    reply_meaning(&in_a)["calls"],
                    reply_meaning(&reply)["calls"],
                    "{a}->{b}->{a} {name}"
                );
                assert_eq!(
                    visible(&reply_meaning(&in_a)),
                    visible(&reply_meaning(&reply)),
                    "{a}->{b}->{a} {name}"
                );
            }
        }
    }
}

#[test]
fn token_counting_crosses_between_anthropic_and_responses() {
    let pairs = [
        (
            DialectId::AnthropicMessages,
            "count_tokens",
            DialectId::OpenAiResponses,
        ),
        (
            DialectId::OpenAiResponses,
            "input_tokens",
            DialectId::AnthropicMessages,
        ),
    ];
    for (a, folder, b) in pairs {
        let mut outcomes = BTreeMap::new();
        for (name, body) in fixtures(a, &format!("requests/{folder}"), "json") {
            let req =
                dispatch::decode_request(a, RequestKind::CountTokens, &body, &HeaderList::new())
                    .unwrap_or_else(|e| panic!("{a} {name}: {e}"));
            let mut ctx = EgressCtx::new(&req);
            let outcome = match dispatch::check(b, &req, &EgressCtx::new(&req))
                .and_then(|()| dispatch::encode_request(b, &req, &mut ctx))
            {
                Ok(enc) => {
                    let back = dispatch::decode_request(
                        b,
                        RequestKind::CountTokens,
                        &enc.body,
                        &enc.headers,
                    )
                    .unwrap_or_else(|e| panic!("{a}->{b} {name}: target cannot decode: {e}"));
                    assert_eq!(
                        request_meaning(&back)["turns"],
                        request_meaning(&req)["turns"],
                        "{a}->{b} {name}"
                    );
                    json!({"path": enc.path, "body": pretty(&enc.body)})
                }
                Err(e) => {
                    json!({"unsupported": e.unsupported_feature().expect("refusals name a feature"), "message": e.message, "param": e.param})
                }
            };
            outcomes.insert(name, outcome);
        }
        for (name, body) in fixtures(a, &format!("responses/{folder}"), "json") {
            let ctx = EgressCtx::for_kind(RequestKind::CountTokens, StreamMode::Off);
            let reply = dispatch::decode_response(a, 200, &HeaderList::new(), &body, &ctx).unwrap();
            let enc_ctx = EncodeCtx::new(RequestKind::CountTokens, StreamMode::Off);
            let out = dispatch::encode_response(b, &reply, &enc_ctx).unwrap();
            let back = dispatch::decode_response(b, 200, &HeaderList::new(), &out, &ctx).unwrap();
            match (&reply, &back) {
                (Reply::CountTokens(x), Reply::CountTokens(y)) => {
                    assert_eq!(x.input_tokens, y.input_tokens)
                }
                _ => panic!("{a}->{b} {name}: not a token count"),
            }
            outcomes.insert(format!("response:{name}"), pretty(&out));
        }
        insta::assert_json_snapshot!(snapshot_name("count", a, b), outcomes);
    }
}

#[test]
fn thinking_budgets_translate_to_openai_efforts() {
    for (name, want) in [
        ("thinking-enabled-budget", "medium"),
        ("thinking-interleaved-tools", "high"),
    ] {
        let body = fixtures(DialectId::AnthropicMessages, "requests", "json")
            .into_iter()
            .find(|(n, _)| n == name)
            .map(|(_, b)| b)
            .unwrap();
        let req = dispatch::decode_request(
            DialectId::AnthropicMessages,
            RequestKind::Generate,
            &body,
            &headers(DialectId::AnthropicMessages, name),
        )
        .unwrap();
        for target in [DialectId::OpenAiChat, DialectId::OpenAiResponses] {
            dispatch::check(target, &req, &EgressCtx::new(&req))
                .unwrap_or_else(|e| panic!("{name} -> {target}: {e}"));
            let enc = dispatch::encode_request(target, &req, &mut EgressCtx::new(&req)).unwrap();
            let v: Value = serde_json::from_slice(&enc.body).unwrap();
            let effort = match target {
                DialectId::OpenAiChat => v["reasoning_effort"].clone(),
                _ => v["reasoning"]["effort"].clone(),
            };
            assert_eq!(effort, json!(want), "{name} -> {target}");
        }
    }
}

fn anthropic_fixture(name: &str) -> Request {
    let body = fixtures(DialectId::AnthropicMessages, "requests", "json")
        .into_iter()
        .find(|(n, _)| n == name)
        .map(|(_, b)| b)
        .unwrap();
    dispatch::decode_request(
        DialectId::AnthropicMessages,
        RequestKind::Generate,
        &body,
        &headers(DialectId::AnthropicMessages, name),
    )
    .unwrap()
}

#[test]
fn claude_code_requests_route_to_openai() {
    let req = anthropic_fixture("claude-code-mid-conversation-system");
    for target in [DialectId::OpenAiChat, DialectId::OpenAiResponses] {
        dispatch::check(target, &req, &EgressCtx::new(&req))
            .unwrap_or_else(|e| panic!("{target}: {e}"));
        let enc = dispatch::encode_request(target, &req, &mut EgressCtx::new(&req)).unwrap();
        let v: Value = serde_json::from_slice(&enc.body).unwrap();
        assert!(v.get("context_management").is_none(), "{target}: {v}");
        let items = match target {
            DialectId::OpenAiChat => &v["messages"],
            _ => &v["input"],
        };
        let last = items.as_array().unwrap().last().unwrap();
        assert!(
            matches!(last["role"].as_str(), Some("system" | "developer")),
            "{target}: {last}"
        );
    }
    let mut changed = req.clone();
    let Request::Generate(r) = &mut changed else {
        panic!()
    };
    let system = r.messages.last_mut().unwrap();
    system.extras.fields.insert(
        "output_config".into(),
        kaeso_kpp::RawJson::from_string(r#"{"effort":"high"}"#.into()).unwrap(),
    );
    for target in [DialectId::OpenAiChat, DialectId::OpenAiResponses] {
        let e = dispatch::check(target, &changed, &EgressCtx::new(&changed)).unwrap_err();
        assert_eq!(e.unsupported_feature(), Some("messages[1].output_config"));
    }
    let mid = anthropic_fixture("mid-conversation-system-tool-results");
    for target in [DialectId::OpenAiChat, DialectId::OpenAiResponses] {
        dispatch::check(target, &mid, &EgressCtx::new(&mid))
            .unwrap_or_else(|e| panic!("{target}: {e}"));
    }
}

#[test]
fn claude_code_pre_release_fields_name_the_client_setting() {
    for (name, feature, says) in [
        (
            "context-management-clear-tool-uses",
            "context_management",
            "CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS=1",
        ),
        (
            "task-budget",
            "output_config.task_budget",
            "CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS=1",
        ),
    ] {
        let req = anthropic_fixture(name);
        for target in [DialectId::OpenAiChat, DialectId::OpenAiResponses] {
            let e = dispatch::check(target, &req, &EgressCtx::new(&req)).unwrap_err();
            assert_eq!(e.unsupported_feature(), Some(feature), "{name} -> {target}");
            assert!(
                e.message.contains(says),
                "{name} -> {target}: {}",
                e.message
            );
        }
        dispatch::check(DialectId::AnthropicMessages, &req, &EgressCtx::new(&req)).unwrap();
    }
}

#[test]
fn claude_code_server_tools_and_beta_tool_fields_get_actionable_errors() {
    let refused = |req: &Request| {
        [DialectId::OpenAiChat, DialectId::OpenAiResponses].map(|target| {
            let e = dispatch::check(target, req, &EgressCtx::new(req)).unwrap_err();
            (e.unsupported_feature().unwrap().to_owned(), e.message)
        })
    };
    let mut req = anthropic_fixture("tool-search-deferred-tools");
    dispatch::check(DialectId::AnthropicMessages, &req, &EgressCtx::new(&req)).unwrap();
    for (feature, message) in refused(&req) {
        assert_eq!(feature, "advisor_20260301 tool");
        assert!(
            message.contains("Input tag 'advisor_20260301'"),
            "{message}"
        );
    }
    let Request::Generate(r) = &mut req else {
        panic!()
    };
    r.tools.truncate(2);
    for target in [DialectId::OpenAiChat, DialectId::OpenAiResponses] {
        dispatch::check(target, &req, &EgressCtx::new(&req))
            .unwrap_or_else(|e| panic!("{target}: {e}"));
    }
    let Request::Generate(r) = &mut req else {
        panic!()
    };
    let kaeso_kpp::ToolDef::Function(f) = &mut r.tools[1] else {
        panic!()
    };
    f.extras.fields.insert(
        "eager_input_streaming".into(),
        kaeso_kpp::RawJson::from_string("true".into()).unwrap(),
    );
    for (feature, message) in refused(&req) {
        assert_eq!(feature, "tools[1].eager_input_streaming");
        assert!(
            message.contains("CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS=1"),
            "{message}"
        );
    }
}
