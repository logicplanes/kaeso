# `fixtures/openai_responses/responses`

| file | what it covers | provenance |
|---|---|---|
| `failed.json` | Response object with `status: failed` and `error {code, message}` (shape of a failed response, e.g. when retrieved after background mode) | schema: Response, ResponseError, ResponseErrorCode (openapi.yaml@e95c0fe615f4) |
| `file-input-base64-file-id.json` | answer to the PDF request; `reasoning.effort: low` echoed | schema: Response, OutputMessage, ResponseUsage (openapi.yaml@e95c0fe615f4) |
| `file-input-url.json` | `input_file` with `file_url` + `detail`; Response with `background`, `max_tool_calls`, `service_tier`, `top_logprobs` | doc: https://developers.openai.com/api/reference/resources/responses/methods/create (example "File input" response, verbatim) |
| `image-base64-file-id.json` | reasoning item with empty `summary` (no encrypted content, store default) followed by message | schema: Response, ReasoningItem, OutputMessage, ResponseUsage (openapi.yaml@e95c0fe615f4) |
| `image-url.json` | message item without `type`, `input_image` by URL (no `detail`, as in the doc) | doc: https://developers.openai.com/api/reference/resources/responses/methods/create (example "Image input" response, verbatim) |
| `json-object-mode.json` | non-reasoning model (`reasoning` echo all null), JSON mode output | schema: Response (openapi.yaml@e95c0fe615f4) |
| `max-output-tokens-incomplete.json` | `status: incomplete`, `incomplete_details.reason: max_output_tokens`, message `status: incomplete`, `completed_at: null` | schema: Response.incomplete_details (openapi.yaml@e95c0fe615f4) |
| `multi-turn-roles.json` | reply to multi-turn request; `text.verbosity: low` echoed; non-ASCII text (°) | schema: Response, OutputMessage (openapi.yaml@e95c0fe615f4) |
| `reasoning-effort-high.json` | `reasoning.effort: high`; usage with `reasoning_tokens` | doc: https://developers.openai.com/api/reference/resources/responses/methods/create (example "Reasoning" response, verbatim) |
| `reasoning-encrypted.json` | reasoning item with `summary` + `encrypted_content`; usage with `cached_tokens` and `reasoning_tokens`; `store: false` echoed | schema: Response, ReasoningItem, ResponseUsage (openapi.yaml@e95c0fe615f4) |
| `refusal.json` | message with a `refusal` content part | schema: Response, RefusalContent (openapi.yaml@e95c0fe615f4) |
| `sampling-and-ids.json` | echo of sampling params and ids; `output_text.logprobs` with `bytes` and `top_logprobs`; `service_tier: priority` | schema: Response, OutputTextContent, LogProb, TopLogProb (openapi.yaml@e95c0fe615f4) |
| `structured-output-json-schema.json` | JSON text output; `text.format` echoed | schema: Response, TextResponseFormatJsonSchema (openapi.yaml@e95c0fe615f4) |
| `text-input-string.json` | plain `input` string; minimal request; full Response object | doc: https://developers.openai.com/api/reference/resources/responses/methods/create (example "Text input" response, verbatim) |
| `tool-result-error.json` | assistant reply after a tool error | schema: Response (openapi.yaml@e95c0fe615f4) |
| `tools-function.json` | function tool without `strict` (echoed as `strict: true`), `tool_choice: "auto"`; single `function_call` output | doc: https://developers.openai.com/api/reference/resources/responses/methods/create (example "Functions" response, verbatim) |
| `tools-parallel.json` | two parallel `function_call` items after a reasoning item | schema: Response, FunctionToolCall (openapi.yaml@e95c0fe615f4) |
| `unknown-output-item.json` | MADE-UP output item type `future_output_item` and top-level `x_future_field` | schema: constructed from Response; the item type and the field are invented on purpose (unknown-* rule) |
| `web-search.json` | `web_search_call` item (action search + sources), message with `url_citation` annotation; web_search tool echoed with defaults | schema: Response, WebSearchToolCall, WebSearchActionSearch, UrlCitationBody (openapi.yaml@e95c0fe615f4) |
