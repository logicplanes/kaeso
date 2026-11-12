# `fixtures/gemini_decisions/requests`

| file | what it covers | provenance |
|---|---|---|
| `noul.json` | a `noul` question about a text input | doc: https://aistudio.google.com/docs/decisions-api (Quickstart, verbatim) |
| `noul-criteria.json` | `noul` with `true_description` and `false_description` | doc: https://aistudio.google.com/docs/decisions-api ("Yes or no", verbatim) |
| `choice.json` | a `choice` with four described options | doc: https://aistudio.google.com/docs/decisions-api ("Pick one option", verbatim) |
| `score.json` | a `score` with three levels | doc: https://aistudio.google.com/docs/decisions-api ("Rate on a scale", verbatim) |
| `multiple-questions.json` | two text blocks as input; `noul`, `choice` with an empty description, and `score` in one request | doc: https://aistudio.google.com/docs/decisions-api ("Ask multiple questions", verbatim) |
| `image.json` | a text block and an inline image with `resolution` | doc: https://aistudio.google.com/docs/decisions-api ("Images"), with a real 2 × 2 PNG instead of the placeholder |
| `audio-uri.json` | an audio block by https URL | doc: https://aistudio.google.com/docs/decisions-api ("Audio and video", verbatim) |
| `video-uri.json` | a video block by https URL with `resolution` | doc: https://aistudio.google.com/docs/decisions-api ("Audio and video", verbatim) |
| `document.json` | an inline PDF document block; an option key with an inner space | schema: `DocumentContent`; doc: https://aistudio.google.com/docs/decisions-api ("Limits", option key) |
| `files-api.json` | an image by Files API URI | doc: https://aistudio.google.com/docs/decisions-api ("Input": Files API URIs); sdk: `ImageContent.uri` |
| `input-object.json` | `input` as one content block | sdk: `Input = Union[DecisionInputContent, List[DecisionInputContent], str]` |
| `instructions-blocks.json` | instructions as one text block and as a list of text blocks; options described with `""` | sdk: `NoulQuestionInstructions`, `ChoiceQuestionInstructions` |
| `blocked.json` | the request `responses/blocked.json` answers | schema: constructed |
| `unknown-top-level-field.json` | MADE-UP top-level field `x_future_field` (Extras) | schema: constructed; field invented on purpose (unknown-* rule) |
| `unknown-fields.json` | MADE-UP fields `x_note` on a block, `x_weight` on a question and `x_hint` in `noul` criteria (Extras) | schema: constructed; fields invented on purpose (unknown-* rule) |
