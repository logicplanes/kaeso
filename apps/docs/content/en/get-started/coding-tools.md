---
title: Coding tools
description: Point Claude Code, Codex, Gemini CLI and other coding tools at your install.
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

Claude Code prices tokens with its own table of Claude prices, so its `/cost` and `total_cost_usd` are wrong for other models. The [request logs](../console/request-logs.md) show Kaeso's cost for each request.

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
> Kaeso stores no responses between turns. It refuses a request that asks the provider to store or continue a response (`store: true`, `previous_response_id`), with an error that says so, unless the project allows [responses stored at the provider](../console/organizations-and-projects.md#stored-responses).

## Set up Gemini CLI

Gemini CLI speaks the Gemini API. Set the base URL without a version, since Gemini CLI adds `/v1beta` itself, and choose the model with `-m`:

```bash
export GOOGLE_GEMINI_BASE_URL=https://kaeso.example.com
export GEMINI_API_KEY=sk-kso-…
gemini -m claude-sonnet-5-5
```

Tell Gemini CLI to sign in with the key instead of a Google account, in `~/.gemini/settings.json`:

```json
{ "security": { "auth": { "selectedType": "gemini-api-key" } } }
```

Gemini CLI sends `topK` with every request. Models without top-k sampling, such as GPT and DeepSeek, get the request without it, and the reply's `kaeso-left-out` header names it.

## Set up Qwen Code

Qwen Code speaks OpenAI Chat Completions. Choose OpenAI as its sign-in and the model with `-m`:

```bash
export OPENAI_BASE_URL=https://kaeso.example.com/v1
export OPENAI_API_KEY=sk-kso-…
qwen --auth-type openai -m claude-sonnet-5-5
```

## Set up OpenCode

[OpenCode](https://opencode.ai/docs/providers/) takes Kaeso as a provider. List chat models with an empty entry. OpenCode calls decision models in the System One format, which Kaeso serves at `/v1/systemone`, so mark each [decision model](../api/decisions.md) with `type: "decision"`:

```jsonc
{
  "$schema": "https://opencode.ai/config.json",
  "providers": {
    "kaeso": {
      "name": "Kaeso",
      "env": ["KAESO_API_KEY"],
      "package": "@opencode/ai/providers/openai-compatible",
      "settings": { "baseURL": "https://kaeso.example.com/v1" },
      "models": {
        "claude-sonnet-5-5": {},
        "jev-latest": {
          "type": "decision",
          "compatibility": { "mediaInput": "assets" },
        },
      },
    },
  },
}
```

The model's key is its name in your Kaeso project. `mediaInput: "assets"` makes OpenCode send images as System One assets, the form Kaeso reads; Kaeso places them where the model's provider takes them.

## Set up other tools

Any tool that lets you set an OpenAI or Anthropic base URL works the same way:

| The tool speaks | Base URL | Send the key as |
| --- | --- | --- |
| OpenAI Chat Completions or Responses | `https://kaeso.example.com/v1` | The API key or a bearer token |
| Anthropic Messages | `https://kaeso.example.com` | The API key (`x-api-key`) or a bearer token |

Some tools need more than the base URL:

| Tool | What to set |
| --- | --- |
| Crush | A provider with `type: "openai-compat"` and `base_url: "https://kaeso.example.com/v1"`, or `type: "anthropic"` and `https://kaeso.example.com`. Give each model a `context_window` and a `default_max_tokens`. |
| Goose | `OPENAI_HOST=https://kaeso.example.com`, without `/v1`: Goose adds `v1/chat/completions` itself. For Anthropic Messages, `ANTHROPIC_HOST` instead. |
| Aider | `--model openai/<model>` with `OPENAI_API_BASE=https://kaeso.example.com/v1`, or `--model anthropic/<model>` with `ANTHROPIC_API_BASE=https://kaeso.example.com`. Add `--no-show-model-warnings` for models Aider doesn't know. Its token counts are its own estimates. |
| Continue | A model with `provider: openai` and `apiBase: https://kaeso.example.com/v1`, or `provider: anthropic` and `apiBase: https://kaeso.example.com/v1/`. |
| Open WebUI | `OPENAI_API_BASE_URL=https://kaeso.example.com/v1` and `OPENAI_API_KEY`. For its document search through Kaeso, `RAG_EMBEDDING_ENGINE=openai`, `RAG_OPENAI_API_BASE_URL`, `RAG_OPENAI_API_KEY`, and an embeddings model of the project in `RAG_EMBEDDING_MODEL`. |

## Next steps

- [API keys](../console/api-keys.md): Limit a tool's key to the models it needs, or give it an expiry date.
- [Limits and budgets](../console/limits-and-budgets.md): Cap how many tokens or how much money a tool can spend.
- [Format translation](../concepts/format-translation.md): See what carries across when a tool calls another provider's model.
- [MCP server](../api/mcp.md): Let a coding tool manage your install, with the rights you choose.
