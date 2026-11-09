---
title: OpenAI Speech
description: OpenAI 语音 API 如何映射到 KPP，以及 Gemini、MiniMax 和其他服务商的语音格式如何接收它。
order: 8
---

在这里查阅 OpenAI 语音 API 的任意字段：它在 KPP 中的位置、哪些服务商以何种形式接收它，以及每项映射的来源。修改语音编解码器读写某个字段的方式之前，先查看这里。

- **编解码器：**`kaeso_codecs::openai_speech::OpenAiSpeech`（`crates/kaeso-codecs/src/openai_speech/`），以及 Gemini 编解码器中的语音请求（`crates/kaeso-codecs/src/gemini/audio.rs`）；MiniMax 自己的格式见[其页面](minimax-speech.md)
- **端点：**`POST /v1/audio/speech`
- **Fixture：**`crates/kaeso-codecs/fixtures/openai_speech/`，依据规范和服务商文档构建；`fixtures/gemini/` 包含录制的 Gemini 语音

**来源。**OpenAI OpenAPI 规范，提交 `926a7b8133226630bac025350e84b0ca5c31d45a`（2026-11-07）：`CreateSpeechRequest`、`CreateSpeechResponseStreamEvent`、`SpeechAudioDeltaEvent`、`SpeechAudioDoneEvent`。各服务商的页面列在[服务商](#服务商及其形式)中。

## 决定

- **生成的音频共用一个 KPP 请求。**`Request::Speech(SpeechRequest)`，`SpeechOperation` 为 `Speech`、`Dialogue`、`SoundEffect`、`Music` 或 `VoiceChange`；`Reply::Speech(SpeechResponse)` 包含 base64 编码的音频及其媒体类型；`StreamEvent::Speech(SpeechEvent)` 表示音频片段以及带用量的结束事件。对话、音效、音乐和变声来自 [ElevenLabs](elevenlabs.md)；OpenAI 的语音 API 只生成 `Speech`。
- **声音是名称或 ID。**`Voice::Named` 表示服务商命名的声音，`Voice::Id` 表示按 ID 指定的声音（OpenAI 的 `{"id": …}`、ElevenLabs 的声音 ID）。名称只会传给列出它的服务商。
- **音频边生成边传输。**无论应用是否使用流式，原始音频的回复都用流解码器读取（`Codec::reads_reply_as_stream`），这样应用的第一批字节不必等待最后一批。`Codec::response_media_type` 和 `stream_media_type` 说明编解码器写出的内容：音频的媒体类型，或事件的 `text/event-stream`。
- **不做转换。**服务商没有的格式、声音或采样率会在发送任何内容之前得到 `Unsupported`。

## 表格的读法

- **typed**：有类型的 KPP 字段。
- **extras**：`Extras::fields`，绑定到 `openai_speech`；该方言的服务商会收到它们，其他方言会拒绝它们。
- **form**：编解码器私有的 `Extras::form` 标记。

## 请求

| `CreateSpeechRequest` | KPP | 类别 |
| --- | --- | --- |
| `model` | `SpeechRequest::model` | typed |
| `input` | `input` | typed |
| `voice`，字符串 | `voice: Voice::Named` | typed |
| `voice`，`{"id": …}` | `voice: Voice::Id` | typed |
| `instructions` | `instructions` | typed |
| `response_format` | `format`：`mp3`、`opus`、`aac`、`flac`、`wav` 为 `AudioFormat`，`pcm` 为 `Pcm16`（24 kHz、16 位、单声道） | typed |
| 没有 `response_format` | `format: Mp3`，带 `default_format` 标记 | typed、form |
| `speed` | `speed`；整数保持为整数（`integers`） | typed、form |
| `stream_format: "sse"` | `stream: On` | typed |
| `stream_format: "audio"` | `stream: Off`，带 `stream_format` 标记 | form |
| `sample_rate`、`language`、`seed`（其他服务商的形式） | `sample_rate`、`language`、`seed` | typed |
| 其他任何字段 | 该字段 | extras |

## 回复和流

| 线上格式 | KPP |
| --- | --- |
| 音频字节及其 `content-type` | `SpeechResponse::audio`，以 base64 保存并带该媒体类型；边到达边读取时为 `SpeechEvent::audio` 片段 |
| `speech.audio.delta` `audio` | `SpeechEvent::audio` |
| `speech.audio.done` `usage`（`input_tokens`、`output_tokens`） | 带 `Usage` 的 `SpeechEvent::done`，输出计为 `output_audio_tokens` |
| `error` 事件、错误体 | 通过 OpenAI 错误编解码器得到的 `KppError` |

Kaeso 向应用写出音频字节，媒体类型与 OpenAI 对所要求格式的回复相同（`audio/mpeg`、`audio/opus`、`audio/aac`、`audio/flac`、`audio/wav`、`audio/pcm`）；对于 `stream_format: "sse"`，则每个片段写一个 `speech.audio.delta`，最后写 `speech.audio.done`。没有事件的服务商返回的完整回复会变成一个增量和结束事件。

**OpenAI 的用量。**OpenAI 只在事件中报告用量。除非模型没有事件（`sse_refused`：`tts-1`、`tts-1-hd`），Kaeso 都向 OpenAI 请求事件（`SpeechProfile::usage_in_events`），需要音频的应用仍然收到字节。往返测试在比较前会移除这个 `stream_format`。

## 服务商及其形式

`SpeechProfile` 描述每个使用 OpenAI 语音 API 某种形式的服务商：路径和请求体（`SpeechBody`）、回复（`SpeechReply`）、声音、格式、默认格式、事件（`SpeechEvents`），以及它在 OpenAI 字段之外接受的字段。未指定格式的应用会得到 OpenAI 的 mp3，因此对默认格式不同的服务商，Kaeso 会明确指定 mp3。

| Profile | 形式 | 来源 |
| --- | --- | --- |
| `OPENAI` | 规范 | 上述规范 |
| `GROQ` | 仅 `wav`（依据 Orpheus 指南；其 API 参考列出更多）、`sample_rate`，没有事件，没有 `instructions` | [Orpheus](https://console.groq.com/docs/text-to-speech/orpheus) |
| `TOGETHER` | `mp3` 或 `wav`、`sample_rate`、`language`，没有 `speed`，没有事件 | [`AudioSpeechRequest`](https://docs.together.ai/reference/audio-speech) |
| `MISTRAL` | `voice_id`，带 base64 `audio_data` 的 JSON 回复，`stream: true` 时的事件携带 `audio_data` 和 `UsageInfo` | [generate speech](https://docs.mistral.ai/studio/audio/text-to-speech/generate-speech)、[stream speech](https://docs.mistral.ai/studio/audio/text-to-speech/stream-speech) |
| `OPENROUTER` | `mp3` 或 `pcm`，默认 `pcm`，没有事件 | [TTS](https://openrouter.ai/docs/guides/overview/multimodal/tts) |
| `XAI` | `POST /v1/tts`：`text`、`voice_id`、必填的 `language`（应用未指定时为 `auto`）、`output_format {codec, sample_rate}`，没有模型 | [voice](https://docs.x.ai/developers/rest-api-reference/inference/voice) |
| `VERCEL` | `POST /v4/ai/speech-model`，模型放在 `ai-model-id` 中，`text`、`voice`、`outputFormat`（文档只写了 `mp3`），带 base64 `audio` 的 JSON 回复 | [text to speech](https://vercel.com/docs/ai-gateway/modalities/text-to-speech) |

SpaceX 和 Vercel 的请求体不接受其他方言的字段；extras 中的字段会被拒绝。

## Gemini 语音

语音请求会变成一次 `generateContent` 调用（`gemini/audio.rs`）：文本作为用户的部分，设置 `responseModalities: ["AUDIO"]`，用 `speechConfig.voiceConfig.prebuiltVoiceConfig.voiceName` 指定 Gemini 的 30 个声音之一，再加上 `speechConfig.languageCode` 和 `seed`。`wav`、`mp3` 和 `pcm` 分别变为 `responseFormat.audio.mimeType` 的 `AUDIO_WAV`、`AUDIO_MP3` 和 `AUDIO_L16`，在 Agent Platform 上是只含一项的列表（`GeminiProfile::response_format_list`）。Gemini 只以流式发送 16 位 PCM，因此 `pcm` 发往 `streamGenerateContent`，其他格式一次性返回。`instructions`（请像 Google 的指南那样把风格写进 `input`）、`speed`、`sample_rate`、时间戳和其他操作都是 `Unsupported`。没有音频的回复是错误，被 Google 的过滤器拦截时为 `ContentFiltered`。录制的 fixture `gemini/requests/speech-mp3.json` 及其回复显示，`AUDIO_MP3` 的回复为 `audio/mp3; rate=44100; channels=1`。

## MiniMax 语音

`minimax_speech` 是 MiniMax 自己的格式，即 `POST /v1/t2a_v2`。[MiniMax Speech](minimax-speech.md) 列出了它的字段映射、拒绝项和错误。
