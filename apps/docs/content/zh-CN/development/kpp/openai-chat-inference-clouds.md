---
title: Chat Completions 推理云
description: 托管开放模型的推理云为 Chat Completions 增加了什么，以及编解码器如何适配它们。
order: 6
nav: 推理云
---

托管开放模型的推理云使用 [OpenAI Chat Completions](openai-chat.md)，但各有差异。在修改它们的配置之前，请先在这里查看每个推理云增加的内容；它们的配置在[服务商配置](openai-chat-providers.md#服务商配置)表中，其他服务商见 [Chat Completions 服务商](openai-chat-providers.md)。

它们的 fixture 位于 `fixtures/openai_chat/providers/<id>/`（[fixtures](fixtures.md)）。

## Fireworks AI

来源：Fireworks 的 [Create Chat Completion](https://docs.fireworks.ai/api-reference/post-chatcompletions) 参考（其 OpenAPI 文档）以及 [OpenAI compatibility](https://docs.fireworks.ai/tools-sdks/openai-compatibility) 和 [reasoning](https://docs.fireworks.ai/guides/reasoning) 页面，2026-11-09 阅读。由于没有可用的密钥，`fixtures/openai_chat/providers/fireworks-ai/` 中的回复和流都据此构建；错误是在没有有效密钥的情况下实时返回的。Kaeso 的基础 URL 为 `https://api.fireworks.ai/inference`，使用 `Authorization: Bearer` 和 `fw_` 开头的密钥。

| Fireworks 字段 | KPP | 说明 |
|---|---|---|
| `message.reasoning_content`、`delta.reasoning_content` | `Block::Reasoning` | 通过配置的 `reasoning_fields`。其他格式的应用发回的思考内容以 `reasoning_content` 到达 Fireworks，这正是其 reasoning 指南在工具调用时所要求的。文档没有描述 `content` 中的 `<think>` 标签。 |
| 带 `xhigh`、`max`、布尔值或Token 数的 `reasoning_effort`；Anthropic 形式的 `thinking`；`reasoning_history` | `reasoning.effort`、extras | 目录中的推理强度决定其他格式发送什么；Chat 应用自己的值原样发送。 |
| `top_k`、`min_p`、`typical_p`、`repetition_penalty`、`mirostat_*`、`context_length_exceeded_behavior`、`prompt_cache_isolation_key`、`perf_metrics_in_response`、`raw_output` 及 Fireworks 的其他请求字段 | extras | 当请求发往其他方言时被拒绝，与其他仅限 Chat 的字段相同。 |
| 超出上下文窗口剩余空间的 `max_tokens` | 发送 | Fireworks 会将其调低而不是拒绝，除非请求设置 `context_length_exceeded_behavior: "error"`。 |
| `user` | `user` | Fireworks 的 schema 没有 `safety_identifier`，因此配置以 `user` 发送终端用户的 ID（`end_user_as_user()`）。 |
| `video_url` 部分 | `Block::Video` | `with_video_url()`；Fireworks 的视频模型接受它（`requests/video-url`）。 |
| 回复或结束 chunk 中的 `perf_metrics` | extras | 发往其他方言时省略。 |
| 默认在最后一个 `choices: []` 的 chunk 中发送 usage | `Usage` | 即使没有 `stream_options.include_usage` 也会发送。 |
| OpenAI `error` 对象中的错误，带大写的 `code`（例如 `UNAUTHORIZED`）、`type: "error"` 和顶层的 `request_id` | `KppError` | 类别由状态码决定：401 表示密钥被拒绝（`errors/recorded-invalid-api-key`）。 |
| `GET /v1/models` | `ModelList` | 错误的密钥返回 401（`errors/list-invalid-api-key`），因此模型检查能发现它。Fireworks 没有记录此路径上的列表；控制台从 KPMR 获取其模型。 |

Fireworks 没有记录图像、语音、转录、视频或Token 计数端点，因此这些保持为空，`count_tokens` 为 `None`。它的 Responses API 以 `prompt_tokens` 和 `completion_tokens` 报告用量，而不是 OpenAI 的字段名，因此 Kaeso 不向它发送 Responses；Responses 应用通过 Chat Completions 访问 Fireworks。

## Nebius Token Factory

来源：Nebius 的 [OpenAPI document](https://api.tokenfactory.nebius.com/openapi.json)（版本 `20261105-4c58718c5`）、其[文本生成](https://docs.tokenfactory.nebius.com/api-reference/examples/text-generation)示例和[概览](https://docs.tokenfactory.nebius.com/ai-models-inference/overview)，2026-11-09 阅读。由于没有可用的密钥，`fixtures/openai_chat/providers/nebius/` 中的回复和流都据此构建，其中一个是文档自己的示例；错误是在没有有效密钥的情况下实时返回的。Kaeso 的基础 URL 为 `https://api.tokenfactory.nebius.com`，使用 `Authorization: Bearer`。

| Nebius 字段 | KPP | 说明 |
|---|---|---|
| `message.reasoning_content`、`delta.reasoning_content`、`message.reasoning`、`delta.reasoning` | `Block::Reasoning` | 通过配置的 `reasoning_fields`；使用哪个字段取决于模型，与 Together AI 相同。 |
| `developer` 角色 | `Role::System` | Nebius 只列出 `system`、`user`、`assistant` 和 `tool`，因此 `system_role()` 发送 `system`。 |
| `user` | `user` | Nebius 的 schema 没有 `safety_identifier`，因此终端用户的 ID 以 `user` 发送（`end_user_as_user()`）。 |
| `video_url` 部分 | `Block::Video` | `with_video_url()`（`requests/video-url`）。 |
| `top_k`、`repetition_penalty` 等 vLLM 采样字段，`extra_body`，消息上的 `partial` 和 Kimi 的 `tools` | extras | 当请求发往其他方言时被拒绝，与其他仅限 Chat 的字段相同。 |
| 每个回复中的 `service_tier`、`stop_reason`、`function_call: null`、`tool_calls: []`、`system_fingerprint: null` | extras | 发往其他方言时省略（`responses/text-generation-doc`）。 |
| 带 JSON schema 的 `response_format` | 发送 | Nebius 的说明只允许 `json_object` 和 `text`；schema 由 Nebius 自行响应。 |
| 带 OpenAI `error` 对象和顶层 `detail` 的错误 | `KppError` | 类别由状态码决定：401 表示密钥被拒绝（`errors/recorded-invalid-api-key`）。 |
| `GET /v1/models` | `ModelList` | 错误的密钥返回只有 `{"detail": "…"}` 的 401（`errors/list-invalid-api-key`），应用会以 OpenAI 的格式收到 Nebius 的消息，因此模型检查能发现它。 |

Nebius 不提供图像、音频、Messages 或Token 计数：它的 OpenAPI 文档中没有这样的路径，`/v1/images/generations`、`/v1/audio/transcriptions` 和 `/v1/messages` 返回 404。它的 Responses API 不被使用：Responses 应用通过 Chat Completions 访问 Nebius。

## SambaNova

来源：SambaNova 的 [OpenAPI document](https://raw.githubusercontent.com/sambanova/sambanova-inference-api-spec/refs/heads/main/openapi.documented.json)（SambaNova cloud API 1.2.0）及其 [OpenAI compatibility](https://docs.sambanova.ai/docs/en/features/openai-compatibility)、[文本生成](https://docs.sambanova.ai/docs/en/features/text-generation)和 [API error codes](https://docs.sambanova.ai/docs/en/api-reference/using-the-api/api-error-codes) 页面，2026-11-09 阅读。由于没有可用的密钥，`fixtures/openai_chat/providers/sambanova/` 包含该文档自己的示例回复和示例流、据此构建的回复、SambaCloud 无需密钥即提供的模型列表，以及在没有有效密钥的情况下实时返回的错误。Kaeso 的基础 URL 为 `https://api.sambanova.ai`，使用 `Authorization: Bearer`。

| SambaNova 字段 | KPP | 说明 |
|---|---|---|
| `message.reasoning`、`delta.reasoning`、`channel` | `Block::Reasoning`、extras | 通过配置的 `reasoning_fields`。assistant 消息接受发回的 `reasoning`。 |
| `chat_template_kwargs`（`enable_thinking`）、`top_k`、`repetition_penalty` | extras | DeepSeek-V3.2 和 Gemma 4 只有在 `enable_thinking` 时才思考，而只有 Chat 应用能发送它。当请求发往其他方言时被拒绝。 |
| `frequency_penalty`、`presence_penalty` | 拒绝 | SambaNova 接受并忽略它们，因此配置在发送之前拒绝它们。 |
| 终端用户的 ID、Responses 应用的 `prompt_cache_key` | 省略 | 请求 schema 两者都没有，并拒绝未声明的属性（`without_end_user()`、`without_cache_key()`）。其他未声明的字段原样发送，由 SambaNova 自己返回 400。 |
| `video_url` 部分 | `Block::Video` | `with_video_url()`；Gemma 4 接受视频。 |
| 浮点数形式的 `created`、`system_fingerprint` | extras | `responses/palindrome-poem-doc`。 |
| 带 SambaNova 计时字段的 `usage`（`time_to_first_token`、`total_latency`、`acceptance_rate`、`is_last_response` 等） | `Usage`、`raw` usage | 出现在回复中，以及流的最后一个 `choices: []` 的 chunk 中（`streams/stream-length-doc`）。 |
| 带 OpenAI `error` 对象和顶层 `request_id` 的错误；410 `model_deprecated` | `KppError` | 类别由状态码决定：401 表示密钥被拒绝，410 表示未找到。 |
| `GET /v1/models`、`GET /v1/models/{id}` | `ModelList`、`ModelInfo` | 无需密钥即可返回，带 `context_length`、`max_completion_tokens` 和 `pricing`（`models/list-recorded`），因此模型检查无法发现错误的密钥。 |

SambaCloud 不提供嵌入和 Whisper；SambaNova 只为 SambaStack 记录了这两项，SambaCloud 对它们的模型返回 `model_not_found`（`errors/recorded-embeddings-model-not-found`）。它的 Responses API 不被使用：Responses 应用通过 Chat Completions 访问 SambaNova。

## Scaleway

来源：Scaleway 的 [Generative APIs reference](https://www.scaleway.com/en/developers/api/generative-apis/)、其 [reasoning 指南](https://www.scaleway.com/en/docs/generative-apis/how-to/query-reasoning-models/)、[音频指南](https://www.scaleway.com/en/docs/generative-apis/how-to/query-audio-models/)和[错误页面](https://www.scaleway.com/en/docs/generative-apis/api-cli/understanding-errors/)，2026-11-09 阅读。由于没有可用的密钥，`fixtures/openai_chat/providers/scaleway/` 中的回复和流都据此构建，流的前几个 chunk 来自 reasoning 指南自己的示例；错误是在没有有效密钥的情况下实时返回的。Kaeso 的基础 URL 为 `https://api.scaleway.ai`，使用 `Authorization: Bearer` 和 IAM 私密密钥，访问该密钥的默认项目。

| Scaleway 字段 | KPP | 说明 |
|---|---|---|
| `message.reasoning_content`、`delta.reasoning` | `Block::Reasoning` | 通过配置的 `reasoning_fields`。只有旧模型会把思考内容放在 `content` 的 `<think>` 标签中，而这些模型已经停止服务。 |
| `developer` 角色 | `Role::System` | Scaleway 只列出 `system`、`user`、`assistant` 和 `tool`，因此 `system_role()` 发送 `system`。 |
| 终端用户的 ID | 省略 | 请求 schema 没有 `user` 或 `safety_identifier`（`without_end_user()`）。 |
| `wav` 或 `mp3` 格式的 `input_audio` 部分 | `Block::Audio` | 与 OpenAI 相同（`requests/input-audio`）。 |
| 流 choice 上的 `token_ids` | extras | 发往其他方言时省略。 |
| `error.code` 中以数字表示 HTTP 状态码、`type` 例如 `unauthorized` 或 `forbidden`、`param: ""` 的错误 | `KppError` | 类别由状态码决定。Scaleway 不认识的密钥返回 403 `forbidden`（`errors/recorded-invalid-api-key`），与没有 Generative APIs 权限的密钥相同，因此应用会将其视为不允许发出该请求的密钥。 |
| 旧的错误格式 `{"object": "error", "message", "type", "param", "code"}` | `KppError` | 旧模型仍会发送它（`errors/legacy-model-error-doc`）；应用会收到带有其消息的 OpenAI 错误。 |

Scaleway 不提供 Messages、图像、语音或 Token 计数：`/v1/messages`、`/v1/images/generations` 和 `/v1/audio/speech` 返回 404 `route_not_found`。它的 Responses API 无状态，只支持函数工具，不被使用：Responses 应用通过 Chat Completions 访问 Scaleway。
