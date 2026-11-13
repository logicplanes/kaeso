# system_one / providers / typesafe (TypeSafe)

TypeSafe's own API at `https://api.typesafe.ai` with `Authorization: Bearer`. Its System One requests, replies and errors are the format's own fixtures one folder up; this folder holds what only TypeSafe the provider answers: refused keys and its model listing.

Sources: TypeSafe's OpenAPI document `https://api.typesafe.ai/openapi.json`, version 0.2.0 (`GET /v1/models`, `ModelMetadataList`, `ModelMetadata`), and https://docs.typesafe.ai/models and https://docs.typesafe.ai/api/errors, read on 2026-11-12. No key was available, so no decision was recorded. The two refused-key bodies were recorded on 2026-11-13 from requests without a key and with a made-up one, which TypeSafe answers without a key of its own.

| file | what it covers | provenance |
|---|---|---|
| `errors/recorded-invalid-key-401.json`, `.meta.json` | 401 for a key TypeSafe doesn't know: `detail` object with `error_type: "authentication_error"` | recorded: `POST /v1/systemone` and `GET /v1/models` with a made-up key, 2026-11-13 |
| `errors/recorded-missing-key-403.json`, `.meta.json` | 403 for a request without a key, the same `error_type`; read as a refused key | recorded: `POST /v1/systemone` and `GET /v1/models` without `Authorization`, 2026-11-13 |
| `models/list-doc.json` | `GET /v1/models` in TypeSafe's own shape, `{"models": [{name, description, release_date}]}` | doc: OpenAPI `ModelMetadataList` example, verbatim |
| `models/list.json` | the listing with both versioned ids and both aliases | schema: `ModelMetadataList`; ids and dates from https://docs.typesafe.ai/models, descriptions built |

## Doubts

- Whether `GET /v1/models` lists the versioned ids as well as the aliases isn't documented; its description says "models and aliases". Kaeso treats the listing as partial, so a model it leaves out is never reported unavailable.
- TypeSafe has no `GET /v1/models/{model}`: it answered `404 {"detail":"Not Found"}` with and without a key (2026-11-13).
