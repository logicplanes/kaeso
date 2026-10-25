---
title: Coding tools
description: Point Claude Code, Codex and other coding tools at your install.
order: 3
---

Coding tools call the same APIs as any app, so they need only a base URL and an API key. Their requests then show up in the request logs and count against the key's limits and budgets.

## Create a key for each tool

Give each person and each tool an [API key](../console/api-keys.md) of its own, so their usage and limits stay apart. The key's project needs every model the tool asks for in its catalogue.

## Set up Claude Code

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

- `ANTHROPIC_AUTH_TOKEN` sends the key as a bearer token, which Kaeso accepts. Claude Code then skips its own sign-in.
- The three model variables choose what the `opus`, `sonnet` and `haiku` choices call. Claude Code also uses the Haiku model for background tasks, so all three must be in the project's catalogue.
- A model variable can also name an alias of the project or an OpenAI model. Kaeso translates the requests.

With an OpenAI model behind it, Claude Code may send pre-release Anthropic fields that Kaeso can't translate. Kaeso then answers with an error that names them. Add `"CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS": "1"` to `env` to leave them out.

Claude Code's `WebSearch` uses the web search of GPT and Grok models. Models served through Chat Completions, such as Mistral, have no web search, so they answer `WebSearch` without searching.

> [!WARNING]
> Keep the key in your user settings or your shell profile. A project's settings file gets committed, and everyone who can read the repository could use the key.

## Set up Codex

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

`model` can name any model in the project's catalogue, including a Claude model or an alias. Codex reads the key from the environment, so set it before you start Codex:

```bash
export KAESO_API_KEY=sk-kso-…
codex
```

Codex sends OpenAI-only parts with every turn. Kaeso translates them for other models: it turns Codex's tool groups and MCP tools into plain tools, and its web search into the model's own web search. A thread can switch models: the history of a GPT model's tools carries over.

Models without web search, such as Mistral, get the request without it. Claude and Grok search only live, while Codex's default, `web_search = "cached"`, asks for OpenAI's cache of pages, so with that default they get no web search either. Set `web_search = "live"` to let them search.

> [!NOTE]
> Kaeso stores no responses between turns. It refuses a request that asks the provider to store or continue a response (`store: true`, `previous_response_id`), with an error that says so.

## Set up other tools

Any tool that lets you set an OpenAI or Anthropic base URL works the same way:

| The tool speaks | Base URL | Send the key as |
| --- | --- | --- |
| OpenAI Chat Completions or Responses | `https://kaeso.example.com/v1` | The API key or a bearer token |
| Anthropic Messages | `https://kaeso.example.com` | The API key (`x-api-key`) or a bearer token |

## Next steps

- [API keys](../console/api-keys.md): Limit a tool's key to the models it needs, or give it an expiry date.
- [Limits and budgets](../console/limits-and-budgets.md): Cap how many tokens or how much money a tool can spend.
- [Format translation](../concepts/format-translation.md): See what carries across when a tool calls another provider's model.
- [MCP server](../api/mcp.md): Let a coding tool manage your install, with the rights you choose.
