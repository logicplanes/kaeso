---
title: Concepts
description: Learn the terms these pages use, and how Kaeso handles requests, formats and rights.
order: 2
---

These pages explain how Kaeso works, so you know what to expect before you rely on it. Every page of this documentation uses the terms below in the same sense.

- [Providers](providers.md): See which providers Kaeso calls, how to use one and what differs between them.
- [Supported providers](supported-providers.md): Look up every provider's id, formats and keys.
- [Request flow](request-flow.md): Follow a model request through the gateway, and learn which error it gets and what it is charged.
- [Format translation](format-translation.md): See what carries across when an app calls another provider's model.
- [Rights](rights.md): Look up every right, which roles hold it and the rules for giving it.

## Terms

| Term | Meaning |
| --- | --- |
| Install | One Kaeso deployment: the gateway, the console and their databases, on one domain ([self-hosting](../self-hosting/index.md)) |
| Instance admin | A user who runs the install: its users, organizations and settings. Being one gives no rights inside organizations ([instance administration](../console/instance-administration.md)) |
| Organization | Holds members, provider keys, platform keys and projects ([organizations and projects](../console/organizations-and-projects.md)) |
| Project | Holds a model catalogue, API keys, service accounts, limits and request logs. Every organization has a default project |
| Member | A person in an organization. A member has an organization role, and a project role in each of their projects |
| Service account | A project member for software, such as a deployment or a job. Its keys keep working when the people who made them leave |
| Right, role | A right allows one kind of action, such as `keys.all.view`. A role is a named set of rights ([rights](rights.md)) |
| API key | `sk-kso-…`, the key of a member or service account in one project. With inference access it calls models, and with platform access it manages the project ([API keys](../console/api-keys.md)) |
| Platform key | `sk-kso-org-…`, a key that manages a whole organization on the Platform API. It never calls models |
| Provider | A company whose models Kaeso calls, such as OpenAI, Anthropic or Groq. Its id, such as `groq`, names it in model names and settings ([providers](providers.md)) |
| Provider key | The organization's key for a model provider, such as OpenAI or Anthropic, which Kaeso uses to call the provider. Apps never see it ([provider keys](../console/provider-keys.md)) |
| Master key | The install's own key, which encrypts the secrets it stores, such as provider keys ([backups and master keys](../self-hosting/backups.md)) |
| Catalogue | The models a project may call, with their prices, limits and capabilities. They come from models.dev, and a project can add custom models or override fields ([models and aliases](../console/models-and-aliases.md)) |
| Alias | A project's own name for a model in its catalogue, such as `fast` |
| Limit | A request or token rate, or a token or spend budget, for a project, a member or a key. A hard budget refuses requests, and a soft one only alerts ([limits and budgets](../console/limits-and-budgets.md)) |
| Request log | The record of one model request: status, tokens, cost, timing and, where kept, its content ([request logs](../console/request-logs.md)) |
| API format | The shape of a request and its reply: OpenAI Chat Completions, Responses or Embeddings, or Anthropic Messages |
| KPP | The Kaeso Provider Protocol, the one format the gateway translates every API format into and out of ([format translation](format-translation.md)) |
| Model API, Platform API | The two APIs of an install. Apps call models on `/v1`, and the console and scripts manage the install on `/platform/v1` ([API](../api/index.md)) |
