---
title: System One
description: How every field of TypeSafe's System One decision format maps to KPP and back.
order: 8
---

Look up any field of TypeSafe's System One format here: where it goes in KPP, how each provider takes it, and the source of its mapping. Check it before you change how the codec reads or writes that field.

- **Codec:** `kaeso_codecs::system_one::SystemOne` (`crates/kaeso-codecs/src/system_one/`)
- **Endpoints:** `POST /v1/systemone`, as JSON or as a `multipart/form-data` upload, and the OpenAI [models](#models) endpoints
- **Fixtures:** `crates/kaeso-codecs/fixtures/system_one/`, spec-built from TypeSafe's OpenAPI document and documentation, none recorded

**Sources.** TypeSafe's OpenAPI document `https://api.typesafe.ai/openapi.json`, version 0.2.0 (`SystemOneRequest`, `Question`, `NoulCriteria`, `Asset`, `SystemOneResponse`, `Answer`, `HTTPBadRequestError`, `HTTPValidationError`), and its pages on [System One](https://docs.typesafe.ai/api/system-one), [images](https://docs.typesafe.ai/images), [errors](https://docs.typesafe.ai/api/errors) and [confidence](https://docs.typesafe.ai/confidence), read on 2026-11-12.

The gateway doesn't route decision requests yet; the codec is ready for it.

## How to read the tables

- **typed**: a typed KPP field.
- **extras**: `Extras::fields`, bound to `system_one`; another dialect refuses them ([extras](cross-format.md#extras)).
- **form**: a codec-private `Extras::form` marker; `verbatim` keeps `null` spellings of declared keys.

## Request: `SystemOneRequest`

A decision request decodes to `Request::Decide(DecideRequest)`. Its questions keep their order, and `answers[i]` of the reply answers `questions[i]`.

| Wire field | KPP | Notes |
|---|---|---|
| `model` | `DecideRequest.model` typed | Required. |
| `state` string | `input = DecisionInput::Text` typed | |
| `state` object or array | `input = DecisionInput::Json` typed | Kept byte for byte, key order included. |
| `state` with `image_url` parts | `input = DecisionInput::Json` and `assets` typed | Each part, at any depth, becomes an asset under a new reference, `kaeso-image-<n>-ref`, written where the part stood; the part's own fields stay extras of the asset, form `state_part`. Perplexity, OpenRouter and Baseten take images this way ([images](#images)). |
| `questions` | `questions` typed | An object of named questions; anything else is `InvalidRequest`. Each key is `Question.name`. |
| `questions.*.type` | `Question.kind` typed | `noul` is `Predicate`, `choice`, `score` and `box` keep their names. Another type is `InvalidRequest`. |
| `questions.*.instructions` string | `Instructions::Text` typed | |
| `questions.*.instructions` object or array | `Instructions::Json` typed | |
| `questions.*.instructions` `null` | none, form (`verbatim`) | |
| noul `criteria` object | `Predicate.when_true`, `.when_false` typed | `true` and `false` as `Description::Text` or `Description::Json`; `null` is form (`verbatim`). The object itself is form `criteria_object`, so `{}` comes back. |
| noul `criteria` `null` | form (`verbatim`) | |
| choice `criteria` object | `Choice.options` typed | Each key is `ChoiceValue::Text`, in order; its description as for noul. A `null` description is form `null` on the option. |
| score `criteria` array | `Score.levels` typed | Each item is the level's `description`; level `i` scores `i`. |
| box `criteria` | extras | A box question has no criteria; one sent anyway stays on the question. |
| `assets` | `assets` typed | Keyed by reference, in order. |
| `assets.*.type` | none | Must be `image`, else `InvalidRequest`; absent is form `no_type`. |
| `assets.*.format` | the source's media type | `png` is `image/png`, `jpg` and `jpeg` are `image/jpeg`, kept as written in form `format`. Another value is `InvalidRequest`. |
| `assets.*.data_base64` | `Asset.source = Source::Base64` typed | Needs `format`. Text that isn't base64 is `InvalidRequest`. |
| `assets.*.part` | `Asset.source = Source::Base64` typed | In an upload only: the bytes of the named part, a slice of the body. Form `part`, `part_type` and `filename`. A `data_base64` beside it stays extras, as TypeSafe ignores it. |
| unknown keys | extras | At every level: request, question, criteria, asset. |
| header `typesafe-danger-zone-images` | extras `header:typesafe-danger-zone-images` | Forwarded to System One only. |
| header `typesafe-danger-zone-long-context` | extras `header:typesafe-danger-zone-long-context` | Forwarded to System One only. |

An upload's first part is `request`, the JSON request. Every other part is an image that exactly one asset names in `part`, with the type `image/png`, `image/jpeg`, `image/jpg`, `application/octet-stream` or none. A missing, unused or repeated part, or another type, is `InvalidRequest`. Uploads are re-sent as uploads only to TypeSafe; other providers get the images as base64.

`DecideRequest::validate()` asks for a model, at least one question, at least one option and level, names used once, and every asset referenced from a string of the state, the instructions or the descriptions. Object keys never count, as TypeSafe doesn't replace them. `decode_request` refuses requests of every other kind.

## Images

Every image is an asset that strings reference. Where it goes depends on the provider's `SystemOneProfile` in `EgressCtx::system_one_profile`:

| `MediaPlacement` | Written as | Refused |
|---|---|---|
| `Assets` | `assets` with `data_base64`, or the upload's parts again | Image URLs |
| `State` | An `image_url` part with a data URL in place of every whole-string reference in the state | A reference inside a longer string, in instructions or in descriptions |
| `StateOrAssets` | Parts where the app sent parts, `assets` for the rest | Image URLs in `assets` |
| `Conversation` | A user message under `state.messages`: each string a `text` part, each image an `image_url` part | Images in any other state than text, a list of strings or `{"messages": …}` |
| `Root` | A top-level `images` list of data URLs, taken out of a state array in order | A reference anywhere but as a whole item of a state array |
| `None` | Nothing | Every image |

Before sending, the codec reads each image's format and size from its header without decoding it (`kaeso_codecs::image_size`) and refuses what the profile refuses. Images are never converted or resized. The refusals name `image URL`, `image format`, `image size` or `animated image`, with the field in `param`.

## Questions and options

| KPP | Written as | Notes |
|---|---|---|
| `Question.name` | the question's key | Kept when every question has a name, used once and valid for the provider; otherwise every question is sent as `q0`…`qN-1`, named in `adapted` as `names`. The reply is read back by position. |
| `Instructions::Blocks` | a string, or an array of strings, JSON values and image references | Images become assets. |
| `ChoiceValue::Bool` | `"true"`, `"false"` | Named in `adapted` as `booleans`. `true` and `"true"` in one question are `Unsupported` (`repeated choice value`). |
| `Level` with `label` and a text `description` | `"label: description"` | Named in `adapted` as `levels`. A label alone is written as the level. |
| `QuestionKind::Box` | `box` | `Unsupported` (`box question`) where the profile answers none. |
| `DecideRequest.cache`, `Block.cache` | none | Hints System One has no place for, named in `left_out` as `cache`. |
| `DecideRequest.user` | none | A hint, named in `left_out` as `user`. |
| `DecisionInput::Messages` | the state | One user message only; its text, JSON and image blocks become a string or an array. More messages are `Unsupported` (`conversation input`). |

## Response: `SystemOneResponse`

The decoder reads each answer under the key its question was sent with, from `EgressCtx`. An answer to a question that wasn't asked, a missing answer, an option or level that wasn't asked, or an answer of another type is a provider error.

| Wire field | KPP | Notes |
|---|---|---|
| `model` | `DecideResponse.model` typed | The model that answered, such as `jev-1.14.0` for `jev-latest`. |
| `answers.*` noul | `AnswerValue::Predicate(PredicateAnswer { probability })` typed | `noul` is the probability of yes. |
| `answers.*` choice | `AnswerValue::Choice(ChoiceAnswer { option, probabilities })` typed | `option` is the chosen option's index; `probabilities` has one entry per option, `None` where the reply has none. |
| `answers.*` score | `AnswerValue::Score(ScoreAnswer { score, probabilities })` typed | `probabilities` by level, from the keys `"0"`, `"1"`, …. |
| `answers.*.legend` | form `legend` | Rebuilt from the request's levels when the reply crosses formats. |
| `answers.*` box | `AnswerValue::Box(BoxAnswer)` typed | Edges as fractions; `confidence: null` is form `null_confidence`. |
| `answers.*.confidence` | `Answer.confidence` typed | See [confidence](#confidence). |
| `answers.*` refusal | `AnswerValue::Refusal` typed | TypeSafe never sends one; written for an answer another format refused. |
| numbers written as integers | form `integers` | Written as integers again while the value is unchanged. |
| `usage.input_tokens` | `usage.input_tokens` typed | Images included. |
| `usage.output_tokens` | `usage.output_tokens` typed | TypeSafe counts them and doesn't charge for them. |
| `usage` | `usage.raw` | Other keys, such as OpenRouter's `cost`, come back in the same format. |
| unknown keys | extras | At every level. |

Answers are written in question order under the app's own keys, which the encoder takes from the app's request in `EncodeCtx::request`.

### Confidence

A reply from a System One provider keeps its `confidence` as sent. An answer that comes from another format gets TypeSafe's [documented formulas](https://docs.typesafe.ai/confidence): `(p_max − 1/n) / (1 − 1/n)` for a choice and `max(0, 1 − Σ p_i·|i − m| / MAD_unif)` for a score, 1 for one option or level. A yes/no answer carries none, and a box keeps the provider's or `null`.

Perplexity computes a score's confidence against the distances from the top level instead of from the middle, so its values differ from TypeSafe's. Its replies keep them as sent.

## Streaming

Decisions don't stream. The stream encoder returns `Unsupported` (`streaming`) for every event; the stream decoder reports one `StreamEvent::Error` with `Unsupported` (`streaming`) and then nothing.

## Errors

| Status | Body | KPP |
|---|---|---|
| 400 | `{"detail": "…"}` or `{"detail": {"message": …}}` | `InvalidRequest`; `ContextTooLong` when the body names `max_tokens_exceeded` |
| 401 | `{"detail": …}` | `Authentication` |
| 413 | `{"detail": …}` | `RequestTooLarge` |
| 415 | `{"detail": …}` | `InvalidRequest` |
| 422 | `{"detail": [{"loc", "msg", "type", "input"?, "ctx"?}]}` | `InvalidRequest`, the first problem's location without `body` in `param` |
| 429 | `{"detail": …}`, `retry-after`, `retry-after-ms` | `RateLimited` with its wait |
| 529 | `{"detail": …}` | `Overloaded` |

A body without `detail` is read like an OpenAI error, which covers OpenRouter's and Perplexity's `error` envelope and Cloudflare's. A System One error reaches a System One app as sent. Any other error is written as `{"detail": message}` with the status of its kind: 422 with `detail[{loc, msg, type: "value_error"}]` for an `InvalidRequest` with a `param`, 400 for other invalid or unsupported requests, 402 for billing, 529 for an overloaded provider.

## Models

TypeSafe's `GET /v1/models` answers `{"models": [{name, description, release_date}]}`, and no header tells TypeSafe's SDK from OpenAI's. This codec reads and writes OpenAI's listing, as described under [models](openai-chat.md#models) on the OpenAI Chat Completions page.

## Providers that use this format

Each provider has a `SystemOneProfile`, and providers other than TypeSafe keep their fixtures in `fixtures/system_one/providers/<id>/` ([fixtures](fixtures.md)).

### TypeSafe

`SystemOneProfile::TYPESAFE`, `POST /v1/systemone`. Images go in `assets` or as upload parts, PNG and JPEG of 280 to 1568 pixels a side and at most 16 MiB, still images only, with `TypeSafe-Danger-Zone-Images: true` whenever a request has an image or a box question. Box questions need Jev 1.14. The long-context header goes only when the app sent it. Up to 255 options and 10 levels.

### OpenRouter

`SystemOneProfile::OPENROUTER`, `POST /v1/systemone` under `https://openrouter.ai/api`. Sources: the [Jev guide](https://openrouter.ai/docs/guides/community/jev), the [Decisions API reference](https://openrouter.ai/docs/api/api-reference/alphadecisions/submit-a-decisions-request) and the multimodal decisions guide, read on 2026-11-12. OpenRouter takes images as `image_url` parts in the state and as `assets`, but not as uploads. Its reply adds `id`, `provider` and `usage.cost`, which stay in extras.

### Perplexity

`SystemOneProfile::PERPLEXITY`, `POST /v1/decisions` with a System One body. Source: the [Decisions API quickstart](https://docs.perplexity.ai/docs/decisions/quickstart), read on 2026-11-12. Images are `image_url` parts with PNG, JPEG or WebP data URLs anywhere in the state, at most 8. Perplexity answers no box questions and refuses a top-level field it doesn't document, so an app's own fields are `Unsupported` before sending. `mode` and `max_staleness` aren't sent.

### Baseten

`SystemOneProfile::BASETEN`, `POST /v1/systemone`. Source: the [System One reference](https://docs.baseten.co/reference/inference-api/system-one), read on 2026-11-12. Images go in a user message under `state.messages`, PNG, JPEG or WebP, at most 16; `service_tier` stays in extras. No box questions.

### Fireworks AI

`SystemOneProfile::FIREWORKS`, `POST /v1/systemone` under `https://api.fireworks.ai/inference`, text only and no box questions. The model is in private preview, so the profile follows LiteLLM's provider page rather than Fireworks' own.

### Cloudflare Workers AI

`SystemOneProfile::CLOUDFLARE_WORKERS_AI`, `POST /ai/run/@cf/cloudflare/{model}`, from the [Clef model page](https://developers.cloudflare.com/workers-ai/models/clef/). The body names the model by its short name, such as `clef`. A request takes at most 64 questions with keys of letters, digits, `_`, `.` and `-`, and at most 4 PNG, JPEG or WebP images of 4 MiB in a top-level `images` list. The reply is the `result` of Cloudflare's envelope. The gateway doesn't send to it yet.

### Other servers

`SystemOneProfile::GENERIC` takes every question type, sends image URLs where its placement writes parts, and checks no image limits. Its path comes from `EgressCtx::with_base_path`, and its image placement from `SystemOneProfile::with_media`, for vLLM, llama.cpp and other servers of the format.
