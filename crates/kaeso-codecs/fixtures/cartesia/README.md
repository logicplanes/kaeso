# Fixtures: `cartesia`

Wire bodies for Cartesia's API at version `2026-08-14` (`Cartesia-Version`): Sonic text to speech (`POST /tts/bytes`, JSON in, audio out) and Ink batch speech to text (`POST /stt`, `multipart/form-data` in, JSON out). No key was available, so every body that only a successful call produces is built from the reference pages; the errors were answered live without a valid key on 2026-11-14.

## Sources (read 2026-11-14)

- `POST /tts/bytes` reference and its OpenAPI excerpt: https://docs.cartesia.ai/api-reference/tts/bytes
- `POST /stt` reference (`TranscriptionResponse`, `TranscriptionWord`): https://docs.cartesia.ai/api-reference/stt/transcribe
- Output formats: https://docs.cartesia.ai/build-with-cartesia/capability-guides/tts-output-audio-format
- Errors: https://docs.cartesia.ai/use-the-api/api-errors

## Files

| File | What it covers | Provenance |
|---|---|---|
| `requests/speech/mp3-built.json` | mp3 at Cartesia's defaults, `language`, a voice id as a string | built from the reference's `TTSRequest`; the voice is the reference's default voice |
| `requests/speech/wav-generation-config-built.json` | wav `pcm_s16le` at 24 kHz, `generation_config` with `speed`, `volume` and `emotion`, `normalization` | built from the reference |
| `requests/speech/raw-pcm-built.json` | raw `pcm_s16le` at 24 kHz, OpenAI's `pcm` | built from the reference |
| `responses/stt/words-built.json` | a transcript with `language`, `duration` and `words` | built from `TranscriptionResponse`; words carry a leading space as Cartesia's cookbook says |
| `responses/stt/text-only-built.json` | the required fields only | built from `TranscriptionResponse` |
| `errors/recorded-no-credentials-401.json` | `POST /tts/bytes` without a key | live; request id zeroed |
| `errors/recorded-invalid-credentials-401.json` | `POST /stt` with a made-up key | live; request id zeroed |
| `errors/recorded-voices-not-logged-in-401.json` | `GET /voices` with a made-up key: no `error_code` | live; request id zeroed |
| `errors/voice-not-found-404-built.json`, `errors/concurrency-limited-429-built.json` | `error_code` values of the errors page | built from the errors page's shape and codes |
| `plain/recorded-no-version-400.txt` | a request without `Cartesia-Version`, answered as plain text | live |
