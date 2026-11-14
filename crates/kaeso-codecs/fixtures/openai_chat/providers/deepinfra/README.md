# Fixtures: DeepInfra (`deepinfra`) on `openai_chat`

DeepInfra's OpenAI-compatible Chat Completions, `POST https://api.deepinfra.com/v1/chat/completions`, with `Authorization: Bearer`. DeepInfra's guides write the base URL as `https://api.deepinfra.com/v1/openai`; its OpenAPI document serves the same API at `/v1/…` under the host, which is Kaeso's base URL `https://api.deepinfra.com`. Model ids name their maker, such as `deepseek-ai/DeepSeek-V4-Flash-0731`.

No key was available. `responses/greeting-published.json` is a reply DeepInfra sent to `openai/gpt-oss-120b`, published unchanged in an issue of another project; the other replies and the stream are built from DeepInfra's OpenAPI document and the examples of its guides. The model listing was answered live without a key, as DeepInfra lists its models publicly, and cut to three of its models; the key errors were answered live without a valid key.

## Sources (read 2026-11-14)

| Source | Address |
|---|---|
| OpenAPI document (`OpenAIChatCompletionsIn`, `ChatCompletionAssistantMessage`, content parts, `StreamOptions`) | https://api.deepinfra.com/openapi.json |
| Chat Completions guide (supported parameters, service tiers, `fail_fast` and its 429) | https://docs.deepinfra.com/chat/overview.md |
| Streaming (usage in the chunk that sets `finish_reason`) | https://docs.deepinfra.com/chat/streaming.md |
| Tool calling (the tool call of `responses/tools-doc.json`) | https://docs.deepinfra.com/chat/tool-calling.md |
| Prompt caching (`prompt_cache_key`, `prompt_tokens_details.cached_tokens`) | https://docs.deepinfra.com/chat/prompt-caching.md |
| Reasoning (`reasoning_effort`, the `reasoning` object) | https://docs.deepinfra.com/chat/reasoning.md |
| Authentication (keys, scopes, the 403 of a missing scope) | https://docs.deepinfra.com/account/authentication.md |
| A reply of `openai/gpt-oss-120b` (`responses/greeting-published.json`) | https://github.com/openclaw/openclaw/issues/11533 |

## Files

| File | What it covers | Provenance |
|---|---|---|
| `requests/greeting-published.json`, `responses/greeting-published.json` | `reasoning_content` beside the answer, `name: null`, an empty `tool_calls`, `usage.estimated_cost` and `prompt_tokens_details: null` | published |
| `requests/tools-doc.json`, `responses/tools-doc.json` | one function tool and `tool_choice: "auto"`; the guide's tool call `call_X0xYqdnoUonPJpQ6HEadxLHE` with `finish_reason: "tool_calls"` | doc |
| `requests/cached-doc.json`, `responses/cached-doc.json` | `prompt_cache_key`, and the guide's usage with 4800 of 5000 input tokens cached, plus `cache_write_tokens: null` | doc |
| `requests/reasoning-stream.json`, `streams/reasoning-stream.sse` | `reasoning_effort: "low"`; `delta.reasoning_content`, then the answer, and the usage in the chunk that sets `finish_reason` | doc |
| `requests/invalid-api-key.json`, `errors/recorded-invalid-api-key.json` | 401 `invalid_api_key`, "User is not authorized to access this resource" | live, with a made-up key |
| `requests/missing-api-key.json`, `errors/recorded-missing-api-key.json` | 401 `invalid_api_key`, "missing API key" | live |
| `errors/engine-overloaded-doc.json` | 429 `engine_overloaded`, which `fail_fast` asks for when a model is busy | doc |
| `errors/missing-scope-doc.json` | 403 `{"detail": {"error": …}}` for a key without the `inference` scope, without OpenAI's `error` object | doc |
| `models/list-recorded.json` | `GET /v1/models` answered without a key, cut to three models: `root`, `parent` and DeepInfra's `metadata` (prices per million tokens, tags) | live |

## Where DeepInfra differs from OpenAI Chat Completions

| Difference | How KPP carries it | Source |
|---|---|---|
| the roles are `system`, `user`, `assistant` and `tool`; there is no `developer` | the profile sends `system` (`system_role()`) | OpenAPI document |
| `max_tokens` and no `max_completion_tokens` | the output limit goes as `max_tokens` (`max_tokens()`) | OpenAPI document |
| `user` and no `safety_identifier` | the end user's id goes as `user` (`end_user_as_user()`) | OpenAPI document |
| reasoning comes as `reasoning_content` on the message and the delta, and assistant messages take it back | a reasoning block through the `chat` profile's `reasoning_fields(["reasoning_content"])` | OpenAPI document; published reply |
| `video_url` and `audio_url` content parts | `video_url` is sent as written (`with_video_url()`); a Chat app's `audio_url` part stays an `Unknown` block, refused when the request goes to another dialect | OpenAPI document |
| the schema has no `parallel_tool_calls`, `top_logprobs`, `logit_bias`, `metadata`, `store`, `modalities`, `audio`, `prediction`, `prompt_cache_retention`, `web_search_options`, `functions` or `function_call`, and DeepInfra drops undeclared fields without a word | refused before sending (`refusing([…])`); `parallel_tool_calls: true`, which only allows parallel calls, is left out and named in `kaeso-left-out` | OpenAPI document (`additionalProperties` not set) |
| `top_k`, `min_p`, `repetition_penalty`, `stop_token_ids`, `fail_fast`, `models` (fallbacks), `prompt_cache_options`, `cache_salt`, `chat_template_kwargs`, `media_io_kwargs` and the `reasoning` object | request `Extras`; refused when the request goes to another dialect | OpenAPI document |
| `service_tier` takes `priority` (50% more) and `flex` (20% less) | sent as written; a `priority` request's cost is marked unpriced, as for OpenAI | Chat Completions guide |
| `usage.estimated_cost`, `prompt_tokens_details: null`, `cache_write_tokens`, and `name: null` on the message | extras and `raw` usage, round-trip | published reply; prompt caching guide |
| a key without the `inference` scope gets 403 with `{"detail": {"error": …}}` | `KppError` of kind `PermissionDenied` from the status | authentication guide |
| `GET /v1/models` answers without a key and lists every model kind (chat, embeddings, images, speech, video), and there is no `GET /v1/models/{model}` (404 `{"detail": "Not Found"}`) | a model check reads the whole listing, and can't spot a wrong key | live |
| the Anthropic Messages API is served at `/anthropic/v1/messages`, outside the `/v1` of the other formats | not used: no model sends as Messages and token counts are refused | OpenAPI document |
