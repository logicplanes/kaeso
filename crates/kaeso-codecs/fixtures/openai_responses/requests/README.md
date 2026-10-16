# `fixtures/openai_responses/requests`

| file | what it covers | provenance |
|---|---|---|
| `error-mid-stream.json` | plain streaming request | schema: CreateResponse (openapi.yaml@e95c0fe615f4) |
| `explicit-nulls.json` | every nullable field set to explicit `null` (incl. `previous_response_id`/`conversation`/`background`: null must NOT be treated as stateful); `tools: []` | schema: CreateResponse (nullable anyOf members) (openapi.yaml@e95c0fe615f4) |
| `failed-stream.json` | plain request whose stream fails | schema: CreateResponse (openapi.yaml@e95c0fe615f4) |
| `file-input-base64-file-id.json` | `input_file` as base64 PDF data URL with `filename`, and as `file_id` with `detail` | schema: CreateResponse, InputFileContent (openapi.yaml@e95c0fe615f4) |
| `file-input-url.json` | `input_file` with `file_url` + `detail`; Response with `background`, `max_tool_calls`, `service_tier`, `top_logprobs` | doc: https://developers.openai.com/api/reference/resources/responses/methods/create (example "File input"; same text as openapi.yaml createResponse x-oaiMeta) |
| `function-call-output-array.json` | `function_call_output.output` as an array of `input_text`, `input_image`, `input_file` | schema: FunctionCallOutputItemParam (output oneOf array of InputTextContentParam / InputImageContentParamAutoParam / InputFileContentParam) (openapi.yaml@e95c0fe615f4) |
| `function-call-stream.json` | `tool_choice` forcing a function (`{type: function, name}`), `reasoning.effort: minimal`, streaming | schema: CreateResponse, ToolChoiceFunction (openapi.yaml@e95c0fe615f4) |
| `image-base64-file-id.json` | `input_image` as base64 data URL (`detail: low`), as `file_id` (`detail: high`) and as URL (`detail: original`); message item with explicit `type` | schema: CreateResponse, InputMessage, InputImageContent (openapi.yaml@e95c0fe615f4) |
| `image-url.json` | message item without `type`, `input_image` by URL (no `detail`, as in the doc) | doc: https://developers.openai.com/api/reference/resources/responses/methods/create (example "Image input"; same text as openapi.yaml createResponse x-oaiMeta) |
| `item-reference-breakpoint.json` | `item_reference` input item (kept as an Unknown item), `input_text` with `prompt_cache_breakpoint` (explicit cache hint), assistant message with `phase`, `prompt_cache_options` | schema: ItemReferenceParam, InputTextContent.prompt_cache_breakpoint (PromptCacheBreakpointConfig), MessagePhase, PromptCacheOptionsParam (`ttl: 30m`, the only value) (openapi.yaml@e95c0fe615f4) |
| `json-object-mode.json` | `text.format` `json_object` (JSON mode), `temperature: 0` | schema: ResponseFormatJsonObject (openapi.yaml@e95c0fe615f4) |
| `logprobs-no-obfuscation-stream.json` | `stream_options.include_obfuscation: false`, logprobs in stream | schema: ResponseStreamOptions (openapi.yaml@e95c0fe615f4) |
| `max-output-tokens-incomplete-stream.json` | same with `stream: true` | schema: CreateResponse (openapi.yaml@e95c0fe615f4) |
| `max-output-tokens-incomplete.json` | `max_output_tokens` at its minimum (16) | schema: CreateResponse.max_output_tokens (openapi.yaml@e95c0fe615f4) |
| `multi-turn-roles.json` | item list with roles system/developer/user/assistant; `content` as string AND as parts; messages with and without `type`; assistant output message fed back (`id`, `status`, `output_text` with `annotations: []`); non-leading developer message; `text.verbosity` | schema: InputParam, EasyInputMessage, InputMessage, OutputMessage, ResponseTextParam (openapi.yaml@e95c0fe615f4) |
| `reasoning-effort-high.json` | `reasoning.effort: high`; usage with `reasoning_tokens` | doc: https://developers.openai.com/api/reference/resources/responses/methods/create (example "Reasoning"; same text as openapi.yaml createResponse x-oaiMeta) |
| `reasoning-encrypted.json` | stateless reasoning round trip: `store: false`, `include: ["reasoning.encrypted_content"]`, `reasoning.effort`/`summary`; reasoning item fed back with `summary` + `encrypted_content`, then function_call + function_call_output | schema: CreateResponse, ReasoningItem, IncludeEnum, Reasoning (openapi.yaml@e95c0fe615f4) |
| `reasoning-text-stream.json` | open-weight reasoning model streaming `reasoning_text` content | schema: CreateResponse (openapi.yaml@e95c0fe615f4) |
| `reasoning-summary-stream.json` | reasoning with summaries + encrypted content, streaming | schema: CreateResponse, Reasoning (openapi.yaml@e95c0fe615f4) |
| `reasoning-summary-sampling-forced-tool.json` | reasoning `effort: low` + `summary: detailed` together with `temperature`, `top_p`, `tool_choice: required` and a `max_output_tokens` below Anthropic's minimum thinking budget: what Anthropic forbids next to thinking (kpp R30, R31, D4) | schema: CreateResponse, Reasoning, ToolChoiceOptions (openapi.yaml@e95c0fe615f4) |
| `refusal-stream.json` | refusal request with `stream: true` | schema: CreateResponse (openapi.yaml@e95c0fe615f4) |
| `refusal.json` | structured-output request that the model refuses | schema: CreateResponse (openapi.yaml@e95c0fe615f4) |
| `sampling-and-ids.json` | `temperature`, `top_p`, `top_logprobs` + `include` logprobs, `max_output_tokens`, `truncation: auto`, `store: false`, `prompt_cache_key`, `safety_identifier`, `user`, `metadata`, `service_tier: priority` | schema: CreateResponse, ModelResponseProperties, ServiceTierResponses (openapi.yaml@e95c0fe615f4) |
| `stateful-background.json` | `background: true` with `store: true` (Unsupported, D7) | schema: ResponseProperties.background (openapi.yaml@e95c0fe615f4) |
| `stateful-conversation-id-string.json` | `conversation` string form (Unsupported, D7) | schema: ConversationParam (openapi.yaml@e95c0fe615f4) |
| `stateful-conversation.json` | `conversation` object form (Unsupported, D7) | schema: ConversationParam (openapi.yaml@e95c0fe615f4) |
| `stateful-previous-response-id.json` | `previous_response_id` (codec must reject: Unsupported, D7) | schema: ResponseProperties.previous_response_id (openapi.yaml@e95c0fe615f4) |
| `structured-output-json-schema.json` | `text.format` `json_schema` with `strict: true`, `name`, `description` | schema: TextResponseFormatJsonSchema; content modelled on the structured-outputs guide (openapi.yaml@e95c0fe615f4) |
| `text-basic-stream.json` | `instructions` + string input + `stream: true` | doc: https://developers.openai.com/api/reference/resources/responses/methods/create (example "Streaming") |
| `text-input-string.json` | plain `input` string; minimal request; full Response object | doc: https://developers.openai.com/api/reference/resources/responses/methods/create (example "Text input"; same text as openapi.yaml createResponse x-oaiMeta) |
| `tool-choice-allowed-tools.json` | `tool_choice` `allowed_tools` (mode required) mixing a function and a built-in tool; `parallel_tool_calls: false` | schema: ToolChoiceAllowed (openapi.yaml@e95c0fe615f4) |
| `tool-choice-none.json` | `tool_choice: "none"` with tools present | schema: ToolChoiceOptions (openapi.yaml@e95c0fe615f4) |
| `tool-result-error.json` | `function_call` fed back + `function_call_output` with string output describing a tool error | schema: FunctionToolCall, FunctionCallOutputItemParam (openapi.yaml@e95c0fe615f4) |
| `tools-function.json` | function tool without `strict` (echoed as `strict: true`), `tool_choice: "auto"`; single `function_call` output | doc: https://developers.openai.com/api/reference/resources/responses/methods/create (example "Functions"; same text as openapi.yaml createResponse x-oaiMeta) |
| `tools-parallel-stream.json` | same as tools-parallel with `stream: true` | schema: CreateResponse (openapi.yaml@e95c0fe615f4) |
| `tools-parallel.json` | two strict function tools, `tool_choice: "required"`, `parallel_tool_calls: true` | schema: CreateResponse, FunctionTool, ToolChoiceOptions (openapi.yaml@e95c0fe615f4) |
| `unknown-event-stream.json` | plain streaming request | schema: CreateResponse (openapi.yaml@e95c0fe615f4) |
| `unknown-item-type.json` | MADE-UP input item type `future_item` (must become an Unknown block) | schema: constructed; item type and field invented on purpose (unknown-* rule) |
| `unknown-top-level-field.json` | MADE-UP top-level request field `x_future_field` (must land in Extras) | schema: constructed; field invented on purpose (unknown-* rule) |
| `web-search-stream.json` | web search request with `stream: true` | schema: CreateResponse (openapi.yaml@e95c0fe615f4) |
| `web-search.json` | built-in `web_search` tool (Unknown/Extras carrier), `include` sources, `max_tool_calls` | schema: WebSearchTool, IncludeEnum, ResponseProperties.max_tool_calls (openapi.yaml@e95c0fe615f4) |
