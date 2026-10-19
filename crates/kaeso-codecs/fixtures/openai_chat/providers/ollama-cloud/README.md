# Fixtures: Ollama Cloud (`ollama-cloud`)

Ollama Cloud serves OpenAI Chat Completions at `https://ollama.com/v1/chat/completions` with `Authorization: Bearer <key>`. Kaeso has no Ollama Cloud key, so nothing here is a `cargo xtask record` recording: every file is either a live reply to a request that needs no key, or a payload copied from Ollama's own documentation and tests.

## Sources

| Source | Version |
|---|---|
| Docs: [OpenAI compatibility](https://docs.ollama.com/api/openai-compatibility), [Cloud](https://docs.ollama.com/cloud), [Authentication](https://docs.ollama.com/api/authentication), [Errors](https://docs.ollama.com/api/errors), [Quickstart](https://docs.ollama.com/quickstart) | read 2026-10-19; their source is `docs/` in `ollama/ollama` |
| `ollama/ollama` (`openai/openai.go`, `middleware/openai.go` and their tests) | commit `36a6e994d2f6c4ab94b3555185154d069e5dba87` (2026-10-17) |
| Live `https://ollama.com`, no key | 2026-10-19 00:36 UTC, server build `e174f4d23706a7f5eb27165058e7b2fd062fe335` (`x-build-commit`) |

## Files

| File | What it covers | Provenance |
|---|---|---|
| `models/list-live.json` | `GET /v1/models`: 22 cloud models, ids with `:` tags (`gpt-oss:20b`, `deepseek-v4.1-flash:1017`), `owned_by: "ollama"`, no `shutdown_date` | live, unauthenticated (the endpoint needs no key), verbatim |
| `models/retrieve-live.json` | `GET /v1/models/gpt-oss:20b`, the colon unescaped in the path | live, unauthenticated, verbatim |
| `errors/unauthorized-live.json` | 401 for `POST /v1/chat/completions` without a key: OpenAI error shape with `type: "api_error"`, `param` and `code` `null` | live, unauthenticated, verbatim |
| `errors/model-not-found-live.json` | 404 for `GET /v1/models/no-such-model:1b`: `type: "not_found_error"`, which OpenAI does not use | live, unauthenticated, verbatim |
| `requests/quickstart-doc.json` | the Chat Completions body of the cloud quickstart (`gemma4:31b`) | doc: Quickstart, "OpenAI Chat Completions" tab, verbatim |
| `requests/assistant-reasoning-tool-call.json` | an assistant turn with Ollama's `reasoning` field (its thinking) next to `tool_calls` | test: `TestChatMiddleware` "chat handler with tools and thinking content" in `middleware/openai_test.go`, verbatim |
| `requests/reasoning-object-effort.json` | Ollama's `reasoning: {effort}` object next to `reasoning_effort` | test: the body `TestThinkingCompatibilityScope` sends for "chat nested precedence/generic" in `middleware/thinking_test.go` |

The `.meta.json` files hold only the status; the live replies' other headers (cookies, request and trace ids) are not kept.

## Differences from OpenAI Chat Completions

- `reasoning` on an assistant message (request), on `choices[].message` and on stream `delta`s (reply) carries the model's thinking (`openai/openai.go`). Kaeso keeps it as an extra bound to Chat Completions: a Chat Completions app gets it back unchanged, a request that carries it is refused toward another format, and a reply's `reasoning` does not reach an app in another format, as with any unknown reply field. Mapping it to thinking or reasoning items is left to the cross-format reasoning design.
- `reasoning: {effort}` is accepted next to `reasoning_effort` and wins over it. Kaeso keeps the object as an extra.
- Only `max_tokens` limits the output; `max_completion_tokens` is not read (`openai/openai.go`). The Chat Completions encoder sends `max_completion_tokens` for requests from other formats, so their limit is ignored by Ollama Cloud.
- `tool_choice`, `logit_bias`, `user` and `n` are not supported (OpenAI compatibility page); the server decodes requests without rejecting unknown fields, so they are ignored rather than refused.
- Replies carry `system_fingerprint: "fp_ollama"`, ids `chatcmpl-<n>`, and, on the usage chunk of a stream with `include_usage`, a `timings` object (`openai/openai.go`). No real reply or stream body exists in the docs or tests, so there is no fixture for them.
- Errors use the OpenAI shape, with `type` `invalid_request_error` (400), `not_found_error` (404) or `api_error` (everything else, 401 included). Kaeso takes the kind from the status.
- `/v1/embeddings` answers 404 on ollama.com (live, 2026-10-19), so the provider has no embeddings.
