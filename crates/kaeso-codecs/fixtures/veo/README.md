# Fixtures: `veo`

Veo on Gemini Enterprise Agent Platform: `{model}:predictLongRunning` and `{model}:fetchPredictOperation`.

## Sources

- https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/video/generate-videos-from-text, `VideoGenerationModelInstance` and `VideoGenerationModelParams`, read 2026-11-09.
- No fixture is recorded: Kaeso never calls the platform live. Video bytes are a few bytes standing for an MP4.

The layout and the provenance marks are in [Fixtures and recordings](https://docs.kaeso.app/development/kpp/fixtures). `polls/` holds the provider's answers to a job's status call.

## Files

| file | what it covers | provenance |
|---|---|---|
| `errors/permission-denied-403.json` | an error, with its status in `.meta.json` | doc: Agent Platform video generation |
| `polls/done-storage.json` | a job as the provider reports it | doc: Agent Platform video generation |
| `polls/done.json` | a job as the provider reports it | doc: Agent Platform video generation |
| `polls/error.json` | a job as the provider reports it | doc: Agent Platform video generation |
| `polls/filtered.json` | a job as the provider reports it | doc: Agent Platform video generation |
| `polls/running-no-done.json` | a job as the provider reports it | doc: Agent Platform video generation |
| `polls/running.json` | a job as the provider reports it | doc: Agent Platform video generation |
| `requests/first-last-frame.json` | a request | doc: Agent Platform video generation |
| `requests/references.json` | a request | doc: Agent Platform video generation |
| `requests/text-to-video.json` | a request | doc: Agent Platform video generation |
| `responses/started.json` | the reply that starts a job | doc: Agent Platform video generation |
