# `openai_transcriptions/providers/mistral`

Mistral's Voxtral Transcribe, `POST /v1/audio/transcriptions`. Nothing here is recorded.

| file | what it covers | provenance |
|---|---|---|
| `responses/json.json` | the one JSON reply with a segment and usage with `prompt_audio_seconds` | doc: https://docs.mistral.ai/studio/audio/speech-to-text/transcribe-files.md (read 2026-11-08), text shortened and one segment from the same page |
| `streams/stream-transcribe.sse` | `transcription.language`, `transcription.text.delta` and `transcription.done` | doc: the same page's stream and https://docs.mistral.ai/openapi.yaml event types; text and usage made up |
