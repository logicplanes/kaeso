---
title: 防护规则
description: 让您自己的服务在模型收到请求之前检查请求，并在应用收到回复之前检查回复。
order: 4
---

防护规则是您组织的一个 HTTP 服务。请求发送给模型之前，以及在您选择时模型的回复返回之前，Kaeso 都会询问它。它可以放行请求、拦截请求或改写请求中的文本。Kaeso 知道每个请求由哪个密钥、哪个成员和哪个项目发送，请求日志会保留每一次判定。

您需要 `guardrails.view` 才能查看**组织设置** › **防护规则**，需要 `guardrails.manage` 才能修改。

## 添加防护规则

1. 打开**组织设置** › **防护规则**，选择**添加防护规则**。
2. 输入名称以及 Kaeso 发送请求的地址，例如 `https://guardrail.example.com/check`。防护规则拦截应用时，应用会看到这个名称。
3. 选择它检查的内容：**请求**、**回复**或两者。对于回复，选择流式回复是不经检查直接发出（**不检查直接流式输出**），还是等待检查（**暂扣并检查**）。
4. 设置超时时间，默认 2000 毫秒，最多 10000 毫秒。
5. 选择防护规则无法回答时的处理方式：**拒绝请求**或**放行**。
6. 选择**添加防护规则**并复制签名密钥。它只显示一次。

位于私有网络的地址只有在安装允许其网络（`guardrails.allowed_networks`，见[配置](../self-hosting/configuration.md#防护规则)）时才可用。纯 `http` 只能用于这些网络，Kaeso 也从不跟随重定向。

每行的菜单中有**编辑**、**测试**、**新密钥**和**删除**。**新密钥**会立即替换签名密钥。**测试**会像真实请求一样发送一段示例提示词，或一段提示词加一条回复，并显示判定结果；示例的任何内容都不会被保存。

## 选择项目

在有项目使用之前，防护规则不会检查任何内容。打开项目的**设置**，在**防护规则**下选择**选择防护规则**。勾选防护规则并排好顺序：请求按从上到下的顺序经过它们，每一条看到的都是上一条处理后的文本。

只有在组织中拥有 `guardrails.manage` 的人才能选择防护规则。项目自己的管理员可以看到列表，但不能移除防护规则：它是组织的策略。

## Kaeso 检查哪些请求

防护规则检查聊天请求：OpenAI Chat Completions 和 Responses、Anthropic Messages、Gemini 以及 Playground。嵌入、重排序、决策、图像、音频、视频、实时会话和 token 计数不经检查直接通过。

请求阶段每个请求只运行一次：在 Kaeso 读取请求并确认调用方可以使用该模型之后，在[响应缓存](response-cache.md)和[别名路由](../concepts/alias-routes.md)的第一步之前。被拦截的请求永远不会由缓存回答，路由的每一步收到的都是检查后的文本。

已超出某项请求速率的调用方会在 Kaeso 询问防护规则之前被拒绝。被拦截的请求不会到达任何服务商，也不产生费用，但会作为一个请求记入日志并计入请求速率。

回复阶段在 Kaeso 为应用写出完整回复之前运行，来自响应缓存的回复也包括在内。只有当防护规则暂扣流时，流式回复才会被检查：Kaeso 会读取服务商的完整流，检查后再以流的形式发给应用，因此应用等待第一个 token 的时间会更长。

## 请求

Kaeso 向防护规则的地址发送 JSON：

```json
{
  "id": "req_01KA2Z8M3C0Y6V1W9T4H5J7Q2E.request",
  "phase": "request",
  "request_id": "req_01KA2Z8M3C0Y6V1W9T4H5J7Q2E",
  "organization": "org_01K9ZB2M7VQ6CG3N8R4T5W1X2Y",
  "project": "proj_01K9ZB3D1F8H2J4K6M8N0P2Q4R",
  "key": "key_01K9ZB4E2G9J3K5M7P9R1T3V5X",
  "member": "usr_01K9ZB1A6C8E0G2J4L6N8Q0S2U",
  "service_account": null,
  "model": "claude-haiku-4-5",
  "format": "anthropic_messages",
  "source": "api",
  "messages": [
    { "role": "system", "content": [{ "type": "text", "text": "Answer in one sentence." }] },
    {
      "role": "user",
      "content": [
        { "type": "text", "text": "What is in this picture?" },
        { "type": "image", "media_type": "image/png", "size": 48213 }
      ]
    }
  ]
}
```

- `phase` 为 `request` 或 `reply`。检查回复时会附加 `reply`，即包含回复各部分的一条 `assistant` 消息。
- `model` 是应用发送的名称，包括别名。`format` 是应用的格式，`source` 为 `api`、`playground` 或 `test`；测试不会给出项目。
- 部分可以是 `text`、`refusal`、`tool_call`（带 `id`、`name` 以及作为 JSON 文本的 `arguments`）、`tool_result`（带 `call_id`、`is_error` 和自己的 `content`）、`json`、`reasoning` 或 `other`。图像、音频、视频和文档分别为 `image`、`audio`、`video` 和 `document`，只带 `media_type` 和以字节计的 `size`，从不包含其字节内容。

## 回答

防护规则以 JSON 对象和 2xx 状态码回答：

```json
{ "action": "allow" }
```

```json
{ "action": "block", "message": "Customer data can't go to a model.", "category": "pii" }
```

```json
{
  "action": "replace",
  "category": "pii",
  "messages": [
    { "role": "system", "content": [{ "type": "text", "text": "Answer in one sentence." }] },
    {
      "role": "user",
      "content": [
        { "type": "text", "text": "What is in this picture?" },
        { "type": "image", "media_type": "image/png", "size": 48213 }
      ]
    }
  ]
}
```

- 每种操作都可以带可选的 `category`，它会保存在日志中，最多 64 个字符。
- `message` 是可选的；Kaeso 会把它附加到应用收到的错误中，最多 300 个字符。
- `replace` 返回 Kaeso 发送的消息，角色和部分相同、顺序相同。Kaeso 采用其中的文本、工具调用参数和工具结果，媒体、ID 和名称保持不变。对于回复，`messages` 中只有那一条 `assistant` 消息。暂扣的流无法改写。

Kaeso 会忽略它不认识的字段。不属于以上任何一种的回答、非 2xx 状态码、形状不同的 `replace`，或在超时时间内没有回答，都视为失败：此时由防护规则的**出错时**设置决定。

## 校验签名

Kaeso 像 [Standard Webhooks](https://www.standardwebhooks.com) 的投递一样为每个请求签名。请求头 `webhook-id`、`webhook-timestamp` 和 `webhook-signature` 分别携带投递 ID、以 Unix 秒计的时间，以及 `v1,` 加上 `id.timestamp.body` 的 base64 编码 HMAC-SHA256。密钥是去掉 `whsec_` 前缀后经 base64 解码的签名密钥。任何 Standard Webhooks 库都能校验它；请拒绝时间戳超过几分钟的请求。

## 应用看到的内容

被拦截的请求会以应用自身的格式收到 400，错误类型为内容过滤。消息会写明防护规则并附上其原因，例如 "The guardrail `PII check` blocked this request: Customer data can't go to a model." 被拦截的回复则会写 "blocked the model's reply"。Kaeso 不会回退到路由的其他步骤，因为每一步收到的都是同一个请求。

设为出错时拒绝的防护规则失败时，应用会收到 503（OpenAI 格式中为 500）："The guardrail `PII check` could not check this request, so Kaeso refused it; try again shortly."

每个经过检查的回复都带有 `kaeso-guardrails`，这是一个结构化字段列表，每条防护规则一项：名称、`phase`、`action`、`category`、`failure` 以及以毫秒计的 `dur`。

```http
kaeso-guardrails: "PII check";phase=request;action=replace;category="pii";dur=18
```

[请求日志](request-logs.md)的检查器在**防护规则**下显示相同的内容，**防护规则**筛选可以找出被防护规则放行、拦截或改写的请求。`kaeso_guardrail_runs_total{phase, action}` 和 `kaeso_guardrail_duration_seconds{phase}` 在[指标](../self-hosting/monitoring.md)中统计它们。

## 后续步骤

- [请求日志](request-logs.md)：找出被防护规则拦截或改写的请求。
- [请求流程](../concepts/request-flow.md)：了解防护规则在网关各项检查中的位置。
- [错误](../api/errors.md)：了解防护规则拒绝请求时应用收到的错误。
