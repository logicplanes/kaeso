# `fixtures/gemini_decisions/responses`

Each reply answers the request of the same name in `requests/`. Replies name no model.

| file | what it covers | provenance |
|---|---|---|
| `noul.json` | a `noul` answer and usage by modality | doc: https://aistudio.google.com/docs/decisions-api (Quickstart, verbatim) |
| `noul-criteria.json` | a `noul` answer near 0 | doc: https://aistudio.google.com/docs/decisions-api ("Yes or no", answer verbatim, with `usage` added) |
| `choice.json` | a choice with probabilities and `confidence` | doc: https://aistudio.google.com/docs/decisions-api ("Pick one option", answer verbatim, with `usage` added) |
| `score.json` | a score with `legend`, probabilities and `confidence` | doc: https://aistudio.google.com/docs/decisions-api ("Rate on a scale", answer verbatim, with `usage` added) |
| `multiple-questions.json` | three answers; probabilities of `0.0` | doc: https://aistudio.google.com/docs/decisions-api ("Ask multiple questions", verbatim) |
| `image.json` | a `noul` answer; text and image tokens | schema: `CreateDecisionResponse`, `ModalityTokens` |
| `audio-uri.json` | text and audio tokens | schema: `CreateDecisionResponse` |
| `video-uri.json` | text, video and audio tokens | schema: `CreateDecisionResponse` |
| `document.json` | a choice; text and document tokens | schema: `CreateDecisionResponse` |
| `files-api.json` | a score; text and image tokens | schema: `CreateDecisionResponse` |
| `input-object.json` | a `noul` answer | schema: `CreateDecisionResponse` |
| `instructions-blocks.json` | a `noul` and a choice | schema: `CreateDecisionResponse` |
| `blocked.json` | `prompt_feedback.block_reason` with empty `answers` (KPP: `ContentFiltered`) | schema: `PromptFeedback` ("If `prompt_feedback.block_reason` is set … `answers` is empty") |
| `unknown-top-level-field.json` | MADE-UP top-level field `x_trace` (Extras) | schema: constructed; field invented on purpose (unknown-* rule) |
| `unknown-fields.json` | MADE-UP answer field `x_rationale_id` (Extras); `total_cached_tokens` of 0 | schema: constructed; field invented on purpose (unknown-* rule); sdk: `Usage.total_cached_tokens` |
