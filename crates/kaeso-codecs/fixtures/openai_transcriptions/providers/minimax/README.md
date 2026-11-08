# `openai_transcriptions/providers/minimax`

MiniMax's `POST /v1/speech_to_text`. Nothing here is recorded.

| file | what it covers | provenance |
|---|---|---|
| `responses/json.json` | `json` with `duration` | doc: https://platform.minimax.io/docs/api-reference/speech/speech-to-text/api/openapi.json example (read 2026-11-08) |
| `responses/verbose.json` | `verbose_json` with speakers | doc: as above |
| `streams/stream-transcribe.sse` | `{index, delta, finish}` events and the end with `duration` | doc: as above |
