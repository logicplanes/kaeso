# Fixtures: Cohere (`cohere`) on `openai_embeddings`

Cohere's Compatibility API, `POST https://api.cohere.ai/compatibility/v1/embeddings` with `Authorization: Bearer`. Source: https://docs.cohere.com/docs/compatibility-api, read 2026-10-30, which lists `input`, `model` and `encoding_format` (`float` or `base64`) as supported and `dimensions` and `user` as unsupported.

Every file under `responses/` and `errors/` was recorded live with `cargo xtask record` on 2026-10-30; `RECORDED.md` in each folder names the request and model.

| file | what it covers |
|---|---|
| `requests/embed-string.json`, `responses/recorded-embed-string.json` | one string with `encoding_format: float`; 384 floats from `embed-english-light-v3.0` |
| `requests/embed-array.json`, `responses/recorded-embed-array.json` | two strings |
| `requests/embed-base64.json`, `responses/recorded-embed-base64.json` | `encoding_format: base64`, answered as base64 |
| `requests/embed-dimensions.json`, `responses/recorded-embed-dimensions.json` | `dimensions: 256` on `embed-v4.0`, which Cohere honours although its page lists it as unsupported |
| `requests/embed-dimensions-v3.json`, `errors/recorded-embed-dimensions-v3.json` | 400: `output_dimension is not supported for this model` |
| `requests/embed-token-ids.json`, `errors/recorded-embed-token-ids.json` | 422 for token ids as input |
| `requests/embed-generation-model.json`, `errors/recorded-embed-generation-model.json` | 400 for a generation model |

The error bodies are `{"id", "message"}`, without OpenAI's `error` object, so apps get an OpenAI error with Cohere's message; `openai_embeddings::tests::cohere_error_bodies_reach_apps_with_cohere_message` checks that instead of the round trip.
