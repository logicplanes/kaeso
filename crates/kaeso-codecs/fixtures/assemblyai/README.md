# Fixtures: `assemblyai`

Wire bodies for AssemblyAI's Sync API (`POST https://sync.assemblyai.com/v1/transcribe`, an upload of `audio` and a JSON `config`, the model in `X-AAI-Model`). No key was available: the replies are the OpenAPI document's example or built from its `SyncTranscriptResponse`; the refused key and the request without a model were answered live on 2026-11-14.

## Sources (read 2026-11-14)

- Sync API reference and OpenAPI document: https://www.assemblyai.com/docs/api-reference/sync-api/transcribe
- Sync API limits and errors: https://www.assemblyai.com/docs/sync-stt

## Files

| File | What it covers | Provenance |
|---|---|---|
| `responses/doc.json` | words without times (`timestamps` off), `confidence`, `audio_duration_ms`, `session_id`, `request_time_ms` | the OpenAPI document's example, verbatim |
| `responses/timestamps-built.json` | words with `start` and `end` in milliseconds (`timestamps: true`) | built from `SyncTranscriptResponse` |
| `responses/unaligned-built.json` | a word the service couldn't align, without times | built from the `timestamps` description |
| `errors/recorded-invalid-key-404.json` | a made-up key: 404 `Invalid API key`, where the document says 401 | live |
| `errors/audio-too-large-413-doc.json`, `inference-timeout-504-doc.json`, `too-many-requests-429-doc.json` | documented titles | the document's titles and descriptions |
| `plain/recorded-no-model-404.txt` | a request without `X-AAI-Model`: plain text | live |
