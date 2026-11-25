# anthropic_messages / providers / qianfan (Baidu Qianfan)

Qianfan's Anthropic-compatible Messages API, `POST https://qianfan.baidubce.com/anthropic/v1/messages`, at the host root beside the `/v2` Chat Completions root. The page's examples send `x-api-key` with `anthropic-version: 2023-06-01`; its Claude Code example sends the key as `ANTHROPIC_AUTH_TOKEN`, that is `Authorization: Bearer`, which Kaeso sends. An optional `appid` header picks a custom app. Every chat model is compatible. A request that arrives in this dialect is sent here as it came.

No key was available. The requests are the [Anthropic Claude API 兼容](https://cloud.baidu.com/doc/qianfan-docs/s/6mh3e6gjp) page's examples, read on 2026-11-25 (the page is dated 2026-03-13); it shows no reply, so the reply and the stream are constructed in Anthropic's shape, and the message id is zeros.

| file | what it covers | provenance |
|---|---|---|
| `requests/hello.json`, `responses/hello.json` | the page's SDK example (`deepseek-v3.2`, `Hello!`) | doc (request); the reply is constructed |
| `requests/hello-stream.json`, `streams/hello-stream.sse` | the page's curl example: content as a text-block array, `max_tokens: 1000`, `stream: true` | doc (request); the stream is constructed |

The page documents no differences from Anthropic beyond the model names and the `appid` header.
