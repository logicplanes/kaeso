---
title: OpenRouter
description: Count the requests you send to OpenRouter, with their tokens and cost, through its Broadcast to an OpenTelemetry collector.
order: 7
---

OpenRouter's Broadcast sends a trace of every request you make to OpenRouter to a destination of your choice. With a Kaeso source as an OpenTelemetry collector destination, each request becomes an observed request in the project, with its model, tokens and cost.

[Add a source](index.md#add-a-source) of the kind **OpenRouter**. Then, in OpenRouter's Broadcast settings, add an OpenTelemetry Collector destination with these values:

```text
https://kaeso.example.com/ingest/otlp/v1/traces
```

```json
{ "Authorization": "Bearer sk-kso-src-…" }
```

The first is the endpoint, the full path of the traces signal; the second is the headers. Send a request to OpenRouter, and the source shows **Connected** once its trace arrives.

## What Kaeso reads

Each trace's GenAI span becomes a request of the provider OpenRouter, with the model, the tokens, the cost OpenRouter charged (`gen_ai.usage.total_cost`) and how long it took. OpenRouter names models as `provider/model`, such as `anthropic/claude-sonnet-4.5`. Kaeso prices them from the project's catalogue or KPMR when it knows the model, and otherwise keeps OpenRouter's cost.

OpenRouter's traces carry the `user` you send with a request as `user.id`. When it's an email address and the source matches members by email, the request counts for that member; otherwise it counts for the source's member or service account. Give each team its own OpenRouter key and source when their requests should count apart.

## Keep the prompts

OpenRouter puts the prompt and the reply into its traces. Kaeso keeps them only when **Keep prompts and replies** is on for the source and the project keeps content.

## Next steps

- [Sources](../concepts/sources.md): How Kaeso counts observed requests, and whose they are.
- [OpenTelemetry](opentelemetry.md): What Kaeso reads from GenAI spans and log events.
