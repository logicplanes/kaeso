# `fixtures/openai_decisions/responses`

Each reply answers the request of the same name in `requests/`.

| file | what it covers | provenance |
|---|---|---|
| `predicate.json` | a predicate answer and usage | doc: https://developers.openai.com/api/reference/resources/decisions/methods/create (response example, verbatim) |
| `choice.json` | a choice with probabilities and `confidence` | doc: https://developers.openai.com/api/docs/guides/decisions (response excerpt, with `model` and `usage` added) |
| `score.json` | a score with labelled probabilities and `confidence` | doc: https://developers.openai.com/api/docs/guides/decisions (response excerpt, with `model` and `usage` added) |
| `image.json` | a predicate about an image | doc: https://developers.openai.com/api/docs/guides/decisions (response excerpt, with `model` and `usage` added) |
| `input-json.json` | a predicate about JSON evidence | schema: `Decision` |
| `instructions-images.json` | a predicate about images in instructions | schema: `Decision` |
| `instructions-parts.json` | a choice and a score | schema: `Decision` |
| `booleans.json` | boolean choices and boolean probability values | schema: `AnswerAnswerResourceChoice` (`choice`, `value` as string or boolean) |
| `null-names.json` | answers with `name: null` | sdk: `decision.py` (`name: Optional[str]`) |
| `repeated-names.json` | two answers with the same name | schema: `Decision` |
| `conversation.json` | a predicate and a choice about a conversation | schema: `Decision` |
| `cache.json` | usage with `cached_tokens` and `cache_write_tokens`, both counted in `input_tokens` | doc: https://developers.openai.com/api/docs/guides/decisions ("Check `usage.input_tokens_details`"); schema: `Usage` |
| `refusal.json` | a refusal beside a choice answer | doc: https://developers.openai.com/api/reference/resources/decisions/methods/create (`Refusal`) |
| `unknown-top-level-field.json` | the predicate reply to `unknown-top-level-field` | doc: https://developers.openai.com/api/reference/resources/decisions/methods/create (response example) |
| `unknown-fields.json` | MADE-UP answer field `x_rationale_id` and top-level field `x_trace` (Extras) | schema: constructed; fields invented on purpose (unknown-* rule) |
