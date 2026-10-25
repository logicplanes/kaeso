---
title: API
description: Call models from your apps and manage your install over HTTP.
order: 4
---

Every install serves two APIs on its domain. Apps call models through the model API at `/v1` with an API key, and scripts manage the install through the Platform API at `/platform/v1`. Agents reach the Platform API through the MCP server at `/mcp`.

- [Model API](model-api.md): Call models with the OpenAI and Anthropic SDKs, and look up endpoints, headers and model names.
- [Errors](errors.md): Read what a failed model request returns in each format, and why it failed.
- [Platform API](platform-api.md): Look up authentication, conventions, errors and routes for managing an install.
- [Automate with the Platform API](automation.md): Set up a project, a model and an app's key from a script.
- [MCP server](mcp.md): Connect coding agents, choose what they may do, and look up their tools and sign-in.
