# Fixtures: IONOS AI Model Hub (`ionos`) on `openai_chat`

IONOS CLOUD's OpenAI-compatible AI Model Hub, `POST https://openai.inference.de-txl.ionos.com/v1/chat/completions`, with `Authorization: Bearer` and an IONOS Cloud API token (a JWT). Kaeso's base URL is `https://openai.inference.de-txl.ionos.com`, in Berlin, IONOS's only region. Model ids name their maker and are case-sensitive, such as `openai/gpt-oss-120b` and `Qwen/Qwen3.8-27B`.

No token was available. The replies are the examples of IONOS's model pages, unchanged; the request each answers is built from the page's example prompt. The stream is built from the schema around the final chunk the docs show. The key errors were answered live without a valid token.

## Sources (read 2026-11-14)

| Source | Address |
|---|---|
| Create chat completion (OpenAPI 3.0.3, `ChatCompletionRequest`, `ChatCompletionResponse`, `Error`) | https://docs.ionos.com/cloud/api-reference/ai/ai-model-hub/chat-completions/create-chat-completion |
| Error codes | https://docs.ionos.com/cloud/ai/ai-model-hub/error-codes |
| GPT OSS 120B (reasoning effort, the reply with `reasoning` and `reasoning_content`, the usage chunk) | https://docs.ionos.com/cloud/ai/ai-model-hub/models/llms/openai-gpt-oss-120b |
| Qwen3.8 27B (effort levels `none`, `low`, `medium`, `xhigh`, the reply with `reasoning`) | https://docs.ionos.com/cloud/ai/ai-model-hub/models/llms/qwen3-8-27b |
| Mistral Small 24B Instruct (the image reply) | https://docs.ionos.com/cloud/ai/ai-model-hub/models/llms/mistral-small-24b-instruct |
| Access management and tokens | https://docs.ionos.com/cloud/ai/ai-model-hub/how-tos/access-management |
| Data handling (Germany) | https://docs.ionos.com/cloud/ai/ai-model-hub/governance-and-compliance/data-handling |

## Files

| File | What it covers | Provenance |
|---|---|---|
| `requests/gpt-oss-reasoning-doc.json`, `responses/gpt-oss-reasoning-doc.json` | `reasoning_effort: "medium"`; the reply carries the same thinking as `reasoning` and `reasoning_content` | doc |
| `requests/qwen-reasoning-doc.json`, `responses/qwen-reasoning-doc.json` | Qwen3.8's thinking as `reasoning` only | doc |
| `requests/mistral-image-doc.json`, `responses/mistral-image-doc.json` | an image part; the reply's vLLM fields: `refusal`, `annotations`, `audio`, `function_call`, `reasoning` and `reasoning_content` all `null`, `tool_calls: []`, `stop_reason`, `token_ids`, `service_tier: null`, `system_fingerprint: null`, `prompt_tokens_details: null`, `prompt_logprobs`, `prompt_token_ids`, `kv_transfer_params` | doc |
| `requests/usage-stream-doc.json`, `streams/usage-stream-doc.sse` | `stream_options.include_usage`; the usage in a last chunk with `choices: []` (the docs' chunk), the chunks before it built from the schema | doc |
| `requests/missing-api-key.json`, `errors/recorded-missing-api-key.json` | 401 in IONOS's own shape, `{"httpStatus", "messages": [{"errorCode": "paas-auth-1", "message"}]}` | live |
| `requests/invalid-api-key.json`, `errors/recorded-invalid-api-key.json` | 401 `paas-token-1` for a made-up token | live |
| `errors/validation-error-doc.json` | 400 `VALIDATION_ERROR` in IONOS's shape | doc |
| `errors/overloaded-doc.json` | 529 `server_overloaded` in OpenAI's shape, the one IONOS error that has it | doc |

## Where IONOS differs from OpenAI Chat Completions

| Difference | How KPP carries it | Source |
|---|---|---|
| `developer` is a role only some models take; others answer 400 | the profile sends `system` (`system_role()`), as IONOS advises | create chat completion |
| `user` and no `safety_identifier` | the end user's id goes as `user` (`end_user_as_user()`) | OpenAPI document |
| no `prompt_cache_key`; prefix caching is automatic | a Responses app's cache key is left out (`without_cache_key()`) | OpenAPI document; model pages |
| reasoning as `reasoning`, on some models also as `reasoning_content` with the same text | a reasoning block through `reasoning_fields(["reasoning", "reasoning_content"])` | OpenAPI document; model pages |
| `video_url` parts on video models; no `input_audio` or `file` parts | `video_url` sent as written (`with_video_url()`); audio is refused by the catalogue, whose models take none | OpenAPI document |
| no `metadata`, `store`, `modalities`, `audio`, `prediction`, `prompt_cache_retention`, `service_tier`, `web_search_options`, `functions` or `function_call`, and the docs don't say whether undeclared fields are refused | refused before sending (`refusing([…])`) | OpenAPI document |
| `max_tokens` and `max_completion_tokens` both count reasoning, and which applies when both are sent is model-specific | the app's field goes as written | create chat completion |
| errors as `{"httpStatus", "messages": [{"errorCode", "message"}]}`, except 529 in OpenAI's shape | `KppError` with IONOS's messages joined; the kind follows the status | error codes |
| `GET /v1/models` needs a token and lists only `id`, `object`, `created` and `owned_by`; there is no `GET /v1/models/{model}` | a model check reads the listing | OpenAPI document; live 404 `ROUTE_NOT_FOUND` |
