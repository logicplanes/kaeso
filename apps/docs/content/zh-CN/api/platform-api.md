---
title: Platform API
description: 像控制台一样通过 HTTP 管理部署。
order: 3
---

Platform API 管理组织、项目、成员、密钥、服务商密钥、模型、限额、用量和日志。它位于 `/platform/v1` 下，与模型 API 使用同一域名。控制台只使用这个 API，因此控制台能做的一切，脚本也都能做。

完整的参考是 OpenAPI 文档，即仓库根目录中由网关代码生成的 [`openapi.json`](https://github.com/logicplanes/kaeso/blob/dev/openapi.json)；`kaeso openapi` 会输出当前运行版本的文档，例如 `docker compose exec gateway kaeso openapi`。其中每个操作都注明了它所检查的那一项权限。

## 约定

| 主题 | 规则 |
| --- | --- |
| 形式 | 基于 JSON 的 REST；资源名称使用复数 kebab case（`service-accounts`），字段使用 `snake_case`；请求正文以 `content-type: application/json` 发送，端点不认识的字段会被拒绝 |
| 时间 | RFC 3339，UTC |
| 金额 | 以十进制字符串表示的美元，例如 `"0.0001"` |
| ID | 前缀加按时间排序的部分：`org_…`、`proj_…`、`usr_…`、`key_…`、`opk_…`、`svc_…`、`cred_…`、`model_…`、`alias_…`、`lim_…`、`role_…`、`req_…`、`evt_…` 等 |
| 列表 | `limit`（1 到 100，默认 20）和 `after`，后者为上一页的 `next_cursor`；响应为 `{"data": [...], "has_more": true, "next_cursor": "…"}`，最后一页的 `next_cursor` 为 `null`；按 ID 排序，即按创建时间排序，请求日志和审计日志则从新到旧；筛选条件为查询参数 |
| 创建 | 对集合发送 `POST`，以 `201` 和该资源响应 |
| 更新 | 发送包含待更改字段的 `PATCH` |
| 操作 | `POST …/archive`、`…/disable`、`…/enable`、`…/replace`、`…/restore`；归档不等于删除 |
| 删除 | `DELETE`，仅用于确实会删除内容的地方，例如服务商密钥、别名、目录中的模型、限额、自定义角色、会话、邀请、账户 |
| 权限 | `orgs/{org}/*` 上的每个操作都会在其路径中的组织或项目上检查一项权限，该权限在操作的描述中注明；`auth/*`、`me/*` 和 `instance/*` 遵循各自的规则：公开、已登录用户、实例管理员 |
| 近期登录 | 使用会话时，敏感操作需要在过去 15 分钟内登录过：删除或恢复组织、密钥的平台访问权限、添加和更改服务商密钥、创建平台密钥、单点登录和双重验证设置、删除日志等；否则响应为 `403 recent_sign_in_required` |
| 已计划删除的组织 | 在恢复或删除之前为只读：读取操作按调用者的权限正常进行，组织中的每项更改，包括退出组织和接受加入该组织的邀请，都返回 `409 org_scheduled_for_deletion`，但 `POST orgs/{org}/restore` 和实例管理员的 `instance/orgs/{org}/*` 除外；其密钥完全无法通过身份验证 |
| 审计 | 每项更改都会在同一事务中写入其所在层级的审计日志 |
| 请求 ID | 每个响应都带有 `kaeso-request-id` |

`auth/session` 会在每个成员资格中显示已计划删除的组织：其 `org_state`、`org_deletion_due_at`，以及缩减为仅能查看、导出和 `org.delete` 的权限。它还包含实例的 `org_deletion` 策略（`allowed`、`grace_days`）。

## 身份验证

| 方式 | 方法 | 用途 |
| --- | --- | --- |
| 会话 | HttpOnly Cookie `__Host-kaeso_session`（在位于 localhost 等回环地址的 `http://` 部署上为 `kaeso_session`），由 `auth/*` 下的登录端点设置；更改操作还必须发送 `Origin`，其值为部署公开 URL 的源 | 控制台，以及使用 curl 的人员 |
| 具有平台访问权限的 API 密钥 | `Authorization: Bearer sk-kso-…`；以其成员或服务账号的权限行事，且仅限于其所属项目 | 单个项目的脚本和 CI |
| 平台密钥 | `Authorization: Bearer sk-kso-org-…`；在其组织及组织内的所有项目中以管理员角色的权限行事，且从不调用模型 | 整个组织的自动化 |

项目密钥默认具有模型调用权限。平台访问权限在创建密钥时通过 `"access": "platform"` 或 `"both"` 选择，并需要 `keys.platform_access.grant` 权限。平台密钥只能通过会话创建。请参阅 [API 密钥](../console/api-keys.md)。

密钥用于 `orgs/{org}/*`：`me/*`、`instance/*` 以及 `auth/*` 中需要登录的路由会以 `403` 拒绝密钥，组织路由也会以 `403` 拒绝项目密钥。要求单点登录的组织会以 `403` 拒绝其他会话，要求双重验证的组织会拒绝没有第二重验证的密码会话。

来自同一地址的使用未知 API 密钥的请求过多时（模型 API 和 Platform API 合并计算），会以 `429 rate_limited` 和 `Retry-After` 被拒绝，直到时间窗口结束。相关设置见 [API 密钥验证失败](../self-hosting/configuration.md#api-密钥查找失败)。

## 错误

```json
{
  "error": {
    "type": "permission_denied",
    "message": "You need the right `models.manage` in this project; ask one of its admins for it.",
    "right": "models.manage"
  }
}
```

| 状态码 | `type` |
| --- | --- |
| 400 | `invalid_request` |
| 401 | `authentication` |
| 403 | `permission_denied` 或 `recent_sign_in_required` |
| 404 | `not_found` |
| 405 | `invalid_request`，带有 `Allow` |
| 409 | `conflict`；对等待删除的组织进行更改时为 `org_scheduled_for_deletion` |
| 413 | `request_too_large`，正文大于 `KAESO_BODY_LIMITS_PLATFORM_API`（默认 1 MiB）时 |
| 429 | `rate_limited`，带有 `Retry-After` |
| 500 | `internal`；消息中注明请求 ID |
| 502, 503, 504, 429 | `provider_error`，仅用于试验场的模型请求 |
| 503 | `unavailable` |

`param` 注明出错的字段、查询参数或路径参数；当某一项权限即可允许该请求时，`right` 注明调用者缺少的权限；不适用时两者都会省略。类型或形式错误的参数，或不是 JSON 的正文，返回 `400 invalid_request`，未知路径返回 `404 not_found`。消息是一句写给人看的简短句子；控制台会原样显示。

## 使用 curl 的完整示例

以下请求先使用密码登录，然后使用会话 Cookie。需要 `jq`。

```bash
KAESO=https://kaeso.example.com
JAR=$(mktemp)
```

登录。部署的第一位用户则通过设置链接注册。

```bash
curl -s -c "$JAR" -H "Origin: $KAESO" -H 'content-type: application/json' \
  -d '{"email": "owner@example.com", "password": "correct horse battery"}' \
  "$KAESO/platform/v1/auth/password"
```

开启双重验证时，响应为带有 `challenge` 的 `202`。使用 `POST /platform/v1/auth/totp` 和 `{"challenge": "…", "code": "123456"}` 完成登录，也可以用 `recovery_code` 代替验证码。

从会话中获取组织，并创建项目：

```bash
ORG=$(curl -s -b "$JAR" "$KAESO/platform/v1/auth/session" | jq -r '.memberships[0].org_id')

PROJECT=$(curl -s -b "$JAR" -H "Origin: $KAESO" -H 'content-type: application/json' \
  -d '{"name": "Support bot"}' \
  "$KAESO/platform/v1/orgs/$ORG/projects" | jq -r .id)
```

添加组织的 Anthropic 密钥。密钥值不会再次显示；只有最后四个字符保持可见。服务商的第一个密钥即为其默认密钥，供所有未分配密钥的项目使用。

```bash
curl -s -b "$JAR" -H "Origin: $KAESO" -H 'content-type: application/json' \
  -d '{"provider": "anthropic", "name": "Production", "secret": "sk-ant-…"}' \
  "$KAESO/platform/v1/orgs/$ORG/credentials"
```

向项目目录添加模型。其参数来自 models.dev。

```bash
curl -s -b "$JAR" -H "Origin: $KAESO" -H 'content-type: application/json' \
  -d '{"provider": "anthropic", "model_id": "claude-haiku-4-5"}' \
  "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/models"
```

为应用创建 API 密钥。响应中的 `secret` 就是密钥，仅显示一次。

```bash
KEY=$(curl -s -b "$JAR" -H "Origin: $KAESO" -H 'content-type: application/json' \
  -d '{"name": "support-bot", "expires_in_days": 90}' \
  "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/keys" | jq -r .secret)
```

使用官方 SDK 或 curl 调用模型 API：

```bash
curl -s "$KAESO/v1/messages" -H "x-api-key: $KEY" -H 'anthropic-version: 2023-06-01' \
  -H 'content-type: application/json' \
  -d '{"model": "claude-haiku-4-5", "max_tokens": 100, "messages": [{"role": "user", "content": "Hello"}]}'
```

读取用量和费用，可按天（默认）、`week`、`month`、`model`、`key` 或 `member` 分组。`usage/span` 给出有用量的第一天和最后一天，`{"first_day": "2026-08-15", "last_day": "…"}`，便于按周或按月读取较长的时间段。按天分组时，`from` 和 `to` 最多相隔 400 天。

```bash
curl -s -b "$JAR" "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/usage?group_by=model"
curl -s -b "$JAR" "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/usage/span"
curl -s -b "$JAR" "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/usage?from=2026-08-15&group_by=week"
```

从新到旧读取请求日志，并读取一个条目及其内容。读取内容需要 `logs.content.view` 权限。

```bash
curl -s -b "$JAR" "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/logs?limit=10"
curl -s -b "$JAR" "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/logs/req_…"
```

项目的调用也可以使用具有平台访问权限的密钥：将 Cookie 和 `Origin` 替换为 `-H "Authorization: Bearer $KAESO_PLATFORM_KEY"`。每个调用都需要相应的权限：`models.manage`、`keys.own.create`、`usage.view` 和 `logs.metadata.view`。添加服务商密钥和创建项目属于组织权限，项目密钥从不具有这些权限；请为此使用平台密钥或会话。

## 功能领域

| 路径 | 内容 |
| --- | --- |
| `auth/*` | 首次启动设置、登录方式、密码和双重验证、登录链接、密码重置、单点登录、会话、在实例允许该地址时注册、邀请 |
| `me`、`me/*` | 个人资料、头像和语言（`locale`：`auto`、`en`、`de`、`zh-CN`）、邮箱、密码、双重验证和恢复码、会话、成员资格、自己的密钥、通知、创建组织、删除账户 |
| `orgs/{org}/*` | 组织及其头像、成员、邀请、角色和权限、项目及默认项目、服务商密钥（每个服务商可有多个、一个默认密钥、分配给项目）、安全（单点登录、强制单点登录和强制双重验证、密钥有效期）、平台密钥、日志设置、存储和删除、用量、审计日志、所有权转让、删除和恢复 |
| `orgs/{org}/projects/{project}/*` | 项目及其图片、成员及其模型访问设置、服务账号、密钥及其模型和限额、密钥策略、目录（搜索、预览、批量）和别名、限额、内容日志、请求日志和导出、用量、审计日志、[试验场](#试验场) |
| `instance/*` | 用户及其邀请、会话和实例管理员标记，组织及其成员、项目、用量和存储、设置（含测试邮件）、models.dev 状态和同步、系统健康状况、存储和日志缓冲区、实例审计日志；供实例管理员使用 |

实例管理员可以看到每个组织的元数据，但从不包括其内容。他们在 `instance/orgs` 下创建、重命名、计划删除、恢复和删除组织，并通过 `POST instance/orgs/{org}/owner` 将任何有效用户（包括自己）设为所有者；这是他们进入组织进行管理的方式，此操作会记录在该组织的审计日志中。只有他们可以通过 `POST instance/users/{user}/reset-2fa` 重置用户的双重验证。

没有计费路由。`billing.view` 和 `billing.manage` 权限用于 Kaeso Cloud，在 `orgs/{org}/rights` 中带有 `cloud_only` 标记。

## 角色和权限

每位组织成员有一个组织角色，每位项目成员和每个服务账号有一个项目角色，每个角色都可以单独添加或移除权限。角色可以是内置角色，也可以是组织的自定义角色（`role_…`）。组织角色还可以包含在所有项目中的权限。[权限](../concepts/rights.md)列出了所有权限和内置角色。

| 路由 | 权限 | 内容 |
| --- | --- | --- |
| `GET orgs/{org}/rights` | `org.view` | 按类别列出的所有权限，包括其名称、描述、标记（`sensitive`、`recent_sign_in`、`cloud_only`）及其所需的权限 |
| `GET orgs/{org}/roles`、`GET …/roles/{role}` | `org.view` | 内置角色和自定义角色及其权限、在所有项目中的权限以及持有者数量；`?scope=org` 或 `project` |
| `POST orgs/{org}/roles`、`PATCH …/roles/{role}`、`DELETE …/roles/{role}` | `roles.manage` | 创建、更改和删除自定义角色，最多 50 个；更改会立即作用于所有持有者，已被持有的角色无法删除 |
| `GET orgs/{org}/members/{user}/rights` | `members.view` | 成员的有效组织权限、在所有项目中的权限，以及在调用者可见的每个所属项目中的权限，每项都附有来源 |
| `GET orgs/{org}/projects/{project}/members/{user}/rights` | `project.members.view` | 成员在此项目中的有效权限及其来源 |
| `GET orgs/{org}/projects/{project}/my-rights` | `project.view` | 调用者自己在此项目中的权限及其来源 |
| `GET orgs/{org}/rights/{right}/grantors`、`GET …/projects/{project}/rights/{right}/grantors` | `members.view`、`project.view` | 最多 10 位持有该权限且可以更改权限的有效成员，所有者排在前面：即可以向谁申请 |
| `GET orgs/{org}/projects/{project}/audit-log` | `project.audit_log.view` | 在项目中所做的更改 |

授权以 `{"role": "developer", "rights_added": [], "rights_removed": []}` 的形式发送，其中 `role` 是内置角色的名称或自定义角色的 ID；`preset` 是其已弃用的名称。返回时带有 `role`（`{id, kind, name}`）、有效的 `rights`，组织授权还带有 `project_rights`。邀请接受 `role` 和 `project_roles: [{project_id, role}]`；单点登录设置接受 `default_role`。

每次权限更改都遵循授权规则：

- 只授予调用者拥有的权限：组织中的组织权限、该项目中的项目权限，只有在调用者在所有项目中都拥有某些权限时，才能授予在所有项目中的这些权限。
- 只更改或移除其全部权限调用者都拥有的成员。
- 只有所有者可以设置、更改、移除或邀请所有者。
- 授予的每项权限都必须连同其所需的权限一起授予，否则响应为 `400`，例如“`logs.delete` needs `logs.storage.view`.”

有效权限的来源包括 `owner`、`role`（组织角色，针对组织权限）、`project_role`、`added`、`org_role`（组织角色在所有项目中的权限）、`org_right`（延伸到项目中的组织权限，例如 `projects.view.all`）和 `key`（在项目中行事的 API 密钥）。

## 试验场

控制台的[试验场](../console/playground.md)以已登录成员的身份向项目的模型发送请求。两个路由都需要项目权限 `playground.use` 和会话；API 密钥和平台密钥会以 `403` 被拒绝，因为应用在 `/v1` 上调用模型。

| 路由 | 内容 |
| --- | --- |
| `POST orgs/{org}/projects/{project}/playground/responses` | 带有 `"stream": true` 的 OpenAI Responses 请求（`model`、字符串或输入项形式的 `input`、`instructions`、`temperature`、`top_p`、`max_output_tokens`、`reasoning.effort`、`reasoning.summary`）；响应是与 `/v1/responses` 发送的相同的 Responses 事件流，带有保活注释 |
| `GET orgs/{org}/projects/{project}/playground/models` | 成员在此可调用的模型：不含嵌入模型的目录，并按成员的模型访问设置收窄，包括要发送的名称、别名、是否进行推理、其推理强度、是否接受 `temperature`、限制、价格，以及项目是否有对应的服务商密钥 |

试验场请求与其他模型请求一样，只是由成员在不使用密钥的情况下发出：

- **相同的处理路径。** 模型在项目目录中解析，包括别名和 `provider/model`，并根据成员的模型访问设置进行检查。使用项目的服务商密钥，关闭流会取消对服务商的请求。
- **限额。** 成员和项目的速率与预算适用；由于没有密钥，密钥限额不适用。
- **用量和日志。** 请求计入项目费用，算作该成员的用量（按 `key` 分组的用量将其显示为 `playground`），并以来源 `playground` 记录到日志中，项目记录内容时还会包含其内容。`logs` 和 `logs/export` 可按 `source=api` 或 `source=playground` 筛选。
- **错误。** 流开始之前，失败以 Platform API 错误返回：`400 invalid_request`（目录之外的模型时 `param` 为 `model`，`"stream": false` 时为 `stream`）、超出成员模型访问范围的模型返回 `403 permission_denied`、已归档的项目或没有密钥的服务商返回 `409 conflict`、`409 org_scheduled_for_deletion`、注明相应限额的 `429 rate_limited`、`503 unavailable`，以及服务商失败或拒绝服务商密钥（`502`）、对其进行速率限制（`429`）、过载（`503`）或超时（`504`）时的 `provider_error`。流开始之后，错误以 Responses 的 `error` 事件返回，与 `/v1` 上相同。

```bash
curl -N -b "$JAR" -H "Origin: $KAESO" -H 'content-type: application/json' \
  -d '{"model": "claude-haiku-4-5", "input": "Hello", "stream": true, "reasoning": {"summary": "auto"}}' \
  "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/playground/responses"
```

## 控制台如何使用它

控制台与网关同源提供服务，因此会话 Cookie 是第一方 Cookie，不涉及 CORS。控制台：

- 通过 `auth/*` 让人员登录，只保存 HttpOnly Cookie；
- 每次更改都发送 `Origin`，与浏览器自行发送的方式相同；
- 读取 `auth/session` 以获取已登录用户、其成员资格及其中的组织角色、权限和在所有项目中的权限，以及其是否为实例管理员、是否可以创建组织（`can_create_orgs`）；
- 读取项目的 `my-rights` 以决定在其中显示的内容，并从 `orgs/{org}/rights` 获取权限名称、分组和依赖关系；
- 使用上述游标对列表分页，并根据 `models/search` 和 `models/preview` 填写模型表单；
- 将 `recent_sign_in_required` 显示为重新登录的提示，然后重复该操作；
- 以流式文件下载导出内容（`logs/export`、`audit-log/export`）；
- 使用 `fetch` 流式接收试验场的回复；**停止**会中止该 `fetch`，从而取消对服务商的请求；
- 绘制用量图表时，约三个月以内按天，约一年以内按周，更长则按月，**全部时间**从 `usage/span` 的 `first_day` 开始。
