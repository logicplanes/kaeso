use kaeso_kpp::Request;
use serde_json::{Value, json};

use super::*;
use crate::anthropic_messages::AnthropicMessages;
use crate::bedrock_converse::BedrockConverse;
use crate::openai_chat::OpenAiChat;
use crate::openai_responses::OpenAiResponses;
use crate::test_support::fixtures;

fn decoded(name: &str) -> Request {
    let body = fixtures("gemini", "requests", "json")
        .into_iter()
        .find(|(n, _)| n == name)
        .unwrap_or_else(|| panic!("no fixture {name}"))
        .1;
    decode_body(&body)
}

fn decode_body(body: &[u8]) -> Request {
    crate::dispatch::decode_gemini_request(
        "m",
        GeminiAction::GenerateContent,
        body,
        &HeaderList::new(),
    )
    .unwrap()
}

fn sent<C: Codec>(req: &Request, ctx: EgressCtx) -> (Value, Vec<String>) {
    let mut ctx = ctx;
    C::check(req, &ctx).unwrap();
    let out = C::encode_request(req, &mut ctx).unwrap();
    (serde_json::from_slice(&out.body).unwrap(), ctx.left_out)
}

#[test]
fn a_functions_return_schema_is_left_out_for_every_other_format_and_named() {
    let req = decoded("sdk-automatic-function-calling");
    let named = ["tools[0].functionDeclarations[0].responseJsonSchema"];
    let (chat, left_out) = sent::<OpenAiChat>(&req, EgressCtx::new(&req));
    assert_eq!(left_out, named);
    assert_eq!(
        chat["tools"][0]["function"],
        json!({
            "name": "get_weather",
            "description": "Weather for a city.\n\nArgs:\n  city: The city name.\n  unit: celsius or fahrenheit.",
            "parameters": {
                "type": "object",
                "properties": {"city": {"type": "string"}, "unit": {"type": "string", "default": "celsius"}},
                "required": ["city"]
            }
        })
    );
    let (anthropic, left_out) = sent::<AnthropicMessages>(&req, EgressCtx::new(&req));
    assert_eq!(left_out, named);
    assert!(anthropic["tools"][0].get("response_json_schema").is_none());
    let (_, left_out) = sent::<OpenAiResponses>(&req, EgressCtx::new(&req));
    assert_eq!(left_out, named);
    let bedrock = EgressCtx::new(&req).with_api_root("");
    let (_, left_out) = sent::<BedrockConverse>(&req, bedrock);
    assert_eq!(left_out, named);

    let req = decoded("sdk-function-response-schema");
    let (_, left_out) = sent::<OpenAiChat>(&req, EgressCtx::new(&req));
    assert_eq!(left_out, ["tools[0].functionDeclarations[0].response"]);
}

#[test]
fn gemini_gets_a_functions_return_schema_back_as_written() {
    let req = decoded("sdk-automatic-function-calling");
    let (body, left_out) = sent::<Gemini>(&req, EgressCtx::new(&req));
    assert!(left_out.is_empty());
    assert_eq!(
        body["tools"][0]["functionDeclarations"][0]["response_json_schema"],
        json!({"type": "string"})
    );
}

#[test]
fn declarations_are_named_by_their_tool_and_place_in_it() {
    let body = json!({
        "contents": [{"role": "user", "parts": [{"text": "Hi"}]}],
        "tools": [
            {"functionDeclarations": [
                {"name": "a", "parametersJsonSchema": {"type": "object"}},
                {"name": "b", "parametersJsonSchema": {"type": "object"}, "responseJsonSchema": {"type": "string"}}
            ]},
            {"googleSearch": {}},
            {"functionDeclarations": [
                {"name": "c", "response": {"type": "STRING"}, "behavior": "NON_BLOCKING"}
            ]}
        ]
    });
    let req = decode_body(&serde_json::to_vec(&body).unwrap());
    let Request::Generate(g) = &req else {
        panic!("not a generation");
    };
    assert_eq!(
        declarations::wire_paths(g),
        [
            Some("tools[0].functionDeclarations[0]".to_owned()),
            Some("tools[0].functionDeclarations[1]".to_owned()),
            None,
            Some("tools[2].functionDeclarations[0]".to_owned()),
        ]
    );
    let mut without_search = req.clone();
    if let Request::Generate(g) = &mut without_search {
        g.tools.remove(2);
    }
    let ctx = EgressCtx::new(&without_search);
    let err = OpenAiChat::check(&without_search, &ctx).unwrap_err();
    assert_eq!(
        err.param.as_deref(),
        Some("tools[2].functionDeclarations[0]"),
        "{err:?}"
    );
    assert!(
        err.message.starts_with(
            "`tools[2].functionDeclarations[0].behavior` comes from a Gemini API request"
        ),
        "{}",
        err.message
    );
}
