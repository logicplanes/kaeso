# `elevenlabs/image`

ElevenLabs' image generations, `POST /v1/flows/image` and `GET /v1/flows/image/{id}`. Nothing here is recorded: every file is built from ElevenLabs' OpenAPI document and its image and video guide (https://elevenlabs.io/docs/eleven-api/guides/cookbooks/image-and-video).

| file | what it covers | provenance |
|---|---|---|
| `requests/gpt-image-edit.json` | a GPT Image edit of an inline image and an asset, with quality, background and aspect ratio | schema: https://api.elevenlabs.io/openapi.json `ImageGenerationRequest`, `MediaGenerationCreateResponse`, `MediaGenerationResponse` and their examples (read 2026-11-09) |
| `requests/gemini.json` | a Gemini 3 Pro image with an aspect ratio and a resolution | schema: https://api.elevenlabs.io/openapi.json `ImageGenerationRequest`, `MediaGenerationCreateResponse`, `MediaGenerationResponse` and their examples (read 2026-11-09) |
| `requests/seedream.json` | a Seedream 5 Pro image with a seed | schema: https://api.elevenlabs.io/openapi.json `ImageGenerationRequest`, `MediaGenerationCreateResponse`, `MediaGenerationResponse` and their examples (read 2026-11-09) |
| `responses/started.json` | the reply that starts a generation | schema: https://api.elevenlabs.io/openapi.json `ImageGenerationRequest`, `MediaGenerationCreateResponse`, `MediaGenerationResponse` and their examples (read 2026-11-09) |
| `polls/generating.json` | a generation still being made | schema: https://api.elevenlabs.io/openapi.json `ImageGenerationRequest`, `MediaGenerationCreateResponse`, `MediaGenerationResponse` and their examples (read 2026-11-09) |
| `polls/completed.json` | a finished generation with its link | schema: https://api.elevenlabs.io/openapi.json `ImageGenerationRequest`, `MediaGenerationCreateResponse`, `MediaGenerationResponse` and their examples (read 2026-11-09) |
| `polls/failed.json` | a generation that timed out | schema: https://api.elevenlabs.io/openapi.json `ImageGenerationRequest`, `MediaGenerationCreateResponse`, `MediaGenerationResponse` and their examples (read 2026-11-09) |
