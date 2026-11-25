# anthropic_messages / providers / deepinfra (DeepInfra)

DeepInfra's Anthropic-compatible Messages API, `POST https://api.deepinfra.com/anthropic/v1/messages`, and its token count, `POST /anthropic/v1/messages/count_tokens`. The key works as `x-api-key` or `Authorization: Bearer`, which Kaeso sends. Every chat model is served, plus the resold Claude models under `anthropic/`; a bare Claude id such as `claude-opus-5-5` runs on `anthropic/claude-opus-5-5`, and the reply's `model` is the id that was sent. A request that arrives in this dialect is sent here as it came.

No key was available. The requests are the [Anthropic SDK & Claude Code](https://docs.deepinfra.com/integrations/anthropic) page's examples, read on 2026-11-25; the page shows no reply, so the reply and the stream are constructed in Anthropic's shape, and the message id is zeros.

| file | what it covers | provenance |
|---|---|---|
| `requests/hello.json`, `responses/hello.json` | the page's create-message example (`zai-org/GLM-5.2`) | doc (request); the reply is constructed |
| `requests/poem-stream.json`, `streams/poem-stream.sse` | the page's streaming example | doc (request); the stream is constructed |

No token-count fixture: the page shows a count request but no reply, and the codec tests pin the number of shared `requests/count_tokens` fixtures. For Claude models the count is an estimate. `output_config.effort` reaches the model as DeepInfra's `reasoning_effort`. The page says not every Anthropic-specific feature may be supported, without listing them.
