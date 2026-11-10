# Fixtures: Baseten (`baseten`) on `openai_chat`

Baseten Model APIs, the shared OpenAI-compatible Chat Completions at `POST https://inference.baseten.co/v1/chat/completions`, with `Authorization: Bearer` (Baseten also takes `Authorization: Api-Key`). Kaeso's base URL is `https://inference.baseten.co`. A dedicated deployment has its own address and is a custom provider instead.

Not recorded from a successful call. The reply is the reasoning guide's own example, the stream the streaming guide's own excerpt; both get the `id`, `object`, `created` and `model` the schema names and the examples leave out, and the stream the usage chunk `stream_options.include_usage` asks for. The error was answered live, without a valid key.

## Sources (read 2026-11-10)

| Source | Address |
|---|---|
| Model APIs (supported models, the three API formats, `/v1/models`, error codes) | https://docs.baseten.co/inference/model-apis/overview |
| Chat Completions OpenAPI document (`ChatCompletionRequest`, `ChatCompletionResponse`, rate-limit headers) | https://docs.baseten.co/reference/inference-api/llm-openapi-spec.json |
| Messages OpenAPI document | https://docs.baseten.co/reference/inference-api/messages-openapi-spec.json |
| Responses OpenAPI document | https://docs.baseten.co/reference/inference-api/responses-openapi-spec.json |
| Reasoning (`reasoning_effort`, `chat_template_args`, `reasoning_content`, `reasoning.max_tokens`) | https://docs.baseten.co/inference/model-apis/reasoning |
| Streaming | https://docs.baseten.co/inference/streaming |
| Inference errors | https://docs.baseten.co/inference/errors |
| Pricing and limits (rate-limit headers) | https://docs.baseten.co/inference/model-apis/pricing-and-limits |

## Files

| File | What it covers | Provenance |
|---|---|---|
| `requests/reasoning-doc.json`, `responses/reasoning-doc.json` | GLM 5.2 with `chat_template_args.enable_thinking`; `message.reasoning_content` beside the answer | doc |
| `requests/text-stream-doc.json`, `streams/text-stream-doc.sse` | a stream without `logprobs` or `finish_reason` on its first chunk, the usage in a last chunk with `choices: []` | doc |
| `requests/invalid-api-key.json`, `errors/recorded-invalid-api-key.json` | 401 `{"error": "Authentication required."}`, the error as a bare string; a request without a key gets the same | live, with a made-up key |

## Where Baseten differs from OpenAI Chat Completions

| Difference | How KPP carries it | Source |
|---|---|---|
| thinking comes as `reasoning_content`; Beam needs it back on earlier assistant messages | a reasoning block through the profile's `reasoning_fields(["reasoning_content"])`, which also sends it back there | reasoning guide |
| thinking is turned on or off per model with `reasoning_effort` or `chat_template_args.enable_thinking`, and capped with `reasoning.max_tokens` | `reasoning_effort` is `reasoning.effort`; `chat_template_args` and the `reasoning` object are request `Extras`, which go as written and are refused towards another dialect | reasoning guide |
| the request schema has `max_tokens` and no `max_completion_tokens` | the output limit goes as `max_tokens` (`max_tokens()`) | OpenAPI document |
| the roles are `system`, `user`, `assistant` and `tool` | `developer` messages go as `system` (`system_role()`) | OpenAPI document |
| the end user as `user`, no `safety_identifier` | `end_user_as_user()` | OpenAPI document |
| `top_k`, `min_p`, `repetition_penalty`, `chat_template`, `documents` and other vLLM-style fields | request `Extras` | OpenAPI document |
| errors as `{"error": "<message>"}` or a structured error object | `KppError`; the kind follows the status: 401 is a refused key | live; inference errors |
| `x-ratelimit-limit-requests`, `-remaining-requests`, `-limit-tokens`, `-remaining-tokens` | not read | OpenAPI document |
| a stream can end without `[DONE]` when the model fails after the headers | apps get the stream decoder's error that the stream ended before the reply was complete | inference errors |
| `GET /v1/models` with a wrong key answers 401 with a body shorter than its `Content-Length` | not a fixture: the body can't be read in full | live |
| `/v1/messages` takes only text and tool blocks, no thinking or images; `/v1/responses` is stateless | neither is used: Messages and Responses apps reach Baseten through Chat Completions | OpenAPI documents |
