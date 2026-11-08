# `openai_speech/providers/vercel`

Vercel AI Gateway's speech model protocol, `POST /v4/ai/speech-model` with the model in `ai-model-id`. Nothing here is recorded.

| file | what it covers | provenance |
|---|---|---|
| `requests/speech.json` | `text`, `voice` and `outputFormat` for `openai/tts-1` | doc: https://vercel.com/docs/ai-gateway/modalities/text-to-speech.md (read 2026-11-08) |
| `responses/speech.json` | base64 `audio` and `warnings` | doc: as above |
