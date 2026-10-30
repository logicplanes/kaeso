# openai_embeddings / providers / mistral (Mistral)

Mistral, provider id `mistral`, base URL `https://api.mistral.ai`, `POST /v1/embeddings` with `Authorization: Bearer`. API reference: https://docs.mistral.ai/api/ (`EmbeddingRequest`, `EmbeddingResponse`), read on 2026-10-19.

Every file under `responses/` and `errors/` was recorded live from `mistral-embed` with `cargo xtask record` on 2026-10-19; `RECORDED.md` in each folder names the request. The replies keep their 1024 floats as sent.

| file | what it covers |
|---|---|
| `requests/embed-string.json`, `responses/recorded-embed-string.json` | one string with `encoding_format: float`; the reply has an `id` and usage with `completion_tokens: 0` and several `null` fields (Extras) |
| `requests/embed-array.json`, `responses/recorded-embed-array.json` | two strings |
| `requests/embed-base64-ignored.json`, `responses/recorded-embed-base64-ignored.json` | `encoding_format: base64` is accepted, but Mistral answers with floats; the codec encodes them as base64 for an app that asked for it |
| `requests/embed-output-dimension.json`, `errors/recorded-embed-output-dimension.json` | `output_dimension` on `mistral-embed`: HTTP 400, `code: "3051"` |
| `errors/recorded-string-input.json` | OpenAI's `dimensions`: HTTP 422 `extra_forbidden` |
| `errors/recorded-token-ids.json` | token ids as `input`: HTTP 422 |

These error bodies have no `error` object, so the codec answers apps with an OpenAI error that carries their top-level `message` when it is a string and quotes the body otherwise; `openai_embeddings::tests::error_bodies_without_an_error_object_reach_apps_as_openai_errors_with_their_message` checks that instead of the round trip.
