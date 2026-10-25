#![cfg(test)]

use std::path::Path;

use kaeso_codecs::{EgressCtx, HeaderList, dispatch};
use kaeso_kpp::{DialectId, KppError, Request, RequestKind};
use serde_json::{Value, json};

const A: DialectId = DialectId::AnthropicMessages;
const C: DialectId = DialectId::OpenAiChat;
const R: DialectId = DialectId::OpenAiResponses;

fn fixture(path: &str) -> Vec<u8> {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(A.as_str())
        .join(path);
    std::fs::read(file).unwrap()
}

fn claude_code(name: &str) -> (Value, HeaderList) {
    let body = serde_json::from_slice(&fixture(&format!("requests/{name}.json"))).unwrap();
    let headers: Value =
        serde_json::from_slice(&fixture(&format!("requests/{name}.headers.json"))).unwrap();
    let mut list = HeaderList::new();
    for (k, v) in headers.as_object().unwrap() {
        list = list.with(k, v.as_str().unwrap());
    }
    (body, list)
}

fn decode(body: &Value, headers: &HeaderList) -> Request {
    dispatch::decode_request(
        A,
        RequestKind::Generate,
        &serde_json::to_vec(body).unwrap(),
        headers,
    )
    .unwrap()
}

fn send(target: DialectId, req: &Request) -> Result<Value, KppError> {
    let mut ctx = EgressCtx::new(req);
    dispatch::check(target, req, &ctx)?;
    let enc = dispatch::encode_request(target, req, &mut ctx)?;
    Ok(serde_json::from_slice(&enc.body).unwrap())
}

fn tool_names(target: DialectId, body: &Value) -> Vec<String> {
    body["tools"]
        .as_array()
        .map(|tools| {
            tools
                .iter()
                .map(|t| match target {
                    C => t["function"]["name"].as_str().unwrap().to_owned(),
                    _ => t["name"].as_str().unwrap().to_owned(),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn tool_output(target: DialectId, body: &Value, call_id: &str) -> Value {
    match target {
        C => body["messages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["tool_call_id"] == call_id)
            .unwrap()["content"]
            .clone(),
        _ => body["input"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["type"] == "function_call_output" && i["call_id"] == call_id)
            .unwrap()["output"]
            .clone(),
    }
}

const SEARCH_CALL: &str = "toolu_0199pumA6DiQ6ow67Ujnv8bf";

#[test]
fn deferred_tools_stay_out_until_tool_search_loads_them() {
    let (body, headers) = claude_code("claude-code-tool-search");
    let first = decode(&body, &headers);
    let (body, headers) = claude_code("claude-code-tool-search-loaded");
    let loaded = decode(&body, &headers);
    for target in [C, R] {
        let sent = send(target, &first).unwrap_or_else(|e| panic!("{target}: {e}"));
        assert_eq!(
            tool_names(target, &sent),
            ["Read", "ToolSearch"],
            "{target}"
        );
        let sent = send(target, &loaded).unwrap_or_else(|e| panic!("{target}: {e}"));
        assert_eq!(
            tool_names(target, &sent),
            ["Read", "ToolSearch", "WebFetch"],
            "{target}"
        );
        assert_eq!(
            tool_output(target, &sent, SEARCH_CALL),
            "WebFetch",
            "{target}"
        );
        assert!(!sent.to_string().contains("defer_loading"), "{target}");
    }
}

#[test]
fn claude_keeps_deferred_tools_and_references_as_sent() {
    for name in ["claude-code-tool-search", "claude-code-tool-search-loaded"] {
        let (body, headers) = claude_code(name);
        let sent = send(A, &decode(&body, &headers)).unwrap();
        assert_eq!(sent, body, "{name}");
    }
}

fn loaded_with(change: impl FnOnce(&mut Value)) -> Request {
    let (mut body, headers) = claude_code("claude-code-tool-search-loaded");
    change(&mut body);
    decode(&body, &headers)
}

fn reference(body: &mut Value) -> &mut Value {
    &mut body["messages"][3]["content"][0]["content"][0]
}

#[test]
fn every_referenced_tool_loads_and_reads_as_its_name() {
    let req = loaded_with(|b| {
        b["messages"][3]["content"][0]["content"] = json!([
            {"type": "tool_reference", "tool_name": "WebFetch"},
            {"type": "tool_reference", "tool_name": "Read"},
        ]);
    });
    for target in [C, R] {
        let sent = send(target, &req).unwrap();
        assert_eq!(
            tool_names(target, &sent),
            ["Read", "ToolSearch", "WebFetch"],
            "{target}"
        );
        let output = tool_output(target, &sent, SEARCH_CALL);
        let text: Vec<&str> = match &output {
            Value::Array(parts) => parts.iter().map(|p| p["text"].as_str().unwrap()).collect(),
            other => vec![other.as_str().unwrap()],
        };
        assert_eq!(text.concat().replace('\n', ""), "WebFetchRead", "{target}");
    }
}

#[test]
fn a_reference_with_a_cache_hint_still_loads_the_tool() {
    let req = loaded_with(|b| {
        reference(b)["cache_control"] = json!({"type": "ephemeral"});
    });
    for target in [C, R] {
        let sent = send(target, &req).unwrap_or_else(|e| panic!("{target}: {e}"));
        assert!(tool_names(target, &sent).contains(&"WebFetch".to_owned()));
    }
}

#[test]
fn a_reference_with_a_field_kaeso_does_not_know_is_refused() {
    let req = loaded_with(|b| {
        reference(b)["x_future"] = json!(1);
    });
    for target in [C, R] {
        let e = send(target, &req).unwrap_err();
        assert_eq!(
            e.unsupported_feature(),
            Some("messages[3].blocks[0].content[0].x_future"),
            "{target}"
        );
    }
}

fn recorded_message(name: &str) -> String {
    let body: Value = serde_json::from_slice(&fixture(&format!("errors/{name}.json"))).unwrap();
    body["error"]["message"].as_str().unwrap().to_owned()
}

#[test]
fn a_reference_to_a_tool_that_is_not_sent_is_refused_like_claude_does() {
    let req = loaded_with(|b| {
        reference(b)["tool_name"] = json!("get_forecast");
    });
    for target in [C, R] {
        let e = send(target, &req).unwrap_err();
        assert_eq!(e.kind.as_str(), "invalid_request", "{target}");
        assert_eq!(e.message, recorded_message("tool-search-unknown-reference"));
        assert_eq!(e.param.as_deref(), Some("messages[3].blocks[0].content[0]"));
    }
}

#[test]
fn deferring_every_tool_is_refused_like_claude_does() {
    let req = loaded_with(|b| {
        for t in b["tools"].as_array_mut().unwrap() {
            t["defer_loading"] = json!(true);
        }
    });
    for target in [C, R] {
        let e = send(target, &req).unwrap_err();
        assert_eq!(e.kind.as_str(), "invalid_request", "{target}");
        assert_eq!(e.message, recorded_message("tool-search-all-deferred"));
        assert_eq!(e.param.as_deref(), Some("tools"));
    }
}

#[test]
fn a_forced_deferred_tool_is_sent_although_no_search_loaded_it() {
    let req = loaded_with(|b| {
        b["tool_choice"] = json!({"type": "tool", "name": "DeferredToolPlaceholder"});
    });
    for target in [C, R] {
        let sent = send(target, &req).unwrap_or_else(|e| panic!("{target}: {e}"));
        assert_eq!(
            tool_names(target, &sent),
            ["Read", "ToolSearch", "WebFetch", "DeferredToolPlaceholder"],
            "{target}"
        );
    }
}

#[test]
fn defer_loading_false_is_left_out_and_other_values_are_refused() {
    let req = loaded_with(|b| {
        b["tools"][0]["defer_loading"] = json!(false);
    });
    for target in [C, R] {
        let sent = send(target, &req).unwrap();
        assert_eq!(tool_names(target, &sent)[0], "Read", "{target}");
        assert!(!sent.to_string().contains("defer_loading"), "{target}");
    }
    let req = loaded_with(|b| {
        b["tools"][0]["defer_loading"] = json!("yes");
    });
    for target in [C, R] {
        let e = send(target, &req).unwrap_err();
        assert_eq!(
            e.unsupported_feature(),
            Some("tools[0].defer_loading"),
            "{target}"
        );
    }
}

#[test]
fn token_counting_leaves_out_tools_that_are_not_loaded() {
    let (body, headers) = claude_code("claude-code-tool-search");
    let req = dispatch::decode_request(
        A,
        RequestKind::CountTokens,
        &serde_json::to_vec(&body).unwrap(),
        &headers,
    )
    .unwrap();
    let mut ctx = EgressCtx::new(&req);
    dispatch::check(R, &req, &ctx).unwrap();
    let enc = dispatch::encode_request(R, &req, &mut ctx).unwrap();
    let sent: Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(tool_names(R, &sent), ["Read", "ToolSearch"]);
}
