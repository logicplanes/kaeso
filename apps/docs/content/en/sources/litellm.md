---
title: LiteLLM
description: Count the requests a LiteLLM proxy serves, with their tokens and cost, from its generic API logger.
order: 6
---

A LiteLLM proxy can send a log of every request it serves to any HTTP endpoint. Sent to a Kaeso source, each becomes an observed request in the project, with its model, provider, tokens and cost.

[Add a source](index.md#add-a-source) of the kind **LiteLLM**, then add a `generic_api` callback to the proxy's `config.yaml`, with your install's address and the source's key:

```yaml
litellm_settings:
  callbacks: ["kaeso"]
callback_settings:
  kaeso:
    callback_type: generic_api
    endpoint: https://kaeso.example.com/ingest/v1/litellm
    headers:
      Authorization: Bearer sk-kso-src-…
```

Restart the proxy and send it a request. LiteLLM sends its logs in batches, every few seconds; then the source shows **Connected** and the request appears in the project's request logs.

## Keep batches small

LiteLLM sends up to 512 logs a batch, each with the request's messages and reply. Kaeso takes at most 8 MiB a batch, so set `DEFAULT_BATCH_SIZE=50` in the proxy's environment if its requests are long. By default, LiteLLM doesn't send a refused batch again.

## Whose requests they are

Each log carries the email address of the LiteLLM key's user when LiteLLM knows it. When the source matches members by email, the request counts for the member with that address. Otherwise it counts for the source's member or service account. The request's own LiteLLM tags become its tags in Kaeso.

## Keep the prompts

LiteLLM's logs always carry the messages and the reply. Kaeso keeps them, with the system prompt, tool calls, tool results and reasoning, only when **Keep prompts and replies** is on for the source and the project keeps content.

## What Kaeso reads

| LiteLLM field | In Kaeso |
| --- | --- |
| `id` | The request's id, so a log sent twice counts once |
| `model`, `custom_llm_provider` | The model and the provider |
| `prompt_tokens`, `completion_tokens` and the provider's usage | Tokens, with cache reads, cache writes and reasoning |
| `status`, `error_str` | Whether it failed, and why |
| `startTime`, `endTime`, `completionStartTime` | When it ran, and the time to the first token |
| `response_cost` | The cost, used when Kaeso has no price for the model; a 0 that LiteLLM explains in `response_cost_failure_debug_info` is an unknown cost |

LiteLLM's MCP tool calls become tool calls in the project. LiteLLM can also send OpenTelemetry instead: a source of the kind [OpenTelemetry](opentelemetry.md) reads its GenAI spans.

## Next steps

- [Sources](../concepts/sources.md): How Kaeso counts observed requests, and whose they are.
- [OpenRouter](openrouter.md): Count your OpenRouter requests with Broadcast.
