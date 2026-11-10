---
title: OpenAI Speech
description: How OpenAI's speech API maps to KPP, and how Gemini, MiniMax and other providers' speech formats take it.
order: 8
---

Look up any field of OpenAI's speech API here: where it goes in KPP, which providers take it in which form, and the source of each mapping. Check it before you change how a speech codec reads or writes a field.

- **Codecs:** `kaeso_codecs::openai_speech::OpenAiSpeech` (`crates/kaeso-codecs/src/openai_speech/`) and speech requests in the Gemini codec (`crates/kaeso-codecs/src/gemini/audio.rs`); MiniMax's own format has [its page](minimax-speech.md)
- **Endpoint:** `POST /v1/audio/speech`
- **Fixtures:** `crates/kaeso-codecs/fixtures/openai_speech/`, built from the specification and the providers' documentation; `fixtures/gemini/` holds recorded Gemini speech

**Sources.** The OpenAI OpenAPI spec at commit `926a7b8133226630bac025350e84b0ca5c31d45a` (2026-11-07): `CreateSpeechRequest`, `CreateSpeechResponseStreamEvent`, `SpeechAudioDeltaEvent`, `SpeechAudioDoneEvent`. The provider pages are named in [providers](#providers-and-their-forms).

## Decisions

- **One KPP request for generated audio.** `Request::Speech(SpeechRequest)` with `SpeechOperation` `Speech`, `Dialogue`, `SoundEffect`, `Music` or `VoiceChange`, `Reply::Speech(SpeechResponse)` with the audio base64-encoded and its media type, and `StreamEvent::Speech(SpeechEvent)` for pieces of audio and the end with its usage. Dialogue, sound effects, music and the voice changer come from [ElevenLabs](elevenlabs.md); OpenAI's speech API makes only `Speech`.
- **Voices are names or ids.** `Voice::Named` for a voice a provider names, `Voice::Id` for a voice by id (OpenAI's `{"id": …}`, an ElevenLabs voice id). A name crosses only to a provider that lists it.
- **Audio flows as it is made.** A reply in raw audio is read with the stream decoder whether or not the app streams (`Codec::reads_reply_as_stream`), so the app's first bytes don't wait for the last. `Codec::response_media_type` and `stream_media_type` name what the codec wrote: the audio's media type, or `text/event-stream` for events.
- **No conversion.** A format, a voice or a sample rate the provider lacks is `Unsupported` before anything is sent.

## How to read the tables

- **typed**: a typed KPP field.
- **extras**: `Extras::fields`, bound to `openai_speech`; providers of the dialect receive them, other dialects refuse them.
- **form**: a codec-private `Extras::form` marker.

## Request

| `CreateSpeechRequest` | KPP | Class |
| --- | --- | --- |
| `model` | `SpeechRequest::model` | typed |
| `input` | `input` | typed |
| `voice`, a string | `voice: Voice::Named` | typed |
| `voice`, `{"id": …}` | `voice: Voice::Id` | typed |
| `instructions` | `instructions` | typed |
| `response_format` | `format`: `mp3`, `opus`, `aac`, `flac`, `wav` as `AudioFormat`, `pcm` as `Pcm16` (24 kHz, 16-bit, mono) | typed |
| no `response_format` | `format: Mp3`, with the marker `default_format` | typed, form |
| `speed` | `speed`; an integer stays one (`integers`) | typed, form |
| `stream_format: "sse"` | `stream: On` | typed |
| `stream_format: "audio"` | `stream: Off` with the marker `stream_format` | form |
| `sample_rate`, `language`, `seed` (other providers' forms) | `sample_rate`, `language`, `seed` | typed |
| any other field | the field | extras |

## Reply and stream

| Wire | KPP |
| --- | --- |
| the audio bytes, with their `content-type` | `SpeechResponse::audio` as base64 with that media type, or `SpeechEvent::audio` pieces when read as it arrives |
| `speech.audio.delta` `audio` | `SpeechEvent::audio` |
| `speech.audio.done` `usage` (`input_tokens`, `output_tokens`) | `SpeechEvent::done` with `Usage`, the output as `output_audio_tokens` |
| `error` event, error body | `KppError` through OpenAI's error codec |

To an app, Kaeso writes the audio bytes, with the media type OpenAI answers the asked format with (`audio/mpeg`, `audio/opus`, `audio/aac`, `audio/flac`, `audio/wav`, `audio/pcm`), or, for `stream_format: "sse"`, a `speech.audio.delta` per piece and `speech.audio.done`. A whole reply from a provider without events becomes one delta and the end.

**OpenAI's usage.** OpenAI reports usage only in its events. Kaeso asks OpenAI for events (`SpeechProfile::usage_in_events`) unless the model has none (`sse_refused`: `tts-1`, `tts-1-hd`), and an app that wants the audio still gets bytes. The round-trip test removes that `stream_format` before comparing.

## Providers and their forms

`SpeechProfile` describes each provider that speaks a form of OpenAI's speech API: its path and body (`SpeechBody`), its reply (`SpeechReply`), voices, formats, default format, events (`SpeechEvents`) and the fields it takes beyond OpenAI's. An app that named no format gets OpenAI's mp3, so Kaeso names mp3 to a provider whose default differs.

| Profile | Form | Source |
| --- | --- | --- |
| `OPENAI` | the spec | the spec above |
| `GROQ` | `wav` only (the Orpheus guide; its API reference lists more), `sample_rate`, no events, no `instructions` | [Orpheus](https://console.groq.com/docs/text-to-speech/orpheus) |
| `TOGETHER` | `mp3` or `wav`, `sample_rate`, `language`, no `speed`, no events | [`AudioSpeechRequest`](https://docs.together.ai/reference/audio-speech) |
| `MISTRAL` | `voice_id`, a JSON reply with base64 `audio_data`, events with `stream: true` carrying `audio_data` and `UsageInfo` | [generate speech](https://docs.mistral.ai/studio/audio/text-to-speech/generate-speech), [stream speech](https://docs.mistral.ai/studio/audio/text-to-speech/stream-speech) |
| `OPENROUTER` | `mp3` or `pcm`, `pcm` by default, no events | [TTS](https://openrouter.ai/docs/guides/overview/multimodal/tts) |
| `XAI` | `POST /v1/tts`: `text`, `voice_id`, a required `language` (`auto` when the app names none), `output_format {codec, sample_rate}`, no model | [voice](https://docs.x.ai/developers/rest-api-reference/inference/voice) |
| `VERCEL` | `POST /v4/ai/speech-model`, the model in `ai-model-id`, `text`, `voice`, `outputFormat` (`mp3` only documented), a JSON reply with base64 `audio` | [text to speech](https://vercel.com/docs/ai-gateway/modalities/text-to-speech) |
| `SILICONFLOW` | voices named `<model>:<voice>`, `mp3`, `opus`, `wav` or `pcm`, `sample_rate`, `speed`, no events, no `instructions` | [create speech](https://docs.siliconflow.com/en/api-reference/audio/create-speech) |

SpaceX's and Vercel's bodies take no fields from other dialects; a field in extras is refused.

## Gemini speech

A speech request becomes a `generateContent` call (`gemini/audio.rs`) with the text as the user's part, `responseModalities: ["AUDIO"]`, `speechConfig.voiceConfig.prebuiltVoiceConfig.voiceName` for one of Gemini's 30 voices, `speechConfig.languageCode` and `seed`. `wav`, `mp3` and `pcm` become `responseFormat.audio.mimeType` `AUDIO_WAV`, `AUDIO_MP3` and `AUDIO_L16`, a list of one on the Agent Platform (`GeminiProfile::response_format_list`). Gemini streams 16-bit PCM only, so `pcm` goes to `streamGenerateContent` and the other formats come whole. `instructions` (write the style into `input`, as Google's guide does), `speed`, `sample_rate`, timestamps and other operations are `Unsupported`. A reply without audio is an error, `ContentFiltered` when Google's filters stopped it. The recorded fixtures `gemini/requests/speech-mp3.json` and its reply show `AUDIO_MP3` answered as `audio/mp3; rate=44100; channels=1`.

## MiniMax speech

`minimax_speech` is MiniMax's own format, `POST /v1/t2a_v2`. [MiniMax Speech](minimax-speech.md) maps its fields, refusals and errors.
