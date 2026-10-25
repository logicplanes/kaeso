# anthropic_messages / responses

Exact non-streaming HTTP 200 bodies. `count_tokens/` holds `POST /v1/messages/count_tokens` responses.

| file | what it covers | provenance |
|---|---|---|
| `text-simple.json` | Plain text, `end_turn`, full usage incl. `cache_creation` breakdown, `service_tier`, `inference_geo`, `stop_details: null`. | schema: Message, Usage (field set as in the SDK's recorded stream fixtures) |
| `system-string-multi-turn.json` | Short multi-turn answer (prompt below the cache minimum, so no cache tokens). | schema: Message |
| `system-blocks-cache-ttl.json` | Cache writes split across TTLs: `cache_creation_input_tokens` = `ephemeral_5m` + `ephemeral_1h`. | schema: Usage, CacheCreation (rule from https://platform.claude.com/docs/en/build-with-claude/prompt-caching#1-hour-cache-duration) |
| `image-base64.json` | Answer about an image. | schema: Message |
| `document-pdf-base64.json` | Text blocks with a `page_location` citation (`file_id: null`). | schema: ResponseTextBlock, ResponsePageLocationCitation |
| `document-text-citations.json` | Text blocks with `char_location` citations (doc response structure); uncited blocks omit `citations`. | schema: ResponseCharLocationCitation (content from https://platform.claude.com/docs/en/build-with-claude/citations#response-structure) |
| `document-custom-content.json` | Text blocks with `content_block_location` citations. | schema: ResponseContentBlockLocationCitation |
| `tools-single.json` | Text + one `tool_use`, `stop_reason: tool_use`. | schema: ResponseToolUseBlock |
| `tools-parallel.json` | Text + four parallel `tool_use` blocks. | schema: ResponseToolUseBlock |
| `tool-result-multi-turn.json` | Final answer after tool results incl. an error result. | schema: Message |
| `tool-choice-tool.json` | Forced tool: content starts with `tool_use`, no text. | schema: Message |
| `thinking-enabled-budget.json` | `thinking` block with readable text + `signature`, then text; `output_tokens_details.thinking_tokens`. | schema: ResponseThinkingBlock, OutputTokensDetails |
| `thinking-adaptive-effort.json` | Adaptive, summarized thinking + text. | schema: ResponseThinkingBlock |
| `thinking-display-omitted.json` | Omitted display: `thinking: ""` with a non-empty `signature`. | schema: ResponseThinkingBlock (behaviour from https://platform.claude.com/docs/en/build-with-claude/thinking#controlling-thinking-display) |
| `thinking-disabled.json` | No thinking blocks. | schema: Message |
| `thinking-redacted.json` | `thinking` + `redacted_thinking` (`data`) + `tool_use`. | schema: ResponseRedactedThinkingBlock |
| `thinking-dropped-block.json` | Reply to a request sent with `thinking.block_binding.prefix_mismatch_behavior: "drop_block"` and the `thinking-binding-controls-2026-08-01` beta: top-level `input_transformations` with a `thinking_dropped` entry (`path`, `reason: prefix_binding_mismatch`). | doc: https://platform.claude.com/docs/en/build-with-claude/preserved-thinking (`input_transformations`, present on every response to a request with the beta), not recorded; content as `thinking-display-omitted.json` |
| `thinking-interleaved-tools.json` | Interleaved: new `thinking` block after a tool result, then a second `tool_use`. | schema: Message |
| `thinking-adaptive-interleaved-tools.json` | Adaptive interleaved, display omitted: empty `thinking` + signature, then `tool_use`. | schema: Message |
| `structured-output-json-schema.json` | JSON text matching the request's schema in a single text block. | schema: Message (JSON from https://platform.claude.com/docs/en/build-with-claude/structured-outputs) |
| `sampling-stop-sequences.json` | `stop_reason: stop_sequence`, `stop_sequence: "END"`. | schema: Message |
| `metadata-service-tier.json` | `usage.service_tier: priority`, `inference_geo: us`. | schema: Usage |
| `web-search-server-tool.json` | `server_tool_use` + `web_search_tool_result` (with `caller: {type: direct}`, one `page_age: null`) + `web_search_result_location` citation; `usage.server_tool_use`. | schema: ResponseServerToolUseBlock, ResponseWebSearchToolResultBlock (content from https://platform.claude.com/docs/en/agents-and-tools/tool-use/web-search-tool#response; `caller` as in sdk: tests/fixtures/fable-fallback/stream-a-toolrefusal.sse@b4b7916deaf4e5570cd627b1ae7ee4394a4de39f) |
| `pause-turn.json` | `stop_reason: pause_turn` ending with an unanswered `server_tool_use`. | schema: Message (semantics from https://platform.claude.com/docs/en/build-with-claude/handling-stop-reasons#pause_turn) |
| `max-tokens.json` | Truncated text, `stop_reason: max_tokens`, `output_tokens` = `max_tokens`. | schema: Message |
| `refusal.json` | `content: []`, `stop_reason: refusal`, `stop_details {type, category: cyber, explanation}`. | doc: https://platform.claude.com/docs/en/build-with-claude/refusals-and-fallback#what-a-refusal-looks-like (usage expanded to the full field set) |
| `unknown-fields.json` | **Made up**: top-level `x_future_field`, content block `future_block`, `usage.x_future_usage`. | schema: none (deliberately invented unknown-* fixture) |
| `cache-prewarm.json` | Pre-warm response: empty `content`, `stop_reason: max_tokens`, `usage.iterations` (documented field missing from the OpenAPI `Usage` schema; must survive as Extras). No `stop_details` key. | doc: https://platform.claude.com/docs/en/build-with-claude/prompt-caching#pre-warming-the-cache |
| `multi-turn-array-content.json` | Response `model` is the dated id `claude-haiku-4-5-20251001` while the request used the alias `claude-haiku-4-5`. | schema: Message |
| `count_tokens/basic-system-string.json` | `{input_tokens: 14}` | doc: https://platform.claude.com/docs/en/build-with-claude/token-counting |
| `count_tokens/tools.json` | `{input_tokens: 403}` | doc: https://platform.claude.com/docs/en/build-with-claude/token-counting |
| `count_tokens/thinking.json` | `{input_tokens: 88}` | doc: https://platform.claude.com/docs/en/build-with-claude/token-counting |
| `count_tokens/pdf-base64.json` | `{input_tokens: 2188}` | doc: https://platform.claude.com/docs/en/build-with-claude/token-counting |
