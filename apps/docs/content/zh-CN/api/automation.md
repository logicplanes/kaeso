---
title: 用 Platform API 实现自动化
nav: 自动化
description: 通过脚本为应用设置项目、模型和密钥，并读取其用量和日志。
order: 4
---

通过脚本设置应用所需的一切：项目、服务商密钥、模型，以及应用自己的 API 密钥。这些请求使用平台密钥，它在整个组织中以管理员角色的权限行事。

## 准备工作

- 一个平台密钥，在组织设置的**平台密钥**中创建，放在环境变量 `KAESO_PLATFORM_KEY` 中（[平台密钥](../console/api-keys.md#平台密钥)）。
- 组织的 ID `org_…`，可从控制台的地址中找到。
- 放在 `ANTHROPIC_API_KEY` 中的 Anthropic API 密钥；组织已有时可省略。
- `curl` 和 `jq`。

把组织的地址存入一个变量，用您组织的 ID 替换 `org_…`：

```bash
ORG_URL=https://kaeso.example.com/platform/v1/orgs/org_…
```

## 创建项目

创建项目并保存其 ID：

```bash
PROJECT=$(curl -s "$ORG_URL/projects" \
  -H "Authorization: Bearer $KAESO_PLATFORM_KEY" \
  -H "content-type: application/json" \
  -d '{"name": "Support bot"}' | jq -r .id)
```

`PROJECT` 现在保存着新项目的 ID `proj_…`。

## 添加服务商密钥

添加组织的 Anthropic 密钥。组织已有时，跳过此步。

```bash
curl -s "$ORG_URL/credentials" \
  -H "Authorization: Bearer $KAESO_PLATFORM_KEY" \
  -H "content-type: application/json" \
  -d "$(jq -n --arg secret "$ANTHROPIC_API_KEY" \
    '{provider: "anthropic", name: "Production", secret: $secret}')"
```

某个服务商的第一个密钥会成为它的默认密钥；除非为项目另行分配，每个项目都使用它。响应只显示密钥的最后四个字符，Kaeso 不会再次显示密钥本身。

## 添加模型

把 Claude Haiku 添加到项目的目录：

```bash
curl -s "$ORG_URL/projects/$PROJECT/models" \
  -H "Authorization: Bearer $KAESO_PLATFORM_KEY" \
  -H "content-type: application/json" \
  -d '{"provider": "anthropic", "model_id": "claude-haiku-4-5"}'
```

Kaeso 从 models.dev 获取模型的价格、限制和能力，并在响应中显示。

## 创建应用的密钥

为应用创建一个具有开发者角色的服务账号，使它的密钥不依赖于某个人。然后为它创建一个 90 天后过期的密钥：

```bash
ACCOUNT=$(curl -s "$ORG_URL/projects/$PROJECT/service-accounts" \
  -H "Authorization: Bearer $KAESO_PLATFORM_KEY" \
  -H "content-type: application/json" \
  -d '{"name": "support-bot", "role": "developer"}' | jq -r .id)

KAESO_API_KEY=$(
  curl -s "$ORG_URL/projects/$PROJECT/service-accounts/$ACCOUNT/keys" \
    -H "Authorization: Bearer $KAESO_PLATFORM_KEY" \
    -H "content-type: application/json" \
    -d '{"name": "support-bot", "expires_in_days": 90}' | jq -r .secret
)
```

`KAESO_API_KEY` 现在保存着应用的密钥 `sk-kso-…`。Kaeso 只显示一次密钥本身，请把它存放到应用读取的位置。

## 调用模型

像应用那样，用新密钥发送一个请求：

```bash
curl https://kaeso.example.com/v1/messages \
  -H "x-api-key: $KAESO_API_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "content-type: application/json" \
  -d '{
    "model": "claude-haiku-4-5",
    "max_tokens": 1024,
    "messages": [
      {"role": "user", "content": "Write a haiku about gateways."}
    ]
  }'
```

响应是来自 Claude 的 Messages 回复。应用如何在代码中设置 SDK，见[模型 API](model-api.md)。

## 读取用量和日志

按模型读取项目的用量，以及它最新的十个请求：

```bash
curl -s "$ORG_URL/projects/$PROJECT/usage?group_by=model" \
  -H "Authorization: Bearer $KAESO_PLATFORM_KEY"
curl -s "$ORG_URL/projects/$PROJECT/logs?limit=10" \
  -H "Authorization: Bearer $KAESO_PLATFORM_KEY"
```

请求会在几秒内出现在日志中。调用方拥有 `costs.view` 权限时，用量中包含费用。用量接受以下查询参数：

- `group_by`：`day`（默认）、`week`、`month`、`model`、`key` 或 `member`。
- `from` 和 `to`：UTC 日期，例如 `2026-08-15`。按天分组时，两者最多相隔 400 天。
- `usage/span` 返回有用量的第一天和最后一天，便于按周或按月读取较长的时间段。

在地址后加上日志条目的 ID 即可读取单个请求，例如 `logs/req_…`。项目记录内容时，条目中包含请求的内容。

## 轮换应用的密钥

在应用的密钥过期前替换它，或在密钥泄露时立即替换。轮换会创建一个名称、访问权限、模型和限额都相同的新密钥，旧密钥在 `grace_hours`（0、1、24 或 168）内继续可用。先查出密钥的 ID，再以 24 小时的宽限期轮换它：

```bash
KEY=$(curl -s "$ORG_URL/projects/$PROJECT/keys?service_account=$ACCOUNT" \
  -H "Authorization: Bearer $KAESO_PLATFORM_KEY" | jq -r '.data[0].id')

KAESO_API_KEY=$(
  curl -s "$ORG_URL/projects/$PROJECT/service-accounts/$ACCOUNT/keys/$KEY/rotate" \
    -H "Authorization: Bearer $KAESO_PLATFORM_KEY" \
    -H "content-type: application/json" \
    -d '{"grace_hours": 24, "expires_in_days": 90}' | jq -r .secret
)
```

`KAESO_API_KEY` 现在保存的是新密钥。请在 24 小时内把它存到应用读取的位置，之后旧密钥会过期。新密钥的 `rotated_from` 和旧密钥的 `replaced_by` 将两者关联起来，请求日志和用量则保留各个密钥自己的 ID。

## 改用项目密钥

具有平台访问权限的 API 密钥在其所属项目内同样适用，并以其成员或服务账号的权限行事。以 `Authorization: Bearer sk-kso-…` 发送。它可以凭 `models.manage` 添加模型，凭 `usage.view` 和 `logs.metadata.view` 读取用量和日志。

创建项目和添加服务商密钥属于组织权限，项目密钥永远不会拥有。属于成员的密钥可以用 `POST orgs/{org}/projects/{project}/keys` 为该成员创建更多密钥。如何为密钥授予平台访问权限，见 [API 密钥](../console/api-keys.md)。

## 改用会话

用密码登录，把会话 Cookie 保存在文件中，并在每个更改内容的请求中发送 `Origin`：

```bash
JAR=$(mktemp)

curl -s -c "$JAR" https://kaeso.example.com/platform/v1/auth/password \
  -H "Origin: https://kaeso.example.com" \
  -H "content-type: application/json" \
  -d '{
    "email": "owner@example.com",
    "password": "correct horse battery staple"
  }'
```

之后，上面的请求改用 `-b "$JAR" -H "Origin: https://kaeso.example.com"` 代替 `Authorization` 请求头即可。会话以您自己的权限行事，并可用 `POST orgs/{org}/projects/{project}/keys` 为您自己的用户创建密钥。

开启双重验证时，`auth/password` 返回 202 和一个 `challenge`。以 `{"challenge": "…", "code": "123456"}` 的形式发送到 `POST auth/totp`，也可以用 `recovery_code` 代替 `code`。只有在实例开启密码登录时（默认开启），才能用密码登录。在新实例中，第一个用户通过 `kaeso setup-link` 输出的设置链接注册（[安装](../self-hosting/installation.md)）。

## 后续步骤

- [Platform API](platform-api.md)：查阅每个路由、约定、错误和权限。
- [API 密钥](../console/api-keys.md)：把密钥限制在应用所需的模型和速率内。
- [限额和预算](../console/limits-and-budgets.md)：限制项目的请求数、Token 数和支出。
