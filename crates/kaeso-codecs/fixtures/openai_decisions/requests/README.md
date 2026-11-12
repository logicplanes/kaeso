# `fixtures/openai_decisions/requests`

| file | what it covers | provenance |
|---|---|---|
| `predicate.json` | a predicate about a text input | doc: https://developers.openai.com/api/reference/resources/decisions/methods/create (curl example, verbatim) |
| `choice.json` | a choice with four described options | doc: https://developers.openai.com/api/docs/guides/decisions ("Select from fixed options", verbatim) |
| `score.json` | a score with three labelled and described levels | doc: https://developers.openai.com/api/docs/guides/decisions ("Score against a rubric", verbatim) |
| `image.json` | a user message with text and a PNG data URL | doc: https://developers.openai.com/api/docs/guides/decisions ("Check an image for visible damage"), with a real 2 × 2 PNG |
| `input-json.json` | a user message with text and an `input_json` part | doc: https://developers.openai.com/api/docs/guides/decisions ("Evaluate JSON data", verbatim) |
| `instructions-images.json` | an image URL in the input and one in a question's instructions | doc: https://developers.openai.com/api/docs/guides/decisions ("Include images in question instructions", verbatim) |
| `instructions-parts.json` | instructions of `input_text`, `input_image` with `detail` and `input_json` parts; options without descriptions; levels without descriptions | schema: `DecisionQuestionInputPart`; sdk: `decision_create_params.py` (`QuestionQuestionParamChoiceChoice`, `QuestionQuestionParamScoreLevel`) |
| `booleans.json` | boolean choice values beside strings | schema: `choices[].value` ("string or boolean") |
| `null-names.json` | questions without a name | schema: `questions[].name` (optional) |
| `repeated-names.json` | two questions with the same name | schema: `questions[].name` (no uniqueness rule) |
| `conversation.json` | developer, system, user and assistant messages (`phase`, `output_text`, `refusal`), `function_call` with `id` and `status`, `function_call_output` with parts, `custom_tool_call` with `namespace`, `custom_tool_call_output`; `type: "message"` on one message | doc: https://developers.openai.com/api/reference/resources/decisions/methods/create (input items); sdk: `InputUnionMember1*` |
| `cache.json` | `prompt_cache_key`, `prompt_cache_options`, `safety_identifier`, `prompt_cache_breakpoint` on a text and an image part | doc: https://developers.openai.com/api/reference/resources/decisions/methods/create; doc: https://developers.openai.com/api/docs/guides/decisions ("Reuse shared input with prompt caching") |
| `refusal.json` | two questions, one of which the reply refuses | schema: `Decision.answers` |
| `unknown-top-level-field.json` | MADE-UP top-level field `x_future_field` (Extras) | schema: constructed; field invented on purpose (unknown-* rule) |
| `unknown-fields.json` | MADE-UP fields `x_note` on a part, `x_tag` on a choice and `x_weight` on a question (Extras) | schema: constructed; fields invented on purpose (unknown-* rule) |
