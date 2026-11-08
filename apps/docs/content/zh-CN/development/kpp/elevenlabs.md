---
title: ElevenLabs
description: ElevenLabs 的 API 如何映射到 KPP，适用于 ElevenLabs 格式的应用以及作为服务商的 ElevenLabs。
order: 9
---

查阅 ElevenLabs 的生成、转写和列表端点如何映射到 KPP，即 Kaeso 如何从 ElevenLabs 格式的应用读取它们、如何写给 ElevenLabs。修改 ElevenLabs 编解码器之前，先查看这里。

- **编解码器：**`kaeso_codecs::elevenlabs::ElevenLabs`（`crates/kaeso-codecs/src/elevenlabs/`）
- **Fixture：**`crates/kaeso-codecs/fixtures/elevenlabs/`，在免费套餐上实时录制，每个端点一次，共约 40 个额度

**来源。**ElevenLabs 的 OpenAPI 文档 [api.elevenlabs.io/openapi.json](https://api.elevenlabs.io/openapi.json)（2026-11-08 查阅）、其[错误](https://elevenlabs.io/docs/eleven-api/resources/errors)和[费用响应头](https://elevenlabs.io/docs/api-reference/introduction)，以及官方 Python SDK 的路径（每个操作的 `x-fern-sdk-method-name`）。

## 决定

- **路径指明调用。**`ElevenLabsCall::from_path` 读取操作、声音 ID 以及 `/stream` 和 `/with-timestamps` 形式；应用的请求用 `dispatch::decode_elevenlabs_request` 解码。生成变为 `Request::Speech`（`Speech`、`Dialogue`、`SoundEffect`、`Music`、`VoiceChange`），语音转文本变为 `Request::Transcription`。
- **声音 ID，从不猜测。**ElevenLabs 按 ID 接收声音。`Voice::Id`，或者像 ElevenLabs 的 ID 那样由二十个字母和数字组成的名称，作为 ID 发送；其他任何名称，例如 OpenAI 的 `alloy`，都是 `Unsupported`，并提示使用 `GET /v1/voices`。
- **音频边生成边传输。**对于其他格式的应用，Kaeso 调用 `/stream` 路径，它边生成边发送音频；WAV 例外，只有非流式路径接受它。`/with-timestamps` 返回 JSON，整体读取（`EgressCtx::speech_reply_json`）；`/stream/with-timestamps` 每个片段返回一行 JSON，作为带对齐信息的 `SpeechEvent` 读取。
- **字符数来自响应头。**ElevenLabs 按字符对语音计费，并在 `character-cost` 响应头中给出字符数；`elevenlabs::billed_characters` 读取它来计算费用。
- **默认模型。**没有 `model_id` 的请求会得到 ElevenLabs 为该操作记录的默认模型（`eleven_multilingual_v2`，对话为 `eleven_v3`，`eleven_text_to_sound_v2`、`music_v1`、`eleven_english_sts_v2`），以便 Kaeso 解析它；`default_model` 标记让它不出现在发给 ElevenLabs 的请求中。
- **查询参数只传给 ElevenLabs。**`output_format` 是有类型的；`enable_logging` 和 `optimize_streaming_latency` 是 extras，因此其他服务商会拒绝它们，而不是丢失零保留设置。

## 请求

| ElevenLabs | KPP | 类别 |
| --- | --- | --- |
| 路径 `voice_id` | `voice: Voice::Id` | typed |
| 查询 `output_format`，例如 `mp3_44100_128`、`pcm_24000`、`opus_48000_64`、`wav_16000`、`ulaw_8000` | `format`、`sample_rate`、`bitrate`；`pcm_24000` 为 `Pcm16`；保留原样的值 | typed、form |
| 其他查询参数 | `query` 字段 | extras |
| `text`（语音、音效）、`prompt`（音乐） | `input` | typed |
| `inputs[]` `{text, voice_id}` | `dialogue` 的各句 | typed |
| `model_id` | `model` | typed |
| `language_code` | `language` | typed |
| `seed` | `seed` | typed |
| `voice_settings.speed` | `speed` | typed |
| `duration_seconds`、`music_length_ms` | 以秒计的 `duration` | typed |
| `/with-timestamps` | `timestamps` | typed |
| `/stream` | `stream_path` 标记；带时间戳时为 `stream: On` | form、typed |
| `voice_settings` 的其余部分、`pronunciation_dictionary_locators`、`previous_text`、`apply_text_normalization`、`audio_effects`…… | 该字段 | extras |
| 变声上传的 `audio`、`seed`；其他字段 | `audio`、`seed`；其余为 extras | typed、extras |
| 语音转文本上传的 `file`，或 `source_url` / `cloud_storage_url` | `audio` | typed |
| `language_code`、`diarize`、`num_speakers`、`temperature` | `language`、`diarize`、`speakers`、`temperature` | typed |
| `timestamps_granularity` 为 `word`（默认）或 `none` | `format` 为 `verbose_json` 或 `json`；`word` 还设置 `timestamps`；`character` 为 `timestamps_granularity` 标记 | typed、form |
| `tag_audio_events`、`additional_formats`、`keyterms`、`entity_detection`、`webhook`…… | 该字段 | extras |

来自其他格式时，`mp3`、`opus`、`wav`、`pcm16`、μ-law 和 A-law 变为 ElevenLabs 列出的某个 `output_format`；请求未指定速率时使用免费套餐的速率：MP3 为 44.1 kHz、128 kbps，Opus 为 48 kHz、64 kbps，WAV 和 PCM 为 24 kHz。`aac`、`flac` 以及 ElevenLabs 未列出的速率为 `Unsupported`。`speed` 传给 `voice_settings.speed`；`instructions` 为 `Unsupported`。

## 回复

| ElevenLabs | KPP |
| --- | --- |
| 带 `content-type` 和 `character-cost` 的音频字节 | `SpeechResponse::audio`、`characters`；片段为 `SpeechEvent` |
| `audio_base64`、`alignment {characters, character_start_times_seconds, character_end_times_seconds}`、`normalized_alignment` | `audio`、`alignment: SpeechAlignment`；`normalized_alignment` 为 extras |
| `/stream/with-timestamps` 的一行 | 带对齐信息的 `SpeechEvent` |
| `text`、`language_code`、`words[]` `{text, start, end, type, speaker_id, logprob}`、`audio_duration_secs` | `text`、`language`、带 `speaker` 的 `words`（`type`、`logprob` 和 `characters` 为 extras）、`duration` 和 `usage_seconds`；其余为 extras |
| 每个声道一份转写稿（`transcripts`） | 拒绝：一份转写稿容纳不了多个声道 |

OpenAI 应用收到的 ElevenLabs 词不含其 `spacing` 和 `audio_event` 条目，因为它们不是词。ElevenLabs 应用收到其他服务商的转写稿时，`language_probability` 为 1，每个词为 `type: word`。

## 错误

ElevenLabs 返回 `{"detail": {"type", "code", "message", "status", "request_id", "param"}}`；较早的端点返回 `{"detail": {"status", "message"}}`，校验错误返回 `{"detail": [{"loc", "msg"}]}`。Kaeso 读取这三种结构，把 `type` 和 `code` 映射为错误类型（`insufficient_credits` 和 `payment_required` 映射为计费，`rate_limit_error` 映射为速率限制），并以文档规定的结构向 ElevenLabs 应用写出自己的错误。ElevenLabs 自己的错误连同其状态码按原样传递。

## 列表

`GET /v1/models` 以 ElevenLabs 的 `ModelResponseModel` 结构（`model_id` 和 `name`）列出项目的 ElevenLabs 模型。声音列表是该账户在 ElevenLabs 上自己的声音：`voice_listing_request` 用应用的查询构建 `GET /v1/voices`、`/v2/voices` 或 `/v1/voices/{voice_id}`，`voices_reply` 在传递之前检查回复是否为声音列表或单个声音。
