---
title: Coding tools
description: Use Kaeso from Claude Code, Codex and other tools that speak an OpenAI or Anthropic API.
order: 3
---

Coding tools call the same APIs as any app, so a base URL and an API key are all they need. Their requests then show up in the request logs and count against the key's limits and budgets.

Give each person and each tool a key of its own, so their usage and limits stay apart. The key's project needs every model the tool asks for in its catalogue.

## Claude Code

Claude Code speaks Anthropic Messages. Point it at your install in `~/.claude/settings.json`:

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

- `ANTHROPIC_AUTH_TOKEN` sends the key as a bearer token, which Kaeso accepts, and makes Claude Code skip its own sign-in.
- The three model variables choose what the `opus`, `sonnet` and `haiku` choices call. Claude Code also uses the Haiku model for background tasks, so all three must name models in the project's catalogue.
- A model variable can also name an alias of the project or an OpenAI model; Kaeso translates the requests.

Keep the key in your user settings or your shell profile, never in a project's settings file that gets committed.

> [!TIP]
> With an OpenAI model behind it, Claude Code may send pre-release Anthropic fields that Kaeso can't translate, and Kaeso refuses them with an error that names them. Add `"CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS": "1"` to `env` to leave them out.

## Codex

Codex speaks OpenAI Responses. Add Kaeso as a model provider in `~/.codex/config.toml`:

```toml
model = "gpt-5-mini"
model_provider = "kaeso"

[model_providers.kaeso]
name = "Kaeso"
base_url = "https://kaeso.example.com/v1"
env_key = "KAESO_API_KEY"
wire_api = "responses"
```

Codex reads the key from the environment:

```bash
export KAESO_API_KEY=sk-kso-…
codex
```

`model` can name any model in the project's catalogue, a Claude model or an alias included.

> [!NOTE]
> Kaeso stores no responses between turns. A request that asks the provider to store or continue a response (`store: true`, `previous_response_id`) is refused with an error that says so.

## Other tools

Any tool that lets you set an OpenAI or Anthropic base URL works the same way:

| The tool speaks | Base URL | Key |
| --- | --- | --- |
| OpenAI Chat Completions or Responses | `https://kaeso.example.com/v1` | As the API key or bearer token |
| Anthropic Messages | `https://kaeso.example.com` | As the API key (`x-api-key`) or bearer token |

[Model API](../api/model-api.md) lists every endpoint, and [format translation](../concepts/format-translation.md) what crosses between formats and what doesn't.
