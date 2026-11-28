---
title: Antigravity
description: 用一个在每次调用后上报的钩子脚本，从 Antigravity 的转录中统计它的模型调用。
order: 4
---

Antigravity 没有 OpenTelemetry 导出，但它会运行钩子，并为每个会话保留一份转录，其中包含每次模型调用的 Token。一个小型钩子脚本会在每次调用后把新增的调用发送到 Kaeso 来源，它们随后成为项目中的已观测请求。

[添加一个](index.md#添加来源)类型为 **Antigravity** 的来源。把这个脚本保存为 `~/.gemini/config/kaeso-source.sh`，并填入您的安装地址和来源密钥。它需要 `curl` 和 `jq`：

```bash
KAESO_INGEST="https://kaeso.example.com/ingest/v1/antigravity"
KAESO_SOURCE_KEY="sk-kso-src-…"
STATE="${TMPDIR:-/tmp}/kaeso-antigravity"
hook=$(cat)
printf '{}'
exec > /dev/null 2>&1
(
  log=$(printf '%s' "$hook" | jq -r '.transcriptPath // empty')
  conversation=$(printf '%s' "$hook" | jq -r '.conversationId // empty')
  full="$(dirname "$log")/transcript_full.jsonl"
  [ -n "$conversation" ] && [ -f "$full" ] || exit 0
  mkdir -p "$STATE" && mark="$STATE/$conversation"
  sent=$(cat "$mark" 2> /dev/null || echo -1)
  body=$({ printf '%s\n' "$hook"; cat "$full"; } | jq -cs --argjson sent "$sent" '.[0] as $hook | [.[1:][] | select(.step_index > $sent)] as $new | [$new[] | select(.type == "PLANNER_RESPONSE") | .step_index + range(1; 1 + (.tool_calls // [] | length))] as $results | {hook: ($hook | del(.lastUserInput)), steps: ([$new[] | if .type == "PLANNER_RESPONSE" then {type, step_index, created_at, model, input_tokens, cache_read_tokens, cache_write_tokens, cache_creation_input_tokens, output_tokens, tool_calls: (.tool_calls // [] | map({name}))} elif (.step_index as $i | any($results[]; . == $i)) then {step_index, type, status, created_at} else empty end] | .[:1000])}')
  last=$(printf '%s' "$body" | jq '[.steps[].step_index] | max // empty')
  [ -n "$last" ] || exit 0
  printf '%s' "$body" | curl -fsS -m 30 "$KAESO_INGEST" -H "Authorization: Bearer $KAESO_SOURCE_KEY" \
    -H "Content-Type: application/json" --data-binary @- && printf '%s' "$last" > "$mark"
) < /dev/null &
```

然后把钩子加入 `~/.gemini/config/hooks.json`，或加入现有的该文件：

```json
{
  "kaeso": {
    "PostInvocation": [
      {
        "type": "command",
        "command": "sh ~/.gemini/config/kaeso-source.sh",
        "timeout": 15
      }
    ]
  }
}
```

在 Antigravity 中开始一个会话。第一次模型调用后，来源会显示**已连接**，调用也会出现在项目的请求日志中。

## 钩子如何工作

每次模型调用后，Antigravity 都会运行该脚本并等待其回答。脚本会立即回答，并在后台发送，因此不会有模型调用等待 Kaeso。它读取 Antigravity 指明的转录旁边的会话文件 `transcript_full.jsonl`，发送自上次发送以来新增的模型调用，每次最多 1,000 个步骤。每次调用都会附带它调用的工具名称，以及运行这些工具的步骤。它按会话在 `$TMPDIR/kaeso-antigravity` 下记住最后发送的调用。

工作区 `.agents/hooks.json` 中的钩子只上报该工作区的会话；`~/.gemini/config` 中的钩子上报所有会话。

## 保留提示词

脚本默认不发送您的最后一个提示词和每个步骤的文本。要保留它们，请为来源开启**保留提示词和回复**，然后从来源详情中重新复制脚本：开启内容后，脚本会发送它们。

## Kaeso 读取的内容

转录中的每个 `PLANNER_RESPONSE` 步骤都会成为一个请求，包含模型、输入 Token、缓存 Token、输出 Token 和时间。Antigravity 命名模型时会在末尾加上设置：推理强度级别（例如 `gemini-3.8-flash-low`）或 `thinking`（例如 `claude-opus-4-6-thinking`）。目录或 KPMR 认识该模型及其这项设置时，Kaeso 按该模型记录并计价请求，把级别保留为其推理强度，并根据模型确定服务商（[Token 与费用](../concepts/sources.md#token-与费用)）。因此各个级别显示为同一个模型，Claude 也归属 Anthropic。两者都不认识的模型保留完整名称且没有费用，但其 Token 仍会统计。

Antigravity 的转录不含耗时，因此其请求没有耗时，也不计入延迟数据。

对 MCP 工具的调用会成为工具调用，带有服务器和工具，以及运行它的步骤是否成功。在 Kaeso 读取工具调用之前复制的脚本不会发送工具调用；请从来源详情中重新复制脚本。

## 下一步

- [来源](../concepts/sources.md)：Kaeso 如何统计已观测的请求，以及请求属于谁。
- [Gemini CLI](gemini-cli.md)：统计 Gemini CLI 的请求和 MCP 工具调用。
