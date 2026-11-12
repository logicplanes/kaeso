//! A provider whose API path names the account, such as Cloudflare AI Gateway's
//! `/client/v4/accounts/{account_id}/ai/v1`, takes the place of `/v1` with the context's API root.

#![cfg(test)]

use kaeso_codecs::{EgressCtx, HeaderList, dispatch};
use kaeso_kpp::{DialectId, Request, RequestKind};
use serde_json::{Value, json};

const ROOT: &str = "/client/v4/accounts/0123456789abcdef0123456789abcdef/ai/v1";

fn decode(dialect: DialectId, body: &Value) -> Request {
    dispatch::decode_request(
        dialect,
        RequestKind::Generate,
        &serde_json::to_vec(body).unwrap(),
        &HeaderList::new(),
    )
    .unwrap()
}

fn path(dialect: DialectId, body: &Value, ctx: impl Fn(EgressCtx) -> EgressCtx) -> String {
    let req = decode(dialect, body);
    let mut ctx = ctx(EgressCtx::new(&req));
    dispatch::encode_request(dialect, &req, &mut ctx)
        .unwrap()
        .path
}

#[test]
fn chat_responses_and_messages_go_under_the_api_root() {
    let chat = json!({ "model": "@cf/zai-org/glm-4.7-flash", "messages": [{ "role": "user", "content": "Hi" }] });
    let responses = json!({ "model": "openai/gpt-5.6-luna", "input": "Hi" });
    let messages = json!({ "model": "anthropic/claude-haiku-5.5", "max_tokens": 8, "messages": [{ "role": "user", "content": "Hi" }] });
    let root = |c: EgressCtx| c.with_api_root(format!("{ROOT}/"));
    assert_eq!(
        path(DialectId::OpenAiChat, &chat, root),
        format!("{ROOT}/chat/completions")
    );
    assert_eq!(
        path(DialectId::OpenAiResponses, &responses, root),
        format!("{ROOT}/responses")
    );
    assert_eq!(
        path(DialectId::AnthropicMessages, &messages, root),
        format!("{ROOT}/messages")
    );
    assert_eq!(
        path(DialectId::AnthropicMessages, &messages, |c| c),
        "/v1/messages"
    );
}

#[test]
fn a_base_path_goes_before_the_api_root() {
    let chat = json!({ "model": "m", "messages": [{ "role": "user", "content": "Hi" }] });
    assert_eq!(
        path(DialectId::OpenAiChat, &chat, |c| c
            .with_api_root(ROOT)
            .with_base_path(
                "/v1/projects/p/locations/l/endpoints/openapi"
            )),
        "/v1/projects/p/locations/l/endpoints/openapi/chat/completions"
    );
}
