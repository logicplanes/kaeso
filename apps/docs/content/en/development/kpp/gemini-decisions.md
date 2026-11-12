---
title: Gemini Decisions
description: How every field of Gemini's Decisions API maps to KPP and back.
order: 8
---

Look up any field of Gemini's Decisions API here: where it goes in KPP, what a request from another decision format becomes, and the source of its mapping. Check it before you change how the codec reads or writes that field.

- **Codec:** `kaeso_codecs::gemini_decisions::GeminiDecisions` (`crates/kaeso-codecs/src/gemini_decisions/`)
- **Endpoints:** `POST /v1beta/decisions` and the Gemini [models](#models) endpoints
- **Fixtures:** `crates/kaeso-codecs/fixtures/gemini_decisions/`, spec-built from the guide, the API reference and `google-genai`'s types, none recorded

**Sources.** Google's [Decisions API guide](https://aistudio.google.com/docs/decisions-api) and [Decisions API reference](https://ai.google.dev/api/decisions), read on 2026-11-12. `google-genai` 3.3.0 for Python: `types/decisions` and the Interactions content blocks they reuse.

The gateway doesn't route decision requests yet; the codec is ready for it.

## How to read the tables

- **typed**: a typed KPP field.
- **extras**: `Extras::fields`, bound to `gemini_decisions`; another dialect refuses them unless they are [allowlisted](cross-format.md#allowlist).
- **form**: a codec-private `Extras::form` marker; `verbatim` keeps `null` spellings of declared keys.

## Request: `CreateDecisionRequest`

A decision request decodes to `Request::Decide(DecideRequest)`. Its questions keep the order of the `questions` object, and `answers[i]` of the reply answers `questions[i]`.

| Wire field | KPP | Notes |
|---|---|---|
| `model` | `DecideRequest.model` typed | Required. |
| `input` string | `input = DecisionInput::Text` typed | |
| `input` content block | `input = DecisionInput::Messages` typed | One user `Message` with one block ([content blocks](#content-blocks)); form `object`. |
| `input` list of blocks | `input = DecisionInput::Messages` typed | One user `Message` with the blocks in order. |
| `questions` | `questions` typed | An object keyed by question key. A list is `InvalidRequest` naming `/v1/decisions`. |
| `questions` key | `Question.name` typed | |
| `questions.*.type` | `Question.kind` typed | `noul` is `QuestionKind::Predicate`, `choice` and `score` keep their names. Another type is `InvalidRequest`. |
| `instructions` string | `Instructions::Text` typed | |
| `instructions` text block or list of text blocks | `Instructions::Blocks` typed | Text blocks only; a single block is form `object`. A media block is `InvalidRequest`. |
| `noul` `criteria` | `Predicate` typed | `true_description` and `false_description` are `when_true` and `when_false`; form `criteria` keeps the object when both are missing. |
| `choice` `criteria` | `Choice.options` typed | Each key is a `ChoiceValue::Text`, its string a `Description::Text`, `""` included. A value that isn't a string is `InvalidRequest`. |
| `score` `criteria` | `Score.levels` typed | Each string is a level's `Description::Text`; levels are lowest first. |
| unknown keys | extras | At every level: request, question, `noul` criteria and block. |

A body with `state` and no `input` is a System One request, `InvalidRequest` naming `/v1/systemone`.

### Content blocks

| Block | KPP | Notes |
|---|---|---|
| `text` | `Block::Text` | `annotations` stays in extras. |
| `image`, `audio`, `video`, `document` with `data` | `Block::Image`, `Audio`, `Video`, `Document` with `Source::Base64` | `mime_type` is required with `data`. Base64 that doesn't decode is `InvalidRequest`. |
| the same with `uri` | `Source::FileId` for a Files API URI, `Source::Url` otherwise | `mime_type` is form `mime_type`. Kaeso never fetches a URL. |
| `data` and `uri` together, or neither | none | `InvalidRequest`. |
| `resolution` | extras | On images and videos. A hint, [allowlisted](cross-format.md#allowlist). |
| another `type` | `Block::Unknown` | Bound to `gemini_decisions`; another format refuses it. |

## Writing requests

The encoder writes what KPP holds, and refuses before sending what Gemini's format can't take. A request of another format crosses with these rules; [decisions across formats](decisions-across-formats.md) has the reasons.

| KPP | Written as | Notes |
|---|---|---|
| `Question.name` | the question key | Kept when every question has a name, used once and matching `[A-Za-z0-9_-]{1,64}`; otherwise `q0`…`qN-1` for every question, named in `adapted` as `names` (D19). |
| `DecisionInput::Json` | the input string, its JSON as written | Google's guide asks for JSON as text (D13); no adaptation is named. |
| `Block::Json` | a text block with its JSON as written | |
| an asset reference in a string | the string cut around it, the image an `image` block in its place | In the input string and in strings of a state list; the text of one list item and the next is joined with a line break. Anywhere else it is `Unsupported` (`image reference`), in instructions `image in question instructions`, in descriptions `image in criteria`. |
| `Instructions::Json`, `Description::Json` | their JSON as written, as text | Named in `adapted` as `instructions` or `criteria`. |
| `Instructions` missing | `""` | `instructions` is required; a request in its own format keeps it absent. |
| `ChoiceValue::Bool` | `"true"`, `"false"` | Named in `adapted` as `booleans` (D18); a question with both `true` and `"true"` is `Unsupported`. |
| an option key Google refuses | none | `Unsupported` (`option key`): more than 128 characters or 512 bytes, a space at either end, a control character or a non-ASCII space. |
| `ChoiceOption.description` missing | `""` | |
| `Level` with label and description | `"label: description"` | Named in `adapted` as `levels`. A level without text is `Unsupported` (`score level`). |
| `Source::Base64`, data URL | `data` and `mime_type` | |
| `Source::Url` | `uri` and `mime_type` | `https` only. The MIME type comes from the file extension, as for [Gemini](gemini.md); without one the image is `Unsupported`. `http`, `gs://` and YouTube URLs are `Unsupported`. |
| `Source::FileId` of Gemini | `uri` | Other providers' file ids are `Unsupported`. |
| `Source::Text` | none | `Unsupported` (`text document`): send the text as a text block. |
| OpenAI's `detail` `low` or `high` | `resolution` | `auto` and `original` are left out. |
| images in instructions, refusals, tool items | none | `Unsupported`, naming the part. |
| `QuestionKind::Box` | none | `Unsupported` (`box question`). |
| fewer than 2 or more than 255 options or levels | none | `Unsupported` (`options`, `levels`). |
| more than 1,000 questions | none | `Unsupported` (`questions`) (D5). |
| a conversation | none | `Unsupported` (`conversation input`) (D12). |
| `cache`, `user`, cache breakpoints | none | Left out, named in `left_out` as `cache` and `user`. |

`EgressCtx` keeps the key and the option keys of every question sent, and the model, so the reply is read back by position.

## Response: `CreateDecisionResponse`

| Wire field | KPP | Notes |
|---|---|---|
| none | `DecideResponse.model` typed | Gemini's reply names no model; the decoder takes the request's. |
| `answers` | `answers` typed | One per question, by key. A key that wasn't asked, a missing answer or a type that doesn't match the question is a provider error. |
| `noul.noul` | `PredicateAnswer.probability` typed | Google clamps it to 0.01 to 0.99; KPP keeps it as sent. |
| `choice.choice` | `ChoiceAnswer.option` typed | The index of the option with that key. |
| `choice.probabilities` | `ChoiceAnswer.probabilities` typed | By option; `None` where the reply has no entry. |
| `score.score` | `ScoreAnswer.score` typed | |
| `score.probabilities` | `ScoreAnswer.probabilities` typed | By level index. |
| `score.legend` | form `legend` | Rebuilt from the request's levels when the reply crosses formats. |
| `confidence` | `Answer.confidence` typed | See [confidence](#confidence). |
| `usage.total_input_tokens` | `usage.input_tokens` typed | Less `total_cached_tokens`, which is `usage.cache_read_tokens`. |
| `usage.input_tokens_by_modality` | `usage.input_image_tokens`, `input_audio_tokens` typed | Image, video and document tokens count as image tokens, as Google's guide counts them. |
| `usage.total_output_tokens` | `usage.output_tokens` typed | 0 at Google. |
| `usage` | `usage.raw` | The whole usage comes back in the same format. |
| `prompt_feedback.block_reason` | none | The input was blocked: `ContentFiltered`, see [errors](#errors). Without a reason, `prompt_feedback` stays in extras. |
| numbers written as integers | form `integers` | Written as integers again while the value is unchanged. |
| unknown keys | extras | At every level. |

The encoder writes the answers under the app's keys, in question order, and takes every option key and level from the app's request in `EncodeCtx::request`. It writes no `model`. A reply from another format gets `total_input_tokens` with the cache counted, `input_tokens_by_modality` with text as the rest, `total_output_tokens` as reported and `total_tokens`.

### Confidence

A reply from Gemini keeps its `confidence` as sent. An answer from another format gets Google's rule: the top probability for a choice and a score, `max(p, 1 − p)` for a yes/no question (D8). `confidence` is always written, since Google's SDK requires it.

### Refusals

Gemini's format has no refusal answer. A reply with refusals from OpenAI reaches a Gemini app as an error, `ContentFiltered` with a message that names the refused questions (D9).

## Streaming

Decisions don't stream. The stream encoder returns `Unsupported` (`streaming`) for every event; the stream decoder reports one `StreamEvent::Error` with `Unsupported` (`streaming`) and then nothing.

## Errors

Errors use Google's envelope, read and written as on [Gemini](gemini.md#errors). A blocked input is a 400 whose `code` is a safety code (`safety`, `prohibited_content`, `blocklist`, `image_safety`, `content_blocked`) instead of the HTTP status. KPP reads it as `ContentFiltered`. A reply with `prompt_feedback.block_reason` answers nothing, so it is `ContentFiltered` too (D10).

A Gemini error reaches a Gemini app as sent. Any other `ContentFiltered` is written as `{"error": {"code", "message"}}` with status 400. Its code is the block reason's code, else `safety`. Every other error gets Google's envelope with the status of its kind.

## Models

The codec reads and writes Gemini's listing, as described under [models](gemini.md#models) on the Gemini page.

## Providers that use this format

### Google AI Studio

`POST /v1beta/decisions`, `gemini-decision-1` only: 1 to 1,000 questions, 2 to 255 options and levels, text, image, audio, video and PDF input. `EgressCtx::with_base_path` sends to another path. Gemini Enterprise Agent Platform doesn't serve decisions yet (D21).
