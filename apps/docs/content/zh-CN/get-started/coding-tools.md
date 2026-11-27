---
title: 编程工具
description: 让 Claude Code、Codex、Gemini CLI 和其他编程工具使用您的实例。
order: 3
---

编程工具调用的 API 与任何应用相同，因此只需要一个基础 URL 和一个 API 密钥。之后，它们的请求会出现在请求日志中，并计入该密钥的限额和预算。

## 为每个工具创建密钥

为每个人和每个工具分别创建 [API 密钥](../console/api-keys.md)，让各自的用量和限额互不影响。密钥所属项目的目录中，必须包含工具会请求的每个模型。

## 设置 Claude Code

Claude Code 使用 Anthropic Messages。在 `~/.claude/settings.json` 中把它指向您的实例：

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

- `ANTHROPIC_AUTH_TOKEN` 以 Bearer 令牌发送密钥，Kaeso 接受这种方式。Claude Code 随后会跳过自己的登录。
- 三个模型变量决定 `opus`、`sonnet` 和 `haiku` 选项调用哪个模型。Claude Code 还会用 Haiku 模型执行后台任务，因此这三个模型都必须在项目目录中。
- 模型变量也可以填写项目的别名或 OpenAI 模型，Kaeso 会转换请求。

背后是 OpenAI 模型时，Claude Code 可能会发送 Kaeso 无法转换的 Anthropic 预发布字段。Kaeso 会返回一个指出这些字段的错误。在 `env` 中加入 `"CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS": "1"` 即可不发送它们。

Claude Code 的 `WebSearch` 会使用 GPT 和 Grok 模型自带的网页搜索。通过 Chat Completions 提供的模型（例如 Mistral）没有网页搜索，因此它们会在不搜索的情况下回答 `WebSearch`。

Claude Code 用它自己的 Claude 价格表计算 token 费用，因此对其他模型而言，它的 `/cost` 和 `total_cost_usd` 并不准确。[请求日志](../console/request-logs.md)显示 Kaeso 为每个请求计算的费用。

> [!WARNING]
> 请把密钥放在用户设置或 shell 配置文件中。项目的设置文件会被提交，能读取该仓库的任何人都可能使用这个密钥。

## 设置 Codex

Codex 使用 OpenAI Responses。在 `~/.codex/config.toml` 中把 Kaeso 添加为模型服务商：

```toml
model = "gpt-5-mini"
model_provider = "kaeso"

[model_providers.kaeso]
name = "Kaeso"
base_url = "https://kaeso.example.com/v1"
env_key = "KAESO_API_KEY"
wire_api = "responses"
```

`model` 可以填写项目目录中的任意模型，包括 Claude 模型或别名。Codex 从环境变量中读取密钥，因此请在启动 Codex 之前设置它：

```bash
export KAESO_API_KEY=sk-kso-…
codex
```

Codex 每一轮都会发送只有 OpenAI 认识的部分。Kaeso 会为其他模型转换它们：把 Codex 的工具分组和 MCP 工具变为普通工具，把它的网页搜索变为模型自己的网页搜索。线程可以切换模型：GPT 模型的工具历史会一并保留。

没有网页搜索的模型（例如 Mistral）收到的请求不含网页搜索。Claude 和 Grok 只做实时搜索，而 Codex 的默认值 `web_search = "cached"` 要求使用 OpenAI 的页面缓存，因此使用该默认值时它们也没有网页搜索。设置 `web_search = "live"` 即可让它们搜索。

> [!NOTE]
> Kaeso 不会在轮次之间保存响应。如果请求要求服务商保存或延续某个响应（`store: true`、`previous_response_id`），Kaeso 会拒绝它，并在错误中说明原因，除非项目允许[在服务商处存储回复](../console/organizations-and-projects.md#存储的回复)。

## 设置 Gemini CLI

Gemini CLI 使用 Gemini API。先在 `~/.gemini/settings.json` 中让它用密钥登录，而不是用 Google 账号；否则已保存的 Google 登录会优先：

```json
{ "security": { "auth": { "selectedType": "gemini-api-key" } } }
```

然后设置不带版本的基础 URL（Gemini CLI 会自己加上 `/v1beta`），并用 `-m` 选择模型：

```bash
export GOOGLE_GEMINI_BASE_URL=https://kaeso.example.com
export GEMINI_API_KEY=sk-kso-…
gemini -m claude-sonnet-5-5
```

Gemini CLI 在每个请求中都发送 `topK`。没有 top-k 采样的模型（例如 GPT 和 DeepSeek）收到的请求不含它，回复的 `kaeso-left-out` 响应头会注明。

## 设置 Qwen Code

Qwen Code 使用 OpenAI Chat Completions。选择 OpenAI 作为登录方式，并用 `-m` 选择模型：

```bash
export OPENAI_BASE_URL=https://kaeso.example.com/v1
export OPENAI_API_KEY=sk-kso-…
qwen --auth-type openai -m claude-sonnet-5-5
```

## 设置 OpenCode

[OpenCode](https://opencode.ai/docs/providers/) 可以把 Kaeso 作为服务商。用空条目列出聊天模型。OpenCode 以 System One 格式调用决策模型，Kaeso 在 `/v1/systemone` 提供该格式，因此请用 `type: "decision"` 标记每个[决策模型](../api/decisions.md)：

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

模型的键是它在 Kaeso 项目中的名称。`mediaInput: "assets"` 让 OpenCode 以 System One 资源的形式发送图片，这是 Kaeso 读取的形式；Kaeso 会把图片放到模型服务商要求的位置。

## 设置其他工具

任何可以设置 OpenAI 或 Anthropic 基础 URL 的工具，都可以用同样的方式接入：

| 工具使用的格式 | 基础 URL | 密钥的发送方式 |
| --- | --- | --- |
| OpenAI Chat Completions 或 Responses | `https://kaeso.example.com/v1` | API 密钥或 Bearer 令牌 |
| Anthropic Messages | `https://kaeso.example.com` | API 密钥（`x-api-key`）或 Bearer 令牌 |

有些工具需要的不只是基础 URL：

| 工具 | 需要设置的内容 |
| --- | --- |
| Crush | 一个服务商，`type: "openai-compat"` 加 `base_url: "https://kaeso.example.com/v1"`，或 `type: "anthropic"` 加 `https://kaeso.example.com`。为每个模型设置 `context_window` 和 `default_max_tokens`。 |
| Goose | `OPENAI_HOST=https://kaeso.example.com`，不带 `/v1`：Goose 会自己加上 `v1/chat/completions`。使用 Anthropic Messages 时改用 `ANTHROPIC_HOST`。 |
| Aider | `--model openai/<model>` 加 `OPENAI_API_BASE=https://kaeso.example.com/v1`，或 `--model anthropic/<model>` 加 `ANTHROPIC_API_BASE=https://kaeso.example.com`。对 Aider 不认识的模型加上 `--no-show-model-warnings`。它的 token 数是自己的估算。 |
| Continue | 一个模型，`provider: openai` 加 `apiBase: https://kaeso.example.com/v1`，或 `provider: anthropic` 加 `apiBase: https://kaeso.example.com/v1/`。 |
| Open WebUI | `OPENAI_API_BASE_URL=https://kaeso.example.com/v1` 和 `OPENAI_API_KEY`。若要通过 Kaeso 进行文档搜索，设置 `RAG_EMBEDDING_ENGINE=openai`、`RAG_OPENAI_API_BASE_URL=https://kaeso.example.com/v1`（它不沿用聊天连接的地址）、`RAG_OPENAI_API_KEY`，并在 `RAG_EMBEDDING_MODEL` 中填写项目的一个嵌入模型。 |

## 下一步

- [API 密钥](../console/api-keys.md)：把工具的密钥限制在它需要的模型内，或为它设置过期日期。
- [限额和预算](../console/limits-and-budgets.md)：限制工具可以消耗的 Token 数量或金额。
- [格式转换](../concepts/format-translation.md)：了解工具调用其他服务商的模型时，哪些内容能够保留。
- [MCP 服务器](../api/mcp.md)：让编程工具按您选择的权限管理实例。
