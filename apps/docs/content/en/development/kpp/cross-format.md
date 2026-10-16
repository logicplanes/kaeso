---
title: Crossing formats
description: The rules the codecs follow when a request or reply moves from one API format to another, and the reasons for them.
order: 1
---

An app may speak one API format while the provider speaks another. These are the rules the codecs follow then, with the reason for each. A request that stays in its own format is reproduced exactly and none of these rules apply.

## Requests

`cross::check_foreign` walks a request and refuses with `Unsupported` what the target can't use:

- extras fields from another dialect, unless the field is on the [allowlist](extras-allowlist.md) or crosses by its value (`cross::is_droppable_value`); extras that hold fields but no origin are refused too, because nobody could write them back;
- unknown blocks, tools and tool choices from another dialect;
- provider file ids the target provider can't resolve ([files](#files));
- citations from another dialect: no target documents what citations in earlier turns do, and the formats don't map.

Not refused, because the encoders drop them instead: reasoning blocks the target can't replay ([reasoning](#reasoning)), and cache hints where the target caches by itself ([cache hints](#cache-hints)).

`check()` also refuses what a target can't express at all. Anthropic needs `max_tokens`, so a generation request without `sampling.max_output_tokens` is `InvalidRequest` for it, naming `max_tokens`, rather than getting an invented value.

## Replies

Replies flow from one provider to one app, and refusing a finished, paid-for reply would be worse than showing less of it. Data of another dialect is therefore, in this order of preference (`cross::reply_policy`, `cross::reasoning_reply_policy`):

1. **mapped** where the app's dialect can represent it: reasoning of another dialect becomes an OpenAI Responses reasoning item or an Anthropic `thinking` block, with the original block wrapped in a [reasoning blob](#reasoning-blobs) the app sends back unchanged;
2. **left out** where it can't, and replaced by [keepalive](#keepalives) frames in streams so the connection never goes silent: reasoning for OpenAI Chat apps (the gateway [keeps it](#reasoning-kept-by-the-gateway) between turns), unknown blocks and stream extras from another dialect, foreign citations and extras.

The gateway logs the KPP reply in full, so nothing is lost from Kaeso's records. Unknown blocks, citations and extras only reach another dialect when the provider sends something the request did not ask for, because the request side refuses foreign server tools, citations and similar features.

Stop reasons the app's format doesn't have are mapped to the closest one it has; each [mapping page](index.md) lists them.

## Extras

Extras captured from dialect A are written back only to dialect A. When a request carrying extras goes to another dialect, the target's `check()` returns `Unsupported` naming the field, unless the field is on the [extras allowlist](extras-allowlist.md): then it is dropped silently. The allowlist only holds fields known to have no effect on what the model writes, such as beta headers and resolution hints, each with the real client that sends it.

## Usage

`Usage::input_tokens` excludes cache reads and cache writes, which are fields of their own. This matches the price columns of models.dev, so a cost is the sum of each count times its price. The provider's raw usage object is always kept.

## System messages

A system message after the start of a conversation is kept as a `Role::System` message.

- Going to Anthropic, the system messages of a request decoded from Anthropic Messages are sent unchanged, and the API judges their placement and which models support them. Claude Code sends such messages on every request.
- System messages from other dialects are `Unsupported` for Anthropic: Anthropic allows them only for some models and only after a user turn, while OpenAI system and developer messages may sit anywhere.
- Going to OpenAI, they become later `system` or `developer` messages.

## Reasoning

Anthropic signs its thinking blocks and OpenAI encrypts its reasoning items. Opaque reasoning (signatures, encrypted content, item ids) is only ever replayed to the dialect and provider that issued it.

- **In requests,** readable reasoning text and summaries going to another dialect are dropped as a hint; the [reasoning blob](#reasoning-blobs) next to them restores the original block when the conversation returns to its provider.
- **In replies,** reasoning of another dialect is shown in the app's own reasoning shape where it has one, with the original block in a reasoning blob, so it reaches its origin again on the next turn. OpenAI Chat apps have no such shape, so the gateway keeps their reasoning between turns.
- **Reasoning settings** convert between an effort and a token budget:

| From | To | Becomes |
| --- | --- | --- |
| Effort `none` | Anthropic | No thinking, Anthropic's default |
| Effort `low` to `max` | Anthropic model with adaptive thinking | Adaptive thinking with that effort |
| Effort `low` to `max` | Anthropic model without adaptive thinking | `enabled` thinking with a budget from the table below |
| A budget alone | Anthropic | `enabled` thinking with that budget |
| An Anthropic budget | OpenAI | An effort from the table below |
| Anthropic `thinking: {type: "disabled"}` | OpenAI | Effort `none` |

Both OpenAI dialects document `none` as the lowest value of their shared `ReasoningEffort` enum, so a disabled Anthropic thinking always has an equivalent. The effort `minimal` has no row and stays `Unsupported` for Anthropic.

### Effort and budget

The conversion lives in `kaeso_codecs::cross::reasoning_map`.

| Anthropic `budget_tokens` | OpenAI effort |
| --- | --- |
| below 4,096 | `low` |
| 4,096 to 16,383 | `medium` |
| 16,384 and more | `high` |

An explicit effort in the same request wins over the budget.

| OpenAI effort | Anthropic `budget_tokens` |
| --- | --- |
| `low` | 2,048 |
| `medium` | 8,192 |
| `high`, `xhigh`, `max` | 24,576 |

This table applies only when the target model has no adaptive thinking (Claude 4.5 and earlier), which the gateway says with `EgressCtx::adaptive_thinking = Some(false)` from the catalogue's `adaptive_thinking` field; Claude 4.7 and later refuse `enabled` thinking and get adaptive thinking with the effort instead. The budget is then fitted to Anthropic's rules: at least 1,024 and less than `max_tokens`, so it becomes `max_tokens − 1` when that is smaller. When `max_tokens` is 1,024 or less no valid budget exists, so `max_tokens` is raised by 1,024 and the budget is 1,024: the app keeps its own `max_tokens` for the answer. `reasoning_map::budget_within` does the fitting and raising, `reasoning_map::fit_budget` only the fitting.

The sources are OpenAI's `ReasoningEffort` enum in the OpenAPI spec (`none`, `minimal`, `low`, `medium`, `high`, `xhigh`, `max`; not every model supports every value) and Anthropic's [extended thinking](https://platform.claude.com/docs/en/build-with-claude/extended-thinking) page: `budget_tokens` is at least 1,024 and less than `max_tokens`, and adaptive thinking doesn't exist on Claude Sonnet 4.5, Opus 4.5, Haiku 4.5 and earlier Claude 4 models.

### Readable reasoning and summaries

`reasoning.summary` and `reasoning.include_encrypted` are hints across dialects: they only control what reasoning comes back, so they are dropped where the target has no equivalent. Codex sends both on every request.

OpenAI returns readable reasoning only when the request sets `reasoning.summary`, and Claude Opus 4.7 and later return thinking blocks with empty text unless the request sets `thinking.display: "summarized"`. So:

- a request from another dialect with a reasoning summary (`auto`, `concise`, `detailed`) and thinking on goes to Anthropic with `display: "summarized"`, except for token counting;
- an Anthropic Messages request with thinking on (`enabled`, `adaptive`, or an effort other than `none`) and a `display` other than `omitted` goes to OpenAI Responses with `summary: "auto"`.

The gateway also tells the encoders whether the target model reasons (`EgressCtx::reasoning`, from the catalogue's `reasoning`). With `Some(true)`, a summary without an effort, mode or budget (OpenAI `reasoning: {summary: "auto"}`) turns Anthropic thinking on at the model's default depth: adaptive without an effort, or the `medium` budget, OpenAI's default effort. Anthropic `adaptive` or `enabled` thinking without an effort asks OpenAI Responses for `include: ["reasoning.encrypted_content"]`. Requests without reasoning settings, the effort `none`, and models that don't reason or aren't known never get reasoning switched on. OpenAI Chat apps, whose format shows no reasoning, ask for no summaries.

### Thinking and sampling

When Kaeso turns thinking on for a request from another dialect, what Anthropic forbids next to thinking is adjusted instead of refused, because sampling settings and forced tools are routine in OpenAI apps:

- `temperature`, `top_p` and `top_k` are left out. Anthropic refuses `temperature` and `top_k` with thinking and limits `top_p` to 0.95 to 1, and Claude Opus 4.7 and later refuse any value that isn't the default.
- `enabled` thinking is not sent with a forced tool (`tool_choice` `required` or a named tool), a combination Anthropic refuses. Where Anthropic accepts forced tools, with adaptive thinking, the reply starts with the tool call and holds no thinking anyway, so nothing is lost. A forced tool with adaptive thinking is sent unchanged.

An Anthropic app's own settings are sent as received.

### Tool turns without thinking

Anthropic refuses a request with `enabled` thinking whose last assistant turn made tool calls without a replayable thinking block first ("a final assistant message must start with a thinking block"). Such a request goes to Anthropic without `thinking` for that turn. Adaptive thinking is kept, because adaptive models don't need the leading thinking block.

Since reasoning travels with the conversation, this is rare. It still happens when the turn really came from another provider (a model switch in the middle of a tool loop), when an app dropped the reasoning item, thinking block or blob, or when the gateway no longer holds a Chat app's reasoning because it expired or another key sent the turn.

## Reasoning continuity

Without help, every tool-loop turn of an app on another provider's model would lose that model's reasoning: budget thinking would be switched off on every continuation turn, and adaptive thinking would lose what it reasoned before. `kaeso_codecs::continuity` makes reasoning travel with the conversation.

### Reasoning blobs

For OpenAI Responses and Anthropic Messages apps, KPP wraps the whole reasoning block of another dialect (readable text, summaries, signature, encrypted content, redacted data, item id) into a reasoning blob: `continuity::REASONING_BLOB_PREFIX` (`kpp:reasoning:v1:`) followed by the block's KPP JSON, form markers removed, in unpadded base64url.

- OpenAI Responses apps get a reasoning item with an `rs_kaeso_…` id, the thinking as one summary text, and the blob in `encrypted_content`; streams carry it as `response.reasoning_summary_*` events.
- Anthropic Messages apps get a `thinking` block with the summaries and reasoning text as thinking, paragraphs separated by a blank line, and the blob as its `signature`, streamed as one `signature_delta` when the block stops.

Both request decoders recognise a blob wherever it sits (`continuity::unwrap_reasoning`) and restore the original block with its origin, so the encoders replay it to its own dialect and provider like any other reasoning.

The blob is sent whether or not the app asked for `include: ["reasoning.encrypted_content"]`, and whatever its `store` setting: Kaeso stores nothing, and the SDK agent loops (openai-agents' `to_input_list()`, Codex, Claude Code, the Anthropic tool runner) send output items and content blocks back as received. The blob holds the readable text a second time, because it must restore the block exactly (Anthropic checks thinking text against its signature) while apps may edit or drop the readable copy; when an app sends both back, the blob wins.

The prefix contains `:`, which is in neither the base64 alphabet of Anthropic signatures nor the base64url alphabet of OpenAI's encrypted content, so a blob is never mistaken for the providers' own data. A value with the prefix that doesn't decode is kept as the provider's own opaque data, and the provider then refuses it, so nothing is dropped silently. The blob is not encrypted or signed by Kaeso: it holds what the app already saw and data only the issuing provider can verify.

Requests from other dialects with an effort other than `none` ask OpenAI Responses for `include: ["reasoning.encrypted_content"]`; without an effort Kaeso can't tell that the model reasons, and OpenAI refuses encrypted reasoning for models that don't. Since these requests carry `store: false` ([Responses storage and ids](#responses-storage-and-ids)), reasoning without encrypted content is not sent to OpenAI, which could not resolve its item id.

### Reasoning kept by the gateway

Chat Completions has no field for reasoning, and KPP does not invent one. The reply leaves the reasoning out and the gateway keeps it, keyed by the tool calls it came before; tool call ids are unique, issued by the provider and sent back by every Chat client, so they key the reasoning without changing the app's format.

1. After a reply, `continuity::reasoning_for_tool_calls` (a complete reply) or a `continuity::ReasoningCollector` (a stream) returns one `CarriedReasoning` per tool call that replayable reasoning came before: Anthropic signed or redacted thinking, OpenAI encrypted reasoning. Reasoning after the last tool call is not kept.
2. The gateway stores the entries in Valkey, encrypted under the master key, readable only with the API key that made the request, for `KAESO_CONTINUITY_REASONING_TTL` (an hour by default).
3. On the app's next request, after decoding and before `check` and encoding, `continuity::tool_calls_missing_reasoning` lists the tool call ids to look up, and `continuity::restore_reasoning` puts what was found back into the assistant turns: reasoning for a turn's first tool call at the start of the turn, for a later call right after the call before it. Turns that already hold reasoning are left alone.

The encoders replay restored blocks to their own provider only.

## Keepalives

A provider can stream for minutes without producing anything the app's format can show: Claude's thinking for an OpenAI Chat app, Anthropic `ping` events, another dialect's unknown blocks, or an OpenAI reasoning model that sends nothing while it reasons. Proxies with idle timeouts (nginx and AWS ALB after 60 seconds, Cloudflare after 100) would cut the connection, and the time to first byte would equal the whole thinking time. So a live stream never goes silent because of translation: every stream encoder has a keepalive frame that is valid in its dialect and ignored by the official SDKs.

- **OpenAI Chat Completions and Responses:** the SSE comment `: keepalive` followed by a blank line. Lines starting with a colon are comments in the SSE specification, and the OpenAI SDK parsers skip them.
- **Anthropic Messages:** `event: ping` with `data: {"type": "ping"}`, exactly as Anthropic sends it; the Anthropic SDKs skip ping events.

Frames reach the wire two ways:

- **In band.** An encoder adds the frame to what `StreamEncoder::encode` returns when an event from another dialect produced no bytes. A pacer limits this without a clock (`kaeso_codecs::keepalive`): another dialect's extra event, such as `ping`, which the provider already paces, always earns a frame; hidden text (thinking the app can't see, buffered tool arguments) earns one per `keepalive::HIDDEN_TEXT_PER_KEEPALIVE` bytes, 1,024; anything the encoder writes resets the count. Events of the app's own dialect never earn frames, so a same-dialect replay stays exact.
- **Driven by the gateway.** `StreamEncoder::keepalive` returns the frame on demand, for upstream silence the encoder can't see. The gateway writes it when it has written nothing to the app for `KAESO_STREAMS_KEEPALIVE_INTERVAL`, 15 seconds by default.

The OpenAI Chat encoder also sends its role chunk as soon as the response starts, rather than with the first text, so the app hears from the stream while the model thinks.

## Structured output

Structured output going to Anthropic uses Anthropic's native structured outputs, never an emulation through a forced tool. The output format `json_object` has no Anthropic equivalent and is `Unsupported`. Anthropic's output format is a schema alone: a schema's `name` is dropped, while a `description` or `strict: false` are `Unsupported`. Schemas going to OpenAI Chat from Anthropic get the name `response`, because Chat requires one.

## Sampling

Sampling settings a target can't honour are `Unsupported`, never dropped or rescaled:

| Target | Refused |
| --- | --- |
| Anthropic | `n` above 1, `logprobs`, `logit_bias`, frequency and presence penalties, `seed`, and a `temperature` above 1, since Anthropic's range is 0 to 1 |
| OpenAI Chat Completions | `top_k` |
| OpenAI Responses | `top_k`, `seed`, the penalties and stop sequences, which Responses doesn't have |

## Stateful Responses features

`previous_response_id`, `conversation`, `store: true` and `background` need state at OpenAI, and the codec refuses them with `Unsupported`; the gateway, not the codec, would have to resolve them before KPP.

## Responses storage and ids

- Requests from other dialects encoded for OpenAI Responses carry `"store": false`. Kaeso is stateless, and without it OpenAI would store every translated conversation. Requests decoded from Responses keep the app's spelling, so an absent `store` still means OpenAI stores the response.
- Responses output items need ids, which replies from other dialects don't have. Kaeso gives them ids starting with `msg_kaeso_` and `fc_kaeso_`; when an app sends them back, the request decoder removes them, so OpenAI never receives an id it did not issue.

## Cache hints

Cache hints are hints. OpenAI caches by itself, so hints from other dialects are dropped for OpenAI. OpenAI's own `prompt_cache_breakpoint` decodes to a KPP `CacheHint` and is written back only to the dialect it came from.

## Tool errors

`ToolResult.is_error` is a hint for targets without an error flag, OpenAI Chat and Responses: it is dropped and the result's content carries the error text. Refusing it would make every failed tool call of an Anthropic-format client such as Claude Code unroutable to OpenAI.

## Files

A provider file id (`file-…`) is usable within that provider's file store, and the two OpenAI generation dialects share one, so `cross::check_foreign` takes the target's `Origin`, its dialect and provider. File ids crossing to another provider are `Unsupported`.

## The Anthropic version

The Anthropic codec pins one API version, `2023-06-01`, exported as `kaeso_codecs::anthropic_messages::ANTHROPIC_VERSION`. It is the only version Anthropic's [versioning page](https://platform.claude.com/docs/en/api/versioning) lists, the one the official SDKs send, and the one every Anthropic fixture assumes.

- **Egress.** Every request the Anthropic codec encodes carries `anthropic-version: 2023-06-01`, unless the app's own Anthropic request forwarded an `anthropic-version` naming a newer version: a well-formed `YYYY-MM-DD` that sorts after the pinned one. A malformed or older value is ignored. A request from another dialect never carries the header, so it always gets the pinned version.
- **Ingress.** A forwarded `anthropic-version` equal to the pinned version is not stored in the request's extras: the codec sends it anyway, and keeping it would bind every Anthropic SDK request to its dialect. Any other value is kept as the extra `header:anthropic-version` and compared on egress as above.
- **Why newer wins.** Anthropic only adds to a version, with new optional inputs and new output values, and an app that asks for a newer version relies on its behaviour. The decoders tolerate unknown fields, blocks and events, so a newer version degrades to extras and unknown items rather than failing.

To change the pin, change the constant, record the fixtures again with `cargo xtask record` ([recording](fixtures.md#recording-live-responses)), and update this section.
