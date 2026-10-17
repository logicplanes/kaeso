---
title: Request flow
description: The steps a model request takes through the gateway.
order: 1
---

Every model request takes the same path through the gateway. Each request is translated, never passed through as it came, and sent once, never retried. A refusal at any step reaches the app as an error in its own format.

```text
app ─▶ key ─▶ decode ─▶ model ─▶ limits ─▶ provider key ─▶ encode and send ─▶ provider
app ◀─ encode reply ◀─ decode reply ◀──────────────────────────────────────── provider
                             └─▶ usage, cost and request log
```

## Key

The gateway reads the API key from `x-api-key`, else from `Authorization: Bearer`. The key decides the project, and acts there with the rights of its member or service account. It needs inference access, and its owner the right `api.request`; organization platform keys never call models. A key looked up in the last 30 seconds is answered from memory.

## Decode

The body may be up to `KAESO_BODY_LIMITS_MODEL_API` (32 MiB); a larger one is refused with 413. KPP decodes the request from the app's format, such as Chat Completions or Messages, into its canonical format. A request that can't be read is refused here, and so is one that asks for what Kaeso never sends to a provider, such as more than one choice or stored Responses state.

## Model

The model name is looked up in the project's catalogue: first as an alias, then as `provider/model`, then as the model's own id. A name that isn't there is refused with 404, and an id that several providers serve in the project with 400. The model access of the member and of the key is checked next (403).

The catalogue entry holds the model's limits, prices and capabilities, the format its provider is called in, whether it reasons, whether it has adaptive thinking, and its timeout. A request that needs what the model lacks is refused with 400:

- images, PDFs, tools, a JSON schema or reasoning;
- an effort level the model doesn't list, or adaptive thinking for a Claude model without it;
- a forced tool choice (`required`, `any` or a named tool) for a model that takes none, such as Claude Fable 5.1, Opus 5.5 and Sonnet 5.5; the message names the field and value the app sent;
- a temperature other than 1 for a model that accepts none, unless Kaeso turns Claude's thinking on and leaves the temperature out;
- embeddings from a generation model, or text from an embedding model.

A request in an OpenAI format for a Claude model without a maximum number of output tokens gets the model's output limit. A Messages request without `max_tokens` is refused with 400, as Anthropic refuses it.

## Limits

Every limit that applies is checked at once, in one step in Valkey: the rates and budgets of the key, of its member and of the project, for all models or for this one.

| Situation | Result |
| --- | --- |
| Over a rate or a hard budget | 429 with `Retry-After` |
| Over a soft budget | Passes; soft budgets only alert |
| Valkey unreachable | 503 for a request under a hard budget; every other request passes unchecked |
| The project's limits can't be loaded from PostgreSQL | 503 for every request, since Kaeso can't tell which limits apply |

## Provider key

The gateway uses the provider key assigned to the project for the model's provider, else the organization's default for that provider. Without either, the request is refused with 400. The key is decrypted with the master key and kept only in the instance's memory, for at most 30 seconds.

## Encode and send

KPP checks the request against the provider's format and encodes it. What that format can't express is refused here, before anything reaches the provider ([format translation](format-translation.md)). For a Chat Completions app served through another format, the reasoning kept from earlier turns is put back first ([reasoning between turns](format-translation.md#reasoning-between-turns)).

| Provider | API called |
| --- | --- |
| OpenAI | Responses, or Chat Completions when the catalogue says so; Embeddings for embeddings |
| Anthropic | Messages |

Token counts go to Anthropic's `/v1/messages/count_tokens` and OpenAI's `/v1/responses/input_tokens`. `KAESO_PROVIDERS_OPENAI_BASE_URL` and `KAESO_PROVIDERS_ANTHROPIC_BASE_URL` set where the providers are.

| Timeout | Setting | Default | Limits |
| --- | --- | --- | --- |
| Connect | `KAESO_UPSTREAM_CONNECT_TIMEOUT` | 10 s | Connecting to the provider, TCP and TLS |
| Request | `KAESO_UPSTREAM_REQUEST_TIMEOUT` | 600 s | A whole request that doesn't stream |
| First byte | `KAESO_UPSTREAM_FIRST_BYTE_TIMEOUT` | 60 s | A stream, until the provider answers |
| Idle | `KAESO_UPSTREAM_STREAM_IDLE_TIMEOUT` | 300 s | A stream, the longest silence between two chunks |

A model's own timeout in the catalogue replaces the request and first-byte timeouts. A timeout is answered with 504, and a provider that can't be reached with 502 ([provider calls](../self-hosting/configuration.md#provider-calls)).

## Reply

The reply is decoded into the canonical format and encoded in the app's format, event by event for a stream. A reply is never refused for what it holds: what the app's format can't express is left out of what the app receives, and kept in the request log when content logging is on.

A provider's error reaches an app that speaks the provider's format as the provider sent it, and any other app with the same meaning in its own format. A model the provider doesn't serve to the project's provider key gets Kaeso's own message instead ([provider errors](../api/errors.md#provider-errors)).

Replies carry `kaeso-model` and `kaeso-provider`. A stream's headers go out as soon as the provider answers, and while the stream is open and silent, the gateway writes keepalives that the app's SDK ignores ([long streams](../self-hosting/domains-and-tls.md#long-streams)).

## When a stream fails

Once a stream's headers are sent, its status can't change. A stream that fails after that ends with an error event in the app's format, and the request log records the failure:

| Cause | Status logged | Error kind |
| --- | --- | --- |
| The provider sends an error event | The error's status, else 502 | The error's kind |
| The connection to the provider breaks, or the stream ends before the reply is complete | 502 | `stream_interrupted` |
| The provider is silent for longer than `KAESO_UPSTREAM_STREAM_IDLE_TIMEOUT` | 504 | `timeout` |
| KPP can't encode an event for the app | The error's status, else 500 | The error's kind |

Usage and `kaeso_stream_failures_total` count such a stream as an error, while `kaeso_requests_total` counts it as 200, the status that was sent ([monitoring](../self-hosting/monitoring.md)). Every streamed entry in the request log records in `stream_progress` how far it got: the provider's events, the bytes and keepalives written to the app, and the time to the first reply bytes.

## When the app goes away

When an app closes the connection, the gateway stops the provider request at once, so the provider stops generating. The request is logged as 499 `client_disconnected`.

Such a stream, and one that breaks off, is charged what the provider reported plus an estimate of the rest:

- the input from the provider's usage at the start of the stream, else from its token-counting endpoint, else from the size of the request;
- the output from the content received.

These log entries say `usage.estimated: true`, and costs and limits use the same numbers. Output estimates err low for reasoning the provider doesn't stream.

## When the gateway shuts down

On a shutdown signal, the gateway takes no new connections and gives open requests and streams `KAESO_SHUTDOWN_GRACE` (30 seconds) to finish. A request still open then gets the error "Kaeso is restarting" in the app's format: 503, or 500 for apps in an OpenAI format, which the SDKs retry. A stream that already started ends with an error event instead. Both are charged and logged with the error kind `shutdown`, a stream with estimated usage. The gateway then waits up to 10 seconds for these entries before it writes usage and logs for the last time.

## Usage and logs

The request's tokens are charged at the model's prices at the time of the request, to the key, the member and the project, and count against their budgets. The request log records the metadata, and the request and reply when content logging is on for both the organization and the project.

Logging never holds a request up. Entries wait in memory and are written to ClickHouse in batches; when the buffer is full, the oldest entries are dropped.
