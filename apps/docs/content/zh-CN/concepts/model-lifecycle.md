---
title: 模型生命周期
description: Kaeso 如何得知服务商停用某个模型、提醒仍在调用它的项目，并把它们切换到替代模型。
order: 5
---

服务商会在固定日期停用模型，停用日之后对该模型的请求会失败。Kaeso 从服务商发布公告到停用日全程跟踪每一次停用：[KPMR](../kpmr/index.md) 发布相关日期和服务商推荐的替代模型，网关找出仍在调用该模型的项目，提前提醒它们，并可一键切换，或在项目允许时自动切换。

## KPMR 发布的内容

目录中的每个模型都有 `lifecycle`：服务商宣布停用、弃用和停用该模型的日期，推荐的替代模型 `replacement`，以及公告来源 `source`，宣布之前均为 `null`（[目录格式](../kpmr/catalogue.md#生命周期)）。模型的 `status` 会在这些日期变为 `deprecated`，然后变为 `retired`，已停用的模型仍保留在目录中。KPMR 会在发布每个日期之前审核它（[停用和模型系列](../kpmr/lifecycle-and-lines.md)）。

## 即将停用的模型显示在哪里

- **模型页面：**有停用日期的模型会标记**停用日期**及当天，提示中会给出替代模型。项目中有模型将在 90 天内停用时，页面会显示提示。
- **即将停用选项卡：**项目中所有被服务商停用的模型，最近停用的排在前面。点击后会打开详情面板，显示日期、替代模型及价格差异，调用该模型的别名和路由步骤（包括回退和拆分分支），以及最近 30 天的调用方：密钥、成员、服务账户以及按 User-Agent 区分的客户端应用。每个调用方都链接到其在[请求日志](../console/request-logs.md)中的请求，图表显示自公告以来每天的请求数，便于判断迁移是否在推进。
- **组织概览：**提示列出使用即将停用模型的项目，以及每个项目最早的停用日期。

调用方和图表需要 `usage.view`，其余内容需要 `models.view`。

## 响应中带有日期

来自已公布日期的模型的响应会带有三个标准响应头，这样 SDK、代理和您自己的日志无需打开控制台即可看到提醒：

```http
Deprecation: @1793750400
Sunset: Thu, 04 Feb 2027 00:00:00 GMT
Link: <https://kpmr.kaeso.app/models/anthropic/claude-sonnet-5-5>; rel="successor-version", <https://example.com/deprecations>; rel="deprecation"; type="text/html"
```

`Deprecation`（[RFC 9745](https://www.rfc-editor.org/rfc/rfc9745)）是以 Unix 秒表示的弃用日期，`Sunset`（[RFC 8594](https://www.rfc-editor.org/rfc/rfc8594)）是停用日期，`Link` 指向替代模型的 KPMR 页面和服务商的公告。

## 停用之后

直接或通过别名步骤指定已停用模型的请求会以 `410` 失败，并返回注明停用日期和替代模型的 Kaeso 错误，而不是服务商的 `404`：

```text
The model `anthropic/claude-sonnet-4-5-20250929` was retired on 2026-11-30; its provider recommends `claude-sonnet-5-5`.
```

在别名中，该步骤按不可用处理，路由会[回退](alias-routes.md#回退)到下一个步骤。

## 切换到替代模型

即将停用模型详情面板中的**切换到替代模型**会先显示将要变更的每个路由步骤和价格差异，然后一步完成变更：如果项目中没有替代模型，会按 KPMR 的价格将其添加到目录，并把调用该模型的每个别名步骤指向它。模型本身仍保留在目录中，因此直接指定它的应用在停用前仍可使用。切换需要 `models.manage`，并在审计日志中记录为 `catalogue_model.move`。

**设置 → 模型停用**下有两项项目设置，会在停用日自动生效。两者默认关闭，需要 `models.manage`；更改会记录为 `project.model_lifecycle`：

| 设置 | 作用 |
| --- | --- |
| **停用时切换别名** | 在停用日，Kaeso 会像**切换到替代模型**一样，把调用该模型的别名和路由步骤切换到替代模型，并发送通知和 `model.moved` Webhook 事件 |
| **用替代模型应答已停用的模型** | 停用后，指定已停用模型的请求由替代模型应答，而不是失败。`kaeso-model` 会注明实际应答的模型，请求日志会把已停用的模型显示为跳过的步骤。由于价格和行为会变化，默认关闭 |

只有调用方有权使用替代模型时，替代模型才会应答。如果项目中没有替代模型，Kaeso 会在停用日将其添加到目录。

## 提醒

[您的邮件](../console/account.md#选择接收的邮件)**项目**分类下的两项通知会发送给能查看项目模型的成员，并且只针对该项目目录中的模型：

| 通知 | 时间 |
| --- | --- |
| 模型已弃用 | KPMR 为项目使用的模型发布停用日期或替代模型时 |
| 模型即将停用 | 停用前 30 天、7 天和 1 天，且项目在最近 7 天内仍调用过该模型时；以及 Kaeso 在停用日切换其别名时 |

每项通知也会作为 [Webhook 事件](../console/webhooks.md)发送：`model.deprecated`、`model.shutdown_soon` 和 `model.moved`，包含目录模型的 ID、日期、替代模型，`model.shutdown_soon` 还包含最近 7 天的请求数。网关每小时检查一次。

## Platform API 和 MCP

| 端点 | 作用 | 权限 |
| --- | --- | --- |
| `GET /orgs/{org}/projects/{project}/deprecations` | 即将停用的模型及其别名、路由步骤和调用方 | `models.view` |
| `GET /orgs/{org}/projects/{project}/deprecations/{model}/move` | 切换将会变更的内容 | `models.view` |
| `POST /orgs/{org}/projects/{project}/deprecations/{model}/move` | 切换到替代模型 | `models.manage` |
| `GET`、`PUT /orgs/{org}/projects/{project}/model-lifecycle` | 两项设置 | `models.view`、`models.manage` |
| `GET /orgs/{org}/deprecations` | 组织中使用即将停用模型的项目 | `org.view` |

Kaeso 的 [MCP 服务器](../api/mcp.md)以工具形式提供相同的操作。计数器 `kaeso_retired_model_requests_total{outcome}` 统计指定了已停用模型的步骤，按被拒绝或由替代模型应答区分（[监控](../self-hosting/monitoring.md)）。

## 后续步骤

- [停用和模型系列](../kpmr/lifecycle-and-lines.md)：了解这些日期的来源以及 KPMR 如何审核它们。
- [别名路由](alias-routes.md)：通过回退在多个模型之间路由请求。
- [Webhook](../console/webhooks.md)：在您自己的系统中接收提醒。
