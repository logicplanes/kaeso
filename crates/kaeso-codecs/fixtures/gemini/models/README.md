# `fixtures/gemini/models`

Response bodies of `GET /v1beta/models` (`ListModelsResponse`: `models`, `nextPageToken`) and `GET /v1beta/models/{model}` (`Model`). They were fetched on 2026-11-08 and indented by two spaces.

| file | what it covers | provenance |
|---|---|---|
| `recorded-list-page-size-5.json` | the first page for `pageSize=5`, with `nextPageToken` | live `GET /v1beta/models?pageSize=5` |
| `recorded-list-next-page.json` | the page for that `nextPageToken` | live `GET /v1beta/models?pageSize=5&pageToken=…` |
| `recorded-get-gemini-3.5-flash.json` | one model | live `GET /v1beta/models/gemini-3.5-flash` |
