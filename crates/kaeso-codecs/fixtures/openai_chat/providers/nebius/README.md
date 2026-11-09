# Fixtures: Nebius Token Factory (`nebius`) on `openai_chat`

Nebius Token Factory's OpenAI-compatible Chat Completions, `POST https://api.tokenfactory.nebius.com/v1/chat/completions`, with `Authorization: Bearer`. Kaeso's base URL is `https://api.tokenfactory.nebius.com`. Model ids name their maker, such as `zai-org/GLM-5.3`; a `-fast` suffix chooses the faster flavor of a model.

No key was available. The replies and the stream here are built from Nebius' OpenAPI document (`https://api.tokenfactory.nebius.com/openapi.json`, version `20261105-4c58718c5`) and its examples, read on 2026-11-09; `responses/text-generation-doc.json` is the documentation's own example reply, with its masked id filled in. Only the errors were answered live, by requests without a valid key.

## Sources (read 2026-11-09)

| Source | Address |
|---|---|
| Create chat completion (OpenAPI 3.1) | https://docs.tokenfactory.nebius.com/api-reference/inference/create-chat-completion |
| Text generation example (`responses/text-generation-doc.json`) | https://docs.tokenfactory.nebius.com/api-reference/examples/text-generation |
| Introduction and authentication, the key page | https://docs.tokenfactory.nebius.com/api-reference/introduction |
| Model flavors (`-fast`) and generation parameters | https://docs.tokenfactory.nebius.com/ai-models-inference/overview |
| Rate limits and their headers | https://docs.tokenfactory.nebius.com/ai-models-inference/rate-limits |
| Public endpoints and regions | https://docs.tokenfactory.nebius.com/public-serverless |
| Models leaving serverless on 2026-11-09 | https://docs.tokenfactory.nebius.com/november-2026-deprecation-notice |

## Files

| File | What it covers | Provenance |
|---|---|---|
| `requests/text-generation-doc.json`, `responses/text-generation-doc.json` | the documentation's example: `function_call: null`, `tool_calls: []`, `stop_reason`, `system_fingerprint: null` | doc |
| `requests/reasoning.json`, `responses/reasoning.json` | `reasoning_effort: "low"` to GLM-5.3; `reasoning_content` on the message, the cached and reasoning token counts, `service_tier` | doc |
| `requests/tools.json`, `responses/tools.json` | one function tool to Kimi K3; a tool call with Kimi's `functions.get_weather:0` id | doc |
| `requests/reasoning-stream.json`, `streams/reasoning-stream.sse` | `delta.reasoning`, then the answer; `system_fingerprint` on every chunk, the usage in a last chunk with `choices: []` | doc |
| `requests/video-url.json` | a `video_url` part, which the `chat` profile sends as written (`with_video_url()`) | doc |
| `requests/invalid-api-key.json`, `errors/recorded-invalid-api-key.json` | 401 `unauthenticated` "Couldn't authenticate. Reason: Unable authenticate", with a top-level `detail` | live, with a made-up key |
| `requests/missing-api-key.json`, `errors/recorded-missing-api-key.json` | 401 `unauthenticated` without a key | live |
| `errors/list-invalid-api-key.json` | 401 from `GET /v1/models`: `{"detail": "…"}` without an `error` object | live, with a made-up key |

## Where Nebius differs from OpenAI Chat Completions

| Difference | How KPP carries it | Source |
|---|---|---|
| reasoning comes as `reasoning_content` or as `reasoning`, by model; both are declared on messages and deltas | a reasoning block through the `chat` profile's `reasoning_fields(["reasoning_content", "reasoning"])` | OpenAPI document |
| the roles are `system`, `user`, `assistant` and `tool`; there is no `developer` | the profile sends `system` (`system_role()`) | OpenAPI document (`ChatMessageRole`) |
| `user` and no `safety_identifier` | the end user's id goes as `user` (`end_user_as_user()`) | OpenAPI document |
| `video_url` content parts | sent as written (`with_video_url()`) | OpenAPI document (`ChatCompletionContentPartVideoParam`) |
| `reasoning_effort` also takes `xhigh` and `max` | the catalogue's effort levels from KPMR | OpenAPI document |
| the request takes any vLLM sampling field, such as `top_k` or `repetition_penalty`, and `extra_body`; messages take `partial` and Kimi's `tools` | request `Extras`; refused when the request goes to another dialect, like other Chat-only fields | overview ("the full set of vLLM parameters"), OpenAPI document |
| `service_tier` on every reply, `stop_reason`, `function_call: null`, `tool_calls: []` | extras, round-trip | OpenAPI document, example |
| `response_format` takes only `json_object` and `text`, the description says | sent as written; Nebius answers a JSON schema itself | OpenAPI document |
| errors carry OpenAI's `error` object and a top-level `detail`; `GET /v1/models` answers with `detail` alone | `KppError`; the kind follows the status, so 401 is `Authentication` | live |
| no image, speech, transcription or video endpoint, no Messages endpoint and no token count: `/v1/images/generations`, `/v1/audio/transcriptions` and `/v1/messages` answer 404 | those stay empty and `count_tokens` is `None` | OpenAPI document; live probes |
| rate limits as `x-ratelimit-*` headers, with `x-ratelimit-over-limit: yes` when a request over the limit still ran | not read | rate limits |
