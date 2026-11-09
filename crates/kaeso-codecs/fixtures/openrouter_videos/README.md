# Fixtures: `openrouter_videos`

OpenRouter's video generation API: `POST /api/v1/videos` and `GET /api/v1/videos/{jobId}`.

## Sources

- OpenRouter's OpenAPI document as https://openrouter.ai/docs/api/api-reference/video-generation/ shows it, read 2026-11-09.
- No fixture is recorded: no OpenRouter key was available.

The layout and the provenance marks are in [Fixtures and recordings](https://docs.kaeso.app/development/kpp/fixtures). `polls/` holds the provider's answers to a job's status call.

## Files

| file | what it covers | provenance |
|---|---|---|
| `errors/bad-request-400.json` | an error, with its status in `.meta.json` | schema: OpenRouter video OpenAPI |
| `errors/no-credits-402.json` | an error, with its status in `.meta.json` | schema: OpenRouter video OpenAPI |
| `polls/completed-signed.json` | a job as the provider reports it | schema: OpenRouter video OpenAPI |
| `polls/completed.json` | a job as the provider reports it | schema: OpenRouter video OpenAPI |
| `polls/expired.json` | a job as the provider reports it | schema: OpenRouter video OpenAPI |
| `polls/failed.json` | a job as the provider reports it | schema: OpenRouter video OpenAPI |
| `polls/in-progress.json` | a job as the provider reports it | schema: OpenRouter video OpenAPI |
| `requests/audio-reference.json` | a request | schema: OpenRouter video OpenAPI |
| `requests/frames-and-references.json` | a request | schema: OpenRouter video OpenAPI |
| `requests/text-to-video.json` | a request | schema: OpenRouter video OpenAPI |
| `responses/started.json` | the reply that starts a job | schema: OpenRouter video OpenAPI |
