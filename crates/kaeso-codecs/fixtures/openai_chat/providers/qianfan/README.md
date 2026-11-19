# Fixtures: Baidu Qianfan (`qianfan`) on `openai_chat`

Baidu Qianfan's Chat Completions, `POST https://qianfan.baidubce.com/v2/chat/completions` with a `bce-v3/…` API key as `Authorization: Bearer`. The API sits at `/v2`; `/v1/chat/completions` and `/v2/v1/chat/completions` answer 404. The reference is a field list, not an OpenAPI document; it is mostly in Chinese.

No key was available. The requests and replies are the API reference's examples and a reply another project published; the stream is the web-search guide's; the errors were answered live without a valid key or are the reference's examples.

## Sources (read 2026-11-14)

| Source | Address |
|---|---|
| Chat reference (文本生成), updated 2026-11-11 | https://cloud.baidu.com/doc/qianfan-api/s/wmwbl48n4 |
| Vision reference (视觉理解) | https://cloud.baidu.com/doc/qianfan-api/s/ymwbl48ky |
| Error codes | https://cloud.baidu.com/doc/qianfan-api/s/rmwbl43x7 |
| Model listing | https://cloud.baidu.com/doc/qianfan-api/s/Amwbl43sp |
| Function calling guide | https://cloud.baidu.com/doc/qianfan-docs/s/xm95lyys5 |
| Web search guide | https://cloud.baidu.com/doc/qianfan-docs/s/Wm8r4sw29 |
| A published `ernie-speed-128k` reply | https://github.com/immersive-translate/immersive-translate/issues/3106 |

## Files

| File | What it covers | Provenance |
|---|---|---|
| `requests/greeting-doc.json`, `responses/greeting-doc.json` | a system and a user message; `choices[].flag` | reference examples, verbatim |
| `requests/tool-calls-doc.json`, `responses/tool-calls-doc.json` | two parallel tool calls, `content: ""` | reference examples, verbatim |
| `requests/after-tool-results-doc.json`, `responses/after-tool-results-doc.json` | tool results with `name` | reference examples, verbatim |
| `responses/reasoning-tool-calls-doc.json` | `reasoning_content` beside tool calls, `cached_tokens`, `reasoning_tokens` | function calling guide, verbatim |
| `responses/vision-doc.json` | a vision reply | vision reference, verbatim |
| `responses/greeting-published.json` | a real `ernie-speed-128k` reply | published |
| `requests/web-search-stream-built.json`, `streams/web-search-stream-built.sse` | `web_search`, `delta_tag: "search_status"`, `search_results` beside the choices | the guide's stream with its elided middle (`......`) left out; the request is built for it |
| `errors/recorded-invalid-iam-token.json` | 401 for an unknown key | live, with a made-up key; id zeroed |
| `errors/recorded-over-rate-limit.json` | the BCE gateway's 429 shape, which `GET /v2/models` answers a made-up key with | live; request id zeroed |
| `errors/malformed-json-doc.json` | 400 `malformed_json` | reference example |
| `errors/invalid-appid-published.json` | 401 `invalid_appId`, a key bound to an app without the `appid` header | published |
| `models/list-doc.json` | `GET /v2/models` with Qianfan's own fields | reference example |

## Where Qianfan differs from OpenAI Chat Completions

| Difference | How KPP carries it | Source |
|---|---|---|
| paths under `/v2` | the base URL ends at `/v2`; `api_root` is empty | reference, live 404s |
| no `developer` role | sent as `system` | reference |
| `user` for the end user | `end_user_as_user()` | reference |
| `thinking: {"type"}` switch, `reasoning_content` | `thinking_type()`, `reasoning_fields` | deep thinking guide |
| `flag`, `ban_round`, `search_results`, `delta_tag` | extras | reference, web search guide |
| no `n`, `logprobs`, `store`, `service_tier`, `web_search_options` and others | refused before sending | reference |
| gateway errors `{"requestId", "code", "message"}` | read by their `message`, kind by status | live |
