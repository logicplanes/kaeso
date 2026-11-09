---
title: MiniMax Speech
description: How KPP's speech requests map to MiniMax's synchronous text-to-speech API.
order: 8
---

Look up how a speech request reaches MiniMax here: which field goes where, what is refused, and the source of each mapping.

- **Codec:** `kaeso_codecs::minimax_speech::MinimaxSpeech` (`crates/kaeso-codecs/src/minimax_speech/`)
- **Path:** `POST /v1/t2a_v2` (T2A v2), from the root of MiniMax's host, with `Authorization: Bearer`
- **Fixtures:** `crates/kaeso-codecs/fixtures/minimax_speech/`, built from MiniMax's OpenAPI document, none recorded

**Sources.** MiniMax's [synchronous text to speech](https://platform.minimax.io/docs/api-reference/speech-t2a-http) reference and its [OpenAPI document](https://platform.minimax.io/docs/api-reference/speech/t2a/api/openapi.json), and its [error codes](https://platform.minimax.io/docs/api-reference/errorcode), read 2026-11-08.

## Decisions

- **Whole replies.** MiniMax's stream framing isn't documented, so Kaeso always sends `stream: false` and reads the whole reply. For an app that streams, the reply becomes one piece of audio and the end, with the usage and the billed characters.
- **Hex audio.** Kaeso asks for `output_format: hex`, so the audio comes inside the reply and not as a link. The hex becomes base64 in KPP.
- **Characters from the reply.** `extra_info.usage_characters` is what MiniMax bills, and becomes `SpeechResponse::characters` for the cost.
- **Errors can come with HTTP 200.** A failing `base_resp` is an error whatever the status, mapped by its code as for [MiniMax Images](minimax-images.md#errors).

## Request

| KPP | MiniMax | Notes |
| --- | --- | --- |
| `model` | `model` | |
| `input` | `text` | |
| `voice` | `voice_setting.voice_id` | A name or an id goes as written: MiniMax's voices depend on the model. |
| `speed` | `voice_setting.speed` | An integer stays one. |
| `format` | `audio_setting.format`: `mp3`, `wav`, `flac`, `opus`, and `pcm` for `Pcm16` | |
| `sample_rate` | `audio_setting.sample_rate` | `Pcm16` without a rate gets `24000`, since KPP's `pcm16` is 24 kHz. |
| `bitrate` (kilobits) | `audio_setting.bitrate` (bits per second) | |
| `stream` | `stream: false` | Kaeso never asks MiniMax to stream. |
| — | `output_format: hex` | Sent unless a MiniMax-format request named another. |
| other keys of `voice_setting` and `audio_setting`, `pronunciation_dict`, `voice_modify`, `language_boost` and other keys | extras | Sent only from a MiniMax-format request; other dialects refuse them. |

## Refused

Each of these is `Unsupported` before anything is sent, with the field in `param`:

| KPP | `param` | Why |
| --- | --- | --- |
| an operation other than `Speech`: dialogue, sound effects, music, the voice changer | `model` | T2A makes speech from text only. |
| dialogue lines or source audio | `input` | |
| a format other than mp3, pcm, flac, wav or opus | `response_format` | Kaeso never converts audio. |
| `instructions`, `language`, `seed`, timestamps, `duration` | the field | T2A v2 has no such fields Kaeso maps. |

## Reply

| MiniMax | KPP |
| --- | --- |
| `data.audio`, hex | the audio as base64; hex that doesn't decode is a `ProviderError` |
| `extra_info.audio_format` | the audio's media type (`audio/mpeg`, `audio/wav`, `audio/flac`, `audio/opus`, `audio/pcm`); `mp3` when missing |
| `extra_info.usage_characters` | `characters` |
| the rest of `data`, `extra_info`, `base_resp` | extras |

To a MiniMax-format app, the codec writes the audio as hex in `data.audio`, with `base_resp` `0`.

## Fixtures

| File | Covers | Built from |
| --- | --- | --- |
| `requests/speech.json` | Voice and audio settings, `pronunciation_dict`, `voice_modify`, `language_boost`, hex output | The OpenAPI document's example "Request(Non-streaming)" |
| `responses/speech.json` | Hex audio, `extra_info` with `usage_characters`, `base_resp` | The same document's reply example; the hex is ten made-up MP3 bytes |
| `errors/invalid-params-200.json` | A failing `base_resp` with HTTP 200 | The schema `T2aV2Resp.base_resp` and the error codes; the message is made up |

No MiniMax key was available, so nothing is recorded. The folder's `README.md` lists the provenance.
