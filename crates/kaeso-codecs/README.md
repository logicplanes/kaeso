# kaeso-codecs

Sans-IO codecs between [KPP](https://crates.io/crates/kaeso-kpp) and the wire formats Kaeso speaks, each in both directions (app ingress and provider egress):

| Dialect | Requests and responses | SSE streams | Token counting | Errors and model listings |
| --- | --- | --- | --- | --- |
| Anthropic Messages | yes | yes | yes | yes |
| OpenAI Chat Completions | yes | yes | no | yes |
| OpenAI Responses | yes | yes | yes | yes |
| OpenAI Embeddings | yes | no | no | yes |

What a dialect cannot do is refused with an `Unsupported` error.

```toml
[dependencies]
kaeso-kpp = "0.3"
kaeso-codecs = "0.3"
```

An app sent an Anthropic Messages request, and the model is served through OpenAI Chat Completions:

```rust
use kaeso_codecs::{EgressCtx, EncodeCtx, HeaderList, dispatch};
use kaeso_kpp::{DialectId, RequestKind, serde_json};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let body = br#"{"model":"claude-sonnet-4-6","max_tokens":256,"messages":[{"role":"user","content":"Hello"}]}"#;
    let headers = HeaderList::new().with("anthropic-version", "2023-06-01");
    let req = dispatch::decode_request(DialectId::AnthropicMessages, RequestKind::Generate, body, &headers)?;
    req.validate()?;

    let mut egress = EgressCtx::new(&req).with_provider("openai");
    let outgoing = dispatch::encode_request(DialectId::OpenAiChat, &req, &mut egress)?;
    assert_eq!(outgoing.path, "/v1/chat/completions");

    let reply_body = br#"{"id":"chatcmpl-1","object":"chat.completion","created":1791734780,"model":"gpt-5.5","choices":[{"index":0,"message":{"role":"assistant","content":"Hi!"},"finish_reason":"stop"}],"usage":{"prompt_tokens":8,"completion_tokens":2,"total_tokens":10}}"#;
    let reply = dispatch::decode_response(DialectId::OpenAiChat, 200, &HeaderList::new(), reply_body, &egress)?;
    let answer = dispatch::encode_response(DialectId::AnthropicMessages, &reply, &EncodeCtx::for_request(req))?;
    let answer: serde_json::Value = serde_json::from_slice(&answer)?;
    assert_eq!(answer["content"][0]["text"], "Hi!");
    Ok(())
}
```

`encode_request` runs the target's `check()` first and returns an `Unsupported` error that names any feature the target cannot express. Streams work the same way with `dispatch::stream_decoder` and `dispatch::stream_encoder`; errors use `dispatch::decode_error` and `dispatch::encode_error`. `cross` holds the rules for data that crosses dialects.

The crates are developed in the [Kaeso repository](https://github.com/logicplanes/kaeso). The reference documentation, with how the codecs are built and every field of every wire format, is at [docs.kaeso.app/development/kpp](https://docs.kaeso.app/development/kpp/).

License: MIT.
