#![cfg(test)]

use std::path::Path;
use std::sync::Arc;

use kaeso_codecs::sse::{SseItem, parse_all};
use kaeso_codecs::{EgressCtx, EncodeCtx, HeaderList, dispatch};
use kaeso_kpp::{DialectId, KppError, Request, RequestKind};
use serde_json::{Value, json};

const A: DialectId = DialectId::AnthropicMessages;
const C: DialectId = DialectId::OpenAiChat;
const R: DialectId = DialectId::OpenAiResponses;

fn fixture(dialect: DialectId, path: &str) -> Vec<u8> {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(dialect.as_str())
        .join(path);
    std::fs::read(file).unwrap()
}

fn codex(name: &str) -> Value {
    serde_json::from_slice(&fixture(R, &format!("requests/{name}.json"))).unwrap()
}

fn decode(body: &Value) -> Request {
    let mut req = dispatch::decode_request(
        R,
        RequestKind::Generate,
        &serde_json::to_vec(body).unwrap(),
        &HeaderList::new(),
    )
    .unwrap();
    if let Request::Generate(g) = &mut req {
        g.sampling.max_output_tokens.get_or_insert(4096);
    }
    req
}

fn xai(req: &Request) -> EgressCtx {
    EgressCtx::new(req)
        .with_provider("xai")
        .with_tool_namespaces(false)
}

fn send(target: DialectId, req: &Request, ctx: &mut EgressCtx) -> Result<Value, KppError> {
    dispatch::check(target, req, ctx)?;
    let enc = dispatch::encode_request(target, req, ctx)?;
    Ok(serde_json::from_slice(&enc.body).unwrap())
}

fn tool_names(target: DialectId, body: &Value) -> Vec<String> {
    body["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| match target {
            C => t["function"]["name"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            _ => t["name"]
                .as_str()
                .or(t["type"].as_str())
                .unwrap_or_default()
                .to_owned(),
        })
        .collect()
}

fn renamed(bytes: &[u8], from: &str, to: &str) -> Vec<u8> {
    String::from_utf8(bytes.to_vec())
        .unwrap()
        .replace(from, to)
        .into_bytes()
}

fn reply_items(provider: DialectId, body: &[u8], app: &Arc<Request>, ctx: &EgressCtx) -> Value {
    let reply = dispatch::decode_response(provider, 200, &HeaderList::new(), body, ctx).unwrap();
    let out =
        dispatch::encode_response(R, &reply, &EncodeCtx::for_request(Arc::clone(app))).unwrap();
    serde_json::from_slice::<Value>(&out).unwrap()["output"].clone()
}

fn stream_events(
    provider: DialectId,
    bytes: &[u8],
    app: &Arc<Request>,
    ctx: &EgressCtx,
) -> Vec<Value> {
    let mut dec = dispatch::stream_decoder(provider, ctx).unwrap();
    let mut events = dec.feed(bytes);
    events.extend(dec.finish());
    let mut enc = dispatch::stream_encoder(R, &EncodeCtx::for_request(Arc::clone(app))).unwrap();
    let mut out = Vec::new();
    for e in &events {
        out.extend(enc.encode(e).unwrap());
    }
    out.extend(enc.finish().unwrap());
    parse_all(&out)
        .unwrap()
        .into_iter()
        .filter_map(|i| match i {
            SseItem::Event(e) => serde_json::from_str(&e.data).ok(),
            _ => None,
        })
        .collect()
}

fn calls(items: &[Value]) -> Vec<Value> {
    items
        .iter()
        .filter(|i| i["type"] == "function_call")
        .map(|i| json!({"name": i["name"], "namespace": i["namespace"]}))
        .collect()
}

fn done_items(events: &[Value]) -> Vec<Value> {
    events
        .iter()
        .filter(|e| e["type"] == "response.output_item.done")
        .map(|e| e["item"].clone())
        .collect()
}

#[test]
fn namespace_tools_flatten_for_every_target_without_namespaces() {
    let req = decode(&codex("codex-namespace-tools"));
    let flat = [
        "exec_command",
        "multi_agent_v1__spawn_agent",
        "multi_agent_v1__wait_agent",
        "mcp__fake__echo",
    ];
    for (target, mut ctx) in [
        (A, EgressCtx::new(&req)),
        (C, EgressCtx::new(&req)),
        (R, xai(&req)),
    ] {
        let mut body = codex("codex-namespace-tools");
        if target == C {
            body["tools"].as_array_mut().unwrap().pop();
        }
        let req = decode(&body);
        let sent = send(target, &req, &mut ctx).unwrap_or_else(|e| panic!("{target}: {e}"));
        assert_eq!(&tool_names(target, &sent)[..4], flat, "{target}");
        let text = sent.to_string();
        assert!(!text.contains("namespace\""), "{target}: {sent}");
        assert_eq!(
            sent.get("client_metadata").is_some(),
            target == R,
            "only the same format keeps `client_metadata`: {target}"
        );
        let call = match target {
            A => sent["messages"][2]["content"][0]["name"].clone(),
            C => sent["messages"][3]["tool_calls"][0]["function"]["name"].clone(),
            _ => sent["input"][2]["name"].clone(),
        };
        assert_eq!(call, "mcp__fake__echo", "{target}: {sent}");
    }
    let echo = match send(A, &req, &mut EgressCtx::new(&req)).unwrap()["tools"][3]["description"]
        .as_str()
    {
        Some(d) => d.to_owned(),
        None => panic!("the flattened tool has no description"),
    };
    assert_eq!(echo, "Tools in the mcp__fake namespace.\n\nEcho text back");
}

#[test]
fn openai_keeps_namespaces_and_client_metadata() {
    let body = codex("codex-namespace-tools");
    let req = decode(&body);
    let sent = send(R, &req, &mut EgressCtx::new(&req).with_provider("openai")).unwrap();
    let mut want = body.clone();
    want["max_output_tokens"] = json!(4096);
    assert_eq!(sent, want);
}

#[test]
fn namespaced_calls_come_back_with_their_namespace() {
    let app = Arc::new(decode(&codex("codex-namespace-tools")));
    let want = vec![json!({"name": "echo", "namespace": "mcp__fake"})];

    let mut ctx = xai(&app);
    send(R, &app, &mut ctx).unwrap();
    let body = fixture(R, "providers/xai/responses/recorded-tools-function.json");
    let body = renamed(&body, "\"get_current_weather\"", "\"mcp__fake__echo\"");
    let items = reply_items(R, &body, &app, &ctx);
    assert_eq!(calls(items.as_array().unwrap()), want, "xAI reply");
    let sse = fixture(R, "providers/xai/streams/recorded-function-call-stream.sse");
    let sse = renamed(&sse, "\"get_weather\"", "\"mcp__fake__echo\"");
    let events = stream_events(R, &sse, &app, &ctx);
    assert_eq!(calls(&done_items(&events)), want, "xAI stream");
    let added: Vec<Value> = events
        .iter()
        .filter(|e| {
            e["type"] == "response.output_item.added" && e["item"]["type"] == "function_call"
        })
        .map(|e| e["item"]["namespace"].clone())
        .collect();
    assert_eq!(added, vec![json!("mcp__fake")]);

    let mut ctx = EgressCtx::new(&app).with_provider("anthropic");
    send(A, &app, &mut ctx).unwrap();
    let body = fixture(A, "responses/tools-single.json");
    let body = renamed(&body, "\"get_weather\"", "\"mcp__fake__echo\"");
    let items = reply_items(A, &body, &app, &ctx);
    assert_eq!(calls(items.as_array().unwrap()), want, "Anthropic reply");
    let sse = renamed(
        &fixture(A, "streams/tools-single.sse"),
        "\"get_weather\"",
        "\"mcp__fake__echo\"",
    );
    let events = stream_events(A, &sse, &app, &ctx);
    assert_eq!(calls(&done_items(&events)), want, "Anthropic stream");
}

#[test]
fn calls_the_app_did_not_namespace_keep_their_name() {
    let app = Arc::new(decode(&codex("codex-namespace-tools")));
    let ctx = xai(&app);
    let body = fixture(R, "providers/xai/responses/recorded-tools-function.json");
    let items = reply_items(R, &body, &app, &ctx);
    assert_eq!(
        calls(items.as_array().unwrap()),
        vec![json!({"name": "get_current_weather", "namespace": null})]
    );
}

#[test]
fn a_flattened_name_that_is_taken_is_refused() {
    let mut body = codex("codex-namespace-tools");
    body["tools"].as_array_mut().unwrap().push(json!({
        "type": "function",
        "name": "mcp__fake__echo",
        "parameters": {"type": "object"},
    }));
    let req = decode(&body);
    let e = send(A, &req, &mut EgressCtx::new(&req)).unwrap_err();
    assert_eq!(e.unsupported_feature(), Some("namespace tool"));
    assert_eq!(e.param.as_deref(), Some("tools[4]"));
    assert!(e.message.contains("`mcp__fake__echo`"), "{}", e.message);
    assert!(send(R, &req, &mut EgressCtx::new(&req)).is_ok());
}

#[test]
fn namespaces_with_fields_kaeso_does_not_know_are_refused() {
    let mut body = codex("codex-namespace-tools");
    body["tools"][2]["defer_loading"] = json!(true);
    let req = decode(&body);
    let e = send(A, &req, &mut EgressCtx::new(&req)).unwrap_err();
    assert_eq!(e.unsupported_feature(), Some("tools[2].defer_loading"));
    assert_eq!(e.param.as_deref(), Some("tools[2].defer_loading"));
    body["tools"][2]
        .as_object_mut()
        .unwrap()
        .remove("defer_loading");
    body["tools"][2]["tools"][0]["defer_loading"] = json!(true);
    let req = decode(&body);
    let e = send(A, &req, &mut EgressCtx::new(&req)).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("tools[2].tools[0].defer_loading"));
    assert!(send(R, &req, &mut EgressCtx::new(&req)).is_ok());
}

#[test]
fn hosted_web_search_becomes_anthropics_server_tool() {
    let mut body = codex("codex-namespace-tools");
    body["tools"][3] = json!({
        "type": "web_search",
        "external_web_access": true,
        "search_context_size": "medium",
        "filters": {"allowed_domains": ["docs.kaeso.app"]},
        "user_location": {"type": "approximate", "city": "Berlin", "country": "DE", "region": null, "timezone": "Europe/Berlin"},
    });
    let req = decode(&body);
    let sent = send(A, &req, &mut EgressCtx::new(&req)).unwrap();
    assert_eq!(
        sent["tools"][4],
        json!({
            "type": "web_search_20250305",
            "name": "web_search",
            "allowed_domains": ["docs.kaeso.app"],
            "user_location": {"type": "approximate", "city": "Berlin", "country": "DE", "timezone": "Europe/Berlin"},
        })
    );
}

#[test]
fn web_search_settings_claude_cannot_honour_are_refused() {
    for (field, value, param) in [
        (
            "external_web_access",
            json!(false),
            "tools[3].external_web_access",
        ),
        (
            "search_context_size",
            json!("high"),
            "tools[3].search_context_size",
        ),
        (
            "filters",
            json!({"blocked_domains": ["example.com"]}),
            "tools[3].filters.blocked_domains",
        ),
        (
            "user_location",
            json!({"type": "exact"}),
            "tools[3].user_location.type",
        ),
    ] {
        let mut body = codex("codex-namespace-tools");
        body["tools"][3][field] = value;
        let req = decode(&body);
        let e = send(A, &req, &mut EgressCtx::new(&req)).unwrap_err();
        assert_eq!(e.param.as_deref(), Some(param), "{field}");
        assert!(e.message.contains("Anthropic Messages"), "{}", e.message);
    }
    let mut body = codex("codex-namespace-tools");
    body["tools"][3]["external_web_access"] = json!(false);
    let req = decode(&body);
    let e = send(A, &req, &mut EgressCtx::new(&req)).unwrap_err();
    assert!(
        e.message.contains("`web_search = \"live\"`"),
        "{}",
        e.message
    );
}

#[test]
fn claudes_web_searches_reach_the_app_as_web_search_calls() {
    let app = Arc::new(decode(&codex("codex-namespace-tools")));
    let ctx = EgressCtx::new(&app).with_provider("anthropic");
    let want = json!({
        "id": "ws_kaeso_msg01R2R14PadDFvMb1xgawpzvm_1",
        "type": "web_search_call",
        "status": "completed",
        "action": {"type": "search", "query": "claude shannon birth date"},
    });
    let items = reply_items(
        A,
        &fixture(A, "responses/web-search-server-tool.json"),
        &app,
        &ctx,
    );
    assert_eq!(items[1], want);
    assert_eq!(items.as_array().unwrap().len(), 3, "{items}");

    let events = stream_events(
        A,
        &fixture(A, "streams/web-search-server-tool.sse"),
        &app,
        &ctx,
    );
    let search: Vec<&str> = events
        .iter()
        .filter(|e| e["item_id"] == want["id"] || e["item"]["id"] == want["id"])
        .map(|e| e["type"].as_str().unwrap())
        .collect();
    assert_eq!(
        search,
        [
            "response.output_item.added",
            "response.web_search_call.in_progress",
            "response.web_search_call.searching",
            "response.web_search_call.completed",
            "response.output_item.done",
        ]
    );
    assert_eq!(done_items(&events)[1], want);
}

#[test]
fn a_search_without_results_stays_in_progress() {
    let app = Arc::new(decode(&codex("codex-namespace-tools")));
    let ctx = EgressCtx::new(&app).with_provider("anthropic");
    let items = reply_items(A, &fixture(A, "responses/pause-turn.json"), &app, &ctx);
    let statuses: Vec<&Value> = items
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["type"] == "web_search_call")
        .map(|i| &i["status"])
        .collect();
    assert_eq!(statuses, [&json!("completed"), &json!("in_progress")]);
}

#[test]
fn earlier_web_searches_are_left_out_for_claude() {
    let req = decode(&codex("codex-web-search-resume"));
    let sent = send(A, &req, &mut EgressCtx::new(&req)).unwrap();
    let turns: Vec<(&str, &str)> = sent["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| {
            (
                m["role"].as_str().unwrap(),
                m["content"][0]["text"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        &turns[1..],
        [
            ("user", "When was Claude Shannon born?"),
            ("assistant", "Claude Shannon was born on April 30, 1916."),
            ("user", "And where?"),
        ]
    );
}

#[test]
fn web_searches_kaeso_reported_are_left_out_for_openai_and_others_kept() {
    let body = codex("codex-web-search-resume");
    let req = decode(&body);
    let sent = send(R, &req, &mut EgressCtx::new(&req)).unwrap();
    assert_eq!(sent["input"][2], body["input"][2]);

    let mut ours = body.clone();
    ours["input"][2]["id"] = json!("ws_kaeso_msg01R2R14PadDFvMb1xgawpzvm_1");
    let req = decode(&ours);
    let sent = send(R, &req, &mut EgressCtx::new(&req)).unwrap();
    let types: Vec<&str> = sent["input"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["type"].as_str().unwrap())
        .collect();
    assert_eq!(types, ["message", "message", "message", "message"]);
}

#[test]
fn chat_models_refuse_hosted_web_search_with_a_codex_hint() {
    let req = decode(&codex("codex-namespace-tools"));
    let e = send(C, &req, &mut EgressCtx::new(&req)).unwrap_err();
    assert_eq!(e.unsupported_feature(), Some("web_search tool"));
    assert!(
        e.message.contains("`web_search = \"disabled\"`"),
        "{}",
        e.message
    );
}
