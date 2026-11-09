# `elevenlabs/video`

ElevenLabs' video generations, `POST /v1/flows/video` and `GET /v1/flows/video/{id}`. Nothing here is recorded: video generation is a paid feature the free tier doesn't call.

| file | what it covers | provenance |
|---|---|---|
| `errors/quota-401.json` | an error | schema: https://api.elevenlabs.io/openapi.json `VideoGenerationRequest`, `MediaGenerationResponse` (read 2026-11-08) |
| `polls/completed.json` | a job as ElevenLabs reports it | schema: https://api.elevenlabs.io/openapi.json `VideoGenerationRequest`, `MediaGenerationResponse` (read 2026-11-08) |
| `polls/failed.json` | a job as ElevenLabs reports it | schema: https://api.elevenlabs.io/openapi.json `VideoGenerationRequest`, `MediaGenerationResponse` (read 2026-11-08) |
| `polls/generating.json` | a job as ElevenLabs reports it | schema: https://api.elevenlabs.io/openapi.json `VideoGenerationRequest`, `MediaGenerationResponse` (read 2026-11-08) |
| `requests/seedance.json` | a request | schema: https://api.elevenlabs.io/openapi.json `VideoGenerationRequest`, `MediaGenerationResponse` (read 2026-11-08) |
| `requests/veo-frames.json` | a request | schema: https://api.elevenlabs.io/openapi.json `VideoGenerationRequest`, `MediaGenerationResponse` (read 2026-11-08) |
| `requests/veo-references.json` | a request | schema: https://api.elevenlabs.io/openapi.json `VideoGenerationRequest`, `MediaGenerationResponse` (read 2026-11-08) |
| `requests/veo.json` | a request | schema: https://api.elevenlabs.io/openapi.json `VideoGenerationRequest`, `MediaGenerationResponse` (read 2026-11-08) |
| `responses/started.json` | the reply that starts a job | schema: https://api.elevenlabs.io/openapi.json `VideoGenerationRequest`, `MediaGenerationResponse` (read 2026-11-08) |
