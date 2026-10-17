---
title: 编程工具
description: 在 Claude Code、Codex 以及其他使用 OpenAI 或 Anthropic API 的工具中使用 Kaeso。
order: 3
---

编程工具调用的 API 与普通应用相同，只需一个基础 URL 和一个 API 密钥。之后，它们的请求会出现在请求日志中，并计入该密钥的限额和预算。

为每个人、每个工具各创建一个密钥，让用量和限额互不混淆。密钥所属项目的目录中必须包含工具会请求的每个模型。

## Claude Code

Claude Code 使用 Anthropic Messages。在 `~/.claude/settings.json` 中将其指向自己的安装：

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

- `ANTHROPIC_AUTH_TOKEN` 以 Bearer 令牌的形式发送密钥，Kaeso 接受这种方式；设置后，Claude Code 会跳过自己的登录。
- 三个模型变量决定 `opus`、`sonnet` 和 `haiku` 这三个选项调用哪个模型。Claude Code 还会用 Haiku 模型执行后台任务，因此三者都必须指向项目目录中的模型。
- 模型变量也可以填写项目的别名或 OpenAI 模型；Kaeso 会转换请求。

将密钥放在用户设置或 shell 配置文件中，切勿放在会被提交的项目设置文件里。

> [!TIP]
> 后端是 OpenAI 模型时，Claude Code 可能会发送 Kaeso 无法转换的 Anthropic 预发布字段，Kaeso 会以指明这些字段的错误拒绝它们。在 `env` 中添加 `"CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS": "1"` 即可省略它们。

## Codex

Codex 使用 OpenAI Responses。在 `~/.codex/config.toml` 中将 Kaeso 添加为模型服务商：

```toml
model = "gpt-5-mini"
model_provider = "kaeso"

[model_providers.kaeso]
name = "Kaeso"
base_url = "https://kaeso.example.com/v1"
env_key = "KAESO_API_KEY"
wire_api = "responses"
```

Codex 从环境变量中读取密钥：

```bash
export KAESO_API_KEY=sk-kso-…
codex
```

`model` 可以填写项目目录中的任意模型，包括 Claude 模型或别名。

> [!NOTE]
> Kaeso 不会在轮次之间保存响应。要求服务商保存或延续某个响应的请求（`store: true`、`previous_response_id`）会被拒绝，错误中会说明这一点。

## 其他工具

任何可以设置 OpenAI 或 Anthropic 基础 URL 的工具都可以用同样的方式接入：

| 工具使用的格式 | 基础 URL | 密钥 |
| --- | --- | --- |
| OpenAI Chat Completions 或 Responses | `https://kaeso.example.com/v1` | 作为 API 密钥或 Bearer 令牌 |
| Anthropic Messages | `https://kaeso.example.com` | 作为 API 密钥（`x-api-key`）或 Bearer 令牌 |

[模型 API](../api/model-api.md) 列出了所有端点，[格式转换](../concepts/format-translation.md)说明了哪些内容可以在格式之间传递、哪些不能。
