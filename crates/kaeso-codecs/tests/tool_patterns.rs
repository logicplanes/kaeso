#![cfg(test)]

use std::path::Path;

use kaeso_codecs::openai_chat::ChatProfile;
use kaeso_codecs::{EgressCtx, HeaderList, ToolPatterns, dispatch};
use kaeso_kpp::{DialectId, Request, RequestKind};
use serde_json::{Value, json};

const A: DialectId = DialectId::AnthropicMessages;
const C: DialectId = DialectId::OpenAiChat;
const R: DialectId = DialectId::OpenAiResponses;

const DEEPSEEK: ChatProfile =
    ChatProfile::compatible("DeepSeek").tool_patterns(ToolPatterns::Compiled);

fn decode(dialect: DialectId, body: &Value) -> Request {
    dispatch::decode_request(
        dialect,
        RequestKind::Generate,
        &serde_json::to_vec(body).unwrap(),
        &HeaderList::new(),
    )
    .unwrap()
}

fn sent(target: DialectId, req: &Request, profile: ChatProfile) -> (Value, Vec<String>) {
    let mut ctx = EgressCtx::new(req).with_chat_profile(profile);
    dispatch::check(target, req, &ctx).unwrap();
    let enc = dispatch::encode_request(target, req, &mut ctx).unwrap();
    (serde_json::from_slice(&enc.body).unwrap(), ctx.adapted)
}

fn chat_request(parameters: Value) -> Value {
    json!({
        "model": "deepseek-flash",
        "messages": [{"role": "user", "content": "Publish the page."}],
        "tools": [{"type": "function", "function": {"name": "publish", "parameters": parameters}}]
    })
}

fn claude_code_inline_tools() -> Request {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(A.as_str())
        .join("requests/claude-code-inline-tools.json");
    let body: Value = serde_json::from_slice(&std::fs::read(file).unwrap()).unwrap();
    decode(A, &body)
}

fn artifact_pattern(body: &Value) -> &Value {
    let tools = body["tools"].as_array().unwrap();
    let artifact = tools
        .iter()
        .find(|t| t["function"]["name"] == "Artifact")
        .unwrap();
    &artifact["function"]["parameters"]["properties"]["file_paths"]["items"]["pattern"]
}

#[test]
fn claude_codes_nul_pattern_reaches_deepseek_as_a_hex_escape_and_others_as_sent() {
    let req = claude_code_inline_tools();
    let (body, adapted) = sent(C, &req, DEEPSEEK);
    assert_eq!(artifact_pattern(&body), r"^[^\x00]*$");
    assert_eq!(
        adapted,
        [
            "messages[4].blocks[1]",
            "messages[4].blocks[2]",
            "messages[7].blocks[1]",
            "messages[7].blocks[2]",
            "messages[7].blocks[3]",
            "tools[2]",
        ]
    );
    let (body, adapted) = sent(C, &req, ChatProfile::OPENAI);
    assert_eq!(artifact_pattern(&body), r"^[^\0]*$");
    assert!(!adapted.contains(&"tools[2]".to_owned()));
    let (body, _) = sent(R, &req, DEEPSEEK);
    assert_eq!(
        body["tools"][2]["parameters"]["properties"]["file_paths"]["items"]["pattern"],
        r"^[^\0]*$"
    );
}

#[test]
fn a_chat_apps_patterns_are_compiled_the_same_way() {
    let req = decode(
        C,
        &chat_request(json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "pattern": r"^[^\0]*$"},
                "key": {"type": "string", "pattern": r"^[\b]$"},
                "id": {"type": "string", "pattern": "^[a-z]+$"}
            }
        })),
    );
    let (body, adapted) = sent(C, &req, DEEPSEEK);
    let properties = &body["tools"][0]["function"]["parameters"]["properties"];
    assert_eq!(properties["path"]["pattern"], r"^[^\x00]*$");
    assert_eq!(properties["key"]["pattern"], r"^[\x08]$");
    assert_eq!(properties["id"]["pattern"], "^[a-z]+$");
    assert_eq!(adapted, ["tools[0]"]);
}

#[test]
fn a_pattern_deepseek_cannot_take_at_all_is_left_out_of_its_property() {
    let req = decode(
        C,
        &chat_request(json!({
            "type": "object",
            "properties": {
                "pair": {"type": "string", "pattern": r"^(?<x>a)\k<x>$", "minLength": 2},
                "none": {"type": "string", "pattern": "^[]$"}
            }
        })),
    );
    let (body, adapted) = sent(C, &req, DEEPSEEK);
    assert_eq!(
        body["tools"][0]["function"]["parameters"]["properties"],
        json!({
            "pair": {"type": "string", "minLength": 2},
            "none": {"type": "string"}
        })
    );
    assert_eq!(adapted, ["tools[0]"]);
}

#[test]
fn patterns_deepseek_takes_leave_the_request_untouched() {
    let body = chat_request(json!({
        "type": "object",
        "properties": {
            "id": {"type": "string", "pattern": "^org_[0-9a-hjkmnp-tv-z]{26}$"},
            "amount": {"oneOf": [{"type": "string", "pattern": r"^(?:[0-9]+(?:\.[0-9]+)?)$"}, {"type": "integer"}]},
            "literal": {"const": r"\0", "enum": [r"\0"]}
        }
    }));
    let req = decode(C, &body);
    let (sent_body, adapted) = sent(C, &req, DEEPSEEK);
    assert_eq!(sent_body["tools"], body["tools"]);
    assert!(adapted.is_empty());
}
