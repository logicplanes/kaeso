# Fixtures: Scaleway (`scaleway`) on `openai_chat`

Scaleway Generative APIs, the serverless OpenAI-compatible Chat Completions at `POST https://api.scaleway.ai/v1/chat/completions`, with `Authorization: Bearer` and a Scaleway IAM secret key. Kaeso's base URL is `https://api.scaleway.ai`, which reaches the key's default Project; another Project's path is `https://api.scaleway.ai/{project_id}/v1/…`, so its base URL is `https://api.scaleway.ai/{project_id}`.

No key was available. The replies and the stream here are built from Scaleway's Generative APIs reference (`https://www.scaleway.com/en/developers/api/generative-apis/v1/schema.yml`) and guides, read on 2026-11-09; the stream's first chunks are the reasoning guide's own example. The errors were answered live, by requests without a valid key.

## Sources (read 2026-11-09)

| Source | Address |
|---|---|
| Generative APIs reference (OpenAPI, "OpenAI Compatible API v1") | https://www.scaleway.com/en/developers/api/generative-apis/ |
| OpenAI API compatibility | https://www.scaleway.com/en/docs/generative-apis/reference-content/openai-compatibility/ |
| How to query reasoning models (`reasoning_content`, `delta.reasoning`, `<think>` in legacy models) | https://www.scaleway.com/en/docs/generative-apis/how-to/query-reasoning-models/ |
| How to query audio models (`input_audio`, transcriptions) | https://www.scaleway.com/en/docs/generative-apis/how-to/query-audio-models/ |
| Using Generative APIs (authentication, Projects) | https://www.scaleway.com/en/docs/generative-apis/api-cli/using-generative-apis/ |
| Understanding errors (recent and legacy error formats) | https://www.scaleway.com/en/docs/generative-apis/api-cli/understanding-errors/ |
| Rate limits and their headers | https://www.scaleway.com/en/docs/generative-apis/reference-content/rate-limits/ |

## Files

| File | What it covers | Provenance |
|---|---|---|
| `requests/reasoning.json`, `responses/reasoning.json` | the reasoning guide's request to `gpt-oss-120b`; `message.reasoning_content` beside the answer | doc |
| `requests/reasoning-stream.json`, `streams/reasoning-stream.sse` | `qwen3.5-397b-a17b` streams its reasoning as `delta.reasoning`, then the answer; `token_ids: null` on every choice, the usage in a last chunk with `choices: []` | doc |
| `requests/input-audio.json` | an `input_audio` part in WAV, as the audio guide sends to `voxtral-small-24b-2507` | doc |
| `requests/invalid-api-key.json`, `errors/recorded-invalid-api-key.json` | **403** `forbidden` "insufficient permissions to access the resource": Scaleway answers a key it doesn't know like a key without the permission | live, with a made-up key |
| `requests/missing-api-key.json`, `errors/recorded-missing-api-key.json` | 401 `unauthorized` "missing 'Authorization' header" | live |
| `errors/list-invalid-api-key.json` | 403 from `GET /v1/models` | live, with a made-up key |
| `errors/recorded-route-not-found.json` | 404 `route_not_found` for `/v1/messages`, which Scaleway doesn't serve | live |
| `errors/legacy-model-error-doc.json` | the legacy error shape with `object: "error"` at the top level, which older models still send; the values are made up | doc |

## Where Scaleway differs from OpenAI Chat Completions

| Difference | How KPP carries it | Source |
|---|---|---|
| reasoning comes as `message.reasoning_content` in replies and as `delta.reasoning` in streams | a reasoning block through the `chat` profile's `reasoning_fields(["reasoning_content", "reasoning"])` | reasoning guide |
| the roles are `system`, `user`, `assistant` and `tool`; there is no `developer` | the profile sends `system` (`system_role()`) | reference (`ChatCompletionRequestMessage`) |
| the request has no `user` or `safety_identifier` | the end user's id is left out (`without_end_user()`) | reference |
| content parts are `text`, `image_url` and `input_audio` (`wav` or `mp3`) | as OpenAI's | reference |
| `token_ids` on stream choices | extras, round-trip | reasoning guide |
| errors carry the HTTP status as a number in `error.code`, `type` such as `unauthorized` or `forbidden`, and `param: ""`; legacy models answer with `object: "error"` at the top level | `KppError`; the kind follows the status, so a wrong key (403) is `PermissionDenied`, which apps get as a key that isn't allowed the request | understanding errors; live |
| no Messages, images, speech or token-count endpoint: `/v1/messages`, `/v1/images/generations` and `/v1/audio/speech` answer 404 `route_not_found` | those stay empty and `count_tokens` is `None` | reference; live probes |
| rate limits as `x-ratelimit-{limit,remaining,reset}-{requests,tokens}` headers | not read | rate limits |
