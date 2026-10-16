# `fixtures/openai_chat/models`

Response bodies of the OpenAI Models API: `GET /v1/models` (`ListModelsResponse`: `object: "list"`, `data`) and `GET /v1/models/{model}` (`Model`: `id`, `object: "model"`, `created`, `owned_by`, `shutdown_date`). Files named `retrieve*` are single `Model` bodies; all others are lists.

The model shape is the same for every OpenAI API, so these fixtures serve all three OpenAI codecs: `openai_chat`, `openai_responses` and `openai_embeddings` round-trip them through the shared module `crates/kaeso-codecs/src/openai_models.rs`. There are no copies under `fixtures/openai_responses/` or `fixtures/openai_embeddings/`.

Source: OpenAI OpenAPI spec `openapi.yaml` at commit `e95c0fe615f45a19e8878af923a729948445a6bd` (2026-10-09): schemas `Model` and `ListModelsResponse`, operations `listModels` (no query parameters) and `retrieveModel`.

| file | what it covers | provenance |
|---|---|---|
| `list.json` | three models, `shutdown_date` `null` and a date | doc: `listModels` `x-oaiMeta.examples.response` (openapi.yaml@e95c0fe615f4), verbatim |
| `retrieve.json` | one model with a `shutdown_date` | doc: `retrieveModel` `x-oaiMeta.examples.response` (openapi.yaml@e95c0fe615f4), verbatim |
| `unknown-fields.json` | made-up `x_future_field` on a model and `x_future_list_field` on the list; a fine-tuned model id and a model without `shutdown_date` (the spec does not require it) | invented on purpose (`unknown-*` convention); otherwise schema: `Model` |
| `list-recorded.json` | Live `GET /v1/models`, the first 6 of 135 models kept (shape unchanged, incl. `shutdown_date`). | recorded 2026-10-11 from api.openai.com |
| `retrieve-recorded.json` | Live `GET /v1/models/gpt-5.4-mini`. | recorded 2026-10-11 from api.openai.com |

Round trip: every file decodes and re-encodes to an equal JSON value with each of the three OpenAI codecs.
