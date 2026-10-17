---
title: Introduction
description: Kaeso is an open-source AI gateway you run yourself.
---

Kaeso is an open-source AI gateway you run yourself, so applications talk to one API while you keep provider keys, routing, and usage under your control.

Applications call Kaeso with the official OpenAI or Anthropic SDKs and a Kaeso API key. Kaeso checks the key, the model and the limits, sends the request to the provider with your provider key, and returns the reply in the format the application asked in. Every request is logged and counted, so you see who used which model and what it cost.

New here? The [quick start](get-started/quick-start.md) runs Kaeso with Docker Compose and sends a first request in a few minutes.

This documentation describes Kaeso {{version}}.

## What you get

- **One API for every app.** The OpenAI Chat Completions, Responses and Embeddings formats and the Anthropic Messages format, so the SDKs your apps already use keep working. Chat Completions, Responses and Messages can call any chat model in the catalogue, from OpenAI or Anthropic.
- **Provider keys stay with you.** Apps get Kaeso API keys (`sk-kso-…`) that you can limit, expire and disable. Provider keys are encrypted under a master key that only your install holds.
- **Organizations and projects.** Members, roles and rights, service accounts, a model catalogue and aliases per project.
- **Limits and budgets.** Request and token rates, token and spend budgets for a project, a member or a single key.
- **Request logs and usage.** Every request with its status, tokens, cost and timing, and its content when you choose to keep it.
- **A console.** A web interface for all of the above, in English, German and Simplified Chinese.

## How the parts fit

```text
app ──▶ gateway ──▶ OpenAI or Anthropic
          ▲
console ──┘
```

| Part | What it is |
| --- | --- |
| Gateway | The server applications call. It serves the model API under `/v1` and the Platform API under `/platform/v1`, and keeps its state in PostgreSQL, request logs in ClickHouse and counters in Valkey. |
| Console | The web interface, served on the same domain as the gateway. It uses only the Platform API, so a script can do everything it does. |
| KPP | The Kaeso Provider Protocol: one canonical format for model requests and the codecs that translate between it and each API format. The gateway uses it for every request. |

The [request flow](concepts/request-flow.md) explains what happens to a request on its way through the gateway, and [format translation](concepts/format-translation.md) what KPP does with it.

## Where to start

| If you want to | Read |
| --- | --- |
| Run Kaeso and send the first request | [Quick start](get-started/quick-start.md) |
| Call models from your code | [Calling models](get-started/calling-models.md) |
| Use Kaeso from Claude Code or Codex | [Coding tools](get-started/coding-tools.md) |
| Plan and operate an install | [Self-hosting](self-hosting/index.md) |
| Find your way around the console | [Using the console](console/index.md) |
| Call the APIs from code | [API](api/index.md) |
| Work on Kaeso itself | [Development](development/index.md) |

Kaeso is MIT licensed. Its source, the gateway, KPP, the console, the UI library and this documentation, is one repository on GitHub: [logicplanes/kaeso](https://github.com/logicplanes/kaeso).
