---
title: Chat Completions 推理云
description: 托管开放模型的推理云为 Chat Completions 增加了什么，以及编解码器如何适配它们。
order: 6
nav: 推理云
---

托管开放模型的推理云使用 [OpenAI Chat Completions](openai-chat.md)，但各有差异。在修改它们的配置之前，请先在这里查看每个推理云增加的内容；它们的配置在[服务商配置](openai-chat-providers.md#服务商配置)表中，其他服务商见 [Chat Completions 服务商](openai-chat-providers.md)。

它们的 fixture 位于 `fixtures/openai_chat/providers/<id>/`（[fixtures](fixtures.md)）。

## Amazon Bedrock

来源：AWS 关于[端点](https://docs.aws.amazon.com/bedrock/latest/userguide/endpoints.html)、[Chat Completions](https://docs.aws.amazon.com/bedrock/latest/userguide/inference-chat-completions.html)、[Messages](https://docs.aws.amazon.com/bedrock/latest/userguide/inference-messages-api.html) 和 [API 密钥](https://docs.aws.amazon.com/bedrock/latest/userguide/api-keys.html)的指南及其 [API 兼容性](https://docs.aws.amazon.com/bedrock/latest/userguide/models-api-compatibility.html)表，2026-11-14 阅读。未通过成功的调用测试：AWS 未公开这些路径的任何回复，因此 `fixtures/openai_chat/providers/amazon-bedrock/` 中是指南里的请求，以及用虚构密钥实时得到的错误。每个密钥调用一个区域，即 `https://bedrock-runtime.{region}.amazonaws.com`，使用 `Authorization: Bearer`；`provider.rs` 中该服务商的 `Regional` 条目列出各区域，并为每种方言指定其在该主机上的 API 根路径。

| Amazon Bedrock 字段 | KPP | 说明 |
|---|---|---|
| `/openai/v1/chat/completions` 上的 Chat Completions | `EgressCtx::api_root` | `/openai/v1` 取代 `/v1`；请求体与 OpenAI 相同。Claude 模型改为发送到 `/anthropic/v1/messages`（见 [Messages 服务商](anthropic-messages-providers.md#amazon-bedrock)）。 |
| `amazon-bedrock-guardrailConfig` | extras | Bedrock 自有的防护规则请求体字段（`requests/guardrail-config-doc`）；请求发往其他方言时会被拒绝。 |
| 401 `permission_denied_error`，`code: "access_denied"` | `KppError` | 按状态码为 `Authentication`；格式错误、未签名和未知的密钥各有自己的消息（`errors/recorded-*-key*`）。 |
| 带 `x-amzn-ErrorType` 的 `{"Message": …}` | `KppError` | bedrock-runtime 回复其自有操作时使用的格式；消息从 `Message` 读取，类型按状态码（`errors/recorded-native-invalid-api-key`）。 |
| `GET /openai/v1/models` | — | 不提供（404 `UnknownOperationException`），因此模型检查无法确定，也不发送任何请求。 |

目录未注明格式的模型，Bedrock 只通过其自有 API InvokeModel 和 Converse 提供，KPP 不读取它们的 AWS 事件流，因此这类模型会在发送前被拒绝。Bedrock 的 Responses 路径不被使用：Responses 应用通过 Chat Completions 访问它。Kaeso 自行估算 Token 计数：bedrock-runtime 只在 `/model/{modelId}/count-tokens` 以自有格式计数。

## Baseten

来源：Baseten 的 [Model APIs](https://docs.baseten.co/inference/model-apis/overview) 概览、其 Chat Completions、Messages 和 Responses 的 OpenAPI 文档、[reasoning](https://docs.baseten.co/inference/model-apis/reasoning) 和[流式](https://docs.baseten.co/inference/streaming)指南，以及[推理错误](https://docs.baseten.co/inference/errors)页面，2026-11-10 阅读。未经成功调用测试：`fixtures/openai_chat/providers/baseten/` 中的回复是 reasoning 指南自己的示例，流是流式指南自己的摘录，并按 schema 补全；错误是在没有有效密钥的情况下实时返回的。Kaeso 的基础 URL 为 `https://inference.baseten.co`，使用 `Authorization: Bearer`。

| Baseten 字段 | KPP | 说明 |
|---|---|---|
| `message.reasoning_content` | `Block::Reasoning` | 通过配置的 `reasoning_fields`。应用发回的思考内容会以 `reasoning_content` 发给 Baseten，Beam 要求之前的 assistant 消息带有它。 |
| `chat_template_args`、带有 `max_tokens` 的 `reasoning`、`top_k`、`min_p`、`repetition_penalty` 以及 Baseten 的其他请求字段 | extras | 请求发往其他方言时会被拒绝，与其他仅限 Chat 的字段相同。 |
| `max_completion_tokens` | `max_tokens` | schema 只有 `max_tokens`（`max_tokens()`）。 |
| `developer` 角色 | `Role::System` | schema 列出 `system`、`user`、`assistant` 和 `tool`，因此 `system_role()` 发送 `system`。 |
| `user` | `user` | schema 没有 `safety_identifier`，因此终端用户的 ID 以 `user` 发送（`end_user_as_user()`）。 |
| `{"error": "<message>"}` 形式的错误 | `KppError` | 类别由状态码决定：401 表示密钥被拒绝（`errors/recorded-invalid-api-key`）。 |
| 没有 `[DONE]` 就结束的流 | `KppError` | Baseten 可能在没有错误事件的情况下提前结束流；应用会收到流解码器的错误，说明流在回复完成前就结束了。 |

Baseten 的 Messages 端点不被使用：它的 schema 只接受文本和工具块，不接受思考内容或图像。它不存储任何内容的 Responses API 也不被使用：Messages 和 Responses 应用通过 Chat Completions 访问 Baseten。Model APIs 不提供嵌入、图像、音频或 Token 计数。

## Cerebras

来源：Cerebras 的 [Chat Completions 参考](https://inference-docs.cerebras.ai/api-reference/chat-completions)和 [OpenAI 兼容](https://inference-docs.cerebras.ai/resources/openai)页面（2026-11-02 查阅），以及 `fixtures/openai_chat/providers/cerebras/` 中使用 `gpt-oss-120b` 和 `qwen-3.8-27b` 的实时录制。Kaeso 的基础 URL 为 `https://api.cerebras.ai`，接受 `Authorization: Bearer`。

| Cerebras 字段 | KPP | 说明 |
|---|---|---|
| `message.reasoning`、`delta.reasoning` | `Block::Reasoning` | 通过配置的 `reasoning_fields`，与 Groq 相同。其他格式的应用发回的思考内容以 `reasoning` 到达 Cerebras，Cerebras 接受它（`responses/recorded-reasoning-sent-back`）。Chat 应用的 `reasoning_content`（例如 Grok CLI 发送的）会被 Cerebras 以 400 拒绝，因此同样以 `reasoning` 发送（`replay_reasoning_as_read()`）。 |
| `time_info`、`system_fingerprint`、`choices[].reasoning_logprobs` | extras | 发往其他方言时省略。 |
| `completion_tokens_details.accepted_prediction_tokens` 和 `rejected_prediction_tokens`、`prompt_tokens_details.image_tokens` | `raw` 用量 | 缓存 token 和推理 token 按[用量](openai-chat.md#用量completionusage)类型化。 |
| 结束数据块上的 `usage` 和 `time_info`，即使没有 `stream_options.include_usage` | `Usage` | Cerebras 从不发送 `choices: []` 的数据块。 |
| `reasoning_effort` | `reasoning.effort` | `gpt-oss-120b` 接受 `low`、`medium` 和 `high`，`qwen-3.8-27b` 还接受 `none`，与 KPMR 所列一致；其他强度由 Kaeso 在发送前拒绝，Cerebras 也会以 400 回应（`errors/recorded-reasoning-effort-*`）。`max_completion_tokens` 包含推理。 |
| `reasoning_format`、`clear_thinking`（请求） | extras | 请求发往其他方言时被拒绝，与其他仅限 Chat 的字段相同。 |
| Cerebras 未声明的字段，例如 `safety_identifier`、`metadata`、`store` 或 `web_search_options` | 发送 | Cerebras 以 400 `property '…' is unsupported` 回应，该错误会到达应用。配置把终端用户的 ID 以 `user` 发送（`end_user_as_user()`），因此 Claude Code 的 `metadata.user_id` 会到达那里。 |
| `logit_bias`、`logprobs`、`top_logprobs`、两种惩罚参数、`seed`、`stop`、`prompt_cache_key`、`parallel_tool_calls`、`service_tier`、`prediction` | 发送 | Cerebras 接受它们（`responses/recorded-logit-bias-honoured` 显示偏置生效）。`stop` 序列也会结束推理，因此回复可能没有 `content`（`responses/recorded-sampling`）。 |
| `developer` 角色、第一轮之后的 `system` 消息 | `Role::System` | Qwen 的聊天模板以 400 拒绝两者。`system_role()` 发送 `system`；对于其他格式的应用（例如 Claude Code 或 Codex）的后续系统消息，`leading_system_only()` 把其文本移入下一个用户轮次。 |
| 最后一条 assistant 消息 | 拒绝 | Cerebras 会重新回答并重复它（`responses/recorded-prefill`），因此 `without_prefill()` 在发送前拒绝它。 |
| assistant 消息上的 `refusal`，包括 `null` | 拒绝 | Cerebras 以 400 回应（`errors/recorded-assistant-refusal`）；配置在发送前拒绝它。 |
| 没有 `parameters` 的函数 | `parameters` | `with_tool_parameters()` 发送 `gpt-oss-120b` 的聊天模板要求的空对象 schema（`errors/recorded-tool-without-parameters`）。 |
| `gpt-oss-120b` 上同时带 `tools` 和 `response_format`、以 `https` URL 提供的图像，或发给不接受图像的模型的图像 | 发送 | Cerebras 以 400 回应（`errors/recorded-tools-with-response-format`、`errors/recorded-image-url`、`errors/recorded-image-gpt-oss`），该错误会到达应用。 |
| 顶层带 OpenAI 的 `message`、`type`、`param` 和 `code`，但没有 `error` 对象的错误 | `KppError` | 消息和 `param` 来自 Cerebras，类别取决于状态码：错误的密钥（401 `wrong_api_key`）是被拒绝的密钥，账户的每分钟 token 限制（429 `too_many_tokens_error`，`retry-after: 60`）是速率限制。Chat 应用以 OpenAI 的形式收到错误。 |
| `GET /v1/models` | `ModelList` | 该密钥可调用的模型，带 `created: 0` 和 `owned_by: "Cerebras"`；错误的密钥得到 401，因此模型检查能发现它。 |

Cerebras 没有 Responses、Messages、embeddings 或 token 计数端点（都返回 404），因此 `embeddings` 和 `count_tokens` 为 `None`。

## Azure

来源：Azure 的 v1 OpenAPI 文档（[azure-rest-api-specs](https://github.com/Azure/azure-rest-api-specs/tree/main/specification/ai/data-plane/OpenAI.v1) 中的 `azure-v1-v1-generated.json`），以及 [API 生命周期](https://learn.microsoft.com/en-us/azure/foundry/openai/api-version-lifecycle)、[端点](https://learn.microsoft.com/en-us/azure/foundry/foundry-models/concepts/endpoints)、[内容流式传输](https://learn.microsoft.com/en-us/azure/foundry/openai/concepts/content-streaming)和[内容筛选](https://learn.microsoft.com/en-us/azure/foundry-classic/openai/concepts/content-filter-annotations)页面，2026-11-14 读取。没有可用的密钥：`fixtures/openai_chat/providers/azure/` 中的一个回复是 Azure 发送、并发布在 Stack Overflow 回答中的回复，其余回复和流来自文档示例；密钥错误使用编造的密钥实时获得。每个密钥带有其资源的端点，例如 `https://example.openai.azure.com`，Kaeso 调用 `{endpoint}/openai/v1/…`，密钥放在 `api-key` 中，不带 `api-version`。

| Azure 字段 | KPP | 说明 |
|---|---|---|
| `model` | 部署名称 | 模型有 `upstream_model` 时使用它，否则使用模型 ID。 |
| `prompt_filter_results`（或 `prompt_annotations`）、`choices[].content_filter_results` | extras | 对 Chat 应用原样往返，对其他方言省略（`responses/greeting-published`）。 |
| `id`、`object`、`model` 为空且 `choices: []` 的第一个 chunk | 开始 | 回复的 ID 和模型来自下一个 chunk（`streams/filter-pass-doc`）。 |
| choice 中只有 `content_filter_results` 和 `content_filter_offsets` 的 chunk | extras | 也会出现在 `finish_reason: "stop"` 之后；它们不带文本。 |
| `finish_reason: "content_filter"` | `StopReason::ContentFilter` | 也可出现在没有 `delta` 的 chunk 中（`streams/filter-blocked-doc`），以及没有 `content` 的回复中（`responses/filtered-blocklist-doc`）。 |
| 带 `code: "content_filter"` 和 `innererror` 的 400 | `KppError` | `ContentFiltered`，带 Azure 的消息（`errors/content-filter-prompt-doc`）。 |
| 字符串形式的数字 `error.code`，例如 `"401"` | `KppError` | 类型按状态码确定（`errors/recorded-invalid-api-key`）。 |
| `{"statusCode": 401, "message": …}` | `KppError` | Azure 对其不接受的 Entra ID 令牌的回复（`errors/recorded-entra-token`）；对于用 Entra ID 登录的密钥，这是一个被拒绝的密钥，消息会注明服务主体需要的角色。 |
| `GET /openai/v1/models` | `ModelList` | 资源可以部署的模型，而不是它的部署，因此模型检查只显示密钥及其端点是否可用（`ModelListing::Catalogue`）。 |

v1 API 以 OpenAI 的请求体提供 Responses，回复中另有 `content_filters`，保留在 extras 中；模型的**发送格式**为 Responses 时，Kaeso 以 Responses 发送它（`fixtures/openai_responses/providers/azure/`）。Claude 部署只响应 `{endpoint}/anthropic` 下的 Anthropic Messages API，图像、语音合成、转写和视频在 v1 API 中仅为预览版，因此 Kaeso 都不调用。Kaeso 自行估算 Token 计数。

## DeepInfra

来源：DeepInfra 的 [OpenAPI document](https://api.deepinfra.com/openapi.json) 及其 [Chat Completions](https://docs.deepinfra.com/chat/overview.md)、[流式传输](https://docs.deepinfra.com/chat/streaming.md)、[工具调用](https://docs.deepinfra.com/chat/tool-calling.md)、[提示缓存](https://docs.deepinfra.com/chat/prompt-caching.md)和[认证](https://docs.deepinfra.com/account/authentication.md)指南，2026-11-14 阅读。由于没有可用的密钥：`fixtures/openai_chat/providers/deepinfra/` 中的一个回复是 DeepInfra 实际发送、发布在另一个项目 issue 中的回复，其余回复和流据指南构建；模型列表和密钥错误是在没有有效密钥的情况下实时返回的。DeepInfra 的指南将基础 URL 写为 `https://api.deepinfra.com/v1/openai`；它的 OpenAPI 文档在 `/v1/…` 下提供同一 API，因此 Kaeso 的基础 URL 为 `https://api.deepinfra.com`，使用 `Authorization: Bearer`。

| DeepInfra 字段 | KPP | 说明 |
|---|---|---|
| `message.reasoning_content`、`delta.reasoning_content` | `Block::Reasoning` | 通过配置的 `reasoning_fields`；assistant 消息会将其传回。 |
| `max_completion_tokens` | `max_tokens` | schema 只有 `max_tokens`（`max_tokens()`）。 |
| `developer` 角色 | `Role::System` | schema 列出 `system`、`user`、`assistant` 和 `tool`，因此 `system_role()` 发送 `system`。 |
| `user` | `user` | schema 没有 `safety_identifier`，因此终端用户的 ID 以 `user` 发送（`end_user_as_user()`）。 |
| `video_url` 部分 | `Block::Video` | `with_video_url()`。 |
| `parallel_tool_calls`、`top_logprobs`、`logit_bias`、`metadata`、`store` 以及 schema 中没有的其他字段 | 拒绝 | DeepInfra 会悄悄丢弃未声明的字段，因此配置会拒绝它们；`parallel_tool_calls: true` 只是允许并行调用，会被省略，`store: false` 这样的[中性值](sampling.md#中性值)也一样。 |
| `top_k`、`min_p`、`repetition_penalty`、`fail_fast`、`models`、`prompt_cache_options`、`cache_salt`、`chat_template_kwargs` 和 `reasoning` 对象 | extras | 当请求发往其他方言时被拒绝，与其他仅限 Chat 的字段相同。 |
| `usage.estimated_cost`、带 `cache_write_tokens` 的 `prompt_tokens_details`、`name: null` | extras 和原始用量 | 发往其他方言时省略（`responses/greeting-published`、`responses/cached-doc`）。 |
| 缺少 `inference` 权限范围的密钥收到的 403 `{"detail": {"error": …}}` | `KppError` | `PermissionDenied`，带 DeepInfra 的消息（`errors/missing-scope-doc`）。 |
| 带 `root`、`parent` 和 DeepInfra `metadata` 的 `GET /v1/models` | `ModelList` | 无需密钥即可返回（`models/list-recorded`），因此模型检查无法发现错误的密钥。没有 `GET /v1/models/{model}`。 |

DeepInfra 的 Responses API 不存储任何内容，不被使用：Responses 应用通过 Chat Completions 访问 DeepInfra。它的 Anthropic 兼容端点和 Token 计数位于 `/anthropic/v1` 下，在其他格式的 `/v1` 之外，因此 Kaeso 自行估算 Token 计数。在 KPMR 收录这些模型及其价格之前，它以 OpenAI 格式提供的图像、语音合成、转写和视频不被使用。

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

## Groq

来源：Groq 的 [API 参考](https://console.groq.com/docs/api-reference)和[推理指南](https://console.groq.com/docs/reasoning)（2026-10-19 阅读），以及 `fixtures/openai_chat/providers/groq/` 中的实时录制。

| Groq 字段 | KPP | 说明 |
|---|---|---|
| `reasoning_format`、`include_reasoning`（请求） | extras | 请求发往其他方言时会被拒绝，与其他仅限 Chat 的字段相同。 |
| `user`、`safety_identifier`（请求） | KPP `user` | Groq 接受 `user` 并拒绝 `safety_identifier`，因此该配置以 `user` 发送终端用户的 ID（`end_user_as_user()`）。 |
| `prompt_cache_key`（请求） | extras | Groq 会拒绝它。因此网关在发送前拒绝 Chat 应用自己的键；OpenAI Responses 应用的键（Codex 在每个请求中都会发送）则被省略，并在 `kaeso-left-out` 中注明（`without_cache_key()`）。 |
| `message.reasoning`、`delta.reasoning` | `Block::Reasoning` | 通过配置的 `reasoning_fields`。Chat 应用会原样收到它们；Anthropic Messages 应用得到 `thinking` 块，Responses 应用得到推理项（[推理](reasoning.md)）。 |
| `delta.channel`（GPT-OSS 推理中为 `analysis`） | 数据块的 extras | Chat 应用随推理一起收回；对其他方言省略。 |
| `x_groq`、`usage_breakdown`、`service_tier`（`on_demand`） | extras | 发往其他方言时省略。 |
| `usage.queue_time`、`prompt_time`、`completion_time`、`total_time` | 用量的 `raw` | Token 数（包括 `reasoning_tokens`）按[用量](openai-chat.md#用量completionusage)中的类型化字段处理。 |
| 结束数据块中的 `usage`，即使没有 `stream_options.include_usage` | `Usage` | 无论 Groq 是否另外发送 `include_usage` 请求的用量数据块，每种方言都会收到用量。 |
| 额外的模型字段（`active`、`context_window`、`max_completion_tokens`、`pricing`、`supported_features` 等） | 模型 extras | |

## Hugging Face

来源：Hugging Face 的 [Inference Providers](https://huggingface.co/docs/inference-providers/index) 概览、其 [chat completion](https://huggingface.co/docs/inference-providers/tasks/chat-completion) 参考及 `huggingface.js` 中的 schema，以及 [Claude Code](https://huggingface.co/docs/inference-providers/integrations/claude-code) 和 [Responses API](https://huggingface.co/docs/inference-providers/guides/responses-api) 指南，2026-11-10 阅读。未经成功调用测试：`fixtures/openai_chat/providers/huggingface/` 中的回复和流根据参考构建，思考内容放在路由器背后服务商记录的字段中；模型列表和错误是在没有有效密钥的情况下实时返回的。Kaeso 的基础 URL 为 `https://router.huggingface.co`，使用 `Authorization: Bearer` 和 `hf_` 令牌。

| Hugging Face 字段 | KPP | 说明 |
|---|---|---|
| `message.reasoning_content`、`message.reasoning` 及其增量 | `Block::Reasoning` | 通过配置的 `reasoning_fields`；路由器转发其背后服务商的字段。 |
| `max_completion_tokens` | `max_tokens` | schema 只有 `max_tokens`（`max_tokens()`）。 |
| `developer` 角色 | `Role::System` | `role` 会按原样转给服务商，而它们并非都接受 `developer`，因此 `system_role()` 发送 `system`。 |
| 终端用户的 ID | 省略 | schema 没有 `user` 或 `safety_identifier`（`without_end_user()`）。 |
| 模型 ID 上的 `:provider`、`:fastest`、`:cheapest` 或 `:preferred` 后缀 | `model` | 按原样发送；回复中的模型名不带后缀（`responses/provider-suffix`）。 |
| `tool_prompt` | extras | 请求发往其他方言时会被拒绝，与其他仅限 Chat 的字段相同。 |
| `{"error": "<message>"}` 形式的错误 | `KppError` | 类别由状态码决定：401 表示密钥被拒绝（`errors/recorded-invalid-api-key`）。 |
| 带有 `architecture` 以及包含状态、上下文长度、价格和速度的 `providers` 数组的 `GET /v1/models` | `ModelList` | 无需密钥即可返回（`models/list-recorded`），因此模型检查无法发现错误的密钥。 |

路由器的 OpenAI 兼容 API 只用于聊天：`/v1/embeddings`、`/v1/images/generations` 和 `/v1/audio/*` 返回 404。它的 Responses API 不被使用，因为它对被拒绝的令牌返回 200 和 `status: "failed"`。它的 Messages 端点也不被使用：它对被拒绝的令牌返回 `api_error`，应用会将其视为服务商失败，而不是密钥被拒绝。`/v1/messages/count_tokens` 不检查令牌就返回结果，且没有文档，因此 Kaeso 自行估算 Token 计数。

## IONOS AI Model Hub

来源：IONOS 的 [create chat completion](https://docs.ionos.com/cloud/api-reference/ai/ai-model-hub/chat-completions/create-chat-completion) 参考（OpenAPI 3.0.3）、其[错误码](https://docs.ionos.com/cloud/ai/ai-model-hub/error-codes)和模型页面，2026-11-14 阅读。由于没有可用的令牌，`fixtures/openai_chat/providers/ionos/` 中的回复是模型页面的示例，流围绕文档展示的最后一个分块构建；密钥错误是在没有有效令牌的情况下实时返回的。Kaeso 的基础 URL 为位于柏林的 `https://openai.inference.de-txl.ionos.com`，使用带 IONOS Cloud API 令牌的 `Authorization: Bearer`。

| IONOS 字段 | KPP | 说明 |
|---|---|---|
| `message.reasoning`、`message.reasoning_content` | `Block::Reasoning` | 通过配置的 `reasoning_fields`；GPT OSS 在两个字段中发送相同的思考内容，Qwen3.8 只在 `reasoning` 中发送（`responses/gpt-oss-reasoning-doc`、`responses/qwen-reasoning-doc`）。 |
| `developer` 角色 | `Role::System` | 只有部分模型接受 `developer`；IONOS 建议使用 `system`（`system_role()`）。 |
| `user` | `user` | schema 没有 `safety_identifier`（`end_user_as_user()`）。 |
| `prompt_cache_key` | 省略 | IONOS 自行缓存前缀，没有该字段（`without_cache_key()`）。 |
| `store`、`metadata`、`service_tier` 以及 schema 未声明的其他字段 | 拒绝 | 文档未说明 IONOS 会拒绝还是丢弃它们。 |
| `stop_reason`、`token_ids`、`prompt_logprobs`、`prompt_token_ids`、`kv_transfer_params` 以及消息中为 `null` 的字段 | extras | 发往其他方言时省略（`responses/mistral-image-doc`）。 |
| `{"httpStatus", "messages": [{"errorCode", "message"}]}` | `KppError` | IONOS 自己的错误格式，各条消息合并；类别由状态码决定（`errors/recorded-invalid-api-key`、`errors/validation-error-doc`）。只有 529 采用 OpenAI 的格式。 |
| `GET /v1/models` | `ModelList` | 需要令牌，只返回 ID；没有 `GET /v1/models/{model}`。 |

IONOS 的 Responses API 不保存状态，不被使用：Responses 应用通过 Chat Completions 访问 IONOS。在 KPMR 收录相应模型之前，它的图像（FLUX）、转写（Whisper）和重排序不被使用；它不提供语音合成、视频、Messages 或 Token 计数。

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

## Novita AI

来源：Novita AI 的 [chat completion](https://docs.novita.ai/api-reference/model-apis-llm-create-chat-completion.md)、[嵌入](https://docs.novita.ai/api-reference/model-apis-llm-create-embeddings.md)、[模型列表](https://docs.novita.ai/api-reference/model-apis-llm-list-models.md)和[错误码](https://docs.novita.ai/api-reference/basic-error-code.md)参考，及其[推理](https://docs.novita.ai/guides/llm-reasoning.md)、[交错思考](https://docs.novita.ai/guides/llm-interleaved-thinking.md)、[提示缓存](https://docs.novita.ai/guides/llm-prompt-cache.md)指南和 [FAQ](https://docs.novita.ai/guides/LLM-FAQ.md)，2026-11-14 阅读。由于没有可用的密钥：`fixtures/openai_chat/providers/novita-ai/` 中的回复是指南中的示例，流据参考构建；模型列表、单个模型和密钥错误是在没有有效密钥的情况下实时返回的。基础 URL 为 `https://api.novita.ai/openai`，参考在其下提供 `/v1/chat/completions`、`/v1/embeddings` 和 `/v1/models`，使用 `Authorization: Bearer`。

| Novita AI 字段 | KPP | 说明 |
|---|---|---|
| `message.reasoning_content`、`delta.reasoning_content` | `Block::Reasoning` | 仅在 `separate_reasoning: true` 时返回，配置会发送它（`separate_reasoning()`）；否则推理内容留在 `content` 中。Chat 应用自己的 `separate_reasoning` 保持不变。 |
| `enable_thinking` | `ReasoningConfig` | `thinking_switch()`；Anthropic 应用的预算以 `thinking_budget` 发送，参考中没有该字段。 |
| `max_completion_tokens` | `max_tokens` | 参考只有 `max_tokens`，它也计入推理 Token。 |
| `developer` 角色 | `Role::System` | 参考列出 `system`、`user` 和 `assistant`；其指南也发送 `tool` 消息。 |
| `user`、`safety_identifier` | 省略 | 参考没有终端用户字段（`without_end_user()`）。 |
| `video_url` 部分 | `Block::Video` | `with_video_url()`。 |
| `store`、`metadata`、`prompt_cache_key`、`parallel_tool_calls`、`service_tier` 以及参考中没有的其他字段 | 拒绝 | 参考没有说明 Novita AI 是否拒绝未声明的字段，因此配置会拒绝它们；`parallel_tool_calls: true` 会被省略。参考中同样没有的 `tool_choice` 会发送，因为工具调用需要它。 |
| 回复消息上的 `reasoning_details` 和 `name`，`prompt_tokens_details` 中的 `cache_creation_Prompt_tokens` 和 `cache_read_Prompt_tokens` | extras 和原始用量 | 发往其他方言时省略（`responses/interleaved-tool-call-doc`、`responses/cached-doc`）。 |
| `{"code", "reason", "message", "metadata"}` 形式的错误 | `KppError` | 类型取决于状态码：401 `FAILED_TO_AUTH`，403 `INVALID_API_KEY` 或 `NOT_ENOUGH_BALANCE`，404 `MODEL_NOT_FOUND`，429，503（`errors/`）。 |
| 带以 1/10,000 美元计的价格、`pricing`、`features` 和 `endpoints` 的 `GET /v1/models` | `ModelList` | 无需密钥即可返回，且只列出聊天模型（`models/list-recorded`）；`GET /v1/models/{model}` 对嵌入模型也会返回（`models/retrieve-embedding-recorded`），因此模型检查能找到模型，但无法发现错误的密钥（`ModelListing::Partial`）。 |

Novita AI 的 Anthropic 兼容端点位于 `/anthropic` 下，在其 OpenAI 基础 URL 之外，`/v1/responses` 也没有文档，因此 Kaeso 只调用 Chat Completions 并自行估算 Token 计数。它的 `/v1/completions`、`/v1/rerank`、批处理 API、OpenAI Images API 中的图像模型，以及它自己的语音、转写和视频任务 API 不被使用。

## NVIDIA

来源：NVIDIA 的 [LLM APIs](https://docs.api.nvidia.com/nim/reference/llm-apis) 参考及其模型页面的 OpenAPI 文档，例如 [`nvidia/nemotron-3-super-120b-a12b`](https://docs.api.nvidia.com/nim/reference/nvidia-nemotron-3-super-120b-a12b-infer)，2026-11-10 阅读。未经成功调用测试：`fixtures/openai_chat/providers/nvidia/` 中的回复和流据此构建，流的请求是参考自己的示例；模型列表和错误是在没有有效密钥的情况下实时返回的。Kaeso 的基础 URL 为 `https://integrate.api.nvidia.com`，使用 `Authorization: Bearer` 和 `nvapi-` 密钥。

| NVIDIA 字段 | KPP | 说明 |
|---|---|---|
| `message.reasoning_content`、`delta.reasoning_content` | `Block::Reasoning` | 通过配置的 `reasoning_fields`。 |
| `chat_template_kwargs`（`enable_thinking`、`low_effort`）、`reasoning_budget` | extras | 模型自己的思考开关；Chat 应用发送的会按原样转发，请求发往其他方言时会被拒绝。 |
| `max_completion_tokens` | `max_tokens` | 请求 schema 只有 `max_tokens`（`max_tokens()`）。 |
| `developer` 角色 | `Role::System` | schema 列出 `system`、`context`、`user` 和 `assistant`，因此 `system_role()` 发送 `system`。 |
| 终端用户的 ID | 省略 | schema 没有 `user` 或 `safety_identifier`（`without_end_user()`）。 |
| 以 RFC 9457 问题文档表示、带有 `status`、`title` 和 `detail` 的错误 | `KppError` | 类别由状态码决定：NVIDIA 不认识的密钥返回 403（`errors/recorded-invalid-api-key`），已退役的模型返回 410（`errors/recorded-retired-model`）。 |
| 带有 `NVCF-REQID` 头的 202 | — | 结果尚未就绪时 NVIDIA 的回复，需要轮询；Kaeso 不轮询。 |
| `GET /v1/models` | `ModelList` | 无需密钥即可返回（`models/list-recorded`），因此模型检查无法发现错误的密钥。 |

NVIDIA 不提供 Responses、Messages 或 Token 计数：`/v1/responses` 和 `/v1/messages/count_tokens` 返回 404，`/v1/messages` 对每个模型都返回 `model not found`。它的嵌入模型接受 `input_type` 和 `truncate`，这些字段保留在 extras 中（[OpenAI Embeddings](openai-embeddings.md#nvidia)）。

## Ollama Cloud

来源：Ollama 的 [Cloud](https://docs.ollama.com/cloud)、[身份验证](https://docs.ollama.com/api/authentication)、[OpenAI 兼容性](https://docs.ollama.com/api/openai-compatibility)和[错误](https://docs.ollama.com/api/errors)页面（2026-10-30 阅读），以及 `fixtures/openai_chat/providers/ollama-cloud/` 中用免费套餐的 `gpt-oss:20b` 和 `gemma4:31b` 录制的实时记录。

| Ollama Cloud 字段 | KPP | 说明 |
|---|---|---|
| 带标签的模型 ID，例如 `gpt-oss:20b` | `model` | 原样发送，在 `GET /v1/models/{model}` 中也是如此。 |
| `max_completion_tokens`（被忽略）、`max_tokens`（生效） | `max_output_tokens` | 配置中的 `max_tokens()` 把每个应用的输出上限作为 `max_tokens` 发送。 |
| `reasoning: {effort}`（请求） | extras | Ollama 在 `reasoning_effort` 之外的写法；请求发往其他方言时被拒绝，与其他仅限 Chat 的字段相同。 |
| `message.reasoning`、`delta.reasoning` | `Block::Reasoning` | 通过配置中的 `reasoning_fields`，与 Groq 相同。推理块旁边带有 `content: ""`，不会打开文本块。其他格式的应用发回的推理会以助手轮次中的 `reasoning` 送达 Ollama。 |
| `system_fingerprint`（`fp_ollama`） | extras | 发往其他方言时省略。 |
| `usage.prompt_tokens_details.cached_tokens` | `Usage.cache_read_tokens` | Ollama 不报告推理 token。 |
| `delta` 为空的结束块，随后是 `choices: []` 的用量块 | `Usage` | 与 OpenAI 的发送方式相同。 |
| `logprobs`、`top_logprobs`、`logit_bias` | 拒绝 | Ollama 接受这些字段但不返回任何结果；配置在发送前拒绝它们。 |
| `auto` 以外的 `tool_choice`、`response_format` | 发送 | 在 2026-10-30 的实时测试中，Ollama 的云端模型忽略了这两个字段；Kaeso 原样发送。 |
| 助手消息中的 `refusal` 内容部分 | 发送 | Ollama 返回 400 `invalid message format`（`errors/recorded-content-array-all-roles`），应用会以自己的格式收到。 |
| OpenAI 格式的错误，`type` 为 `api_error`、`not_found_error` 或 `invalid_request_error` | `KppError` | 类型取决于状态码：401 表示密钥错误，402 表示套餐不包含该模型（`errors/recorded-model-outside-plan`），404 表示模型不存在。 |
| `GET /v1/models`：OpenAI 的列表、`owned_by: "ollama"`，以及顶层采用 Codex 模型目录格式的 `models` 数组 | `ModelList`、extras | Ollama 返回模型列表和 `GET /v1/models/{model}` 时不检查密钥，因此模型检查无法发现错误的密钥。 |

## OVHcloud AI Endpoints

来源：AI Endpoints 的 [OpenAPI document](https://oai.endpoints.kepler.ai.cloud.ovh.net/doc/gpt-oss-120b/openapi.json) 及其 [Getting started](https://docs.ovhcloud.com/en/guides/public-cloud/ai-machine-learning/ai-endpoints-getting-started.md)、[Capabilities](https://docs.ovhcloud.com/en/guides/public-cloud/ai-machine-learning/ai-endpoints-capabilities.md)、[Function calling](https://docs.ovhcloud.com/en/guides/public-cloud/ai-machine-learning/ai-endpoints-function-calling.md) 和 [Structured output](https://docs.ovhcloud.com/en/guides/public-cloud/ai-machine-learning/ai-endpoints-structured-output.md) 指南，2026-11-14 阅读。AI Endpoints 不带密钥也会回答，每个 IP 每个模型每分钟 2 次请求，因此 `fixtures/openai_chat/providers/ovhcloud/` 中的回复、流、模型列表和错误均于 2026-11-14 不带密钥实时录制，密钥错误使用虚构的密钥。Kaeso 的基础 URL 是 `https://oai.endpoints.kepler.ai.cloud.ovh.net`，即去掉 `/v1` 的文档基础地址，使用 `Authorization: Bearer`。OVHcloud 在法国托管这些模型。

| AI Endpoints 字段 | KPP | 说明 |
|---|---|---|
| `message.reasoning`、`delta.reasoning` | `Block::Reasoning` | 通过配置的 `reasoning_fields`；assistant 消息可以将其带回（`requests/recorded-tool-result`）。在思考时被截断的回复没有 `content`（`responses/recorded-thinking-length`）。 |
| `developer` 角色 | `Role::System` | Mistral Small 3.2 对它返回 400（`errors/recorded-developer-role-mistral`），因此 `system_role()` 发送 `system`。 |
| `user`、`safety_identifier` | 省略 | `user` 返回 400，schema 也未声明其他终端用户字段（`without_end_user()`）。 |
| `metadata`、`store`、`modalities`、`audio`、`prediction`、`prompt_cache_key`、`web_search_options`、`functions`、`function_call` | 拒绝 | 未声明，而解析器至少会悄悄放行 `metadata`、`store`、`prediction`、`prompt_cache_key` 和 `web_search_options`；Responses 应用的缓存键会被省略（`without_cache_key()`）。 |
| 其他未声明的字段，例如 `service_tier`、`verbosity` 和 `top_k` | 原样发送 | AI Endpoints 返回 400 并逐一列出（`errors/recorded-service-tier`）。 |
| `tool_calls: []`、delta 上的 `"name": "assistant"`、usage 块上的 `system_fingerprint` | extras | 转到其他方言时省略。 |
| 仅含 `{"message": …}` 的错误，或其中以 JSON 文本形式包含的 vLLM 错误 | `KppError` | 消息连同状态码一起引用，并从文本中读出嵌套的 `error.message`。403“Forbidden: authentication failed.”表示密钥被拒绝（`refused_key_messages`、`errors/recorded-invalid-api-key`）。 |
| 带有 `pricing`（每 Token 美元）、`context_length` 和 `max_completion_tokens` 的 `GET /v1/models` | `ModelList` | 列出所有类型的模型，并以 403 拒绝错误的密钥（`models/list-recorded`）。 |

AI Endpoints 的 Responses API 要求 `store: false`，因此不使用：Responses 应用通过 Chat Completions 访问它。它不提供 Messages 或 Token 计数。模型 ID 区分大小写；小写别名的回复在 `model` 中给出列表中的 ID，因此 KPMR 收录 `GET /v1/models` 列出的 ID。在 KPMR 收录带价格的转写模型之前，不使用它的转写。

## SambaNova

来源：SambaNova 的 [OpenAPI document](https://raw.githubusercontent.com/sambanova/sambanova-inference-api-spec/refs/heads/main/openapi.documented.json)（SambaNova cloud API 1.2.0）及其 [OpenAI compatibility](https://docs.sambanova.ai/docs/en/features/openai-compatibility)、[文本生成](https://docs.sambanova.ai/docs/en/features/text-generation)和 [API error codes](https://docs.sambanova.ai/docs/en/api-reference/using-the-api/api-error-codes) 页面，2026-11-09 阅读。由于没有可用的密钥，`fixtures/openai_chat/providers/sambanova/` 包含该文档自己的示例回复和示例流、据此构建的回复、SambaCloud 无需密钥即提供的模型列表，以及在没有有效密钥的情况下实时返回的错误。Kaeso 的基础 URL 为 `https://api.sambanova.ai`，使用 `Authorization: Bearer`。

| SambaNova 字段 | KPP | 说明 |
|---|---|---|
| `message.reasoning`、`delta.reasoning`、`channel` | `Block::Reasoning`、extras | 通过配置的 `reasoning_fields`。assistant 消息接受发回的 `reasoning`。 |
| `chat_template_kwargs`（`enable_thinking`）、`top_k`、`repetition_penalty` | extras | DeepSeek-V3.2 和 Gemma 4 只有在 `enable_thinking` 时才思考，而只有 Chat 应用能发送它。当请求发往其他方言时被拒绝。 |
| `frequency_penalty`、`presence_penalty` | 拒绝 | SambaNova 接受并忽略它们，因此配置在发送之前拒绝它们；`0` 会被省略（[中性值](sampling.md#中性值)）。 |
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

## SiliconFlow

来源：SiliconFlow 的 [chat completions](https://docs.siliconflow.com/en/api-reference/chat-completions/chat-completions) 参考、其 [reasoning](https://docs.siliconflow.com/en/userguide/capabilities/reasoning) 指南和 [error handling](https://docs.siliconflow.com/en/faqs/error-code) 页面，2026-11-10 阅读。未经成功调用测试：`fixtures/openai_chat/providers/siliconflow/` 中的请求是 reasoning 指南自己的示例，回复和流根据参考构建；错误是在没有有效密钥的情况下实时返回的。Kaeso 的基础 URL 为 `https://api.siliconflow.com`，使用 `Authorization: Bearer` 和 `sk-` 密钥。

| SiliconFlow 字段 | KPP | 说明 |
|---|---|---|
| `message.reasoning_content`、`delta.reasoning_content` | `Block::Reasoning` | 通过配置的 `reasoning_fields`。流增量在其填充的字段旁带有 `content: null`。 |
| `enable_thinking`、`thinking_budget` | `reasoning` | `thinking_switch()`，与 QwenCloud 相同：应用的思考设置会开启或关闭它们；Chat 应用自己的值按原样发送。 |
| `max_tokens` | `max_tokens` | 它只限制回答，因此使用 `max_tokens()` 和服务商的 `reasoning_beyond_output_limit`。 |
| `developer` 角色 | `Role::System` | 参考只列出 `system`、`user` 和 `assistant`，因此 `system_role()` 发送 `system`。 |
| 终端用户的 ID | 省略 | 参考没有 `user` 或 `safety_identifier`（`without_end_user()`）。 |
| `min_p`、`top_k` | extras | 请求发往其他方言时会被拒绝，与其他仅限 Chat 的字段相同。 |
| `{"code", "message", "data"}` 形式的错误 | `KppError` | SiliconFlow 的消息；类别由状态码决定：401 表示密钥被拒绝（`errors/recorded-invalid-api-key`）。 |

SiliconFlow 不提供 Responses 或 Token 计数：`/v1/responses` 和 `/v1/messages/count_tokens` 返回 404。它的 Messages 端点不被使用，因为其参考只记录了文本和工具块。它的嵌入、语音和转录使用 OpenAI 的形式（[OpenAI Embeddings](openai-embeddings.md#siliconflow)）；图像和视频则不是。

## Together AI

来源：Together AI 的 [Chat Completions 参考](https://docs.together.ai/reference/chat-completions-1)和 [OpenAI 兼容性说明](https://docs.together.ai/docs/inference/openai-compatibility)（2026-10-19 阅读）、`togethercomputer/together-py` 中的 SDK 类型，以及 `fixtures/openai_chat/providers/togetherai/` 中的实时录制。

| Together AI 字段 | KPP | 说明 |
|---|---|---|
| `top_k`、`min_p`、`repetition_penalty`、`safety_model`、`context_length_exceeded_behavior`（请求） | extras | 请求发往其他方言时会被拒绝，与其他仅限 Chat 的字段相同。 |
| `message.reasoning_content` 或 `message.reasoning`（视模型而定），以及增量中的同名字段 | `Block::Reasoning` | 通过配置的 `reasoning_fields`。Chat 应用会原样收到它们；Anthropic Messages 应用得到 `thinking` 块，Responses 应用得到推理项（[推理](reasoning.md)）。流式推理片段带有 `content: ""`，它不会打开文本块。 |
| choice 上的 `matched_stop` 和 `seed`、流式 choice 上的 `text`、增量中的 `token_id`，以及顶层的 `prompt`、`metadata` 和 `warnings` | extras | 发往其他方言时省略。 |
| 与 `completion_tokens_details.reasoning_tokens` 并列的 `usage.reasoning_tokens` | 用量的 `raw` | Token 数按[用量](openai-chat.md#用量completionusage)中的类型化字段处理。 |
| 采用 OpenAI 错误结构、带 Together AI 自有 `type` 和 `code`（`start_generation_failed`、`model_not_available`）的错误，有时带顶层 `id` | `KppError` | 类别取决于状态码；密钥错误时返回 401 `invalid_api_key`，即 `Authentication`。 |
| `GET /v1/models`：模型的裸数组，带 `type`、`display_name`、`organization`、`context_length`、`config` 和 `pricing` | `ModelList`、模型 extras | 对含 `/` 的 ID，`GET /v1/models/{model}` 返回 404，因此模型检查依赖模型列表。 |

## Volcengine Ark

来源：方舟的 [Base URL 及鉴权](https://docs.volcengine.com/docs/zh/ark/base-url-and-authentication.md)页面、[Chat API](https://docs.volcengine.com/docs/zh/ark/chat-api.md) 参考（更新于 2026-11-12）、[深度思考](https://docs.volcengine.com/docs/zh/ark/deep-thinking.md)指南、[模型列表](https://docs.volcengine.com/docs/zh/ark/models-api.md)和[错误码](https://docs.volcengine.com/docs/zh/ark/error-codes.md)，阅读于 2026-11-14。未经成功调用测试：`fixtures/openai_chat/providers/volcengine/` 中的回复、流和列表是参考与指南中的示例，一个流发布在另一个项目的 issue 中，密钥错误是在没有有效密钥时实时得到的。Kaeso 的基础 URL 是 `https://ark.cn-beijing.volces.com/api/v3`，使用 `Authorization: Bearer`。

| Volcengine Ark 字段 | KPP | 说明 |
|---|---|---|
| `<base>/chat/completions`、`<base>/models` | 服务商的 API 根路径 | 方舟的 API 位于 `/api/v3`，而不是 `/v1`；基础 URL 以此结尾，根路径为空（[API 根路径](#api-根路径)）。 |
| `thinking: {"type": "enabled" \| "disabled"}` | `reasoning` | `thinking_type()`：模型默认思考，而 `minimal` 会关闭 Seed 的思考，在 DeepSeek V4.1 Flash 上却是 `low`，因此关闭思考只以开关发送。 |
| `reasoning_effort`（`none` 到 `max`） | `reasoning.effort` | 每个可调节思考的模型都接受全部 7 档取值，并映射到自己的档位。 |
| `message.reasoning_content`、`delta.reasoning_content` | `Block::Reasoning` | 通过配置的 `reasoning_fields`；助手消息会收回它。 |
| `message.encrypted_content`、`delta.encrypted_content` | extras 以及推理块的 opaque | `encrypted_reasoning_field`：Chat 应用原样收到并发回；其他应用在自己的推理载体中携带它，Kaeso 把它写回助手消息，这是方舟在多轮工具调用中的要求（`responses/tools-encrypted-content-doc`、`requests/tools-encrypted-content-replay-doc`）。 |
| `max_completion_tokens`、`max_tokens` | `max_output_tokens` | `max_completion_tokens` 计入思考和回答，最大 65536；在大多数模型上 `max_tokens` 只计回答。按应用的写法发送，Kaeso 自己的上限以 `max_completion_tokens` 发送；服务商设有 `reasoning_beyond_output_limit`。 |
| 终端用户的 ID | 省略 | 没有 `user` 或 `safety_identifier`（`without_end_user()`）。 |
| `service_tier`（`auto`、`default`、`fast`、`flex`） | extras | 原样发送；OpenAI 的 `priority` 和 `scale` 被拒绝（`refusing_values`）。 |
| `web_search_options` | extras | 仅限豆包搜索；`{}` 即开启。`search_context_size` 和 `user_location` 不生效，因此被拒绝。回复带有不含偏移的 `url_citation` 标注和 `usage.tool_usage.web_search`（`responses/web-search-doc`）。 |
| `seed`、`n`、`metadata`、`store`、`modalities`、`audio`、`prediction`、`prompt_cache_retention`、`functions`、`function_call` | 拒绝 | 不在 schema 中，且方舟没有说明是否拒绝未声明的字段。 |
| `prompt_cache_key`、`video_url` 部分、`logprobs`、`logit_bias`、`parallel_tool_calls` | 原样发送 | 在 schema 中；推理模型会自行拒绝其中一些。 |
| 不带 `chatcmpl-` 的 ID、回复中的 `service_tier`、`usage.prompt_tokens_details.audio_tokens` | extras 和原始 usage | 对 Chat 应用往返保留。 |
| OpenAI 信封中的错误，带方舟的 `code` 和类似 HTTP 的 `type`，消息末尾是请求 ID | `KppError` | 类别由状态码决定：401 表示密钥被拒绝（`errors/recorded-*-api-key`）。 |
| `GET /models`，带方舟的 `name`、`version`、`domain`、`token_limits`、`modalities` 和 `features`，没有 `owned_by` | `ModelList` | 需要密钥。它列出基础模型版本，不含账户的接入点（`ep-…`），因此列表不完整（`models/list-doc`）。 |

方舟的嵌入只提供 `/embeddings/multimodal`，整段输入只返回一个向量，因此该服务商没有嵌入方言。它的 Responses 和 Anthropic Messages 端点不被使用，也没有 Token 计数。BytePlus ModelArk 在 `https://ark.ap-southeast.bytepluses.com/api/v3` 提供相同的 API，有自己的密钥和模型 ID，通过基础 URL 使用。

### API 根路径

API 不在 `/v1` 下的服务商在 `Provider::api_root` 中指定其 API 根路径：即在每种方言的路径中取代 `/v1` 的部分。网关通过 `EgressCtx::with_api_root` 把它交给编解码器（`pipeline::egress_ctx`，以及用于模型列表和模型检查的 `availability::models_ctx`）。Chat Completions、Responses、Messages 和嵌入把各自的 `/v1/…` 路径放在根路径下，OpenAI 模型列表把 `/v1/models` 发送为 `<root>/models`。`Some("")` 表示基础 URL 本身就止于根路径，例如 Volcengine Ark 的 `…/api/v3`，因此 `/v1/chat/completions` 变为 `/chat/completions`；其他所有服务商为 `None`，保留 `/v1`。密钥的账户范围（例如 Cloudflare AI Gateway 的 `/client/v4/accounts/{account_id}/ai/v1`）在之后应用，并取代该根路径；自定义服务商的基础 URL 自行包含其根路径。由于默认基础 URL 止于根路径，管理员设置的基础 URL 就是文档中的地址本身，例如 BytePlus ModelArk 的地址。

## StepFun

来源：StepFun 的 [Chat Completions 参考](https://platform.stepfun.ai/docs/en/api-reference/chat/chat-completion-create)，其[推理](https://platform.stepfun.ai/docs/en/guides/developer/reasoning)、[提示缓存](https://platform.stepfun.ai/docs/en/guides/developer/prompt-cache)、[工具调用](https://platform.stepfun.ai/docs/en/guides/developer/tool-call)和[视频](https://platform.stepfun.ai/docs/en/guides/developer/video-chat)指南及[错误码](https://platform.stepfun.ai/docs/en/api-reference/error-codes)，以及其中国服务的[参考](https://platform.stepfun.com/docs/zh/api-reference/chat/chat-completion-create)和[错误码](https://platform.stepfun.com/docs/zh/api-reference/error-codes)，2026-11-14 阅读。由于没有可用的密钥：`fixtures/openai_chat/providers/stepfun-ai/` 中的回复和文本流是参考和指南中的示例，带推理的回复和流据参考构建，密钥错误是在没有有效密钥的情况下实时返回的。Kaeso 的基础 URL 为 `https://api.stepfun.ai`（中国服务为 `https://api.stepfun.com`），使用 `Authorization: Bearer`。

| StepFun 字段 | KPP | 说明 |
|---|---|---|
| `message.reasoning`、`delta.reasoning`，以及 `reasoning_format: "deepseek-style"` 时的 `reasoning_content` | `Block::Reasoning` | 通过配置的 `reasoning_fields`。中国服务的参考会同时发送两个内容相同的字段：读取第一个，另一个保留为 extra（`responses/both-reasoning-fields-schema`）。 |
| `reasoning_format`（请求） | extras | 当请求发往其他方言时被拒绝，与其他仅限 Chat 的字段相同。 |
| 与各项计数并列的 `usage.cached_tokens` | `cache_read_tokens` | 当缺少中国服务参考中列出的 `prompt_tokens_details.cached_tokens` 时读取（`responses/cached-doc`）；`prompt_tokens` 包含缓存的 token。 |
| `max_completion_tokens`、`developer` 角色 | `max_tokens`、`Role::System` | 参考只有 `max_tokens`，角色为 `system`、`user`、`assistant` 和 `tool`（`max_tokens()`、`system_role()`）。 |
| `video_url` 部分 | `Block::Video` | `with_video_url()`。 |
| `user`、`safety_identifier`、Responses 应用的 `prompt_cache_key` | 省略 | 参考没有终端用户或缓存键字段，StepFun 会自行缓存提示（`without_end_user()`、`without_cache_key()`）。 |
| `stream_options` | 省略 | 每个块都会主动带上累计用量（`without_stream_options()`、`streams/text-doc`）。 |
| `presence_penalty`、`seed`、`logprobs`、`logit_bias`、`parallel_tool_calls`、`prompt_cache_key`、`store` 以及参考中没有的其他字段 | 拒绝 | 文档没有说明 StepFun 会拒绝还是丢弃未声明的字段，因此配置会在发送前拒绝它们；`parallel_tool_calls: true` 只是允许并行调用，会被省略，`store: false` 这样的[中性值](sampling.md#中性值)和消息中的 `name`（[参与者名称](openai-chat.md#参与者名称)）也一样。StepFun 指南中发送的 `tool_choice` 会照常发送。 |
| 最后一个块之前的 `finish_reason: ""` 和 `role: ""` | 无 | 与其他服务商一样，按没有值读取。 |
| 带 `finish_reason: "stop"` 的工具调用 | `StopReason::EndTurn` | 文档中的两个工具调用回复都这样结束（`responses/tools-doc`）。Chat 映射把 `stop` 读作一轮结束，因此 Anthropic 应用会收到带 `stop_reason: "end_turn"` 的 `tool_use` 块。 |
| 没有 `code` 的 `{"error": {"message", "type"}}` 错误 | `KppError` | 错误类型取决于状态码，StepFun 的标识位于 `type`。中国服务的 429 `project_credit_limit_exceeded` 和 `member_project_credit_limit_exceeded` 属于计费拒绝，而不是速率限制。 |
| `GET /v1/models` | `ModelList` | 需要密钥，并同时列出对话模型和语音模型（`models/list-doc`），因此模型检查能发现错误的密钥。 |

StepFun 的 Anthropic Messages 端点、Responses API 和 Token 计数（`/v1/token/count`）不被使用：所有应用都通过 Chat Completions 访问 StepFun，Kaeso 自行估算 Token 计数。`input_audio` 接受不带 `format` 的 data URL，用于 KPMR 未收录的 StepFun 音频对话模型；它的语音合成、转写和实时 API 也不被使用。

## Tencent TokenHub

来源：TokenHub 的 [API 使用说明](https://cloud.tencent.com/document/product/1823/130078)、[Chat Completions 字段说明](https://cloud.tencent.com/document/product/1823/135872)、[深度思考](https://cloud.tencent.com/document/product/1823/131208)、[混元调用指南](https://cloud.tencent.com/document/product/1823/132252)、[错误码](https://cloud.tencent.com/document/product/1823/131595)和[模型列表](https://cloud.tencent.com/document/product/1823/130051)页面，2026-11-14 阅读。由于没有可用的密钥：`fixtures/openai_chat/providers/tencent-tokenhub/` 中的回复和流是混元调用指南的示例，另一个项目 issue 中发布的真实流的两个分块与指南中的流一致，密钥错误是在没有有效密钥的情况下实时返回的。所有路径都位于主机的 `/v1` 下，因此 Kaeso 的基础 URL 为 `https://tokenhub.tencentmaas.com`（广州），使用 `Authorization: Bearer`；其他地域和腾讯云国际站是其他主机，各有自己的密钥。

| TokenHub 字段 | KPP | 说明 |
|---|---|---|
| `message.reasoning_content`、`delta.reasoning_content` | `Block::Reasoning` | 通过配置的 `reasoning_fields`；assistant 消息会将其传回，Hy4 和 Hy3 需要这样才能在多轮之间保留思考（`requests/reasoning-sent-back-doc`）。 |
| `developer` 角色 | `Role::System` | 参考列出 `system`、`user`、`assistant` 和 `tool`，因此 `system_role()` 发送 `system`。 |
| `max_tokens`、`max_completion_tokens` | 原样 | TokenHub 两者都接受，优先使用 `max_completion_tokens`。 |
| `video_url` 部分 | `Block::Video` | `with_video_url()`。 |
| `audio`、`modalities`、`logit_bias`、`metadata`、`store`、`service_tier` 以及参考中没有的其他字段 | 拒绝 | TokenHub 会静默忽略未声明的字段（“会被静默忽略”），因此配置会拒绝它们。 |
| `thinking`、`enable_thinking`、`thinking_budget`、`interleaved_thinking`、`reasoning_split`、`web_search_options`、`extra_body` | extras | 当请求发往其他方言时被拒绝，与其他仅限 Chat 的字段相同。 |
| `reasoning_effort` `no_think` | 不发送 | Hy4 和 Hy3 用 `no_think` 关闭思考，它没有对应的 KPP 级别；目录列出 `high`（Hy4）以及 `low`、`high`（Hy3），因此 `none` 会变为模型的最低级别，并在 `kaeso-adapted` 中注明（[始终推理的模型](reasoning.md#始终推理的模型)）。 |
| `reasoning_details`、`search_info`、`usage.cache_read_tokens`、`usage.cache_write_tokens` | extras 和原始用量 | 发往其他方言时省略。 |
| 带 `message_zh`、六位数 `code`、`type`、`source`、`request_id` 的错误 | `KppError` | 类别由状态码决定；`type` 在参考和实时回复之间不一致，429 的 `code` 可能是数字且没有 `type`（`errors/rate-limit-doc`）。状态码说明不足时由 `code` 决定类别：`400003`（输入超出上下文）为 `ContextTooLong`，`400004`（模型不存在）为 `NotFound`，`451001` 为 `ContentFiltered`。 |
| 带 `name` 和 `status`（`online`、`pre-offline`）的 `GET /v1/models` | `ModelList` | 需要密钥（`models/list-doc`）；没有 `GET /v1/models/{model}`。 |

TokenHub 的 Responses 和 Anthropic Messages 端点只支持部分模型，不被使用：Responses 和 Messages 应用通过 Chat Completions 访问 TokenHub，Kaeso 自行估算 Token 计数，因为 TokenHub 不提供。它的图像、视频、语音和多模态嵌入使用它自己的格式。

## Venice

来源：Venice 的 [OpenAPI document](https://docs.venice.ai/swagger.yaml)（版本 `20261113.174621`），以及它的 [API spec](https://docs.venice.ai/api-reference/api-spec)、[OpenAI 迁移指南](https://docs.venice.ai/guides/getting-started/openai-migration)、[错误码](https://docs.venice.ai/api-reference/error-codes)和[推理模型](https://docs.venice.ai/guides/features/reasoning-models)页面，2026-11-14 阅读。由于没有可用的密钥：`fixtures/openai_chat/providers/venice/` 中的回复和流是 OpenAPI document 的示例，以及另一个项目录制的测试所公开的回复；密钥错误是在没有有效密钥的情况下实时返回的。

| Venice 字段 | KPP | 说明 |
|---|---|---|
| `venice_parameters`（请求） | extras | 除非 `include_venice_system_prompt` 为 `false`，Venice 会加上自己的系统提示词，因此当请求未设置该键时，配置的 `with_option_defaults` 会写入它，也会写入 Chat 应用自己的 `venice_parameters`，其中的其他键按原样发送。请求发往其他方言时被拒绝。 |
| `message.reasoning_content`、`delta.reasoning_content` | `Block::Reasoning` | 通过配置的 `reasoning_fields`。 |
| `user` | 省略 | Venice 会丢弃它（`without_end_user()`）；它自己的 `anon_user_id` 保留在 extras 中。 |
| `metadata`、`store` | 拒绝 | Venice 接受但不处理。 |
| Venice 不认识的其他字段 | 发送 | Venice 自己以 400 拒绝（`additionalProperties: false`），因此配置不再拒绝其他字段。 |
| `video_url` 部分、`wav` 和 `mp3` 以外格式的 `input_audio` | `Block::Video`、`Block::Audio` | `with_video_url()`、`any_audio_format()`。 |
| `cost`、回复中的 `venice_parameters`、`choices[].stop_reason`、`prompt_logprobs` | extras | 发往其他方言时省略。 |
| 带有 `choices: []`、`usage` 和 `cost` 的最后一个流块 | usage | Venice 在每个流中都会发送，即使没有 `stream_options.include_usage`。 |
| 错误 `{"error": "<文本>"}` 和 `{"error", "details", "issues"}` | `KppError` | 类型取决于状态码，文本即消息（`errors/recorded-invalid-api-key`、`errors/unknown-model-published`）。 |
| `GET /v1/models` | `ModelList` | 只列出文本模型（`models/list-recorded`），因此列表不完整。 |

Venice 的 Responses API 处于测试阶段，未被使用：Responses 和 Messages 应用通过 Chat Completions 访问 Venice，Kaeso 自行估算 Token 计数，因为 Venice 不提供。

## Baidu Qianfan

来源：千帆的[文本生成](https://cloud.baidu.com/doc/qianfan-api/s/wmwbl48n4)和[视觉理解](https://cloud.baidu.com/doc/qianfan-api/s/ymwbl48ky)参考、[公共请求头](https://cloud.baidu.com/doc/qianfan-api/s/Nmwbl43w1)、[错误码](https://cloud.baidu.com/doc/qianfan-api/s/rmwbl43x7)，以及[深度思考](https://cloud.baidu.com/doc/qianfan-docs/s/Wm95lyynv)和[联网搜索](https://cloud.baidu.com/doc/qianfan-docs/s/Wm8r4sw29)指南（2026-11-14 阅读），以及 `fixtures/openai_chat/providers/qianfan/`：参考文档中的请求和回复、一个公开的回复、联网搜索指南中的流（省略其中被删节的部分），以及在没有有效密钥时实际返回的密钥错误和网关错误。

| 千帆字段 | KPP | 说明 |
|---|---|---|
| `<base>/chat/completions`、`<base>/embeddings`、`<base>/models` | 服务商的 API 根路径 | 千帆的 API 位于 `/v2`；基础 URL 止于此处，根路径为空（[API 根路径](#api-根路径)）。 |
| `system` 角色，无 `developer` | `Role::System` | `system_role()`。 |
| `user` | 最终用户 | `end_user_as_user()`。 |
| `thinking: {"type": "enabled" \| "disabled"}`、`reasoning_effort` | `reasoning` | `thinking_type()`；`low` 和 `medium` 按 `high` 处理，`xhigh` 按 `max` 处理。 |
| `message.reasoning_content`、`delta.reasoning_content` | `Block::Reasoning` | `reasoning_fields`。 |
| `max_completion_tokens`、`max_tokens` | `max_output_tokens` | 两者都计入思考和回答。 |
| `choices[].flag`、`ban_round`、`search_results`、`delta_tag` | extras | 千帆的安全标记和联网搜索来源；对 Chat 应用可往返。 |
| `web_search`、`penalty_score`、`enable_thinking`、`thinking_budget`、`metadata` | extras | 按 Chat 应用写入的原样发送。 |
| `logprobs`、`top_logprobs`、`logit_bias`、`store`、`service_tier`、`prompt_cache_key`、`prompt_cache_retention`、`verbosity`、`modalities`、`audio`、`prediction`、`web_search_options`、`functions`、`function_call` | 拒绝 | 不在参考文档中，且千帆没有说明是否拒绝未声明的字段。 |
| OpenAI 格式的错误，带千帆的 `code`；BCE 网关错误为 `{"requestId", "code", "message"}` | `KppError` | `tokens_too_long` 和 `characters_too_long` 为上下文过长，`account_overdue` 为计费错误，`model_offline` 为未找到。错误的密钥在聊天时得到 401 `invalid_iam_token`，在模型列表上得到 429 `OverRateLimit`（`errors/recorded-*`）。 |
| `GET /models`，带千帆的 `status`、`pricing`、`features` 和 `endpoints` | `ModelList` | 需要密钥；不完整，已部署的服务不在其中（`models/list-doc`）。 |
