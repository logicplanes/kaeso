# `fixtures/openai_responses/requests/input_tokens`

| file | what it covers | provenance |
|---|---|---|
| `items-image-reasoning.json` | item list with image, reasoning item (encrypted), function_call/output, `text.format` json_schema | schema: TokenCountsBody, InputItem (openapi.yaml@e95c0fe615f4) |
| `string-input.json` | string input | doc: https://developers.openai.com/api/reference/resources/responses/subresources/input_tokens/methods/count (example request, same as openapi.yaml) |
| `tools-and-instructions.json` | `instructions`, tools, `tool_choice`, `parallel_tool_calls`, `reasoning`, `text`, `truncation` | schema: TokenCountsBody (openapi.yaml@e95c0fe615f4) |
