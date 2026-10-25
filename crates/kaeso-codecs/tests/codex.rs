#![cfg(test)]

use std::path::Path;
use std::sync::Arc;

use kaeso_codecs::openai_chat::ChatProfile;
use kaeso_codecs::openai_responses::ResponsesProfile;
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
        .with_responses_profile(
            ResponsesProfile::compatible("SpaceX")
                .xai_web_search()
                .without_null_reasoning_content(),
        )
}

fn openai(req: &Request) -> EgressCtx {
    EgressCtx::new(req)
        .with_provider("openai")
        .with_responses_profile(ResponsesProfile::OPENAI)
}

fn mistral(req: &Request) -> EgressCtx {
    EgressCtx::new(req)
        .with_provider("mistral")
        .with_chat_profile(ChatProfile::compatible("Mistral").system_role())
}

fn sent_with(target: DialectId, req: &Request, mut ctx: EgressCtx) -> (Value, Vec<String>) {
    let body = send(target, req, &mut ctx).unwrap_or_else(|e| panic!("{target}: {e}"));
    (body, ctx.left_out)
}

fn texts(blocks: &Value) -> Vec<&str> {
    blocks
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["text"].as_str().unwrap())
        .collect()
}

fn roles(messages: &Value) -> Vec<&str> {
    messages
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["role"].as_str().unwrap())
        .collect()
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
    let sent = send(R, &req, &mut openai(&req)).unwrap();
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
    body["tools"][3]["external_web_access"] = json!("yes");
    let req = decode(&body);
    let e = send(A, &req, &mut EgressCtx::new(&req)).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("tools[3].external_web_access"));
    assert!(e.message.contains("`true` or `false`"), "{}", e.message);
}

#[test]
fn a_cached_only_web_search_is_left_out_where_search_is_always_live() {
    let mut body = codex("codex-developer-resume");
    body["tools"][1]["external_web_access"] = json!(false);
    let req = decode(&body);
    for (target, ctx) in [(A, EgressCtx::new(&req)), (R, xai(&req))] {
        let (sent, left_out) = sent_with(target, &req, ctx);
        assert_eq!(tool_names(target, &sent), ["exec_command"], "{target}");
        assert_eq!(left_out, ["tools[1]"], "{target}");
    }
    let (sent, left_out) = sent_with(R, &req, openai(&req));
    assert_eq!(sent["tools"][1], body["tools"][1]);
    assert!(left_out.is_empty());
}

#[test]
fn every_hosted_search_type_is_treated_as_web_search() {
    for ty in [
        "web_search_preview",
        "web_search_preview_2025_03_11",
        "web_search_2025_08_26",
    ] {
        let mut body = codex("codex-developer-resume");
        body["tools"][1] = json!({"type": ty, "search_context_size": "medium"});
        let req = decode(&body);
        let (sent, _) = sent_with(A, &req, EgressCtx::new(&req));
        assert_eq!(sent["tools"][1]["type"], "web_search_20250305", "{ty}");
        let (sent, _) = sent_with(R, &req, xai(&req));
        assert_eq!(sent["tools"][1], json!({"type": "web_search"}), "{ty}");
        let (sent, left_out) = sent_with(C, &req, mistral(&req));
        assert_eq!(tool_names(C, &sent), ["exec_command"], "{ty}");
        assert_eq!(left_out, ["tools[1]"], "{ty}");
    }
}

#[test]
fn image_results_are_left_out_for_claude_and_asked_of_xai() {
    let mut body = codex("codex-developer-resume");
    body["tools"][1]["search_content_types"] = json!(["text", "image"]);
    let req = decode(&body);
    let (sent, left_out) = sent_with(A, &req, EgressCtx::new(&req));
    assert_eq!(
        sent["tools"][1],
        json!({"type": "web_search_20250305", "name": "web_search"})
    );
    assert_eq!(left_out, ["tools[1].search_content_types"]);
    let (sent, left_out) = sent_with(R, &req, xai(&req));
    assert_eq!(
        sent["tools"][1],
        json!({"type": "web_search", "enable_image_search": true})
    );
    assert!(left_out.is_empty());

    body["tools"][1]["search_content_types"] = json!(["text"]);
    let req = decode(&body);
    assert!(sent_with(A, &req, EgressCtx::new(&req)).1.is_empty());
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

fn decode_as(dialect: DialectId, body: &Value) -> Request {
    dispatch::decode_request(
        dialect,
        RequestKind::Generate,
        &serde_json::to_vec(body).unwrap(),
        &HeaderList::new(),
    )
    .unwrap()
}

fn content_types(message: &Value) -> Vec<&str> {
    message["content"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["type"].as_str().unwrap())
        .collect()
}

#[test]
fn leading_developer_messages_become_claudes_system_prompt() {
    let body = codex("codex-developer-resume");
    let req = decode(&body);
    let Request::Generate(g) = &req else {
        panic!("not a generation")
    };
    assert_eq!(
        g.system.len(),
        4,
        "the decoder hoists items with an `id` too"
    );
    let (sent, _) = sent_with(A, &req, EgressCtx::new(&req));
    let developer: Vec<&str> = body["input"].as_array().unwrap()[..2]
        .iter()
        .flat_map(|m| texts(&m["content"]))
        .collect();
    assert_eq!(texts(&sent["system"]), developer);
    assert_eq!(
        roles(&sent["messages"]),
        ["user", "user", "assistant", "user"]
    );
}

#[test]
fn instructions_lead_the_hoisted_developer_messages() {
    let mut body = codex("codex-developer-resume");
    body["instructions"] = json!("Answer briefly.");
    let req = decode(&body);
    let (sent, _) = sent_with(A, &req, EgressCtx::new(&req));
    let system = texts(&sent["system"]);
    assert_eq!(system[0], "Answer briefly.");
    assert_eq!(system[1], body["input"][0]["content"][0]["text"]);
    assert_eq!(system.len(), 5);
}

#[test]
fn openai_gets_codex_requests_unchanged() {
    for name in ["codex-developer-resume", "codex-model-switch"] {
        let body = codex(name);
        let req = decode(&body);
        let (sent, left_out) = sent_with(R, &req, openai(&req));
        let mut want = body.clone();
        want["max_output_tokens"] = json!(4096);
        assert_eq!(sent, want, "{name}");
        assert!(left_out.is_empty(), "{name}");
    }
}

#[test]
fn later_developer_messages_join_the_next_user_turn_for_claude() {
    let body = codex("codex-model-switch");
    let req = decode(&body);
    let (sent, _) = sent_with(A, &req, EgressCtx::new(&req));
    assert_eq!(texts(&sent["system"]).len(), 4);
    assert!(!roles(&sent["messages"]).contains(&"system"), "{sent}");
    let last = sent["messages"].as_array().unwrap().last().unwrap();
    let text = texts(&last["content"]);
    assert!(text[0].starts_with("<model_switch>"), "{text:?}");
    assert!(text[1].starts_with("<collaboration_mode>"), "{text:?}");
    assert_eq!(text[2], "What did you do?");
}

#[test]
fn a_later_developer_message_without_a_user_turn_after_it_follows_the_tool_results() {
    let mut body = codex("codex-model-switch");
    body["input"].as_array_mut().unwrap().pop();
    let req = decode(&body);
    let (sent, _) = sent_with(A, &req, EgressCtx::new(&req));
    let last = sent["messages"].as_array().unwrap().last().unwrap();
    assert_eq!(last["role"], "user");
    assert_eq!(content_types(last), ["tool_result", "text", "text"]);

    let mut body = codex("codex-developer-resume");
    let input = body["input"].as_array_mut().unwrap();
    input.pop();
    input.push(json!({"type": "message", "role": "developer", "content": "Be brief."}));
    let req = decode(&body);
    let (sent, _) = sent_with(A, &req, EgressCtx::new(&req));
    let last = sent["messages"].as_array().unwrap().last().unwrap();
    assert_eq!(last["role"], "user");
    assert_eq!(texts(&last["content"]), ["Be brief."]);
}

#[test]
fn chat_system_messages_reach_claude_as_system_prompt_and_user_turn_text() {
    let body = json!({
        "model": "m",
        "max_tokens": 1024,
        "messages": [
            {"role": "system", "content": "You are a coding assistant."},
            {"role": "developer", "content": "Prefer small diffs."},
            {"role": "user", "content": "Fix the bug."},
            {"role": "assistant", "content": "Done."},
            {"role": "system", "content": "Reminder: reply with a diff."},
            {"role": "user", "content": "And the test?"},
        ],
    });
    let req = decode_as(C, &body);
    let (sent, _) = sent_with(A, &req, EgressCtx::new(&req));
    assert_eq!(
        texts(&sent["system"]),
        ["You are a coding assistant.", "Prefer small diffs."]
    );
    assert_eq!(roles(&sent["messages"]), ["user", "assistant", "user"]);
    assert_eq!(
        texts(&sent["messages"][2]["content"]),
        ["Reminder: reply with a diff.", "And the test?"]
    );
}

#[test]
fn anthropic_system_messages_stay_messages() {
    let body = json!({
        "model": "claude-sonnet-5-5",
        "max_tokens": 1024,
        "messages": [
            {"role": "system", "content": "Be brief."},
            {"role": "user", "content": "Hi"},
            {"role": "assistant", "content": "Hello."},
            {"role": "user", "content": "Bye"},
            {"role": "system", "content": "Say goodbye."},
        ],
    });
    let req = decode_as(A, &body);
    let (sent, _) = sent_with(A, &req, EgressCtx::new(&req));
    assert_eq!(sent, body);
}

#[test]
fn chat_models_get_hoisted_developer_messages_as_their_system_prompt() {
    let req = decode(&codex("codex-model-switch"));
    let (sent, _) = sent_with(C, &req, mistral(&req));
    let roles = roles(&sent["messages"]);
    assert_eq!(roles[0], "system");
    assert_eq!(
        roles.iter().filter(|r| **r == "system").count(),
        3,
        "{roles:?}"
    );
}

#[test]
fn chat_models_get_codex_requests_without_hosted_web_search() {
    let req = decode(&codex("codex-developer-resume"));
    let (sent, left_out) = sent_with(C, &req, mistral(&req));
    assert_eq!(tool_names(C, &sent), ["exec_command"]);
    assert_eq!(left_out, ["tools[1]"]);

    let req = decode(&codex("codex-web-search-resume"));
    let (sent, _) = sent_with(C, &req, mistral(&req));
    assert!(!sent.to_string().contains("web_search"), "{sent}");
}

#[test]
fn chat_models_refuse_a_forced_hosted_web_search() {
    let mut body = codex("codex-developer-resume");
    body["tool_choice"] = json!({"type": "web_search"});
    let req = decode(&body);
    let e = send(C, &req, &mut mistral(&req)).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("tool_choice"), "{e}");
}

#[test]
fn tool_settings_go_with_web_search_when_it_was_the_only_tool() {
    let mut body = codex("codex-developer-resume");
    body["tools"].as_array_mut().unwrap().remove(0);
    let req = decode(&body);
    let (sent, _) = sent_with(C, &req, mistral(&req));
    for key in ["tools", "tool_choice", "parallel_tool_calls"] {
        assert!(sent.get(key).is_none(), "{key}: {sent}");
    }

    body["tool_choice"] = json!("required");
    let req = decode(&body);
    let e = send(C, &req, &mut mistral(&req)).unwrap_err();
    assert_eq!(e.unsupported_feature(), Some("tool_choice `required`"));
    assert_eq!(e.param.as_deref(), Some("tool_choice"));
}

#[test]
fn gpt_tool_history_reaches_claude_and_chat_models_as_tool_calls() {
    let body = codex("codex-model-switch");
    let patch = body["input"][7]["input"].clone();
    let req = decode(&body);
    let (sent, _) = sent_with(A, &req, EgressCtx::new(&req));
    let uses: Vec<Value> = sent["messages"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|m| m["content"].as_array().unwrap())
        .filter(|b| b["type"] == "tool_use" || b["type"] == "tool_result")
        .map(|b| {
            json!([
                b["type"],
                b["id"].as_str().or(b["tool_use_id"].as_str()),
                b["name"]
            ])
        })
        .collect();
    assert_eq!(
        uses,
        [
            json!(["tool_use", "call_patch_1", "apply_patch"]),
            json!(["tool_result", "call_patch_1", null]),
            json!(["tool_use", "call_shell_2", "local_shell"]),
            json!(["tool_result", "call_shell_2", null]),
        ]
    );
    let patch_use = sent["messages"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|m| m["content"].as_array().unwrap())
        .find(|b| b["name"] == "apply_patch")
        .unwrap();
    assert_eq!(patch_use["input"], json!({"input": patch}));

    let (sent, _) = sent_with(C, &req, mistral(&req));
    let calls: Vec<(&str, &str)> = sent["messages"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|m| m["tool_calls"].as_array().into_iter().flatten())
        .map(|c| {
            (
                c["id"].as_str().unwrap(),
                c["function"]["name"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        calls,
        [
            ("call_patch_1", "apply_patch"),
            ("call_shell_2", "local_shell")
        ]
    );
    let shell = sent["messages"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|m| m["tool_calls"].as_array().into_iter().flatten())
        .find(|c| c["function"]["name"] == "local_shell")
        .unwrap();
    let args: Value =
        serde_json::from_str(shell["function"]["arguments"].as_str().unwrap()).unwrap();
    assert_eq!(args, body["input"][9]["action"]);
}

#[test]
fn verbosity_reaches_chat_models_that_take_it_and_is_left_out_elsewhere() {
    let mut body = codex("codex-developer-resume");
    body["tools"].as_array_mut().unwrap().pop();
    body["text"] = json!({"verbosity": "low"});
    let req = decode(&body);
    let (sent, left_out) = sent_with(C, &req, EgressCtx::new(&req));
    assert_eq!(sent["verbosity"], "low");
    assert!(left_out.is_empty());
    for (target, ctx) in [(C, mistral(&req)), (A, EgressCtx::new(&req))] {
        let (sent, left_out) = sent_with(target, &req, ctx);
        assert!(!sent.to_string().contains("verbosity"), "{target}: {sent}");
        assert_eq!(left_out, ["text.verbosity"], "{target}");
    }
}

#[test]
fn xai_gets_its_own_web_search_fields() {
    let mut body = codex("codex-developer-resume");
    body["tools"][1] = json!({
        "type": "web_search",
        "external_web_access": true,
        "search_context_size": "medium",
        "filters": {"allowed_domains": ["docs.kaeso.app"]},
        "enable_image_understanding": true,
    });
    let req = decode(&body);
    let (sent, _) = sent_with(R, &req, xai(&req));
    assert_eq!(
        sent["tools"][1],
        json!({
            "type": "web_search",
            "filters": {"allowed_domains": ["docs.kaeso.app"]},
            "enable_image_understanding": true,
        })
    );
    let (sent, _) = sent_with(R, &req, openai(&req));
    assert_eq!(sent["tools"][1], body["tools"][1]);
}

#[test]
fn web_search_fields_xai_has_no_equivalent_for_are_refused() {
    for (field, value, param) in [
        (
            "search_context_size",
            json!("high"),
            "tools[1].search_context_size",
        ),
        (
            "user_location",
            json!({"type": "approximate", "city": "Berlin"}),
            "tools[1].user_location",
        ),
        (
            "filters",
            json!({"allowed_domains": ["a.example", "b.example", "c.example", "d.example", "e.example", "f.example"]}),
            "tools[1].filters.allowed_domains",
        ),
        (
            "filters",
            json!({"allowed_domains": ["a.example"], "excluded_domains": ["b.example"]}),
            "tools[1].filters",
        ),
    ] {
        let mut body = codex("codex-developer-resume");
        body["tools"][1][field] = value;
        let req = decode(&body);
        let e = send(R, &req, &mut xai(&req)).unwrap_err();
        assert_eq!(e.param.as_deref(), Some(param), "{field}: {e}");
        assert!(e.message.contains("SpaceX"), "{}", e.message);
    }
}

#[test]
fn xai_gets_replayed_reasoning_without_null_content() {
    let body = codex("codex-developer-resume");
    let req = decode(&body);
    let (sent, _) = sent_with(R, &req, xai(&req));
    let mut want = body["input"][4].clone();
    want.as_object_mut().unwrap().remove("content");
    assert_eq!(sent["input"][4], want);
    assert_eq!(sent["tools"][1], json!({"type": "web_search"}));
}

#[test]
fn custom_responses_providers_get_requests_as_openai_does_but_without_namespaces() {
    let body = codex("codex-namespace-tools");
    let req = decode(&body);
    let ctx = EgressCtx::new(&req).with_responses_profile(ResponsesProfile::compatible("OpenAI"));
    let (sent, _) = sent_with(R, &req, ctx);
    let search = sent["tools"].as_array().unwrap().last().unwrap();
    assert_eq!(search, &body["tools"][3]);
    assert!(!sent.to_string().contains("\"namespace\""), "{sent}");

    let ctx = EgressCtx::new(&req)
        .with_responses_profile(ResponsesProfile::compatible("OpenAI"))
        .with_tool_namespaces(true);
    let (sent, _) = sent_with(R, &req, ctx);
    assert_eq!(
        sent["tools"][1], body["tools"][1],
        "the request's own setting wins"
    );
}

#[test]
fn every_tool_left_out_has_a_row_on_the_cross_format_page() {
    for language in ["en", "de", "zh-CN"] {
        let page = std::fs::read_to_string(format!(
            "{}/../../apps/docs/content/{language}/development/kpp/cross-format.md",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        let rows: Vec<&str> = page.lines().filter(|l| l.starts_with("| `")).collect();
        for (origin, tool, target, _) in kaeso_codecs::cross::TOOLS_LEFT_OUT {
            let cells = [
                format!("`{tool}`"),
                format!("`{}`", origin.as_str()),
                format!("`{}`", target.as_str()),
            ];
            assert!(
                rows.iter()
                    .any(|r| cells.iter().all(|c| r.contains(c.as_str()))),
                "{language}: no row for {tool} from {origin} to {target}"
            );
        }
    }
}

#[test]
fn xai_gets_codexs_second_turn_as_the_request_it_answered() {
    let xai_fixture = |path: &str| -> Value {
        serde_json::from_slice(&fixture(R, &format!("providers/xai/{path}"))).unwrap()
    };
    let app = Arc::new(decode_as(R, &xai_fixture("requests/codex-turn-one.json")));
    let reply = reply_items(
        R,
        &fixture(R, "providers/xai/responses/recorded-codex-turn-one.json"),
        &app,
        &xai(&app),
    );
    let codex = xai_fixture("requests/codex-turn-two-null-content.json");
    assert_eq!(
        reply[0]["encrypted_content"], codex["input"][4]["encrypted_content"],
        "Codex sends back the reasoning item it received"
    );
    let req = decode_as(R, &codex);
    let (sent, left_out) = sent_with(R, &req, xai(&req));
    assert_eq!(sent, xai_fixture("requests/codex-turn-two.json"));
    assert!(left_out.is_empty());
}
