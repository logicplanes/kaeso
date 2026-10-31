# kaeso-kpp

KPP, the Kaeso Provider Protocol: the canonical, stateless in-process representation of model requests, responses, stream events, errors, token counts, embeddings and model listings that every Kaeso wire format translates through.

This crate holds the KPP types and the rules that need no wire format: `validate()` checks a request, `required_capabilities()` says what a model must support to serve it, and `ModelList::page` pages a model listing. It has no codec logic and no I/O. The codecs that translate the Anthropic and OpenAI wire formats into KPP and back are in [`kaeso-codecs`](https://crates.io/crates/kaeso-codecs).

```toml
[dependencies]
kaeso-kpp = "0.5"
```

```rust
use kaeso_kpp::{GenerateRequest, Message, Request, StreamMode, serde_json};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let req = GenerateRequest::new("claude-sonnet-4-6")
        .with_system("Answer in one sentence.")
        .with_message(Message::user("Hello"))
        .with_stream(StreamMode::on(true));
    req.validate()?;
    assert!(req.required_capabilities().streaming);

    let json = serde_json::to_value(Request::Generate(req))?;
    assert_eq!(json["generate"]["model"], "claude-sonnet-4-6");
    Ok(())
}
```

Both crates are developed in the [Kaeso repository](https://github.com/logicplanes/kaeso). The reference documentation, with every type and every field of every wire format, is at [docs.kaeso.app/development/kpp](https://docs.kaeso.app/development/kpp/).

License: MIT.
