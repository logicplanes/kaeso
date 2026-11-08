---
title: OpenAI Transcriptions
description: OpenAI 转写和翻译 API 如何映射到 KPP，以及其他服务商的转写格式如何接收它。
order: 8
---

在这里查阅 OpenAI 转写和翻译 API 的任意字段：它在 KPP 中的位置、哪些服务商以何种形式接收它，以及每项映射的来源。修改转写编解码器读写某个字段的方式之前，先查看这里。

- **编解码器：**`kaeso_codecs::openai_transcriptions::OpenAiTranscriptions`（`crates/kaeso-codecs/src/openai_transcriptions/`）、Gemini 编解码器中的转写（`gemini/audio.rs`），以及用于 QwenCloud 的 Chat Completions 编解码器中的转写（`openai_chat/transcription.rs`）
- **端点：**`POST /v1/audio/transcriptions` 和 `POST /v1/audio/translations`，上传为 `multipart/form-data`
- **Fixture：**`crates/kaeso-codecs/fixtures/openai_transcriptions/`，依据规范和服务商文档构建；`fixtures/gemini/` 包含一份录制的 Gemini 转写稿

**来源。**OpenAI OpenAPI 规范，提交 `926a7b8133226630bac025350e84b0ca5c31d45a`（2026-11-07）：`CreateTranscriptionRequest`、`CreateTranslationRequest`、`AudioResponseFormat`、`CreateTranscriptionResponseJson`、`CreateTranscriptionResponseVerboseJson`、`CreateTranscriptionResponseDiarizedJson`、`CreateTranslationResponseJson`、`CreateTranslationResponseVerboseJson`、`TranscriptionSegment`、`TranscriptionWord`、`TranscriptTextUsageTokens`、`TranscriptTextUsageDuration`、`TranscriptTextDeltaEvent`、`TranscriptTextSegmentEvent`、`TranscriptTextDoneEvent`。multipart 字段名依据 openai-python 3.41.0。RFC 7578。

## 决定

- **两个端点共用一个 KPP 请求。**`Request::Transcription(TranscriptionRequest)`，`TranscriptionOperation` 为 `Transcribe` 或 `Translate`；路径指明操作，因此应用的上传用 `dispatch::decode_transcription_request` 解码。`Reply::Transcription(TranscriptionResponse)` 包含文本、文本到达时的形式（`TranscriptFormat`）、语言、时长、带说话人的带时间片段和词、用量以及计费秒数。`StreamEvent::Transcript(TranscriptEvent)` 携带增量、完成的片段和结束事件。
- **由时间信息生成字幕。**服务商不支持应用要求的形式时，Kaeso 向它请求带片段时间戳的 `verbose_json`（或服务商带时间的词），自己写出 `srt`、`vtt` 或片段（`openai_transcriptions::subtitles`）：SubRip 使用编号的字幕条和 `HH:MM:SS,mmm` 时间，WebVTT 使用 `WEBVTT` 和 `HH:MM:SS.mmm`。只有词没有片段的回复会变成一条从第一个词到最后一个词的字幕。任何时间信息都无法生成的形式，例如没有说话人的 `diarized_json`，会在发送任何内容之前得到 `Unsupported`。
- **不做转换。**Kaeso 不解码也不转换音频；上传内容按原样发送。

## 请求

| `CreateTranscriptionRequest` 字段 | KPP | 类别 |
| --- | --- | --- |
| `file` | `audio: AudioInput`，包含上传的字节、媒体类型和文件名 | typed |
| `model` | `model` | typed |
| `language` | `language` | typed |
| `prompt` | `prompt` | typed |
| `response_format` | `format: TranscriptFormat` | typed |
| `temperature` | `temperature`；整数保持为整数 | typed、form |
| `timestamp_granularities[]` | `timestamps`；保留字段带或不带 `[]` 的写法 | typed、form |
| `stream` | `stream`；明确的 `false` 会保留 | typed、form |
| `diarize`、`url` 或 `file_url` 中的链接（其他服务商的形式） | `diarize`、作为链接的 `audio` | typed、form |
| `include[]`、`chunking_strategy`、`known_speaker_names[]`、`known_speaker_references[]`、`languages[]`、`keywords[]` 以及其他任何字段 | 该字段，列表为数组 | extras |

## 回复和流

| 线上格式 | KPP |
| --- | --- |
| `json`：`text`、`usage`、`logprobs`、`languages` | `text`、`usage`；其余为 extras |
| `verbose_json`：`language`、`duration`、`segments`、`words`、`usage` | `language`、`duration`、`segments`（`id`、`start`、`end`、`text`；`seek`、`tokens` 和各项分数为 extras；数字 `id` 以 `numeric_id` 标记）、`words` |
| `diarized_json`：带 `speaker` 的 `segments` | 带 `speaker` 的 `segments` |
| `text`、`srt`、`vtt` | 原样的 `text`，带其 `format` |
| 用量 `{"type": "tokens", …}` | `Usage`，音频部分计为 `input_audio_tokens` |
| 用量 `{"type": "duration", "seconds"}` | `usage_seconds` |
| `transcript.text.delta` `delta`、`segment_id`、`logprobs` | `TranscriptEvent::delta`、`segment_id`；`logprobs` 为 extras |
| `transcript.text.segment` | `TranscriptEvent::segment` |
| `transcript.text.done` | 带完整转写稿的 `TranscriptEvent::done` |

Kaeso 用 `Codec::response_media_type` 以应用要求的形式写出转写稿：`application/json`，`text`、`srt` 和 `vtt` 为 `text/plain; charset=utf-8`，或事件。其他服务商的形式添加的字段，例如 Groq 的 `x_groq` 或 Mistral 的 `finish_reason`，属于该服务商，不会出现在发给 OpenAI 应用的回复中；服务商的用量结构也一样，Kaeso 以 OpenAI 的形式写出用量。

## 服务商及其形式

`TranscriptionsProfile` 描述每个使用该 API 某种形式的服务商：路径、请求体（`TranscriptionBody`）、形式、是否接受 `response_format`、在哪里接收语言（`LanguageField`）、如何接收时间设置（`TimestampsField`）、链接、流，以及 OpenAI 字段之外的字段。

| Profile | 形式 | 来源 |
| --- | --- | --- |
| `OPENAI` | 规范 | 上述规范 |
| `GROQ` | `json`、`text`、`verbose_json`；链接放在 `url` 中；支持翻译；不支持流 | [speech to text](https://console.groq.com/docs/speech-to-text) |
| `TOGETHER` | `json`、`verbose_json`；`timestamp_granularities`；链接放在 `file` 中；`diarize`；支持翻译 | [transcription](https://docs.together.ai/docs/inference/transcription/overview) |
| `MISTRAL` | 一种 JSON 形式，发送 `timestamp_granularities=segment` 时带 `segments`，没有词级时间；`file_url`、`file_id`；`diarize`；`transcription.*` 事件；没有 `prompt`，不支持翻译 | [transcribe files](https://docs.mistral.ai/studio/audio/speech-to-text/transcribe-files) |
| `ZAI` | 位于主机根路径下的 `/api/paas/v4/audio/transcriptions`，`Authorization: Bearer`；JSON 文本；没有 `language`，没有 `temperature`；结束事件可能缺少文本，Kaeso 用增量补全 | [GLM-ASR](https://docs.z.ai/guides/audio/glm-asr-2512) |
| `XAI` | `/v1/stt`；每个回复都有带时间的词（`text`、编号的 `speaker`）；链接放在 `url` 中；`diarize`；文件放在最后 | [voice](https://docs.x.ai/developers/rest-api-reference/inference/voice) |
| `OPENROUTER` | `json`、`verbose_json`；链接放在 `source_url` 中；用量带 `seconds` | [STT](https://openrouter.ai/docs/guides/overview/multimodal/stt) |
| `COHERE` | 位于主机根路径下的 `/v2/audio/transcriptions`；必须提供 `language`；只返回文本 | [transcribe](https://docs.cohere.com/v2/docs/transcribe) |
| `MINIMAX` | 位于主机根路径下的 `/v1/speech_to_text`，`Authorization: Bearer`；`json`、`verbose_json`、`srt`、`vtt`；语言放在 `language` 请求头中；`timestamp_level` 为 `sentence` 或 `word`；`{index, delta, finish, duration}` 事件 | [speech to text](https://platform.minimax.io/docs/api-reference/speech-to-text) |
| `VERCEL` | `/v4/ai/transcription-model`；带 base64 `audio` 和 `mediaType` 的 JSON，模型放在 `ai-model-id` 中；回复带 `durationInSeconds` | [speech to text](https://vercel.com/docs/ai-gateway/modalities/speech-to-text) |

## Gemini 转写

转写会变成一次对转写模型的 `generateContent` 调用，音频作为 `inlineData`，并设置 `generationConfig.audioTranscriptionConfig`（[audio transcription](https://aistudio.google.com/docs/transcribe)，discovery 修订版 20261106 中的 `AudioTranscriptionConfig`）：`language` 变为 `languageCodes`，`diarize` 或 `diarized_json` 变为 `diarization: true`，所有带时间的形式和词级时间戳都设置 `wordTimestamp: true`。回复的文本部分即文本；每个 `audioTranscription` 部分是一个片段，带其 `speakerLabel` 和词，`"0.400s"` 这样的偏移量按秒读取。翻译、链接、`prompt`、`temperature` 和说话人数量都是 `Unsupported`。`fixtures/gemini/requests/transcribe-diarized.json` 及其录制的回复显示，一段 1 秒的 MP3 被转写为一个说话人和两个带时间的词。

## QwenCloud 转写

QwenCloud 通过其 OpenAI 兼容的 `chat/completions` 提供 `qwen3-asr-flash`（[Qwen-ASR](https://www.alibabacloud.com/help/en/model-studio/qwen-asr-api-reference)），因此发给 QwenCloud 的转写经过 `openai_chat` 编解码器：一条用户消息，包含一个 `input_audio` 部分，其 `data` 为音频的 data URL（或应用的链接），再加上 `asr_options.language` 和 `stream: false`。回复消息的内容即文本，其 `audio_info` 注释为语言，`usage.seconds` 为计费秒数。只能使用 `json` 和 `text`：该模型在此 API 上不返回时间戳。
