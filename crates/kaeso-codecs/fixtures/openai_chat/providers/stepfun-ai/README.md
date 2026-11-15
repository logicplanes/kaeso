# Fixtures: StepFun (`stepfun-ai`) on `openai_chat`

StepFun's OpenAI-compatible Chat Completions, `POST https://api.stepfun.ai/v1/chat/completions`, with `Authorization: Bearer`. Kaeso's base URL is `https://api.stepfun.ai`. The China service answers the same paths at `https://api.stepfun.com` with keys of its own; KPMR lists it as `stepfun`, and Kaeso reaches it through this provider with that base URL.

No key was available. The replies, the text stream and the model listing are the examples of StepFun's reference and guides, unchanged; the replies and the stream with reasoning are built from the reference, which documents the fields but shows no reply with them. The key errors were answered live without a valid key; their `acw_tc` cookie and request headers aren't part of the fixtures.

## Sources (read 2026-11-14)

| Source | Address |
|---|---|
| Chat Completions reference (request fields, `reasoning`, `reasoning_format`, `usage.cached_tokens`, the examples of every tab) | https://platform.stepfun.ai/docs/en/api-reference/chat/chat-completion-create |
| Chat Completions reference of the China service (`reasoning_content` beside `reasoning`, `prompt_tokens_details`, `completion_tokens_details`) | https://platform.stepfun.com/docs/zh/api-reference/chat/chat-completion-create |
| Tool call guide (`requests/tools-doc.json`, `responses/tools-doc.json`) | https://platform.stepfun.ai/docs/en/guides/developer/tool-call |
| Prompt cache guide (`responses/cached-doc.json`) | https://platform.stepfun.ai/docs/en/guides/developer/prompt-cache |
| JSON mode guide (`requests/json-schema-doc.json`) | https://platform.stepfun.ai/docs/en/guides/developer/json-mode |
| Video guide (`requests/video-url-doc.json`) | https://platform.stepfun.ai/docs/en/guides/developer/video-chat |
| Reasoning guide (`reasoning` in replies and deltas) | https://platform.stepfun.ai/docs/en/guides/developer/reasoning |
| Models list and retrieve (`models/list-doc.json`, `models/retrieve-doc.json`) | https://platform.stepfun.ai/docs/en/api-reference/models/list, https://platform.stepfun.ai/docs/en/api-reference/models/retrieve |
| Error codes | https://platform.stepfun.ai/docs/en/api-reference/error-codes, https://platform.stepfun.com/docs/zh/api-reference/error-codes |

## Files

| File | What it covers | Provenance |
|---|---|---|
| `requests/text-chat-doc.json`, `responses/text-chat-doc.json` | the "Text Chat" tab: a system and a user message; a reply with a UUID id and no `system_fingerprint` | doc |
| `requests/image-url-doc.json`, `responses/image-url-doc.json` | the "Image Understanding" tab: an `image_url` part | doc |
| `requests/video-url-doc.json` | a `video_url` part before the instruction | doc |
| `requests/reasoning-effort-doc.json` | the "Reasoning Effort" tab: `reasoning_effort: "medium"` and `max_tokens` | doc |
| `responses/reasoning-effort-schema.json` | `message.reasoning` beside the answer | built from the schema |
| `requests/deepseek-style.json` | `reasoning_format: "deepseek-style"`, StepFun's own field | built from the schema |
| `responses/both-reasoning-fields-schema.json` | `reasoning` and `reasoning_content` with the same text, and `prompt_tokens_details` and `completion_tokens_details`, as the China service's reference describes them | built from the schema |
| `requests/json-schema-doc.json` | `response_format` of type `json_schema` with `strict` | doc |
| `requests/tools-doc.json`, `responses/tools-doc.json` | `tool_choice: "auto"` and one function; a reply with `content: ""`, the tool call, `finish_reason: "stop"` and `usage.cached_tokens` | doc |
| `responses/cached-doc.json` | `usage.cached_tokens: 512` beside `prompt_tokens: 591`, which includes them | doc |
| `requests/reasoning-stream.json`, `streams/reasoning-stream-schema.sse` | `delta.reasoning`, then the answer, with `role: ""`, `finish_reason: ""` and a running usage on every chunk, and a first chunk with `created: 0` | built from the schema and `streams/text-doc.sse` |
| `streams/text-doc.sse` | the "Streaming Response" tab; the page prints the events without blank lines between some of them, which are added here | doc |
| `requests/invalid-api-key.json`, `errors/recorded-invalid-api-key.json` | 401 `{"error": {"message": "Incorrect API key provided", "type": "invalid_api_key"}}` | live, with a made-up key |
| `requests/missing-api-key.json`, `errors/recorded-missing-api-key.json` | the same 401 without a key | live |
| `models/list-doc.json`, `models/retrieve-doc.json` | `GET /v1/models` with a chat and a speech model, `owned_by: "stepai"`; `GET /v1/models/{model}` | doc |

## Where StepFun differs from OpenAI Chat Completions

| Difference | How KPP carries it | Source |
|---|---|---|
| the roles are `system`, `user`, `assistant` and `tool`; there is no `developer` | the profile sends `system` (`system_role()`) | reference |
| `max_tokens` and no `max_completion_tokens` | the output limit goes as `max_tokens` (`max_tokens()`) | reference |
| no `user`, `safety_identifier` or `prompt_cache_key`; caching is automatic | the end user and a Responses app's cache key are left out (`without_end_user()`, `without_cache_key()`); a Chat app's `prompt_cache_key` is refused | reference; prompt cache guide |
| no `stream_options`; every chunk carries the running usage | not sent (`without_stream_options()`); the app's `include_usage` decides whether it sees usage | reference |
| `video_url` parts | sent as written (`with_video_url()`) | reference; video guide |
| reasoning comes as `reasoning`, or as `reasoning_content` with `reasoning_format: "deepseek-style"`; the China service's reference sends both with the same text | a reasoning block through `reasoning_fields(["reasoning", "reasoning_content"])`; the second field stays an extra | reference; China service's reference |
| `usage.cached_tokens` beside the counts instead of `prompt_tokens_details.cached_tokens` | read as cache reads when the details don't name them | reference; prompt cache guide |
| the reference leaves out `presence_penalty`, `seed`, `logprobs`, `top_logprobs`, `logit_bias`, `parallel_tool_calls`, `prompt_cache_retention`, `store`, `metadata`, `service_tier`, `modalities`, `audio`, `prediction`, `web_search_options`, `functions`, `function_call` and `name` on messages, and doesn't say whether it refuses them | refused before sending (`refusing([…])`, `refusing_in_messages(["name"])`); `parallel_tool_calls: true` is left out and named in `kaeso-left-out` | reference |
| `tool_choice` isn't in the reference, but its guides send `"auto"` | sent as written | tool call guide |
| tool calls come with `finish_reason: "stop"` and `content: ""` | `stop` reads as the end of a turn, as for every Chat provider | tool call guide; tool call reference |
| `finish_reason: ""` and `role: ""` before the last chunk | read as no value | reference |
| `input_audio` takes a data URL in `data` and no `format`, for StepFun's audio chat models | not adapted; KPMR lists no StepFun model that takes audio | reference |
| errors carry their identifier in `type` and have no `code` or `param` | the kind follows the status; the China service's 429 `project_credit_limit_exceeded` and `member_project_credit_limit_exceeded` are billing refusals | live; error codes |
| `GET /v1/models` needs a key | a model check spots a wrong key | live |
