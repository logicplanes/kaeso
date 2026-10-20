# `fixtures/openai_chat/providers/deepseek`

DeepSeek, provider id `deepseek`, over its OpenAI-compatible Chat Completions API at `https://api.deepseek.com/v1/chat/completions`, with `authorization: Bearer`. DeepSeek also serves Anthropic Messages at `https://api.deepseek.com/anthropic`; Kaeso doesn't call that endpoint, so nothing here comes from it.

Sources, read 2026-10-19:

- API reference: https://api-docs.deepseek.com/api/create-chat-completion, https://api-docs.deepseek.com/guides/thinking_mode/, https://api-docs.deepseek.com/quick_start/error_codes, https://api-docs.deepseek.com/quick_start/rate_limit, https://api-docs.deepseek.com/guides/reasoning_model
- No official SDK: DeepSeek points to the OpenAI SDKs.

Models used: `deepseek-flash` (DeepSeek-V4.1-Flash). The account's model listing also has `deepseek-v4-pro`. Everything below is a live recording; the request fixtures under `requests/` are written for DeepSeek, with `system` instead of `developer` messages and DeepSeek's own `thinking` field.

| file | what it covers | provenance |
|---|---|---|
| `requests/thinking-reasoning-content.json` | thinking mode on (`thinking.type: enabled`), `reasoning_effort: low` | written for DeepSeek, API reference |
| `requests/thinking-stream.json` | the same, streamed with `stream_options.include_usage` | as above |
| `requests/thinking-tools-auto.json`, `requests/thinking-tools-stream.json` | a function tool in thinking mode, `tool_choice: auto`, plain and streamed | as above |
| `requests/thinking-tool-choice-required.json` | `tool_choice: required` in thinking mode, which DeepSeek refuses | as above |
| `requests/reasoning-content-sent-back.json` | an earlier assistant turn carrying `reasoning_content`, sent back to DeepSeek | as above; the turn is the reply in `responses/recorded-thinking-reasoning-content.json` |
| `requests/tool-turn-without-reasoning-content.json`, `requests/pro-tool-turn-without-reasoning-content.json` | thinking mode with tools: the tool call from `responses/recorded-thinking-tools-auto.json` and its result sent back without the turn's `reasoning_content`, as an app in another format sends it; with `deepseek-flash` and `deepseek-v4-pro` | as above |
| `requests/prompt-cache-hit.json` | a 282-token system prompt, thinking off, sent twice so the second call hits DeepSeek's prompt cache | as above |
| `requests/max-completion-tokens-limit.json`, `responses/recorded-max-completion-tokens-limit.json` | `max_completion_tokens: 20`, which DeepSeek ignores: the reply stops on its own after 133 completion tokens (`finish_reason: stop`). The `chat` profile's `max_tokens()` sends every app's output limit as `max_tokens`, which DeepSeek documents and honours | written for DeepSeek, recorded 2026-10-20 |
| `requests/json-object-system.json` | `response_format: json_object` with a `system` message | as above |
| `requests/missing-model.json`, `requests/authentication-fails.json` | an unknown model; a made-up key | as above |
| `responses/recorded-thinking-reasoning-content.json` | `message.reasoning_content` next to `content`; `completion_tokens_details.reasoning_tokens`; `prompt_cache_hit_tokens` and `prompt_cache_miss_tokens` | recorded 2026-10-19 (see `responses/RECORDED.md`) |
| `responses/recorded-thinking-tools-auto.json` | a tool call with `reasoning_content` and `content: ""`; each `tool_calls[]` entry carries `index` | recorded 2026-10-19 |
| `responses/recorded-reasoning-content-sent-back.json` | DeepSeek answers a conversation that sends `reasoning_content` back (no error) | recorded 2026-10-19 |
| `responses/recorded-tool-turn-without-reasoning-content.json`, `responses/recorded-pro-tool-turn-without-reasoning-content.json` | both models answer the tool result; neither returns the 400 the thinking-mode guide describes for a missing `reasoning_content` | recorded 2026-10-19 |
| `responses/recorded-prompt-cache-hit.json` | the second call: `prompt_cache_hit_tokens: 128`, equal to `prompt_tokens_details.cached_tokens` | recorded 2026-10-19 |
| `responses/recorded-json-object-system.json` | JSON mode | recorded 2026-10-19 |
| `responses/recorded-stop-array.json` | the canonical `stop-array` request; thinking is on by default, so the reply has `reasoning_content` | recorded 2026-10-19 |
| `responses/recorded-logprobs-top-logprobs-doc.json` | the canonical `logprobs-top-logprobs-doc` request; `logprobs.content[]` with `bytes` and `top_logprobs` | recorded 2026-10-19 |
| `streams/recorded-thinking-stream.sse` | `delta.reasoning_content` chunks, then `delta.content`; `usage: null` on every chunk but the last | recorded 2026-10-19 (see `streams/RECORDED.md`) |
| `streams/recorded-thinking-tools-stream.sse` | reasoning chunks, then a streamed tool call; the last chunk's usage has a cache hit of 128 tokens | recorded 2026-10-19 |
| `streams/recorded-stream-include-usage.sse` | the canonical `stream-include-usage` request | recorded 2026-10-19 |
| `errors/recorded-plain-developer-string.json` | HTTP 422: DeepSeek refuses the `developer` role (`unknown variant`) | recorded 2026-10-19 (see `errors/RECORDED.md`) from the canonical `plain-developer-string` request |
| `errors/recorded-thinking-tool-choice-required.json` | HTTP 400 `Thinking mode does not support this tool_choice` | recorded 2026-10-19 |
| `errors/recorded-missing-model.json` | HTTP 400 naming the supported model names; `code: invalid_request_error` | recorded 2026-10-19 |
| `errors/recorded-authentication-fails.json` | HTTP 401 `authentication_error`, the made-up key masked by DeepSeek | recorded 2026-10-19 with a made-up key |
| `models/list-recorded.json` | `GET /models`: both models, without `created`, with DeepSeek's extra fields `name`, `context_window`, `max_output_tokens`, `input_modalities`, `output_modalities`, `effort` and `api_capabilities`, kept in the model's extras | recorded 2026-10-19 from api.deepseek.com |

No recording has DeepSeek's keep-alive lines or its HTTP 402, because neither can be asked for: keep-alives come only while the server is busy, and 402 only on an empty balance. `src/openai_chat/tests_deepseek.rs` adds the keep-alive line the rate-limit page documents, `: keep-alive`, and empty lines before a reply body to recorded fixtures, and checks a 402 by its status.

## Where DeepSeek differs from OpenAI Chat Completions

| difference | how KPP carries it | source |
|---|---|---|
| `message.reasoning_content`, `delta.reasoning_content` | extras of the reply and of the stream deltas; round-trips within Chat, left out for other formats ([replies](https://docs.kaeso.app/development/kpp/cross-format#replies)) | API reference, `reasoning_content` |
| `reasoning_content` on an assistant message sent back | extras of the message, sent again as it is; DeepSeek accepts it | `responses/recorded-reasoning-content-sent-back.json` |
| the [thinking-mode guide](https://api-docs.deepseek.com/guides/thinking_mode/) asks for `reasoning_content` on every later request with tools, or a 400 | apps in other formats can't send it back, since replies to them leave it out; the live API answers without it | `responses/recorded-tool-turn-without-reasoning-content.json`, `responses/recorded-pro-tool-turn-without-reasoning-content.json` |
| `thinking` in a request | extras of the request; `Unsupported` towards other formats | API reference, `thinking` |
| `usage.prompt_cache_hit_tokens`, `usage.prompt_cache_miss_tokens` | kept in the raw usage; the typed cache read comes from `prompt_tokens_details.cached_tokens`, which DeepSeek sets to the same value | API reference, `usage` |
| `tool_calls[].index` in a reply that doesn't stream | extras of the tool call | `responses/recorded-thinking-tools-auto.json` |
| no `developer` role | the `chat` profile's `system_role()` sends a Chat app's `developer` messages as `system`, as Kaeso writes them for other formats | `errors/recorded-plain-developer-string.json` |
| `: keep-alive` comments in streams, empty lines before a body that doesn't stream | stream extras that round-trip; whitespace JSON ignores | rate-limit page |
| HTTP 402 for an exhausted balance | `Billing` | error codes page |
| `finish_reason` `insufficient_system_resource`, `aborted` | `StopReason::Other` | API reference, `finish_reason` |
