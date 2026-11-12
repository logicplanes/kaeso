# `fixtures/system_one/requests`

| file | what it covers | provenance |
|---|---|---|
| `noul.json` | a yes/no question, text state | doc: OpenAPI `SystemOneRequest` example `noul` (openapi.json 0.2.0, read 2026-11-12) |
| `choice.json` | a choice with three described options | doc: OpenAPI example `choice` |
| `score.json` | a score with three levels | doc: OpenAPI example `score` |
| `box.json`, `box.headers.json` | a box question, its image as a base64 asset, `TypeSafe-Danger-Zone-Images: true` | doc: OpenAPI example `box`, verbatim (16 × 16 PNG) |
| `structured-state.json` | object state; choice criteria as text, object, array and `null` | schema: `SystemOneRequest.state`, `ChoiceQuestion.criteria` |
| `noul-criteria.json` | noul criteria with `true` and `false`, a structured `true` with a `null` `false`, `criteria: null`, only `true`, no instructions | schema: `NoulQuestion`, `NoulCriteria`; statement wording from the schema's examples |
| `instructions-forms.json` | array state; instructions `null`, object and array; score levels as object, array and text; one option, one level | schema: `*.instructions`, `ScoreQuestion.criteria` |
| `long-context.json`, `long-context.headers.json` | `TypeSafe-Danger-Zone-Long-Context: true` | schema: the operation's header parameters |
| `assets.json`, `assets.headers.json` | two images, one as an object value of the state and one mid-sentence in instructions, `format` `jpg` and `png` | doc: https://docs.typesafe.ai/images ("Sending images in a request"), with real 280 × 280 images in place of the shortened base64 |
| `assets-criteria.json`, `assets-criteria.headers.json` | images in choice criteria, as a whole value and in an array; `format` `jpeg`; an asset without `type`; a box question | doc: https://docs.typesafe.ai/images ("Criteria"), with real images and a box question added |
| `multipart/door.json` | the `request` part of an upload whose asset names the part `door-photo`; the tests write the upload around it | doc: https://docs.typesafe.ai/images ("Over HTTP", multipart tab) |
| `unknown-top-level-field.json` | MADE-UP top-level field `x_future_field` (Extras) | schema: constructed; field invented on purpose (unknown-* rule) |
| `unknown-fields.json` | MADE-UP question field `x_weight`; options without descriptions | schema: constructed; field invented on purpose (unknown-* rule) |
