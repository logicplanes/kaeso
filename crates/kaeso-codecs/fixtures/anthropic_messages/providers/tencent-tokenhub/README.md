# anthropic_messages / providers / tencent-tokenhub (Tencent Cloud TokenHub)

TokenHub's Anthropic Messages API, `POST https://tokenhub.tencentmaas.com/v1/messages` (Singapore: `tokenhub-intl.tencentmaas.com`), for every model the overview marks as Anthropic-compatible. The overview names `x-api-key` for this path; the field reference says `Authorization: Bearer`, which Kaeso sends, is accepted too, and lists `anthropic-version: 2023-06-01` as required. A request that arrives in this dialect is sent here as it came.

No key was available. The requests are the examples of the [Anthropic Message 协议字段说明](https://cloud.tencent.com/document/product/1823/135874) page (updated 2026-11-03), with `hy3` for the examples' blank model; the overview is the [语言模型调用概览](https://cloud.tencent.com/document/product/1823/130079) page (updated 2026-11-16), both read on 2026-11-25. The replies are constructed from the page's response field tables; the message id is zeros.

| file | what it covers | provenance |
|---|---|---|
| `requests/thinking.json`, `responses/thinking.json` | the page's thinking example (`budget_tokens: 2048`); a `thinking` block without `signature`, which the page says `hy3` doesn't return; `service_tier` in usage | doc (request, no signature for `hy3`, usage fields); the reply is constructed |
| `requests/system-stream.json`, `streams/system-stream.sse` | the page's system-plus-stream example: `system` as a text-block array; a `ping`; `message_start` with the input and cache counts, `message_delta` with the final `output_tokens` | doc (request, event order); the stream is constructed |

Documented differences from Anthropic:

- Support depends on the model: thinking, forced `tool_choice` with thinking, `output_config.format`, `stop_sequence` echo, `document` citations and `search_result` blocks vary or are missing. Some models (as `hy3`) send no `signature`.
- `cache_control` is partly honoured; TTL breakdowns are often incomplete, and models without caching report 0.
- An error after the stream started comes as an `error` event or as a closed connection.
