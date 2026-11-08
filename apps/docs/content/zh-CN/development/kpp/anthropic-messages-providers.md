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
| `refusing(fields)` | 拒绝设置了这些字段之一的请求，字段使用 Anthropic 中的名称：`top_k`、`stop_sequences`（按应用的写法命名，例如 Chat 应用的 `stop`），或 Anthropic 应用的某个顶层字段，例如 `container`：`` `top_k` is not supported by Example Cloud; remove it from the request. `` 在跨格式时可以省略的字段，例如 Claude Code 只清除思考内容的 `context_management`（[extras](cross-format.md#extras)），会被省略并在 `kaeso-left-out` 中注明；它的其他取值会被拒绝，并附上针对 Claude Code 的提示。 | 这些字段是语义字段，接受却不处理它们的服务商会悄悄丢弃它们。 |
| `sampling_fixed_at(values)` | 拒绝与服务商始终使用的采样值（`FixedValue`，以千分之一为单位）不同的 `temperature` 或 `top_p`；该值本身按原样发送。 | 服务商忽略该字段，因此只有它自己的值会被遵守。 |
| `auto_tool_choice_only()` | 拒绝任何格式中不为 `auto` 的 `tool_choice`。 | 无论应用要求什么，服务商都会自己选择工具。 |
| `with_video(media_types)` | 把 `Block::Video` 作为 `video` 块发送；其他媒体类型的 base64 来源为 `Unsupported`。 | 只有在文档中写明该块的服务商才会收到它。 |
| `raw_predict()` | 将生成请求发送到 `{base_path}/{model}:rawPredict`（流式时为 `:streamRawPredict`），不含 `model`；将 Token 计数发送到 `{base_path}/count-tokens:rawPredict`，保留 `model`；两者都带上 `anthropic_version` `vertex-2023-10-16`，除非应用自己发送了。Google 格式的错误按 Gemini 错误读取。 | Google 平台提供 Claude 的方式（[使用 Claude](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/partner-models/claude/use-claude)、[计算 Token](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/partner-models/claude/count-tokens)、Anthropic 的 SDK）。Gemini Enterprise Agent Platform 使用它，并拒绝 [Anthropic 列为](https://platform.claude.com/docs/en/build-with-claude/claude-models-on-google-cloud/google-managed)不可用的 `mcp_servers` 和 `container`。 |

拒绝发生在为目标调整请求时、编码之前，因此不会发送任何内容，拒绝以应用自己的格式送达。

| 服务商 | 设置 | 来源 |
|---|---|---|
| MiniMax（`minimax`） | 拒绝 `top_k`、`stop_sequences`、`mcp_servers`、`container` 和 `context_management`；`sampling_fixed_at([("temperature", 1), ("top_p", 0.95)])`；`auto_tool_choice_only()`；`with_video(MINIMAX_VIDEO_MEDIA_TYPES)` | MiniMax 的 [Anthropic API 兼容性](https://platform.minimax.io/docs/api-reference/text-anthropic-api)页面（2026-11-08 阅读），其表格将 `temperature`、`top_p`、`top_k`、`stop_sequences`、`mcp_servers`、`context_management` 和 `container` 列为被忽略，说明 MiniMax 始终使用 1 的 `temperature` 和 0.95 的 `top_p`，并且 `tool_choice` 只支持 `auto` |

## Vercel

来源：Vercel 的 [Anthropic Messages API](https://vercel.com/docs/ai-gateway/sdks-and-apis/anthropic-messages-api) 和 [extended thinking](https://vercel.com/docs/ai-gateway/sdks-and-apis/anthropic-messages-api/reasoning) 页面（2026-10-29 阅读），以及 `fixtures/anthropic_messages/providers/vercel/` 中的实时录制。当 Vercel 模型的**发送格式**为 Messages 时，它以 Messages 发送；Vercel 的 `generation` 方言是 Chat Completions。

| Vercel 字段 | KPP | 说明 |
|---|---|---|
| 回复中的 `provider_metadata` | extras | Anthropic 应用原样收到。 |
| 没有 `signature` 的 `thinking` 块 | `Block::Reasoning` | Anthropic 应用按 Vercel 发送的样子收到。 |
| OpenAI 封装中的错误 | `KppError` | 见[错误](anthropic-messages.md#错误)。 |
| `POST /v1/messages/count_tokens` | — | Vercel 只对 Claude 模型精确计数，其余为估算，因此 Kaeso 拒绝所有 Vercel 模型的 Token 计数。 |

## Z.ai

来源：Z.ai 的[错误码](https://docs.z.ai/api-reference/api-code)、[快速开始](https://docs.z.ai/devpack/quick-start)和[思考设置](https://docs.z.ai/devpack/latest-model)页面（2026-11-08 阅读），以及 `fixtures/anthropic_messages/providers/zai/` 中实时录制和探测的错误。Messages 是 Z.ai 的 `generation` 方言，地址为 `https://api.z.ai/api/anthropic`。Z.ai 没有说明 Anthropic 回复、流或它接受的字段列表，因此请求按发给 Anthropic 的方式发送，Kaeso 不为 Z.ai 拒绝任何字段，Z.ai 自己的错误会送达应用。它的思考设置页面列出了 `thinking.type` `adaptive` 和 `output_config.effort`，其他方言的推理强度会转换为它们。

| Z.ai 字段 | KPP | 说明 |
|---|---|---|
| OpenAI 封装中作为 `error.type` 的 Z.ai 数字错误码（`errors/recorded-invalid-api-key`） | `KppError` 类别 | Z.ai 对某些错误码使用的状态码在 Anthropic 的表中含义不同，例如余额不足时返回 429。以错误码为准：`1113`、`1309` 和 `1314` → `Billing`；`1211` → `NotFound`；`1220`、`1311` 和 `1315` → `PermissionDenied`；`1261` → `ContextTooLong`；`1301` → `ContentFiltered`；`1305` → `Overloaded`；`1000`、`1001`、`1003` 和 `1005` → `Authentication`。其他错误码按状态码处理。 |
| `{"code", "msg", "success": false}`，也可能带 HTTP 200（`errors/list-invalid-api-key`） | `KppError` | Z.ai 的平台封装，它在检查密钥之前用它回应自己不提供的路径，其中包括 `GET /v1/models`。`decode_models` 和 `decode_model` 无论状态码如何都把它读作错误，因此用错误的密钥检查模型时会报告密钥被拒绝。 |
| `POST /v1/messages/count_tokens` | — | 没有相关说明；Kaeso 在发送任何内容之前拒绝 Z.ai 模型的 Token 计数。 |
## MiniMax

来源：MiniMax 的 [Anthropic API 兼容性](https://platform.minimax.io/docs/api-reference/text-anthropic-api)页面及其 [OpenAPI 文档](https://platform.minimax.io/docs/api-reference/text/api/openapi-chat-anthropic.json)（2026-11-08 阅读）、`fixtures/anthropic_messages/providers/minimax/` 中取自该文档的示例，以及用一个虚构密钥实时录制的一个错误。Messages 是 MiniMax 的 `generation` 方言，地址为 `https://api.minimax.io/anthropic`，用于 `MiniMax-M3` 和 `MiniMax-M3.1-Flash`。

| MiniMax 字段 | KPP | 说明 |
|---|---|---|
| 由 32 位十六进制数字组成、没有 `msg_` 的回复 `id`，没有 `stop_sequence` | `id`，form `none` | Anthropic 应用按 MiniMax 发送的样子收到回复（`responses/deep-thinking`）。 |
| 带有 MiniMax 自己的 `signature` 的 `thinking` 块，以及 `signature_delta` | 带有来源为 `minimax` 的不透明 Anthropic 签名的 `Block::Reasoning` | MiniMax 要求在之后的轮次中原样发回这些块。其他格式只把它们回放给 MiniMax（[推理数据块](reasoning.md#推理数据块)）；Anthropic 应用自己的回放按原样发送，Anthropic 会拒绝 MiniMax 的签名。 |
| `message_start` 中的 `service_tier` | extras | `streams/image-url-stream`。 |
| `video` 内容块（请求） | `Block::Video` | MiniMax 自己的块；只有 MiniMax 的配置会发送它，Anthropic 会拒绝它。 |
| `thinking.type` `adaptive` 和 `output_config.effort`（请求） | `reasoning` | MiniMax 只说明了 `adaptive` 和 `disabled`；`MiniMax-M3.1-Flash` 以 400 拒绝 `disabled`，该错误会送达应用。 |
| `POST /v1/messages/count_tokens` | — | MiniMax 文档中的响应把计数放在 `usage` 下而不是顶层，因此 Kaeso 在发送任何内容之前拒绝 MiniMax 模型的 Token 计数。 |
| `GET /v1/models`、`GET /v1/models/{model_id}` | `ModelList`、`ModelInfo` | Anthropic 的结构（`models/list-documented`、`models/retrieve-documented`）；错误的密钥会得到 401，因此**检查可用性**能发现它。 |
| 错误 | `KppError` | Anthropic 的结构，带有 MiniMax 自己的 `request_id`（`errors/recorded-invalid-api-key`）。 |

## 后续步骤

- [Anthropic Messages](anthropic-messages.md)：查看 Anthropic Messages 格式的每个字段在 KPP 中的去向。
- [Fixture 和录制](fixtures.md)：使用 `cargo xtask record anthropic_messages --provider <id>` 录制服务商的 fixture。
