---
title: API
description: Call models from your apps and manage your install over HTTP.
order: 4
---

Every install serves two APIs on its domain. Apps call models through the model API at `/v1` and `/v1beta` with an API key, and scripts manage the install through the Platform API at `/platform/v1`. Agents reach the Platform API through the MCP server at `/mcp`, and clients call a project's tools through its MCP portal at `/mcp/<project id>`.

## Next steps

- [Model API](model-api.md): Call models with the OpenAI, Anthropic and Google Gen AI SDKs, and look up endpoints, headers and model names.
- [Images API](images.md): Generate and edit images with OpenAI's Images API on every provider that makes images.
- [Audio API](audio.md): Make speech and transcribe or translate audio with OpenAI's audio API on every provider that offers it.
- [ElevenLabs API](elevenlabs.md): Call ElevenLabs speech, sound, music and transcription with the official ElevenLabs SDKs.
- [Errors](errors.md): Read what a failed model request returns in each format, and why it failed.
- [Platform API](platform-api.md): Look up authentication, conventions, errors and routes for managing an install.
- [Automate with the Platform API](automation.md): Set up a project, a model and an app's key from a script.
- [MCP server](mcp.md): Connect coding agents, choose what they may do, and look up their tools and sign-in.
- [MCP portal API](mcp-portal.md): Call a project's tools over MCP, and look up the portal's sign-in, versions and errors.
