# Fixtures: NVIDIA (`nvidia`) on `openai_chat`

NVIDIA's hosted NIM API, the models of [build.nvidia.com](https://build.nvidia.com), OpenAI-compatible Chat Completions at `POST https://integrate.api.nvidia.com/v1/chat/completions`, with `Authorization: Bearer` and an `nvapi-` key. Kaeso's base URL is `https://integrate.api.nvidia.com`. A NIM container someone runs is a custom provider of the `nim` server kind instead; these fixtures are the hosted API's.

Not recorded from a successful call. The reply and the stream are built from the OpenAPI documents of the model reference pages, read on 2026-11-10: the request is the reference's own example for `nvidia/nemotron-3-super-120b-a12b`, and the thinking is in `reasoning_content`, the field the reference's code samples read. The model listing and the errors were answered live, without a valid key; NVIDIA lists its models publicly and answers for a retired model before checking the key.

## Sources (read 2026-11-10)

| Source | Address |
|---|---|
| LLM APIs (base URL, the models by publisher) | https://docs.api.nvidia.com/nim/reference/llm-apis |
| `nvidia/nemotron-3-super-120b-a12b` reference and OpenAPI document (`ChatRequest`, `ChatCompletion`, `ChatCompletionChunk`, `Errors`, the 202, 422 and 500 replies) | https://docs.api.nvidia.com/nim/reference/nvidia-nemotron-3-super-120b-a12b-infer |
| `z-ai/glm-5.3` reference (`ChatCompletionRequest`, the 402 reply) | https://docs.api.nvidia.com/nim/reference/z-ai-glm-5-3-infer |
| `openai/gpt-oss-120b` reference (`reasoning_content` in its code samples) | https://docs.api.nvidia.com/nim/reference/openai-gpt-oss-120b-infer |

## Files

| File | What it covers | Provenance |
|---|---|---|
| `requests/reasoning.json`, `responses/reasoning.json` | thinking turned on with `chat_template_kwargs.enable_thinking`; `message.reasoning_content` beside the answer | doc |
| `requests/reasoning-stream.json`, `streams/reasoning-stream.sse` | the reference's example with `reasoning_budget`; `delta.reasoning_content`, then the answer; the chunk schema has no usage | doc |
| `requests/invalid-api-key.json`, `errors/recorded-invalid-api-key.json` | **403** `{"status", "title": "Forbidden", "detail": "Authorization failed"}`, an RFC 9457 problem document (`application/problem+json`), for a key NVIDIA doesn't know | live, with a made-up key |
| `requests/retired-model.json`, `errors/recorded-retired-model.json` | **410** `Gone` with the end of life in `detail`, answered before the key is checked | live |
| `errors/unprocessable-entity-doc.json` | the reference's 422 problem document with `instance` and `requestId` | doc |
| `models/list-recorded.json` | `GET /v1/models` answered without a key, cut to four models: `id`, `object`, `created`, `owned_by` | live |

## Where NVIDIA differs from OpenAI Chat Completions

| Difference | How KPP carries it | Source |
|---|---|---|
| thinking comes as `reasoning_content` | a reasoning block through the profile's `reasoning_fields(["reasoning_content"])` | the references' code samples |
| thinking is turned on per model with `chat_template_kwargs` (`enable_thinking`, `low_effort`) or `reasoning_effort`, and capped with `reasoning_budget` | a Chat app's `chat_template_kwargs` and `reasoning_budget` are request `Extras` and go as written; refused when the request goes to another dialect | references |
| the request schemas have `max_tokens` and no `max_completion_tokens` | the output limit goes as `max_tokens` (`max_tokens()`) | references |
| the roles are `system`, `context`, `user` and `assistant`; no `developer` | `developer` messages go as `system` (`system_role()`) | `Message` schema |
| no `user` or `safety_identifier` | the end user's id is left out (`without_end_user()`) | references |
| errors are problem documents with `status`, `title` and `detail` | `KppError` with NVIDIA's `detail`; the kind follows the status: 403 is a key that isn't allowed the request, 410 a model that is gone | live |
| a request without a key gets 401 with the plain text ``Header of type `authorization` was missing`` | not a fixture: Kaeso always sends the key | live |
| a 202 with an `NVCF-REQID` header when the result isn't ready, to poll for | not read: Kaeso doesn't poll | references |
| `GET /v1/models` answers without a key | a model check can't spot a wrong key | live |
| `/v1/responses`, `/v1/messages/count_tokens`, `/v1/images/generations` and `/v1/audio/*` answer 404; `/v1/messages` answers 404 `model not found` for every model | Chat Completions and Embeddings only; token counts refused | live |
