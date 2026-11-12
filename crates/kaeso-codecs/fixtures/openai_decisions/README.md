# Fixtures: `openai_decisions`

Wire bodies for OpenAI's Decisions API, `POST /v1/decisions`: typed questions about an input, answered with probabilities.

## Sources

- OpenAI's API reference, [Create a decision](https://developers.openai.com/api/reference/resources/decisions/methods/create), read on 2026-11-12: `input` and its items and parts, `questions` (`Predicate`, `Choice`, `Score`), `prompt_cache_key`, `prompt_cache_options`, `safety_identifier`, and the `Decision` response with its answers and usage.
- OpenAI's [Decisions guide](https://developers.openai.com/api/docs/guides/decisions) and [error codes](https://developers.openai.com/api/docs/guides/error-codes), read on 2026-11-12.
- `openai-python` 3.49.0 (2026-11-10): `src/openai/types/decision_create_params.py` and `src/openai/types/decision.py`.
- The requests and response excerpts of the reference and the guide are verbatim (`doc:`), completed with `model` and `usage` where the guide shows an excerpt. Everything else is built from the reference and the SDK types (`schema:`, `sdk:`). Every file is spec-built: no key was used, so nothing here is a live recording.

The layout and the provenance marks (`doc:`, `sdk:`, `schema:`) are in [Fixtures and recordings](https://docs.kaeso.app/development/kpp/fixtures), and the rules every constructed fixture follows in [Fixture conventions](https://docs.kaeso.app/development/kpp/fixture-conventions). Each folder's `README.md` lists every file, what it covers and its provenance.

A response is read against the questions its request sent, so every file in `responses/` has a request of the same name in `requests/`. In the built replies, `confidence` follows TypeSafe's formulas, which the guide's two examples match.

## Doubts

- OpenAI documents no error codes of its own for decisions. The error bodies use the envelope and codes of the error-codes guide; the 400 for too many questions is written in OpenAI's usual wording for an array over its maximum. Confirm them by recording.
- The reference marks a request's `name` as an optional string and an answer's `name` as a string or `null`; the built replies write `null` for unnamed questions, as the reference's refusal description says.
