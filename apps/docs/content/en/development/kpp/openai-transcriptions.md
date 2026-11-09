---
title: OpenAI Transcriptions
description: How OpenAI's transcription and translation API maps to KPP, and how other providers' transcription formats take it.
order: 8
---

Look up any field of OpenAI's transcription and translation API here: where it goes in KPP, which providers take it in which form, and the source of each mapping. Check it before you change how a transcription codec reads or writes a field.

- **Codecs:** `kaeso_codecs::openai_transcriptions::OpenAiTranscriptions` (`crates/kaeso-codecs/src/openai_transcriptions/`), transcriptions in the Gemini codec (`gemini/audio.rs`) and in the Chat Completions codec for QwenCloud (`openai_chat/transcription.rs`)
- **Endpoints:** `POST /v1/audio/transcriptions` and `POST /v1/audio/translations`, uploads as `multipart/form-data`
- **Fixtures:** `crates/kaeso-codecs/fixtures/openai_transcriptions/`, built from the specification and the providers' documentation; `fixtures/gemini/` holds a recorded Gemini transcript

**Sources.** The OpenAI OpenAPI spec at commit `926a7b8133226630bac025350e84b0ca5c31d45a` (2026-11-07): `CreateTranscriptionRequest`, `CreateTranslationRequest`, `AudioResponseFormat`, `CreateTranscriptionResponseJson`, `CreateTranscriptionResponseVerboseJson`, `CreateTranscriptionResponseDiarizedJson`, `CreateTranslationResponseJson`, `CreateTranslationResponseVerboseJson`, `TranscriptionSegment`, `TranscriptionWord`, `TranscriptTextUsageTokens`, `TranscriptTextUsageDuration`, `TranscriptTextDeltaEvent`, `TranscriptTextSegmentEvent`, `TranscriptTextDoneEvent`. openai-python 3.41.0 for the multipart field names. RFC 7578.

## Decisions

- **One KPP request for both endpoints.** `Request::Transcription(TranscriptionRequest)` with `TranscriptionOperation` `Transcribe` or `Translate`; the path names the operation, so apps' uploads are decoded with `dispatch::decode_transcription_request`. `Reply::Transcription(TranscriptionResponse)` holds the text, the form it came in (`TranscriptFormat`), the language, the duration, timed segments and words with speakers, the usage and the billed seconds. `StreamEvent::Transcript(TranscriptEvent)` carries deltas, finished segments and the end.
- **Subtitles from timing.** When the provider lacks the app's form, Kaeso asks it for `verbose_json` with segment timestamps (or a provider's timed words) and writes `srt`, `vtt` or segments itself (`openai_transcriptions::subtitles`): numbered cues and `HH:MM:SS,mmm` times for SubRip, `WEBVTT` and `HH:MM:SS.mmm` for WebVTT. A reply with words and no segments becomes one cue from its first word to its last. A form no timing can make, such as `diarized_json` without speakers, is `Unsupported` before anything is sent.
- **No conversion.** Kaeso doesn't decode or convert audio; the upload goes as it came.

## Request

| `CreateTranscriptionRequest` field | KPP | Class |
| --- | --- | --- |
| `file` | `audio: AudioInput` with the upload's bytes, media type and file name | typed |
| `model` | `model` | typed |
| `language` | `language` | typed |
| `prompt` | `prompt` | typed |
| `response_format` | `format: TranscriptFormat` | typed |
| `temperature` | `temperature`; an integer stays one | typed, form |
| `timestamp_granularities[]` | `timestamps`; the field's spelling with or without `[]` | typed, form |
| `stream` | `stream`; an explicit `false` stays | typed, form |
| `diarize`, a link in `url` or `file_url` (other providers' forms) | `diarize`, `audio` as a link | typed, form |
| `include[]`, `chunking_strategy`, `known_speaker_names[]`, `known_speaker_references[]`, `languages[]`, `keywords[]` and any other field | the field, lists as arrays | extras |

## Reply and stream

| Wire | KPP |
| --- | --- |
| `json`: `text`, `usage`, `logprobs`, `languages` | `text`, `usage`; the rest extras |
| `verbose_json`: `language`, `duration`, `segments`, `words`, `usage` | `language`, `duration`, `segments` (`id`, `start`, `end`, `text`; `seek`, `tokens` and the scores as extras; a numeric `id` marked `numeric_id`), `words` |
| `diarized_json`: `segments` with `speaker` | `segments` with `speaker` |
| `text`, `srt`, `vtt` | `text` as written, with its `format` |
| usage `{"type": "tokens", …}` | `Usage`, the audio share as `input_audio_tokens` |
| usage `{"type": "duration", "seconds"}` | `usage_seconds` |
| `transcript.text.delta` `delta`, `segment_id`, `logprobs` | `TranscriptEvent::delta`, `segment_id`; `logprobs` extras |
| `transcript.text.segment` | `TranscriptEvent::segment` |
| `transcript.text.done` | `TranscriptEvent::done` with the whole transcript |

Kaeso writes a transcript in the app's form with `Codec::response_media_type`: `application/json`, `text/plain; charset=utf-8` for `text`, `srt` and `vtt`, or events. Fields another provider's form added, such as Groq's `x_groq` or Mistral's `finish_reason`, are that provider's and are left out of an OpenAI app's reply, as is a provider's usage shape, which Kaeso writes in OpenAI's form.

## Providers and their forms

`TranscriptionsProfile` describes each provider that speaks a form of the API: paths, body (`TranscriptionBody`), forms, whether it takes `response_format` at all, where it takes the language (`LanguageField`), how it takes timing (`TimestampsField`), links, streams and the fields beyond OpenAI's.

| Profile | Form | Source |
| --- | --- | --- |
| `OPENAI` | the spec | the spec above |
| `GROQ` | `json`, `text`, `verbose_json`; links in `url`; translations; no streams | [speech to text](https://console.groq.com/docs/speech-to-text) |
| `TOGETHER` | `json`, `verbose_json`; `timestamp_granularities`; links in `file`; `diarize`; translations | [transcription](https://docs.together.ai/docs/inference/transcription/overview) |
| `MISTRAL` | one JSON form with `segments` when `timestamp_granularities=segment` is sent, no word timing; `file_url`, `file_id`; `diarize`; `transcription.*` events; no `prompt`, no translations | [transcribe files](https://docs.mistral.ai/studio/audio/speech-to-text/transcribe-files) |
| `ZAI` | `/api/paas/v4/audio/transcriptions` from the root of the host, `Authorization: Bearer`; JSON text; no `language`, no `temperature`; events whose end may lack the text, which Kaeso fills from the deltas | [GLM-ASR](https://docs.z.ai/guides/audio/glm-asr-2512) |
| `XAI` | `/v1/stt`; timed words in every reply (`text`, a numbered `speaker`); links in `url`; `diarize`; the file last | [voice](https://docs.x.ai/developers/rest-api-reference/inference/voice) |
| `OPENROUTER` | `json`, `verbose_json`; links in `source_url`; usage with `seconds` | [STT](https://openrouter.ai/docs/guides/overview/multimodal/stt) |
| `COHERE` | `/v2/audio/transcriptions` from the root of the host; `language` required; the text alone | [transcribe](https://docs.cohere.com/v2/docs/transcribe) |
| `MINIMAX` | `/v1/speech_to_text` from the root of the host, `Authorization: Bearer`; `json`, `verbose_json`, `srt`, `vtt`; the language in a `language` header; `timestamp_level` `sentence` or `word`; `{index, delta, finish, duration}` events | [speech to text](https://platform.minimax.io/docs/api-reference/speech-to-text) |
| `VERCEL` | `/v4/ai/transcription-model`; JSON with base64 `audio` and `mediaType`, the model in `ai-model-id`; a reply with `durationInSeconds` | [speech to text](https://vercel.com/docs/ai-gateway/modalities/speech-to-text) |
| `SCALEWAY` | OpenAI's path and form; `json` only, with `usage` of type `duration`; no timestamps, no translations | [Generative APIs reference](https://www.scaleway.com/en/developers/api/generative-apis/) (`CreateAudioTranscriptionResponse`) |

## Gemini transcription

A transcription becomes a `generateContent` call to a transcribe model with the audio as `inlineData` and `generationConfig.audioTranscriptionConfig` ([audio transcription](https://aistudio.google.com/docs/transcribe), discovery `AudioTranscriptionConfig` at revision 20261106): `language` as `languageCodes`, `diarize` or `diarized_json` as `diarization: true`, and `wordTimestamp: true` for every form with times and for word timestamps. The reply's text parts are the text; each `audioTranscription` part is a segment with its `speakerLabel` and words, offsets such as `"0.400s"` read as seconds. Translations, links, `prompt`, `temperature` and a speaker count are `Unsupported`. `fixtures/gemini/requests/transcribe-diarized.json` and its recorded reply show a 1-second MP3 transcribed with a speaker and two timed words.

## QwenCloud transcription

QwenCloud serves `qwen3-asr-flash` through its OpenAI-compatible `chat/completions` ([Qwen-ASR](https://www.alibabacloud.com/help/en/model-studio/qwen-asr-api-reference)), so a transcription to QwenCloud goes through the `openai_chat` codec: one user message with an `input_audio` part whose `data` is the audio as a data URL (or the app's link), `asr_options.language` and `stream: false`. The reply's message content is the text, its `audio_info` annotation the language, and `usage.seconds` the billed seconds. Only `json` and `text` are possible: the model returns no timestamps on this API.
