---
title: AssemblyAI
description: How KPP's transcription requests map to AssemblyAI's Sync API.
order: 8
---

Look up how a transcription request reaches AssemblyAI here: which field goes where, what Kaeso refuses, the Sync API's limits, and the source of each mapping.

- **Codec:** `kaeso_codecs::assemblyai::AssemblyAi` (`crates/kaeso-codecs/src/assemblyai/`)
- **Path:** `POST /v1/transcribe` on `https://sync.assemblyai.com` (or `https://sync.eu.assemblyai.com`), with the key as `Authorization: Bearer` and the model in `X-AAI-Model`
- **Fixtures:** `crates/kaeso-codecs/fixtures/assemblyai/`, the OpenAPI document's example and replies built from its schema, with a refused key and a request without a model recorded live

**Sources.** AssemblyAI's [Sync API reference](https://www.assemblyai.com/docs/api-reference/sync-api/transcribe) and its OpenAPI document, read 2026-11-14.

## Limits

The Sync API answers in the same request. It takes audio of 80 milliseconds to 20 minutes and up to 256 MB, and answers within 30 seconds, otherwise with 504 `Inference Timeout`, which reaches the app as a timeout. Longer recordings need AssemblyAI's async API, which Kaeso doesn't call.

## Request

| KPP | Sync API |
| --- | --- |
| `file` | the `audio` part, with the upload's content type |
| `model` | the `X-AAI-Model` header (`universal-3-6-pro`) |
| `language` | `config.language_codes`, its primary subtag, for the 32 languages the API steers toward; another language is refused, and without one the API transcribes without steering |
| `prompt` | `config.prompt`, at most 6000 characters; AssemblyAI adds it to the model's instructions |
| `verbose_json`, `srt`, `vtt`, word timestamps | `config.timestamps: true`; Kaeso writes the segments and subtitles from the timed words |

The `config` part is left out when it would be empty. Kaeso refuses `temperature`, segment timestamps, speaker labels (`diarized_json`), translations, streaming and audio by link before sending: the Sync API has no place for them.

## Result

| Sync API | KPP |
| --- | --- |
| `text` | the text |
| `audio_duration_ms` | the audio's length, which the cost is billed by |
| `words[]` `{text, start, end}` in milliseconds | timed words; a word the API couldn't align comes without times, and then the words stay as AssemblyAI wrote them |
| `words[].confidence`, `confidence`, `session_id`, `request_time_ms` | extras |

The Sync API reports no language and no segments.

## Errors

| Sync API | KPP |
| --- | --- |
| `{"status", "title", "detail"}` (problem details) | `detail` as the message, the kind from the status; `Bad Audio`, `Audio Too Short`, `Audio Too Large` and `Unsupported Media Type` name `file` |
| 404 `Invalid API key` | an authentication error: the API answers a refused key with 404, not the documented 401 |
| plain text, such as `Not found` without `X-AAI-Model` | the text, the kind from the status |

## Model check

The Sync API lists no models and has no request that checks a key without transcribing, so **Check availability** answers that it can't check one; the first transcription shows whether the key works.
