---
title: Format translation
description: How KPP carries requests between the OpenAI and Anthropic formats, and what it refuses.
order: 2
---

KPP, the Kaeso Provider Protocol, is one canonical format for model requests, with a codec for each API format that translates to and from it. The gateway uses it for every request, so an app in any of these formats can call a model of either provider.

| Format | Endpoints | Streaming |
| --- | --- | --- |
| Anthropic Messages | `/v1/messages`, `/v1/messages/count_tokens` | yes |
| OpenAI Chat Completions | `/v1/chat/completions` | yes |
| OpenAI Responses | `/v1/responses`, `/v1/responses/input_tokens` | yes |
| OpenAI Embeddings | `/v1/embeddings` | no |

`GET /v1/models` and `GET /v1/models/{id}` list the project's models in the Anthropic shape when the request carries `anthropic-version`, and in the OpenAI shape otherwise. Embeddings go to OpenAI embedding models only.

## Three rules

1. **Same format, no loss.** A request that goes to a provider in the format it came in arrives as it was sent. Only the model name changes to the provider's model id, and a stream always asks for usage, which the app sees only if it asked.
2. **Refuse, never drop.** When a request crosses formats, a field the target can't honor is refused with an error that names it. Only hints that don't change what the model writes are left out.
3. **Signed reasoning goes home only.** Encrypted or signed reasoning is only ever sent back to the format and provider that issued it.

## What carries across

- Conversations: system instructions, user and assistant turns, text, images and documents. A system message in the middle of an Anthropic conversation reaches OpenAI as a later system message.
- Tools: function tools, tool choice, parallel tool calls, tool calls and tool results.
- Tool names and call ids the target doesn't accept are rewritten in a fixed way and mapped back in the reply: other characters become `_`, the name is cut to fit, and a hash of the original is added. OpenAI accepts names of up to 64 letters, digits, `_` and `-`; Anthropic up to 128.
- Structured output: JSON schemas.
- Sampling: temperature, top P, maximum tokens and stop sequences, where the target has them. A value stays the same but is written as the target format writes it, so `1` can arrive as `1.0`.
- The end user's id: Anthropic `metadata.user_id`, OpenAI `safety_identifier`.
- Reasoning: efforts and thinking budgets are converted into each other.
- Streams: translated event by event into the app's format.

## What is refused

| Going to | Refused, with an error |
| --- | --- |
| Anthropic | `seed`, frequency and presence penalties, a temperature above 1, the `json_object` output format, a JSON schema with a description or with `strict: false`, system messages in the middle of a conversation from another format, refusals in the history, images other than JPEG, PNG, GIF and WebP, documents other than PDF and plain text, the `minimal` effort |
| OpenAI Chat Completions | `top_k`, more than four stop sequences, text documents, document URLs, document titles, context and citations, image file ids, images and documents outside user messages |
| OpenAI Responses | `top_k`, `seed`, penalties, stop sequences, text documents, document titles, context and citations |
| Any provider | More than one choice (`n`); Chat Completions audio input (`input_audio`); stored state: `store: true`, `previous_response_id`, `conversation`, `background` |
| Another format | Fields of the app's format that KPP has no place for, such as Chat Completions' `logprobs`, `logit_bias` and `service_tier`; Chat Completions' deprecated function calling (`functions`, `function_call`, `function` messages); another format's built-in tools, such as web search, and their results; citations; file ids from another provider |

Hints that don't change what the model writes are left out when the target has no place for them: cache hints, beta and version headers, image detail, file names, prompt cache keys, Anthropic's `thinking.display`, Anthropic context management that only clears earlier thinking, and the error flag of a tool result, which OpenAI doesn't have; the result's text still says what failed. Requests translated into Responses carry `store: false`, so OpenAI keeps nothing.

## Reasoning

| From the app | To | Becomes |
| --- | --- | --- |
| A thinking budget | OpenAI | An effort: below 4,096 tokens `low`, 4,096 to 16,383 `medium`, from 16,384 `high` |
| Thinking turned off | OpenAI | The effort `none` |
| An effort from `low` to `max` | Claude with adaptive thinking | Adaptive thinking with that effort |
| An effort from `low` to `max` | Claude without adaptive thinking | A budget: `low` 2,048, `medium` 8,192, `high`, `xhigh` and `max` 24,576 tokens |
| The effort `none` | Claude | No thinking |

- **Effort and budget.** An effort in the same request wins over a budget. A Claude model counts as having no adaptive thinking when its catalogue entry says so; Claude 4.5 and earlier have none. A budget is fitted to Anthropic's rules: at least 1,024 tokens and less than `max_tokens`. When `max_tokens` is 1,024 or less, the budget is 1,024 and `max_tokens` is raised by 1,024, so the answer keeps its room.
- **Thinking and sampling.** When Kaeso turns Claude's thinking on for a request from another format, it leaves out `temperature`, `top_p` and `top_k`, which Anthropic refuses together with thinking. A forced tool call (`required` or a named tool) goes without budget thinking, which Anthropic refuses with a forced tool; adaptive thinking keeps the forced tool. An Anthropic app's own settings are sent as received.
- **Tool turns from elsewhere.** When budget thinking is on and the last assistant turn's tool calls hold no thinking Claude can replay, for example because another provider made them, that request goes without thinking. Adaptive thinking stays on.

## Reasoning between turns

Reasoning from another provider comes back to the app in the app's own format, with the original wrapped inside, so the app can send it back. A Responses app gets a reasoning item that holds the original in `encrypted_content`; an Anthropic app gets a `thinking` block that holds it in `signature`. When the app sends it back, KPP restores the original for the provider that wrote it. Translated requests with an effort other than `none` ask OpenAI for its encrypted reasoning, so it can come back too.

Chat Completions has no field for reasoning. When a Chat Completions app is served by Claude, or by an OpenAI model through Responses, the gateway keeps the reasoning that came before each tool call and puts it back when the app sends the tool result. The model keeps its train of thought through tool loops.

- It lives in Valkey for `KAESO_CONTINUITY_REASONING_TTL` (1 hour), encrypted under the master key and readable only with the API key that made the request. It is never logged. `0s` turns it off ([reasoning continuity](../self-hosting/configuration.md#reasoning-continuity)).
- Reasoning after the last tool call is not kept, nor reasoning from a stream that failed.
- When the reasoning is gone (expired, another key, Valkey unreachable), the request goes on without it. `kaeso_reasoning_continuity_total` counts reasoning kept, restored, missing and unavailable ([monitoring](../self-hosting/monitoring.md)).

Readable reasoning follows what the app asks for in its own format:

- A Responses app that sets `reasoning.summary` gets Claude's thinking as reasoning summaries, streamed as `response.reasoning_summary_text.delta` events. The gateway asks Claude for summarized thinking, which Claude Opus 4.7 and later otherwise hide.
- An Anthropic app with thinking on gets an OpenAI model's reasoning summaries as thinking text. The gateway asks OpenAI for them unless the app set `thinking.display` to `omitted`. OpenAI may refuse summaries to organizations it has not verified; its error is passed on.
- A Chat Completions app sees no reasoning.

The catalogue says whether a model reasons (models.dev `reasoning`). For a model that reasons, `reasoning: {summary: "auto"}` without an effort turns Claude's thinking on at its default depth: adaptive thinking, or the 8,192 token budget. An Anthropic app's thinking without an effort brings back OpenAI's encrypted reasoning for the next turn. Reasoning is never turned on for a model that doesn't reason or isn't known to, and a request that asks for an effort, thinking or a budget on such a model is refused.
