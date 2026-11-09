# Fixtures: `openai_videos`

OpenAI's Videos API: `POST /v1/videos`, `POST /v1/videos/{id}/remix`, `GET /v1/videos`, `GET /v1/videos/{id}`, `DELETE /v1/videos/{id}`, and the file a job made.

## Sources

- OpenAI OpenAPI spec at commit `926a7b8133226630bac025350e84b0ca5c31d45a` (2026-11-07): `CreateVideoJsonBody`, `CreateVideoMultipartBody`, `ImageRefParam-2`, `CreateVideoRemixBody`, `VideoResource`, `VideoListResource`, `DeletedVideoResource`, `ErrorResponse`, and the `x-oaiMeta` examples of each video path.
- openai-python 3.41.0, whose multipart field names the upload fixture follows.
- No fixture is recorded: OpenAI shut its video models down on 2026-09-24, and no key was available. `content/sample.mp4` is a short clip made for the tests.

The layout and the provenance marks are in [Fixtures and recordings](https://docs.kaeso.app/development/kpp/fixtures). `polls/` holds the provider's answers to a job's status call.

## Files

| file | what it covers | provenance |
|---|---|---|
| `content/sample.mp4` | a made video's file | test clip |
| `errors/invalid-size-400.json` | an error, with its status in `.meta.json` | spec: openai-openapi@926a7b8 |
| `lists/deleted.json` | a listing or a deletion | spec: openai-openapi@926a7b8 |
| `lists/page.json` | a listing or a deletion | spec: openai-openapi@926a7b8 |
| `requests/create-every-field.json` | a request | spec: openai-openapi@926a7b8 |
| `requests/create-file-reference.json` | a request | spec: openai-openapi@926a7b8 |
| `requests/create-upload.multipart` | a request | spec: openai-openapi@926a7b8 |
| `requests/create.json` | a request | spec: openai-openapi@926a7b8 |
| `requests/remix.json` | a request | spec: openai-openapi@926a7b8 |
| `responses/completed.json` | the reply that starts a job | spec: openai-openapi@926a7b8 |
| `responses/failed.json` | the reply that starts a job | spec: openai-openapi@926a7b8 |
| `responses/queued.json` | the reply that starts a job | spec: openai-openapi@926a7b8 |
| `responses/remix-queued.json` | the reply that starts a job | spec: openai-openapi@926a7b8 |
