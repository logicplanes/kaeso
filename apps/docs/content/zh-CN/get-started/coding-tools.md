---
title: 编程工具
description: 在 Claude Code、Codex 以及其他支持 OpenAI 或 Anthropic API 的工具中使用 Kaeso。
order: 3
---

编程工具调用的 API 与普通应用相同，因此只需一个基础 URL 和一个 API 密钥即可通过 Kaeso 工作。之后，每个工具的请求都会出现在请求日志中，并计入该密钥的限额和预算。

为每个人和每个工具各创建一个密钥，使用量和限额就能互不影响。密钥所属项目的目录中必须包含该工具请求的所有模型。

## Claude Code

Claude Code 使用 Anthropic Messages 格式。在 `~/.claude/settings.json` 中将它指向自己的安装：

```json
{
  "env": {
    "ANTHROPIC_BASE_URL": "https://kaeso.example.com",
    "ANTHROPIC_AUTH_TOKEN": "sk-kso-…",
    "ANTHROPIC_DEFAULT_OPUS_MODEL": "claude-opus-4-8",
    "ANTHROPIC_DEFAULT_SONNET_MODEL": "claude-sonnet-5-5",
    "ANTHROPIC_DEFAULT_HAIKU_MODEL": "claude-haiku-4-5"
  }
}
```

- `ANTHROPIC_AUTH_TOKEN` 以 Bearer Token 的形式发送密钥，Kaeso 接受这种方式。设置了它之后，Claude Code 会跳过自己的登录。
- 三个模型变量决定 `opus`、`sonnet` 和 `haiku` 这几个模型选项分别对应哪个模型。Claude Code 还会将 Haiku 模型用于后台任务，因此请将三个模型都添加到项目的目录中，或者将它们设为目录中已有的模型。
- 模型变量也可以指定项目的某个别名，或者一个 OpenAI 模型。Kaeso 会转换请求。

将密钥保存在用户设置或 shell 配置文件中，切勿放在会被提交的项目设置文件里。

> [!TIP]
> 当 Claude Code 使用 OpenAI 模型时，Kaeso 会拒绝一些无法转换的 Anthropic 预发布字段，错误信息会说明这一点。在 `env` 中添加 `"CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS": "1"` 即可省略这些字段。

## Codex

Codex 使用 OpenAI Responses 格式。在 `~/.codex/config.toml` 中将 Kaeso 添加为模型服务商：

```toml
model = "gpt-5-mini"
model_provider = "kaeso"

[model_providers.kaeso]
name = "Kaeso"
base_url = "https://kaeso.example.com/v1"
env_key = "KAESO_API_KEY"
wire_api = "responses"
```

然后在启动 Codex 之前将密钥放入环境变量：

```bash
export KAESO_API_KEY=sk-kso-…
codex
```

`model` 可以指定项目目录中的任何模型，包括 Claude 模型或别名。

> [!NOTE]
> Kaeso 是无状态的：它不会为后续轮次保存响应。要求服务商保存或继续一个已保存响应的请求（`store: true`、`previous_response_id`）会被拒绝，并返回说明原因的错误。

## 其他工具

任何可以设置 OpenAI 或 Anthropic 基础 URL 的工具都以同样的方式工作：

| 工具使用的格式 | 基础 URL | 密钥 |
| --- | --- | --- |
| OpenAI Chat Completions 或 Responses | `https://kaeso.example.com/v1` | 作为 API 密钥或 Bearer Token |
| Anthropic Messages | `https://kaeso.example.com` | 作为 API 密钥（`x-api-key`）或 Bearer Token |

[模型 API](../api/model-api.md) 列出了所有端点，[格式转换](../concepts/format-translation.md)说明了哪些内容可以在格式之间转换、哪些不能。
