# Fixtures: Upstage (`upstage`) on `openai_chat`

Upstage's OpenAI-compatible Chat Completions (the Solar API), `POST https://api.upstage.ai/v1/chat/completions`, with `Authorization: Bearer`. Upstage's guides write the base URL as `https://api.upstage.ai/v1`, so Kaeso's base URL is the host, `https://api.upstage.ai`. The older `https://api.upstage.ai/v1/solar/…` still answers. Model ids are aliases, such as `solar-pro4`, that point to a dated snapshot (`solar-pro4-260806`), and replies name the snapshot.

No key was available. The replies and the stream are the examples of Upstage's API reference and guides, copied unchanged; the key error was answered live with a made-up key.

## Sources (read 2026-11-14)

| Source | Address |
|---|---|
| API reference: request schema, the "Default" reply and the "Streaming" tab | https://console.upstage.ai/api/chat |
| Chat guide (the reply of `responses/greeting-doc.json`) | https://console.upstage.ai/docs/capabilities/generate/chat |
| Reasoning (`reasoning_effort`, `message.reasoning`, `delta.reasoning`) | https://console.upstage.ai/docs/capabilities/generate/reasoning |
| Structured outputs (`json_schema` with `strict: true`) | https://console.upstage.ai/docs/capabilities/generate/structured-outputs |
| Tool calling (the request of `requests/tools-doc.json`) | https://console.upstage.ai/docs/capabilities/generate/tool-calling |
| Error codes (`invalid_api_key` and the others) | https://console.upstage.ai/docs/resources/error-codes |

## Files

| File | What it covers | Provenance |
|---|---|---|
| `requests/greeting-doc.json`, `responses/greeting-doc.json` | reasoning off: no `reasoning`, `reasoning_tokens: 0`; a bare UUID as `id`, `system_fingerprint: null`, every `*_tokens_details` field OpenAI has | doc (chat guide) |
| `requests/greeting-reasoning-doc.json`, `responses/greeting-reasoning-doc.json` | the API reference's "Default" reply, with `message.reasoning` beside the answer and 17 reasoning tokens. The reference pairs it with a request without `reasoning_effort`, as written here | doc (API reference) |
| `requests/reasoning-medium-doc.json`, `responses/reasoning-medium-doc.json` | `reasoning_effort: "medium"`; 363 of 371 output tokens are reasoning | doc (reasoning guide) |
| `requests/structured-output-doc.json`, `responses/structured-output-doc.json` | `response_format` `json_schema` with `strict: true`; the JSON comes as a string in `content`, with `reasoning` | doc (structured outputs guide) |
| `requests/tools-doc.json` | one function tool, `tool_choice: "auto"`, `parallel_tool_calls: false`; the guide prints no tool-call reply, so there is none here | doc (tool calling guide) |
| `requests/reasoning-stream-doc.json`, `streams/reasoning-stream-doc.sse` | the role chunk, `delta.reasoning`, then `delta.content`, and a last chunk with `delta: {}` and `finish_reason`; no usage, since the request asks for none. The reference leaves out chunks with a `...` line; they are left out here too, so the text is cut short | doc (API reference) |
| `requests/invalid-api-key.json`, `errors/recorded-invalid-api-key.json` | 401 `invalid_api_key` with OpenAI's `error` object and `param: ""`; a missing key gets the same body | live, with a made-up key |

## Where Upstage differs from OpenAI Chat Completions

| Difference | How KPP carries it | Source |
|---|---|---|
| the roles are `system`, `user`, `assistant` and `tool`; there is no `developer` | the profile sends `system` (`system_role()`) | API reference |
| `max_tokens` and no `max_completion_tokens` | the output limit goes as `max_tokens` (`max_tokens()`) | API reference |
| no `user` or `safety_identifier` | the end user's id isn't sent (`without_end_user()`) | API reference |
| reasoning comes as `reasoning` on the message and the delta; it is off unless `reasoning_effort` is `low` to `max` | a reasoning block through the `chat` profile's `reasoning_fields(["reasoning"])` | API reference; reasoning guide |
| the schema has no `stop`, `seed`, `logprobs`, `top_logprobs`, `logit_bias`, `metadata`, `store`, `modalities`, `audio`, `prediction`, `service_tier`, `prompt_cache_retention`, `web_search_options`, `verbosity`, `functions` or `function_call`, and the docs don't say whether Upstage refuses or drops an undeclared field | refused before sending (`refusing([…])`) | API reference |
| `content` is a string; no image, audio, video or file parts | the Solar models take text only, so the catalogue refuses media before the codec sees it | API reference; model pages |
| `frequency_penalty` defaults to 1.1, not 0 | sent as written; an app that leaves it out gets Upstage's default | API reference |
| `id` is a bare UUID, `system_fingerprint` is always `null`, `choices[].logprobs` is always `null` | round-trip | API reference |
| a 404 for an unknown path has no JSON body (`text/plain`, `404 page not found`), and there is no `GET /v1/models/{model}` | the status decides the error kind | error codes; live |
| `GET /v1/models` needs a key and isn't in the API reference | a model it leaves out is inconclusive (`ModelListing::Partial`) | Upstage's Claude Code script |
| rate-limit headers are `X-Upstage-RateLimit-*`, and a 429 carries `X-Upstage-RateLimit-Retry-After-*` as a Unix time, not `Retry-After` | not read | rate-limit guide |
