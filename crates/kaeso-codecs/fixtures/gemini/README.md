# Fixtures: `gemini`

Wire bodies for the Gemini API on Google AI Studio: `POST /v1beta/models/{model}:generateContent`, `:streamGenerateContent?alt=sse` and `:countTokens`, and the model listing `GET /v1beta/models`.

The layout and the provenance marks (`doc:`, `sdk:`, `schema:`) are in [Fixtures and recordings](https://docs.kaeso.app/development/kpp/fixtures), and the rules every constructed fixture follows in [Fixture conventions](https://docs.kaeso.app/development/kpp/fixture-conventions). Each folder's `README.md` lists every file, what it covers and its provenance.

## Sources

- The Gemini API discovery document, `https://generativelanguage.googleapis.com/$discovery/rest?version=v1beta`, revision `20261106`, downloaded 2026-11-08. Schema names in `schema:` rows refer to it.
- The Gemini API documentation at https://aistudio.google.com/docs (text dump `https://aistudio.google.com/llms-full.txt`, read 2026-11-08): text generation, thinking, thought signatures, function calling, structured output, token counting, embeddings, models and troubleshooting.
- `google-genai` 2.32.0, the official Python SDK, as a tie-breaker; the SDK conformance suite runs it.

## Recordings

Every `recorded-*` file comes from the live API with a free-tier Google AI Studio key, through `cargo xtask record gemini`, on 2026-11-08. A Gemini request names its model and method in the path, so a request fixture's `<name>.meta.json` holds `{"model", "action"}` where it needs another model or method than `--model` and `generateContent`. `gemini-3.5-flash` answered as `gemini-3.6-flash`.

## Gaps

- No safety block could be reproduced; `responses/prompt-blocked.json` is built from the schema.
- Image output is not available to this key (`errors/recorded-image-output.json`, `errors/image-generation-region.json`); `responses/image-output.json` is built from the schema.
- No 401 was seen: Google answers a rejected key with 400 and the reason `API_KEY_INVALID`.
