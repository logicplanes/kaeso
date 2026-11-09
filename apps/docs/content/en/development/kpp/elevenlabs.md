---
title: ElevenLabs
description: How ElevenLabs' API maps to KPP, for ElevenLabs-format apps and for ElevenLabs as a provider.
order: 9
---

Look up how ElevenLabs' generation, transcription, video and listing endpoints map to KPP, as Kaeso reads them from ElevenLabs-format apps and writes them to ElevenLabs. Check it before you change the ElevenLabs codec.

- **Codec:** `kaeso_codecs::elevenlabs::ElevenLabs` (`crates/kaeso-codecs/src/elevenlabs/`)
- **Fixtures:** `crates/kaeso-codecs/fixtures/elevenlabs/`, recorded live once per endpoint

**Sources.** ElevenLabs' OpenAPI document, [api.elevenlabs.io/openapi.json](https://api.elevenlabs.io/openapi.json) (read 2026-11-08), its [errors](https://elevenlabs.io/docs/eleven-api/resources/errors) and [cost headers](https://elevenlabs.io/docs/api-reference/introduction), and the official Python SDK's paths (each operation's `x-fern-sdk-method-name`).

## Decisions

- **The path names the call.** `ElevenLabsCall::from_path` reads the operation, the voice id and the `/stream` and `/with-timestamps` forms; apps' requests are decoded with `dispatch::decode_elevenlabs_request`. Generation becomes `Request::Speech` (`Speech`, `Dialogue`, `SoundEffect`, `Music`, `VoiceChange`), speech to text `Request::Transcription`.
- **Voice ids, never guessed.** ElevenLabs takes voices by id. A `Voice::Id`, or a name of twenty letters and digits as ElevenLabs' ids are, goes as the id; any other name, such as OpenAI's `alloy`, is `Unsupported` with a hint to `GET /v1/voices`.
- **Audio flows as it is made.** For an app of another format, Kaeso calls the `/stream` path, which sends the audio as it is made, except for WAV, which only the non-streaming path takes. `/with-timestamps` answers JSON, read whole (`EgressCtx::speech_reply_json`); `/stream/with-timestamps` answers a line of JSON per piece, read as `SpeechEvent`s with their alignment.
- **Characters from the header.** ElevenLabs bills speech per character and names the characters in its `character-cost` header; `elevenlabs::billed_characters` reads it for the cost.
- **Default models.** A request without `model_id` gets ElevenLabs' documented default for the operation (`eleven_multilingual_v2`, `eleven_v3` for dialogue, `eleven_text_to_sound_v2`, `music_v1`, `eleven_english_sts_v2`) so that Kaeso can resolve it; the marker `default_model` keeps it out of the request ElevenLabs gets.
- **Query parameters cross only to ElevenLabs.** `output_format` is typed; `enable_logging` and `optimize_streaming_latency` are extras, so another provider refuses them instead of losing a zero-retention setting.

## Requests

| ElevenLabs | KPP | Class |
| --- | --- | --- |
| path `voice_id` | `voice: Voice::Id`; an id that isn't letters and digits is `InvalidRequest` with `param: voice_id`, before any credential is looked up | typed |
| query `output_format`, such as `mp3_44100_128`, `pcm_24000`, `opus_48000_64`, `wav_16000`, `ulaw_8000` | `format`, `sample_rate`, `bitrate`; `pcm_24000` is `Pcm16`; the value as written | typed, form |
| other query parameters | the field `query` | extras |
| `text` (speech, sound effects), `prompt` (music) | `input` | typed |
| `inputs[]` `{text, voice_id}` | `dialogue` lines; a malformed `voice_id` is `InvalidRequest` with `param: inputs[i].voice_id` | typed |
| `model_id` | `model` | typed |
| `language_code` | `language` | typed |
| `seed` | `seed` | typed |
| `voice_settings.speed` | `speed` | typed |
| `duration_seconds`, `music_length_ms` | `duration` in seconds | typed |
| `/with-timestamps` | `timestamps` | typed |
| `/stream` | the marker `stream_path`; with timestamps `stream: On` | form, typed |
| the rest of `voice_settings`, `pronunciation_dictionary_locators`, `previous_text`, `apply_text_normalization`, `audio_effects`, … | the field | extras |
| voice changer upload `audio`, `seed`; fields | `audio`, `seed`; the rest extras | typed, extras |
| speech to text upload `file`, or `source_url` / `cloud_storage_url` | `audio` | typed |
| `language_code`, `diarize`, `num_speakers`, `temperature` | `language`, `diarize`, `speakers`, `temperature` | typed |
| `timestamps_granularity` `word` (the default) or `none` | `format` `verbose_json` or `json`; `word` also `timestamps`; `character` the marker `timestamps_granularity` | typed, form |
| `tag_audio_events`, `additional_formats`, `keyterms`, `entity_detection`, `webhook`, … | the field | extras |

From another format, `mp3`, `opus`, `wav`, `pcm16`, μ-law and A-law become an `output_format` ElevenLabs lists, with rates every plan offers when the request names none: 44.1 kHz at 128 kbps for MP3, 48 kHz at 64 kbps for Opus, 24 kHz for WAV and PCM. `aac`, `flac` and rates ElevenLabs doesn't list are `Unsupported`. `speed` goes to `voice_settings.speed`; `instructions` is `Unsupported`.

## Replies

| ElevenLabs | KPP |
| --- | --- |
| audio bytes with `content-type` and `character-cost` | `SpeechResponse::audio`, `characters`; pieces as `SpeechEvent`s |
| `audio_base64`, `alignment {characters, character_start_times_seconds, character_end_times_seconds}`, `normalized_alignment` | `audio`, `alignment: SpeechAlignment`; `normalized_alignment` extras |
| a line of `/stream/with-timestamps` | a `SpeechEvent` with its alignment |
| `text`, `language_code`, `words[]` `{text, start, end, type, speaker_id, logprob}`, `audio_duration_secs` | `text`, `language`, `words` with `speaker` (`type`, `logprob` and `characters` extras), `duration` and `usage_seconds`; the rest extras |
| a transcript per channel (`transcripts`) | refused: one transcript has no place for several channels |

An OpenAI app gets ElevenLabs' words without its `spacing` and `audio_event` entries, which aren't words. An ElevenLabs app gets another provider's transcript with `language_probability` 1 and each word as `type: word`.

## Errors

ElevenLabs answers `{"detail": {"type", "code", "message", "status", "request_id", "param"}}`; older endpoints `{"detail": {"status", "message"}}`, validation `{"detail": [{"loc", "msg"}]}`. Kaeso reads all three, maps `type` and `code` to the error kind (`insufficient_credits` and `payment_required` to billing, `rate_limit_error` to rate limited) and writes its own errors to ElevenLabs apps in the documented shape. ElevenLabs' own errors pass on as they came, with their status.

## Video generations

ElevenLabs makes video with Veo 3.1 and Seedance 2 models through `POST /v1/flows/video`, reported by `GET /v1/flows/video/{id}`; the body depends on `model_id` (`VideoGenerationRequest`). Kaeso sends video jobs there for an app of another format; ElevenLabs-format apps don't call it through Kaeso. The fixtures in `fixtures/elevenlabs/video/` are built from the OpenAPI document.

| KPP | ElevenLabs |
| --- | --- |
| `model` | `model_id`: `veo-3.1-generate-001`, `veo-3.1-fast-generate-001`, `bytedance-seedance-v2`, `-v2-fast`, `-v2-mini`, `-v2.5` |
| `prompt`, `seed`, `audio` | `prompt`, `seed`, `generate_audio` |
| `negative_prompt` | `negative_prompt`, Veo models only |
| `seconds`, `aspect_ratio`, `resolution` | `duration_secs`, `aspect_ratio`, `resolution`, each from the model's lists |
| `FirstFrame`, `LastFrame` images | `start_frame`, `end_frame`: `{type: "inline_base64", content_base64, mime_type}` or `{type: "asset", asset_id}` |
| `Reference` images | `images[]` `{image, role}`, Veo models only, `role` `subject` unless the app named `style` |
| reply `status` `pending`, `generating`, `completed`, `failed` | `queued`, `in_progress`, `completed`, `failed` |
| reply `content_url`, `content_mime_type` | the output link and its media type, valid for about an hour |
| reply `failure_reason`, `error_message` | `error.code`, `error.message` |

Images by web link, `creatify-aurora` (it needs audio input) and remixes are refused before sending.

## Listings

`GET /v1/models` lists the project's ElevenLabs models in ElevenLabs' `ModelResponseModel` shape, `model_id` and `name`. The voice listings are the account's own at ElevenLabs: `voice_listing_request` builds `GET /v1/voices`, `/v2/voices` or `/v1/voices/{voice_id}` with the app's query, and `voices_reply` checks that the answer is a voice list or a voice before it passes on.

## Realtime sockets

`elevenlabs::realtime` reads ElevenLabs' realtime WebSockets without I/O, for the gateway's session proxy. These messages aren't translated: an ElevenLabs socket reaches only ElevenLabs, so no KPP type stands between them.

| Function | What it does |
| --- | --- |
| `RealtimeApi::from_path`, `path` | Names the socket of `/v1/text-to-speech/{voice_id}/stream-input` and `/multi-stream-input`, `/v1/text-to-dialogue/stream-input` and `/multi-stream-input`, and `/v1/speech-to-text/realtime`, and writes its path at ElevenLabs with the voice as one checked segment |
| `read_client_message` | Takes `xi_api_key`, `authorization` and `single_use_token` out of an app's message and counts what it carries: the characters of `text` or of each `inputs[].text`, or the bytes of `audio_base_64` and its `sample_rate` |
| `bytes_per_second` | The bytes one second of `pcm_*` (16-bit) or `ulaw_8000` audio takes |
| `error_message` | The socket's own error message, `{"message_type", "error"}` for speech to text and `{"message", "error", "code", "param"}` otherwise, and the close code: 1008 for what the app can fix, 1011 otherwise |

Sources: ElevenLabs' [WebSocket references](https://elevenlabs.io/docs/websockets) and [text to dialogue WebSocket](https://elevenlabs.io/docs/api-reference/text-to-dialogue/ttd-websocket), read 2026-11-09, and the official `elevenlabs` Python SDK 2.77.0 (`realtime_tts.py`, `realtime/`).
