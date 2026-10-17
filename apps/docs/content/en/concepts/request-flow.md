---
title: Request flow
description: The steps a model request takes through the gateway.
order: 1
---

Every model request takes the same path through the gateway. Nothing is passed through untranslated, and nothing is retried. Every refusal on the way is an error in the app's own format.

```text
app ─▶ key ─▶ decode ─▶ model ─▶ limits ─▶ provider key ─▶ encode and send ─▶ provider
app ◀─ encode reply ◀─ decode reply ◀──────────────────────────────────────── provider
                             └─▶ usage, cost and request log
```

## Key

The gateway reads the API key from `x-api-key`, else from `Authorization: Bearer`. The key decides the project and acts with the rights of its member or service account there. It needs inference access, and its owner needs the right `api.request`; organization platform keys can't call models. Keys looked up in the last 30 seconds are answered from memory.

## Decode

The body may be up to `KAESO_BODY_LIMITS_MODEL_API` (32 MiB); a larger one is refused with 413. KPP decodes the request from the app's format, such as OpenAI Chat Completions or Anthropic Messages, into Kaeso's canonical format. A request that can't be read, or that asks for something Kaeso never sends to a provider, such as more than one choice or stored Responses state, is refused here.

## Model

The model name is resolved in the project's catalogue: an alias first, then `provider/model`, then the model's own id. A name that isn't there is refused with 404, and a model id that several providers serve in the project with 400. The member's model access and the key's are checked (403).

The catalogue entry gives the model's limits, prices, capabilities, the format the provider is called in, whether it reasons, whether it has adaptive thinking, and its timeout. The request is checked against it and refused with 400 when it needs what the model lacks:

- images, PDFs, tools, a JSON schema or reasoning;
- an effort level the model doesn't list, or adaptive thinking for a Claude model without it;
- a forced tool choice (`required`, `any` or a named tool) for a model that takes none, such as Claude Fable 5.1, Opus 5.5 and Sonnet 5.5; the message names the app's own field and value;
- a temperature other than 1, for a model that accepts none, unless Kaeso turns Claude's thinking on and leaves the temperature out;
- embeddings from a generation model, or text from an embedding model.

A request in an OpenAI format for a Claude model without a maximum number of output tokens gets the model's output limit. Anthropic Messages requires `max_tokens`, so a Messages request without it is refused with 400, as Anthropic refuses it.

## Limits

Every limit that applies is checked at once, in one step in Valkey: the rates and budgets of the key, of its member and of the project, for every model or for this one. A request over a rate or a hard budget is refused with 429 and `Retry-After`. Soft budgets don't refuse requests. When Valkey can't be reached, a request under a hard budget is refused with 503, and other requests pass unchecked. When the project's limits can't be loaded from PostgreSQL, every request is refused with 503, because Kaeso can't tell which limits apply.

## Provider key

The project's provider key for the model's provider is chosen: the key assigned to the project, else the organization's default. Without one, the request is refused with 400. The key is decrypted with the master key and kept only in the instance's memory, for at most 30 seconds.

## Encode and send

KPP checks the request against the provider's format and encodes it; what that format can't express is refused here, before anything reaches the provider ([format translation](format-translation.md)). For a Chat Completions app served through another format, the reasoning kept from earlier turns is put back first ([reasoning between turns](format-translation.md#reasoning-between-turns)).

| Model | API called |
| --- | --- |
| OpenAI | Responses, or Chat Completions when the catalogue says so; Embeddings for embeddings |
| Anthropic | Messages |

Token counts go to Anthropic's `/v1/messages/count_tokens` or OpenAI's `/v1/responses/input_tokens`. `KAESO_PROVIDERS_OPENAI_BASE_URL` and `KAESO_PROVIDERS_ANTHROPIC_BASE_URL` set where the providers are.

| Timeout | Setting | Default | Limits |
| --- | --- | --- | --- |
| Connect | `KAESO_UPSTREAM_CONNECT_TIMEOUT` | 10 s | Connecting to the provider, TCP and TLS |
| Request | `KAESO_UPSTREAM_REQUEST_TIMEOUT` | 600 s | A whole request that doesn't stream |
| First byte | `KAESO_UPSTREAM_FIRST_BYTE_TIMEOUT` | 60 s | A stream, until the provider answers |
| Idle | `KAESO_UPSTREAM_STREAM_IDLE_TIMEOUT` | 300 s | A stream, the longest silence between two chunks |

A model's own timeout in the catalogue replaces the request and first-byte timeouts. A timeout is answered with 504, a provider that can't be reached with 502 ([provider calls](../self-hosting/configuration.md#provider-calls)).

## Reply

A provider's error arrives as the provider sent it when the app speaks the provider's format, and keeps its meaning in the app's format otherwise ([provider errors](../api/errors.md#provider-errors)). A reply is decoded into the canonical format and encoded in the app's format, event by event for a stream. A reply is never refused for what it holds: what the app's format can't express is left out of what the app receives, and kept in the request log when content logging is on.

Replies carry `kaeso-model` and `kaeso-provider`. A stream's headers are sent as soon as the provider answers. While a stream is open and silent, the gateway writes keepalives the app's SDK ignores ([long streams](../self-hosting/domains-and-tls.md#long-streams)).

## When a stream fails

Once a stream's headers are sent, its status can't change. A stream that fails after that ends with an error event in the app's format, and the request log records it as a failure:

| Cause | Status logged | Error kind |
| --- | --- | --- |
| The provider sends an error event | The error's status, else 502 | The error's kind |
| The connection to the provider breaks off, or the stream ends before the reply is complete | 502 | `stream_interrupted` |
| The provider is silent for longer than `KAESO_UPSTREAM_STREAM_IDLE_TIMEOUT` | 504 | `timeout` |
| KPP can't encode an event for the app | The error's status, else 500 | The error's kind |

It counts as an error in usage and in `kaeso_stream_failures_total`, while `kaeso_requests_total` counts it as 200 ([monitoring](../self-hosting/monitoring.md)). Every streamed entry in the request log says how far it got in `stream_progress`: the provider's events, the bytes and keepalives written to the app, and the time to the first reply bytes.

## When the app goes away

When an app closes the connection, the gateway stops the provider request at once, so the provider stops generating. The request is logged as 499 `client_disconnected`.

A stream is charged what the provider reported, plus an estimate of the rest:

- the input from the provider's usage at the start of the stream, else from its token-counting endpoint, else from the size of the request;
- the output from the content received.

The same applies to a stream that breaks off. Such log entries say `usage.estimated: true`, and costs and limits use the same numbers. Output estimates err low for reasoning the provider doesn't stream.

## When the gateway shuts down

On a shutdown signal the gateway takes no new connections and lets open requests and streams run for `KAESO_SHUTDOWN_GRACE` (30 seconds). One still open then gets an error in the app's format, "Kaeso is restarting": 503, or 500 for OpenAI-format apps, which the SDKs retry; a stream that already started ends with an error event instead. It is charged and logged with the error kind `shutdown`, with a stream's usage estimated. The gateway waits up to 10 seconds for these before it writes usage and logs for the last time.

## Usage and logs

The request's tokens are charged at the model's prices at the time of the request, to the key, the member and the project, and counted against their budgets. A log entry records the metadata, and the request and reply when content logging is on for the organization and the project. Logging never holds a request up: entries wait in memory and are written to ClickHouse in batches. When the buffer is full, the oldest entries are dropped.
