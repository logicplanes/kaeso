# Fixtures: `fal`

Wire bodies for fal's queue (`https://queue.fal.run`, `Authorization: Key …`): `POST /{model id}` with the model's own input, the queue status, and the result each model returns. No key was available: a model can't be run without one, so every body that only a run produces is built from fal's documents; the refused-key and unknown-model errors were answered live on 2026-11-14.

## Sources (read 2026-11-14)

- The queue document and the Queue API's OpenAPI document (`SubmitResponse.example`, `QueueStatus`): https://fal.ai/docs/documentation/model-apis/inference/queue
- The per-model OpenAPI documents (`https://fal.ai/api/openapi/queue/openapi.json?endpoint_id=<id>`) of each model mapped here, for its `Input` and `Output`.
- The webhooks document's result payload: https://fal.ai/docs/documentation/model-apis/inference/webhooks
- The model and request errors pages: https://fal.ai/docs/documentation/model-apis/errors.md, https://fal.ai/docs/documentation/model-apis/request-errors.md

## Files

| Folder | What it covers | Provenance |
|---|---|---|
| `requests/images/*.json`, `requests/videos/*.json` | the body Kaeso sends each mapped model, with the model id in `*.meta.json` | built from each model's OpenAPI `Input` |
| `queue/submitted.json` | the submit reply | the Queue API's `SubmitResponse.example`, verbatim |
| `queue/submitted-short.json`, `in-queue.json`, `in-progress.json`, `completed.json` | statuses without `status_url`, as the queue doc's examples show them | built from `QueueStatus` and the queue doc's examples |
| `queue/failed.json`, `queue/cancelled.json` | `COMPLETED` with `error` and `error_type` | built from the queue doc's description and the request-errors page |
| `results/images/queue-doc.json` | an image result | the queue doc's example |
| `results/images/webhook-doc.json` | an image result | the webhooks doc's payload, verbatim |
| `results/images/flux-sync.json`, `results/images/seedream.json`, `results/videos/*.json` | data URIs with `sync_mode`, Seedream's and the video models' outputs | built from each model's OpenAPI `Output` |
| `moderated/nsfw-black-image.json` | `has_nsfw_concepts: [true]`, FLUX's black safety image | built from FLUX's `Output` schema |
| `errors/recorded-*.json` | 401 without a key, with a key on the status URL and as Bearer; 404 for an unknown model | live, without a valid key |
| `errors/content-policy-422.json`, `generation-timeout-504.json`, `request-timeout-504.json` | model and request errors | the errors pages' shapes; texts as documented where given, otherwise built |

## Doubts

- fal's billing per megapixel, image or second comes from KPMR (logicplanes/kpmr#1574); replies carry no usage.
- Whether every mapped model returns `has_nsfw_concepts` is documented for FLUX only.
