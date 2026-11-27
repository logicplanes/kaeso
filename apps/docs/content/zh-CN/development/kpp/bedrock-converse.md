---
title: Amazon Bedrock Converse
description: KPP 的生成请求如何映射到 Amazon Bedrock 的 Converse API，Kaeso 以此格式调用 Nova、Llama 和 Bedrock 的其他聊天模型。
order: 9
---

在这里查找生成请求如何到达 Amazon Bedrock 的 Converse API：哪个字段放在哪里、如何读取二进制流，以及每项映射的来源。

- **编解码器：**`kaeso_codecs::bedrock_converse::BedrockConverse`（`crates/kaeso-codecs/src/bedrock_converse/`）
- **路径：**`POST /model/{modelId}/converse` 和 `/model/{modelId}/converse-stream`，模型 ID 或推理配置文件经百分号编码，位于密钥所在区域的运行时主机上
- **测试数据：**`crates/kaeso-codecs/fixtures/bedrock_converse/`，依据下列参考资料构建，没有录制的数据

**来源。**[Converse](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_Converse.html)、[ConverseStream](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_ConverseStream.html)、[ContentBlock](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_ContentBlock.html)、[Amazon Nova 的工具调用](https://docs.aws.amazon.com/nova/latest/userguide/tool-use-invocation.html)和[事件流编码](https://smithy.io/2.0/aws/amazon-eventstream.html)，阅读于 2026-11-16。

## 决定

- **仅用于出站。**没有应用向 Kaeso 发送 Converse；该编解码器不解码 Converse 请求，也不以它写回复。
- **哪些模型。**KPMR 把 `bedrock_converse` 设为 Bedrock 中既不支持 Chat Completions 也不支持 Messages 的聊天模型的出站方言，例如 Amazon Nova、Meta Llama 和 Mistral Pixtral。Messages 支持的 Claude 模型仍走 Messages。
- **密钥。**Converse 接受作为 Bearer 令牌的 Bedrock API 密钥，以及用 Signature Version 4 签名的 AWS 凭证。
- **模型系列特有的字段暂不写入。**推理设置、`top_k`、JSON 输出格式以及其他放在 `additionalModelRequestFields` 中的字段为 `Unsupported`，绝不丢弃。

## 请求

| KPP | Converse |
| --- | --- |
| `system` 中的文本块 | `system[]` `{text}` |
| `messages[]` | `messages[]` `{role, content[]}`；同一角色的连续消息会合并，因为 Converse 要求角色交替 |
| 文本、拒绝 | `{text}` |
| 图片，以字节或 `data:` URL 提供 | `{image: {format, source: {bytes}}}`，格式为 `png`、`jpeg`、`gif` 或 `webp` |
| 文档 | `{document: {format, name, source}}`：字节，或文本文档的 `text`；`name` 取自标题，否则为 `Document N` |
| 工具调用 | `{toolUse: {toolUseId, name, input}}` |
| 工具结果 | `{toolResult: {toolUseId, content[], status}}`，内容为 `text`、`json`、`image` 或 `document`，出错时为 `status: "error"` |
| Converse 自己的推理 | `{reasoningContent: {reasoningText: {text, signature}}}` 或 `{redactedContent}` |
| 缓存提示 | 在带提示的块或工具之后加一个 `{cachePoint: {type: "default", ttl}}` 块，`ttl` 为 `5m` 或 `1h` |
| `max_output_tokens`、`temperature`、`top_p`、`stop` | `inferenceConfig.maxTokens`、`temperature`、`topP`、`stopSequences` |
| 函数工具 | `toolConfig.tools[].toolSpec` `{name, description, inputSchema: {json}, strict}` |
| `tool_choice` 为 auto、required 或某个工具 | `toolConfig.toolChoice` `{auto}`、`{any}`、`{tool: {name}}` |

Converse 的规则不接受的工具名称和调用 ID（名称为 `[a-zA-Z0-9_-]`，ID 为 `[a-zA-Z0-9_.:-]`，最多 64 个字符）会在发送时改写，并在回复中还原。

以 `Unsupported` 拒绝：通过网络链接或文件 ID 提供的图片和文档、音频和视频块、文档引用、托管工具、`tool_choice: none`、`parallel_tool_calls: false`、推理设置、结构化输出、`top_k`、`seed` 和惩罚参数，但值为 `0` 的惩罚参数会被省略（[中性值](sampling.md#中性值)）。其他格式的推理不会发送。

## 回复

| Converse | KPP |
| --- | --- |
| `output.message.content[]` `text` | 文本块 |
| `toolUse` | 工具调用，其 ID 和名称与请求发送时一致 |
| `reasoningContent` | 推理块，签名或被遮蔽的内容作为不透明数据 |
| 其他块 | 未知块 |
| `stopReason` 为 `end_turn`、`tool_use`、`max_tokens`、`stop_sequence` | 相同的停止原因 |
| `guardrail_intervened`、`content_filtered` | `content_filter` |
| `model_context_window_exceeded` | `max_tokens` |
| `usage.inputTokens`、`outputTokens`、`cacheReadInputTokens`、`cacheWriteInputTokens` | 输入、输出、缓存读取和缓存写入 Token |
| `x-amzn-requestid` 头 | 回复的 ID |

## 流

`converse-stream` 以 AWS 的二进制事件流编码（`application/vnd.amazon.eventstream`）作答，而不是服务器发送事件。`bedrock_converse::eventstream::Framer` 读取其中的消息：12 字节的前导部分（总长度、头部长度及其 CRC32）、头部、负载和消息的 CRC32。校验和不匹配时，流以错误结束。

| 事件 | KPP |
| --- | --- |
| `messageStart` | 回复开始 |
| 带 `toolUse` 的 `contentBlockStart` | 一个工具调用块开始 |
| `contentBlockDelta` 的 `text`、`toolUse.input`、`reasoningContent` | 文本、工具参数、推理文本或不透明数据的增量；文本块或推理块随第一个增量开始 |
| `contentBlockStop` | 该块结束 |
| `messageStop` | 停止原因，保留到用量到达 |
| `metadata` | 用量，随后回复结束 |
| 异常消息，例如 `throttlingException` | 应用格式的错误事件 |

## 错误

Bedrock 在 `x-amzn-errortype` 头或正文的 `__type` 中给出错误类型，在 `message` 中给出消息：

| 错误 | KPP |
| --- | --- |
| `ValidationException` | `invalid_request`；消息表明输入过长时为 `context_too_long` |
| `ThrottlingException`、`ServiceQuotaExceededException` | `rate_limited` |
| `AccessDeniedException` | `permission_denied`，即被拒绝的密钥 |
| `UnrecognizedClientException`、`InvalidSignatureException`、`ExpiredTokenException` | `authentication` |
| `ModelNotReadyException`、`ServiceUnavailableException` | `overloaded` |
| `ModelTimeoutException` | `timeout` |
| `ResourceNotFoundException` | `not_found` |
| `ModelErrorException`、`InternalServerException` | `provider_error` |
