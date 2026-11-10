# Fixtures: Hugging Face (`huggingface`) on `openai_chat`

Hugging Face Inference Providers, the router's OpenAI-compatible Chat Completions at `POST https://router.huggingface.co/v1/chat/completions`, with `Authorization: Bearer` and a fine-grained Hugging Face token with the "Make calls to Inference Providers" permission. Kaeso's base URL is `https://router.huggingface.co`. The router passes each request on to one of the providers that serve the model: the fastest by default, the one a `:provider` suffix names (`openai/gpt-oss-120b:groq`), or the one a `:fastest`, `:cheapest` or `:preferred` suffix picks.

Not recorded from a successful call. The reply and the stream are built from the router's chat completion reference (the request and response schemas of `huggingface.js` `packages/tasks/src/tasks/chat-completion/spec`), read on 2026-11-10, with the thinking in the field the provider behind the router uses: `reasoning` for `openai/gpt-oss-120b` as Groq and Cerebras serve it, `reasoning_content` as Novita and Together AI serve DeepSeek's models. The model listing and the error were answered live, without a valid key; the router lists its models publicly.

## Sources (read 2026-11-10)

| Source | Address |
|---|---|
| Inference Providers (base URL, token permission, provider suffixes and policies, `GET /v1/models`) | https://huggingface.co/docs/inference-providers/index |
| Chat completion task reference (request and response fields) | https://huggingface.co/docs/inference-providers/tasks/chat-completion |
| Chat completion schemas | https://github.com/huggingface/huggingface.js/tree/main/packages/tasks/src/tasks/chat-completion/spec |
| Hub API (the listing's `providers` array) | https://huggingface.co/docs/inference-providers/hub-api |
| Responses API (beta) | https://huggingface.co/docs/inference-providers/guides/responses-api |
| Claude Code (the Anthropic-compatible Messages endpoint) | https://huggingface.co/docs/inference-providers/integrations/claude-code |

## Files

| File | What it covers | Provenance |
|---|---|---|
| `requests/provider-suffix.json`, `responses/provider-suffix.json` | a model id with the `:cheapest` policy; the reply names the model without it, carries `message.reasoning` beside the answer, `system_fingerprint: null` and `logprobs: null` | doc |
| `requests/reasoning-stream.json`, `streams/reasoning-stream.sse` | a model pinned to one provider with `:novita`; `delta.reasoning_content`, then the answer, the usage in a last chunk with `choices: []` | doc |
| `requests/invalid-api-key.json`, `errors/recorded-invalid-api-key.json` | 401 `{"error": "Invalid username or password."}`, the error as a bare string; the same message is in the `x-error-message` header | live, with a made-up token |
| `models/list-recorded.json` | `GET /v1/models` answered without a token, cut to three models and two providers each: `architecture` and, per provider, `status`, `context_length`, `pricing` per million tokens, `supports_tools`, latency and throughput | live |

## Where Hugging Face differs from OpenAI Chat Completions

| Difference | How KPP carries it | Source |
|---|---|---|
| thinking comes in the field of the provider behind the router: `reasoning_content` or `reasoning` | a reasoning block through the profile's `reasoning_fields(["reasoning_content", "reasoning"])` | the providers' own documentation |
| the request schema has `max_tokens` and no `max_completion_tokens` | the output limit goes as `max_tokens` (`max_tokens()`) | chat completion schema |
| the request schema has no `user` or `safety_identifier` | the end user's id is left out (`without_end_user()`) | chat completion schema |
| `role` is any string, passed on to providers that don't all take `developer` | `developer` messages go as `system` (`system_role()`) | chat completion schema |
| `tool_prompt` and other fields beyond OpenAI's | request `Extras`; refused when the request goes to another dialect | chat completion schema |
| model ids with a `:provider` or policy suffix | sent as written; the reply's `model` drops the suffix | Inference Providers |
| errors as `{"error": "<message>"}` | `KppError`; the kind follows the status, so 401 is `Authentication` | live |
| a request without a token gets an HTML page with 401 | not a fixture: Kaeso always sends the key | live |
| `GET /v1/models` answers without a token | a model check can't spot a wrong token | live |
| `GET /v1/models/{id}` answers `{"data": {…}}`, not the model object | not used | live |
| `POST /v1/responses` answers a refused token with 200 and `status: "failed"` | Responses is not used; Responses apps reach the router through Chat Completions | live |
| `POST /v1/messages`, the Anthropic-compatible endpoint the Claude Code guide points at, answers a refused token with 401 `api_error` and the router's error body as its message | Messages is not used, as that would reach apps as a failed provider instead of a refused key; Messages apps reach the router through Chat Completions | live |
| `POST /v1/messages/count_tokens` answers without checking the token and isn't documented | token counts are refused before sending | live |
| `/v1/embeddings`, `/v1/images/generations` and `/v1/audio/*` answer 404 | `embeddings` is `None`, and images, speech and transcriptions are empty | live; the index says the OpenAI-compatible endpoint is for chat only |
