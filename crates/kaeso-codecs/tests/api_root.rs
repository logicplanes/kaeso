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

// Volcengine Ark serves its API at `/api/v3`, not `/v1`: with the base URL ending at that root,
// the root is empty and every path loses its `/v1`; a root naming `/api/v3` goes in its place.
#[test]
fn an_empty_api_root_drops_v1_from_chat_listing_and_embeddings_paths() {
    let chat = json!({ "model": "doubao-seed-2-1-pro-260915", "messages": [{ "role": "user", "content": "Hi" }] });
    let embed = json!({ "model": "text-embedding-3-small", "input": "Hi" });
    let empty = |c: EgressCtx| c.with_api_root("");
    assert_eq!(
        path(DialectId::OpenAiChat, &chat, empty),
        "/chat/completions"
    );
    let req = dispatch::decode_request(
        DialectId::OpenAiEmbeddings,
        RequestKind::Embed,
        &serde_json::to_vec(&embed).unwrap(),
        &HeaderList::new(),
    )
    .unwrap();
    let mut ctx = EgressCtx::new(&req).with_api_root("");
    assert_eq!(
        dispatch::encode_request(DialectId::OpenAiEmbeddings, &req, &mut ctx)
            .unwrap()
            .path,
        "/embeddings"
    );
    assert_eq!(
        path(DialectId::OpenAiChat, &chat, |c| c.with_api_root("/api/v3")),
        "/api/v3/chat/completions"
    );
}

#[test]
fn the_openai_model_listing_and_model_check_go_under_the_api_root() {
    let query = kaeso_kpp::ModelsQuery::default();
    for dialect in [
        DialectId::OpenAiChat,
        DialectId::OpenAiResponses,
        DialectId::OpenAiEmbeddings,
    ] {
        let ctx = EgressCtx::for_kind(RequestKind::Generate, kaeso_kpp::StreamMode::Off);
        let list = |ctx: &EgressCtx| {
            dispatch::encode_models_request_with(dialect, &query, ctx)
                .unwrap()
                .path
        };
        let one = |ctx: &EgressCtx| {
            dispatch::encode_model_request_with(dialect, "doubao-seed-2-1-pro-260915", ctx)
                .unwrap()
                .path
        };
        assert_eq!(list(&ctx), "/v1/models", "{dialect:?}");
        assert_eq!(
            one(&ctx),
            "/v1/models/doubao-seed-2-1-pro-260915",
            "{dialect:?}"
        );
        let rooted = ctx.clone().with_api_root("");
        assert_eq!(list(&rooted), "/models", "{dialect:?}");
        assert_eq!(
            one(&rooted),
            "/models/doubao-seed-2-1-pro-260915",
            "{dialect:?}"
        );
        assert_eq!(
            list(&ctx.with_api_root("/api/v3/")),
            "/api/v3/models",
            "{dialect:?}"
        );
    }
}
