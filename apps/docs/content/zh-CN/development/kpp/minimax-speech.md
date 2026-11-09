---
title: MiniMax Speech
description: KPP 的语音请求如何映射到 MiniMax 的同步文本转语音 API。
order: 8
---

在这里查阅语音请求如何到达 MiniMax：哪个字段放在哪里、哪些内容会被拒绝，以及每项映射的来源。

- **编解码器：**`kaeso_codecs::minimax_speech::MinimaxSpeech`（`crates/kaeso-codecs/src/minimax_speech/`）
- **路径：**`POST /v1/t2a_v2`（T2A v2），从 MiniMax 主机的根路径开始，带 `Authorization: Bearer`
- **Fixture：**`crates/kaeso-codecs/fixtures/minimax_speech/`，依据 MiniMax 的 OpenAPI 文档构建，没有录制的

**来源。**MiniMax 的[同步文本转语音](https://platform.minimax.io/docs/api-reference/speech-t2a-http)参考、其 [OpenAPI 文档](https://platform.minimax.io/docs/api-reference/speech/t2a/api/openapi.json)和[错误码](https://platform.minimax.io/docs/api-reference/errorcode)，2026-11-08 查阅。

## 决定

- **完整回复。** MiniMax 的流格式没有文档，因此 Kaeso 总是发送 `stream: false` 并读取完整回复。对流式应用，回复成为一段音频和结束事件，附带用量和计费字符数。
- **十六进制音频。** Kaeso 请求 `output_format: hex`，这样音频在回复中返回，而不是作为链接。在 KPP 中十六进制转为 base64。
- **从回复读取字符数。** `extra_info.usage_characters` 是 MiniMax 计费的字符数，用于费用，成为 `SpeechResponse::characters`。
- **错误可能随 HTTP 200 返回。** 失败的 `base_resp` 无论状态码如何都是错误，按其代码映射，与 [MiniMax Images](minimax-images.md#错误) 相同。

## 请求

| KPP | MiniMax | 说明 |
| --- | --- | --- |
| `model` | `model` | |
| `input` | `text` | |
| `voice` | `voice_setting.voice_id` | 名称或 ID 按原样发送：MiniMax 的声音取决于模型。 |
| `speed` | `voice_setting.speed` | 整数保持为整数。 |
| `format` | `audio_setting.format`：`mp3`、`wav`、`flac`、`opus`，`Pcm16` 为 `pcm` | |
| `sample_rate` | `audio_setting.sample_rate` | 没有采样率的 `Pcm16` 使用 `24000`，因为 KPP 的 `pcm16` 为 24 kHz。 |
| `bitrate`（千比特） | `audio_setting.bitrate`（比特每秒） | |
| `stream` | `stream: false` | Kaeso 从不请求 MiniMax 流式传输。 |
| — | `output_format: hex` | 除非 MiniMax 格式的请求指定了其他值，否则发送此值。 |
| `voice_setting` 和 `audio_setting` 的其他键、`pronunciation_dict`、`voice_modify`、`language_boost` 和其他键 | extras | 只从 MiniMax 格式的请求发送；其他方言会拒绝它们。 |

## 拒绝项

以下每项都会在发送任何内容之前返回 `Unsupported`，并在 `param` 中写明字段：

| KPP | `param` | 原因 |
| --- | --- | --- |
| `Speech` 以外的操作：对话、音效、音乐、变声 | `model` | T2A 只从文本生成语音。 |
| 对话行或源音频 | `input` | |
| mp3、pcm、flac、wav、opus 以外的格式 | `response_format` | Kaeso 从不转换音频。 |
| `instructions`、`language`、`seed`、时间戳、`duration` | 该字段 | T2A v2 中没有 Kaeso 映射的此类字段。 |

## 回复

| MiniMax | KPP |
| --- | --- |
| `data.audio`，十六进制 | 作为 base64 的音频；无法解码的十六进制为 `ProviderError` |
| `extra_info.audio_format` | 音频的媒体类型（`audio/mpeg`、`audio/wav`、`audio/flac`、`audio/opus`、`audio/pcm`）；缺失时为 `mp3` |
| `extra_info.usage_characters` | `characters` |
| `data` 的其余部分、`extra_info`、`base_resp` | extras |

对 MiniMax 格式的应用，编解码器把音频以十六进制写入 `data.audio`，`base_resp` 为 `0`。

## Fixture

| 文件 | 涵盖内容 | 构建依据 |
| --- | --- | --- |
| `requests/speech.json` | 声音和音频设置、`pronunciation_dict`、`voice_modify`、`language_boost`、十六进制输出 | OpenAPI 文档中的示例 "Request(Non-streaming)" |
| `responses/speech.json` | 十六进制音频、带 `usage_characters` 的 `extra_info`、`base_resp` | 同一文档的回复示例；十六进制是十个虚构的 MP3 字节 |
| `errors/invalid-params-200.json` | HTTP 200 下失败的 `base_resp` | 模式 `T2aV2Resp.base_resp` 和错误码；消息是虚构的 |

没有可用的 MiniMax 密钥，因此没有录制内容。该文件夹的 `README.md` 列出了来源。
