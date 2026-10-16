# kaeso-kpp

KPP, the Kaeso Provider Protocol: the canonical, stateless in-process representation of model requests, responses, stream events, errors, token counts, embeddings and model listings that every Kaeso wire format translates through.

This crate holds types only (no codec logic, no I/O). The codecs that translate the Anthropic and OpenAI wire formats into KPP and back are in [`kaeso-codecs`](https://crates.io/crates/kaeso-codecs). Both crates are developed in the [Kaeso repository](https://github.com/logicplanes/kaeso) and documented in [KPP internals](https://docs.kaeso.app/development/kpp).

License: MIT.
