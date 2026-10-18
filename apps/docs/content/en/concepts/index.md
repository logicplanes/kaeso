---
title: Concepts
description: The terms these pages use, and how Kaeso handles requests, formats and rights.
order: 2
---

[Request flow](request-flow.md) follows a model request through the gateway, [format translation](format-translation.md) explains what KPP carries between API formats, and [rights](rights.md) lists what each role may do. Every page uses these words in the same sense.

## Terms

| Term | Meaning |
| --- | --- |
| Install | One Kaeso deployment: the gateway, the console and their databases, on one domain ([self-hosting](../self-hosting/index.md)) |
| Instance admin | A user who runs the install: its users, organizations and settings. It gives no rights inside organizations ([instance administration](../console/instance-administration.md)) |
| Organization | Holds members, provider keys, platform keys and projects ([organizations and projects](../console/organizations-and-projects.md)) |
| Project | Holds a model catalogue, API keys, service accounts, limits and request logs. Every organization has a default project |
| Member | A person in an organization, with an organization role and a project role in each of their projects |
| Service account | A project member for software, such as a deployment or a job; its keys outlive the people who made them |
| Role, right | A right allows one kind of action, such as `keys.all.view`; a role is a named set of rights ([rights](rights.md)) |
| API key | `sk-kso-…`: a key of a member or service account in one project. With inference access it calls models on the model API, with platform access it manages the project on the Platform API ([API keys](../console/api-keys.md)) |
| Platform key | `sk-kso-org-…`: a key that manages a whole organization on the Platform API and never calls models |
| Provider key | The organization's OpenAI or Anthropic key, which Kaeso uses to call the provider; apps never see it ([provider keys](../console/provider-keys.md)) |
| Master key | The install's own key that encrypts the secrets it stores, such as provider keys ([backups and master keys](../self-hosting/backups.md)) |
| Catalogue | The models a project may call, with their prices, limits and capabilities: from models.dev by default, or set in the project for a custom model or an override ([models and aliases](../console/models-and-aliases.md)) |
| Alias | A project's own name for a model in its catalogue, such as `fast` |
| Limit | A request or token rate, or a token or spend budget, for a project, a member or a key; a hard budget refuses requests, a soft one only alerts ([limits and budgets](../console/limits-and-budgets.md)) |
| Request log | The record of one model request: status, tokens, cost, timing and, where kept, its content ([request logs](../console/request-logs.md)) |
| API format | The shape of a request and its reply: OpenAI Chat Completions, Responses or Embeddings, or Anthropic Messages |
| KPP | The Kaeso Provider Protocol: the one format the gateway translates every API format into and out of ([format translation](format-translation.md)) |
| Model API, Platform API | The two APIs of an install: `/v1`, which apps call for models, and `/platform/v1`, which the console and scripts use to manage it ([API](../api/index.md)) |
