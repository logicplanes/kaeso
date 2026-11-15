---
title: Deepgram
description: How KPP's transcriptions and speech map to Deepgram's speech-to-text and Aura text-to-speech APIs.
order: 8
---

Look up how a transcription or a speech request reaches Deepgram here: which field goes where, what is refused, and the source of each mapping.

- **Codec:** `kaeso_codecs::deepgram::Deepgram` (`crates/kaeso-codecs/src/deepgram/`)
- **Paths:** `POST /v1/listen` (pre-recorded audio) and `POST /v1/speak` (Aura), every option in the query string, with `Authorization: Token`
- **Fixtures:** `crates/kaeso-codecs/fixtures/deepgram/`; the errors and the model listing recorded live, the replies from Deepgram's documentation

**Sources.** Deepgram's [pre-recorded audio](https://developers.deepgram.com/docs/pre-recorded-audio), [text to speech](https://developers.deepgram.com/docs/text-to-speech), [media output settings](https://developers.deepgram.com/docs/tts-media-output-settings), [encoding](https://developers.deepgram.com/docs/tts-encoding), [language detection](https://developers.deepgram.com/docs/language-detection), [utterances](https://developers.deepgram.com/docs/utterances) and [errors](https://developers.deepgram.com/docs/errors) pages and the OpenAPI document of `deepgram/deepgram-api-specs` at commit `0ccc6f4`, read 2026-11-14.

## Decisions

- **The language is detected when the app names none.** OpenAI detects the language; Deepgram assumes English. Kaeso sends `detect_language=true` then, which picks one language and bills the monolingual price, never `language=multi`.
- **Smart formatting, always.** Kaeso sends `smart_format=true`, which Deepgram includes at no cost, so the transcript is punctuated and capitalised as OpenAI's is.
- **Every transcript form comes from one JSON reply.** Deepgram answers JSON only. For `srt`, `vtt`, `verbose_json` and `diarized_json`, Kaeso asks for `utterances=true` and writes the form from the utterances: one subtitle or segment per utterance, with its speaker for `diarized_json`.
- **The voice is the model.** Deepgram names each Aura voice as a model, `aura-2-thalia-en`. Kaeso's models are the families `aura-2` and `aura`, and `voice` names the voice: `thalia` (English unless the request names a language), `thalia-en`, or the whole id.
- **Whole transcripts, streamed audio.** A transcript is answered whole; an app that streams gets it as one piece and the end. Speech comes back as Deepgram sends it, piece by piece.
- **Characters from the header.** `dg-char-count` is what Deepgram bills, and becomes the request's characters for the cost. A transcript's `metadata.duration`, times its channels, is the billed length.
- **Deepgram's defaults stay.** Kaeso sends no `mip_opt_out`, so the account's Model Improvement Program setting applies, and no `callback`, `redact`, `keyterm` or other paid add-on.

## Transcription request

| KPP | Deepgram | Notes |
| --- | --- | --- |
| `model` | `?model=` | |
| `audio`, uploaded | the body, with the file's media type as `content-type` | `application/octet-stream` when the upload names none; Deepgram reads the format from the bytes. |
| `audio`, a link | `{"url": …}` as JSON | |
| `language` | `?language=` | Without it: `?detect_language=true`. |
| `format` that needs timing, `timestamps: [segment]` | `?utterances=true` | Words are always timed. |
| `diarize`, or `format: diarized_json` | `?diarize_model=latest` | `diarize` (without `_model`) is deprecated. |
| — | `?smart_format=true` | |
| query parameters of a Deepgram-format request | the same parameters, in order | Other dialects refuse them. |

## Speech request

| KPP | Deepgram | Notes |
| --- | --- | --- |
| `model` and `voice` | `?model=` | `aura-2` with `thalia` is `aura-2-thalia-en`; a model that names its voice goes as it is. |
| `input` | `{"text": …}` | At most 2,000 characters. |
| `format` | `?encoding=`: `mp3`, `opus`, `aac`, `flac`; `wav` is `linear16` with `container=wav`; `pcm16` is `linear16` with `container=none` and `sample_rate=24000` | Deepgram's `linear16` is 16-bit little-endian PCM. Without a format, `mp3`. |
| `sample_rate` | `?sample_rate=` | |
| `bitrate` (kilobits) | `?bit_rate=` (bits per second) | |
| `speed` | `?speed=` | 0.7 to 1.5. |

## Refused

Each of these is `Unsupported`, or an invalid request, before anything is sent, with the field in `param`:

| KPP | `param` | Why |
| --- | --- | --- |
| translations | `model` | Deepgram transcribes in the language spoken. |
| `prompt`, `temperature`, `num_speakers` | the field | Deepgram has no counterpart. |
| a file id | `file` | |
| a speech operation other than `Speech`, dialogue or source audio | `model`, `input` | |
| `instructions`, `seed`, timestamps, `duration` | the field | |
| a format other than mp3, opus, aac, flac, wav or pcm | `response_format` | |
| `speed` outside 0.7 to 1.5 | `speed` | |
| text over 2,000 characters | `input` | Deepgram answers 413 above it. |
| no voice with `aura-2` or `aura`, a voice of another family, a voice with characters other than lowercase letters, digits and hyphens | `voice` | |

## Transcription reply

| Deepgram | KPP |
| --- | --- |
| `results.channels[0].alternatives[0].transcript` | `text` |
| its `words[]`: `punctuated_word` or `word`, `start`, `end`, `speaker` | `words` |
| `results.utterances[]`: `transcript`, `start`, `end`, `speaker`, `id` | `segments` |
| `results.channels[0].detected_language` | `language` |
| `metadata.duration` | `duration`; times `metadata.channels` for the billed length |
| the rest of `metadata`, the channel, the alternative, `results` and the utterances | extras |

A reply with more than one channel or alternative is a `ProviderError`: Kaeso asks for neither.

## Speech reply

The audio itself, with its `content-type`; `audio/mpeg` when it has none. `dg-char-count` is the billed characters.

## Errors

| Deepgram | KPP |
| --- | --- |
| `INVALID_AUTH`, 401 | `Authentication` |
| `INSUFFICIENT_PERMISSIONS`, 403 | `PermissionDenied` |
| `ASR_PAYMENT_REQUIRED`, 402 | `Billing` |
| `Payload Too Large`, `PAYLOAD_TOO_LARGE`, 413 | `RequestTooLarge` |
| `TOO_MANY_REQUESTS`, `Too Many Requests`, 429 | `RateLimited` |
| `Bad Request` and other 400s, 415, 422 | `InvalidRequest` |
| 404 | `NotFound` |
| 503 | `Overloaded` |
| 504 | `Timeout` |

The message is `err_msg`, or `message` and `details`, or a plain-text body as it is. Deepgram's own errors pass back to a Deepgram-format app as they came.

## Fixtures

The folder's `README.md` lists every file, its provenance and where Deepgram differs from OpenAI's audio APIs. No successful reply is recorded: no Deepgram key was available.
