---
title: Audio API
description: Make speech and transcribe or translate audio through Kaeso with OpenAI's audio API, on every provider that offers it.
order: 3
---

Apps make speech and transcribe audio through Kaeso with OpenAI's audio API and the official OpenAI SDKs. Kaeso sends each request to the model's provider in that provider's own form, and the reply comes back in OpenAI's: the audio itself, `speech.audio.*` events, or a transcript in the form the app asked for.

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

## Endpoints

| Endpoint | Body | Reply |
| --- | --- | --- |
| `POST /v1/audio/speech` | JSON: `model`, `input`, `voice`, `instructions`, `response_format`, `speed`, `stream_format` | the audio in the asked format, sent as it is made; with `stream_format: "sse"`, `speech.audio.delta` events and `speech.audio.done` with the usage |
| `POST /v1/audio/transcriptions` | an upload: `file`, `model`, `language`, `prompt`, `response_format`, `temperature`, `timestamp_granularities[]`, `stream`, and `include[]`, `chunking_strategy` and the known speakers for OpenAI | `json`, `text`, `srt`, `verbose_json`, `vtt` or `diarized_json`; with `stream: true`, `transcript.text.*` events |
| `POST /v1/audio/translations` | an upload: `file`, `model`, `prompt`, `response_format`, `temperature` | the English transcript, `json`, `text`, `srt`, `verbose_json` or `vtt` |

Keys, model resolution with aliases and routes, limits and budgets, request logs and errors in OpenAI's format work as for every other model request. The upload's body is bounded by [`body_limits.audio`](../self-hosting/configuration.md#request-and-response-bodies), 26 MiB by default; OpenAI takes files of up to 25 MB.

## Speech

- **Voices.** A voice crosses only where the provider lists it: OpenAI's voices for OpenAI, Gemini's 30 prebuilt voices for Google models, SpaceX's voice ids for SpaceX. Providers whose voices depend on the model (Groq, Together AI, Mistral, OpenRouter, MiniMax) take the name as it is. QwenCloud takes a voice its lists name for the model, such as `Cherry` for `qwen3-tts-flash`, or a voice id such as a cloned voice's. ElevenLabs takes a voice by its id, as its `GET /v1/voices` lists it: send the id as `voice`, or `{"id": "…"}`; a name such as `alloy` is refused, since Kaeso doesn't guess which ElevenLabs voice it means.
- **Formats** are never converted. A format the provider lacks is refused before anything is sent, with `response_format` in `param`. When the app names no format, OpenAI answers mp3, so Kaeso asks every provider for mp3 by name; `qwen3-tts-flash` makes only wav and pcm, so an app names one of them.
- **Streaming.** The audio goes to the app as the provider sends it. Asked for `stream_format: "sse"`, Kaeso writes `speech.audio.delta` events from any provider; a provider that sends its audio whole gets one delta and the end. OpenAI reports usage only in its events, so Kaeso asks OpenAI for events and turns them back into audio for an app that wants the audio; `tts-1` and `tts-1-hd` have no events and are billed per character.
- **Settings** a provider lacks, such as `instructions` for Gemini or `speed` for Together AI, are refused with the field in `param`.
- **QwenCloud** streams its speech as it makes it, and Kaeso always asks for that. A whole `qwen3-tts-flash` reply in wav is a link, which Kaeso downloads as it does [QwenCloud's images](images.md#providers). Its real-time models take only WebSocket connections and are refused.

## Transcription and translation

- **Every form.** When the provider lacks the app's form, Kaeso asks it for timed segments and writes the subtitles or the segments itself: `srt` and `vtt` from `verbose_json` on Groq, from Mistral's segments, from Gemini's and SpaceX's timed words, and from ElevenLabs' words. A form no provider timing can make, such as `diarized_json` without speakers, is refused.
- **Links.** An app's upload goes as an upload. A link an app sends in `url` or `file_url`, or an ElevenLabs app in `source_url`, goes as a link to the providers that take one (`url` at Groq and SpaceX, `file` at Together AI, `file_url` at Mistral, `source_url` at OpenRouter and ElevenLabs); OpenAI takes none.
- **Streaming.** OpenAI, Mistral, Z.ai and MiniMax stream their transcripts; Kaeso writes them as `transcript.text.delta` and `transcript.text.done`. Other providers answer whole, and the app gets one delta and the end.
- **Translations** go only to providers with a translations endpoint: OpenAI, Groq and Together AI.
- **QwenCloud's** file transcription models (`*-filetrans`) answer only asynchronous tasks, and its streaming and real-time models only WebSocket connections, so they are refused.

## Providers

| Provider | Speech | Transcription and translation | Source |
| --- | --- | --- | --- |
| OpenAI | `/v1/audio/speech`: `gpt-4o-mini-tts`, `tts-1`, `tts-1-hd` | `/v1/audio/transcriptions` and `/translations`: `gpt-transcribe`, `gpt-4o-transcribe`, `gpt-4o-mini-transcribe`, `gpt-4o-transcribe-diarize`, `whisper-1` | [text to speech](https://developers.openai.com/api/docs/guides/text-to-speech), [speech to text](https://developers.openai.com/api/docs/guides/speech-to-text) |
| Google AI Studio | `generateContent` with `responseModalities: ["AUDIO"]`: `gemini-3.8-flash-tts`, `gemini-3.8-flash-lite-tts` | `generateContent` with `audioTranscriptionConfig`: `gemini-3.5-transcribe` | [speech generation](https://aistudio.google.com/docs/speech-generation), [audio transcription](https://aistudio.google.com/docs/transcribe) |
| Gemini Enterprise Agent Platform | `generateContent` with `AUDIO`: `gemini-3.8-flash-tts`, `gemini-3.8-flash-lite-tts`, `gemini-2.5-*-tts` | `generateContent` with `audioTranscriptionConfig` | [Gemini-TTS](https://docs.cloud.google.com/text-to-speech/docs/gemini-tts) |
| Groq | Orpheus: `canopylabs/orpheus-v1-english`, `canopylabs/orpheus-arabic-saudi`, wav only | `whisper-large-v3`, `whisper-large-v3-turbo`; translations | [text to speech](https://console.groq.com/docs/text-to-speech/orpheus), [speech to text](https://console.groq.com/docs/speech-to-text) |
| Together AI | Orpheus, Cartesia Sonic, Kokoro; mp3 or wav | Whisper, Parakeet, Nemotron ASR; translations | [text to speech](https://docs.together.ai/docs/inference/text-to-speech/overview), [transcription](https://docs.together.ai/docs/inference/transcription/overview) |
| Mistral | Voxtral TTS (`voxtral-mini-tts-3-0`) | Voxtral Transcribe 3 | [audio](https://docs.mistral.ai/studio/audio/migrate-from-voxtral-2) |
| OpenRouter | `/api/v1/audio/speech`, mp3 or pcm | `/api/v1/audio/transcriptions` | [TTS](https://openrouter.ai/docs/guides/overview/multimodal/tts), [STT](https://openrouter.ai/docs/guides/overview/multimodal/stt) |
| Vercel | `/v4/ai/speech-model`, mp3 | `/v4/ai/transcription-model` | [text to speech](https://vercel.com/docs/ai-gateway/modalities/text-to-speech), [speech to text](https://vercel.com/docs/ai-gateway/modalities/speech-to-text) |
| SpaceX | `/v1/tts` | `/v1/stt`: `grok-voice-transcribe-2.0` | [voice](https://docs.x.ai/developers/rest-api-reference/inference/voice) |
| MiniMax | `/v1/t2a_v2`: `speech-2.8-*`, `speech-2.6-*`, `speech-02-*` | `/v1/speech_to_text`: `asr-1.0` | [T2A](https://platform.minimax.io/docs/api-reference/speech-t2a-http), [speech to text](https://platform.minimax.io/docs/api-reference/speech-to-text) |
| ElevenLabs | text to speech: `eleven_v4`, `eleven_v3`, `eleven_multilingual_v2`, `eleven_flash_v2_5` and the rest | speech to text: `scribe_v2`, `scribe_v2_medical` | [ElevenLabs API](elevenlabs.md) |
| Z.ai | not offered | `/api/paas/v4/audio/transcriptions`: `glm-asr-2512` | [GLM-ASR](https://docs.z.ai/guides/audio/glm-asr-2512) |
| QwenCloud | DashScope: `qwen-audio-3.1-tts-flash`, `qwen-audio-3.1-tts-plus`, `qwen-audio-3.0-tts-flash`, `qwen-audio-3.0-tts-plus` (mp3, wav, opus, pcm), `qwen3-tts-flash` (wav or pcm) | `qwen3-asr-flash` through `chat/completions`; DashScope: `qwen-audio-3.1-asr-flash`, `qwen-audio-3.0-asr-flash`, `json` and `text` | [Qwen-ASR](https://www.alibabacloud.com/help/en/model-studio/qwen-asr-api-reference), [Qwen-Audio-TTS](https://docs.qwencloud.com/api-reference/speech-synthesis/qwen-audio-tts/http-api), [Qwen3-TTS](https://docs.qwencloud.com/api-reference/speech-synthesis/qwen-tts), [Qwen-Audio-ASR](https://docs.qwencloud.com/api-reference/speech-recognition/qwen-audio-asr/http-api) |
| Cohere | not offered | `/v2/audio/transcriptions`: `cohere-transcribe-03-2026`, `language` required | [transcribe](https://docs.cohere.com/v2/docs/transcribe) |
| Anthropic, DeepSeek, Moonshot, Cerebras, Ollama Cloud | not offered | not offered | [supported providers](../concepts/supported-providers.md) |

A request to a provider without the API is refused before anything is sent, for example "Anthropic offers no speech API Kaeso can call, so `claude-haiku-4-5` can't speak.".

## Costs and logs

- **Costs.** Speech is billed per character (`tts-1`, Groq, Together AI, Mistral, MiniMax, ElevenLabs, QwenCloud) or by tokens with the model's audio output price (`gpt-4o-mini-tts`, Gemini). Transcripts are billed by the minute or the hour of audio (Whisper, Scribe, Voxtral, Groq with its 10-second minimum) or by the second (QwenCloud) or by tokens with the audio input price. Prices come from KPMR's prices per unit and audio token prices; MiniMax, ElevenLabs and QwenCloud report the characters they billed, which Kaeso uses.
- **Logs.** The [request log](../console/request-logs.md) shows the endpoint, the characters of speech or the length of the transcribed audio, and the format. Audio is never logged: an upload is logged as its fields, the audio as its media type and size.
