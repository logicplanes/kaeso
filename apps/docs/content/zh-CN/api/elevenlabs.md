---
title: ElevenLabs API
description: 通过 Kaeso 使用 ElevenLabs 官方 SDK 调用 ElevenLabs 的语音、音效、音乐和转写。
order: 2
---

使用官方 `elevenlabs` SDK 构建的应用可以通过 Kaeso 调用 ElevenLabs：把 Kaeso 设为基础 URL，把 Kaeso 密钥设为 API 密钥。Kaeso 提供 ElevenLabs 的生成和列表端点，带有密钥、限额、日志和费用，并把每个请求发送给模型所属的服务商，因此 ElevenLabs 应用也能调用其他服务商的语音和转写模型。

```bash tab="curl"
curl "https://kaeso.example.com/v1/text-to-speech/21m00Tcm4TlvDq8ikWAM?output_format=mp3_44100_128" \
  -H "xi-api-key: $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{"text": "Your order shipped today.", "model_id": "eleven_flash_v2_5"}' \
  --output speech.mp3
```

```python tab="Python"
import os
from elevenlabs import ElevenLabs

client = ElevenLabs(base_url="https://kaeso.example.com", api_key=os.environ["KAESO_API_KEY"])

audio = client.text_to_speech.convert(
    voice_id="21m00Tcm4TlvDq8ikWAM",
    text="Your order shipped today.",
    model_id="eleven_flash_v2_5",
    output_format="mp3_44100_128",
)
with open("speech.mp3", "wb") as f:
    for chunk in audio:
        f.write(chunk)
```

```ts tab="TypeScript"
import fs from "node:fs";
import { ElevenLabsClient } from "@elevenlabs/elevenlabs-js";

const client = new ElevenLabsClient({ baseUrl: "https://kaeso.example.com", apiKey: process.env.KAESO_API_KEY });

const audio = await client.textToSpeech.convert("21m00Tcm4TlvDq8ikWAM", {
  text: "Your order shipped today.",
  modelId: "eleven_flash_v2_5",
});
fs.writeFileSync("speech.mp3", Buffer.from(await new Response(audio).arrayBuffer()));
```

SDK 在 `xi-api-key` 中发送密钥；Kaeso 在这里接受 Kaeso 密钥，与在 `Authorization: Bearer` 中相同。在控制台中用 ElevenLabs 密钥添加 ElevenLabs 服务商，或者通过 OpenAI SDK 使用[音频 API](audio.md)。

## 端点

| 端点 | SDK 调用 | Kaeso |
| --- | --- | --- |
| `POST /v1/text-to-speech/{voice_id}` 以及 `/stream`、`/with-timestamps`、`/stream/with-timestamps` | `text_to_speech.convert`、`.stream`、`.convert_with_timestamps`、`.stream_with_timestamps` | 语音，音频边生成边发送；时间戳为 ElevenLabs 的 JSON 或其逐行 JSON |
| `POST /v1/text-to-dialogue` 及其流式和时间戳形式 | `text_to_dialogue.convert` 等 | 对话，每句使用各自的声音 |
| `POST /v1/sound-generation` | `text_to_sound_effects.convert` | 音效，按其时长的分钟计费 |
| `POST /v1/music` 和 `/v1/music/stream` | `music.compose`、`music.stream` | 音乐；ElevenLabs 在免费套餐中拒绝 |
| `POST /v1/voice-changer/{voice_id}` 和 `/stream` | `speech_to_speech.convert`、`.stream` | 由上传内容生成换成另一个声音的语音 |
| `POST /v1/speech-to-text` | `speech_to_text.convert` | 带时间的词和说话人的转写稿 |
| `GET /v1/models` | `models.list` | 项目的 ElevenLabs 模型，来自 Kaeso 的目录 |
| `GET /v1/voices`、`GET /v2/voices`、`GET /v1/voices/{voice_id}` | `voices.get_all`、`voices.search`、`voices.get` | 项目所用 ElevenLabs 账户的声音，用项目的 ElevenLabs 密钥向 ElevenLabs 查询 |

`output_format`、`enable_logging` 和 `optimize_streaming_latency` 按原样传给 ElevenLabs。上传的请求体受 [`body_limits.audio`](../self-hosting/configuration.md#请求体与响应体) 限制。

## 其他服务商

ElevenLabs 应用的请求就是 Kaeso 请求：别名或其他服务商的模型能处理时，就由它们处理。文本转语音可以调用 OpenAI、Google 以及[音频 API](audio.md#服务商)的每个服务商，声音 ID 作为声音；语音转文本可以调用每个转写服务商，服务商提供时间信息时也包括带时间的词。只有 ElevenLabs 接受的内容，例如 `speed` 以外的 `voice_settings`、发音词典或 `keyterms`，对其他服务商会被拒绝，`param` 中给出该字段。

## Kaeso 拒绝的内容

账户、声音克隆和设计、配音、工作室项目、历史记录、发音词典、工作区、智能体、一次性令牌以及其他管理端点仍在 ElevenLabs 上使用。Kaeso 以 ElevenLabs 的错误结构返回 404：`{"detail": {"type": "not_found", "message": "Kaeso does not serve `/v1/dubbing`; manage voices, dubbing and your account at ElevenLabs."}}`。`POST /v1/music/detailed` 和 `/v1/music/plan` 也会被拒绝。

实时 WebSocket API（文本转语音输入流、多上下文、对话 WebSocket、实时语音转文本和智能体）不提供。WebSocket 会话在整个生命周期内占用一条上游连接，双向传输音频和文本，并按会话计费，而 Kaeso 的模型管道只翻译一个请求及其回复；提供这些 API 需要单独的会话层，已有 [issue](https://github.com/logicplanes/kaeso/issues/803) 跟踪。

## 费用和日志

ElevenLabs 按字符对语音计费，按 KPMR 中每个模型每百万字符的价格计算；Kaeso 记录 ElevenLabs 在 `character-cost` 响应头中报告的字符数。转写稿按音频的小时计费，音效和音乐按其时长的分钟计费。[请求日志](../console/request-logs.md)写明端点，例如 `text-to-speech` 或 `speech-to-text`，从不保存音频。
