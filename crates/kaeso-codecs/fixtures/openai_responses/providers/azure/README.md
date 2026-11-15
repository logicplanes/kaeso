# Fixtures: Azure (`azure`) on `openai_responses`

Azure's OpenAI v1 API, `POST {endpoint}/openai/v1/responses` without an `api-version`, with the resource key in `api-key`. Kaeso sends a model there when its **Sends as** value is Responses; deployments that don't take the Responses API answer `400 Model not supported`.

No key was available. `responses/bitter-lesson-doc.json` is the reply of the REST example on Microsoft's [endpoints page](https://learn.microsoft.com/en-us/azure/foundry/foundry-models/concepts/endpoints), read 2026-11-14, unchanged, with its request; the error was answered live, against the documentation's example host, with a made-up key.

| File | What it covers | Provenance |
|---|---|---|
| `requests/bitter-lesson-doc.json`, `responses/bitter-lesson-doc.json` | a `deepseek-v3-0324` deployment's reply with one message and `usage` of 28 input and 52 output tokens | doc |
| `errors/recorded-invalid-api-key.json` | 401 with `code: "401"` as a string | live, with a made-up key |

Azure adds `content_filters` to Responses replies (v1 OpenAPI document, `AzureContentFilterForResponsesAPI`); no documented reply carries them, so none is built here.
