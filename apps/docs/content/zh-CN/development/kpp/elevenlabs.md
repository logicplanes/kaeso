---
title: ElevenLabs
description: ElevenLabs 的 API 如何映射到 KPP，适用于 ElevenLabs 格式的应用以及作为服务商的 ElevenLabs。
order: 9
---

查阅 ElevenLabs 的生成、转写、视频和列表端点如何映射到 KPP，即 Kaeso 如何从 ElevenLabs 格式的应用读取它们、如何写给 ElevenLabs。修改 ElevenLabs 编解码器之前，先查看这里。

- **编解码器：**`kaeso_codecs::elevenlabs::ElevenLabs`（`crates/kaeso-codecs/src/elevenlabs/`）
- **Fixture：**`crates/kaeso-codecs/fixtures/elevenlabs/`，实时录制，每个端点一次

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
| 路径 `voice_id` | `voice: Voice::Id`；不全是字母和数字的 ID 在查找凭据之前即为 `InvalidRequest`，`param` 为 `voice_id` | typed |
| 查询 `output_format`，例如 `mp3_44100_128`、`pcm_24000`、`opus_48000_64`、`wav_16000`、`ulaw_8000` | `format`、`sample_rate`、`bitrate`；`pcm_24000` 为 `Pcm16`；保留原样的值 | typed、form |
| 其他查询参数 | `query` 字段 | extras |
| `text`（语音、音效）、`prompt`（音乐） | `input` | typed |
| `inputs[]` `{text, voice_id}` | `dialogue` 的各句；格式不对的 `voice_id` 为 `InvalidRequest`，`param` 为 `inputs[i].voice_id` | typed |
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

来自其他格式时，`mp3`、`opus`、`wav`、`pcm16`、μ-law 和 A-law 变为 ElevenLabs 列出的某个 `output_format`；请求未指定速率时使用所有套餐都提供的速率：MP3 为 44.1 kHz、128 kbps，Opus 为 48 kHz、64 kbps，WAV 和 PCM 为 24 kHz。`aac`、`flac` 以及 ElevenLabs 未列出的速率为 `Unsupported`。`speed` 传给 `voice_settings.speed`；`instructions` 为 `Unsupported`。

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

## 图片生成

ElevenLabs 通过 `POST /v1/flows/image` 使用 OpenAI、Google 和字节跳动的模型生成图片，由 `GET /v1/flows/image/{id}` 报告；请求体取决于 `model_id`（OpenAPI 文档中的 `ImageGenerationRequest`，2026-11-09 阅读，以及[图片和视频指南](https://elevenlabs.io/docs/eleven-api/guides/cookbooks/image-and-video)）。Kaeso 把 OpenAI Images API 请求发往那里，然后按 ElevenLabs 的要求，至少间隔两秒查询生成状态，直到结束，全部在同一个请求内完成；`dispatch::pending_task`、`encode_task_poll` 和 `task_poll_gap` 对应这些步骤。ElevenLabs 格式的应用不通过 Kaeso 调用图片生成：其 SDK 的 `flows.image` 会保存生成 ID，Kaeso 需要像处理视频那样把它们映射到项目。`fixtures/elevenlabs/image/` 中的 fixture 依据 OpenAPI 文档构建；图片生成未实际调用。

| KPP | ElevenLabs |
| --- | --- |
| `model` | `model_id`：`gpt-image-2.5-sunburst`、`gpt-image-2.5-flare`、`gpt-image-2`、`gpt-image-1.5`、`gpt-image-1`、`gemini-3.1-flash-image`、`gemini-3.1-flash-lite-image`、`gemini-3-pro-image`、`gemini-2.5-flash-image`、`bytedance-seedream-5-pro`、`bytedance-seedream-5-lite` |
| `prompt` | `prompt` |
| `images`、`mask` | `images[]`、`mask`：`{type: "inline_base64", content_base64, mime_type}` 或 `{type: "asset", asset_id}`，最多为模型的 5、10 或 14 张；仅 GPT Image 支持蒙版 |
| `quality` | 来自模型列表的 `quality`（`low`、`medium`、`high`，GPT Image 2.5 另有 `xhigh` 和 `max`）；`auto` 不发送 |
| `background` | `background`，仅 GPT Image 1 和 1.5 |
| `size` | `aspect_ratio`：`1024x1024` 为 `1:1`，`1536x1024` 为 `3:2`，`1024x1536` 为 `2:3`，`auto` 为 `auto`；其他尺寸被拒绝 |
| `aspect_ratio`、`resolution` | `aspect_ratio`、`resolution`，均取自模型的列表 |
| `seed` | `seed`，仅 Seedream |
| 回复 `status` 为 `pending` 或 `generating` | 生成尚未完成；Kaeso 再次查询 |
| 回复 `content_url`、`content_mime_type` | 图片链接，约一小时内有效；应用要求 `b64_json` 时由 Kaeso 下载 |
| 回复 `failure_reason`、`error_message` | 错误：`moderated` 为内容过滤，`invalid_parameters` 为无效请求，`timeout`，`charging_failed` 为计费，其他为服务商错误 |

`n` 大于 1、图片链接、`output_format`、`output_compression`、`negative_prompt`、部分图片和变体会在发送前被拒绝。ElevenLabs 以积分计费，KPMR 未列出，因此其费用保持未知。

## 视频生成

ElevenLabs 通过 `POST /v1/flows/video` 使用 Veo 3.1 和 Seedance 2 模型生成视频，由 `GET /v1/flows/video/{id}` 报告；请求体取决于 `model_id`（`VideoGenerationRequest`）。Kaeso 为其他格式的应用把视频任务发往那里；ElevenLabs 格式的应用不通过 Kaeso 调用它。`fixtures/elevenlabs/video/` 中的 fixture 依据 OpenAPI 文档构建。

| KPP | ElevenLabs |
| --- | --- |
| `model` | `model_id`：`veo-3.1-generate-001`、`veo-3.1-fast-generate-001`、`bytedance-seedance-v2`、`-v2-fast`、`-v2-mini`、`-v2.5` |
| `prompt`、`seed`、`audio` | `prompt`、`seed`、`generate_audio` |
| `negative_prompt` | `negative_prompt`，仅限 Veo 模型 |
| `seconds`、`aspect_ratio`、`resolution` | `duration_secs`、`aspect_ratio`、`resolution`，各自取自该模型的列表 |
| `FirstFrame`、`LastFrame` 图片 | `start_frame`、`end_frame`：`{type: "inline_base64", content_base64, mime_type}` 或 `{type: "asset", asset_id}` |
| `Reference` 图片 | `images[]` `{image, role}`，仅限 Veo 模型，`role` 为 `subject`，除非应用指定了 `style` |
| 回复 `status` `pending`、`generating`、`completed`、`failed` | `queued`、`in_progress`、`completed`、`failed` |
| 回复 `content_url`、`content_mime_type` | 输出链接及其媒体类型，有效期约一小时 |
| 回复 `failure_reason`、`error_message` | `error.code`、`error.message` |

网页链接形式的图片、`creatify-aurora`（它需要音频输入）和 remix 会在发送前被拒绝。

## 列表

`GET /v1/models` 以 ElevenLabs 的 `ModelResponseModel` 结构（`model_id` 和 `name`）列出项目的 ElevenLabs 模型。声音列表是该账户在 ElevenLabs 上自己的声音：`voice_listing_request` 用应用的查询构建 `GET /v1/voices`、`/v2/voices` 或 `/v1/voices/{voice_id}`，`voices_reply` 在传递之前检查回复是否为声音列表或单个声音。

## 实时 Socket

`elevenlabs::realtime` 在不做 I/O 的情况下读取 ElevenLabs 的实时 WebSocket，供网关的会话代理使用。这些消息不做翻译：ElevenLabs 的 Socket 只到达 ElevenLabs，因此中间没有 KPP 类型。

| 函数 | 作用 |
| --- | --- |
| `RealtimeApi::from_path`、`path` | 识别 `/v1/text-to-speech/{voice_id}/stream-input` 和 `/multi-stream-input`、`/v1/text-to-dialogue/stream-input` 和 `/multi-stream-input` 以及 `/v1/speech-to-text/realtime` 的 Socket，并写出它在 ElevenLabs 上的路径，声音作为一个经过检查的路径段 |
| `read_client_message` | 从应用的消息中取出 `xi_api_key`、`authorization` 和 `single_use_token`，并统计消息携带的内容：`text` 或每个 `inputs[].text` 的字符数，或 `audio_base_64` 的字节数及其 `sample_rate` |
| `bytes_per_second` | 一秒 `pcm_*`（16 位）或 `ulaw_8000` 音频占用的字节数 |
| `error_message` | 该 Socket 自己的错误消息：语音转文本为 `{"message_type", "error"}`，其他为 `{"message", "error", "code", "param"}`，以及关闭代码：应用可以修正的问题为 1008，其他为 1011 |

来源：ElevenLabs 的 [WebSocket 参考](https://elevenlabs.io/docs/websockets)和[文本转对话 WebSocket](https://elevenlabs.io/docs/api-reference/text-to-dialogue/ttd-websocket)（2026-11-09 阅读），以及官方 `elevenlabs` Python SDK 2.77.0（`realtime_tts.py`、`realtime/`）。
