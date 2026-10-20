# `fixtures/openai_chat/providers/groq`

Groq, provider id `groq`, over its OpenAI-compatible Chat Completions API at `https://api.groq.com/openai/v1/chat/completions`, with `authorization: Bearer`.

Sources, read 2026-10-19:

- OpenAI compatibility: https://console.groq.com/docs/openai
- API reference: https://console.groq.com/docs/api-reference (`POST /openai/v1/chat/completions`, `GET /openai/v1/models`, Responses (beta))
- Reasoning: https://console.groq.com/docs/reasoning (`reasoning_format`, `include_reasoning`, `reasoning_effort`)

Every reply, stream and error below is a live recording made with `cargo xtask record` on 2026-10-19 (see `RECORDED.md` in each folder), with `openai/gpt-oss-20b` and `qwen/qwen3.8-27b`, the cheapest chat models the test account lists. The model listing, the retrieved model and `errors/recorded-invalid-api-key.json` were recorded the same day with a plain `GET`/`POST`; the listing keeps 5 of the account's 11 models, otherwise unchanged. Groq has no embeddings endpoint, so there are no embedding fixtures.

| file | what it covers | provenance |
|---|---|---|
| `requests/reasoning-format-parsed.json` | `reasoning_format: "parsed"` with `reasoning_effort: "low"` for Qwen | reasoning docs; kept in request extras |
| `requests/reasoning-format-hidden.json` | `reasoning_format: "hidden"` | reasoning docs |
| `requests/reasoning-format-raw.json` | `reasoning_format: "raw"` | reasoning docs |
| `requests/include-reasoning-false.json` | `include_reasoning: false` for GPT-OSS | reasoning docs |
| `requests/stream-reasoning-parsed.json` | parsed reasoning, streamed with `stream_options.include_usage` | reasoning docs |
| `requests/reasoning-format-with-include-reasoning.json` | both reasoning switches at once, which Groq refuses | reasoning docs |
| `requests/tools-system-role.json` | `tools-single` with a `system` message and `reasoning_effort: "low"`, which GPT-OSS accepts | `requests/tools-single.json` |
| `requests/stream-tools-system-role.json` | the same, streamed with `include_usage` | as above |
| `requests/model-not-found.json` | a model the account cannot use | — |
| `requests/n-above-one.json` | `n: 2`; KPP refuses it before sending (`Unsupported` `n`), Groq answers 400 | api-reference `n` |
| `responses/recorded-plain-developer-string.json` | text reply with `message.reasoning`, timing fields in `usage`, `usage_breakdown`, `x_groq`, `service_tier` | live |
| `responses/recorded-json-schema.json` | Structured Outputs reply | live |
| `responses/recorded-tools-system-role.json` | tool call with `reasoning` and no `content` | live |
| `responses/recorded-include-reasoning-false.json` | GPT-OSS reply without `reasoning`, `reasoning_tokens` still counted | live |
| `responses/recorded-reasoning-format-parsed.json` | Qwen `reasoning` field | live |
| `responses/recorded-reasoning-format-hidden.json` | no reasoning text, `reasoning_tokens` still counted | live |
| `responses/recorded-reasoning-format-raw.json` | reasoning inside `<think>` in `content` | live |
| `streams/recorded-stream-doc.sse` | `delta.reasoning` with `delta.channel: "analysis"`; `x_groq` on the first and last chunk; `usage` on the finish chunk although `include_usage` was not asked for | live |
| `streams/recorded-stream-include-usage.sse` | finish chunk with `x_groq.usage` and `usage`, then a usage chunk with empty `choices` and `service_tier` | live, `qwen/qwen3.8-27b` |
| `streams/recorded-stream-reasoning-parsed.sse` | Qwen `delta.reasoning` without `channel` | live |
| `streams/recorded-stream-tools-system-role.sse` | a whole tool call in one delta | live |
| `errors/recorded-logit-bias.json` | 400 for `logit_bias`, with `param` | live |
| `errors/recorded-logprobs-top-logprobs-doc.json` | 400 for `logprobs`, with `param` | live, `qwen/qwen3.8-27b` |
| `errors/recorded-n-above-one.json` | 400 for `n: 2`, without `param` | live |
| `errors/recorded-reasoning-format-with-include-reasoning.json` | 400 for both reasoning switches | live |
| `errors/recorded-tools-single.json` | 400 from Qwen's chat template for a `developer` message | live, `qwen/qwen3.8-27b` |
| `errors/recorded-tools-results-parallel-error.json` | 400 for `refusal` on an assistant message | live, `qwen/qwen3.8-27b` |
| `errors/recorded-model-not-found.json` | 404 `model_not_found` | live |
| `errors/recorded-invalid-api-key.json` | 401 `invalid_api_key` | live `GET /openai/v1/models` with a made-up key |
| `models/list-recorded.json` | `GET /openai/v1/models` with Groq's extra model fields | live |
| `models/retrieve-recorded.json` | `GET /openai/v1/models/openai/gpt-oss-20b`, a model id with `/` in the path | live |

## Where Groq differs from OpenAI Chat Completions

| difference | how KPP carries it | source |
|---|---|---|
| `message.reasoning` and `delta.reasoning` (GPT-OSS by default, Qwen with `reasoning_format: "parsed"`); `delta.channel: "analysis"` on GPT-OSS reasoning chunks | extras; round-trip in the same format; not carried into Anthropic Messages or Responses until reasoning gets its own cross-format design | reasoning docs; live |
| `reasoning_format` (`parsed`, `raw`, `hidden`) and `include_reasoning` | request extras for Groq; refused when the target speaks another format | api-reference |
| `reasoning_effort` values differ per model (`none` and `default` only for Qwen, `low`/`medium`/`high` for GPT-OSS) | the gateway refuses a value outside the model's models.dev `reasoning_options` before sending; Groq answers 400 for any other value the model does not take | api-reference; live |
| `usage.queue_time`, `prompt_time`, `completion_time`, `total_time`; top-level `usage_breakdown`, `x_groq` (`id`, `seed`), `service_tier: "on_demand"` | extras, round-trip | live; api-reference example |
| streams: `x_groq` on the first chunk, `x_groq.usage` and `usage` on the finish chunk even without `include_usage`; with `include_usage` also a usage chunk with empty `choices` | extras; the typed usage comes from the chunk's `usage` | live |
| `logprobs`, `top_logprobs`, `logit_bias`, `messages[].name`: 400; `n` above 1: 400 | refused before sending by the `chat` profile (`refusing`, `refusing_in_messages`); `n` above 1 is refused by KPP before sending | openai compatibility page; live |
| Qwen's chat template refuses `developer` messages; `refusal` on assistant messages is refused | the `chat` profile sends `developer` messages as `system` (`system_role()`) and refuses `refusal` before sending | live |
| extra model fields (`active`, `context_window`, `max_completion_tokens`, `pricing`, `supported_features`, …) | extras of the model, round-trip | live |
| no embeddings and no token-count endpoint | `embeddings` and `count_tokens` are `None` | api-reference |
| Responses (beta) answers, but refuses `previous_response_id` and has no `/v1/responses/input_tokens` | not in `dialects` | live probe |
| rate limits as `x-ratelimit-{limit,remaining,reset}-{requests,tokens}` headers | not read | live |
