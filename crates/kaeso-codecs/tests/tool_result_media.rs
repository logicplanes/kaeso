#![cfg(test)]

use std::path::Path;

use kaeso_codecs::{EgressCtx, HeaderList, dispatch};
use kaeso_kpp::{DialectId, Request, RequestKind};
use serde_json::{Value, json};

const A: DialectId = DialectId::AnthropicMessages;
const C: DialectId = DialectId::OpenAiChat;
const R: DialectId = DialectId::OpenAiResponses;

const PIXEL: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";
const MOVED: &str = "The rest of this result is in the next message.";

fn fixture(dialect: DialectId, name: &str) -> Value {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(dialect.as_str())
        .join("requests")
        .join(format!("{name}.json"));
    serde_json::from_slice(&std::fs::read(file).unwrap()).unwrap()
}

fn decode(dialect: DialectId, body: &Value) -> Request {
    let mut req = dispatch::decode_request(
        dialect,
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

fn send(target: DialectId, req: &Request) -> (Value, Vec<String>) {
    let mut ctx = EgressCtx::new(req);
    dispatch::check(target, req, &ctx).unwrap_or_else(|e| panic!("{target}: {e}"));
    let enc =
        dispatch::encode_request(target, req, &mut ctx).unwrap_or_else(|e| panic!("{target}: {e}"));
    (serde_json::from_slice(&enc.body).unwrap(), ctx.adapted)
}

#[test]
fn an_image_a_claude_tool_returned_follows_its_results_for_chat() {
    let req = decode(A, &fixture(A, "tool-result-multi-turn"));
    let (sent, adapted) = send(C, &req);
    let messages = sent["messages"].as_array().unwrap();
    assert_eq!(
        messages[2..].to_vec(),
        vec![
            json!({"role": "tool", "tool_call_id": "toolu_01Ej1YyQ2oLMuN4cPxedXTyn", "content": "18°C, light rain"}),
            json!({"role": "tool", "tool_call_id": "toolu_01i9erRrqtgtQAjVyyvcAdnH", "content": [
                {"type": "text", "text": "24°C, clear skies. Radar image attached."},
                {"type": "text", "text": MOVED},
            ]}),
            json!({"role": "tool", "tool_call_id": "toolu_01krb5XZirSQZiYWNMB6VT9o", "content": "ConnectionError: the time service is not available (HTTP 503)"}),
            json!({"role": "tool", "tool_call_id": "toolu_01tbdagxGypaxguXw1B6gfCo", "content": "2026-10-11T22:14:05+09:00"}),
            json!({"role": "user", "content": [
                {"type": "text", "text": "The rest of the result of tool call `toolu_01i9erRrqtgtQAjVyyvcAdnH`:"},
                {"type": "image_url", "image_url": {"url": format!("data:image/png;base64,{PIXEL}")}},
                {"type": "text", "text": "Keep the answer short."},
            ]}),
        ]
    );
    assert_eq!(adapted, ["tool result toolu_01i9erRrqtgtQAjVyyvcAdnH"]);
}

#[test]
fn images_and_files_a_responses_tool_returned_follow_its_output_for_chat() {
    let req = decode(R, &fixture(R, "function-call-output-array"));
    let (sent, adapted) = send(C, &req);
    let messages = sent["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 4, "{sent}");
    assert_eq!(
        messages[2],
        json!({"role": "tool", "tool_call_id": "call_i4rean9cQaAAMNtPW0tQupGs", "content": [
            {"type": "text", "text": "Chart rendered (1 image)."},
            {"type": "text", "text": MOVED},
        ]})
    );
    let user = &messages[3];
    assert_eq!(user["role"], "user");
    let parts: Vec<&str> = user["content"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["type"].as_str().unwrap())
        .collect();
    assert_eq!(parts, ["text", "image_url", "file"]);
    assert_eq!(
        user["content"][0]["text"],
        "The rest of the result of tool call `call_i4rean9cQaAAMNtPW0tQupGs`:"
    );
    assert!(
        user["content"][2]["file"]["file_data"]
            .as_str()
            .unwrap()
            .starts_with("data:application/pdf;base64,JVBERi0xLjQK"),
        "{user}"
    );
    assert_eq!(adapted, ["tool result call_i4rean9cQaAAMNtPW0tQupGs"]);
}

#[test]
fn formats_that_take_media_in_tool_results_get_them_there() {
    let req = decode(A, &fixture(A, "tool-result-multi-turn"));
    let (sent, adapted) = send(R, &req);
    let output = sent["input"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| {
            i["type"] == "function_call_output" && i["call_id"] == "toolu_01i9erRrqtgtQAjVyyvcAdnH"
        })
        .map(|i| i["output"].clone())
        .unwrap();
    assert_eq!(output[1]["type"], "input_image", "{sent}");
    assert!(!sent.to_string().contains(MOVED));
    assert!(adapted.is_empty(), "{adapted:?}");

    let req = decode(R, &fixture(R, "function-call-output-array"));
    let (sent, adapted) = send(A, &req);
    let result = &sent["messages"][2]["content"][0];
    assert_eq!(result["type"], "tool_result", "{sent}");
    assert_eq!(result["content"][1]["type"], "image", "{sent}");
    assert!(adapted.is_empty(), "{adapted:?}");
}

#[test]
fn chat_requests_without_media_in_tool_results_are_not_adapted() {
    let req = decode(R, &fixture(R, "function-call-output-array"));
    let Request::Generate(mut g) = req else {
        unreachable!()
    };
    for m in &mut g.messages {
        for b in &mut m.blocks {
            if let kaeso_kpp::Block::ToolResult(r) = b {
                r.content.truncate(1);
            }
        }
    }
    let (sent, adapted) = send(C, &Request::Generate(g));
    assert_eq!(sent["messages"].as_array().unwrap().len(), 3, "{sent}");
    assert_eq!(sent["messages"][2]["content"], "Chart rendered (1 image).");
    assert!(adapted.is_empty(), "{adapted:?}");
}
