---
title: 响应缓存
description: 用已存储的回复应答重复的相同请求，不产生服务商费用，并选择回复的保留时长。
order: 9
---

响应缓存会用服务商第一次给出的回复，应答项目已经逐字节发送过的请求。应用在几毫秒内就能收到回复，服务商也不收取任何费用。它适合重复运行相同用例的评测套件、处理常见问题的客服机器人，以及超时后的重试。

在您为项目开启之前，缓存处于关闭状态。开启后，将同一个请求发送两次，第二个响应会带有 `kaeso-cache: hit`：

```bash tab="curl"
curl -i https://kaeso.example.com/v1/messages \
  -H "x-api-key: $KAESO_API_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "content-type: application/json" \
  -d '{
    "model": "claude-haiku-4-5",
    "max_tokens": 256,
    "messages": [{"role": "user", "content": "What are your opening hours?"}]
  }'
```

```python tab="Python"
import os
from anthropic import Anthropic

client = Anthropic(
    base_url="https://kaeso.example.com",
    api_key=os.environ["KAESO_API_KEY"],
)

for _ in range(2):
    raw = client.messages.with_raw_response.create(
        model="claude-haiku-4-5",
        max_tokens=256,
        messages=[{"role": "user", "content": "What are your opening hours?"}],
    )
    print(raw.headers.get("kaeso-cache"))
```

```ts tab="TypeScript"
import Anthropic from "@anthropic-ai/sdk";

const client = new Anthropic({
  baseURL: "https://kaeso.example.com",
  apiKey: process.env.KAESO_API_KEY,
});

for (let i = 0; i < 2; i++) {
  const { response } = await client.messages
    .create({
      model: "claude-haiku-4-5",
      max_tokens: 256,
      messages: [{ role: "user", content: "What are your opening hours?" }],
    })
    .withResponse();
  console.log(response.headers.get("kaeso-cache"));
}
```

## 开启缓存

1. 打开项目的**设置**。
2. 在**响应缓存**下，开启**缓存回复**。
3. 在**回复保留时长**中，选择回复在缓存中保留多久：5 分钟到 7 天。默认为 1 小时。

更改项目设置需要**编辑项目**权限。新的时长适用于此后存储的回复。

## 哪些请求算作相同

Kaeso 比较的是转换后将发送给服务商的请求，因此来自 OpenAI 格式应用和 Anthropic 格式应用的同一请求可以共用一个条目。任何差异都会导致未命中：提示词、模型、`max_tokens`、`temperature`、工具，或 `anthropic-beta` 等请求头。

- **在项目内共享。** 项目的所有密钥和成员共享其条目。其他项目和其他组织永远无法读取它们。
- **每个请求一个回复。** 命中时返回模型第一次给出的回复，即使 `temperature` 会让新的调用给出不同回答。
- **会缓存。** 发往 `/v1/chat/completions`、`/v1/responses`、`/v1/messages` 和 `/v1/embeddings` 的成功非流式请求。
- **不会缓存。** 流式请求、Token 计数、错误，以及超过实例允许大小的回复（除非运维人员更改，否则为 1 MiB）。

## 控制单个请求

应用通过标准请求头 `Cache-Control` 按请求决定：

| `Cache-Control` | Kaeso 的处理 |
| --- | --- |
| 无 | 尽可能从缓存应答，并存储新的回复 |
| `no-cache` | 重新询问服务商，并用新回复替换旧回复 |
| `no-store` | 跳过缓存：不查找，也不存储 |

开启缓存的项目，其每个响应都带有 `kaeso-cache`：`hit`、`miss` 或 `bypass`（不可缓存，或因 `no-store` 跳过）。命中时还带有 `age`，即回复存储以来的秒数。

## 费用、限额和日志

命中不产生费用：在用量、请求日志和支出预算中，其费用均为 0。其 Token 仍按存储的回复所报告的数量计入，因此用量显示应用实际收到的内容。请求和 Token 限额与其他请求一样计入命中（[限额和预算](limits-and-budgets.md)）。

请求日志在**响应缓存**下显示每个请求的结果（[请求日志](request-logs.md)），Platform API 的日志列表可以用 `response_cache` 按它筛选。命中没有首字节时间，也没有服务商请求 ID。

## 清除缓存

在**响应缓存**下，选择**清除缓存**。已存储的回复会立即在每个网关实例上停止提供。关闭**缓存回复**也会清除缓存。

## 为组织允许或禁止

组织管理员决定项目是否可以使用缓存。打开**组织设置**，然后打开**日志记录**，关闭或开启**允许响应缓存**。关闭后，缓存会立即在每个项目中停止，并清除它们的条目。重新开启时，各项目自己的开关保持原样。此操作需要**编辑组织**权限。

运维人员可以使用[响应缓存设置](../self-hosting/configuration.md#响应缓存)为整个实例关闭缓存或调整缓存。

> [!NOTE]
> Kaeso 将缓存的回复保存在 Valkey 中，并用实例的主密钥加密。条目的键由请求通过带密钥的哈希派生，因此 Valkey 从不以可读形式保存提示词。

## 下一步

- [请求日志](request-logs.md)：查看哪些请求由缓存应答。
- [限额和预算](limits-and-budgets.md)：限制项目可以支出的金额。
- [模型 API](../api/model-api.md#响应头)：了解 Kaeso 添加到响应中的每个响应头。
