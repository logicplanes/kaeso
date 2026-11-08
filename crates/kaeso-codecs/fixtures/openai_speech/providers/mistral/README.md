# `openai_speech/providers/mistral`

Mistral's Voxtral TTS, `POST /v1/audio/speech`. Nothing here is recorded.

| file | what it covers | provenance |
|---|---|---|
| `requests/speech.json` | `voice_id` and `mp3` | doc: https://docs.mistral.ai/studio/audio/text-to-speech/generate-speech.md (read 2026-11-08) |
| `responses/speech.json` | the JSON reply with base64 `audio_data` | schema: https://docs.mistral.ai/openapi.yaml `SpeechResponse`; the audio is a few made-up MP3 bytes |
| `streams/stream-speech.sse` | `speech.audio.delta` with `audio_data`, then `speech.audio.done` with `UsageInfo` | doc: https://docs.mistral.ai/studio/audio/text-to-speech/stream-speech.md and the OpenAPI events; the usage numbers are made up |
