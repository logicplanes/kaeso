---
title: Fixture 约定
nav: Fixture 约定
description: 每个构造的 fixture 都遵循的规则，以便编解码器测试能精确比较它。
order: 10
---

非实时录制的 fixture 遵循以下规则，以便测试能精确比较它们：

- **配对。** 基本名称相同的请求、回复和流属于同一组。流与响应是同一个回复，累积后与该响应完全一致；例外情况写在方言文件夹的 `README.md` 中。
- **数字一致。** 总数等于各部分之和，流报告的用量与其回复相同。
- **采用真实格式的虚构值。** ID 遵循服务商的真实格式。签名和加密的推理内容是随机的，但格式正确；重放时服务商会拒绝它们。
- **小而有效的文件。** 图片是 1×1 的 PNG，PDF 是一页且交叉引用表正确，音频是带 8 个静音采样的 WAV 头。
- **格式。** JSON 文件使用两个空格缩进，非 ASCII 字符按 UTF-8 原样写出。SSE 的 `data:` 行与线上一样，是单行 JSON。
- **虚构内容只出现在 `unknown-*` 中。** 其他 fixture 都不包含服务商未记录的字段、块或事件。

## Anthropic Messages

- 每个请求都假定 `anthropic-version: 2023-06-01`，该值不单独存储。只有需要 `anthropic-beta` 的请求才有 `.headers.json`。
- ID 形如 `msg_01…`、`toolu_01…`、`srvtoolu_01…`、`file_011C…` 和 `req_011C…`。
- `Message` 包含 `model`、`id`、`type`、`role`、`content`、`stop_reason`、`stop_sequence`、`stop_details` 和 `usage`，与 SDK 录制的流和文档一致。`container` 和 `diagnostics` 只在使用时才有值，因此省略。
- `usage` 包含 `input_tokens`、两项缓存计数、`cache_creation` 明细、`output_tokens`、`service_tier` 和 `inference_geo`。`output_tokens_details` 只在发生思考后出现，`server_tool_use` 只在运行过服务器工具后出现。
- `cache_creation_input_tokens` 等于其明细之和，`thinking_tokens` 不超过 `output_tokens`。
- `message_start` 携带回复的用量，其中 `output_tokens` 很小；最后一个 `message_delta` 携带累计计数。

## OpenAI Chat Completions

- 没有请求需要转发请求头，因此没有 `.headers.json` 文件。
- 流就是其请求加上 `"stream": true` 和 `stream_options.include_usage` 后返回的内容。其 chunk 的字段顺序为 `id`、`object`、`created`、`model`、`service_tier`、`system_fingerprint`、`choices`、`usage`、`obfuscation`。
- 第一个 chunk 携带 `delta.role`，并带有 `content: ""`，或带有 `content: null` 和第一个工具调用的头部，或带有 `content: null, refusal: ""`。最后一个 delta 之后是一个 `delta: {}` 的 chunk。
- 除最后一个 chunk 外，每个 chunk 都有 `"usage": null`；最后一个 chunk 带 `"choices": []` 并携带 `usage`。每个 chunk 都带有 `obfuscation`，这是规范默认包含的随机填充。
- `prompt_tokens` 包含缓存命中和缓存写入的 Token，`completion_tokens` 包含推理 Token。`total_tokens` 是两者之和。

## OpenAI Responses

- 文档示例使用规范中的模型 `gpt-6-astra`。手工构建的 fixture 使用带推理的 `gpt-5.5` 和不带推理的 `gpt-4.1`，其回复写明快照 `gpt-5.5-2026-04-23` 和 `gpt-4.1-2025-04-14`。
- ID 遵循 `<prefix>_<16 hex>00<8 hex unix time><24 hex>`，前缀为 `resp`、`msg`、`fc`、`rs` 和 `ws`，同一回复中的条目共用其 16 位十六进制前缀。调用 ID 是 `call_` 加 24 个字母和数字。
- 手工构建的 `Response` 对象包含当前 SDK 的所有顶层字段，包括 `access_programs`。文档示例保留其较小的字段集，编解码器对两者都能往返。
- `input_tokens` 包含 `cached_tokens`，`output_tokens` 包含 `reasoning_tokens`。`input_tokens_details.cache_write_tokens` 始终存在。
- 在流中，`event:` 等于 `data.type`，`sequence_number` 从 0 开始递增。各 delta 拼接后等于其 `.done` 文本和最终回复，最后没有 `[DONE]`。
- 每个 `*.delta` 事件都带有 `obfuscation`，但请求设置了 `include_obfuscation: false` 的流和文档自带的流除外。

## OpenAI Embeddings

- `input` 覆盖其四种形式：字符串、字符串数组、Token ID 数组和 Token ID 数组的数组。
- 手工构建的请求设置 `dimensions: 8`，使向量保持简短。它们是随机单位向量，每个值都是经 float32 往返后不变的最短十进制数。
- `base64` 回复包含小端序 float32 值，`usage.total_tokens` 等于 `usage.prompt_tokens`。
