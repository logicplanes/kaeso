# `openai_transcriptions/providers/groq`

Groq's Whisper transcriptions, `POST /openai/v1/audio/transcriptions`. Nothing here is recorded.

| file | what it covers | provenance |
|---|---|---|
| `responses/json.json` | `json` with `x_groq` | doc: https://console.groq.com/docs/api-reference.md "Create transcription" example (read 2026-11-08) |
| `responses/verbose.json` | `verbose_json` with one segment | doc: https://console.groq.com/docs/speech-to-text.md segment example; the envelope after OpenAI's `verbose_json` |
