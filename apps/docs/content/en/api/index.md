---
title: API
description: The two APIs of a Kaeso install and how to call them.
order: 4
---

A Kaeso install serves two APIs on its domain:

| API | Path | Called by | Authentication |
| --- | --- | --- | --- |
| [Model API](model-api.md) | `/v1` | Apps, through the official OpenAI and Anthropic SDKs | A project API key with inference access |
| [Platform API](platform-api.md) | `/platform/v1` | The console, scripts and CI managing organizations, projects, keys, models, limits and logs | A console session, a project API key with platform access, or an organization platform key |

[Errors](errors.md) lists what a failed model request returns in each format. The Platform API's errors, and its OpenAPI document, are on [its page](platform-api.md).
