---
title: Platform API
description: 像控制台一样通过 HTTP 管理部署。
order: 3
---

Platform API 管理组织、项目、成员、密钥、服务商密钥、模型、限额、用量和日志。它位于 `/platform/v1` 下，与模型 API 使用同一域名。控制台只使用这个 API，因此控制台能做的一切，脚本也都能做。

完整的参考是由网关代码生成的 OpenAPI 文档 [`openapi.json`](https://github.com/logicplanes/kaeso/blob/dev/openapi.json)，其中每个操作都注明了它所检查的那一项权限。获取当前运行版本的文档：

```bash
docker compose exec gateway kaeso openapi > openapi.json
```

## 身份验证

| 方式 | 发送 | 以何种权限行事 |
| --- | --- | --- |
| 会话 | `auth/*` 下的登录端点设置的会话 Cookie | 已登录用户的权限 |
| 具有平台访问权限的 API 密钥 | `Authorization: Bearer sk-kso-…` | 其成员或服务账号的权限，仅限于其所属项目 |
| 平台密钥 | `Authorization: Bearer sk-kso-org-…` | 管理员角色的权限，适用于其组织及组织内的所有项目 |

会话供控制台和使用 curl 的人员使用。Cookie 是 HttpOnly 的 `__Host-kaeso_session`；在位于 localhost 等回环地址的 `http://` 部署上为 `kaeso_session`。项目密钥适合单个项目的脚本和 CI，平台密钥适合整个组织的自动化。

- **密钥只能访问 `orgs/{org}/*`。** `me/*`、`instance/*` 以及 `auth/*` 中需要登录的路由会以 403 拒绝密钥，组织级路由也会以 403 拒绝项目密钥。Platform API 只从 `Authorization` 读取密钥，从不读取 `x-api-key`。
- **平台访问权限在创建密钥时选择**，即 `"access": "platform"` 或 `"both"`；默认为 `inference`。这需要 `keys.platform_access.grant` 权限。参见 [API 密钥](../console/api-keys.md)。
- **平台密钥只能在会话中创建**，且创建者必须拥有管理员角色的全部权限。平台密钥从不调用模型。
- **组织策略作用于会话。** 要求单点登录的组织会以 403 拒绝其他会话；要求双重验证的组织会拒绝没有第二重验证的密码会话。
- **未知密钥会被限流。** 来自同一地址、使用未知 API 密钥的请求过多时（模型 API 和 Platform API 合并计算），会收到 `429 rate_limited` 和 `Retry-After`，直到时间窗口结束；相关设置见 [API 密钥查找失败](../self-hosting/configuration.md#api-密钥查找失败)。

### 跨站请求

除 `GET`、`HEAD` 和 `OPTIONS` 之外，凡是携带会话 Cookie 的请求，以及发往公开 `auth/*` 端点的此类请求，都必须发送 `Origin`，其值为部署公开 URL 的源，例如 `Origin: https://kaeso.example.com`。浏览器会自动发送它；`Referer` 不能代替它。缺少该请求头或源不同时，响应为 `403 permission_denied`，消息会指明应发送的请求头。这样，其他网站既无法借用某人的会话执行操作，也无法让浏览器登录到他们的账户。

- 使用 API 密钥或平台密钥的请求不需要 `Origin`。
- 读取请求从不需要它，`GET auth/methods` 和 `GET auth/oidc/start` 也不例外。
- 代为调用 `POST auth/methods`、`POST auth/password` 或 `POST auth/oidc/start` 的服务器需发送部署的源。若要从其他网站把人员送往身份提供商，请改为链接到 `GET auth/oidc/start`；参见[登录和单点登录](../self-hosting/sign-in.md#直接前往提供商的链接)。

### 近期登录

在会话中，部分操作要求在最近 15 分钟内登录过；否则响应为 `403 recent_sign_in_required`，控制台会请用户重新登录。密钥不受此限制。

- 所需权限在 `orgs/{org}/rights` 中带有 `recent_sign_in` 标记的操作：`org.delete`、`org.ownership.transfer`、`provider_credentials.create`、`provider_credentials.replace`、`security.sso.manage`、`security.sso.require`、`platform_keys.create`、`logs.delete` 和 `keys.platform_access.grant`。
- 自己的账户中：更改电子邮件地址或通知设置、关闭双重验证、生成新的恢复码、删除账户。
- 实例管理员：更改实例设置，以及计划删除、恢复或删除组织。

## 约定

| 主题 | 规则 |
| --- | --- |
| 格式 | 基于 JSON 的 REST。路径使用复数 kebab case（`service-accounts`），字段使用 `snake_case`。请求正文以 `content-type: application/json` 发送；端点不认识的字段会以 400 被拒绝 |
| ID | 前缀加按时间排序的部分：`org_…`、`proj_…`、`usr_…`、`key_…`、`opk_…`、`svc_…`、`cred_…`、`model_…`、`alias_…`、`lim_…`、`role_…`、`inv_…`、`ses_…`、`req_…`、`evt_…` |
| 时间 | RFC 3339，UTC |
| 金额 | 以十进制字符串表示的美元，例如 `"0.0001"`。价格和数额以十进制字符串或整数发送；JSON 小数（例如 `0.5`）会以 400 被拒绝 |
| 限额数额 | 最多 10^12 个请求或 Token；支出预算最多 1,000,000,000 美元 |
| 创建 | 对集合发送 `POST`，以 201 和该资源响应 |
| 更新 | 发送只含待更改字段的 `PATCH` |
| 操作 | `POST …/archive`、`…/disable`、`…/enable`、`…/replace`、`…/restore`；归档会保留记录 |
| 删除 | `DELETE`，仅用于确实会删除内容的地方：服务商密钥、别名、目录中的模型、限额、自定义角色、会话、邀请、账户 |
| 审计 | 每项更改都会在同一事务中写入其所在层级的审计日志 |
| 请求 ID | 每个响应都带有 `kaeso-request-id` |

### 列表

列表接受 `limit`（1 到 100，默认 20）和 `after`，后者为上一页的 `next_cursor`；筛选条件是另外的查询参数。列表按 ID 排序，即按创建顺序；请求日志和审计日志则从新到旧。

```json
{ "data": [], "has_more": true, "next_cursor": "…" }
```

最后一页的 `next_cursor` 为 `null`。

### 修订版本

目录中的模型和限额带有 `revision`，每次更改都会递增；读取、创建或更改时会以 `ETag: "<revision>"` 响应。在 `PATCH` 或 `DELETE` 的 `If-Match` 中带回该标签，就只会更改你读取的那个修订版本：条目在此期间被更改时，响应为 `412 conflict`。不带 `If-Match` 时，更改作用于当前修订版本。

### 已计划删除的组织

已计划删除的组织在恢复或删除之前为只读。读取按调用者的权限正常进行。组织中的每项更改，包括退出组织和接受加入该组织的邀请，都返回 `409 org_scheduled_for_deletion`，但 `POST orgs/{org}/restore` 和实例管理员的 `instance/orgs/{org}/*` 除外。其密钥完全无法通过身份验证。

`auth/session` 会在每个成员资格中显示这类组织，带有 `org_state`、`org_deletion_due_at`，以及缩减为仅能查看、导出和 `org.delete` 的权限。它还包含实例的 `org_deletion` 策略（`allowed`、`grace_days`）。

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

消息是写给人看的一句短句，控制台会原样显示。`param` 指出出错的正文字段、查询参数或路径参数；当仅缺一项权限即可放行请求时，`right` 指出调用者缺少的那项权限。不适用时两者都会省略。

| 状态码 | `type` | 何时出现 |
| --- | --- | --- |
| 400 | `invalid_request` | 正文不是 JSON、字段未知、参数类型或格式错误、值超出范围 |
| 401 | `authentication` | 没有会话或密钥，或会话、密钥未知、已过期或已结束 |
| 403 | `permission_denied` | 缺少权限、密钥用于其不能访问的路由、缺少 `Origin`、组织策略 |
| 403 | `recent_sign_in_required` | 参见[近期登录](#近期登录) |
| 404 | `not_found` | 路径未知，或对象不存在或调用者无权查看 |
| 405 | `invalid_request` | 路径不接受该方法；`Allow` 列出它接受的方法 |
| 409 | `conflict` | 更改与当前状态冲突，例如名称已被占用 |
| 409 | `org_scheduled_for_deletion` | 在等待删除的组织中进行更改 |
| 412 | `conflict` | `If-Match` 指定的是较旧的修订版本 |
| 413 | `request_too_large` | 正文大于 `KAESO_BODY_LIMITS_PLATFORM_API`，默认为 1 MiB |
| 429 | `rate_limited` | 未知密钥过多，或试验场请求触及限额；`Retry-After` 说明何时重试 |
| 500 | `internal` | 网关内部故障；消息会给出需报告的请求 ID |
| 502, 503, 504, 429 | `provider_error` | 仅用于试验场的模型请求 |
| 503 | `unavailable` | 无法连接数据库或其他后端服务，例如 “Kaeso cannot reach its database right now; try again shortly.” |

## 使用 curl 的完整示例

以下请求使用平台密钥，为一个应用创建项目，并通过它调用模型。请在控制台中组织的设置里，于**平台密钥**下创建该密钥；组织的 ID 就在控制台的地址中，即 `/orgs/org_…`。这些命令需要 `jq`。

```bash
KAESO=https://kaeso.example.com
ORG=org_…
AUTH="Authorization: Bearer sk-kso-org-…"
```

创建项目：

```bash
PROJECT=$(curl -s -H "$AUTH" -H 'content-type: application/json' \
  -d '{"name": "Support bot"}' \
  "$KAESO/platform/v1/orgs/$ORG/projects" | jq -r .id)
```

添加组织的 Anthropic 密钥。某个服务商的第一个密钥会成为其默认密钥，所有未分配密钥的项目都使用它。密钥本身不会再次显示，只有最后四个字符保持可见。

```bash
curl -s -H "$AUTH" -H 'content-type: application/json' \
  -d '{"provider": "anthropic", "name": "Production", "secret": "sk-ant-…"}' \
  "$KAESO/platform/v1/orgs/$ORG/credentials"
```

把模型加入项目目录。其价格、限制和能力来自 models.dev。

```bash
curl -s -H "$AUTH" -H 'content-type: application/json' \
  -d '{"provider": "anthropic", "model_id": "claude-haiku-4-5"}' \
  "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/models"
```

为应用创建一个具有开发者角色的服务账号，并为其创建一个 90 天后过期的密钥。响应中的 `secret` 就是 API 密钥，只显示这一次。

```bash
ACCOUNT=$(curl -s -H "$AUTH" -H 'content-type: application/json' \
  -d '{"name": "support-bot", "role": "developer"}' \
  "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/service-accounts" | jq -r .id)

KEY=$(curl -s -H "$AUTH" -H 'content-type: application/json' \
  -d '{"name": "support-bot", "expires_in_days": 90}' \
  "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/service-accounts/$ACCOUNT/keys" | jq -r .secret)
```

像应用那样，用新密钥调用模型 API：

```bash
curl -s "$KAESO/v1/messages" -H "x-api-key: $KEY" -H 'anthropic-version: 2023-06-01' \
  -H 'content-type: application/json' \
  -d '{"model": "claude-haiku-4-5", "max_tokens": 100, "messages": [{"role": "user", "content": "Hello"}]}'
```

读取用量，可按 `day`（默认）、`week`、`month`、`model`、`key` 或 `member` 分组；拥有 `costs.view` 权限时包含费用。`from` 和 `to` 是 UTC 日期；按天分组时，两者最多相隔 400 天。`usage/span` 返回第一个和最后一个有用量的日期，便于按周或按月读取较长的时间段。

```bash
curl -s -H "$AUTH" "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/usage?group_by=model"
curl -s -H "$AUTH" "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/usage/span"
curl -s -H "$AUTH" "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/usage?from=2026-08-15&group_by=week"
```

读取请求日志（从新到旧），再读取其中一条。当项目记录内容且调用者拥有 `logs.content.view` 时，条目中包含请求内容。

```bash
curl -s -H "$AUTH" "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/logs?limit=10"
curl -s -H "$AUTH" "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/logs/req_…"
```

### 使用项目密钥

具有平台访问权限的 API 密钥在其所属项目中也能这样使用，权限为其成员或服务账号的权限：添加模型需要 `models.manage`，读取需要 `usage.view` 和 `logs.metadata.view`。创建项目和添加服务商密钥属于组织权限，项目密钥从不具备。属于某个成员的密钥可以通过 `POST …/projects/{project}/keys` 为该成员创建更多密钥。

### 使用会话

只要密码登录处于开启状态（默认开启），就可以用密码登录。把 Cookie 保存在文件中，并在每次更改时发送 `Origin`：

```bash
KAESO=https://kaeso.example.com
JAR=$(mktemp)

curl -s -c "$JAR" -H "Origin: $KAESO" -H 'content-type: application/json' \
  -d '{"email": "owner@example.com", "password": "correct horse battery staple"}' \
  "$KAESO/platform/v1/auth/password"

ORG=$(curl -s -b "$JAR" "$KAESO/platform/v1/auth/session" | jq -r '.memberships[0].org_id')
```

开启双重验证后，`auth/password` 会以 202 和一个 `challenge` 响应；再用 `POST /platform/v1/auth/totp` 和 `{"challenge": "…", "code": "123456"}` 完成登录，也可以用 `recovery_code` 代替 `code`。新部署的第一个用户通过 `kaeso setup-link` 输出的设置链接注册；参见[安装](../self-hosting/installation.md)。

之后，用 `-b "$JAR" -H "Origin: $KAESO"` 代替 `-H "$AUTH"`，上面的请求同样适用。会话还可以通过 `POST …/projects/{project}/keys` 为自己的用户创建密钥。

## 功能领域

| 路径 | 内容 |
| --- | --- |
| `auth/*` | 首次设置、登录方式、密码和双重验证、登录链接、重置密码、单点登录、会话、实例允许该地址时的注册、邀请 |
| `me`、`me/*` | 你的个人资料、头像和语言（`locale`：`auto`、`en`、`de`、`zh-CN`）、电子邮件地址、密码、双重验证和恢复码、会话、成员资格、你的密钥、通知、创建组织、删除账户 |
| `orgs/{org}/*` | 组织及其头像、成员、邀请、角色和权限、项目和默认项目、服务商密钥（每个服务商可有多个、默认密钥、分配给项目）、安全（单点登录、强制单点登录和双重验证、密钥有效期）、平台密钥、日志设置、存储和删除、用量、审计日志、所有权转让、删除和恢复 |
| `orgs/{org}/projects/{project}/*` | 项目及其头像、成员及其模型访问范围、服务账号、密钥及其模型和限额、密钥策略、目录（搜索、预览、批量）和别名、限额、内容日志、请求日志和导出、用量、审计日志、[试验场](#试验场) |
| `instance/*` | 仅限实例管理员：用户及其邀请、会话和管理员标记；组织及其成员、项目、用量和存储；设置和测试邮件；models.dev 的状态和同步；系统健康、存储和日志缓冲区；实例审计日志 |

实例管理员可以看到每个组织的元数据，但看不到其内容。他们在 `instance/orgs` 下创建、重命名、计划删除、恢复和删除组织。`POST instance/orgs/{org}/owner` 可将任何活跃用户设为所有者，包括管理员本人；实例管理员就是通过这种方式进入组织进行管理的，此操作会记录在该组织的审计日志中。只有实例管理员能通过 `POST instance/users/{user}/reset-2fa` 重置用户的双重验证。

没有计费相关的路由。`billing.view` 和 `billing.manage` 权限用于 Kaeso Cloud，在 `orgs/{org}/rights` 中带有 `cloud_only` 标记。

## 角色和权限

每个组织成员有一个组织角色，每个项目成员和服务账号有一个项目角色，并可在角色之外单独添加或移除权限。角色可以是内置角色，也可以是组织的自定义角色（`role_…`）。组织角色还持有在所有项目中的权限。[权限](../concepts/rights.md)列出了所有权限和内置角色。

| 路由 | 权限 | 内容 |
| --- | --- | --- |
| `GET orgs/{org}/rights` | `org.view` | 按类别列出的所有权限，包括名称、描述、标记（`sensitive`、`recent_sign_in`、`cloud_only`）及其依赖的权限 |
| `GET orgs/{org}/roles`、`GET …/roles/{role}` | `org.view` | 内置角色和自定义角色，包括其权限、在所有项目中的权限和持有人数；`?scope=org` 或 `project` |
| `POST orgs/{org}/roles`、`PATCH …/roles/{role}`、`DELETE …/roles/{role}` | `roles.manage` | 创建、更改和删除自定义角色，每个组织最多 50 个 |
| `GET orgs/{org}/members/{user}/rights` | `members.view` | 成员的有效组织权限、在所有项目中的权限，以及在调用者可见的每个项目中的权限，均附来源 |
| `GET orgs/{org}/projects/{project}/members/{user}/rights` | `project.members.view` | 成员在此项目中的有效权限，附来源 |
| `GET orgs/{org}/projects/{project}/my-rights` | `project.view` | 调用者自己在此项目中的权限，附来源 |
| `GET orgs/{org}/rights/{right}/grantors`、`GET …/projects/{project}/rights/{right}/grantors` | `members.view`、`project.view` | 可以向谁申请某项权限：最多 10 位持有该权限且可以更改权限的活跃成员，所有者优先 |
| `GET orgs/{org}/projects/{project}/audit-log` | `project.audit_log.view` | 项目中的更改 |

授权以 `{"role": "developer", "rights_added": [], "rights_removed": []}` 的形式发送，其中 `role` 是内置角色的名称或自定义角色的 ID；`preset` 是它已弃用的旧名称。返回时带有 `role`（`{id, kind, name}`）、有效的 `rights`，组织授权还带有 `project_rights`。邀请接受 `role` 和 `project_roles: [{project_id, role}]`；单点登录设置接受 `default_role`。

每次更改权限，无论针对授权还是自定义角色，都遵循相同的规则：

- 只能授予调用者自己持有的权限：组织权限在组织中持有，项目权限在该项目中持有，在所有项目中的权限只有调用者在所有项目中都持有时才能授予。
- 只能更改或移除其权限全部为调用者所持有的成员。只有所有者可以任命、更改、移除或邀请所有者。
- 授予的每项权限都必须连同其依赖的权限一起授予，否则响应为 400，例如 “`logs.delete` needs `logs.storage.view`.”
- 不能移除其余权限所依赖的权限，否则响应为 400，例如 “`logs.delete` needs `logs.storage.view`; remove `logs.delete` too.” 如果对自定义角色的更改会让某个持有人陷入这种状态，该更改同样会被拒绝。
- 对自定义角色的更改会立即作用于所有持有人；有人持有的角色无法删除。

有效权限的来源包括 `owner`、`role`（组织角色，用于组织权限）、`project_role`、`added`、`org_role`（组织角色在所有项目中的权限）、`org_right`（延伸到项目中的组织权限，例如 `projects.view.all`）以及 `key`（在项目中行事的 API 密钥）。

## 试验场

控制台的[试验场](../console/playground.md)以已登录成员的身份向项目的模型发送请求。它的两个路由都需要会话和项目权限 `playground.use`；密钥会以 403 被拒绝，因为应用应在 `/v1` 上调用模型。

`GET orgs/{org}/projects/{project}/playground/models` 列出成员在试验场中可调用的模型：项目目录中除嵌入模型以外的模型，再按成员的模型访问范围筛选。每个模型都附有要发送的名称、别名、是否支持推理及支持的推理强度、是否接受 `temperature`、限制和价格，以及项目是否有对应的服务商密钥。

`POST orgs/{org}/projects/{project}/playground/responses` 接受一个 OpenAI Responses 请求，字段包括 `model`、`input`（字符串或输入项）、`instructions`、`temperature`、`top_p`、`max_output_tokens`、`reasoning.effort`、`reasoning.summary`，以及必需的 `"stream": true`。响应是 `/v1/responses` 发送的 Responses 事件流，并带有保活注释。使用[上文的会话](#使用会话)：

```bash
curl -N -b "$JAR" -H "Origin: $KAESO" -H 'content-type: application/json' \
  -d '{"model": "claude-haiku-4-5", "input": "Hello", "stream": true, "reasoning": {"summary": "auto"}}' \
  "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/playground/responses"
```

试验场请求与其他模型请求无异，只是由成员直接发出，不使用密钥：

- **相同的处理路径。** 模型在项目目录中解析，包括别名和 `provider/model`，并按成员的模型访问范围检查。使用项目的服务商密钥；关闭流会取消发往服务商的请求。
- **限额。** 成员和项目的速率与预算适用；密钥限额不适用，因为没有密钥。
- **用量和日志。** 请求计入项目的费用和该成员的用量；按 `key` 查看用量时显示为 `playground`。请求以来源 `playground` 记录日志，项目记录内容时也包括内容；`logs` 和 `logs/export` 可用 `source=api` 或 `source=playground` 筛选。

流开始之前的错误是 Platform API 错误：

| 状态码 | `type` | 原因 |
| --- | --- | --- |
| 400 | `invalid_request` | 模型不在目录中，或服务商不向项目的服务商密钥提供该模型时，`param` 为 `model`；发送 `"stream": false` 时为 `stream` |
| 403 | `permission_denied` | 模型超出成员的模型访问范围 |
| 409 | `conflict` | 项目已归档，或尚未为模型的服务商设置服务商密钥 |
| 409 | `org_scheduled_for_deletion` | 组织正在等待删除 |
| 429 | `rate_limited` | 触及速率或预算，消息中会注明 |
| 503 | `unavailable` | 无法读取目录、服务商密钥或限额，或网关正在重启 |
| 502, 429, 503, 504 | `provider_error` | 服务商失败或拒绝了服务商密钥、对请求限速、过载或未及时响应 |

流开始之后，错误以 Responses 的 `error` 事件送达，与 `/v1` 相同。

## 控制台如何使用它

控制台与网关同源提供，因此其会话 Cookie 是第一方 Cookie，无需 CORS。它只保存 HttpOnly Cookie，并像浏览器一样在每次更改时发送 `Origin`。此外，它会：

- 读取 `auth/session`，获取用户、其成员资格（包括组织角色、权限和在所有项目中的权限）、是否为实例管理员，以及能否创建组织（`can_create_orgs`）；
- 读取项目的 `my-rights` 来决定在项目中显示哪些内容，并从 `orgs/{org}/rights` 获取权限名称、分组和依赖关系；
- 用 `models/search` 和 `models/preview` 填充模型表单；
- 收到 `recent_sign_in_required` 时提示重新登录，然后重试该操作；
- 以流式文件下载 `logs/export` 和 `audit-log/export`；
- 用 `fetch` 流式接收试验场的回复，**停止**会中止该请求，从而取消发往服务商的请求；
- 绘制用量图表时，约三个月以内按天、约一年以内按周、更长按月，**全部时间**从 `usage/span` 的 `first_day` 开始。
