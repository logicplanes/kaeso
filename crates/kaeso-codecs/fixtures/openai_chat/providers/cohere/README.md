# Fixtures: Cohere (`cohere`) on `openai_chat`

Cohere's Compatibility API, `POST https://api.cohere.ai/compatibility/v1/chat/completions` and `GET /v1/models`, with `Authorization: Bearer`. Kaeso's base URL is `https://api.cohere.ai/compatibility`. Chat Completions is Cohere's `generation` dialect.

## Sources (read 2026-10-30)

| Source | Address |
|---|---|
| Compatibility API: supported and unsupported parameters, structured outputs, tools, `reasoning_effort` | https://docs.cohere.com/docs/compatibility-api |
| API keys | https://dashboard.cohere.com/api-keys |

Every file under `responses/`, `streams/` and `errors/` was recorded live with `cargo xtask record` on 2026-10-30 (`RECORDED.md` in each folder names the request and model), except the three model-listing files named below, recorded the same day with `curl`. `errors/recorded-invalid-api-key.json` and `errors/list-invalid-api-key.json` were recorded with a placeholder key of zeros; Cohere's message shows its last four characters. Nothing in a recording was changed.

## Chat profile

| setting | recorded source |
|---|---|
| `stop_list()` | `errors/recorded-stop-string`: a `stop` string is refused with 422 (`should be of type []Object`); `responses/recorded-sampling` takes a list |
| `reasoning_fields(["reasoning_content"])` | `responses/recorded-reasoning`, `streams/recorded-reasoning-stream` |
| `refusing([...])` | the 422 bodies `errors/recorded-logit-bias-field`, `recorded-metadata-field`, `recorded-modalities-field`, `recorded-parallel-tool-calls-field`, `recorded-prediction-field`, `recorded-service-tier-field`, `recorded-store-field` and `recorded-top-logprobs-field`; `audio` needs `modalities`, which Cohere refuses. Cohere answers fields it doesn't declare with 200 and ignores them: `responses/recorded-functions-field` and `responses/recorded-web-search-options-field` answer as if the field weren't there, so `functions`, `function_call`, `web_search_options` and `verbosity` are refused too |

Cohere takes the `developer` role (`responses/recorded-developer-role`), `max_completion_tokens`, `seed`, an assistant prefill (`responses/recorded-assistant-prefill` continues it), `user` and `safety_identifier`, so the profile keeps OpenAI's spellings for them.

## Files

| file | what it covers |
|---|---|
| `requests/basic.json`, `responses/recorded-basic.json` | a system and a user message with `max_tokens`; usage with zero `audio_tokens` and prediction token details |
| `requests/developer-role.json`, `responses/recorded-developer-role.json` | a `developer` message and `max_completion_tokens` |
| `requests/sampling.json`, `responses/recorded-sampling.json` | `temperature`, `top_p`, `seed`, `frequency_penalty` and `stop` as a list |
| `requests/assistant-prefill.json`, `responses/recorded-assistant-prefill.json` | a conversation ending with an assistant message, which Cohere continues |
| `requests/structured-output.json`, `responses/recorded-structured-output.json` | `response_format` with a strict JSON schema |
| `requests/tools.json`, `requests/tools-required.json`, `responses/recorded-tools.json`, `responses/recorded-tools-required.json` | a function tool with `tool_choice` `auto` and `required`; Cohere's tool call ids (`get_weather_…`) and `content: null` |
| `requests/tool-result.json`, `responses/recorded-tool-result.json` | a tool result turn |
| `requests/image.json`, `responses/recorded-image.json` | an `image_url` part with a data URL on `command-a-vision-07-2025` |
| `requests/reasoning.json`, `requests/reasoning-off.json`, `responses/recorded-reasoning.json`, `responses/recorded-reasoning-off.json` | `reasoning_effort` `high`, with `message.reasoning_content` and `reasoning_tokens`, and `none`, without them |
| `requests/stream.json`, `streams/recorded-stream.sse` | a stream with `include_usage`: `usage: null` on every chunk, then a last chunk with `choices: []` and the usage |
| `requests/stream-no-options.json`, `streams/recorded-stream-no-options.sse` | a stream without `stream_options`: no usage and no `usage` key |
| `requests/tools-stream.json`, `streams/recorded-tools-stream.sse` | a streamed tool call: name and id first, then the arguments whole |
| `requests/reasoning-stream.json`, `streams/recorded-reasoning-stream.sse` | `delta.reasoning_content`, then content |
| `requests/functions-field.json`, `requests/web-search-options-field.json`, `responses/recorded-functions-field.json`, `responses/recorded-web-search-options-field.json` | fields Cohere ignores with 200 |
| the `requests/*-field.json` files with an `errors/recorded-*-field.json` | the 422 `unprocessable entity: … is not supported` bodies the profile rests on |
| `requests/stop-string.json`, `errors/recorded-stop-string.json` | 422 for a `stop` string |
| `requests/reasoning-effort-medium.json`, `errors/recorded-reasoning-effort-medium.json` | 422: only `none` and `high` |
| `requests/penalties-together.json`, `errors/recorded-penalties-together.json` | 400: `frequency_penalty` with `presence_penalty` |
| `requests/max-tokens-over-limit.json`, `errors/recorded-max-tokens-over-limit.json` | 400 with `error_type: "TOO_MANY_TOKENS"` |
| `requests/image-on-text-model.json`, `errors/recorded-image-on-text-model.json` | 400: an image to a model without vision |
| `requests/file-part.json`, `errors/recorded-file-part.json` | 422: `unrecognized content type 'file'` |
| `requests/unknown-model.json`, `errors/recorded-unknown-model.json` | 404 for an unknown model |
| `requests/invalid-api-key.json`, `errors/recorded-invalid-api-key.json` | 401 for a wrong key |
| `errors/recorded-no-valid-response.json` | 422 `NO_VALID_RESPONSE_GENERATED`, which Cohere once answered to `requests/stream.json`; the next try streamed |
| `models/list-recorded.json`, `models/retrieve-recorded.json` | `GET /v1/models` (all 37 models, unchanged) and `GET /v1/models/command-a-03-2025`, recorded with `curl` |
| `errors/list-invalid-api-key.json`, `errors/retrieve-not-found.json` | the listing's 401 for a wrong key and `GET /v1/models/kaeso-example-no-such-model`'s 404, recorded with `curl` |

No Cohere error body has OpenAI's `error` object: they are `{"id", "message"}`, sometimes with `error_type`. Apps get an OpenAI error that quotes Cohere's message with the status; `openai_chat::tests::cohere_error_bodies_reach_apps_with_cohere_message_and_status` checks every body in `errors/` instead of the round trip. No 429 or 5xx body was recorded.
