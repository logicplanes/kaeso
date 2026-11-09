# Fixtures: `zai_videos`

Z.ai's CogVideoX API: `POST /api/paas/v4/videos/generations` and `GET /api/paas/v4/async-result/{id}`.

## Sources

- https://docs.z.ai/api-reference/video/generate-video and https://docs.z.ai/api-reference/video/get-video-status, read 2026-11-09.
- No fixture is recorded: no Z.ai key was available.

The layout and the provenance marks are in [Fixtures and recordings](https://docs.kaeso.app/development/kpp/fixtures). `polls/` holds the provider's answers to a job's status call.

## Files

| file | what it covers | provenance |
|---|---|---|
| `errors/insufficient-balance-429.json` | an error, with its status in `.meta.json` | doc: Z.ai video reference |
| `polls/fail.json` | a job as the provider reports it | doc: Z.ai video reference |
| `polls/processing.json` | a job as the provider reports it | doc: Z.ai video reference |
| `polls/success.json` | a job as the provider reports it | doc: Z.ai video reference |
| `requests/first-last-frame.json` | a request | doc: Z.ai video reference |
| `requests/image-to-video.json` | a request | doc: Z.ai video reference |
| `requests/text-to-video.json` | a request | doc: Z.ai video reference |
| `responses/started.json` | the reply that starts a job | doc: Z.ai video reference |
