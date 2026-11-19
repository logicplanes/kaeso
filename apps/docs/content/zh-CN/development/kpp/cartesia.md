---
title: Cartesia
description: KPP 的语音合成和转写请求如何映射到 Cartesia 的 API。
order: 8
---

在这里查看语音合成或转写请求如何到达 Cartesia：各字段的去向、Kaeso 拒绝的内容，以及每项映射的来源。

- **编解码器：** `kaeso_codecs::cartesia::Cartesia`（`crates/kaeso-codecs/src/cartesia/`）
- **路径：** 使用 `X-API-Key` 和 `Cartesia-Version: 2026-08-14`：`POST /tts/bytes`（JSON 输入，音频输出），`POST /stt`（上传输入，JSON 输出），模型检查使用 `GET /voices?limit=1`
- **Fixtures：** `crates/kaeso-codecs/fixtures/cartesia/`，根据参考文档构建，并包含实际记录的被拒绝密钥和缺少版本头的请求

**来源。** Cartesia 的 [API 约定](https://docs.cartesia.ai/use-the-api/api-conventions)、[TTS bytes](https://docs.cartesia.ai/api-reference/tts/bytes) 和 [STT](https://docs.cartesia.ai/api-reference/stt/transcribe) 参考、[输出格式](https://docs.cartesia.ai/build-with-cartesia/capability-guides/tts-output-audio-format)、[错误](https://docs.cartesia.ai/use-the-api/api-errors)以及[从 Whisper 迁移指南](https://www.cartesia.ai/migrate/openai-whisper)，2026-11-14 阅读。

## 版本

每个请求都带 `Cartesia-Version: 2026-08-14`，即这些映射所依据、Cartesia 称为最新的版本。Cartesia 保持同一版本的字段稳定；新版本意味着 Kaeso 发布新版本并修改常量 `kaeso_codecs::cartesia::VERSION`。

## 语音合成请求

| KPP | Cartesia `/tts/bytes` |
| --- | --- |
| `model` | `model_id` |
| `input` | `transcript` |
| `voice` | `voice`：以字符串形式的声音 ID |
| `response_format` `mp3` | `output_format` `{"container": "mp3", "sample_rate": 44100, "bit_rate": 128000}` |
| `response_format` `wav` | `{"container": "wav", "encoding": "pcm_s16le", "sample_rate": 24000}` |
| `response_format` `pcm` | `{"container": "raw", "encoding": "pcm_s16le", "sample_rate": 24000}`，即 OpenAI 的 24 kHz 16 位 PCM |
| `sample_rate`、`bitrate` | `sample_rate`（8000 到 48000 中列出的值）、mp3 的 `bit_rate` |
| `speed` | `generation_config.speed`，0.6 到 1.5 |
| `language` | `language` |

Kaeso 在发送前拒绝 Cartesia 不生成的 `opus`、`aac` 和 `flac`，以及 `instructions`、`seed`、时间戳和没有声音的请求。音频在 Cartesia 生成时流式返回。Cartesia 不认识的声音会返回 `voice_not_found`，应用收到针对 `voice` 的未找到错误。

## 转写请求

| KPP | Cartesia `/stt` |
| --- | --- |
| `file` | `file` |
| `model` | `model` |
| `language` | `language`，必填：批量 API 默认英语且不检测语言 |
| `verbose_json`、`srt`、`vtt`、词级时间戳 | `timestamp_granularities[]=word`；Kaeso 由这些词生成片段和字幕 |

Kaeso 在发送前拒绝 `prompt`、`temperature`、片段时间戳、说话人标签（`diarized_json`）、翻译、流式和通过链接提供的音频。

## 转写结果

| Cartesia | KPP |
| --- | --- |
| `text` | 文本 |
| `language`、`duration` | 语言和音频时长，按时长计费 |
| `words[]` `{word, start, end}` | 带时间的词，保留开头的空格 |
| `type`、`request_id` | extras |

## 错误

| Cartesia | KPP |
| --- | --- |
| `{"error_code", "title", "message", "request_id"}` | 取 `message`；`voice_not_found` 为针对 `voice` 的未找到，`model_not_found` 和 `model_sunsetted` 为针对 `model` 的未找到，`quota_exceeded` 为计费错误，`concurrency_limited` 为速率限制，`file_too_large` 为请求过大，其余按状态码确定类型 |
| 纯文本，例如缺少 `Cartesia-Version` 时 | 文本，按状态码确定类型 |

## 模型检查

Cartesia 不列出模型，因此**检查可用性**读取 `GET /voices?limit=1`，只能说明密钥有效。
