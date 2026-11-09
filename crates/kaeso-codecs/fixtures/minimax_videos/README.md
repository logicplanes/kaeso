# Fixtures: `minimax_videos`

MiniMax's Hailuo (`/v1/video_generation`, `/v1/query/video_generation`, `/v1/files/retrieve`) and H3 (`/v2/video_generation`, `/v2/query/video_generation/{task_id}`) video APIs.

## Sources

- MiniMax's OpenAPI documents at https://platform.minimax.io/docs/api-reference/video/generation/api/text-to-video.json, `image-to-video.json`, `start-end-to-video.json`, `openapi.json` and `v2-video-generation.json`, read 2026-11-09.
- No fixture is recorded: no MiniMax key was available.

The layout and the provenance marks are in [Fixtures and recordings](https://docs.kaeso.app/development/kpp/fixtures). `polls/` holds the provider's answers to a job's status call.

## Files

| file | what it covers | provenance |
|---|---|---|
| `errors/h3-auth-401.json` | an error, with its status in `.meta.json` | schema: MiniMax video OpenAPI |
| `errors/sensitive-input-200.json` | an error, with its status in `.meta.json` | schema: MiniMax video OpenAPI |
| `files/retrieved.json` | a file id turned into a link | schema: MiniMax video OpenAPI |
| `polls/h3-cancelled.json` | a job as the provider reports it | schema: MiniMax video OpenAPI |
| `polls/h3-failed.json` | a job as the provider reports it | schema: MiniMax video OpenAPI |
| `polls/h3-running.json` | a job as the provider reports it | schema: MiniMax video OpenAPI |
| `polls/h3-succeeded.json` | a job as the provider reports it | schema: MiniMax video OpenAPI |
| `polls/hailuo-fail.json` | a job as the provider reports it | schema: MiniMax video OpenAPI |
| `polls/hailuo-preparing.json` | a job as the provider reports it | schema: MiniMax video OpenAPI |
| `polls/hailuo-processing.json` | a job as the provider reports it | schema: MiniMax video OpenAPI |
| `polls/hailuo-queueing.json` | a job as the provider reports it | schema: MiniMax video OpenAPI |
| `polls/hailuo-success.json` | a job as the provider reports it | schema: MiniMax video OpenAPI |
| `requests/h3-frames.json` | a request | schema: MiniMax video OpenAPI |
| `requests/h3-text.json` | a request | schema: MiniMax video OpenAPI |
| `requests/hailuo-first-frame.json` | a request | schema: MiniMax video OpenAPI |
| `requests/hailuo-first-last.json` | a request | schema: MiniMax video OpenAPI |
| `requests/hailuo-text.json` | a request | schema: MiniMax video OpenAPI |
| `responses/h3-started.json` | the reply that starts a job | schema: MiniMax video OpenAPI |
| `responses/hailuo-started.json` | the reply that starts a job | schema: MiniMax video OpenAPI |
