# `fixtures/system_one/responses`

Each reply answers the request of the same name in `requests/`. `confidence` follows TypeSafe's documented formulas (https://docs.typesafe.ai/confidence) in the built replies.

| file | what it covers | provenance |
|---|---|---|
| `noul.json` | `noul` answer | doc: OpenAPI `SystemOneResponse` example `noul` (openapi.json 0.2.0, read 2026-11-12) |
| `choice.json` | `choice`, `confidence`, `probabilities` | doc: OpenAPI example `choice` |
| `score.json` | `score`, `confidence`, `legend`, `probabilities` | doc: OpenAPI example `score` |
| `box.json` | `box` with `confidence: null` | doc: OpenAPI example `box` |
| `structured-state.json` | answers in another order than the questions; a resolved model name | schema: `SystemOneResponse` ("key order not guaranteed") |
| `noul-criteria.json` | five noul answers | schema: `NoulAnswer` |
| `instructions-forms.json` | a legend of object, array and text levels, one option with confidence 1, one level with confidence 1 | schema: `ScoreAnswer.legend`, `ChoiceAnswer` |
| `long-context.json` | one noul answer | schema: `NoulAnswer` |
| `assets.json` | answers to questions about images | schema: `SystemOneResponse` |
| `assets-criteria.json` | a choice over image options and a box with a confidence | schema: `ChoiceAnswer`, `BoxAnswer` |
| `unknown-top-level-field.json` | the noul reply to `unknown-top-level-field` | doc: OpenAPI example `noul` |
| `unknown-fields.json` | MADE-UP answer field `x_rationale_id` and top-level field `x_trace` (Extras) | schema: constructed; fields invented on purpose (unknown-* rule) |
