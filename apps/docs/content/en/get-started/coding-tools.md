---
title: Coding tools
description: Use Kaeso from Claude Code, Codex and other tools that speak an OpenAI or Anthropic API.
order: 3
---

Coding tools call the same APIs as any app, so they work through Kaeso with a base URL and an API key. Each tool's requests then show up in the request logs and count against the key's limits and budgets.

Create a key for each person and tool, so usage and limits stay apart. The key's project must have every model the tool asks for in its catalogue.

## Claude Code

Claude Code speaks the Anthropic Messages format. Point it at your install in `~/.claude/settings.json`:

```json
{
  "env": {
    "ANTHROPIC_BASE_URL": "https://kaeso.example.com",
    "ANTHROPIC_AUTH_TOKEN": "sk-kso-…",
    "ANTHROPIC_DEFAULT_OPUS_MODEL": "claude-opus-5-5",
    "ANTHROPIC_DEFAULT_SONNET_MODEL": "claude-sonnet-5-5",
    "ANTHROPIC_DEFAULT_HAIKU_MODEL": "claude-haiku-4-5"
  }
}
```

- `ANTHROPIC_AUTH_TOKEN` sends the key as a bearer token, which Kaeso accepts. With it set, Claude Code skips its own sign-in.
- The three model variables choose what the `opus`, `sonnet` and `haiku` model choices resolve to. Claude Code also uses the Haiku model for background tasks, so add all three to the project's catalogue, or set them to models it has.
- A model variable may also name an alias of the project, or an OpenAI model. Kaeso translates the requests.

Keep the key in your user settings or your shell profile, never in a project's settings file that is committed.

> [!TIP]
> When Claude Code works with an OpenAI model, Kaeso refuses some pre-release Anthropic fields it can't translate, and the error says so. Add `"CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS": "1"` to `env` to leave them out.

## Codex

Codex speaks the OpenAI Responses format. Add Kaeso as a model provider in `~/.codex/config.toml`:

```toml
model = "gpt-5-mini"
model_provider = "kaeso"

[model_providers.kaeso]
name = "Kaeso"
base_url = "https://kaeso.example.com/v1"
env_key = "KAESO_API_KEY"
wire_api = "responses"
```

Then put the key in the environment before starting Codex:

```bash
export KAESO_API_KEY=sk-kso-…
codex
```

`model` may name any model in the project's catalogue, including a Claude model or an alias.

> [!NOTE]
> Kaeso is stateless: it doesn't store responses for later turns. Requests that ask the provider to keep or continue a stored response (`store: true`, `previous_response_id`) are refused with an error that says so.

## Other tools

Any tool that lets you set an OpenAI or Anthropic base URL works the same way:

| Tool speaks | Base URL | Key |
| --- | --- | --- |
| OpenAI Chat Completions or Responses | `https://kaeso.example.com/v1` | as the API key or bearer token |
| Anthropic Messages | `https://kaeso.example.com` | as the API key (`x-api-key`) or bearer token |

[Model API](../api/model-api.md) lists every endpoint, and [format translation](../concepts/format-translation.md) what can and can't cross between formats.
