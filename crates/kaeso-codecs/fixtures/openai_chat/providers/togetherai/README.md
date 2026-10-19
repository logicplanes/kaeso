# openai_chat / providers / togetherai

Together AI (`togetherai`), recorded live with `cargo xtask record openai_chat --provider togetherai` on 2026-10-19.

| | |
|---|---|
| API reference | https://docs.together.ai/reference/chat-completions-1 and https://docs.together.ai/docs/inference/openai-compatibility, read 2026-10-19 |
| SDK types | `togethercomputer/together-py@c55bb3003581d8be51f8eb102debf314cb3bcfe2`, `src/together/types/chat/` |
| Base URL | `https://api.together.ai` (the SDK's default `https://api.together.ai/v1`) |
| Models | `deepseek-ai/DeepSeek-V4-Flash-0731` (reasoning in `reasoning_content`), `Qwen/Qwen3.5-9B` (reasoning in `reasoning`), and `meta-llama/Meta-Llama-3.1-8B-Instruct-Turbo` for the refused non-serverless model |

What Together adds to the dialect, as seen in these files and the SDK types:

- Requests: `top_k`, `min_p`, `repetition_penalty`, `safety_model`, `context_length_exceeded_behavior`. They stay request `Extras` for Together and are refused in another format.
- Replies: `reasoning_content` or `reasoning` on the message and in deltas, `matched_stop` and `seed` on a choice, `text` on a stream choice and `token_id` in its delta, top-level `prompt`, `metadata` and `warnings`, and `usage.reasoning_tokens` beside `completion_tokens_details.reasoning_tokens`. All of them stay in `Extras`.
- Errors: OpenAI's envelope with Together's own `type` and `code` values, sometimes with a top-level `id`.

## requests

| file | what it covers |
|---|---|
| `context-length-truncate.json` | `context_length_exceeded_behavior: error` with a `max_tokens` above the context window |
| `non-serverless-model.json` | a model Together serves only on a dedicated endpoint |
| `reasoning-reply.json` | a reasoning model that answers in `reasoning` |
| `reasoning-stream.json` | the same, streamed with `stream_options.include_usage` |
| `safety-model.json` | `safety_model` |
| `sampling-extras.json` | `top_k`, `min_p`, `repetition_penalty` |
| `stream-usage-short.json` | a short stream with `stream_options.include_usage` |
| `tool-call-auto.json` | one function tool, `tool_choice: auto` |

## responses

| file | request | what it covers |
|---|---|---|
| `recorded-json-schema.json` | `openai_chat/requests/json-schema.json` | Structured Outputs, `reasoning_content: null` |
| `recorded-reasoning-reply.json` | `reasoning-reply.json` | `reasoning`, `prompt: []`, `metadata`, `seed: null`, `tool_calls: []`, `usage.reasoning_tokens`, `finish_reason: length` |
| `recorded-safety-model.json` | `safety-model.json` | a reply with `safety_model` set |
| `recorded-sampling-extras.json` | `sampling-extras.json` | `reasoning_content`, `completion_tokens_details.reasoning_tokens` |
| `recorded-stop-string-seed-penalties.json` | `openai_chat/requests/stop-string-seed-penalties.json` | `matched_stop`, a message with only `reasoning_content` |
| `recorded-tool-call-auto.json` | `tool-call-auto.json` | one tool call, no `content` |
| `recorded-tools-parallel.json` | `openai_chat/requests/tools-parallel.json` | text and two tool calls |

## streams

| file | request | what it covers |
|---|---|---|
| `recorded-reasoning-stream.sse` | `reasoning-stream.json` | `reasoning` deltas, `text` and `delta.token_id`, usage chunk with `reasoning_tokens`, `[DONE]` |
| `recorded-stream-usage-short.sse` | `stream-usage-short.json` | `reasoning_content` deltas, usage chunk with empty `choices`, `[DONE]` |

## errors

| file | status | what it covers |
|---|---|---|
| `recorded-context-length-truncate.json` | 400 | `type: Bad Request`, `code: start_generation_failed` |
| `recorded-non-serverless-model.json` | 400 | top-level `id`, `code: model_not_available` |

Each subfolder's `RECORDED.md` lists the model and time of every recording.
