---
title: Sources
description: Count and log the requests that coding agents and other gateways send to providers without Kaeso.
order: 7
---

Coding agents such as Claude Code and Codex, and gateways such as LiteLLM, call their providers directly. A source lets them report those calls to Kaeso, so your request logs, usage, costs and limits include them while the calls themselves never pass through Kaeso.

```mermaid
flowchart LR
  accTitle: An observed request
  accDescr: A client such as Claude Code calls its provider directly. Separately, it sends telemetry about the call to the project's source on Kaeso, which logs and counts the request.
  client([Claude Code]) -->|model request| provider[Anthropic]
  provider -->|reply| client
  client -.->|telemetry, with the source key| ingest[Kaeso: /ingest]
  ingest --> logs[(Request logs)]
  ingest --> usage[(Usage, costs and limits)]
```

## Observed, not routed

Kaeso observes a source's requests; it doesn't route them. The client keeps its own provider key, base URL and model, and Kaeso learns about a request only after it ended. So Kaeso never changes, delays or refuses such a request: aliases, guardrails, fallbacks and the response cache don't apply.

Everything after the request works as for the gateway's own requests. Each one gets a row in the request log, marked **Observed**, and counts in usage, costs and analytics for the member it belongs to. In the console, **Sources** lists a project's sources, and the request log, analytics and the tool-call log can show only what one source reported.

## What a source sends

A source belongs to one project and has a kind, which decides how Kaeso reads what it sends:

| Kind | What it sends |
| --- | --- |
| Claude Code | OpenTelemetry log events over OTLP/HTTP |
| Codex | OpenTelemetry log events over OTLP/HTTP |
| Gemini CLI | OpenTelemetry log events over OTLP/HTTP |
| Antigravity | The model calls of its transcript, from a hook script, as JSON |
| LiteLLM | The proxy's generic API logs, as JSON |
| OpenRouter | Broadcast traces over OTLP/HTTP |
| OpenTelemetry | GenAI spans or log events from any app or collector, over OTLP/HTTP |
| HTTP | Batches of requests and tool calls in Kaeso's own JSON |

OTLP goes to `/ingest/otlp/v1/logs`, `/ingest/otlp/v1/traces` and `/ingest/otlp/v1/metrics`, as protobuf or JSON, gzipped or not. JSON goes to `/ingest/v1/events`, `/ingest/v1/litellm` and `/ingest/v1/antigravity`. Every deployment template routes `/ingest` to the gateway.

Kaeso counts requests only from log events and spans. Metrics never add requests, so a client that exports both isn't counted twice, and they don't connect a source on their own: a source shows **Connected** once a request or tool call arrives. Of the tool calls clients report, only MCP tool calls are kept; a client's own tools, such as a shell, are left out.

## Whose requests they are

A request counts for a member, or for a service account, like any request through the gateway. When the event names an email address and the source matches members by email, the request counts for the organization member with that address. Otherwise it counts for the source's member or service account, chosen when the source is added.

Either way, the member must be able to see the project, and a service account must still be active in it. A record that would count for nobody who can is refused, so a request never counts for someone outside the project. Once a source's own member can no longer see the project, or its service account is archived, Kaeso refuses all of its records, whatever addresses they name.

## Tokens and cost

Clients count tokens differently: Claude Code leaves cached input out of its input count, while Codex, Gemini CLI and OpenTelemetry include it. Kaeso turns each count into its own: input without what came from or went into the cache, and output with the reasoning in it.

A request costs what the project's catalogue says the model costs. Without a catalogue entry, Kaeso uses the model's price from [KPMR](../kpmr/index.md). When neither has a price, the request keeps the cost the source reported, such as Claude Code's or LiteLLM's, and its details say so. Cache writes that Anthropic keeps for an hour cost the hour's price.

When a record names no provider, Kaeso looks for the model at the provider the kind of source usually calls, then at the model's maker, or at the one provider that sells it. A model named with an effort level at its end, such as Antigravity's `gemini-3.8-flash-low`, is priced as the model without it.

## Duplicates and refusals

Clients send an event again when an export failed, so Kaeso keeps each request and tool call once. A source that reports the same id within seven days counts it once, and its activity counts the rest as duplicates. An app that exports a call both as a span and as a log event written in that span counts it once too.

A client that sends its requests through Kaeso's gateway and also reports them to a source would count each one twice. Kaeso matches such a copy to the gateway's request: same project, model and tokens, starting or ending within five seconds of it. The copy then counts as a duplicate.

> [!WARNING]
> The match misses when the client names the model differently from the request it sent, such as an alias. Point a client at the gateway or at a source, not both.

Kaeso refuses a record that:

- names no model or no tool, or has no id;
- is more than seven days old, or more than five minutes in the future;
- has a token count over 10 million, or costs more than $100;
- has a status that isn't an HTTP status;
- would count for nobody who can see the project.

A refused record changes nothing, and the source's sheet shows the reason for the last one. A batch that Kaeso can't read is refused whole, and so is a batch in which no record is one Kaeso reads, such as the events of a client it has no reader for. An OTLP exporter learns how many records were refused from the answer's `partialSuccess`.

## Limits and alerts

Observed requests count toward the project's limits and budgets, and toward each member's and service account's, so budget alerts include them. A limit's view shows how much of its use sources reported.

> [!IMPORTANT]
> Kaeso can't stop an observed request, because it isn't in its path. A hard budget that observed requests use up still refuses the requests that do go through Kaeso. So a source key can use up the project's budgets as an API key can: keep it as secret as one, and disable the source or give it a new key if it leaks.

## Prompts and replies

A source keeps the prompts, replies and tool arguments its client sends only when **Keep prompts and replies** is on for it and the project keeps request content. Most clients leave content out of their telemetry unless you turn it on; the guide for each one says how.

When a client sends the whole exchange, as Gemini CLI, LiteLLM and GenAI spans do, Kaeso keeps all of it: the system prompt, the earlier messages, tool calls and their results, and the reasoning. Clients export in batches, so a prompt can arrive a batch before its request; Kaeso holds it for an hour for the request it belongs to.

## The source key

Each source has a key, `sk-kso-src-…`, which Kaeso shows once and keeps only as a hash. It works only on `/ingest`: it calls no model and reaches neither the Platform API nor the MCP portal. A client sends it as `Authorization: Bearer sk-kso-src-…`, or in the `X-Kaeso-Source-Key` header. Kaeso also takes `Bearer%20sk-kso-src-…`, as some exporters send the value of `OTEL_EXPORTER_OTLP_HEADERS` without decoding it.

A batch holds at most 1,000 records and 8 MiB, 16 MiB once unzipped or 8 MiB for JSON. A source may send 600 batches a minute, three at a time, and 6,000 records (requests and tool calls) a minute and 200,000 a day; past that Kaeso answers 429 with `Retry-After` and keeps nothing of the batch. A batch with more fields or values than Kaeso can read at once is refused with 413. A disabled or archived source, or one in an archived project, has its batches refused. Seeing sources takes the right `sources.view`, which every project role has; adding and changing them takes `sources.manage`, which admins and developers have.

## Next steps

- [Connecting sources](../sources/index.md): Add a source in the console and follow what it reports.
- [Claude Code](../sources/claude-code.md): Count Claude Code's requests and MCP tool calls.
- [HTTP](../sources/http.md): Send requests and tool calls from your own scripts.
