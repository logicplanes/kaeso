---
title: Decisions across formats
nav: Decisions
description: How decision requests and answers move between OpenAI's Decisions API, TypeSafe's System One API and Gemini's Decisions API.
order: 3
---

Decision requests cross between OpenAI's Decisions API ([OpenAI Decisions](openai-decisions.md)), TypeSafe's System One API ([System One](system-one.md)) and Gemini's Decisions API ([Gemini Decisions](gemini-decisions.md)) through one `DecideRequest`. A target refuses what its format can't say before anything is sent. A few named adaptations are listed in the reply's `kaeso-adapted` header, and hints the target has no place for in `kaeso-left-out`. The IDs in brackets are the design decisions of the decision formats.

## Requests between OpenAI and System One

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

## Requests to and from Gemini

| What | OpenAI or System One app to Gemini | Gemini app to OpenAI or System One |
| --- | --- | --- |
| Question names (D19) | Kept when every question has a name, used once and matching `[A-Za-z0-9_-]{1,64}`; otherwise all sent as `q0`…`qN-1`, `adapted: names` | Each key is the question's name |
| Option keys | Each value, booleans as `"true"` and `"false"` (`adapted: booleans`). A key Google refuses is `Unsupported` (`option key`) | Each key is the option's value |
| Option descriptions | A missing description is `""` | Kept, `""` included |
| Score levels | As for System One: `"label: description"`, `adapted: levels`; a level without text is `Unsupported` | Each level's text, as a System One level |
| Structured state (D13) | Its JSON as written, as the input string; no adaptation is named, as Google's guide asks for it | — |
| JSON parts, instructions and descriptions | Their JSON as written, as text; `adapted: instructions` or `criteria` | — |
| Predicate criteria | `true_description` and `false_description` | Folded into OpenAI's instructions (D11); System One's `true` and `false` |
| Conversations (D12) | `Unsupported` (`conversation input`) | — |
| Images | Bytes and data URLs as `data`; `https` URLs as `uri` with the MIME type of their extension; `http`, `gs://` and YouTube URLs are `Unsupported` | Inline images cross; Files API URIs are `Unsupported` (`file id`) |
| Asset references | In the input string or a string of a state list, an `image` block in place; list items' text joined with a line break; elsewhere `Unsupported` | — |
| Images in instructions or descriptions | `Unsupported`: Gemini takes text only there | — |
| Audio, video and PDF | — | `Unsupported`, naming the part |
| `detail` and `resolution` | OpenAI's `low` and `high` become `resolution`; other values are left out | `low` and `high` become OpenAI's `detail`; other values are left out |
| Box questions | `Unsupported` | — |
| Limits (D5) | 1,000 questions, 2 to 255 options, 2 to 255 levels | As in the table above |
| `prompt_cache_key`, `prompt_cache_options`, `safety_identifier`, breakpoints | Left out, `left_out: cache` or `user` | — |

Google refuses an option key of more than 128 characters or 512 bytes, with a space at either end, a control character or a non-ASCII space. Gemini joins adjacent text blocks with no separator, so its blocks reach System One as a state list of strings and OpenAI as text parts.

## Replies

Replies cross by position: the encoder of the app's format takes every name, option and level from the app's request.

- **Confidence (D8).** A reply in its own format keeps its `confidence` byte for byte. One from another format gets it recomputed from the probabilities: TypeSafe's formulas for System One apps, and the same for OpenAI apps, whose two documented examples match them. Gemini apps get Google's rule: the top probability, and `max(p, 1 − p)` for a yes/no answer. A predicate answer has none for OpenAI and System One apps.
- **Refusals (D9).** An OpenAI refusal reaches a System One app as `{"type": "refusal"}` under the question's key. TypeSafe's SDKs skip answer types they don't know, and the other answers still count. Gemini's format has no refusal answer, so a reply with refusals reaches a Gemini app as `ContentFiltered`, a 400 with the code `safety` that names the refused questions.
- **Safety blocks (D10).** Gemini reports a blocked input as a 400 with a safety code, or as a reply with `prompt_feedback.block_reason` and no answers. Both reach every app as `ContentFiltered` in its own format.
- **Labels and legends.** An OpenAI app gets each level's label from its request; System One and Gemini apps get a `legend` from their request's levels.
- **Model.** Gemini's replies name no model, so the decoder takes the request's; Gemini apps get none.
- **Usage.** KPP counts input tokens without the cache. OpenAI's `input_tokens` holds cache reads and writes, so the decoder subtracts them and the encoder adds them back; `total_tokens` is input plus output. Gemini counts input by modality: KPP keeps image tokens, video and PDF pages included, and audio tokens apart. A Gemini app gets `input_tokens_by_modality` with text as the rest.
