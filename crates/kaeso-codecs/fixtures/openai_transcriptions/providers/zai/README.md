# `openai_transcriptions/providers/zai`

Z.ai's GLM-ASR, `POST /api/paas/v4/audio/transcriptions`. Nothing here is recorded.

| file | what it covers | provenance |
|---|---|---|
| `responses/json.json` | `AudioTranscriptionResponse`: id, created, request_id, model, text | schema: https://docs.z.ai/openapi.json (read 2026-11-08); Z.ai publishes no example, so every value is made up |
| `streams/stream-transcribe.sse` | `AudioTranscriptionStreamResponse` deltas and the end without `text` | schema: as above; values made up |
