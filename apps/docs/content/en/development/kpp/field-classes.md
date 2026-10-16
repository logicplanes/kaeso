---
title: Field classes
description: Every KPP field as semantic, which a target must honour, or a hint, which it may ignore.
order: 2
---

Every KPP field is either **semantic** or a **hint** (see the [design principles](index.md#design-principles)).

- **Semantic:** the target must honour it. If the target dialect or model cannot express it, the codec's `check()` returns `Unsupported` naming the field. Nothing is sent.
- **Hint:** the target may ignore it. The codec drops it silently where the target has no equivalent.

"Dropped" below always means *only where the target cannot express it*; a target that can express a hint always receives it.

## `GenerateRequest`

| Field | Class | Notes |
| --- | --- | --- |
| `model` | semantic | Resolved by the gateway before encoding. |
| `system` | semantic | |
| `messages` | semantic | |
| `tools` (`Function`) | semantic | Its `cache` is a hint, as for blocks ([cache hints](cross-format.md#cache-hints)). |
| `tools` (`Unknown`) | semantic | Only expressible by its origin dialect. |
| `tool_choice` | semantic | `Unknown` only expressible by its origin dialect. |
| `parallel_tool_calls` | semantic | |
| `output` | semantic | `JsonObject` has no Anthropic equivalent and is `Unsupported`, with no emulation ([structured output](cross-format.md#structured-output)). |
| `reasoning.mode`, `.effort`, `.budget_tokens` | semantic | Converted between effort and budget by the [effort and budget](cross-format.md#effort-and-budget) table. |
| `reasoning.summary` | **hint** | Only controls whether summaries come back. Anthropic: `thinking.display: "summarized"` ([readable reasoning and summaries](cross-format.md#readable-reasoning-and-summaries)). |
| `reasoning.include_encrypted` | **hint** | Only controls whether encrypted reasoning comes back; OpenAI Responses only ([readable reasoning and summaries](cross-format.md#readable-reasoning-and-summaries)). |
| `sampling.max_output_tokens` | semantic | |
| `sampling.temperature`, `.top_p` | semantic | |
| `sampling.top_k` | semantic | Anthropic only; `Unsupported` elsewhere ([sampling](cross-format.md#sampling)). |
| `sampling.stop` | semantic | OpenAI Chat Completions takes at most four; OpenAI Responses has none and returns `Unsupported` ([sampling](cross-format.md#sampling)). |
| `sampling.seed` | semantic | OpenAI Chat Completions only; `Unsupported` elsewhere ([sampling](cross-format.md#sampling)). |
| `sampling.frequency_penalty`, `.presence_penalty` | semantic | OpenAI Chat Completions only; `Unsupported` elsewhere ([sampling](cross-format.md#sampling)). |
| `user` | **hint** | An end-user id for abuse monitoring; it never changes the output. |
| `stream` | semantic | |
| `extras.fields` | semantic | Only written back to the origin dialect; elsewhere `Unsupported` unless on the [extras allowlist](extras-allowlist.md). |
| `extras.form` | **hint** | Presentation markers; ignored by other dialects. |

## `Message`

| Field | Class | Notes |
| --- | --- | --- |
| `role` | semantic | A `System` message after the start of the conversation is `Unsupported` for Anthropic unless the request came from Anthropic Messages ([system messages](cross-format.md#system-messages)). |
| `blocks` | semantic | Block order is preserved. |
| `extras` | as for the request | |

## `Block`

| Variant / field | Class | Notes |
| --- | --- | --- |
| `Text.text` | semantic | |
| `Text.citations` | semantic | Raw and origin-bound; another dialect gets `Unsupported`. |
| `Text.cache`, `Image.cache`, `Document.cache`, `ToolCall.cache`, `ToolResult.cache` | **hint** | Dropped for OpenAI, which caches by itself ([cache hints](cross-format.md#cache-hints)). |
| `Image.source` | semantic | A `FileId` works only with the provider that issued it ([files](cross-format.md#files)). OpenAI Chat Completions takes images only as a URL or base64. |
| `Document.source` | semantic | Text sources only for Anthropic. OpenAI Chat Completions also refuses URL sources. |
| `Document.title`, `.context` | semantic | Anthropic only; `Unsupported` for OpenAI. |
| `Document.citations` | semantic | Anthropic only; `Unsupported` for OpenAI, which cannot cite documents. |
| `ToolCall.*` | semantic | Ids and names are kept; a target that rejects one, for its characters or its length, gets a deterministic rewrite, reversed on the way back. |
| `ToolResult.call_id`, `.content` | semantic | |
| `ToolResult.is_error` | **hint** for targets without an error flag | Dropped for OpenAI Chat and Responses; the content carries the error text ([tool errors](cross-format.md#tool-errors)). |
| `Reasoning.summary`, `.text` | **hint** | Readable reasoning is dropped for another dialect's request ([reasoning](cross-format.md#reasoning)). In replies it is shown together with the original block in a [reasoning blob](cross-format.md#reasoning-blobs). |
| `Reasoning.opaque` | semantic for its origin | Replayed byte for byte to its origin dialect and provider; never sent elsewhere, where the whole block is dropped ([reasoning](cross-format.md#reasoning)). |
| `Reasoning.origin` | — | Decides replay ([reasoning](cross-format.md#reasoning)). |
| `Refusal.text` | semantic | Anthropic has no refusal block: `Unsupported` in requests to it, and a text block in replies to its apps. |
| `Unknown` | semantic | Origin dialect only. |
| `extras` of every block | as for the request | Allowlisted per block kind ([extras allowlist](extras-allowlist.md)). |

## `CacheHint`

| Field | Class | Notes |
| --- | --- | --- |
| `ttl` | **hint** | |
| `extras` | **hint** | Dropped with the hint. |

## `EmbedRequest`

| Field | Class | Notes |
| --- | --- | --- |
| `model`, `input`, `dimensions`, `encoding` | semantic | |
| `user` | **hint** | |
| `extras` | as for `GenerateRequest` | |

## Replies and stream events

Replies and stream events flow from one provider to one app. They have no hints: a field the app's dialect cannot express is either derivable, such as a stop reason mapped to the closest equivalent, or carried in extras for same-dialect replay. Each dialect's mapping page lists these mappings: [Anthropic Messages](anthropic-messages.md), [OpenAI Chat Completions](openai-chat.md), [OpenAI Responses](openai-responses.md).

Reasoning reaches an app of another dialect where its format can carry it, wrapped so it can come back ([reasoning blobs](cross-format.md#reasoning-blobs), `cross::reasoning_reply_policy`). Unknown blocks reach only apps of their own dialect. Everything else is left out of the app's view ([replies](cross-format.md#replies), `cross::reply_policy`), with [keepalive](cross-format.md#keepalives) frames in streams. The gateway logs the full KPP reply.
