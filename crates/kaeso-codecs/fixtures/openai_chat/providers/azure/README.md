# Fixtures: Azure (`azure`) on `openai_chat`

Azure OpenAI and Microsoft Foundry through Azure's OpenAI v1 API, `POST {endpoint}/openai/v1/chat/completions` without an `api-version`, with the resource key in `api-key`. Each key carries its resource's endpoint, such as `https://example.openai.azure.com`; `model` names a deployment of the resource, which Kaeso fills with the model's `upstream_model` when it has one.

No key was available. `responses/greeting-published.json` is a reply Azure sent to a `gpt-35-turbo` deployment, published unchanged in a Stack Overflow answer. The other replies and the two streams are the samples of Microsoft's documentation; in `streams/filter-blocked-doc.sse` a model name the page wraps across two lines (`"gpt-35-` / `turbo"`) is joined again, and the `...` lines the samples elide are left out. The requests are built for them. The key errors were answered live, against the documentation's example host, with a made-up key; Azure answers the same 401 on every v1 path.

## Sources (read 2026-11-14)

| Source | Address |
|---|---|
| v1 OpenAPI document (`CreateChatCompletionRequest`, `prompt_filter_results`, `content_filter_results`) | https://github.com/Azure/azure-rest-api-specs/blob/a6943a926f76/specification/ai/data-plane/OpenAI.v1/azure-v1-v1-generated.json |
| API lifecycle (`{endpoint}/openai/v1`, no `api-version`) | https://learn.microsoft.com/en-us/azure/foundry/openai/api-version-lifecycle |
| Content streaming (the two streams, their first chunk with `choices: []` and the annotation-only chunks) | https://learn.microsoft.com/en-us/azure/foundry/openai/concepts/content-streaming |
| Blocklists (`responses/filtered-blocklist-doc.json`, `errors/content-filter-prompt-doc.json`) | https://learn.microsoft.com/en-us/azure/foundry-classic/openai/how-to/use-blocklists |
| Content filter annotations (`errors/content-filter-simple-doc.json`) | https://learn.microsoft.com/en-us/azure/foundry-classic/openai/concepts/content-filter-annotations |
| A reply with `prompt_annotations` (`responses/greeting-published.json`) | https://stackoverflow.com/questions/76757194 |
| `DeploymentNotFound` (`errors/deployment-not-found-published.json`) | https://techcommunity.microsoft.com/tag/chatgpt |

## Files

| File | What it covers | Provenance |
|---|---|---|
| `requests/greeting-published.json`, `responses/greeting-published.json` | `prompt_annotations` and per-choice `content_filter_results` beside an OpenAI reply | published |
| `requests/filtered-blocklist-doc.json`, `responses/filtered-blocklist-doc.json` | a reply a blocklist stopped: `finish_reason: "content_filter"`, a message without `content`, `custom_blocklists` as a list | doc |
| `requests/filter-pass-doc.json`, `streams/filter-pass-doc.sse` | a first chunk with empty `id`, `object` and `model`, `prompt_annotations` and `choices: []`; annotation-only chunks with `content_filter_offsets`, one after `finish_reason: "stop"` | doc |
| `requests/filter-blocked-doc.json`, `streams/filter-blocked-doc.sse` | the same with `prompt_filter_results`, ending in an annotation-only chunk with `finish_reason: "content_filter"` | doc |
| `errors/content-filter-prompt-doc.json` | 400 `content_filter` with `innererror.content_filter_result` for a prompt a blocklist matched | doc |
| `errors/content-filter-simple-doc.json` | 400 `content_filter` without `innererror` | doc |
| `errors/deployment-not-found-published.json` | 404 `DeploymentNotFound` | published |
| `requests/invalid-api-key.json`, `errors/recorded-invalid-api-key.json` | 401 with `code: "401"` as a string, and no `type` or `param` | live, with a made-up key |
| `errors/recorded-unknown-path.json` | 404 `Resource not found` for a path the API doesn't serve, answered before the key is checked | live, with a made-up key |
| `errors/recorded-entra-token.json` | 401 `{"statusCode", "message"}` for a bearer that looks like an Entra ID token, without OpenAI's `error` object | live, with a made-up token |

## Where Azure differs from OpenAI Chat Completions

| Difference | How KPP carries it | Source |
|---|---|---|
| each key belongs to one resource, called at its own endpoint | the key's endpoint and `/openai` replace the base URL (`KeyEndpoint`) | API lifecycle |
| the key goes in `api-key` | `AuthStyle::ApiKey`; a bearer works too, but one shaped like a JWT is read as an Entra ID token | live |
| `model` is a deployment's name, which may differ from the model's id | the model's `upstream_model` value | endpoints page |
| `prompt_filter_results` (`prompt_annotations` in older replies) and `content_filter_results` on each choice | extras: round-trip for Chat apps, left out towards other dialects | v1 OpenAPI document; published reply |
| a stream's first chunk has empty `id`, `object` and `model` and `choices: []` | the reply's id and model come from the next chunk | content streaming |
| annotation-only chunks carry `content_filter_results` and offsets in a choice without `delta`, also after the finish | extras; no text | content streaming |
| a blocked reply ends with `finish_reason: "content_filter"`, in a stream on an annotation-only chunk | `StopReason::ContentFilter` | blocklists; content streaming |
| a blocked prompt is a 400 with `code: "content_filter"` and Azure's `innererror` | `ContentFiltered` | blocklists |
| an unknown field gets 400 "Unrecognized request argument supplied" | nothing is refused before sending; Azure answers | published replies (openai/openai-python#1724, Azure/azure-sdk-for-net#45719) |
| `GET /openai/v1/models` lists the models a resource may deploy, not its deployments | `ModelListing::Catalogue`: a model check only shows that the key and its endpoint work | published reports (langflow-ai/langflow#14023, different-ai/openwork#2431) |
| Claude deployments answer only Anthropic's Messages API under `{endpoint}/anthropic` | not called yet | Claude in Microsoft Foundry |
