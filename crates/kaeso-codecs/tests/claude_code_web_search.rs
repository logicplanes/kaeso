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

const SPACEX: ResponsesProfile = ResponsesProfile::compatible("SpaceX").xai_web_search();

fn fixture(dialect: DialectId, path: &str) -> Vec<u8> {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(dialect.as_str())
        .join(path);
    std::fs::read(file).unwrap()
}

fn web_search() -> Value {
    serde_json::from_slice(&fixture(A, "requests/claude-code-web-search.json")).unwrap()
}

fn decode(kind: RequestKind, body: &Value) -> Request {
    dispatch::decode_request(
        A,
        kind,
        &serde_json::to_vec(body).unwrap(),
        &HeaderList::new(),
    )
    .unwrap()
}

fn send_with(req: &Request, profile: ResponsesProfile) -> Result<Value, KppError> {
    let mut ctx = EgressCtx::new(req).with_responses_profile(profile);
    dispatch::check(R, req, &ctx)?;
    let enc = dispatch::encode_request(R, req, &mut ctx)?;
    Ok(serde_json::from_slice(&enc.body).unwrap())
}

fn send(body: &Value) -> Result<Value, KppError> {
    send_with(
        &decode(RequestKind::Generate, body),
        ResponsesProfile::OPENAI,
    )
}

fn with_tool(change: impl FnOnce(&mut Value)) -> Value {
    let mut body = web_search();
    change(&mut body["tools"][0]);
    body
}

#[test]
fn claude_codes_web_search_becomes_the_hosted_search_of_openai() {
    let sent = send(&web_search()).unwrap();
    assert_eq!(
        sent["tools"],
        json!([{"type": "web_search", "filters": {"allowed_domains": ["blog.rust-lang.org"]}}])
    );
    assert_eq!(sent["max_tool_calls"], 8);
    assert_eq!(
        sent["include"],
        json!([
            "reasoning.encrypted_content",
            "web_search_call.action.sources"
        ])
    );
    assert_eq!(sent["tool_choice"], "auto");
}

#[test]
fn spacex_gets_its_own_names_for_blocked_domains_and_the_limit() {
    let body = with_tool(|t| {
        t.as_object_mut().unwrap().remove("allowed_domains");
        t["blocked_domains"] = json!(["example.com"]);
    });
    let req = decode(RequestKind::Generate, &body);
    let openai = send_with(&req, ResponsesProfile::OPENAI).unwrap();
    assert_eq!(
        openai["tools"][0]["filters"],
        json!({"blocked_domains": ["example.com"]})
    );
    let spacex = send_with(&req, SPACEX).unwrap();
    assert_eq!(
        spacex["tools"][0]["filters"],
        json!({"excluded_domains": ["example.com"]})
    );
    assert_eq!(spacex["max_turns"], 8);
    assert!(spacex.get("max_tool_calls").is_none());
}

#[test]
fn spacex_refuses_what_its_web_search_does_not_take() {
    let body = with_tool(|t| {
        t["allowed_domains"] = json!([
            "a.example",
            "b.example",
            "c.example",
            "d.example",
            "e.example",
            "f.example"
        ]);
    });
    let e = send_with(&decode(RequestKind::Generate, &body), SPACEX).unwrap_err();
    assert_eq!(e.unsupported_feature(), Some("web_search.allowed_domains"));
    assert_eq!(e.param.as_deref(), Some("tools[0].allowed_domains"));
    let body = with_tool(|t| {
        t["user_location"] = json!({"type": "approximate", "country": "PT"});
    });
    let e = send_with(&decode(RequestKind::Generate, &body), SPACEX).unwrap_err();
    assert_eq!(e.unsupported_feature(), Some("web_search.user_location"));
    assert_eq!(e.param.as_deref(), Some("tools[0].user_location"));
}

fn send_to_chat(body: &Value) -> Result<(Value, Vec<String>), KppError> {
    let req = decode(RequestKind::Generate, body);
    let mut ctx = EgressCtx::new(&req).with_chat_profile(ChatProfile::compatible("Mistral"));
    dispatch::check(C, &req, &ctx)?;
    let enc = dispatch::encode_request(C, &req, &mut ctx)?;
    Ok((serde_json::from_slice(&enc.body).unwrap(), ctx.left_out))
}

#[test]
fn chat_models_get_claude_codes_web_search_without_the_tool() {
    let (sent, left_out) = send_to_chat(&web_search()).unwrap();
    assert!(
        sent.get("tools").is_none() && sent.get("tool_choice").is_none(),
        "{sent}"
    );
    assert_eq!(left_out, ["tools[0]"]);
    let mut body = web_search();
    body["tool_choice"] = json!({"type": "any"});
    let e = send_to_chat(&body).unwrap_err();
    assert_eq!(e.unsupported_feature(), Some("tool_choice `required`"));
    body["tool_choice"] = json!({"type": "tool", "name": "web_search"});
    let e = send_to_chat(&body).unwrap_err();
    assert_eq!(e.unsupported_feature(), Some("tool_choice `web_search`"));
}

#[test]
fn the_users_location_is_kept_and_other_settings_are_refused() {
    let sent = send(&with_tool(|t| {
        t["user_location"] =
            json!({"type": "approximate", "city": "Lisbon", "country": "PT", "region": null});
    }))
    .unwrap();
    assert_eq!(
        sent["tools"][0]["user_location"],
        json!({"type": "approximate", "city": "Lisbon", "country": "PT"})
    );
    let e = send(&with_tool(|t| {
        t["user_location"] = json!({"type": "exact", "city": "Lisbon"});
    }))
    .unwrap_err();
    assert_eq!(
        e.unsupported_feature(),
        Some("web_search.user_location.type")
    );
    assert_eq!(e.param.as_deref(), Some("tools[0].user_location.type"));
    let e = send(&with_tool(|t| {
        t["allowed_callers"] = json!(["code_execution_20260120"]);
    }))
    .unwrap_err();
    assert_eq!(e.unsupported_feature(), Some("web_search.allowed_callers"));
    assert_eq!(e.param.as_deref(), Some("tools[0].allowed_callers"));
    let e = send(&with_tool(|t| {
        t["max_uses"] = json!(0);
    }))
    .unwrap_err();
    assert_eq!(e.unsupported_feature(), Some("web_search.max_uses"));
}

#[test]
fn every_version_of_anthropics_web_search_becomes_the_hosted_search() {
    let expected =
        json!([{"type": "web_search", "filters": {"allowed_domains": ["blog.rust-lang.org"]}}]);
    for (version, extra) in [
        (
            "web_search_20250305",
            json!({"allowed_callers": ["direct"]}),
        ),
        ("web_search_20260209", json!({})),
        (
            "web_search_20260209",
            json!({"allowed_callers": ["direct"]}),
        ),
        (
            "web_search_20260209",
            json!({"allowed_callers": ["code_execution_20260120"]}),
        ),
        (
            "web_search_20260318",
            json!({"response_inclusion": "excluded"}),
        ),
        (
            "web_search_20260318",
            json!({"response_inclusion": "full", "allowed_callers": ["direct"]}),
        ),
    ] {
        let body = with_tool(|t| {
            t["type"] = json!(version);
            for (k, v) in extra.as_object().unwrap() {
                t[k] = v.clone();
            }
        });
        let sent = send(&body).unwrap_or_else(|e| panic!("{version} {extra}: {e}"));
        assert_eq!(sent["tools"], expected, "{version} {extra}");
        assert_eq!(sent["max_tool_calls"], 8);
        let (sent, left_out) = send_to_chat(&body).unwrap();
        assert!(sent.get("tools").is_none(), "{version}");
        assert_eq!(left_out, ["tools[0]"]);
    }
    for (version, field, value) in [
        (
            "web_search_20260209",
            "allowed_callers",
            json!(["direct", "code_execution_20260120"]),
        ),
        (
            "web_search_20250305",
            "response_inclusion",
            json!("excluded"),
        ),
        (
            "web_search_20260318",
            "response_inclusion",
            json!("partial"),
        ),
    ] {
        let body = with_tool(|t| {
            t["type"] = json!(version);
            t[field] = value.clone();
        });
        let e = send(&body).unwrap_err();
        assert_eq!(
            e.unsupported_feature(),
            Some(format!("web_search.{field}").as_str()),
            "{version}"
        );
    }
}

#[test]
fn both_domain_lists_are_refused_as_claude_refuses_them() {
    let recorded: Value =
        serde_json::from_slice(&fixture(A, "errors/web-search-both-domain-lists.json")).unwrap();
    let body = with_tool(|t| {
        t["blocked_domains"] = json!(["example.org"]);
    });
    for profile in [ResponsesProfile::OPENAI, SPACEX] {
        let e = send_with(&decode(RequestKind::Generate, &body), profile).unwrap_err();
        assert_eq!(e.kind.as_str(), "invalid_request");
        assert_eq!(e.message, recorded["error"]["message"]);
        assert_eq!(e.param.as_deref(), Some("tools[0]"));
    }
}

#[test]
fn the_chatgpt_plan_route_gets_no_search_limit() {
    let mut req = decode(RequestKind::Generate, &web_search());
    let Request::Generate(g) = &mut req else {
        unreachable!()
    };
    kaeso_codecs::openai_responses::for_chatgpt_plan(g);
    let sent = send_with(&req, ResponsesProfile::OPENAI).unwrap();
    assert!(sent.get("max_tool_calls").is_none(), "{sent}");
    assert_eq!(sent["tools"][0]["type"], "web_search");
    assert!(
        sent["include"]
            .to_string()
            .contains("web_search_call.action.sources")
    );
}

#[test]
fn a_cache_hint_on_the_tool_is_left_out_like_on_other_tools() {
    let sent = send(&with_tool(|t| {
        t["cache_control"] = json!({"type": "ephemeral"});
    }))
    .unwrap();
    assert!(sent["tools"][0].get("cache_control").is_none());
}

#[test]
fn forcing_the_search_is_refused() {
    let mut body = web_search();
    body["tool_choice"] = json!({"type": "tool", "name": "web_search"});
    let e = send(&body).unwrap_err();
    assert_eq!(e.unsupported_feature(), Some("tool_choice `web_search`"));
    assert_eq!(e.param.as_deref(), Some("tool_choice"));
    body["tool_choice"] = json!({"type": "any"});
    assert_eq!(send(&body).unwrap()["tool_choice"], "required");
}

#[test]
fn token_counting_sends_the_tool_without_the_reply_settings() {
    let mut body = web_search();
    body.as_object_mut().unwrap().remove("stream");
    body.as_object_mut().unwrap().remove("max_tokens");
    let req = decode(RequestKind::CountTokens, &body);
    let sent = send_with(&req, ResponsesProfile::OPENAI).unwrap();
    assert_eq!(sent["tools"][0]["type"], "web_search");
    assert!(sent.get("include").is_none() && sent.get("max_tool_calls").is_none());
}

fn anthropic_stream(provider_stream: &[u8]) -> Vec<Value> {
    let app = Arc::new(decode(RequestKind::Generate, &web_search()));
    let ctx = EgressCtx::new(&app);
    let mut dec = dispatch::stream_decoder(R, &ctx).unwrap();
    let mut events = dec.feed(provider_stream);
    events.extend(dec.finish());
    let mut enc = dispatch::stream_encoder(A, &EncodeCtx::for_request(Arc::clone(&app))).unwrap();
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

fn assembled(events: &[Value]) -> Vec<Value> {
    let mut blocks: Vec<Value> = Vec::new();
    let mut json = String::new();
    for e in events {
        match e["type"].as_str().unwrap() {
            "content_block_start" => {
                assert_eq!(e["index"], blocks.len(), "{e}");
                blocks.push(e["content_block"].clone());
                json.clear();
            }
            "content_block_delta" => {
                let block = blocks.last_mut().unwrap();
                match e["delta"]["type"].as_str().unwrap() {
                    "input_json_delta" => {
                        json.push_str(e["delta"]["partial_json"].as_str().unwrap())
                    }
                    "text_delta" => {
                        let text = block["text"].as_str().unwrap().to_owned();
                        block["text"] = json!(text + e["delta"]["text"].as_str().unwrap());
                    }
                    _ => {}
                }
            }
            "content_block_stop" if !json.is_empty() => {
                blocks.last_mut().unwrap()["input"] = serde_json::from_str(&json).unwrap();
            }
            _ => {}
        }
    }
    blocks
}

fn searches(blocks: &[Value]) -> Vec<Value> {
    blocks
        .iter()
        .filter(|b| b["type"] == "server_tool_use" || b["type"] == "web_search_tool_result")
        .cloned()
        .collect()
}

#[test]
fn openais_searches_reach_claude_code_as_web_search_results() {
    let events = anthropic_stream(&fixture(R, "streams/plan-web-search-sources-stream.sse"));
    let blocks = assembled(&events);
    let found = searches(&blocks);
    assert_eq!(found.len(), 2, "{blocks:#?}");
    let id = found[0]["id"].as_str().unwrap();
    assert!(id.starts_with("srvtoolu_kaeso_ws_"), "{id}");
    assert_eq!(found[0]["name"], "web_search");
    assert_eq!(
        found[0]["input"]["query"],
        "Announcing Rust release stable latest version site:blog.rust-lang.org"
    );
    assert_eq!(found[1]["tool_use_id"], id);
    let results = found[1]["content"].as_array().unwrap();
    assert_eq!(results.len(), 8);
    for r in results {
        assert_eq!(r["type"], "web_search_result");
        assert!(
            r["url"]
                .as_str()
                .unwrap()
                .starts_with("https://blog.rust-lang.org/")
        );
        assert_eq!(r["title"], r["url"]);
        assert_eq!(r["encrypted_content"], "");
        assert_eq!(r["page_age"], Value::Null);
    }
    assert!(
        blocks.last().unwrap()["text"]
            .as_str()
            .unwrap()
            .contains("1.99")
    );
}

#[test]
fn spacex_searches_reach_claude_code_as_web_search_results() {
    let events = anthropic_stream(&fixture(
        R,
        "providers/xai/streams/recorded-claude-code-web-search-stream.sse",
    ));
    let found = searches(&assembled(&events));
    let uses: Vec<&Value> = found
        .iter()
        .filter(|b| b["type"] == "server_tool_use")
        .collect();
    let results: Vec<&Value> = found
        .iter()
        .filter(|b| b["type"] == "web_search_tool_result")
        .collect();
    assert_eq!(uses.len(), results.len());
    assert!(!uses.is_empty());
    for (u, r) in uses.iter().zip(&results) {
        assert_eq!(r["tool_use_id"], u["id"]);
        assert!(!u["input"]["query"].as_str().unwrap().is_empty());
        assert!(!r["content"].as_array().unwrap().is_empty());
    }
}

#[test]
fn a_reply_carries_the_searches_in_order_and_a_failed_search_as_an_error() {
    let app = Arc::new(decode(RequestKind::Generate, &web_search()));
    let body = json!({
        "id": "resp_1", "object": "response", "status": "completed", "model": "gpt-6-luna",
        "output": [
            {"id": "ws_1", "type": "web_search_call", "status": "completed",
             "action": {"type": "search", "query": "rust", "sources": [{"type": "url", "url": "https://example.com/a"}]}},
            {"id": "ws_2", "type": "web_search_call", "status": "failed", "action": {"type": "search", "query": "go"}},
            {"id": "ws_3", "type": "web_search_call", "status": "completed", "action": {"type": "open_page", "url": "https://example.com/a"}},
            {"id": "msg_1", "type": "message", "role": "assistant", "status": "completed",
             "content": [{"type": "output_text", "text": "Done.", "annotations": []}]}
        ],
        "usage": {"input_tokens": 1, "output_tokens": 1, "total_tokens": 2}
    });
    let ctx = EgressCtx::new(&app);
    let reply = dispatch::decode_response(
        R,
        200,
        &HeaderList::new(),
        &serde_json::to_vec(&body).unwrap(),
        &ctx,
    )
    .unwrap();
    let out = dispatch::encode_response(A, &reply, &EncodeCtx::for_request(app)).unwrap();
    let out = serde_json::from_slice::<Value>(&out).unwrap();
    assert_eq!(
        out["usage"]["server_tool_use"],
        json!({"web_search_requests": 1})
    );
    let content = out["content"].clone();
    assert_eq!(
        content,
        json!([
            {"type": "server_tool_use", "id": "srvtoolu_kaeso_ws_1", "name": "web_search", "input": {"query": "rust"}},
            {"type": "web_search_tool_result", "tool_use_id": "srvtoolu_kaeso_ws_1", "content": [
                {"type": "web_search_result", "url": "https://example.com/a", "title": "https://example.com/a", "encrypted_content": "", "page_age": null}
            ]},
            {"type": "server_tool_use", "id": "srvtoolu_kaeso_ws_2", "name": "web_search", "input": {"query": "go"}},
            {"type": "web_search_tool_result", "tool_use_id": "srvtoolu_kaeso_ws_2", "content": {"type": "web_search_tool_result_error", "error_code": "unavailable"}},
            {"type": "text", "text": "Done."}
        ])
    );
}

fn replayed() -> Value {
    let mut body = web_search();
    body["messages"] = json!([
        {"role": "user", "content": "Latest Rust?"},
        {"role": "assistant", "content": [
            {"type": "server_tool_use", "id": "srvtoolu_kaeso_ws_1", "name": "web_search", "input": {"query": "rust"}},
            {"type": "web_search_tool_result", "tool_use_id": "srvtoolu_kaeso_ws_1", "content": [
                {"type": "web_search_result", "url": "https://example.com/a", "title": "", "encrypted_content": "", "page_age": null}
            ]},
            {"type": "text", "text": "Rust 1.99."}
        ]},
        {"role": "user", "content": "And Go?"}
    ]);
    body
}

#[test]
fn searches_kaeso_reported_are_left_out_when_the_app_sends_them_back() {
    let (sent, _) = send_to_chat(&replayed()).unwrap();
    assert_eq!(
        sent["messages"][2],
        json!({"role": "assistant", "content": "Rust 1.99."})
    );
    let sent = send(&replayed()).unwrap();
    assert_eq!(
        sent["input"],
        json!([
            sent["input"][0].clone(),
            {"role": "user", "content": "Latest Rust?"},
            {"role": "assistant", "content": "Rust 1.99."},
            {"role": "user", "content": "And Go?"}
        ])
    );
    let req = decode(RequestKind::Generate, &replayed());
    let mut ctx = EgressCtx::new(&req);
    dispatch::check(A, &req, &ctx).unwrap();
    let enc = dispatch::encode_request(A, &req, &mut ctx).unwrap();
    let sent: Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(
        sent["messages"][1]["content"],
        json!([{"type": "text", "text": "Rust 1.99."}])
    );
    assert_eq!(
        ctx.left_out,
        [
            "server_tool_use srvtoolu_kaeso_ws_1",
            "web_search_tool_result srvtoolu_kaeso_ws_1"
        ]
    );
}

#[test]
fn claudes_own_searches_are_left_out_for_other_formats_and_kept_for_claude() {
    let mut body = replayed();
    body["messages"][1]["content"][0]["id"] = json!("srvtoolu_01ABC");
    body["messages"][1]["content"][1]["tool_use_id"] = json!("srvtoolu_01ABC");
    body["messages"][1]["content"][2]["citations"] = json!([{
        "type": "web_search_result_location", "url": "https://example.com/a", "title": "A",
        "encrypted_index": "Eo8B", "cited_text": "Rust 1.99"
    }]);
    let sent = send(&body).unwrap();
    assert_eq!(
        sent["input"][2],
        json!({"role": "assistant", "content": "Rust 1.99."})
    );
    let (sent, left_out) = send_to_chat(&body).unwrap();
    assert_eq!(
        sent["messages"][2],
        json!({"role": "assistant", "content": "Rust 1.99."})
    );
    assert_eq!(
        left_out,
        [
            "tools[0]",
            "server_tool_use srvtoolu_01ABC",
            "web_search_tool_result srvtoolu_01ABC",
            "messages[1].blocks[2].citations"
        ]
    );
    let req = decode(RequestKind::Generate, &body);
    let mut ctx = EgressCtx::new(&req);
    let enc = dispatch::encode_request(A, &req, &mut ctx).unwrap();
    let sent: Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(sent["messages"][1], body["messages"][1]);
    assert!(ctx.left_out.is_empty());
}

fn kpp_stream(items: &[(Value, Value)]) -> Vec<Value> {
    use kaeso_kpp::{
        BlockDelta, BlockHeader, BlockStart, BlockStop, Delta, Origin, RawItem, RawJson,
        ResponseDelta, ResponseStart, ResponseStop, StopReason, StreamEvent, Usage,
    };
    let raw = |v: &Value| RawItem::new(Origin::new(R), RawJson::from_serialize(v).unwrap());
    let app = Arc::new(decode(RequestKind::Generate, &web_search()));
    let mut enc = dispatch::stream_encoder(A, &EncodeCtx::for_request(Arc::clone(&app))).unwrap();
    let mut events = vec![StreamEvent::ResponseStart(ResponseStart::new(
        "resp_1",
        "gpt-6-luna",
    ))];
    for (i, (added, done)) in (0u32..).zip(items) {
        events.push(StreamEvent::BlockStart(BlockStart::new(
            i,
            BlockHeader::Unknown(raw(added)),
        )));
        events.push(StreamEvent::BlockDelta(BlockDelta::new(
            i,
            Delta::Unknown(raw(done)),
        )));
        events.push(StreamEvent::BlockStop(BlockStop::new(i)));
    }
    events.push(StreamEvent::ResponseDelta(ResponseDelta::new(
        Some(StopReason::EndTurn),
        Some(Usage::new(1, 1)),
    )));
    events.push(StreamEvent::ResponseStop(ResponseStop::new()));
    let mut out = Vec::new();
    for e in &events {
        out.extend(enc.encode(e).unwrap());
    }
    parse_all(&out)
        .unwrap()
        .into_iter()
        .filter_map(|i| match i {
            SseItem::Event(e) => serde_json::from_str(&e.data).ok(),
            _ => None,
        })
        .collect()
}

#[test]
fn streams_report_only_searches_and_count_the_completed_ones() {
    let added = |id: &str| json!({"id": id, "type": "web_search_call", "status": "in_progress"});
    let events = kpp_stream(&[
        (
            added("ws_1"),
            json!({"id": "ws_1", "type": "web_search_call", "status": "completed",
                   "action": {"type": "open_page", "url": "https://example.com/a"}}),
        ),
        (
            added("ws_2"),
            json!({"id": "ws_2", "type": "web_search_call", "status": "completed",
                   "action": {"type": "find_in_page", "url": "https://example.com/a", "pattern": "Rust"}}),
        ),
        (
            added("ws_3"),
            json!({"id": "ws_3", "type": "web_search_call", "status": "completed",
                   "action": {"type": "search", "queries": ["rust"], "sources": [{"type": "url", "url": "https://example.com/a"}]}}),
        ),
        (
            added("ws_4"),
            json!({"id": "ws_4", "type": "web_search_call", "status": "failed",
                   "action": {"type": "search", "query": "go"}}),
        ),
    ]);
    let blocks = assembled(&events);
    assert_eq!(
        blocks
            .iter()
            .map(|b| b["type"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "server_tool_use",
            "web_search_tool_result",
            "server_tool_use",
            "web_search_tool_result"
        ]
    );
    assert_eq!(blocks[0]["id"], "srvtoolu_kaeso_ws_3");
    assert_eq!(blocks[0]["input"], json!({"query": "rust"}));
    assert_eq!(blocks[2]["id"], "srvtoolu_kaeso_ws_4");
    let delta = events
        .iter()
        .find(|e| e["type"] == "message_delta")
        .unwrap();
    assert_eq!(
        delta["usage"]["server_tool_use"],
        json!({"web_search_requests": 1})
    );
}
