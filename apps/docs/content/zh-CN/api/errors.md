---
title: 错误
description: 模型请求失败时在各 API 格式下返回的内容，以及每个状态码的含义。
order: 2
---

模型请求失败时，会返回一个 HTTP 状态码和采用应用所调用格式的错误，因此 SDK 会抛出它惯常的错误。每个响应都带有 `kaeso-request-id` 响应头，[请求日志](../console/request-logs.md)中有同一 ID 的条目。Platform API 的错误见[其页面](platform-api.md#错误)。

下面的请求指定了一个不在项目目录中的模型，因此 Kaeso 返回 404：

```bash tab="curl"
curl -i https://kaeso.example.com/v1/responses \
  -H "Authorization: Bearer $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{"model": "gpt-9", "input": "Write a haiku about gateways."}'
```

```python tab="Python"
import os
from openai import APIStatusError, OpenAI

client = OpenAI(
    base_url="https://kaeso.example.com/v1",
    api_key=os.environ["KAESO_API_KEY"],
)

try:
    client.responses.create(
        model="gpt-9",
        input="Write a haiku about gateways.",
    )
except APIStatusError as error:
    print(error.status_code, error.response.headers["kaeso-request-id"])
    print(error.message)
```

```ts tab="TypeScript"
import OpenAI from "openai";

const client = new OpenAI({
  baseURL: "https://kaeso.example.com/v1",
  apiKey: process.env.KAESO_API_KEY,
});

try {
  await client.responses.create({
    model: "gpt-9",
    input: "Write a haiku about gateways.",
  });
} catch (error) {
  if (!(error instanceof OpenAI.APIError)) throw error;
  console.log(error.status, error.headers?.get("kaeso-request-id"));
  console.log(error.message);
}
```

## OpenAI 格式

Chat Completions、Responses 和 Embeddings 使用同一种错误正文：

```json
{
  "error": {
    "message": "The model `gpt-9` does not exist or this key may not use it.",
    "type": "invalid_request_error",
    "param": "model",
    "code": null
  }
}
```

| 状态码 | `type` | `code` |
| --- | --- | --- |
| 400 | `invalid_request_error` | 适用时为 `context_length_exceeded` 或 `content_policy_violation` |
| 401 | `invalid_request_error` | |
| 403 | `request_forbidden` | |
| 404 | `invalid_request_error` | |
| 405 | `invalid_request_error` | |
| 413 | `invalid_request_error` | |
| 429 | `rate_limit_error` | `rate_limit_exceeded` |
| 500、502 | `server_error` | |
| 503 | `service_unavailable_error` | `server_is_overloaded` |
| 504 | `server_error` | `timeout` |

该正文没有存放请求 ID 的字段，请从 `kaeso-request-id` 响应头读取。`Retry-After` 以整秒给出等待时间，向上取整。等待时间含不足一秒的部分时，`retry-after-ms` 以毫秒给出。

## Anthropic 格式

用 Anthropic SDK 发送同一个请求：

```bash tab="curl"
curl -i https://kaeso.example.com/v1/messages \
  -H "x-api-key: $KAESO_API_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "content-type: application/json" \
  -d '{
    "model": "gpt-9",
    "max_tokens": 1024,
    "messages": [
      {"role": "user", "content": "Write a haiku about gateways."}
    ]
  }'
```

```python tab="Python"
import os
from anthropic import Anthropic, APIStatusError

client = Anthropic(
    base_url="https://kaeso.example.com",
    api_key=os.environ["KAESO_API_KEY"],
)

try:
    client.messages.create(
        model="gpt-9",
        max_tokens=1024,
        messages=[
            {"role": "user", "content": "Write a haiku about gateways."}
        ],
    )
except APIStatusError as error:
    print(error.status_code, error.response.headers["kaeso-request-id"])
    print(error.message)
```

```ts tab="TypeScript"
import Anthropic from "@anthropic-ai/sdk";

const client = new Anthropic({
  baseURL: "https://kaeso.example.com",
  apiKey: process.env.KAESO_API_KEY,
});

try {
  await client.messages.create({
    model: "gpt-9",
    max_tokens: 1024,
    messages: [{ role: "user", content: "Write a haiku about gateways." }],
  });
} catch (error) {
  if (!(error instanceof Anthropic.APIError)) throw error;
  console.log(error.status, error.headers?.get("kaeso-request-id"));
  console.log(error.message);
}
```

错误正文在 `request_id` 中带有请求 ID，来自服务商的错误也不例外：

```json
{
  "type": "error",
  "error": {
    "type": "not_found_error",
    "message": "The model `gpt-9` does not exist or this key may not use it."
  },
  "request_id": "req_…"
}
```

| 状态码 | `error.type` |
| --- | --- |
| 400 | `invalid_request_error` |
| 401 | `authentication_error` |
| 403 | `permission_error` |
| 404 | `not_found_error` |
| 405 | `invalid_request_error` |
| 413 | `request_too_large` |
| 429 | `rate_limit_error` |
| 504 | `timeout_error` |
| 529 | `overloaded_error` |
| 其他 | `api_error` |

`Retry-After` 以整秒给出等待时间，向上取整。

## 原因

消息会说明原因以及该怎么做。各状态码背后的原因如下：

| 状态码 | 原因 |
| --- | --- |
| 400 | Kaeso 无法读取请求，或模型不接受请求使用的某项设置或字段。 |
| 400 | 请求中的消息、单条消息的内容块、工具或 embedding 输入超过 Kaeso 接受的数量（[请求](../concepts/request-flow.md#请求)）。 |
| 400 | 请求要求计数 Token，但模型的服务商没有计数 Token 的端点。 |
| 400 | 上下文超出模型的长度上限。 |
| 400 | 项目中有多个服务商提供该模型 ID。请改用 `provider/model`。 |
| 400 | 项目没有该模型服务商的服务商密钥。 |
| 401 | 缺少密钥，或密钥未知、已停用、已归档或已过期。 |
| 401 | 密钥所属的成员或服务账号已不再有效，或其项目或组织无法使用。 |
| 403 | 密钥仅具有平台访问权限，或者是平台密钥。两者都不能调用模型。 |
| 403 | 密钥所属的成员或服务账号缺少 `api.request` 权限。 |
| 403 | 请求适用支出预算，且使用了服务商在 token 之外另行计费的功能，例如服务商运行的工具或 `service_tier: priority`，或使用没有价格的模型（[限额和预算](../console/limits-and-budgets.md#支出预算无法计价的功能)）。 |
| 404 | 模型不在项目目录中，密钥或其成员无权使用该模型，或服务商不向项目的服务商密钥提供该模型。密钥无权使用的模型与不存在的模型返回相同的回答，因此密钥无法得知项目还有哪些模型。 |
| 404 | 路径不存在。 |
| 405 | 该路径不接受此方法。 |
| 413 | 正文大于 `KAESO_BODY_LIMITS_MODEL_API`，默认为 8 MiB。 |
| 429 | 达到了速率限制或预算，或者请求的费用可能超过其剩余额度，消息会指明是哪一个。预算用尽时，消息会说明何时重置。 |
| 429 | 来自您所在地址、使用未知密钥的请求过多。 |
| 500 | Kaeso 无法解密服务商密钥，或模型的服务商在此实例中不可用。 |
| 500 | 模型的服务商是自定义服务商，而实例已关闭自定义服务商。 |
| 502 | Kaeso 无法连接服务商，或传输失败。 |
| 502 | 自定义服务商位于实例不允许的地址，或其回复的形式无法按其格式读取。 |
| 502 | 服务商拒绝了项目的服务商密钥，或因计费或配额原因拒绝了它。 |
| 503 | Kaeso 无法加载目录、密钥、服务商密钥或限额，或正在重启。OpenAI 格式对这些情况返回 500。 |
| 503 | 服务商过载，适用于 OpenAI 格式。 |
| 504 | 服务商未及时响应。 |
| 529 | 服务商过载，适用于 Anthropic 格式。 |

流一旦开始，其状态码就无法再更改。服务商出错或提前结束流时，流会以采用应用所用格式的错误事件结束。

## 服务商错误

当应用和服务商使用相同格式时，Kaeso 会连同状态码原样转发服务商的错误，除非服务商拒绝了项目的服务商密钥。Anthropic 错误还会在 `request_id` 中带上 Kaeso 的 ID。在每个错误中，Kaeso 都会把看起来像密钥或账户、用户、项目、组织 ID 的内容替换为 `[redacted]`。流式错误和为其他格式改写的错误也是如此。

否则，Kaeso 会以应用所用的格式写出错误，并保持其原意：

| 服务商 | 应用收到 |
| --- | --- |
| 拒绝服务商密钥 | 502，所有格式均如此 |
| 因速率限制拒绝请求 | 429 |
| 因计费或配额原因拒绝 | 502，所有格式均如此 |
| 过载 | Anthropic 格式为 529，OpenAI 格式为 503 |
| 返回其他服务器错误 | 其自身的 5xx 状态码 |

被拒绝的服务商密钥是安装的问题，而不是应用的问题。服务商的 401、402 或 403 会让 SDK 报告 Kaeso 密钥错误。服务商的消息还可能包含服务商密钥的片段或服务商的组织 ID。

因此，应用收到的是一个服务商错误，消息为“The provider refused this project's provider key. Ask an organization admin to check it under Providers.”，或带有“for billing or quota reasons”的同一消息。组织管理员可在**服务商**下检查该密钥。

请求日志会保存服务商自己的消息，其中看起来像密钥、项目 ID 或组织 ID 的内容会替换为 `[redacted]`。Kaeso 每个请求只发送一次，从不重发，无论是发给这个模型还是其他模型。SDK 会在错误允许时自行重试。

### 自定义服务商

[自定义服务商](../console/custom-providers.md)的错误消息会以 ID 指明它，方便您区分自己服务器的问题和服务商的问题：

| 发生了什么 | 状态码 | 消息 |
| --- | --- | --- |
| Kaeso 无法连接它 | 502 | “Kaeso cannot reach the custom provider `…`; check its base URL.” |
| 它的地址不被允许 | 502 | “The custom provider `…` is at an address this install does not allow; ask its operator.” |
| 它拒绝了项目的密钥 | 502 | “The custom provider `…` refused this project's key; ask an organization admin to check it under Providers.” |
| 它需要密钥，但在 Kaeso 中设为不使用密钥 | 502 | “The custom provider `…` asks for a key; an organization admin sets how it signs in under Providers.” |
| 它的回复不符合其格式 | 502 | “The custom provider `…` answered in a form `…` cannot read; check its API format.” |

当服务商以其格式返回错误时，您的应用会像对待任何服务商一样收到该错误。请求日志保留服务商的原话。

### 服务商不提供的模型

当服务商对目录中的某个模型返回 404 时，Kaeso 会询问服务商是否仍向项目的服务商密钥提供该模型。如果不再提供，应用会收到 404，消息会指出服务商和模型。

目录随后会将该模型显示为**不可用**（[可用性](../console/models-and-aliases.md#可用性)），除非项目的服务商密钥在请求期间发生了变更。此后，Kaeso 会直接以同样的 404 答复对该模型的后续请求，而不调用服务商，每五分钟仅放行一个请求。第一次答复和之后的每次答复都带有同一条消息，请求日志中也是如此。

如果服务商仍列出该模型，应用会收到服务商原样发送的 404：它涉及的是其他内容，例如文件 ID。如果服务商无法给出答复，例如没有模型列表或列表请求失败，应用同样会收到服务商原样发送的 404。这两种情况下，目录都保持不变。
