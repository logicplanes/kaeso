---
title: Reasoning across formats
nav: Reasoning
description: How the codecs carry reasoning, its settings and its signatures between the API formats.
order: 3
---

Anthropic signs its thinking blocks and OpenAI encrypts its reasoning items. Opaque reasoning (signatures, encrypted content, item ids) is only ever replayed to the dialect and provider that issued it.

## Which reasoning is replayed

- **In requests,** readable reasoning text and summaries going to another dialect are dropped as a hint; the [reasoning blob](#reasoning-blobs) next to them restores the original block when the conversation returns to its provider.
- **Without a signature,** an Anthropic `thinking` block read from Anthropic Messages goes back to Anthropic as received, in requests and replies: an absent `signature` stays absent and `signature: ""` stays `""`. Unsigned Anthropic reasoning that wasn't read from Anthropic Messages, such as a block built by code, has nothing to replay and is left out.
- **In replies,** reasoning of another dialect is shown in the app's own reasoning shape where it has one, with the original block in a reasoning blob, so it reaches its origin again on the next turn. OpenAI Chat apps have no such shape, so the gateway keeps their reasoning between turns.

## Effort and budget

Reasoning settings convert between an effort and a token budget:

| From | To | Becomes |
| --- | --- | --- |
| Effort `none` | Anthropic | No `thinking`, Anthropic's default; models that think without it still do |
| Effort `low` to `max` | Anthropic model with adaptive thinking | Adaptive thinking with that effort |
| Effort `low` to `max` | Anthropic model without adaptive thinking | `enabled` thinking with a budget from the table below |
| A budget alone | Anthropic | `enabled` thinking with that budget |
| An Anthropic budget | OpenAI | An effort from the table below |
| Anthropic `thinking: {type: "disabled"}` | OpenAI | Effort `none` |

Both OpenAI dialects document `none` as the lowest value of their shared `ReasoningEffort` enum, so disabled Anthropic thinking always has an equivalent. The effort `minimal` has no row and stays `Unsupported` for Anthropic.

The conversion lives in `kaeso_codecs::cross::reasoning_map`.

| Anthropic `budget_tokens` | OpenAI effort |
| --- | --- |
| below 4,096 | `low` |
| 4,096 to 16,383 | `medium` |
| 16,384 and more | `high` |

An explicit effort in the same request wins over the budget. When the budget becomes an effort the model does not list among its effort levels, the gateway refuses the request before sending it, naming `thinking.budget_tokens` and the nearest budget that maps to one of the model's levels, or asking to turn thinking off when none does: `mistral-small-latest` takes only `none` and `high`, so it needs a `budget_tokens` of 16,384 or more.

| OpenAI effort | Anthropic `budget_tokens` |
| --- | --- |
| `low` | 2,048 |
| `medium` | 8,192 |
| `high`, `xhigh`, `max` | 24,576 |

This table applies only when the target model has no adaptive thinking (Claude 4.5 and earlier). The gateway says so with `EgressCtx::adaptive_thinking = Some(false)`, from the catalogue's `adaptive_thinking` field; Claude 4.7 and later refuse `enabled` thinking and get adaptive thinking with the effort instead.

The budget is then fitted to Anthropic's rules: at least 1,024 and less than `max_tokens`, so it becomes `max_tokens − 1` when that is smaller. When `max_tokens` is 1,024 or less, no valid budget exists, so `max_tokens` is raised by 1,024 and the budget is 1,024; the app keeps its own `max_tokens` for the answer. `reasoning_map::budget_within` does the fitting and raising, `reasoning_map::fit_budget` only the fitting.

The first source is OpenAI's `ReasoningEffort` enum in the OpenAPI spec: `none`, `minimal`, `low`, `medium`, `high`, `xhigh` and `max`, though not every model supports every value. The second is Anthropic's [extended thinking](https://platform.claude.com/docs/en/build-with-claude/extended-thinking) page. It says that `budget_tokens` is at least 1,024 and less than `max_tokens`, and that adaptive thinking doesn't exist on Claude Sonnet 4.5, Opus 4.5, Haiku 4.5 and earlier Claude 4 models.

## Readable reasoning and summaries

`reasoning.summary` and `reasoning.include_encrypted` are hints across dialects: they only control which reasoning comes back, so they are dropped where the target has no equivalent. Codex sends both on every request.

OpenAI returns readable reasoning only when the request sets `reasoning.summary`, and Claude Opus 4.7 and later return thinking blocks with empty text unless the request sets `thinking.display: "summarized"`. So, except for token counting:

- a request from another dialect with a reasoning summary (`auto`, `concise`, `detailed`) and thinking on goes to Anthropic with `display: "summarized"`;
- an Anthropic Messages request with thinking on (`enabled`, `adaptive`, or an effort other than `none`) and a `display` other than `omitted` goes to OpenAI Responses with `summary: "auto"`.

The gateway also tells the encoders whether the target model reasons (`EgressCtx::reasoning`, from the catalogue's `reasoning`). With `Some(true)`:

- a summary without an effort, mode or budget (OpenAI `reasoning: {summary: "auto"}`) turns Anthropic thinking on at the model's default depth: adaptive without an effort, or the `medium` budget, OpenAI's default effort;
- Anthropic `adaptive` thinking without an effort counts as reasoning for OpenAI Responses: it asks for `include: ["reasoning.encrypted_content"]` and, as above, a summary.

Requests without reasoning settings, the effort `none`, and models that don't reason or aren't known never get reasoning switched on. OpenAI Chat apps, whose format shows no reasoning, ask for no summaries.

Providers that serve Chat Completions return readable reasoning in a field of their own, such as DeepSeek's `reasoning_content` or Groq's `reasoning`, without being asked for a summary. Their chat profile names the field (`reasoning_fields`, [provider profiles](openai-chat.md#provider-profiles)), so the reasoning becomes a KPP reasoning block: Anthropic Messages apps get it as a `thinking` block and Responses apps as a reasoning item with a summary, each with the original block in a [reasoning blob](#reasoning-blobs).

## Thinking and sampling

When Kaeso turns thinking on for a request from another dialect, what Anthropic forbids next to thinking is adjusted instead of refused, because sampling settings and forced tools are routine in OpenAI apps:

- `temperature`, `top_p` and `top_k` are left out. Anthropic refuses `temperature` and `top_k` with thinking and limits `top_p` to 0.95 to 1, and Claude Opus 4.7 and later refuse any value that isn't the default.
- `enabled` thinking is not sent with a forced tool (`tool_choice` `required` or a named tool), a combination Anthropic refuses. Where Anthropic accepts forced tools, with adaptive thinking, the reply starts with the tool call and holds no thinking anyway, so nothing is lost. A forced tool with adaptive thinking is sent unchanged.

An Anthropic app's own settings are sent as received.

## Tool turns without thinking

Anthropic refuses a request with `enabled` thinking whose last assistant turn made tool calls without a replayable thinking block first, signed or redacted ("a final assistant message must start with a thinking block"). Such a request goes to Anthropic without `thinking` for that turn. Adaptive thinking is kept, because adaptive models don't need the leading thinking block.

Since reasoning travels with the conversation, this is rare. It still happens in three cases:

- The turn really came from another provider, after a model switch in the middle of a tool loop.
- The app dropped the reasoning item, thinking block or blob.
- The gateway no longer holds a Chat app's reasoning, because it expired or another key sent the turn.

## Reasoning continuity

Without help, every tool-loop turn of an app on another provider's model would lose that model's reasoning: budget thinking would be switched off on every continuation turn, and adaptive thinking would lose what it reasoned before. `kaeso_codecs::continuity` makes reasoning travel with the conversation, in a blob for Responses and Anthropic apps and in the gateway for Chat apps.

### Reasoning blobs

For OpenAI Responses and Anthropic Messages apps, KPP wraps the whole reasoning block of another dialect into a reasoning blob. That covers its readable text, summaries, signature, encrypted content, redacted data and item id. The blob is `continuity::REASONING_BLOB_PREFIX` (`kpp:reasoning:v1:`) followed by the block's KPP JSON in unpadded base64url, with the form markers removed.

| App | Gets | Carrying the blob in |
| --- | --- | --- |
| OpenAI Responses | A reasoning item with an `rs_kaeso_…` id and the thinking as one summary text, streamed as `response.reasoning_summary_*` events | `encrypted_content` |
| Anthropic Messages | A `thinking` block with the summaries and reasoning text as thinking, paragraphs separated by a blank line | `signature`, streamed as one `signature_delta` when the block stops |

Both request decoders recognise a blob wherever it sits (`continuity::unwrap_reasoning`) and restore the original block with its origin, so the encoders replay it to its own dialect and provider like any other reasoning.

- **Always sent.** The blob is sent whether or not the app asked for `include: ["reasoning.encrypted_content"]`, and whatever its `store` setting. Kaeso stores nothing, and the SDK agent loops send output items and content blocks back as received. Those loops include openai-agents' `to_input_list()`, Codex, Claude Code and the Anthropic tool runner.
- **The blob wins.** It holds the readable text a second time, because it must restore the block exactly (Anthropic checks thinking text against its signature) while apps may edit or drop the readable copy. When an app sends both back, the blob wins.
- **Never mistaken for provider data.** The prefix contains `:`, which is in neither the base64 alphabet of Anthropic signatures nor the base64url alphabet of OpenAI's encrypted content. A value with the prefix that doesn't decode is kept as the provider's own opaque data, and the provider then refuses it, so nothing is dropped silently.
- **Not encrypted or signed by Kaeso.** It holds what the app already saw, and data only the issuing provider can verify.

Requests from other dialects ask OpenAI Responses for `include: ["reasoning.encrypted_content"]` when they set an effort other than `none` or a thinking budget, or turn thinking on for a model the gateway knows reasons ([readable reasoning and summaries](#readable-reasoning-and-summaries)). Otherwise Kaeso can't tell that the model reasons, and OpenAI refuses encrypted reasoning for models that don't. Since these requests carry `store: false` ([Responses storage and ids](cross-format.md#responses-storage-and-ids)), reasoning without encrypted content is not sent to OpenAI, which couldn't resolve its item id.

### Reasoning kept by the gateway

Chat Completions has no field for reasoning, and KPP does not invent one. The reply leaves the reasoning out and the gateway keeps it, keyed by the tool calls it came before. Tool call ids are unique, issued by the provider and sent back by every Chat client, so they key the reasoning without changing the app's format.

1. After a reply, `continuity::reasoning_for_tool_calls` (a complete reply) or a `continuity::ReasoningCollector` (a stream) returns one `CarriedReasoning` per tool call that replayable reasoning came before: Anthropic signed or redacted thinking, or OpenAI encrypted reasoning. Reasoning after the last tool call is not kept.
2. The gateway stores the entries in Valkey, encrypted under the master key and readable only with the API key that made the request, for `KAESO_CONTINUITY_REASONING_TTL` (an hour by default).
3. On the app's next request, after decoding and before `check` and encoding, `continuity::tool_calls_missing_reasoning` lists the tool call ids to look up. `continuity::restore_reasoning` then puts what was found back into the assistant turns. Reasoning for a turn's first tool call goes at the start of the turn, and reasoning for a later call right after the call before it. Turns that already hold reasoning are left alone.

The encoders replay restored blocks to their own provider only.
