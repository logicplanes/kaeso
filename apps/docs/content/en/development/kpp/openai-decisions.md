---
title: OpenAI Decisions
description: How every field of OpenAI's Decisions API maps to KPP and back.
order: 8
---

Look up any field of OpenAI's Decisions API here: where it goes in KPP, what a request from another decision format becomes, and the source of its mapping. Check it before you change how the codec reads or writes that field.

- **Codec:** `kaeso_codecs::openai_decisions::OpenAiDecisions` (`crates/kaeso-codecs/src/openai_decisions/`)
- **Endpoints:** `POST /v1/decisions` and the OpenAI [models](#models) endpoints
- **Fixtures:** `crates/kaeso-codecs/fixtures/openai_decisions/`, spec-built from the API reference and `openai-python`'s types, none recorded

**Sources.** OpenAI's [Create a decision](https://developers.openai.com/api/reference/resources/decisions/methods/create) reference, the [Decisions guide](https://developers.openai.com/api/docs/guides/decisions) and the [error codes](https://developers.openai.com/api/docs/guides/error-codes), read on 2026-11-12. `openai-python` 3.49.0: `types/decision_create_params.py` and `types/decision.py`.

The gateway doesn't route decision requests yet; the codec is ready for it.

## How to read the tables

- **typed**: a typed KPP field.
- **extras**: `Extras::fields`, bound to `openai_decisions`; another dialect refuses them unless they are [allowlisted](cross-format.md#allowlist).
- **form**: a codec-private `Extras::form` marker; `verbatim` keeps `null` spellings of declared keys.

## Request: `DecisionCreateParams`

A decision request decodes to `Request::Decide(DecideRequest)`. Its questions keep their order, and `answers[i]` of the reply answers `questions[i]`.

| Wire field | KPP | Notes |
|---|---|---|
| `model` | `DecideRequest.model` typed | Required. |
| `input` string | `input = DecisionInput::Text` typed | |
| `input` list | `input = DecisionInput::Messages` typed | One `Message` per item ([input items](#input-items)). |
| `questions` | `questions` typed | A list; anything else is `InvalidRequest`. |
| `questions[].type` | `Question.kind` typed | `predicate`, `choice` or `score`. Another type is `InvalidRequest`. |
| `questions[].name` | `Question.name` typed | Optional. Names may repeat; `validate()` allows that. |
| `questions[].instructions` string | `Instructions::Text` typed | |
| `questions[].instructions` list | `Instructions::Blocks` typed | `input_text`, `input_image` and `input_json` parts ([parts](#content-parts)). |
| `choices[].value` | `ChoiceOption.value` typed | A string is `ChoiceValue::Text`, a boolean `ChoiceValue::Bool`. Anything else is `InvalidRequest`. |
| `choices[].description` | `ChoiceOption.description` typed | `Description::Text`. |
| `levels[].label` | `Level.label` typed | Levels are lowest first; level `i` scores `i`. |
| `levels[].description` | `Level.description` typed | `Description::Text`. |
| `prompt_cache_key` | `DecideRequest.cache.key` typed | |
| `prompt_cache_options` with `mode` and `ttl` | `cache.mode`, `cache.ttl` typed | Form `cache_options` keeps an empty object. Any other shape stays whole in extras, a hint. |
| `safety_identifier` | `DecideRequest.user` typed | |
| unknown keys | extras | At every level: request, question, choice, level, item and part. |
| header `openai-beta` | extras `header:openai-beta` | Forwarded to OpenAI Decisions only. |

A `predicate` with `choices` or `levels`, a `choice` without `choices` or with `levels`, and a `score` without `levels` or with `choices` are `InvalidRequest`. A body with `state` and no `input` is a System One request. It is `InvalidRequest` with a message that names `/v1/systemone` (D15).

### Input items

| Item | KPP | Notes |
|---|---|---|
| `role: "user"` message | `Message` with `Role::User` | `type: "message"` is form `typed`; a string content is one text block, form `content_string`. |
| `role: "developer"` message | `Message` with `Role::System` | Form `role` keeps `developer`. Text parts only. |
| `role: "system"` message | `Message` with `Role::System` | Text parts only. |
| `role: "assistant"` message | `Message` with `Role::Assistant` | `output_text` and `refusal` parts; `phase` stays in extras. |
| `function_call` | an assistant `Message` with one `Block::ToolCall` | `call_id`, `name`; `arguments` is held as a JSON string. `id`, `namespace` and `status` stay in extras. |
| `custom_tool_call` | an assistant `Message` with one `Block::ToolCall` | Form `custom`; `input` is held as a JSON string. |
| `function_call_output` | a user `Message` with one `Block::ToolResult` | `output` as text or text and image parts; a string is form `output_string`. |
| `custom_tool_call_output` | a user `Message` with one `Block::ToolResult` | Form `custom`. |
| another `type` | none | `InvalidRequest`: files, audio and item references aren't part of the API. |

### Content parts

| Part | KPP | Notes |
|---|---|---|
| `input_text`, `output_text` | `Block::Text` | A type that doesn't match its role, such as `input_text` in an assistant message, is form `text_type`. |
| `input_image` with a data URL | `Block::Image`, `Source::Base64` | The bytes are decoded; a header other than `data:<type>;base64` is form `data_url_header`. Base64 that doesn't decode is `InvalidRequest`. |
| `input_image` with an `http(s)` URL | `Block::Image`, `Source::Url` | Kaeso never fetches it. |
| `input_image.detail` | extras | A hint, dropped when the request crosses formats. |
| `input_json` | `Block::Json` | Kept byte for byte. |
| `refusal` | `Block::Refusal` | In assistant messages. |
| `prompt_cache_breakpoint` | `Block.cache` | A `CacheHint` holding the breakpoint as written, form `breakpoint`. |
| another `type` | `Block::Unknown` | Bound to `openai_decisions`; another format refuses it. |

## Writing requests

The encoder writes what KPP holds, and refuses before sending what OpenAI's format can't take. A request of another format, such as System One, crosses with these rules; [crossing formats](decisions-across-formats.md) has the reasons.

| KPP | Written as | Notes |
|---|---|---|
| `DecisionInput::Text` | the `input` string | |
| `DecisionInput::Json` | one user message with one `input_json` part | Only an object, or a list of objects, within 32 levels, 10,000 values and 256 KiB of compact JSON. Otherwise an `input_text` part with the JSON as written, named in `adapted` as `state` (D13). |
| `Instructions::Json` | the JSON as written, as an instructions string | Named in `adapted` as `instructions`. |
| `Instructions` missing | `""` | `instructions` is required; a request in its own format keeps it absent. |
| `Predicate.when_true`, `.when_false` | lines added to the instructions | `It is true when: …` and `It is false when: …`, after a blank line. Instructions of parts get a text part. Named in `adapted` as `criteria` (D11). |
| `Description::Json` | its JSON as written, as text | Named in `adapted` as `criteria`. |
| `Level` without a label | its description as the `label` | System One levels have descriptions only. An empty level is `""`. |
| `ChoiceValue::Bool` | `true`, `false` | |
| an asset reference in a string | the string cut around it, the image an `input_image` part in its place | In the input, in a string of a state list, and in instructions. A reference anywhere else, such as inside an object, is `Unsupported` (`image reference`); in a description it is `image in criteria`. |
| `Source::Base64` | a data URL | |
| `Source::Url` | the URL | `http` and `https` only; others are `Unsupported` (`image URL`). |
| `Source::FileId` | none | `Unsupported` (`image file id`): send the image as a data URL. |
| audio, video and document blocks | none | `Unsupported`, naming the part. |
| `QuestionKind::Box` | none | `Unsupported` (`box question`). |
| a choice with fewer than 2 or more than 255 options | none | `Unsupported` (`options`). |
| a score with fewer than 2 or more than 10 levels | none | `Unsupported` (`levels`). |
| more questions than `DecisionsProfile::max_questions` | none | `Unsupported` (`questions`); 400 for OpenAI (D5). |
| `ToolResultBlock.is_error` | none | `Unsupported` (`tool error`): the format has no error mark. |

`EgressCtx` keeps the name and the option values of every question sent, so the reply is read back by position.

## Response: `Decision`

| Wire field | KPP | Notes |
|---|---|---|
| `model` | `DecideResponse.model` typed | |
| `answers[]` | `answers` typed | One per question, in order. Another count, a type that doesn't match the question, or a value that wasn't asked is a provider error. |
| `answers[].name` | none | Written from the request. A reply that names the question otherwise keeps its name in form `name`. |
| `predicate.probability` | `PredicateAnswer.probability` typed | |
| `choice.choice` | `ChoiceAnswer.option` typed | The index of the option with that value; `true` and `"true"` stay apart. |
| `choice.probabilities[]` | `ChoiceAnswer.probabilities` typed | By option; `None` where the reply has no entry. Another order is form `order`. |
| `score.score` | `ScoreAnswer.score` typed | |
| `score.probabilities[]` | `ScoreAnswer.probabilities` typed | By `value`, the level's index. The labels as written are form `labels`, rebuilt from the request when the reply crosses formats. |
| `confidence` | `Answer.confidence` typed | See [confidence](#confidence). |
| `refusal` | `AnswerValue::Refusal` typed | Other answers of the same reply still count. |
| `usage.input_tokens` | `usage.input_tokens` typed | OpenAI counts cache reads and writes in it, KPP doesn't, so both are subtracted. |
| `usage.input_tokens_details.cached_tokens` | `usage.cache_read_tokens` typed | |
| `usage.input_tokens_details.cache_write_tokens` | `usage.cache_write_tokens` typed | |
| `usage.output_tokens` | `usage.output_tokens` typed | 0 at OpenAI. |
| `usage.output_tokens_details.reasoning_tokens` | `usage.reasoning_tokens` typed | |
| `usage` | `usage.raw` | `total_tokens` and other keys come back in the same format. |
| numbers written as integers | form `integers` | Written as integers again while the value is unchanged. |
| unknown keys | extras | At every level, probability entries included. |

The encoder writes the answers in question order and takes every name, option value and level label from the app's request in `EncodeCtx::request`. An unnamed question's answer has `name: null`. A reply from another format gets `input_tokens` with the cache counts added back, and `total_tokens` as input plus output.

### Confidence

A reply from an OpenAI Decisions provider keeps its `confidence` as sent. OpenAI doesn't document how it computes it, but both examples of its guide match TypeSafe's formulas. So an answer from another format gets those: `(p_max − 1/n) / (1 − 1/n)` for a choice, `max(0, 1 − Σ p_i·|i − m| / MAD_unif)` for a score (D8). A predicate has none.

## Streaming

Decisions don't stream. The stream encoder returns `Unsupported` (`streaming`) for every event; the stream decoder reports one `StreamEvent::Error` with `Unsupported` (`streaming`) and then nothing.

## Errors

Errors use OpenAI's envelope, `{"error": {"message", "type", "param", "code"}}`, read and written as on [OpenAI Responses](openai-responses.md#errors-errorresponse). OpenAI documents no error codes of its own for decisions, so the kinds follow the status, the `type` and the `code`. An OpenAI error reaches an OpenAI app as sent. Any other error is written in the envelope with the status of its kind, and a 429 or 503 keeps its `retry-after`.

## Models

The codec reads and writes OpenAI's listing, as described under [models](openai-chat.md#models) on the OpenAI Chat Completions page.

## Providers that use this format

Each provider has a `DecisionsProfile` in `EgressCtx::decisions_profile`.

### OpenAI

`DecisionsProfile::OPENAI`, `POST /v1/decisions`: at most 400 questions, images in the input and in question instructions. Only `gpt-6-luna` answers decisions.

### Other servers

`DecisionsProfile::GENERIC` checks no question limit. Its path comes from `EgressCtx::with_base_path` or `DecisionsProfile::with_path`. `DecisionsProfile::with_instruction_images(false)` refuses images in instructions for a server that takes them only in the input.
