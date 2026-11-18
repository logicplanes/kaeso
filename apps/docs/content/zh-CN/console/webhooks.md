---
title: Webhook
description: 把组织的通知签名后发送到值班工具、聊天和脚本。
order: 9
---

Webhook 会在组织的运营通知（例如预算提醒或被拒绝的服务商密钥）发生时，把它们发送到您指定的地址。值班工具可以据此创建事件，聊天频道可以显示它，脚本可以据此执行操作。无论各人如何设置自己的邮件，Webhook 都会收到通知；在不发送邮件的实例上也是如此。

**Webhook** 是**组织设置**中的一个页面。查看此页面需要 `webhooks.view`，更改需要 `webhooks.manage`（见[权限](../concepts/rights.md)）。一个组织最多有 10 个 Webhook。

## 添加 Webhook

1. 打开**组织设置** › **Webhook**，选择**添加 Webhook**。
2. 为它命名，并输入接收事件的地址，例如 `https://hooks.example.com/kaeso`。
3. 选择格式。**JSON** 发送下文所述的 Kaeso 事件，适合脚本和值班工具。**Slack** 发送一行文本 `{"text": "…"}`，Slack 的传入 Webhook 以及读取同样格式的聊天工具都能接收。
4. 选择它接收的通知，以及它接收整个组织还是某一个项目的通知。
5. 选择**添加 Webhook**，然后复制签名密钥。密钥只显示一次；您的接收方用它校验每次投递。

地址必须是公网 `https` 地址。实例可以通过 `KAESO_WEBHOOKS_ALLOWED_NETWORKS` 允许其他网络，使用 `http` 或 `https`（见[配置](../self-hosting/configuration.md#webhook)）。Kaeso 在 DNS 解析后检查每个连接，不经代理发送，也不跟随重定向。

Webhook 的菜单中有**发送测试**、**关闭**或**开启**、**轮换密钥**和**删除**。**发送测试**会立即发送一个 `webhook.test` 事件，只尝试一次，并告诉您是否送达。**轮换密钥**会立即替换签名密钥，因此接收方需要在下一次投递前换上新密钥。打开一个 Webhook 可以看到它接收的内容以及最近 7 天的投递（最新的 50 条），每条都有尝试次数和接收方最后的应答。

## Webhook 可以接收什么

| 通知 | 事件类型 |
| --- | --- |
| 预算提醒 | `budget.alert` |
| 已达限额 | `limit.reached` |
| 您的密钥即将过期 | `key.expiring` |
| 内容记录已变更 | `project.content_logging_changed` |
| MCP 服务器需要处理 | `mcp_server.signed_out`、`mcp_server.tool_held` |
| 安全设置已变更 | `organization.security_changed` |
| 成员变动 | `organization.member_joined`、`organization.member_left` |
| 服务商密钥需要处理 | `provider_key.refused`、`provider_key.signed_out`、`provider_key.spend_limit_reached`、`provider_key.unlinked` |
| 模型变更 | `model.delisted`、`model.price_changed` |

这些是关于组织及其项目的通知（见[邮件](account.md#选择接收的邮件)）。个人自己的通知，例如其账户安全或其发起的导出，从不发送到 Webhook。绑定了项目的 Webhook 只接收该项目的事件；未绑定项目的 Webhook 接收组织及其所有项目的每个事件。即将过期的密钥会针对每个密钥发送，而不仅告知其所有者。

## 事件

JSON 投递是一个 `POST` 请求，带有 `content-type: application/json`，正文如下：

```json
{
  "id": "msg_01kb2c9w6t3m5q7r8s0v1x2y4z",
  "type": "budget.alert",
  "created_at": "2026-11-16T09:30:00Z",
  "organization": { "id": "org_01kb2c8a4b5c6d7e8f9g0h1j2k", "name": "Acme" },
  "project": { "id": "proj_01kb2c8m3n4p5q6r7s8t9v0w1x", "name": "Checkout" },
  "summary": "Checkout used 80% of its monthly spend budget",
  "data": {
    "limit_id": "lim_01kb2c8y2z3a4b5c6d7e8f9g0h",
    "kind": "spend_budget",
    "period": "month",
    "amount": "100",
    "used": "80.42",
    "percent": 80,
    "applies_to": { "type": "project" },
    "resets_at": "2026-12-01T00:00:00Z"
  },
  "url": "https://kaeso.example.com/orgs/org_01kb2c8a4b5c6d7e8f9g0h1j2k/projects/proj_01kb2c8m3n4p5q6r7s8t9v0w1x/limits"
}
```

- `id` 是事件的 ID，每次尝试都相同。
- 组织级事件的 `project` 为 `null`。
- `summary` 是通知的英文摘要。
- `url` 打开事件相关的控制台页面。
- ID 的格式与 [Platform API](../api/platform-api.md) 相同。金额为字符串：支出以美元计，其他以 token 或请求数计。

`data` 包含：

| 类型 | `data` |
| --- | --- |
| `budget.alert` | `limit_id`、`kind`（`token_budget` 或 `spend_budget`）、`period`、`amount`、`used`、`percent`（越过的阈值）、`applies_to`（`{"type": "project"}`，或带 `name` 的 `member` 或 `key`）、`resets_at`（总额限额为 `null`） |
| `limit.reached` | `limit_id`、`kind`、`period`、`amount`、`applies_to`、`resets_at` |
| `key.expiring` | `key_id`、`key`（密钥名称）、`owner`（用户 ID，服务账户的密钥为 `null`）、`expires_at`、`days` |
| `project.content_logging_changed` | `on`、`by`（更改者） |
| `mcp_server.signed_out` | `server_id`、`server`（服务器名称） |
| `mcp_server.tool_held` | `server_id`、`server`、`tool`：已变更并等待批准的工具 |
| `organization.security_changed` | `change`（`sign_in_rules`、`single_sign_on`、`agent_policy`、`key_policy`、`platform_key_created` 或 `platform_key_rotated`）、`platform_key`（后两种情况下的密钥名称）、`by` |
| `organization.member_joined` | `person`、`sso`（是否通过单点登录加入） |
| `organization.member_left` | `person`、`removed_by`（本人离开时为 `null`） |
| `provider_key.refused` | `provider_key_id`、`provider`、`name`、`default`、`projects`、`refusal`（含服务商的 `status` 和 `message`、`billing`（是否为计费问题）以及 `first_at` 和 `last_at`） |
| `provider_key.signed_out` | `provider_key_id`、`provider`、`name`、`default`、`projects`、`signed_out_at` |
| `provider_key.spend_limit_reached` | `provider_key_id`、`provider`、`name`、`default`、`projects`、`refused_at`、`resets_at`（服务商最迟何时重新放行请求） |
| `provider_key.unlinked` | `provider_key_id`、`provider`、`name`、`default`、`projects`、`unlinked_at` |
| `model.delisted` | `catalogue_model_id`、`provider_id`、`provider`、`model` |
| `model.price_changed` | `catalogue_model_id`、`provider_id`、`provider`、`model`、`prices`：每项变更的价格（`input`、`output`、`cache_read`、`cache_write`、`cache_write_1h`）及其每百万 token 的新旧美元价格 `from` 和 `to`，无价格时为 `null` |
| `webhook.test` | `webhook_id`、`name` |

`person`、`by` 和 `removed_by` 是人名；变更不是由某个人做出时（例如平台密钥），`by` 和 `removed_by` 为 `null`。模型变更会针对目录中有该模型的每个项目各发送一次。以后的版本可能增加新的类型和 `data` 中的新字段；请忽略您不认识的内容。

## 校验投递

Kaeso 按照 [Standard Webhooks](https://www.standardwebhooks.com/) 的规定为每次投递签名，因此可以使用其库进行校验。每个请求都有三个请求头：

| 请求头 | 内容 |
| --- | --- |
| `webhook-id` | 事件的 `id`。接收方两次看到同一个 ID，说明收到了同一事件 |
| `webhook-timestamp` | 本次尝试的发送时间，以 1970 年以来的秒数表示 |
| `webhook-signature` | `v1,` 加上 `{webhook-id}.{webhook-timestamp}.{body}` 的 Base64 HMAC-SHA256，密钥为 `whsec_` 之后部分经 Base64 解码得到的字节 |

请在解析正文之前，对收到的原始正文校验签名，并拒绝与您的时钟相差超过五分钟的时间戳：

```js title="verify.mjs"
import { createHmac, timingSafeEqual } from "node:crypto";

export function verify(secret, headers, body) {
  const id = headers["webhook-id"];
  const timestamp = headers["webhook-timestamp"];
  if (Math.abs(Date.now() / 1000 - Number(timestamp)) > 300) return false;
  const key = Buffer.from(secret.replace(/^whsec_/, ""), "base64");
  const expected = createHmac("sha256", key).update(`${id}.${timestamp}.${body}`).digest();
  return headers["webhook-signature"].split(" ").some((s) => {
    const [version, signature] = s.split(",");
    const given = Buffer.from(signature ?? "", "base64");
    return version === "v1" && given.length === expected.length && timingSafeEqual(given, expected);
  });
}
```

## 投递与重试

- **时间。** 事件会在通知产生后几秒内发出。
- **已投递。** 10 秒内返回 `2xx` 即表示事件已投递。其他任何应答（包括重定向）或无应答都算一次失败的尝试。
- **重试。** 失败的事件会在 1、5、15 和 60 分钟后重试，共五次尝试，每次都重新签名。第五次之后即彻底失败。
- **至少一次。** 事件可能重复送达，顺序也可能不同：请用 `webhook-id` 区分，用 `created_at` 排序。
- **自动关闭。** 连续 20 次尝试失败的 Webhook 会被关闭：其待发送事件被丢弃，审计日志记下此事，拥有 `webhooks.manage` 的人会收到邮件。**开启**后它会重新收到新事件，计数也从头开始。
- **保留。** 投递记录保留 7 天。

网关在 `kaeso_webhook_deliveries_total{outcome}` 中统计每次尝试（见[监控](../self-hosting/monitoring.md#指标)）。Webhook 属于 [Platform API](../api/platform-api.md) 中的 `orgs/{org}/webhooks`，添加、更改、轮换和删除都会记入[审计日志](organizations-and-projects.md#查看审计日志)。

## 下一步

- [限额和预算](limits-and-budgets.md)：设置 Webhook 可以传递其提醒的预算。
- [账户和语言](account.md)：选择哪些通知通过邮件发送给您。
