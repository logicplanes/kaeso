---
title: Decisions across formats
nav: Decisions
description: How decision requests and answers move between OpenAI's Decisions API and TypeSafe's System One API.
order: 3
---

Decision requests cross between OpenAI's Decisions API ([OpenAI Decisions](openai-decisions.md)) and TypeSafe's System One API ([System One](system-one.md)) through one `DecideRequest`. A target refuses what its format can't say before anything is sent. A few named adaptations are listed in the reply's `kaeso-adapted` header, and hints the target has no place for in `kaeso-left-out`. The IDs in brackets are the design decisions of the decision formats.

## Requests

| What | OpenAI app to System One provider | System One app to OpenAI |
| --- | --- | --- |
| Question names (D19) | Kept when every question has a name, used once and valid for the provider; otherwise all sent as `q0`…`qN-1`, `adapted: names` | Each key is the question's `name` |
| Boolean values (D18) | Sent as `"true"` and `"false"`, `adapted: booleans`; a question with both `true` and `"true"` is `Unsupported` | — |
| Score levels | `"label: description"` where a level has both, `adapted: levels`; the label alone otherwise | Each level's text becomes its `label` |
| Structured state (D13) | — | `input_json` within OpenAI's limits; otherwise `input_text` with the JSON as written, `adapted: state` |
| Predicate criteria (D11) | — | Folded into the instructions with one template, `adapted: criteria` |
| JSON instructions and descriptions | — | Their JSON as written, as text, `adapted: instructions` or `criteria` |
| Conversations (D12) | `Unsupported` (`conversation input`): send one user message | — |
| Images | Data URLs become assets; URLs are `Unsupported` (`image URL`): send the image as a data URL (D6) | Each reference in a string of the input, a state list or the instructions becomes an `input_image` part in its place; elsewhere `Unsupported` |
| Box questions | Only where the profile answers them | `Unsupported` |
| Limits (D5) | The profile's question, option, level and image limits | 400 questions, 2 to 255 options, 2 to 10 levels |
| `prompt_cache_key`, `prompt_cache_options`, breakpoints | Left out, `left_out: cache` | — |
| `safety_identifier` | Left out, `left_out: user` | — |
| `detail` | Left out | — |

The criteria template adds a blank line and then `It is true when: <true>` and `It is false when: <false>`, one line each, to the instructions. Instructions made of parts get the lines as one more text part. JSON text is never reformatted.

## Replies

Replies cross by position: the encoder of the app's format takes every name, option and level from the app's request.

- **Confidence (D8).** A reply in its own format keeps its `confidence` byte for byte. One from another format gets it recomputed from the probabilities: TypeSafe's formulas for System One apps, and the same for OpenAI apps, whose two documented examples match them. A predicate answer has none.
- **Refusals (D9).** An OpenAI refusal reaches a System One app as `{"type": "refusal"}` under the question's key. TypeSafe's SDKs skip answer types they don't know, and the other answers still count.
- **Labels and legends.** An OpenAI app gets each level's label from its request; a System One app gets a `legend` from its request's levels.
- **Usage.** KPP counts input tokens without the cache. OpenAI's `input_tokens` holds cache reads and writes, so the decoder subtracts them and the encoder adds them back; `total_tokens` is input plus output.
