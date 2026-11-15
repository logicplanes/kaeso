---
title: Deepgram
description: KPP 的转写和语音请求如何映射到 Deepgram 的语音转文本和 Aura 文本转语音 API。
order: 8
---

在这里查阅转写或语音请求如何到达 Deepgram：哪个字段放在哪里、哪些内容会被拒绝，以及每项映射的来源。

- **编解码器：**`kaeso_codecs::deepgram::Deepgram`（`crates/kaeso-codecs/src/deepgram/`）
- **路径：**`POST /v1/listen`（预录音频）和 `POST /v1/speak`（Aura），所有选项都在查询字符串中，带 `Authorization: Token`
- **Fixture：**`crates/kaeso-codecs/fixtures/deepgram/`；错误和模型列表为实时录制，回复取自 Deepgram 的文档

**来源。**Deepgram 关于[预录音频](https://developers.deepgram.com/docs/pre-recorded-audio)、[文本转语音](https://developers.deepgram.com/docs/text-to-speech)、[媒体输出设置](https://developers.deepgram.com/docs/tts-media-output-settings)、[编码](https://developers.deepgram.com/docs/tts-encoding)、[语言检测](https://developers.deepgram.com/docs/language-detection)、[话语](https://developers.deepgram.com/docs/utterances)和[错误](https://developers.deepgram.com/docs/errors)的页面，以及 `deepgram/deepgram-api-specs` 在提交 `0ccc6f4` 的 OpenAPI 文档，2026-11-14 查阅。

## 决定

- **应用未指定语言时检测语言。** OpenAI 会检测语言；Deepgram 默认英语。此时 Kaeso 发送 `detect_language=true`，它选定一种语言并按单语言价格计费，从不发送 `language=multi`。
- **始终启用智能格式化。** Kaeso 发送 `smart_format=true`，Deepgram 对此不收费，使转写像 OpenAI 的一样带标点和大小写。
- **每种转写形式都来自一个 JSON 回复。** Deepgram 只返回 JSON。对于 `srt`、`vtt`、`verbose_json` 和 `diarized_json`，Kaeso 请求 `utterances=true`，并根据话语写出该形式：每个话语一条字幕或一个片段，`diarized_json` 带上说话人。
- **声音即模型。** Deepgram 把每个 Aura 声音命名为一个模型，例如 `aura-2-thalia-en`。Kaeso 的模型是 `aura-2` 和 `aura` 两个系列，由 `voice` 指定声音：`thalia`（除非请求指定语言，否则为英语）、`thalia-en` 或完整 ID。
- **完整转写，流式音频。** 转写以完整回复返回；流式应用收到一段内容和结束事件。语音按 Deepgram 发送的方式逐段返回。
- **从响应头读取字符数。** `dg-char-count` 是 Deepgram 计费的字符数，用作请求的字符数来计算费用。转写的 `metadata.duration` 乘以声道数即计费时长。
- **保留 Deepgram 的默认设置。** Kaeso 不发送 `mip_opt_out`，因此适用模型改进计划（Model Improvement Program）的设置；也不发送 `callback`、`redact`、`keyterm` 或其他付费附加功能。

## 转写请求

| KPP | Deepgram | 说明 |
| --- | --- | --- |
| `model` | `?model=` | |
| `audio`，上传 | 请求体，以文件的媒体类型作为 `content-type` | 上传未指定时为 `application/octet-stream`；Deepgram 从字节中识别格式。 |
| `audio`，链接 | JSON 形式的 `{"url": …}` | |
| `language` | `?language=` | 未指定时：`?detect_language=true`。 |
| 需要时间信息的 `format`、`timestamps: [segment]` | `?utterances=true` | 单词始终带时间。 |
| `diarize` 或 `format: diarized_json` | `?diarize_model=latest` | `diarize`（不带 `_model`）已弃用。 |
| — | `?smart_format=true` | |
| Deepgram 格式请求的查询参数 | 相同的参数，按原顺序 | 其他方言会拒绝它们。 |

## 语音请求

| KPP | Deepgram | 说明 |
| --- | --- | --- |
| `model` 和 `voice` | `?model=` | `aura-2` 加 `thalia` 即 `aura-2-thalia-en`；已包含声音的模型按原样发送。 |
| `input` | `{"text": …}` | 最多 2,000 个字符。 |
| `format` | `?encoding=`：`mp3`、`opus`、`aac`、`flac`；`wav` 为 `linear16` 加 `container=wav`；`pcm16` 为 `linear16` 加 `container=none` 和 `sample_rate=24000` | Deepgram 的 `linear16` 是 16 位小端 PCM。未指定格式时为 `mp3`。 |
| `sample_rate` | `?sample_rate=` | |
| `bitrate`（千比特） | `?bit_rate=`（比特每秒） | |
| `speed` | `?speed=` | 0.7 到 1.5。 |

## 拒绝项

以下每项都会在发送任何内容之前返回 `Unsupported` 或无效请求，并在 `param` 中写明字段：

| KPP | `param` | 原因 |
| --- | --- | --- |
| 翻译 | `model` | Deepgram 按所说的语言转写。 |
| `prompt`、`temperature`、`num_speakers` | 该字段 | Deepgram 没有对应项。 |
| 文件 ID | `file` | |
| `Speech` 以外的语音操作、对话或源音频 | `model`、`input` | |
| `instructions`、`seed`、时间戳、`duration` | 该字段 | |
| mp3、opus、aac、flac、wav、pcm 以外的格式 | `response_format` | |
| 超出 0.7 到 1.5 的 `speed` | `speed` | |
| 超过 2,000 个字符的文本 | `input` | 超过时 Deepgram 返回 413。 |
| `aura-2` 或 `aura` 未指定声音、属于其他系列的声音、包含小写字母、数字和连字符以外字符的声音 | `voice` | |

## 转写回复

| Deepgram | KPP |
| --- | --- |
| `results.channels[0].alternatives[0].transcript` | `text` |
| 其 `words[]`：`punctuated_word` 或 `word`、`start`、`end`、`speaker` | `words` |
| `results.utterances[]`：`transcript`、`start`、`end`、`speaker`、`id` | `segments` |
| `results.channels[0].detected_language` | `language` |
| `metadata.duration` | `duration`；乘以 `metadata.channels` 得到计费时长 |
| `metadata`、声道、候选结果、`results` 和话语的其余部分 | extras |

包含多个声道或候选结果的回复为 `ProviderError`：Kaeso 不会请求这两者。

## 语音回复

音频本身，带其 `content-type`；没有时为 `audio/mpeg`。`dg-char-count` 是计费字符数。

## 错误

| Deepgram | KPP |
| --- | --- |
| `INVALID_AUTH`，401 | `Authentication` |
| `INSUFFICIENT_PERMISSIONS`，403 | `PermissionDenied` |
| `ASR_PAYMENT_REQUIRED`，402 | `Billing` |
| `Payload Too Large`、`PAYLOAD_TOO_LARGE`，413 | `RequestTooLarge` |
| `TOO_MANY_REQUESTS`、`Too Many Requests`，429 | `RateLimited` |
| `Bad Request` 和其他 400、415、422 | `InvalidRequest` |
| 404 | `NotFound` |
| 503 | `Overloaded` |
| 504 | `Timeout` |

消息取自 `err_msg`，或 `message` 加 `details`，或原样的纯文本请求体。Deepgram 自己的错误按原样返回给 Deepgram 格式的应用。

## Fixture

该文件夹的 `README.md` 列出每个文件、其来源，以及 Deepgram 与 OpenAI 音频 API 的不同之处。没有录制成功的回复：没有可用的 Deepgram 密钥。
