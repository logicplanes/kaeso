---
title: 目录格式
description: KPMR 目录格式 1 的每个字段，以及网关如何读取它们。
order: 2
---

目录是一个采用 Kaeso 自有格式的 JSON 文档，以 `application/vnd.kaeso.kpmr+json; version=1` 提供。本页列出格式 1 的每个字段。下面是一个只含一个服务商和一个模型的简化目录：

```json
{
  "format": 1,
  "revision": "2026-11-21T02:59:02Z-efb348278140",
  "generated_at": "2026-11-21T02:59:02Z",
  "refresh_seconds": 3600,
  "provider_ids": ["302ai", "alibaba", "anthropic"],
  "providers": {
    "anthropic": {
      "id": "anthropic",
      "name": "Anthropic",
      "docs_url": "https://docs.anthropic.com/en/docs/about-claude/models",
      "api_url": null,
      "env": ["ANTHROPIC_API_KEY"],
      "lines": {
        "claude-opus-latest": {
          "model": "claude-opus-5-5",
          "since": "2026-09-22",
          "previous": "claude-opus-5"
        }
      },
      "models": {
        "claude-opus-5-5": {
          "id": "claude-opus-5-5",
          "name": "Claude Opus 5.5",
          "family": "claude-opus",
          "kind": "generation",
          "status": "active",
          "release_date": "2026-09-22",
          "modalities": {
            "input": ["text", "image", "pdf"],
            "output": ["text"]
          },
          "limits": { "context": 1000000, "input": null, "output": 128000 },
          "pricing": {
            "unit": "usd_per_million_tokens",
            "input": "4",
            "output": "20",
            "cache_read": "0.2",
            "cache_write": "5",
            "cache_write_1h": "8",
            "tiers": []
          },
          "reasoning": {
            "efforts": ["low", "medium", "high", "xhigh", "max"],
            "adaptive_thinking": true,
            "thinking_binding": true
          },
          "decisions": null,
          "lifecycle": { "shutdown_on": null, "replacement": null },
          "gateway": { "egress_dialect": null, "router": false }
        }
      }
    }
  }
}
```

整个文档遵循四条规则：

- **每个键都存在。**未知的值为 `null`，而不是 `false`，因此读取方能区分“未知”和“否”。上面的示例只是为了简短才省略了部分键。
- **价格是精确的十进制字符串**，单位为美元，从不使用浮点数。请用十进制库进行加总。
- **服务商和模型是映射**，以 ID 为键并排序。
- **未知的键会被忽略。**格式 1 通过新增字段扩展，读取方忽略不认识的内容。会影响读取方的变更将成为格式 2，在网关迁移期间 KPMR 会同时提供两种格式。

## 文档

| 字段 | 含义 |
| --- | --- |
| `format` | 格式版本，`1` |
| `revision` | 内容的构建时间及其哈希。内容一变，它就会变 |
| `generated_at` | 此内容首次发布的时间，UTC |
| `refresh_seconds` | 再次请求前应等待的秒数 |
| `sources` | 数据的来源及其许可证，以及注册表的 `commit` 和条目数 `entries` |
| `provider_ids` | KPMR 知道的每个服务商的 ID，即使您只请求了其中一部分。[自定义服务商](../console/custom-providers.md)不能使用这些 ID |
| `providers` | 您请求的服务商，以 ID 为键。KPMR 不认识的 ID 会被略去 |

## 服务商

| 字段 | 含义 |
| --- | --- |
| `id` | Kaeso 使用的服务商 ID，例如 `anthropic`（[支持的服务商](../concepts/supported-providers.md)） |
| `name` | 服务商名称 |
| `docs_url` | 服务商的模型文档 |
| `api_url` | OpenAI 兼容 API 的基础地址（如果服务商提供） |
| `env` | 其 SDK 读取密钥的环境变量 |
| `models` | 服务商的模型，以其 API 接受的 ID 为键 |
| `lines` | 服务商的模型系列，以名称为键，例如 `claude-opus-latest`。没有时为空 |

### 系列

`lines` 的每个条目让一个名称指向该服务商的一个模型（[停用和模型系列](lifecycle-and-lines.md#模型系列)）：

| 字段 | 含义 |
| --- | --- |
| `model` | 该系列当前指向的模型 ID |
| `since` | 该模型发布并接管该系列的时间；未知时为 `null` |
| `previous` | 该系列之前指向的模型；只指向过一个模型的系列为 `null` |

## 模型

| 字段 | 含义 |
| --- | --- |
| `id` | 发送给服务商 API 的 ID，与模型的键相同 |
| `name` | 显示名称 |
| `description` | 一句话说明模型的用途 |
| `family` | 模型家族，例如 `claude-sonnet` |
| `kind` | 模型的用途：`generation`、`embedding`、`decision`、`image`、`video`、`speech`、`transcription`、`realtime`、`reranking`、`music` 或 `sound_effects`。遇到不认识的类型时按未知处理 |
| `status` | `active`、`preview`、`deprecated` 或 `retired`；随 `lifecycle` 中的日期变化 |
| `release_date` | 模型的发布日期，`YYYY-MM-DD`；只知道月份时为 `YYYY-MM` |
| `last_updated` | 模型的最后更新日期，格式相同 |
| `knowledge` | 训练数据截止时间，`YYYY-MM` 或 `YYYY-MM-DD` |
| `open_weights` | 是否公开权重 |
| `modalities` | `input` 和 `output`，各为由 `text`、`image`、`audio`、`video` 和 `pdf` 组成的列表。只做决策的模型 `output` 为空 |
| `limits` | `context`、`input` 和 `output`，单位为 Token |
| `pricing` | 模型的价格（[价格](#价格)） |
| `capabilities` | 模型接受什么（[能力](#能力)） |
| `reasoning` | 不推理的模型为 `null`；否则说明其推理方式（[推理](#推理)） |
| `decisions` | 决策模型能回答的问题；不做决策的模型为 `null`（[决策](#决策)） |
| `lifecycle` | 服务商何时停用该模型（[生命周期](#生命周期)） |
| `gateway` | 网关如何发送该模型的请求（[网关](#网关)） |

### 能力

| 字段 | 为 `true` 表示模型 |
| --- | --- |
| `tool_call` | 会调用工具 |
| `forced_tool_choice` | 接受强制调用工具的 `tool_choice`。为 `false` 时，Kaeso 会拒绝强制选择并要求使用 `auto` |
| `structured_output` | 按请求给出的 JSON Schema 回答 |
| `temperature` | 接受 `temperature` |
| `attachments` | 接受文件，例如图片或 PDF |
| `reasoning` | 回答前会推理 |
| `interleaved_reasoning` | 在一次回答中的工具调用之间推理 |

### 推理

| 字段 | 含义 |
| --- | --- |
| `efforts` | 模型接受的推理强度级别，例如 `low` 和 `high`；没有时为 `[]` |
| `adaptive_thinking` | 模型接受自适应思考，由它自己决定思考多久 |
| `thinking_binding` | 思考块必须与其所属的轮次放在一起 |

### 决策

决策模型用概率回答类型化问题，而不是写出文本（[决策 API](../api/decisions.md)）。只做决策的模型为 `kind: "decision"`，例如 `gemini-decision-1`：它的输出模态为空，因此不认识决策的网关不会把它作为聊天模型提供。也写文本的模型（例如 `gpt-6-luna`）保留其类型，同时带有 `decisions`。决策模型按输入 Token 计费；其输出价格为 `0`。

| 字段 | 含义 |
| --- | --- |
| `question_types` | `predicate`（是或否）、`choice`（多个选项之一）、`score`（有序量表上的一个级别）或 `box`（某物在图片中的位置） |
| `max_questions` | 一个请求最多可提的问题数 |
| `max_options` | 一个 `choice` 问题最多可有的选项数 |
| `max_levels` | 一个 `score` 问题最多可有的级别数 |
| `max_images` | 一个请求最多可发送的图片数 |

每个 `max_` 字段未知时为 `null`。

### 生命周期

每个键都存在，服务商宣布之前为 `null`。`status` 在 `deprecated_on` 变为 `deprecated`，在 `shutdown_on` 变为 `retired`（[停用和模型系列](lifecycle-and-lines.md)）。

| 字段 | 含义 |
| --- | --- |
| `announced_on` | 服务商宣布停用的日期，`YYYY-MM-DD` |
| `deprecated_on` | 模型被弃用的日期：仍可使用，但服务商希望应用尽快迁移 |
| `shutdown_on` | 服务商停止提供该模型的日期 |
| `replacement` | 服务商推荐替代使用的同一服务商模型的 ID |
| `source` | 服务商发布公告的页面 |

### 网关

| 字段 | 含义 |
| --- | --- |
| `egress_dialect` | 网关发送该模型请求所用的 API 格式，例如 `openai_responses`；服务商默认格式时为 `null`（[按服务商划分的 API](../concepts/provider-apis.md)） |
| `router` | 为 `true` 表示这是一个为每个请求选择其他模型的路由器 |

## 价格

`pricing` 包含以每百万 Token 美元计的十进制字符串，价格未知时为 `null`。

| 字段 | 含义 |
| --- | --- |
| `unit` | 始终为 `usd_per_million_tokens` |
| `input` | 输入 Token |
| `output` | 输出 Token。有 `output_image` 时仅指文本输出 |
| `cache_read` | 从提示缓存读取的输入 |
| `cache_write` | 写入缓存的输入，保留五分钟 |
| `cache_write_1h` | 写入缓存的输入，保留一小时，在服务商有公开说明时提供 |
| `reasoning` | 推理 Token，在其与输出分开计费时提供 |
| `input_audio`、`output_audio` | 音频输入和输出 Token |
| `input_image`、`output_image` | 图片输入和输出 Token |
| `cache_read_image` | 从缓存读取的图片输入 |
| `tiers` | 长上下文价格，`context_above` 最小的一档在前，没有则为空（[长上下文价格档](#长上下文价格档)） |
| `above_200k` | 长上下文价格的旧形式，为旧版读取方保留，通常为 `null`。请读取 `tiers`，其中有精确的阈值 |
| `per_unit` | 媒体模型按图片、秒或其他单位的价格（[按单位计价](#按单位计价)） |

### 长上下文价格档

当请求的提示词超过 `context_above` 个 Token 时，该请求的所有 Token 均按该档价格计费。档中值为 `null` 的价格沿用基础价格。例如，OpenAI 的 `gpt-5.4` 从 272,000 个 Token 起价格更高：

```json wrap
{ "context_above": 272000, "input": "5", "output": "22.5", "cache_read": "0.5", "cache_write": null }
```

| 字段 | 含义 |
| --- | --- |
| `context_above` | 超过多少个 Token 的提示词适用该档 |
| `input`、`output`、`cache_read`、`cache_write` | 该档的价格 |

### 按单位计价

媒体模型在 `per_unit` 中列出价格：一张图片、一秒视频或一百万个字符的费用。每个条目都说明其适用范围；值为 `null` 的条件适用于任何值。

| 字段 | 含义 |
| --- | --- |
| `per` | `image`、`megapixel`、`video`、`second`、`minute`、`hour`、`million_characters` 或 `search` |
| `of` | 计算输入（`input`）还是输出（`output`） |
| `usd` | 以美元计的价格，十进制字符串 |
| `size` | 适用的图片尺寸，例如 `1024x1024` |
| `quality` | 适用的质量，例如 `high` |
| `resolution` | 适用的分辨率，例如 `1080p` |
| `duration` | 按视频计价时，视频片段的长度（秒） |
| `audio` | 视频价格是否包含声音 |
| `minimum_seconds` | 按时间单位计费时的最短计费时长 |
| `estimate` | 服务商只公布估算价格时为 `true` |

## 网关如何读取目录

网关只读取它调用的服务商，并逐个检查每个模型：

- **无法读取的模型会被跳过**，目录的其余部分仍会应用。这包括键与 `id` 不同、`pricing.unit` 不是 `usd_per_million_tokens`，或价格不是 0 到 1,000,000 之间十进制数的模型。
- **无法读取的价格档或系列会被略去**，模型和服务商不受影响。
- **其他格式的目录**会被整体拒绝，继续使用当前数据。
- **这些值作为 KPMR 的值进入每个项目的目录**，项目可以覆盖它们（[模型和别名](../console/models-and-aliases.md#更改模型的参数)）。

## 下一步

- [KPMR API](api.md)：获取目录并验证其签名。
- [停用和模型系列](lifecycle-and-lines.md)：了解生命周期日期和模型系列如何随时间变化。
- [保持模型数据最新](sync.md)：设置网关获取目录的频率。
