# anthropic_messages / providers / huggingface (Hugging Face Inference Providers)

The Hugging Face router's Messages API, `POST https://router.huggingface.co/v1/messages`, for every chat model whatever the provider serving it, with `Authorization: Bearer`, which Kaeso sends; `x-api-key` is not accepted. A provider can be picked with a model suffix (`:novita`, `:fastest`, `:cheapest`). A request that arrives in this dialect is sent here as it came.

No key was available. The requests are the [Messages API](https://huggingface.co/docs/inference-providers/guides/messages-api.md) guide's examples, read on 2026-11-25; the guide shows no reply, so the reply and the stream are constructed in Anthropic's shape, and the message id is zeros.

| file | what it covers | provenance |
|---|---|---|
| `requests/sky.json`, `responses/sky.json` | the guide's curl example; a reasoning model's `thinking` block before the text, as the guide says; the block has no `signature` and the usage only `input_tokens` and `output_tokens` | doc (request, thinking block before the text); the reply is constructed |
| `requests/count-stream.json`, `streams/count-stream.sse` | the guide's streaming example; the standard Messages events | doc (request, event list); the stream is constructed |

`max_tokens` is required, and reasoning models spend part of it on the `thinking` block. The guide documents no `count_tokens` endpoint.
