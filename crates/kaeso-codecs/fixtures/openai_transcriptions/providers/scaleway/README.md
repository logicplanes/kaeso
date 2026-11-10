# `openai_transcriptions/providers/scaleway`

Scaleway Generative APIs' transcriptions, `POST https://api.scaleway.ai/v1/audio/transcriptions`, with `whisper-large-v3`. Nothing here is recorded.

| file | what it covers | provenance |
|---|---|---|
| `responses/json.json` | `json`, the only format, with `usage` of type `duration` | doc: `CreateAudioTranscriptionResponse` in https://www.scaleway.com/en/developers/api/generative-apis/ (`text`, `usage.type`, `usage.seconds`), read 2026-11-09 |
