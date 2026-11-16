---
title: AssemblyAI
description: KPP 的转写请求如何映射到 AssemblyAI 的 Sync API。
order: 8
---

在这里查看转写请求如何到达 AssemblyAI：各字段的去向、Kaeso 拒绝的内容、Sync API 的限制，以及每项映射的来源。

- **编解码器：** `kaeso_codecs::assemblyai::AssemblyAi`（`crates/kaeso-codecs/src/assemblyai/`）
- **路径：** `https://sync.assemblyai.com`（或 `https://sync.eu.assemblyai.com`）上的 `POST /v1/transcribe`，密钥以 `Authorization: Bearer` 发送，模型放在 `X-AAI-Model` 中
- **Fixtures：** `crates/kaeso-codecs/fixtures/assemblyai/`，OpenAPI 文档中的示例和根据其 schema 构建的回复，并包含实际记录的被拒绝密钥和缺少模型的请求

**来源。** AssemblyAI 的 [Sync API 参考](https://www.assemblyai.com/docs/api-reference/sync-api/transcribe)及其 OpenAPI 文档，2026-11-14 阅读。

## 限制

Sync API 在同一请求中返回结果。它接受 80 毫秒到 20 分钟、最大 256 MB 的音频，并在 30 秒内返回，否则返回 504 `Inference Timeout`，应用收到超时错误。更长的录音需要 AssemblyAI 的异步 API，Kaeso 不调用该 API。

## 请求

| KPP | Sync API |
| --- | --- |
| `file` | `audio` 部分，带上传文件的内容类型 |
| `model` | `X-AAI-Model` 请求头（`universal-3-6-pro`） |
| `language` | `config.language_codes`，取其主子标签，限 API 可引导的 32 种语言；其他语言会被拒绝，不提供语言时 API 不做引导直接转写 |
| `prompt` | `config.prompt`，最多 6000 个字符；AssemblyAI 将其加入模型的指令 |
| `verbose_json`、`srt`、`vtt`、词级时间戳 | `config.timestamps: true`；Kaeso 由带时间的词生成片段和字幕 |

`config` 部分为空时不发送。Kaeso 在发送前拒绝 `temperature`、片段时间戳、说话人标签（`diarized_json`）、翻译、流式和通过链接提供的音频：Sync API 不支持这些。

## 结果

| Sync API | KPP |
| --- | --- |
| `text` | 文本 |
| `audio_duration_ms` | 音频时长，按时长计费 |
| `words[]` `{text, start, end}`，单位为毫秒 | 带时间的词；API 无法对齐的词不带时间，此时这些词按 AssemblyAI 的原样保留 |
| `words[].confidence`、`confidence`、`session_id`、`request_time_ms` | extras |

Sync API 不返回语言，也不返回片段。

## 错误

| Sync API | KPP |
| --- | --- |
| `{"status", "title", "detail"}`（problem details） | 以 `detail` 为消息，按状态码确定类型；`Bad Audio`、`Audio Too Short`、`Audio Too Large` 和 `Unsupported Media Type` 指向 `file` |
| 404 `Invalid API key` | 认证错误：API 对被拒绝的密钥返回 404，而不是文档所写的 401 |
| 纯文本，例如缺少 `X-AAI-Model` 时的 `Not found` | 文本，按状态码确定类型 |

## 模型检查

Sync API 不列出模型，也没有不经转写就能检查密钥的请求，因此**检查可用性**会回复无法检查；第一次转写会显示密钥是否有效。
