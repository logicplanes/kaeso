# openai_chat / responses

Non-streaming 200 bodies

| file | what it covers | provenance |
|---|---|---|
| `audio-output.json` | assistant `audio` object (`id`, `expires_at`, `data`, `transcript`) with `content: null`; `audio_tokens` in usage | schema: ChatCompletionResponseMessage.audio |
| `content-filter.json` | `finish_reason: content_filter` with `content: null` | schema: CreateChatCompletionResponse.choices[].finish_reason (shape as in sdk tests/lib/chat/test_content_filter.py@16d3d3b3c33b1b4f01114a16e4c3eba2293799ba, current field set) |
| `functions-tool-choice-auto-doc.json` | `content: null` + one `tool_calls` entry, `finish_reason: tool_calls`; message WITHOUT `refusal`/`annotations` and usage WITHOUT `prompt_tokens_details` (omitted vs null) | doc: https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create (example "Functions") |
| `image-url-doc.json` | plain text answer to an image prompt | doc: https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create (example "Image input") |
| `json-object.json` | JSON-mode output in `content` | schema: CreateChatCompletionResponse |
| `json-schema.json` | JSON-schema output; `reasoning_tokens` > 0 included in `completion_tokens` | schema: CreateChatCompletionResponse, CompletionUsage |
| `logprobs-top-logprobs-doc.json` | `choices[].logprobs.content` with `top_logprobs`, a `bytes: null` entry, `refusal: null`; message without `refusal`/`annotations` | doc: https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create (example "Logprobs") |
| `max-tokens-length.json` | truncated content, `finish_reason: length` | schema: CreateChatCompletionResponse |
| `moderation.json` | top-level `moderation` object with `input`/`output` `moderation_results` | schema: ChatCompletionModeration, ModerationResultBody |
| `n-2.json` | two `choices` (index 0 and 1); a decoder for a single-choice KPP response must reject or report it | schema: CreateChatCompletionResponse.choices |
| `plain-developer-string.json` | plain text answer; full `usage` details; `service_tier`; no `system_fingerprint` (omitted field) | doc: https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create (example "Default") |
| `prediction-doc.json` | `accepted_prediction_tokens` / `rejected_prediction_tokens` in usage | doc: https://developers.openai.com/api/docs/guides/predicted-outputs (abridged usage example; `choices`, `service_tier` and `id` filled in per schema `CreateChatCompletionResponse`) |
| `reasoning-cached-usage.json` | reasoning model: `completion_tokens_details.reasoning_tokens` (included in `completion_tokens`), `prompt_tokens_details.cached_tokens` (included in `prompt_tokens`) and `cache_write_tokens`; `service_tier: priority` | schema: CompletionUsage (field semantics per https://developers.openai.com/api/docs/guides/prompt-caching and /guides/reasoning) |
| `refusal-sdk-2024.json` | recorded 2024 refusal body (`content: null`, `refusal` string), legacy field set | sdk: tests/lib/chat/test_completions.py::test_parse_pydantic_model_refusal@16d3d3b3c33b1b4f01114a16e4c3eba2293799ba |
| `refusal.json` | `content: null`, `refusal` string, `finish_reason: stop` | schema: ChatCompletionResponseMessage.refusal |
| `tools-custom-grammar.json` | assistant `tool_calls` entry of `type: custom` (`custom.input` string) | schema: ChatCompletionMessageCustomToolCall |
| `tools-parallel-sdk-2024.json` | recorded 2024 body (not the same completion as streams/tools-parallel-sdk-2024.sse): two parallel tool calls; no `annotations`/`service_tier`; usage without `prompt_tokens_details` | sdk: tests/lib/chat/test_completions.py::test_parse_multiple_pydantic_tools@16d3d3b3c33b1b4f01114a16e4c3eba2293799ba |
| `tools-parallel.json` | two parallel tool calls (one with a `null` argument value) | schema: CreateChatCompletionResponse, ChatCompletionMessageToolCalls |
| `tools-results-parallel-error.json` | final text answer after tool results (non-ASCII `°`) | schema: CreateChatCompletionResponse |
| `tools-single.json` | `content: null`, one `tool_calls` entry, `refusal: null`, `annotations: []`, `finish_reason: tool_calls` | schema: CreateChatCompletionResponse, ChatCompletionResponseMessage, ChatCompletionMessageToolCall |
| `unknown-top-level-field.json` | MADE-UP `x_future_field` at top level and in the message, `x_future_choice_field` on the choice | schema: CreateChatCompletionResponse + invented fields (unknown-* fixture) |
| `web-search.json` | `annotations` with two `url_citation` entries whose `start_index`/`end_index` point at the inline markdown citations (Unicode code-point offsets) | schema: ChatCompletionResponseMessage.annotations (URLs are example.com/org placeholders) |
| `unknown-response-shapes.json` | MADE-UP response with a choice at `index` 1 with annotations but no content and an invented `finish_reason`, and invented usage fields | schema: CreateChatCompletionResponse + invented `x_future_*` names (unknown-* fixture) |
