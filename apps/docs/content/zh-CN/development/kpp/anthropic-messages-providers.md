---
title: Messages 服务商
description: Messages 编解码器如何为每个使用该格式的服务商调整请求，以及每个服务商增加了什么。
order: 4
nav: Messages 服务商
---

除 Anthropic 外，还有其他服务商使用 [Anthropic Messages](anthropic-messages.md)，各有差异。在添加服务商或修改其配置之前，请在这里查看编解码器如何为每个服务商调整请求，以及每个服务商增加了什么。

提供 Messages 的其他服务商（Anthropic 以外）会录制到 `fixtures/anthropic_messages/providers/<id>/`（[fixture](fixtures.md)）。它们的额外字段遵循 [Anthropic Messages](anthropic-messages.md) 的规则；本页列出每个服务商增加的内容。

`PROVIDERS` 中的每个服务商都在其 `messages` 字段中指定一个 `kaeso_codecs::anthropic_messages::MessagesProfile`，网关通过 `EgressCtx::with_messages_profile` 把它交给编码器，编码器将它应用于每个请求，无论应用发送的是哪种格式。默认的 `MessagesProfile::ANTHROPIC` 按应用原样发送请求；`MessagesProfile::compatible(name)` 与之相同，只是带上服务商的名称。自定义服务商使用 `ANTHROPIC`。

| 设置 | 编码器的行为 | 为什么允许 |
|---|---|---|
| `refusing(fields)` | 拒绝设置了这些字段之一的请求，字段使用 Anthropic 中的名称：`stop_sequences`（按应用的写法命名，例如 Chat 应用的 `stop`），或 Anthropic 应用的某个顶层字段，例如 `container`：`` `container` is not supported by Example Cloud; remove it from the request. `` 提示字段 `top_k` 会被省略，并按应用的写法在 `kaeso-left-out` 中注明（[top K](sampling.md#top-k)）。在跨格式时可以省略的字段，例如 Claude Code 只清除思考内容的 `context_management`（[extras](cross-format.md#extras)），会被省略并在 `kaeso-left-out` 中注明；它的其他取值会被拒绝，并附上针对 Claude Code 的提示。 | 这些字段是语义字段，接受却不处理它们的服务商会悄悄丢弃它们。 |
| `sampling_fixed_at(values)` | 拒绝与服务商始终使用的采样值（`FixedValue`，以千分之一为单位）不同的 `temperature` 或 `top_p`；该值本身按原样发送。 | 服务商忽略该字段，因此只有它自己的值会被遵守。 |
| `auto_tool_choice_only()` | 拒绝任何格式中不为 `auto` 的 `tool_choice`。 | 无论应用要求什么，服务商都会自己选择工具。 |
| `with_video(media_types)` | 把 `Block::Video` 作为 `video` 块发送；其他媒体类型的 base64 来源为 `Unsupported`。 | 只有在文档中写明该块的服务商才会收到它。 |
| `forced_tool_without_thinking()` | 当请求的 `tool_choice` 强制使用某个工具（`any` 或指定的工具）时，以 `thinking: {"type": "disabled"}` 发送，并在 `kaeso-adapted` 中注明应用的推理字段；来自其他格式的推理强度 `none` 也同样处理。声明了 `deepseek_web_search` 等服务端工具时的 `any` 按原样发送，因为 DeepSeek 在思考时将其视为 `auto`。 | 该服务商会拒绝在模型思考时强制使用工具，而其模型除非被告知不要思考，否则都会思考。 |
| `without_custom_tool_type()` | 发送应用的工具时不带 `type: "custom"`，解码器把它保存为形式标记。 | 没有 `type` 的工具本来就是 `custom`；只把 `type` 当作服务端工具来读的服务商会拒绝它。 |
| `raw_predict()` | 将生成请求发送到 `{base_path}/{model}:rawPredict`（流式时为 `:streamRawPredict`），不含 `model`；将 Token 计数发送到 `{base_path}/count-tokens:rawPredict`，保留 `model`；两者都带上 `anthropic_version` `vertex-2023-10-16`，除非应用自己发送了。Google 格式的错误按 Gemini 错误读取。 | Google 平台提供 Claude 的方式（[使用 Claude](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/partner-models/claude/use-claude)、[计算 Token](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/partner-models/claude/count-tokens)、Anthropic 的 SDK）。Gemini Enterprise Agent Platform 使用它，并拒绝 [Anthropic 列为](https://platform.claude.com/docs/en/build-with-claude/claude-models-on-google-cloud/google-managed)不可用的 `mcp_servers` 和 `container`。 |

拒绝发生在为目标调整请求时、编码之前，因此不会发送任何内容，拒绝以应用自己的格式送达。

## 原生 Messages API

许多自身格式为 Chat Completions 的服务商也提供 Messages，作为 Anthropic 的格式供 Claude Code 和其他 Anthropic 应用使用。`Provider::native` 以 `NativeDialect` 条目列出它们：方言；与服务商的 API 根路径不同时，它自己的 API 根路径（DeepSeek、Moonshot 和 DeepInfra 为 `at("/anthropic/v1")`；根路径从主机开始、而不在基础 URL 的路径之下时为 `from_host_root(…)`，例如 QwenCloud 与 `/compatible-mode/v1` 并列的 `/apps/anthropic/v1`，以及千帆与 `/v2` 并列的 `/anthropic/v1`）；不同时它的认证请求头；它提供的模型（`for_models(&["kimi-k3*"])`；为空表示全部）；以及它的 Token 计数是否在那里响应（`counting_tokens()`）。每个条目也在服务商的 `dialects` 中，因此模型的**发送格式**可以指定它。

当服务商为该模型列出应用自己的方言、且模型的目录条目未指定出站方言时，网关的 `pipeline::native_egress` 以该方言发送生成请求；Token 计数则只在条目支持计数 Token 时如此。KPMR 或组织设置的出站方言始终优先，这样可以让模型继续使用格式转换。请求仍会解码为 KPP 再重新编码，因此服务商的配置与对任何应用一样生效，`kaeso-left-out` 会注明它省略的内容。

| 服务商 | 原生 Messages API | 模型 | Token 计数 | 来源 |
|---|---|---|---|---|
| DeepSeek（`deepseek`） | `/anthropic/v1/messages` | 所有模型 | — | [Anthropic API](https://api-docs.deepseek.com/guides/anthropic_api)、[Messages API](https://api-docs.deepseek.com/api/create-message.md) |
| Moonshot（`moonshotai`） | `/anthropic/v1/messages` | `kimi-k3*` | — | [Messages](https://platform.kimi.ai/docs/api/messages.md) |
| QwenCloud（`alibaba`） | 主机的 `/apps/anthropic/v1/messages` | `qwen*` | — | [Anthropic Messages API](https://docs.qwencloud.com/api-reference/chat/anthropic.md) |
| Baidu Qianfan（`qianfan`） | 主机的 `/anthropic/v1/messages` | 所有聊天模型 | — | [Anthropic 兼容性](https://cloud.baidu.com/doc/qianfan-docs/s/6mh3e6gjp) |
| DeepInfra（`deepinfra`） | `/anthropic/v1/messages` | 所有聊天模型 | `/anthropic/v1/messages/count_tokens` | [Anthropic](https://docs.deepinfra.com/integrations/anthropic) |
| OpenRouter（`openrouter`） | `/api/v1/messages` | 所有模型 | — | [创建消息](https://openrouter.ai/docs/api/api-reference/anthropic-messages/create-a-message.md) |
| Hugging Face（`huggingface`） | `/v1/messages` | 所有聊天模型 | — | [Messages API](https://huggingface.co/docs/inference-providers/guides/messages-api.md) |
| Ollama Cloud（`ollama-cloud`） | `/v1/messages` | 所有模型 | — | [Anthropic 兼容性](https://docs.ollama.com/api/anthropic-compatibility.md) |
| Tencent TokenHub（`tencent-tokenhub`） | `/v1/messages` | 所有模型 | — | [Anthropic 兼容性](https://cloud.tencent.com/document/product/1823/130079) |
| Fireworks AI（`fireworks-ai`） | `/inference/v1/messages` | 所有模型 | — | [Anthropic 兼容性](https://docs.fireworks.ai/tools-sdks/anthropic-compatibility.md) |
| SambaNova（`sambanova`） | `/v1/messages` | 所有语言模型 | `/v1/messages/count_tokens` | [Messages](https://docs.sambanova.ai/docs/en/features/messages) |
| Vercel（`vercel`） | `/v1/messages` | 所有模型 | — | [Anthropic 兼容性](https://vercel.com/docs/ai-gateway/sdks-and-apis/anthropic-compat.md) |

Baseten 和 Standard Compute 的 Messages 端点不算原生：它们会丢弃思考内容，而它们的 Chat Completions 会保留。SpaceX 的 Messages API 已弃用，Venice 和 Perplexity 只为其他产品提供 Messages，因此它们都未列出。所有页面均于 2026-11-25 阅读。

QwenCloud 的 Messages API 以 DashScope 的错误体 `{"request_id", "code", "message"}` 返回错误，被拒绝的流则以 `event:error` 帧携带同一错误体。解码器按错误码读取两者，与 Chat Completions 相同：`InvalidApiKey` 为密钥被拒，`Throttling…` 为速率限制，`DataInspection…` 为内容过滤，`AllocationQuota…`、`Arrearage…` 以及免费额度用尽时的 `AccessDenied` 为计费拒绝（`fixtures/anthropic_messages/providers/alibaba/errors/`）。

| 服务商 | 设置 | 来源 |
|---|---|---|
| Amazon Bedrock（`amazon-bedrock`） | 拒绝 `mcp_servers` 和 `container` | Anthropic 关于 bedrock-runtime 的 [Claude on Amazon Bedrock](https://platform.claude.com/docs/en/build-with-claude/claude-on-amazon-bedrock-legacy) 页面，2026-11-14 阅读，其中列出 MCP 连接器和 Agent Skills 不受支持 |
| DeepSeek（`deepseek`） | 拒绝 `container` 和 `mcp_servers`，省略 `top_k`；`without_custom_tool_type()`, `forced_tool_without_thinking()` | DeepSeek 的 [Anthropic API](https://api-docs.deepseek.com/guides/anthropic_api) 页面，2026-11-27 阅读，其中将这三个字段列为被忽略，并说明客户端工具带 `type: "custom"` 会返回 422 |
| MiniMax（`minimax`） | 拒绝 `stop_sequences`、`mcp_servers`、`container` 和 `context_management`，省略 `top_k`；`sampling_fixed_at([("temperature", 1), ("top_p", 0.95)])`；`auto_tool_choice_only()`；`with_video(MINIMAX_VIDEO_MEDIA_TYPES)` | MiniMax 的 [Anthropic API 兼容性](https://platform.minimax.io/docs/api-reference/text-anthropic-api)页面（2026-11-08 阅读），其表格将 `temperature`、`top_p`、`top_k`、`stop_sequences`、`mcp_servers`、`context_management` 和 `container` 列为被忽略，说明 MiniMax 始终使用 1 的 `temperature` 和 0.95 的 `top_p`，并且 `tool_choice` 只支持 `auto` |
| Ollama Cloud（`ollama-cloud`） | `auto_tool_choice_only()` | Ollama 的 [Anthropic 兼容性](https://docs.ollama.com/api/anthropic-compatibility.md)页面，2026-11-25 阅读，其中将 `tool_choice` 列为不支持 |

## Amazon Bedrock

来源：AWS 的 [Messages 指南](https://docs.aws.amazon.com/bedrock/latest/userguide/inference-messages-api.html)和 Anthropic 的 [Claude on Amazon Bedrock](https://platform.claude.com/docs/en/build-with-claude/claude-on-amazon-bedrock-legacy) 页面（2026-11-14 阅读），以及 `fixtures/anthropic_messages/providers/amazon-bedrock/` 中实时得到的错误。当目录为某个 Bedrock 模型注明 Messages 格式时，该模型以 Messages 发送；对于 AWS API 兼容性表中列出 Messages 的 Claude 模型，KPMR 会这样注明。未通过成功的调用测试：AWS 未公开此路径的任何回复。

| Amazon Bedrock 字段 | KPP | 说明 |
|---|---|---|
| `bedrock-runtime.{region}.amazonaws.com` 上 `/anthropic/v1/messages` 的 Messages | `EgressCtx::api_root` | `/anthropic/v1` 取代 `/v1`；密钥作为 `x-api-key` 发送，`model` 保留在请求体中，`anthropic-version` 为 `2023-06-01`，与 AWS 的 `curl` 示例一致。 |
| 以 403 `permission_error` 返回的被拒绝密钥 | `KppError` | 按类型为 `PermissionDenied`；网关把 Bedrock 针对格式错误、未签名和未知密钥的消息视为被拒绝的密钥（`errors/recorded-*-key*`）。 |
| `POST /v1/messages/count_tokens` | — | bedrock-runtime 只在 `/model/{modelId}/count-tokens` 以自有格式计数，Anthropic 格式的计数只在 bedrock-mantle 上提供（见[计算 Token](https://docs.aws.amazon.com/bedrock/latest/userguide/count-tokens.html)），因此 Kaeso 自行估算 Token 计数。 |
| 流 | Anthropic 的 SSE | AWS 对此路径的流只说明 Anthropic SDK 可以使用它，没有更多描述。 |

## Vercel

来源：Vercel 的 [Anthropic Messages API](https://vercel.com/docs/ai-gateway/sdks-and-apis/anthropic-messages-api) 和 [extended thinking](https://vercel.com/docs/ai-gateway/sdks-and-apis/anthropic-messages-api/reasoning) 页面（2026-10-29 阅读），以及 `fixtures/anthropic_messages/providers/vercel/` 中的实时录制。当 Vercel 模型的**发送格式**为 Messages 时，它以 Messages 发送；Vercel 的 `generation` 方言是 Chat Completions。

| Vercel 字段 | KPP | 说明 |
|---|---|---|
| 回复中的 `provider_metadata` | extras | Anthropic 应用原样收到。 |
| 没有 `signature` 的 `thinking` 块 | `Block::Reasoning` | Anthropic 应用按 Vercel 发送的样子收到。 |
| OpenAI 封装中的错误 | `KppError` | 见[错误](anthropic-messages.md#错误)。 |
| `POST /v1/messages/count_tokens` | — | Vercel 只对 Claude 模型精确计数，其余为估算，因此 Kaeso 对所有 Vercel 模型都自行估算 Token 计数。 |

## Z.ai

来源：Z.ai 的[错误码](https://docs.z.ai/api-reference/api-code)、[快速开始](https://docs.z.ai/devpack/quick-start)和[思考设置](https://docs.z.ai/devpack/latest-model)页面（2026-11-08 阅读），以及 `fixtures/anthropic_messages/providers/zai/` 中实时录制和探测的错误。Messages 是 Z.ai 的 `generation` 方言，地址为 `https://api.z.ai/api/anthropic`。Z.ai 没有说明 Anthropic 回复、流或它接受的字段列表，因此请求按发给 Anthropic 的方式发送，Kaeso 不为 Z.ai 拒绝任何字段，Z.ai 自己的错误会送达应用。它的思考设置页面列出了 `thinking.type` `adaptive` 和 `output_config.effort`，其他方言的推理强度会转换为它们。

| Z.ai 字段 | KPP | 说明 |
|---|---|---|
| OpenAI 封装中作为 `error.type` 的 Z.ai 数字错误码（`errors/recorded-invalid-api-key`） | `KppError` 类别 | Z.ai 对某些错误码使用的状态码在 Anthropic 的表中含义不同，例如余额不足时返回 429。以错误码为准：`1113`、`1309` 和 `1314` → `Billing`；`1211` → `NotFound`；`1220`、`1311` 和 `1315` → `PermissionDenied`；`1261` → `ContextTooLong`；`1301` → `ContentFiltered`；`1305` → `Overloaded`；`1000`、`1001`、`1003` 和 `1005` → `Authentication`。其他错误码按状态码处理。 |
| `{"code", "msg", "success": false}`，也可能带 HTTP 200（`errors/list-invalid-api-key`） | `KppError` | Z.ai 的平台封装，它在检查密钥之前用它回应自己不提供的路径，其中包括 `GET /v1/models`。`decode_models` 和 `decode_model` 无论状态码如何都把它读作错误，因此用错误的密钥检查模型时会报告密钥被拒绝。 |
| `POST /v1/messages/count_tokens` | — | 没有相关说明；Kaeso 不发送任何内容，自行估算 Z.ai 模型的 Token 计数。 |
## MiniMax

来源：MiniMax 的 [Anthropic API 兼容性](https://platform.minimax.io/docs/api-reference/text-anthropic-api)页面及其 [OpenAPI 文档](https://platform.minimax.io/docs/api-reference/text/api/openapi-chat-anthropic.json)（2026-11-08 阅读）、`fixtures/anthropic_messages/providers/minimax/` 中取自该文档的示例，以及用一个虚构密钥实时录制的一个错误。Messages 是 MiniMax 的 `generation` 方言，地址为 `https://api.minimax.io/anthropic`，用于 `MiniMax-M3` 和 `MiniMax-M3.1-Flash`。

| MiniMax 字段 | KPP | 说明 |
|---|---|---|
| 由 32 位十六进制数字组成、没有 `msg_` 的回复 `id`，没有 `stop_sequence` | `id`，form `none` | Anthropic 应用按 MiniMax 发送的样子收到回复（`responses/deep-thinking`）。 |
| 带有 MiniMax 自己的 `signature` 的 `thinking` 块，以及 `signature_delta` | 带有来源为 `minimax` 的不透明 Anthropic 签名的 `Block::Reasoning` | MiniMax 要求在之后的轮次中原样发回这些块。其他格式只把它们回放给 MiniMax（[推理数据块](reasoning.md#推理数据块)）；Anthropic 应用自己的回放按原样发送，Anthropic 会拒绝 MiniMax 的签名。 |
| `message_start` 中的 `service_tier` | extras | `streams/image-url-stream`。 |
| `video` 内容块（请求） | `Block::Video` | MiniMax 自己的块；只有 MiniMax 的配置会发送它，Anthropic 会拒绝它。 |
| `thinking.type` `adaptive` 和 `output_config.effort`（请求） | `reasoning` | MiniMax 只说明了 `adaptive` 和 `disabled`；`MiniMax-M3.1-Flash` 以 400 拒绝 `disabled`，该错误会送达应用。 |
| `POST /v1/messages/count_tokens` | — | MiniMax 文档中的响应把计数放在 `usage` 下而不是顶层，因此 Kaeso 不发送任何内容，自行估算 MiniMax 模型的 Token 计数。 |
| `GET /v1/models`、`GET /v1/models/{model_id}` | `ModelList`、`ModelInfo` | Anthropic 的结构（`models/list-documented`、`models/retrieve-documented`）；错误的密钥会得到 401，因此**检查可用性**能发现它。 |
| 错误 | `KppError` | Anthropic 的结构，带有 MiniMax 自己的 `request_id`（`errors/recorded-invalid-api-key`）。 |

## Fireworks AI

来源：Fireworks 的 [Create a Message](https://docs.fireworks.ai/api-reference/anthropic-messages) 参考和 [Anthropic compatibility](https://docs.fireworks.ai/tools-sdks/anthropic-compatibility) 页面（2026-11-09 阅读），以及据此构建的 `fixtures/anthropic_messages/providers/fireworks-ai/`，其中的错误是在没有有效密钥的情况下实时返回的。当 Fireworks 模型的**发送格式**为 Messages 时，它以 Messages 发送；密钥以 `Authorization: Bearer` 发送。

| Fireworks 字段 | KPP | 说明 |
|---|---|---|
| `output_config.effort`、带 `budget_tokens` 的 `thinking` | `reasoning` | Fireworks 将两者都映射到它的 `reasoning_effort`。 |
| 用量为零的 `message_start`，计数在唯一的 `message_delta` 中 | `Usage` | 与兼容性页面描述一致；`cache_creation_input_tokens` 始终为 0。 |
| `document` 块、来源为文件的 `image` 块、服务器工具、思考时强制的 `tool_choice` 或预填充 | 发送 | Fireworks 返回 400，该错误会到达应用。 |
| 带 `request_id` 的 Anthropic 封装中的错误（`errors/recorded-invalid-api-key`） | `KppError` | 未知的模型或路径会得到 Fireworks 自己的 `{"error": {"code": …}}` 正文，按 OpenAI 的格式读取。 |
| `POST /v1/messages/count_tokens` | — | Fireworks 返回 404，因此 Kaeso 不发送任何内容，自行估算 Token 计数。 |

## SambaNova

来源：SambaNova 的 [Messages API](https://docs.sambanova.ai/docs/en/features/messages) 和 [Anthropic compatibility](https://docs.sambanova.ai/docs/en/features/anthropic-compatibility) 页面以及其 [OpenAPI document](https://raw.githubusercontent.com/sambanova/sambanova-inference-api-spec/refs/heads/main/openapi.documented.json) 中的 `Message` schema（2026-11-09 阅读），以及据此构建的 `fixtures/anthropic_messages/providers/sambanova/`，其中的错误是在没有有效密钥的情况下实时返回的。当 SambaNova 模型的**发送格式**为 Messages 时，它以 Messages 发送；密钥以 `Authorization: Bearer` 发送。

| SambaNova 字段 | KPP | 说明 |
|---|---|---|
| 没有 `signature` 的 `thinking` 块 | `Block::Reasoning` | 推理模型无需任何思考设置就会发送它们（`responses/thinking`）。 |
| 服务器工具、`document` 块、通过 URL 的图像 | 发送 | SambaNova 返回 400，该错误会到达应用。 |
| 带 `request_id` 的 Anthropic 封装中的错误（`errors/recorded-invalid-api-key`） | `KppError` | |
| `POST /v1/messages/count_tokens` | `input_tokens` | Anthropic 的格式。无论模型以哪种格式发送，Kaeso 都在这里计算每个 SambaNova 模型的 Token。 |

## 后续步骤

- [Anthropic Messages](anthropic-messages.md)：查看 Anthropic Messages 格式的每个字段在 KPP 中的去向。
- [Fixture 和录制](fixtures.md)：使用 `cargo xtask record anthropic_messages --provider <id>` 录制服务商的 fixture。
