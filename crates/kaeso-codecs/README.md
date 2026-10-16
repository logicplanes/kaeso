# kaeso-codecs

Sans-IO codecs between [KPP](https://crates.io/crates/kaeso-kpp) and the wire formats Kaeso speaks: Anthropic Messages, OpenAI Chat Completions, OpenAI Responses and OpenAI Embeddings, each in both directions (app ingress and provider egress), for requests, responses, SSE streams, errors, token counting and model listings.

```toml
[dependencies]
kaeso-kpp = "0.1"
kaeso-codecs = "0.1"
```

An app sent an Anthropic Messages request, and the model is served through OpenAI Chat Completions:

```rust
use kaeso_codecs::{EgressCtx, EncodeCtx, dispatch};
use kaeso_kpp::{DialectId, RequestKind};

let req = dispatch::decode_request(DialectId::AnthropicMessages, RequestKind::Generate, &body, &headers)?;
req.validate()?;
let mut egress = EgressCtx::new(&req).with_provider("openai");
let outgoing = dispatch::encode_request(DialectId::OpenAiChat, &req, &mut egress)?;

let reply = dispatch::decode_response(DialectId::OpenAiChat, status, &reply_headers, &reply_body, &egress)?;
let answer = dispatch::encode_response(DialectId::AnthropicMessages, &reply, &EncodeCtx::for_request(&req))?;
```

`encode_request` runs the target's `check()` first and returns an `Unsupported` error that names any feature the target cannot express. Streams work the same way with `dispatch::stream_decoder` and `dispatch::stream_encoder`; errors use `dispatch::decode_error` and `dispatch::encode_error`. `cross` holds the rules for data that crosses dialects.

The crates are developed in the [Kaeso repository](https://github.com/logicplanes/kaeso). How they are built, and every field of every wire format, is documented in [KPP internals](https://docs.kaeso.app/development/kpp).

License: MIT.
