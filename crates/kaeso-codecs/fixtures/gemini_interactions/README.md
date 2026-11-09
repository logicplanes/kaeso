# Fixtures: `gemini_interactions`

Gemini Omni's video jobs on Google AI Studio's Interactions API: `POST /v1beta/interactions` with `background: true` and `GET /v1beta/interactions/{id}`.

## Sources

- https://aistudio.google.com/docs/omni and https://aistudio.google.com/api-reference/interactions-api/create-interaction, read 2026-11-09.
- `requests/recorded-tiny.json`, `responses/recorded-started.json` and `polls/recorded-failed-quota.json` were recorded on 2026-11-08 with a free-tier Google AI Studio key and billing off: one request, a 3-second 360p video, which the free tier refused as `RESOURCE_EXHAUSTED`. Nothing was spent. The rest is built from the reference.

The layout and the provenance marks are in [Fixtures and recordings](https://docs.kaeso.app/development/kpp/fixtures). `polls/` holds the provider's answers to a job's status call.

## Files

| file | what it covers | provenance |
|---|---|---|
| `errors/quota-429.json` | an error, with its status in `.meta.json` | doc: Gemini Omni reference |
| `polls/completed-uri.json` | a job as the provider reports it | doc: Gemini Omni reference |
| `polls/completed.json` | a job as the provider reports it | doc: Gemini Omni reference |
| `polls/failed.json` | a job as the provider reports it | doc: Gemini Omni reference |
| `polls/in-progress.json` | a job as the provider reports it | doc: Gemini Omni reference |
| `polls/recorded-failed-quota.json` | a job as the provider reports it | recorded 2026-11-08 |
| `requests/first-last-frame.json` | a request | doc: Gemini Omni reference |
| `requests/recorded-tiny.json` | a request | recorded 2026-11-08 |
| `requests/references.json` | a request | doc: Gemini Omni reference |
| `requests/remix.json` | a request | doc: Gemini Omni reference |
| `requests/text-to-video.json` | a request | doc: Gemini Omni reference |
| `responses/recorded-started.json` | the reply that starts a job | recorded 2026-11-08 |
| `responses/started.json` | the reply that starts a job | doc: Gemini Omni reference |
