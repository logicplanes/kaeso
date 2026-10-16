---
title: KPP internals
description: How the KPP types and codecs are built, for contributors to kaeso-kpp and kaeso-codecs.
order: 3
nav: KPP internals
---

KPP, the Kaeso Provider Protocol, is the canonical format every model request passes through. The gateway decodes what an app sends into KPP and encodes KPP for the provider, and the reply takes the same path back. What people who call Kaeso see of it is under [format translation](../../concepts/format-translation.md); these pages are for people who change it.

| Page | What it covers |
| --- | --- |
| [Crossing formats](cross-format.md) | The rules for data that moves from one API format to another, and why |
| [Field classes](field-classes.md) | Every KPP field as semantic or hint |
| [Extras allowlist](extras-allowlist.md) | The extras that may be dropped when a request changes format |
| [Anthropic Messages](anthropic-messages.md), [OpenAI Chat Completions](openai-chat.md), [OpenAI Responses](openai-responses.md), [OpenAI Embeddings](openai-embeddings.md) | Every wire field of each format and where it goes in KPP |
| [Fixtures and recordings](fixtures.md) | The real payloads the codecs are tested against, and how to record new ones |

## Crates and modules

| Crate | Holds |
| --- | --- |
| `kaeso-kpp` | The KPP types only: requests, responses, stream events, errors, token counts, embeddings and model listings, with `validate()` and `required_capabilities()`. No codec logic, no I/O. |
| `kaeso-codecs` | One sans-IO codec per API format, called a dialect in the code (`DialectId`), and the building blocks they share. A codec turns bytes into KPP values and KPP values into bytes; streams are fed in chunk by chunk and produce events. |

Both crates are published to crates.io. Their library code may use only `serde`, `serde_json`, `bytes`, `thiserror`, `indexmap`, `base64` and `mime`; `cargo deny` bans `tokio`, `reqwest`, `hyper` and `axum` from them ([dependencies](../dependencies.md#kpp)).

| Module | Dialect |
| --- | --- |
| `anthropic_messages` | Anthropic Messages (`/v1/messages`, `/v1/messages/count_tokens`) |
| `openai_chat` | OpenAI Chat Completions (`/v1/chat/completions`) |
| `openai_responses` | OpenAI Responses (`/v1/responses`, `/v1/responses/input_tokens`) |
| `openai_embeddings` | OpenAI Embeddings (`/v1/embeddings`) |
| `openai_models` | The OpenAI Models API, shared by the three OpenAI codecs |

The shared modules are `codec` (the contract), `dispatch` (the codec functions chosen by `DialectId` at run time), `wire` (serde building blocks), `sse` (the event-stream parser and writer), `cross` (rules for data that crosses dialects), `continuity` (reasoning that travels with the conversation), `keepalive` (frames that keep live streams from going silent), `models` (model-listing helpers) and `testing` (round-trip comparison, also for crates that use the codecs).

## Design principles

- **Stateless.** A `GenerateRequest` always carries the whole conversation.
- **Messages of ordered content blocks.** Block order is kept end to end.
- **Raw where bytes matter.** Tool arguments, JSON schemas, unknown blocks and citations are `RawJson`; opaque reasoning is kept as received and tagged with its `Origin`.
- **Semantic or hint.** Every field is either semantic (a target must honour it or refuse with `Unsupported`) or a hint (it may be ignored). [Field classes](field-classes.md) lists them.
- **Bound extras.** Whatever KPP does not model is kept in `Extras`, tagged with the dialect it came from, and only written back to that dialect. `fields` are semantic: another dialect refuses them unless they are on the [allowlist](extras-allowlist.md). `form` is presentation (a string or an array, `null` or absent, how chunks were grouped) and other dialects ignore it. Unknown keys of nested objects use dotted key paths, such as `function.x`.
- **Refuse before sending.** `check()` returns `Unsupported` naming the feature a target cannot express, before anything is sent.
- **Nothing is dropped silently.** Every field and block of an incoming payload ends up in exactly one place: a typed KPP field, a dialect-bound `Extras` entry, an `Unknown` item, or a `KppError` of kind `Unsupported`. A match arm that discards data is a bug.
- **Additive public API.** Public enums and growable structs are `#[non_exhaustive]`; enum variants wrap `#[non_exhaustive]` structs with constructors (`TextBlock::new`, `Block::text`, …), so fields can be added without breaking code that builds or matches values. The `Codec` trait is sealed.

KPP values and errors are returned by value. `KppError` and some enum variants are large, but a value is built once per request and errors are the cold path; boxing them would make every signature clumsier, so the workspace allows `clippy::large_enum_variant` and `clippy::result_large_err`.

Every KPP type implements `serde` for logs and snapshots. This JSON form is stable across patch releases, but it is not a wire format.

## Types

The types were shaped by what the codecs need to stay lossless:

- **`Extras`** has three fields: `origin`, `fields` (raw JSON, byte for byte) and `form` (presentation markers). Same-dialect round trips must reproduce spellings KPP normalises away, such as `content` as a string or an array, `null` or absent, and chunk boundaries; those markers carry no meaning, so they never make another dialect refuse a request the way unknown `fields` do. Two empty `Extras` are equal whatever their origin.
- **Forwarded headers** (`anthropic-beta`, `anthropic-version`, `openai-beta`) are stored in `Extras::fields` under keys starting with `header:`; repeated headers are joined with `", "`. No body field contains a colon, so the keys never collide, and headers are bound to their dialect like any other extra.
- **Raw items.** `Box<RawValue>` is wrapped in `RawJson`, which compares byte for byte. Unknown blocks, tools, tool choices, stream events and deltas are `RawItem { origin, raw }`; citations are `Citation { origin, raw }`, since citation formats share almost nothing across dialects.
- **Replies.** `Reply` is `Generate`, `CountTokens` or `Embed`, and `RequestKind` names the same three kinds: Anthropic's `/v1/messages` and `/v1/messages/count_tokens` bodies can't be told apart without the endpoint.
- **Tools.** `ToolDef` is a function tool or an unknown tool kept whole; `ToolChoice` is `Auto`, `None`, `Required`, a named tool, or unknown. A function tool's `parameters` are optional, as OpenAI allows.
- **Output and reasoning settings.** `OutputSpec::JsonSchema` has an optional name (Anthropic schemas have none) and a description. `ReasoningConfig` holds the mode, effort, budget, summary and whether to include encrypted reasoning; its enums keep values they don't know in `Other(String)`, since providers add effort levels often.
- **Reasoning blocks** hold the origin, summaries, text parts and opaque parts, because an OpenAI reasoning item carries both an id and encrypted content and may have several text parts, and a summary-only block still needs its origin to decide where it may be replayed.
- **Usage.** `Usage::input_tokens` excludes cache reads and writes, which are fields of their own ([usage](cross-format.md#usage)); `Usage::raw` keeps the provider's usage object with its origin, so an encoder overlays it only for its own dialect. Embedding vectors are `f64`, so provider values print back unchanged.
- **Errors.** `ErrorKind::Billing` exists for Anthropic `billing_error` (402) and OpenAI `insufficient_quota`, which no other kind describes faithfully.

## Streams

A KPP stream is `ResponseStart → (BlockStart → BlockDelta* → BlockStop)* → ResponseDelta* → ResponseStop`, with `Error` possible at any point and `Extra` carrying dialect events KPP does not model. Block indexes are KPP's own: 0, 1, 2, … in the order blocks start. Blocks may interleave (OpenAI streams parallel tool calls this way), so every event names its block. Every lifecycle event carries `Extras`, so a same-dialect replay reproduces the original events exactly, including per-event fields such as OpenAI's `obfuscation`.

Stream decoders report failures in band: the events decoded so far, then a final `StreamEvent::Error`, so nothing decoded earlier in the same chunk is lost.

## The codec contract

`Codec` is a trait of associated functions chosen at compile time: requests, replies, errors, stream decoders and encoders, and model listings, for both sides of the gateway. On the ingress side an app speaks the format to Kaeso; on the egress side Kaeso speaks it to a provider. The gateway learns both dialects per request, so `dispatch` offers the same functions taking a `DialectId`; a dialect without a codec yields `Unsupported`.

`decode_request` takes a `RequestKind`. `EncodeCtx` carries the app's decoded request, because OpenAI Responses echoes request fields in its reply, and a `created_at`, because a sans-IO encoder needs a deterministic clock. `EgressCtx` carries what the gateway knows about the target: the request kind and stream mode, the provider, and whether the model reasons or has adaptive thinking; the request encoder adds the tool ids and names it rewrote, which the reply decoders map back.

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

`encode_request` runs the target's `check()` first. Streams work the same way with `dispatch::stream_decoder` and `dispatch::stream_encoder`, and errors with `dispatch::decode_error` and `dispatch::encode_error`.

## Wire structs

Wire structs must keep every field, including ones the struct does not declare, and must keep raw payloads (`RawValue`) byte for byte. `#[serde(flatten)]` can't do both: flattening buffers the input, and `RawValue` can't be read from a buffer, so tool arguments and schemas would lose their bytes. `wire::Ext<T>` does the same job without buffering:

- every key of the JSON object that `T` does not declare is kept raw in `Ext::extra`, and becomes KPP `Extras::fields`;
- every declared key whose received spelling `T` can't reproduce (`"x": null` for an `Option` skipped when `None`, `"flag": false` for a `bool` skipped when false, `"list": []` for a `Vec` skipped when empty) is kept raw in `Ext::verbatim`, and becomes KPP `Extras::form`.

Serializing an `Ext<T>` writes `T`, then the verbatim keys `T` did not write, then the extra keys. A decode and encode round trip is therefore exact, and a codec that changes a typed field, for example when translating from another dialect, always wins over a stale verbatim spelling. `wire::Maybe<T>` tells absent from `null` where the difference must be visible to the codec.

Wire structs used inside `Ext` follow these rules: no `#[serde(flatten)]`, `alias`, `deny_unknown_fields`, untagged or internally tagged enums; tagged unions are dispatched on `"type"` with `wire::type_tag`; JSON objects with duplicate keys are refused. Fields are declared only when the codec maps them to a typed KPP field; everything else is left undeclared on purpose and travels as extras.

The OpenAI Responses codec reads typed values with plain serde structs and writes with an ordered JSON tree (`openai_responses::node::Node`) that can embed raw JSON byte for byte; `serde_json::Value` sorts object keys and would reformat raw payloads. [How exact spellings survive](openai-responses.md#how-exact-spellings-survive) describes how it reproduces them.

## Server-sent events

`sse::SseParser` follows the [WHATWG event-stream algorithm](https://html.spec.whatwg.org/multipage/server-sent-events.html#event-stream-interpretation), with deliberate differences so that nothing is lost:

- a frame that has fields but no `data:` line is still reported, with empty `data`;
- comments and unknown fields are reported instead of ignored;
- a stream that ends in the middle of a line is reported as `SseError::Truncated` instead of being discarded.

Bytes can be fed in any chunks: lines may be split anywhere, including between the `\r` and `\n` of a CRLF line ending. A frame larger than the parser's limit (`sse::DEFAULT_MAX_FRAME_BYTES`, 16 MiB) is an error, which bounds the memory each stream can take.

## Model listings

Model listing (`GET /v1/models`, `GET /v1/models/{id}`) is part of KPP, with `ModelInfo`, `ModelLifecycle`, `ModelList` and `ModelsQuery`, so the gateway never has to know the wire shapes of the same path itself. Every dialect uses the same path, so the ingress dialect can't come from it: `dispatch::models_dialect` picks it from the headers (`anthropic-version` means Anthropic, anything else OpenAI). `ModelList::page` applies `before_id`, `after_id`, `limit` and the lifecycle filter the same way for every dialect.

- **Query strings** are split on `&` and `=` and percent-decoded as `application/x-www-form-urlencoded` (`+` is a space). Unknown parameters become extras under `query.<name>`.
- **Dates.** KPP stores Unix seconds. RFC 3339 date-times (Anthropic) and full dates (OpenAI `shutdown_date`) are converted with the proleptic Gregorian calendar, without a date library. Sub-second digits and offsets don't survive the conversion; the exact received text is kept in the form marker `spelling` whenever encoding again would not reproduce it. A date after the year 9999 can't be written in RFC 3339: Anthropic `created_at` then gets the documented unknown value (the epoch), other dates `null`.
- **Absent or `null` fields.** An encoder fills a field KPP has no value for with the documented default; when the received body spelled that field differently, the form marker `none` says so.

## Stream encoders

**Anthropic Messages.** Anthropic streams one content block at a time, while KPP streams from other dialects may interleave blocks: OpenAI streams parallel tool calls that way and closes every block at the end. The encoder is an explicit state machine. The first block that starts is active and is written as it arrives; blocks that start while another is active are pending and buffered in start order; when the active block stops, the next pending block is written, with its start, its buffered deltas, and its stop if it already came. A `ResponseDelta` or `ResponseStop` closes every open block first, and several `ResponseDelta`s are merged into one `message_delta`, because the SDKs overwrite `stop_reason` with each one. Anthropic indexes are assigned in the order blocks are written. A stream that doesn't start with `ResponseStart` gets a `message_start` with an empty id.

**OpenAI Chat Completions.** Events decoded from Chat are grouped into their original chunks again with the chunk form markers, so a same-dialect replay reproduces every frame. Events from other dialects become one chunk each. The stream opens with a role chunk as soon as the response starts (`delta: {"role": "assistant", "content": "", "refusal": null}`, OpenAI's own first chunk), so the app hears from the stream while the model thinks; a tool call starts with a header chunk followed by argument chunks; usage is sent only when the app asked for it (`stream_options.include_usage`), as a last chunk with `choices: []`. `ResponseStop` writes `data: [DONE]`; an `Error` writes `data: {"error": …}` and ends the stream without `[DONE]`.

**OpenAI Responses.** The stream engine produces, for each KPP event, the SSE events in the order OpenAI sends them, including every redundant one (`.done` events, part events, item events, the final response object with all output items), and assigns `sequence_number`s. The decoder runs the same engine as a shadow: every SSE event the engine would produce at that point is attached to the KPP event, with differences recorded as scoped overrides; attached events the original did not have are recorded in the form marker `skip`; events that arrive where the engine would not produce them are passed on as `Extra`. A same-dialect stream is reproduced exactly, and a stream from another dialect becomes a complete, valid Responses stream.

Every stream encoder can also write a keepalive frame; [keepalives](cross-format.md#keepalives) says when.

## Tests

| Suite | What it proves |
| --- | --- |
| `crates/kaeso-kpp/tests/types.rs` | The KPP types alone: stable names of error kinds and blocks, `validate()` and `required_capabilities()`. |
| Fixture round trips (each dialect's `tests` modules) | `decode → KPP → encode` reproduces every [fixture](fixtures.md) of a dialect as an equal JSON value (streams: an equal event sequence), under many chunkings for streams. Fixtures that must be refused fail with the named `Unsupported` feature. |
| `crates/kaeso-codecs/tests/cross_matrix.rs` | For every generation fixture of dialect A and every other dialect B: requests go through `decode_A → check_B → encode_B` and must decode again with B's decoder, keeping the conversation; replies and streams go through `decode_A → encode_B → decode_B` and must keep the text, refusals and tool calls, also back in A. Token-count requests and replies cross between Anthropic and Responses. The outcomes are `insta` snapshots. |
| `crates/kaeso-codecs/tests/dispatch.rs` | `dispatch` decodes provider errors and encodes errors with a status in every dialect, and the embeddings codec refuses generation replies. |
| `crates/kaeso-codecs/tests/kpp_proptest.rs` | Random valid KPP requests and replies survive encode and decode for every generation dialect, on the subset every dialect supports. |
| `crates/kaeso-codecs/tests/keepalive.rs` | Whatever an app can't see still puts keepalive frames on the wire regularly, and the frames are invisible to the app's parser. |
| `crates/kaeso-codecs/tests/reasoning_carry.rs` | Reasoning goes to an app of another dialect and comes back on the next turn byte for byte, through Responses, Anthropic and Chat apps. |
| `crates/kaeso-codecs/tests/responses_on_anthropic.rs` | OpenAI Responses apps on Anthropic models and Anthropic apps on OpenAI models: reasoning requests, thinking streams in OpenAI's event order, and tool loops. |
| `e2e/conformance`, served by `crates/kpp-conformance` | The official OpenAI and Anthropic Python SDKs parse everything the encoders produce ([testing](../testing.md#sdk-conformance)). |
| `fuzz/` | Every decoder accepts any bytes without panicking, and the SSE parser yields the same items for any chunking ([fuzzing](../testing.md#fuzzing)). |

`kaeso_codecs::testing` compares JSON structurally: key order never matters, integers compare exactly, and a number written with a fraction or exponent compares by value with any other number, so `1` equals `1.0`. KPP stores `temperature`, `top_p` and the penalties as `f64`, and the providers treat both spellings the same.

When a change alters cross-dialect output, review the snapshot diff: `INSTA_UPDATE=always cargo test -p kaeso-codecs --test cross_matrix`, then read the changed `.snap` files before committing them.

## Changing a codec

1. **Fixture first.** Add the real payload to `crates/kaeso-codecs/fixtures/<dialect>/` with a provenance row in that folder's `README.md` ([fixtures](fixtures.md)).
2. **Name the source.** Every mapping names its source in the dialect's mapping page: the provider's documentation or the OpenAPI schema at the pinned spec commit. The sources are the [OpenAI OpenAPI spec](https://github.com/openai/openai-openapi), the Anthropic OpenAPI spec named on the [Anthropic Messages](anthropic-messages.md) page, the official SDK types ([openai-python](https://github.com/openai/openai-python), [anthropic-sdk-python](https://github.com/anthropics/anthropic-sdk-python)), and the provider documentation. Where the sources are ambiguous, choose the conservative behaviour, `Unsupported`, and open an issue.
3. **Failing test, then code.** Every wire field maps to a typed KPP field, an origin-bound `Extras` entry, an `Unknown` item or an `Unsupported` error.
4. **Update the mapping page** of the dialect, in all three languages, in the same pull request.

Error messages are written for the person who called Kaeso: they say which field the target doesn't support and what to do.
