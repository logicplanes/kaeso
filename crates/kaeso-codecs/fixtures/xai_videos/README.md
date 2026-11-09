# Fixtures: `xai_videos`

SpaceX's video API: `POST /v1/videos/generations`, `POST /v1/videos/edits` and `GET /v1/videos/{request_id}`.

## Sources

- https://docs.x.ai/developers/rest-api-reference/inference/videos and https://docs.x.ai/developers/model-capabilities/video/generation, read 2026-11-09.
- No fixture is recorded: no SpaceX key was available.

The layout and the provenance marks are in [Fixtures and recordings](https://docs.kaeso.app/development/kpp/fixtures). `polls/` holds the provider's answers to a job's status call.

## Files

| file | what it covers | provenance |
|---|---|---|
| `errors/invalid-model-400.json` | an error, with its status in `.meta.json` | doc: SpaceX videos reference |
| `polls/done.json` | a job as the provider reports it | doc: SpaceX videos reference |
| `polls/expired.json` | a job as the provider reports it | doc: SpaceX videos reference |
| `polls/failed.json` | a job as the provider reports it | doc: SpaceX videos reference |
| `polls/moderated.json` | a job as the provider reports it | doc: SpaceX videos reference |
| `polls/pending.json` | a job as the provider reports it | doc: SpaceX videos reference |
| `polls/queued.json` | a job as the provider reports it | doc: SpaceX videos reference |
| `requests/edit.json` | a request | doc: SpaceX videos reference |
| `requests/generate-image.json` | a request | doc: SpaceX videos reference |
| `requests/generate-references.json` | a request | doc: SpaceX videos reference |
| `requests/generate.json` | a request | doc: SpaceX videos reference |
| `responses/started.json` | the reply that starts a job | doc: SpaceX videos reference |
