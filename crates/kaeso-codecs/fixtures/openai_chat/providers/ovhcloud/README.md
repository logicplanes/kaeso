# Fixtures: OVHcloud AI Endpoints (`ovhcloud`) on `openai_chat`

OVHcloud AI Endpoints' OpenAI-compatible Chat Completions, `POST https://oai.endpoints.kepler.ai.cloud.ovh.net/v1/chat/completions`, with `Authorization: Bearer`. The documented base URL is `https://oai.endpoints.kepler.ai.cloud.ovh.net/v1`, and every path is `/v1/…` under the host, which is Kaeso's base URL `https://oai.endpoints.kepler.ai.cloud.ovh.net`. Model ids are case-sensitive (`Qwen3.5-9B`, `Meta-Llama-3_3-70B-Instruct`); lowercase forms work only where OVHcloud's catalogue lists them as aliases, and the reply then names the listed id.

AI Endpoints answers without a key, at 2 requests a minute per IP and model, so every reply here is a real one: each request was sent as written on 2026-11-14 without a key, and the answer was saved unchanged. The key errors were answered live with a made-up key. The model listing was answered live without a key and cut to four of its models. No request ids are kept (OVHcloud sends them only as headers).

## Sources (read 2026-11-14)

| Source | Address |
|---|---|
| OpenAPI document of the unified endpoint (`ChatCompletionParameters`, `ChatMessage`, `Usage`, `FinishReason`) | https://oai.endpoints.kepler.ai.cloud.ovh.net/doc/gpt-oss-120b/openapi.json |
| Getting started (keys, anonymous use, rate limits, the 403 of a wrong key) | https://docs.ovhcloud.com/en/guides/public-cloud/ai-machine-learning/ai-endpoints-getting-started.md |
| Function calling (`tools`, `tool_choice`) | https://docs.ovhcloud.com/en/guides/public-cloud/ai-machine-learning/ai-endpoints-function-calling.md |
| Structured output (`response_format: json_schema`) | https://docs.ovhcloud.com/en/guides/public-cloud/ai-machine-learning/ai-endpoints-structured-output.md |
| Catalogue API (model ids, aliases, reasoning efforts, prices) | https://catalog.endpoints.ai.ovh.net/rest/v1/models_v2 |

## Files

| File | What it covers | Provenance |
|---|---|---|
| `requests/recorded-greeting.json`, `responses/recorded-greeting.json` | `gpt-oss-20b` with a system message and `max_completion_tokens`; `reasoning` beside the answer, an empty `tool_calls`, usage without details | live, without a key |
| `requests/recorded-tools.json`, `responses/recorded-tools.json` | `gpt-oss-120b`, one function tool, `tool_choice: "required"`, `reasoning_effort: "low"`; an empty `content`, `reasoning`, a `chatcmpl-tool-…` call and `completion_tokens_details.reasoning_tokens` | live, without a key |
| `requests/recorded-tool-result.json`, `responses/recorded-tool-result.json` | the same conversation with the assistant's `reasoning` and tool call sent back and the tool's result; the answer without `reasoning` and with `reasoning_tokens: 0` | live, without a key |
| `requests/recorded-thinking-length.json`, `responses/recorded-thinking-length.json` | `Qwen3.5-9B`, which thinks by default, cut by `max_completion_tokens: 16`: a message with `reasoning` and no `content` key, `finish_reason: "length"` | live, without a key |
| `requests/recorded-json-schema.json`, `responses/recorded-json-schema.json` | `Mistral-Small-3.2-24B-Instruct-2506` with `response_format: json_schema` and `temperature: 0`; a message without `reasoning` and `tool_calls` | live, without a key |
| `requests/recorded-reasoning-stream.json`, `streams/recorded-reasoning-stream.sse` | `gpt-oss-20b` streaming with `include_usage`: a first delta with an empty `content`, `delta.reasoning`, then `delta.content`, each later delta with `"name": "assistant"`, the `finish_reason` chunk, and the usage in a chunk of its own with `choices: []` and `system_fingerprint` | live, without a key |
| `requests/recorded-invalid-api-key.json`, `errors/recorded-invalid-api-key.json` | 403 with only a `message`, "Forbidden: authentication failed." and a link to make a new key | live, with a made-up key |
| `requests/recorded-unknown-model.json`, `errors/recorded-unknown-model.json` | 404 `model_not_found` in OpenAI's shape | live, without a key |
| `requests/recorded-developer-role-mistral.json`, `errors/recorded-developer-role-mistral.json` | 400 for a `developer` message on Mistral Small 3.2: vLLM's error passed on as text inside a bare `message` | live, without a key |
| `requests/recorded-service-tier.json`, `errors/recorded-service-tier.json` | 400 "feature 'extra arguments: …' is not currently supported", AI Endpoints' answer to a field its schema doesn't declare | live, without a key |
| `models/list-recorded.json` | `GET /v1/models` without a key, cut to `whisper-large-v3`, `bge-m3`, `gpt-oss-20b` and `Qwen3.5-9B`: every model kind, with `pricing` in USD per token, `context_length` and `max_completion_tokens` | live, without a key |
| `models/retrieve-recorded.json` | `GET /v1/models/gpt-oss-20b` without a key | live, without a key |

## Where OVHcloud AI Endpoints differs from OpenAI Chat Completions

| Difference | How KPP carries it | Source |
|---|---|---|
| no key is needed: anonymous requests are served at 2 a minute per IP and model; a wrong key is refused with 403 | a credential always sends its key; the 403's message marks the key as refused (`refused_key_messages`) | getting started; live |
| errors come in three shapes: OpenAI's `error` object (unknown model), a bare `message` (wrong key, undeclared fields), and vLLM's error as JSON text inside a bare `message` | the bare `message` is quoted; the text of a nested `error.message` is read out of it | live |
| the `developer` role works on some models only (Mistral answers "Unknown message role: developer") | the profile sends `system` (`system_role()`) | OpenAPI document; live |
| `user` is refused with 400, and no other end-user field is declared | the end user isn't sent (`without_end_user()`) | live |
| `prompt_cache_key` passes the parser but isn't declared | a Chat app's is refused before sending; a Responses app's cache key is left out (`without_cache_key()`) | OpenAPI document; live |
| undeclared fields such as `service_tier`, `verbosity`, `prompt_cache_retention`, `top_k` and `chat_template_kwargs` get a 400 naming them all | sent as written; AI Endpoints refuses them itself | live |
| `metadata`, `store`, `modalities`, `audio`, `prediction`, `prompt_cache_key`, `web_search_options`, `functions` and `function_call` aren't declared, and the parser lets at least `metadata`, `store`, `prediction` and `web_search_options` through without a word | refused before sending (`refusing([…])`) | OpenAPI document; live |
| reasoning comes as `reasoning` on the message and the delta, and assistant messages take it back | a reasoning block through the `chat` profile's `reasoning_fields(["reasoning"])` | live; OpenAPI document |
| a reply cut while thinking has no `content` key at all | a reasoning block alone, with the stop reason `length` | live |
| `tool_calls: []` on plain replies and `"name": "assistant"` on stream deltas | extras, round-trip | live |
| the usage comes in a chunk of its own with `choices: []` and `system_fingerprint` | usage, round-trip | live |
| `top_logprobs` goes up to 5 in the schema, not 20 | sent as written | OpenAPI document |
| no `video_url` or `file` content parts | not sent (`video_url` is off in the profile) | OpenAPI document |
| `GET /v1/models` lists chat, embeddings, transcription, speech and image models alike, with prices in USD per token, and refuses a wrong key with 403 | a model check reads the whole listing | live |
| model ids are case-sensitive, with lowercase aliases for some | KPMR lists the ids `GET /v1/models` names | live; catalogue API |
