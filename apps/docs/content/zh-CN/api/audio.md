---
title: 音频 API
description: 通过 Kaeso 使用 OpenAI 的音频 API 生成语音、转写或翻译音频，适用于所有提供该功能的服务商。
order: 2
---

应用通过 Kaeso，使用 OpenAI 的音频 API 和 OpenAI 官方 SDK 生成语音和转写音频。Kaeso 按模型所属服务商自己的形式发送每个请求，回复以 OpenAI 的形式返回：音频本身、`speech.audio.*` 事件，或应用所要求形式的转写稿。

```bash tab="curl"
curl https://kaeso.example.com/v1/audio/speech \
  -H "Authorization: Bearer $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{"model": "gpt-4o-mini-tts", "input": "Your order shipped today.", "voice": "coral"}' \
  --output speech.mp3
```

```python tab="Python"
import os
from openai import OpenAI

client = OpenAI(base_url="https://kaeso.example.com/v1", api_key=os.environ["KAESO_API_KEY"])

with client.audio.speech.with_streaming_response.create(
    model="gpt-4o-mini-tts",
    voice="coral",
    input="Your order shipped today.",
) as response:
    response.stream_to_file("speech.mp3")

with open("meeting.mp3", "rb") as audio:
    transcript = client.audio.transcriptions.create(model="whisper-1", file=audio, response_format="srt")
print(transcript)
```

```ts tab="TypeScript"
import fs from "node:fs";
import OpenAI from "openai";

const client = new OpenAI({ baseURL: "https://kaeso.example.com/v1", apiKey: process.env.KAESO_API_KEY });

const speech = await client.audio.speech.create({ model: "gpt-4o-mini-tts", voice: "coral", input: "Your order shipped today." });
fs.writeFileSync("speech.mp3", Buffer.from(await speech.arrayBuffer()));

const transcript = await client.audio.transcriptions.create({ model: "gpt-4o-transcribe", file: fs.createReadStream("meeting.mp3") });
console.log(transcript.text);
```

## 端点

| 端点 | 请求体 | 回复 |
| --- | --- | --- |
| `POST /v1/audio/speech` | JSON：`model`、`input`、`voice`、`instructions`、`response_format`、`speed`、`stream_format` | 所要求格式的音频，边生成边发送；设置 `stream_format: "sse"` 时为 `speech.audio.delta` 事件和带用量的 `speech.audio.done` |
| `POST /v1/audio/transcriptions` | 上传：`file`、`model`、`language`、`prompt`、`response_format`、`temperature`、`timestamp_granularities[]`、`stream`，对 OpenAI 还有 `include[]`、`chunking_strategy` 和已知说话人 | `json`、`text`、`srt`、`verbose_json`、`vtt` 或 `diarized_json`；设置 `stream: true` 时为 `transcript.text.*` 事件 |
| `POST /v1/audio/translations` | 上传：`file`、`model`、`prompt`、`response_format`、`temperature` | 英文转写稿，`json`、`text`、`srt`、`verbose_json` 或 `vtt` |

密钥、通过别名和路由解析模型、限额和预算、请求日志以及 OpenAI 格式的错误，都与其他模型请求相同。上传的请求体受 [`body_limits.audio`](../self-hosting/configuration.md#请求体与响应体) 限制，默认 26 MiB；OpenAI 接受最大 25 MB 的文件。

## 语音

- **声音。** 只有服务商列出的声音才会传过去：OpenAI 的声音传给 OpenAI，Gemini 的 30 个预置声音传给 Google 模型，SpaceX 的声音 ID 传给 SpaceX。声音取决于模型的服务商（Groq、Together AI、Mistral、OpenRouter、MiniMax）按原样接收名称。QwenCloud 接收其列表为该模型列出的声音，例如 `qwen3-tts-flash` 的 `Cherry`，或声音 ID，例如克隆声音的 ID。ElevenLabs 按 ID 接收声音，即其 `GET /v1/voices` 列出的 ID：把 ID 作为 `voice` 发送，或发送 `{"id": "…"}`；`alloy` 这样的名称会被拒绝，因为 Kaeso 不猜测它对应 ElevenLabs 的哪个声音。
- **格式**从不转换。服务商没有的格式会在发送任何内容之前被拒绝，`param` 中为 `response_format`。应用未指定格式时，OpenAI 返回 mp3，因此 Kaeso 向每个服务商都明确请求 mp3；`qwen3-tts-flash` 只生成 wav 和 pcm，因此应用需指定其中之一。
- **流式。** 音频按服务商发送的样子传给应用。请求 `stream_format: "sse"` 时，Kaeso 为任何服务商写出 `speech.audio.delta` 事件；一次性发送完整音频的服务商会得到一个增量和结束事件。OpenAI 只在事件中报告用量，因此 Kaeso 向 OpenAI 请求事件，再为需要音频的应用还原成音频；`tts-1` 和 `tts-1-hd` 没有事件，按字符计费。
- **设置**：服务商不支持的设置，例如 Gemini 的 `instructions` 或 Together AI 的 `speed`，会被拒绝，`param` 中给出该字段。
- **QwenCloud** 边生成边流式发送语音，Kaeso 总是这样请求。`qwen3-tts-flash` 的完整 wav 回复是一个链接，Kaeso 会像下载 [QwenCloud 的图片](images.md#服务商)一样下载它。它的实时模型只接受 WebSocket 连接，会被拒绝。

## 转写和翻译

- **各种形式。** 服务商不支持应用要求的形式时，Kaeso 向它请求带时间的片段，自己写出字幕或片段：Groq 由 `verbose_json` 生成 `srt` 和 `vtt`，Mistral 由其片段生成，Gemini 和 SpaceX 由其带时间的词生成，ElevenLabs 由其词生成。任何服务商的时间信息都无法生成的形式，例如没有说话人的 `diarized_json`，会被拒绝。
- **链接。** 应用的上传内容以上传形式发送。应用在 `url` 或 `file_url` 中发送的链接，或 ElevenLabs 应用在 `source_url` 中发送的链接，会以链接形式发给接受链接的服务商（Groq 和 SpaceX 的 `url`、Together AI 的 `file`、Mistral 的 `file_url`、OpenRouter 和 ElevenLabs 的 `source_url`）；OpenAI 不接受链接。
- **流式。** OpenAI、Mistral、Z.ai 和 MiniMax 以流式返回转写稿；Kaeso 把它们写成 `transcript.text.delta` 和 `transcript.text.done`。其他服务商一次性返回，应用会得到一个增量和结束事件。
- **翻译**只发给有翻译端点的服务商：OpenAI 和 Groq。
- **QwenCloud 的**文件转写模型（`*-filetrans`）只回复异步任务，其流式和实时模型只接受 WebSocket 连接，因此会被拒绝。

## 服务商

| 服务商 | 语音 | 转写和翻译 | 来源 |
| --- | --- | --- | --- |
| OpenAI | `/v1/audio/speech`：`gpt-4o-mini-tts`、`tts-1`、`tts-1-hd` | `/v1/audio/transcriptions` 和 `/translations`：`gpt-transcribe`、`gpt-4o-transcribe`、`gpt-4o-mini-transcribe`、`gpt-4o-transcribe-diarize`、`whisper-1` | [text to speech](https://developers.openai.com/api/docs/guides/text-to-speech)、[speech to text](https://developers.openai.com/api/docs/guides/speech-to-text) |
| Google AI Studio | 带 `responseModalities: ["AUDIO"]` 的 `generateContent`：`gemini-3.8-flash-tts`、`gemini-3.8-flash-lite-tts` | 带 `audioTranscriptionConfig` 的 `generateContent`：`gemini-3.5-transcribe` | [speech generation](https://aistudio.google.com/docs/speech-generation)、[audio transcription](https://aistudio.google.com/docs/transcribe) |
| Gemini Enterprise Agent Platform | 带 `AUDIO` 的 `generateContent`：`gemini-3.8-flash-tts`、`gemini-3.8-flash-lite-tts`、`gemini-2.5-*-tts` | 带 `audioTranscriptionConfig` 的 `generateContent` | [Gemini-TTS](https://docs.cloud.google.com/text-to-speech/docs/gemini-tts) |
| Groq | Orpheus：`canopylabs/orpheus-v1-english`、`canopylabs/orpheus-arabic-saudi`，仅 wav | `whisper-large-v3`、`whisper-large-v3-turbo`；支持翻译 | [text to speech](https://console.groq.com/docs/text-to-speech/orpheus)、[speech to text](https://console.groq.com/docs/speech-to-text) |
| Together AI | Orpheus、Cartesia Sonic、Kokoro；mp3 或 wav | Whisper、Parakeet、Nemotron ASR；支持翻译 | [text to speech](https://docs.together.ai/docs/inference/text-to-speech/overview)、[transcription](https://docs.together.ai/docs/inference/transcription/overview) |
| Mistral | Voxtral TTS（`voxtral-mini-tts-3-0`） | Voxtral Transcribe 3 | [audio](https://docs.mistral.ai/studio/audio/migrate-from-voxtral-2) |
| OpenRouter | `/api/v1/audio/speech`，mp3 或 pcm | `/api/v1/audio/transcriptions` | [TTS](https://openrouter.ai/docs/guides/overview/multimodal/tts)、[STT](https://openrouter.ai/docs/guides/overview/multimodal/stt) |
| Vercel | `/v4/ai/speech-model`，mp3 | `/v4/ai/transcription-model` | [text to speech](https://vercel.com/docs/ai-gateway/modalities/text-to-speech)、[speech to text](https://vercel.com/docs/ai-gateway/modalities/speech-to-text) |
| SpaceX | `/v1/tts` | `/v1/stt`：`grok-voice-transcribe-2.0` | [voice](https://docs.x.ai/developers/rest-api-reference/inference/voice) |
| MiniMax | `/v1/t2a_v2`：`speech-2.8-*`、`speech-2.6-*`、`speech-02-*` | `/v1/speech_to_text`：`asr-1.0` | [T2A](https://platform.minimax.io/docs/api-reference/speech-t2a-http)、[speech to text](https://platform.minimax.io/docs/api-reference/speech-to-text) |
| ElevenLabs | 文本转语音：`eleven_v4`、`eleven_v3`、`eleven_multilingual_v2`、`eleven_flash_v2_5` 等 | 语音转文本：`scribe_v2`、`scribe_v2_medical` | [ElevenLabs API](elevenlabs.md) |
| Z.ai | 不提供 | `/api/paas/v4/audio/transcriptions`：`glm-asr-2512` | [GLM-ASR](https://docs.z.ai/guides/audio/glm-asr-2512) |
| QwenCloud | DashScope：`qwen-audio-3.1-tts-flash`、`qwen-audio-3.1-tts-plus`、`qwen-audio-3.0-tts-flash`、`qwen-audio-3.0-tts-plus`（mp3、wav、opus、pcm），`qwen3-tts-flash`（wav 或 pcm） | 通过 `chat/completions` 调用 `qwen3-asr-flash`；DashScope：`qwen-audio-3.1-asr-flash`、`qwen-audio-3.0-asr-flash`，`json` 和 `text` | [Qwen-ASR](https://www.alibabacloud.com/help/en/model-studio/qwen-asr-api-reference), [Qwen-Audio-TTS](https://docs.qwencloud.com/api-reference/speech-synthesis/qwen-audio-tts/http-api), [Qwen3-TTS](https://docs.qwencloud.com/api-reference/speech-synthesis/qwen-tts), [Qwen-Audio-ASR](https://docs.qwencloud.com/api-reference/speech-recognition/qwen-audio-asr/http-api) |
| Cohere | 不提供 | `/v2/audio/transcriptions`：`cohere-transcribe-03-2026`，必须提供 `language` | [transcribe](https://docs.cohere.com/v2/docs/transcribe) |
| Anthropic、DeepSeek、Moonshot、Cerebras、Ollama Cloud | 不提供 | 不提供 | [支持的服务商](../concepts/supported-providers.md) |

发给没有该 API 的服务商的请求会在发送任何内容之前被拒绝，例如“Anthropic offers no speech API Kaeso can call, so `claude-haiku-4-5` can't speak.”。

## 费用和日志

- **费用。** 语音按字符计费（`tts-1`、Groq、Together AI、Mistral、MiniMax、ElevenLabs、QwenCloud），或按 Token 以模型的音频输出价格计费（`gpt-4o-mini-tts`、Gemini）。转写稿按音频的分钟或小时计费（Whisper、Scribe、Voxtral，Groq 最少按 10 秒计费）或按秒计费（QwenCloud），或按 Token 以音频输入价格计费。价格来自 KPMR 的按单位价格和音频 Token 价格；MiniMax、ElevenLabs 和 QwenCloud 会报告其计费的字符数，Kaeso 采用这些数字。
- **日志。** [请求日志](../console/request-logs.md)显示端点、语音的字符数或所转写音频的时长，以及格式。音频从不记录：上传内容只记录其字段，音频只记录媒体类型和大小。
