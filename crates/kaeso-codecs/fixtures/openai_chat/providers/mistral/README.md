# openai_chat / providers / mistral (Mistral)

Mistral, provider id `mistral`, base URL `https://api.mistral.ai`, `POST /v1/chat/completions` and `GET /v1/models` with `Authorization: Bearer`. API reference: https://docs.mistral.ai/api/ and its OpenAPI document https://docs.mistral.ai/openapi.yaml, read on 2026-10-19; SDK types: `mistralai/client-python` at commit `6229ed96cb5b`.

Every file under `responses/`, `streams/` and `errors/` was recorded live with `cargo xtask record` on 2026-10-19 (`RECORDED.md` in each folder names the request and model). The requests under `requests/` are written for Mistral: they use its own fields or show what it refuses. `recorded-invalid-api-key` was recorded with a placeholder key. The two files under `models/` are live `GET /v1/models` and `GET /v1/models/ministral-3b-latest` replies; the list keeps 6 of 54 models, unchanged otherwise.

## Chat profile

Mistral rejects every field it doesn't declare with `422 extra_forbidden`. The `chat` profile in `crates/kaeso-core/src/provider.rs` follows these recordings:

| setting | recorded source |
|---|---|
| `max_tokens()` | `errors/recorded-max-completion-tokens-legacy-null`: `max_completion_tokens` is forbidden; `max_tokens` works in every reply here |
| `system_role()` | `errors/recorded-plain-developer-string`: role `developer` is not one of `system`, `user`, `assistant`, `tool` |
| `random_seed()` | `errors/recorded-stop-string-seed-penalties` refuses `seed`; `responses/recorded-safe-prompt-random-seed` takes `random_seed`, which the OpenAPI document gives `minimum: 0` |
| `without_end_user()` | `errors/recorded-user-field`, `errors/recorded-safety-identifier-field` |
| `refusing([...])` | `errors/recorded-function-call-field`, `errors/recorded-functions-field`, `errors/recorded-logit-bias`, `errors/recorded-moderation-field`, `errors/recorded-prompt-cache-options-field`, `errors/recorded-prompt-cache-retention-field`, `errors/recorded-store-field`, `errors/recorded-verbosity-field`, `errors/recorded-web-search-options-field`; Mistral accepts `metadata`, `modalities`, `audio`, `prediction`, `prompt_cache_key` and `stream_options` (`streams/recorded-stream-include-usage.sse` was recorded with `stream_options.include_usage`, which the gateway sets on every stream) |
| `refusing_in_messages(["name", "refusal"])` | `errors/recorded-system-name-multiturn` (`name` on system and user messages), `errors/recorded-assistant-refusal-field`; `responses/recorded-tool-result-named` shows that a tool message may keep its `name` |

## Files

| file | what it covers |
|---|---|
| `requests/system-tool-choice-any.json`, `responses/recorded-system-tool-choice-any.json` | `tool_choice: "any"` (Mistral's own value, kept as `ToolChoice::Unknown`); a tool call with a 9-character id, `index` and `content: ""` |
| `requests/tool-result-named.json`, `responses/recorded-tool-result-named.json` | a tool result with `name` |
| `requests/safe-prompt-random-seed.json`, `responses/recorded-safe-prompt-random-seed.json` | `safe_prompt` and `random_seed` (Extras), penalties and `stop` |
| `requests/reasoning-effort-high.json`, `responses/recorded-reasoning-effort-high.json` | `content` as an array: a `thinking` chunk (`Block::Unknown`) and a `text` chunk; `completion_tokens_details.reasoning_tokens` |
| `requests/reasoning-effort-high-stream.json`, `streams/recorded-reasoning-effort-high-stream.sse` | thinking streamed as `delta.content` arrays (Extras), the answer as strings |
| `requests/magistral-medium.json`, `responses/recorded-magistral-medium.json` | `magistral-medium-latest` answers with a plain string |
| `responses/recorded-max-tokens-length.json`, `responses/recorded-prediction-doc.json`, `responses/recorded-tool-choice-function.json`, `responses/recorded-tool-choice-none.json` | canonical requests: usage with `prompt_tokens_details.cached_tokens`, `completion_tokens_details` and `service_tier` (Extras), `tool_calls: null` |
| `streams/recorded-stream-include-usage.sse`, `requests/stream-no-options.json` with `streams/recorded-stream-no-options.sse` | the padding field `p` on chunks (Extras); usage on the chunk with `finish_reason`, also without `stream_options`; `data: [DONE]` |
| `requests/stream-tool-call.json`, `streams/recorded-stream-tool-call.sse` | a whole tool call in one delta together with `finish_reason` and usage |
| `requests/model-not-found.json`, `errors/recorded-model-not-found.json` | an unknown model: HTTP 400, `type: invalid_model`, `code: "1500"` |
| `requests/prompt-mode-reasoning.json`, `errors/recorded-prompt-mode-reasoning.json` | `prompt_mode: "reasoning"` on `magistral-medium-latest`: HTTP 400, `type: invalid_request_invalid_args`, `code: "3051"` |
| `requests/invalid-api-key.json`, `errors/recorded-invalid-api-key.json` | a wrong key: HTTP 401, `{"detail": "Invalid API Key"}` |
| `errors/recorded-reasoning-cached-usage.json` | two errors in one 422: `service_tier: "priority"` and role `developer` |
| the other `errors/` files | the 422 `extra_forbidden` bodies the profile rests on, in both of Mistral's shapes: `{"detail": [...]}` for fields inside `messages`, and `{"object": "error", "message": {"detail": [...]}, "type": "invalid_request_error", ...}` for top-level fields |
| `models/list-recorded.json`, `models/retrieve-recorded.json` | Mistral's model cards: `capabilities`, `max_context_length`, `aliases`, `deprecation` and more (Extras) |

No Mistral error body has OpenAI's `error` object, so the Chat codec answers apps with an OpenAI error that quotes it; `openai_chat::tests::error_bodies_without_an_error_object_reach_apps_as_openai_errors_that_quote_them` checks that instead of the round trip.

Thinking stays where Mistral puts it: in a reply's `content` array as `Unknown` blocks, and in a stream as the delta's `delta.content` Extras. Apps in another format get the text without it; mapping it to their reasoning blocks is left for the reasoning design.

No 429 or 5xx body was recorded.
