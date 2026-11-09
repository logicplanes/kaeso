# Fixtures: Fireworks AI (`fireworks-ai`) on `openai_chat`

Fireworks AI's OpenAI-compatible Chat Completions, `POST https://api.fireworks.ai/inference/v1/chat/completions`, with `Authorization: Bearer`. Kaeso's base URL is `https://api.fireworks.ai/inference`, without `/v1`. Model ids are resource names such as `accounts/fireworks/models/glm-5p3-flash`.

No key was available, so every reply and stream here is built from Fireworks' OpenAPI document and guides, read on 2026-11-09, in the shape and with the fields they document. Only the errors were answered live, by requests without a valid key, which Fireworks refuses before any model runs. Request ids in them are replaced with zeros.

## Sources (read 2026-11-09)

| Source | Address |
|---|---|
| Create Chat Completion (OpenAPI 3.1, gateway REST API 5.10.0) | https://docs.fireworks.ai/api-reference/post-chatcompletions |
| OpenAI compatibility (`max_tokens` and the context window, usage in streams) | https://docs.fireworks.ai/tools-sdks/openai-compatibility |
| Reasoning (`reasoning_content`, `reasoning_effort`, interleaved thinking) | https://docs.fireworks.ai/guides/reasoning |
| Video and audio inputs (`video_url` parts) | https://docs.fireworks.ai/guides/video-audio-inputs |
| Inference error codes | https://docs.fireworks.ai/guides/inference-error-codes |
| Authentication and the key page | https://docs.fireworks.ai/api-reference/introduction |

## Files

| File | What it covers | Provenance |
|---|---|---|
| `requests/reasoning.json`, `responses/reasoning.json` | `reasoning_effort: "low"` to GLM 5.3 Flash; `message.reasoning_content`, `prompt_tokens_details.cached_tokens`, `completion_tokens_details.reasoning_tokens` | doc |
| `requests/reasoning-stream.json`, `streams/reasoning-stream.sse` | `delta.reasoning_content`, then the answer; the finish chunk, then the trailing chunk with `choices: []` and the usage that Fireworks sends by default | doc |
| `requests/tools.json`, `responses/tools.json` | one function tool to Kimi K3; reasoning before the tool call, `content: null` | doc |
| `requests/reasoning-content-sent-back.json`, `responses/reasoning-content-sent-back.json` | `reasoning_content` on the assistant turn sent back with the tool result, as the reasoning guide asks for interleaved thinking | doc |
| `requests/fireworks-fields.json`, `responses/fireworks-fields.json` | `top_k`, `min_p`, `repetition_penalty`, `context_length_exceeded_behavior`, `prompt_cache_key`, `reasoning_history`, `perf_metrics_in_response`, and the reply's `perf_metrics` | doc |
| `requests/video-url.json` | a `video_url` part, which the `chat` profile sends as written (`with_video_url()`); OpenAI's own profile refuses it | doc |
| `requests/invalid-api-key.json`, `errors/recorded-invalid-api-key.json` | 401 `UNAUTHORIZED` "The API key you provided is invalid." | live, with a made-up key |
| `requests/missing-api-key.json`, `errors/recorded-missing-api-key.json` | 401 `UNAUTHORIZED` without a key | live |
| `errors/list-invalid-api-key.json` | 401 from `GET /v1/models` with a made-up key | live |

## Where Fireworks AI differs from OpenAI Chat Completions

| Difference | How KPP carries it | Source |
|---|---|---|
| reasoning models answer with `message.reasoning_content` and `delta.reasoning_content`, never in `<think>` tags in `content`, and take `reasoning_content` back on assistant messages | a reasoning block through the `chat` profile's `reasoning_fields(["reasoning_content"])`; thinking from apps in other formats goes back as `reasoning_content` | reasoning guide |
| the schema has `user` and no `safety_identifier` | the end user's id goes as `user` (`end_user_as_user()`) | OpenAPI document |
| `video_url` content parts for its video models | sent as written (`with_video_url()`) | video and audio inputs |
| `reasoning_effort` also takes `xhigh`, `max`, booleans and token counts, and `thinking` in Anthropic's form; which values a model takes depends on the model | the catalogue's effort levels from KPMR; a Chat app's own value goes as written | OpenAPI document |
| `top_k`, `min_p`, `typical_p`, `repetition_penalty`, `mirostat_*`, `context_length_exceeded_behavior`, `prompt_cache_isolation_key`, `reasoning_history`, `perf_metrics_in_response`, `raw_output`, `return_token_ids`, `safe_tokenization`, `speculation`, `echo` | request `Extras`; refused when the request goes to another dialect, like other Chat-only fields | OpenAPI document |
| `max_tokens` above what the context window leaves is lowered to fit instead of refused (`context_length_exceeded_behavior: "truncate"` by default) | sent as the app wrote it; an app that wants OpenAI's error sends `context_length_exceeded_behavior: "error"` | OpenAI compatibility |
| streams send usage by default, in a trailing chunk with `choices: []` | the typed usage comes from that chunk | OpenAPI document (`stream_options.include_usage`) |
| `perf_metrics` on replies and on the finish chunk when asked for | extras, round-trip | OpenAPI document |
| errors use OpenAI's `error` object with Fireworks' upper-case `code` (`UNAUTHORIZED`, `NOT_FOUND`) and `type: "error"`, plus a top-level `request_id` | `KppError`; the kind follows the status, so 401 is `Authentication` | live |
| `GET /v1/models` answers 401 for a wrong key | a model check spots a wrong key | live |
| no image, speech, transcription or video endpoint is documented, and no token-count endpoint | `images`, `speech`, `transcriptions` and `videos` are empty, `count_tokens` is `None` | API reference index |
| rate limits as `x-ratelimit-*` headers on serverless | not read | rate limits |
