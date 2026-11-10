# Fixtures: SiliconFlow (`siliconflow`) on `openai_chat`

SiliconFlow's international API, OpenAI-compatible Chat Completions at `POST https://api.siliconflow.com/v1/chat/completions`, with `Authorization: Bearer` and an `sk-` key. Kaeso's base URL is `https://api.siliconflow.com`; the China endpoint, which models.dev and KPMR list as `siliconflow-cn`, is `https://api.siliconflow.cn` and takes keys from there.

Not recorded from a successful call. The requests are the reasoning guide's own examples; the reply and the stream are built from the chat completions reference's response schema, with the thinking in `reasoning_content` as the guide reads it. The error was answered live, without a valid key.

## Sources (read 2026-11-10)

| Source | Address |
|---|---|
| Chat completions reference (OpenAPI: `ChatCompletionRequest`, `ChatCompletionVLMRequest`, `ChatCompletionResponse`, the error bodies) | https://docs.siliconflow.com/en/api-reference/chat-completions/chat-completions |
| Reasoning (`thinking_budget`, `max_tokens` for the answer alone, `reasoning_content`) | https://docs.siliconflow.com/en/userguide/capabilities/reasoning |
| Messages reference | https://docs.siliconflow.com/en/api-reference/chat-completions/messages |
| Error handling (`{"code", "message", "data"}`, the status codes) | https://docs.siliconflow.com/en/faqs/error-code |
| List models | https://docs.siliconflow.com/en/api-reference/models/get-model-list |

## Files

| File | What it covers | Provenance |
|---|---|---|
| `requests/reasoning-doc.json`, `responses/reasoning-doc.json` | the guide's request to `deepseek-ai/DeepSeek-R1` with `thinking_budget`; `message.reasoning_content` beside the answer | doc |
| `requests/reasoning-stream-doc.json`, `streams/reasoning-stream-doc.sse` | the guide's streamed request; `delta.reasoning_content`, then the answer, with `content: null` and `reasoning_content: null` beside the other field and the usage on the last chunk | doc |
| `requests/thinking-off.json` | `enable_thinking: false` for a Qwen3 model, with `min_p` and `top_k` | doc |
| `requests/invalid-api-key.json`, `errors/recorded-invalid-api-key.json` | 401 `{"code": 30014, "data": null, "message": "Token is invalid."}`; a request without a key gets the same | live, with a made-up key |
| `errors/model-does-not-exist-doc.json` | 400 `{"code": 20012, …}` for a model SiliconFlow doesn't serve | doc |

## Where SiliconFlow differs from OpenAI Chat Completions

| Difference | How KPP carries it | Source |
|---|---|---|
| thinking comes as `reasoning_content` | a reasoning block through the profile's `reasoning_fields(["reasoning_content"])` | reasoning guide |
| `enable_thinking` turns thinking on and off for Qwen3, GLM, Hunyuan and DeepSeek V3 models, `thinking_budget` caps it | an app's thinking reaches the model as `enable_thinking` and its budget as `thinking_budget` (`thinking_switch()`); a Chat app's own values go as written | reference |
| `max_tokens` limits only the answer, not the thinking | `max_tokens()`; the provider's `reasoning_beyond_output_limit` makes limits count on the model's output limit | reasoning guide |
| the roles are `system`, `user` and `assistant` | `developer` messages go as `system` (`system_role()`) | reference |
| no `user` or `safety_identifier` | the end user's id is left out (`without_end_user()`) | reference |
| `min_p`, `top_k` | request `Extras`; refused when the request goes to another dialect | reference |
| `video_url` and `audio_url` parts for Qwen3-Omni and Qwen3-VL | not sent: KPP's video blocks are refused, and audio goes as OpenAI's `input_audio` | reference |
| errors as `{"code", "message", "data"}`, without OpenAI's `error` object | `KppError` with SiliconFlow's message; the kind follows the status: 401 is a refused key | live; error handling |
| `GET /v1/models` needs a key | a model check spots a wrong key | live |
| `/v1/responses` and `/v1/messages/count_tokens` answer 404; `/v1/messages` documents only text and tool blocks, without thinking or images | Chat Completions only; token counts refused | live; Messages reference |
