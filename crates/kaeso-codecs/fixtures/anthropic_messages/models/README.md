# `fixtures/anthropic_messages/models`

Response bodies of the Anthropic Models API: `GET /v1/models` (`ListResponse[ModelInfo]`: `data`, `first_id`, `has_more`, `last_id`) and `GET /v1/models/{model_id}` (`ModelInfo`). Files named `retrieve*` are single `ModelInfo` bodies; all others are lists.

Sources: https://platform.claude.com/docs/en/api/models/list.md and https://platform.claude.com/docs/en/api/models/retrieve.md (fetched 2026-10-11), SDK types `anthropic-sdk-python@b4b7916d` (`types/model_info.py`, `model_list_params.py`, `model_capabilities.py`, `model_line.py`), and the pinned OpenAPI spec (`ModelInfo`, `ListResponse_ModelInfo_`). The spec's `ModelInfo` predates `lifecycle`, `deprecated_at`, `retires_at` and `line`; the docs and SDK types have them.

| file | what it covers | provenance |
|---|---|---|
| `list.json` | four models, `active` and `deprecated` lifecycles, `deprecated_at`/`retires_at`, `line`, full and partial `capabilities` (`effort.xhigh: null`), `has_more: false`, consistent `first_id`/`last_id` | schema: ModelInfo per docs list.md and SDK `ModelInfo`; ids, dates and limits are illustrative |
| `list-page.json` | a middle page (`has_more: true`, `first_id`/`last_id` = first/last `data` id), as returned for `limit=2&after_id=claude-opus-5` against `list.json` | schema: same as `list.json` |
| `list-doc.json` | the docs' example response: `first_id`/`last_id` are the placeholders `"first_id"`/`"last_id"` (not the data's ids), fractional-second date-times | doc: https://platform.claude.com/docs/en/api/models/list.md (example response, verbatim) |
| `list-empty.json` | an empty page: `data: []`, `first_id: null`, `last_id: null` | schema: `ListResponse_ModelInfo_` (`first_id`/`last_id` are `string or null`) |
| `retrieve.json` | one `ModelInfo` with every documented field, `deprecated_at`/`retires_at` with milliseconds | doc: https://platform.claude.com/docs/en/api/models/retrieve.md (example response, verbatim) |
| `retrieve-legacy.json` | the spec's older `ModelInfo` shape: no `lifecycle`, `deprecated_at`, `retires_at`, `line`; `capabilities: null` | schema: `ModelInfo` in the pinned OpenAPI spec |
| `retrieve-date-spellings.json` | RFC 3339 spellings that a canonical `…Z` writer would not reproduce: fractional seconds with a `-07:00` offset, lowercase `t` with `+00:00`, `.000Z`; `lifecycle: retired` | schema: ModelInfo per docs; date-time spellings per RFC 3339 §5.6 |
| `unknown-fields.json` | made-up `x_future_field` on a model and `x_future_list_field` on the list, an undocumented `lifecycle` value (`sunsetting`) and `line` value (`x-future-line`) | invented on purpose (`unknown-*` convention) |
| `list-recorded.json` | Live `GET /v1/models?limit=5` (real `capabilities`, `line`, `lifecycle`, token limits, `has_more`). | recorded 2026-10-11 from api.anthropic.com, `anthropic-version: 2023-06-01` |
| `retrieve-recorded.json` | Live `GET /v1/models/claude-haiku-4-5`. | recorded 2026-10-11 from api.anthropic.com |

Round trip: every file decodes and re-encodes to an equal JSON value (`ModelInfo` → `decode_model`/`encode_model`, lists → `decode_models`/`encode_models`).
