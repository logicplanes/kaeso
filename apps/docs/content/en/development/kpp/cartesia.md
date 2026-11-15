---
title: Cartesia
description: How KPP's speech and transcription requests map to Cartesia's API.
order: 8
---

Look up how a speech or transcription request reaches Cartesia here: which field goes where, what Kaeso refuses, and the source of each mapping.

- **Codec:** `kaeso_codecs::cartesia::Cartesia` (`crates/kaeso-codecs/src/cartesia/`)
- **Paths:** with `X-API-Key` and `Cartesia-Version: 2026-08-14`: `POST /tts/bytes` (JSON in, audio out), `POST /stt` (an upload in, JSON out), `GET /voices?limit=1` for the model check
- **Fixtures:** `crates/kaeso-codecs/fixtures/cartesia/`, built from the reference pages, with the refused keys and a request without the version recorded live

**Sources.** Cartesia's [API conventions](https://docs.cartesia.ai/use-the-api/api-conventions), the [TTS bytes](https://docs.cartesia.ai/api-reference/tts/bytes) and [STT](https://docs.cartesia.ai/api-reference/stt/transcribe) references, [output formats](https://docs.cartesia.ai/build-with-cartesia/capability-guides/tts-output-audio-format), [errors](https://docs.cartesia.ai/use-the-api/api-errors) and its [Whisper migration guide](https://www.cartesia.ai/migrate/openai-whisper), read 2026-11-14.

## Version

Every request carries `Cartesia-Version: 2026-08-14`, the version these mappings follow and the one Cartesia names as its latest. Cartesia keeps a version's fields stable; a new version means a new Kaeso release that moves the constant `kaeso_codecs::cartesia::VERSION`.

## Speech request

| KPP | Cartesia `/tts/bytes` |
| --- | --- |
| `model` | `model_id` |
| `input` | `transcript` |
| `voice` | `voice`: the voice id as a string |
| `response_format` `mp3` | `output_format` `{"container": "mp3", "sample_rate": 44100, "bit_rate": 128000}` |
| `response_format` `wav` | `{"container": "wav", "encoding": "pcm_s16le", "sample_rate": 24000}` |
| `response_format` `pcm` | `{"container": "raw", "encoding": "pcm_s16le", "sample_rate": 24000}`, OpenAI's 24 kHz 16-bit PCM |
| `sample_rate`, `bitrate` | `sample_rate` (8000 to 48000 as listed), mp3 `bit_rate` |
| `speed` | `generation_config.speed`, 0.6 to 1.5 |
| `language` | `language` |

Kaeso refuses `opus`, `aac` and `flac`, which Cartesia doesn't make, `instructions`, `seed`, timestamps and a request without a voice before sending. The audio streams back as Cartesia makes it. A voice Cartesia doesn't know answers `voice_not_found`, which reaches the app as a not-found error on `voice`.

## Transcription request

| KPP | Cartesia `/stt` |
| --- | --- |
| `file` | `file` |
| `model` | `model` |
| `language` | `language`, required: the batch API assumes English and doesn't detect |
| `verbose_json`, `srt`, `vtt`, word timestamps | `timestamp_granularities[]=word`; Kaeso writes the segments and subtitles from the words |

Kaeso refuses `prompt`, `temperature`, segment timestamps, speaker labels (`diarized_json`), translations, streaming and audio by link before sending.

## Transcription result

| Cartesia | KPP |
| --- | --- |
| `text` | the text |
| `language`, `duration` | the language and the audio's length, which the cost is billed by |
| `words[]` `{word, start, end}` | timed words, a leading space kept |
| `type`, `request_id` | extras |

## Errors

| Cartesia | KPP |
| --- | --- |
| `{"error_code", "title", "message", "request_id"}` | `message`; `voice_not_found` is not found on `voice`, `model_not_found` and `model_sunsetted` on `model`, `quota_exceeded` billing, `concurrency_limited` rate limited, `file_too_large` too large, otherwise the kind from the status |
| plain text, such as a missing `Cartesia-Version` | the text, the kind from the status |

## Model check

Cartesia lists no models, so **Check availability** reads `GET /voices?limit=1`, which shows only that the key works.
