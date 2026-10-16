---
title: 请求流程
description: 模型请求在网关中经过的各个步骤。
order: 1
---

每个模型请求在网关中都经过相同的路径。没有任何内容会未经转换就直接透传，也没有任何请求会被重试。途中的每次拒绝都是采用应用自身格式的错误。

```text
app ─▶ key ─▶ decode ─▶ model ─▶ limits ─▶ provider key ─▶ encode and send ─▶ provider
app ◀─ encode reply ◀─ decode reply ◀──────────────────────────────────────── provider
                             └─▶ usage, cost and request log
```

## 密钥

网关从 `x-api-key` 读取 API 密钥，否则从 `Authorization: Bearer` 读取。密钥决定所属项目，并以其成员或服务账号在该项目中的权限行事。密钥需要具有模型调用权限，其所有者需要 `api.request` 权限；组织平台密钥无法调用模型。过去 30 秒内查询过的密钥直接从内存中获取结果。

## 解码

请求正文最大可为 `KAESO_BODY_LIMITS_MODEL_API`（32 MiB）；更大的正文会以 413 被拒绝。KPP 将请求从应用的格式（例如 OpenAI Chat Completions 或 Anthropic Messages）解码为 Kaeso 的规范格式。无法读取的请求，或要求 Kaeso 从不发送给服务商的内容的请求（例如多个 choice 或存储的 Responses 状态），会在此被拒绝。

## 模型

模型名称在项目目录中解析：先查别名，再查 `provider/model`，最后查模型自身的 ID。不在目录中的名称会以 404 被拒绝，项目中有多个服务商提供的模型 ID 会以 400 被拒绝。随后检查成员和密钥的模型访问设置（403）。

目录条目给出模型的限制、价格、功能、调用服务商时使用的格式、是否进行推理、是否支持自适应思考，以及其超时。请求会据此进行检查，当请求需要模型不具备的功能时，以 400 被拒绝：

- 图像、PDF、工具、JSON schema 或推理；
- 模型未列出的推理强度等级；
- 对于不接受温度参数的模型，1 以外的温度，除非 Kaeso 开启了 Claude 的思考并省略了温度；
- 向生成模型请求嵌入向量，或向嵌入模型请求文本。

对 Claude 模型的请求如果没有设置最大输出 Token 数，会使用该模型的输出上限。

## 限额

所有适用的限额都在 Valkey 中一步同时检查：密钥、其成员和项目的速率与预算，包括针对所有模型和针对此模型的限额。超出速率或硬性预算的请求会以 429 和 `Retry-After` 被拒绝。软性预算不会拒绝请求。无法连接 Valkey 时，受硬性预算约束的请求会以 503 被拒绝，其他请求则不经检查直接通过。

## 服务商密钥

为模型的服务商选择项目的服务商密钥：分配给项目的密钥，否则为组织的默认密钥。如果没有，请求会以 400 被拒绝。密钥使用主密钥解密，仅保存在实例的内存中，最长 30 秒。

## 编码和发送

KPP 根据服务商的格式检查请求并对其编码；该格式无法表达的内容会在此被拒绝，在任何内容到达服务商之前（[格式转换](format-translation.md)）。对于通过其他格式提供服务的 Chat Completions 应用，会先放回之前轮次中保存的推理内容（[轮次之间的推理](format-translation.md#轮次之间的推理)）。

| 模型 | 调用的 API |
| --- | --- |
| OpenAI | Responses，或在目录如此指定时使用 Chat Completions；嵌入使用 Embeddings |
| Anthropic | Messages |

Token 计数请求发往 Anthropic 的 `/v1/messages/count_tokens` 或 OpenAI 的 `/v1/responses/input_tokens`。`KAESO_PROVIDERS_OPENAI_BASE_URL` 和 `KAESO_PROVIDERS_ANTHROPIC_BASE_URL` 设置服务商的地址。

| 超时 | 设置 | 默认值 | 限制对象 |
| --- | --- | --- | --- |
| 连接 | `KAESO_UPSTREAM_CONNECT_TIMEOUT` | 10 秒 | 连接到服务商，包括 TCP 和 TLS |
| 请求 | `KAESO_UPSTREAM_REQUEST_TIMEOUT` | 600 秒 | 非流式请求的全过程 |
| 首字节 | `KAESO_UPSTREAM_FIRST_BYTE_TIMEOUT` | 60 秒 | 流，直到服务商响应 |
| 空闲 | `KAESO_UPSTREAM_STREAM_IDLE_TIMEOUT` | 300 秒 | 流，两个数据块之间的最长静默时间 |

目录中模型自身的超时会取代请求超时和首字节超时。超时以 504 响应，无法连接服务商则以 502 响应（[服务商调用](../self-hosting/configuration.md#服务商调用)）。

## 回复

应用使用服务商的格式时，服务商的错误按服务商发送的原样到达；否则，错误会在应用的格式中保持其含义（[服务商错误](../api/errors.md#服务商错误)）。回复会被解码为规范格式，再编码为应用的格式，流式回复则逐个事件处理。回复从不会因其内容而被拒绝：应用格式无法表达的内容不会出现在应用收到的内容中，开启内容日志时则保存在请求日志中。

回复带有 `kaeso-model` 和 `kaeso-provider`。服务商一响应，流的响应头就会发送。当流处于打开且静默状态时，网关会写入应用的 SDK 会忽略的保活消息（[长时间的流](../self-hosting/domains-and-tls.md#长时间的流)）。

## 流失败时

流的响应头一旦发送，其状态码就无法更改。此后失败的流会以采用应用格式的错误事件结束，请求日志会将其记录为失败：

| 原因 | 记录的状态码 | 错误类型 |
| --- | --- | --- |
| 服务商发送了错误事件 | 该错误的状态码，否则为 502 | 该错误的类型 |
| 与服务商的连接中断，或流在回复完成前结束 | 502 | `stream_interrupted` |
| 服务商静默时间超过 `KAESO_UPSTREAM_STREAM_IDLE_TIMEOUT` | 504 | `timeout` |
| KPP 无法为应用编码某个事件 | 该错误的状态码，否则为 500 | 该错误的类型 |

它在用量和 `kaeso_stream_failures_total` 中计为错误，而 `kaeso_requests_total` 将其计为 200（[监控](../self-hosting/monitoring.md)）。请求日志中每个流式条目都会在 `stream_progress` 中注明其进行到的位置：服务商的事件、写入应用的字节数和保活消息，以及到第一个回复字节的耗时。

## 应用离开时

应用关闭连接时，网关会立即停止对服务商的请求，服务商随之停止生成。该请求记录为 499 `client_disconnected`。

流按服务商报告的用量计费，再加上对剩余部分的估算：

- 输入取自流开始时服务商的用量，否则取自其 Token 计数端点，否则根据请求的大小估算；
- 输出根据已收到的内容估算。

中途中断的流同样如此。此类日志条目注明 `usage.estimated: true`，费用和限额使用相同的数字。对于服务商不以流式发送的推理内容，输出估算会偏低。

## 网关关闭时

收到关闭信号时，网关不再接受新连接，并让已打开的请求和流继续运行 `KAESO_SHUTDOWN_GRACE`（30 秒）。届时仍未结束的请求会收到采用应用格式的错误“Kaeso is restarting”：状态码为 503，OpenAI 格式的应用为 500，SDK 会自动重试；已开始的流则改为以错误事件结束。它以错误类型 `shutdown` 计费并记录，流的用量为估算值。网关最多等待 10 秒，然后最后一次写入用量和日志。

## 用量和日志

请求的 Token 按请求时的模型价格计入密钥、成员和项目的费用，并计入其预算。日志条目记录元数据；当组织和项目都开启了内容日志时，还会记录请求和回复。日志记录从不阻塞请求：条目在内存中等待，并分批写入 ClickHouse。缓冲区已满时，会丢弃最早的条目。
