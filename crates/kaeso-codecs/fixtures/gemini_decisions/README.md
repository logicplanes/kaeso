# Fixtures: `gemini_decisions`

Wire bodies for Gemini's Decisions API, `POST /v1beta/decisions`: typed questions about an input, answered with probabilities.

## Sources

- Google's [Decisions API guide](https://aistudio.google.com/docs/decisions-api), read on 2026-11-12: the request and response examples, `Input` and its content blocks, question types, limits, the safety error and the limitations.
- Google's [Decisions API reference](https://ai.google.dev/api/decisions), read on 2026-11-12: `CreateDecisionResponse`, `Question`, `NoulQuestion`, `ChoiceQuestion`, `ScoreQuestion`, `Answer`, `PromptFeedback` and the Interactions `Usage`.
- `google-genai` 3.3.0 for Python: `google/genai/_gaos/types/decisions/*` and the Interactions content blocks `TextContent`, `ImageContent`, `AudioContent`, `VideoContent` and `DocumentContent`.
- The requests and response examples of the guide are verbatim (`doc:`), completed with `usage` where the guide shows an answer only. Everything else is built from the reference and the SDK types (`schema:`, `sdk:`). Every file is spec-built: no key was used, so nothing here is a live recording.

The layout and the provenance marks (`doc:`, `sdk:`, `schema:`) are in [Fixtures and recordings](https://docs.kaeso.app/development/kpp/fixtures), and the rules every constructed fixture follows in [Fixture conventions](https://docs.kaeso.app/development/kpp/fixture-conventions). Each folder's `README.md` lists every file, what it covers and its provenance.

A response is read against the questions its request sent, so every file in `responses/` has a request of the same name in `requests/`.

## Doubts

- The guide says a blocked input is an HTTP 400 with a safety `code`; the reference describes a reply with `prompt_feedback.block_reason` and empty `answers`. Both are here (`errors/safety-400.json`, `responses/blocked.json`), and KPP reads both as `ContentFiltered`. Confirm which one the API sends by recording.
- The 400 bodies other than the safety error are written in Google's usual envelope with a message that names the rule, as the guide says. Their exact wording is not documented. Confirm them by recording.
- The reply's `usage` follows the Interactions `Usage`; whether `total_input_tokens` counts a cached part is not documented for decisions, which have no caching. KPP reads `total_cached_tokens` as part of it, as for `generateContent`.
