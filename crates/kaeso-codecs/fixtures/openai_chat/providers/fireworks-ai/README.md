# `fixtures/openai_chat/providers/fireworks-ai`

Fireworks AI, provider id `fireworks-ai`, over its OpenAI-compatible Chat Completions API at `https://api.fireworks.ai/inference/v1/chat/completions`, with `authorization: Bearer`.

Sources, read 2026-10-19:

- API reference: https://docs.fireworks.ai/api-reference/post-chatcompletions (OpenAPI "Gateway REST API" 5.10.0, embedded in the page)
- OpenAI compatibility: https://docs.fireworks.ai/tools-sdks/openai-compatibility
- Reasoning: https://docs.fireworks.ai/guides/reasoning
- Error codes: https://docs.fireworks.ai/guides/inference-error-codes
- Official Python SDK: https://github.com/fw-ai-external/python-sdk at `538be68c6c4b7658003a62065fe1cc9f34359ccc` (types and tests)
- Cookbook: https://github.com/fw-ai/cookbook at `6ada4081cf7bdb5cec7dcf066dd29d784f55b232`

There is no live recording: no test key was used. Every file is a request or error body copied from a curl example or a JSON block of the official documentation, re-indented and otherwise unchanged; placeholders such as `accounts/<account_id>/models/<model_id>` are the documentation's own.

| file | what it covers | provenance |
|---|---|---|
| `requests/stream-without-stream-options-doc.json` | `stream: true` without `stream_options`; Fireworks streams usage by default | https://docs.fireworks.ai/getting-started/quickstart, streaming curl |
| `requests/json-schema-quickstart-doc.json` | `response_format` `json_schema` without `strict` | quickstart, structured output curl |
| `requests/reasoning-effort-medium-doc.json` | `reasoning_effort: "medium"` for `accounts/fireworks/models/glm-5p3` | quickstart, reasoning curl |
| `requests/video-url-part-doc.json` | a `video_url` content part, which only Fireworks has; kept as an unknown part | https://docs.fireworks.ai/guides/video-audio-inputs, curl |
| `requests/service-tier-priority-doc.json` | `service_tier: "priority"` | https://docs.fireworks.ai/serverless/serverless-modes, first curl |
| `requests/router-model-doc.json` | a router model id, `accounts/fireworks/routers/kimi-k3-fast` | serverless modes, second curl |
| `requests/nim-chat-template-kwargs-tools-doc.json` | NIM's `chat_template_kwargs` with `tools` and `tool_choice: "required"` | https://docs.fireworks.ai/tools-sdks/nim-compatibility, tools curl |
| `requests/include-routing-matrix-doc.json` | `include_routing_matrix` and `logprobs: true`, a body field the OpenAPI schema does not list | https://docs.fireworks.ai/guides/rollout-inference, routing matrix curl |
| `errors/deployment-scaling-up-doc.json` | 503 `DEPLOYMENT_SCALING_UP` from a deployment scaled to zero, `type: "error"` | https://docs.fireworks.ai/deployments/autoscaling, "Scaling from zero behavior"; the status is the one the page names, and the page shows no headers |

## Cases without a fixture

The documentation, the OpenAPI document and the SDK repositories hold no complete chat completion, stream, model listing or other error body. Their examples are generated from the schema (`<string>` placeholders), abridged with `...`, or printed by a client, so none of them is a fixture here:

- **Replies and streams:** `message.reasoning_content` and `delta.reasoning_content`, `<think>` text inside `content` on some models, `perf_metrics` (hyphenated keys, with `perf_metrics_in_response: true`), `prompt_token_ids` and `choices[].token_ids`, `raw_output`, and usage in `prompt_tokens_details.cached_tokens`, `completion_tokens_details.reasoning_tokens` and `output_tokens_details`. The OpenAI Chat codec keeps every one of them in extras or in `usage.raw` by its general rules, so they round-trip within this dialect and are refused or left out across dialects like any other unknown key.
- **Stream usage:** the reference says usage comes by default in a last chunk with empty `choices`; the compatibility guide shows it on the `finish_reason` chunk. The codec reads usage from either.
- **Errors:** a plain-string `{"error": "unauthorized"}` 403 for a bad key (seen only as a Python repr in a cookbook notebook), the `{"error": {"code": "NOT_FOUND", ...}, "request_id": "..."}` body (abridged in the docs), and FastAPI-style 422 `detail[]` bodies (schema only).
- **Model listing:** `GET /inference/v1/models` exists (the cookbook's 403 reached it) but is not documented and no reply was found; the gateway then keeps the model as not checked and the console shows the models.dev catalogue.
