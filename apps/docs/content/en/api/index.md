---
title: API
description: The two APIs of a Kaeso install and how to call them.
order: 4
---

A Kaeso install serves two APIs on its domain:

| API | Path | For | Authentication |
| --- | --- | --- | --- |
| Model API | `/v1` | Apps calling models, in the OpenAI and Anthropic formats | A project API key with inference access |
| Platform API | `/platform/v1` | The console and scripts managing organizations, projects, keys, models, limits and logs | A session, a project API key with platform access, or an organization platform key |

| Page | What it covers |
| --- | --- |
| [Model API](model-api.md) | Endpoints, base URLs, keys, model names, streaming, response headers |
| [Errors](errors.md) | What a failed model request returns, in each format |
| [Platform API](platform-api.md) | Conventions, authentication, errors, a walk through with curl, areas, roles and rights, the Playground, the OpenAPI document |
