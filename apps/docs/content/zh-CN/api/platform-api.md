---
title: Platform API
description: 通过 HTTP 管理实例时如何进行身份验证、翻页、处理错误以及查找路由。
order: 3
---

Platform API 管理控制台显示的一切：组织、项目、成员、密钥、服务商密钥、模型、限额、用量和日志。它位于您实例域名下的 `/platform/v1`，控制台只使用这个 API。

把平台密钥放在 `KAESO_PLATFORM_KEY` 中，用您组织的 ID 替换 `org_…`，即可列出组织的项目：

```bash tab="curl"
curl https://kaeso.example.com/platform/v1/orgs/org_…/projects \
  -H "Authorization: Bearer $KAESO_PLATFORM_KEY"
```

```python tab="Python"
import json
import os
import urllib.request

request = urllib.request.Request(
    "https://kaeso.example.com/platform/v1/orgs/org_…/projects",
    headers={"Authorization": f"Bearer {os.environ['KAESO_PLATFORM_KEY']}"},
)
with urllib.request.urlopen(request) as response:
    print(json.load(response))
```

```ts tab="TypeScript"
const url = "https://kaeso.example.com/platform/v1/orgs/org_…/projects";
const response = await fetch(url, {
  headers: { Authorization: `Bearer ${process.env.KAESO_PLATFORM_KEY}` },
});
console.log(await response.json());
```

响应是一页项目：

```json
{
  "data": [
    {
      "id": "proj_…",
      "org_id": "org_…",
      "name": "Support bot",
      "is_default": false,
      "content_logging": true,
      "created_at": "2026-10-01T09:30:00Z"
    }
  ],
  "has_more": false,
  "next_cursor": null
}
```

[用 Platform API 实现自动化](automation.md)逐步演示如何设置项目、模型和应用的密钥。

## OpenAPI 文档

[`openapi.json`](https://github.com/logicplanes/kaeso/blob/dev/openapi.json) 描述每个操作：路径、字段、响应，以及它检查的那一项权限。网关根据自身代码生成该文档。获取您所运行版本的文档：

```bash
docker compose exec gateway kaeso openapi > openapi.json
```

## 身份验证

| 方式 | 发送 | 以何种权限行事 |
| --- | --- | --- |
| 平台密钥 | `Authorization: Bearer sk-kso-org-…` | 管理员角色的权限，适用于其组织及组织内的所有项目 |
| 具有平台访问权限的 API 密钥 | `Authorization: Bearer sk-kso-…` | 其成员或服务账号的权限，仅限于其所属项目 |
| 会话 | `auth/*` 下的登录端点设置的会话 Cookie | 您自己的权限 |

自动化整个组织时使用平台密钥，单个项目的脚本和 CI 使用项目密钥。会话供控制台和使用 curl 的人员使用。会话 Cookie 是 `__Host-kaeso_session`；在位于 localhost 等回环地址的 `http://` 实例上为 `kaeso_session`。

- **密钥只能访问 `orgs/{org}/*`。** `me/*`、`instance/*` 以及 `auth/*` 中需要登录的路由会对密钥返回 403。组织级路由会对项目密钥返回 403。
- **密钥放在 `Authorization` 中。** Platform API 从不读取 `x-api-key`。
- **平台访问权限在创建密钥时选择**，即 `"access": "platform"` 或 `"both"`。默认为 `inference`，授予平台访问权限需要 `keys.platform_access.grant` 权限（[API 密钥](../console/api-keys.md)）。
- **平台密钥只能在会话中创建**，且创建者必须拥有管理员角色的全部权限。平台密钥从不调用模型。
- **密钥通过一个请求轮换。** `POST …/keys/{key}/rotate`、`…/service-accounts/{account}/keys/{key}/rotate` 和 `orgs/{org}/platform-keys/{key}/rotate` 接受 `grace_hours`（0、1、24 或 168）以及新密钥的过期时间。它们返回保留旧密钥设置的新密钥，所需条件与创建该密钥相同，另需管理该密钥的权限（[轮换密钥](../console/api-keys.md#轮换密钥)）。
- **组织策略作用于会话。** 要求单点登录的组织会对其管理的账户的会话返回 403，除非会话来自其身份提供商；访客的会话可以通过。要求双重验证的组织会对没有第二重验证的会话返回 403。例外是来自在该组织有效的单点登录的会话：即其自己的单点登录，或管理该账户的组织的单点登录（见[受管理的账户](../self-hosting/sign-in.md#受管理的账户)）。
- **未知密钥会被限流。** 来自同一地址、使用未知密钥的请求过多时（两个 API 合并计算），会收到 `429 rate_limited` 和 `Retry-After`（[API 密钥查找失败](../self-hosting/configuration.md#api-密钥查找失败)）。

### 跨站请求

携带会话 Cookie 且会更改内容的请求，必须发送值为实例源的 `Origin`，例如 `Origin: https://kaeso.example.com`。发往公开 `auth/*` 端点的此类请求也是如此。浏览器会自动发送该请求头，`Referer` 不能代替它。缺少该请求头或源不同时，Kaeso 返回 `403 permission_denied`，消息会指明应在哪个地址打开 Kaeso。

这样，其他网站既无法借用您的会话执行操作，也无法让您的浏览器登录到他们的账户。

- 使用 API 密钥或平台密钥的请求不需要 `Origin`。
- `GET`、`HEAD` 和 `OPTIONS` 从不需要它，`GET auth/methods` 也不例外。
- 调用 `POST auth/methods`、`POST auth/password` 或 `POST auth/oidc/start` 的服务器需发送实例的源。若要从其他网站把人员送往单点登录，请链接到控制台的登录页，它会先询问地址（[登录和单点登录](../self-hosting/sign-in.md#登录页)）。

### 近期登录

在会话中，部分操作要求在最近 15 分钟内登录过。否则 Kaeso 返回 `403 recent_sign_in_required`，控制台会请您重新登录。您可以使用密码或登录链接确认；只有账户没有其他方式时，才通过单点登录确认。密钥不受此限制。

`GET me/sign-in-again` 列出账户可用的确认方式：`password`、`magic_link`，以及 `set_password`：账户尚无密码，但可以通过 `POST auth/password-reset` 设置一个。只有这三项都为 `false` 时，`sso` 才列出账户已关联其身份提供商的组织。管理该账户的组织排在最前。如果该组织要求单点登录，则只有它的身份提供商可以确认。

`POST me/sign-in-again/sso` 在其中一个身份提供商开始确认。Kaeso 会要求身份提供商进行一次新的登录（`prompt=login`、`max_age=0`），只有同一账户可以确认。随后 Kaeso 会把会话替换为来自该组织单点登录的会话。失败时浏览器会带着 `#sso_error=` 回到 `next`，例如 `other_account` 或 `link_required`。

`cannot_confirm` 表示身份提供商的答复没有显示新的登录：它省略了 `auth_time`，或带回了它沿用的较早登录。此后 `sso` 不再列出该组织，其连接在 `GET orgs/{org}/security` 中显示 `confirms_sign_in: false`。

以下操作要求近期登录：

- **组织和项目中：** 在 `orgs/{org}/rights` 中带有 `recent_sign_in` 标记的权限，即 `org.delete`、`org.ownership.transfer`、`provider_credentials.create`、`provider_credentials.replace`、`security.sso.manage`、`security.sso.require`、`security.2fa.require`、`platform_keys.create`、`logs.delete` 和 `keys.platform_access.grant`。
- **自己的账户中：** 更改电子邮件地址、为没有密码的账户设置密码、关闭双重验证、生成新的恢复码和删除账户。
- **实例管理员：** 更改账户创建、组织创建或允许的域名，选择自动更新，应用更新，以及计划删除或删除组织。选择实例管理员的单点登录也要求近期登录，且只能用密码或登录链接确认。

## 约定

| 主题 | 规则 |
| --- | --- |
| 格式 | 基于 JSON 的 REST：路径使用复数 kebab case（`service-accounts`），字段使用 `snake_case`。请求正文以 `content-type: application/json` 发送；端点不认识的字段会得到 400。 |
| ID | 前缀加按时间排序的部分：`org_…`、`proj_…`、`usr_…`、`mem_…`、`inv_…`、`svc_…`、`key_…`、`opk_…`、`cred_…`、`model_…`、`alias_…`、`lim_…`、`role_…`、`ses_…`、`req_…`、`evt_…`、`exp_…`、`upd_…`。 |
| 时间 | RFC 3339，UTC。每个响应都在 `Kaeso-Time-Zone` 中给出实例的时区，例如 `Europe/Berlin`，即控制台显示时间所用的时区。 |
| 金额 | 以十进制字符串表示的美元，例如 `"0.0001"`。价格和数额以十进制字符串或整数发送；JSON 小数（例如 `0.5`）会得到 400。 |
| 限额数额 | 最多 10^12 个请求或 Token；支出预算最多 1,000,000,000 美元。 |
| 创建 | 对集合发送 `POST`，以 201 和新资源响应。 |
| 更新 | 发送只含待更改字段的 `PATCH`。 |
| 操作 | `POST …/archive`、`…/disable`、`…/enable`、`…/replace` 或 `…/restore`。归档会保留记录。 |
| 删除 | `DELETE`，仅用于确实会删除内容的地方：服务商密钥、别名、目录中的模型、限额、自定义角色、会话、邀请和您的账户。 |
| 审计 | 每项更改都会在同一事务中写入其所在层级的审计日志。 |
| 请求 ID | 每个响应都带有 `kaeso-request-id`。 |

### 列表

列表接受 `limit`（1 到 100，默认 20）和 `after`，后者为上一页的 `next_cursor`。其他查询参数用于筛选。列表按创建顺序排列，但请求日志和审计日志从新到旧。

```json
{ "data": [], "has_more": true, "next_cursor": "…" }
```

最后一页的 `next_cursor` 为 `null`。

### 修订版本

目录中的模型和限额带有 `revision`，每次更改都会递增。读取、创建或更改时会以 `ETag: "<revision>"` 响应。在 `PATCH` 或 `DELETE` 的 `If-Match` 中带回该标签，就只会更改您读取的那个修订版本。条目在此期间被更改时，Kaeso 返回 `412 conflict`；不带 `If-Match` 时，更改作用于当前修订版本。

### 已计划删除的组织

已计划删除的组织在恢复或删除之前为只读。读取按调用方的权限正常进行，其密钥则完全无法通过身份验证。组织中的每项更改都返回 `409 org_scheduled_for_deletion`，包括退出组织和接受加入该组织的邀请。只有 `POST orgs/{org}/restore` 和实例管理员的 `instance/orgs/{org}/*` 仍可使用。

`auth/session` 会在每个成员身份中用 `org_state` 和 `org_deletion_due_at` 标出这类组织。成员在其中的权限缩减为查看、导出和 `org.delete`。会话还包含实例的 `org_deletion` 策略，带有 `allowed` 和 `grace_days`。

## 错误

每个错误都使用同一种正文：

```json
{
  "error": {
    "type": "permission_denied",
    "message": "You need the right `models.manage` in this project; ask one of its admins for it.",
    "right": "models.manage"
  }
}
```

消息是写给人看的一句短句，控制台会原样显示。`param` 指出出错的字段、查询参数或路径参数。当仅缺一项权限即可放行请求时，`right` 指出调用方缺少的那项权限。不适用时两者都会省略。

| 状态码 | `type` | 何时出现 |
| --- | --- | --- |
| 400 | `invalid_request` | 正文不是 JSON，或含有未知字段。参数类型或格式错误，或值超出范围。 |
| 401 | `authentication` | 没有会话或密钥，或会话、密钥未知、已过期或已结束。 |
| 403 | `permission_denied` | 缺少权限、密钥用于其不能访问的路由、缺少 `Origin`，或组织策略不允许。 |
| 403 | `recent_sign_in_required` | 该操作需要[近期登录](#近期登录)。 |
| 403 | `other_sign_in_required` | 会话来自不管理该账户的组织的单点登录，而操作不在该组织内；或者会话来自单点登录，而操作属于实例管理，且该单点登录不来自为实例管理员选定的组织。请使用密码或登录链接登录。 |
| 404 | `not_found` | 路径不存在，或对象不存在或调用方无权查看。 |
| 405 | `invalid_request` | 路径不接受该方法。`Allow` 列出它接受的方法。 |
| 409 | `conflict` | 更改与当前状态冲突，例如名称已被占用。 |
| 409 | `org_scheduled_for_deletion` | 在已计划删除的组织中进行更改。 |
| 412 | `conflict` | `If-Match` 指定的是较旧的修订版本。 |
| 413 | `request_too_large` | 正文大于 `KAESO_BODY_LIMITS_PLATFORM_API`，默认为 1 MiB。 |
| 429 | `rate_limited` | 来自同一地址的登录尝试或未知密钥过多，或试验场请求触及限额。`Retry-After` 说明何时重试。 |
| 500 | `internal` | 网关内部故障。消息会给出可报告的请求 ID。 |
| 503 | `unavailable` | Kaeso 无法连接数据库或其他后端服务，例如“Kaeso cannot reach its database right now; try again shortly.” |
| 429、502、503、504 | `provider_error` | 服务商出错，仅用于[试验场](#试验场)请求。 |

## 路由

路由相对于 `/platform/v1`。[`openapi.json`](#openapi-文档) 列出了每个路由及其字段。

| 路径 | 内容 |
| --- | --- |
| `auth/*` | 首次设置、登录方式、密码和双重验证登录、登录链接、重置密码、单点登录、会话、实例允许该地址时的注册，以及邀请。`auth/session` 返回用户、其带有角色和权限的成员身份、管理该账户的组织（`managed_by`），以及能否创建组织（`can_create_orgs`）。 |
| `me`、`me/*` | 您的个人资料、头像和语言（`locale`：`auto`、`en`、`de` 或 `zh-CN`）、电子邮件地址、密码、双重验证和恢复码。还有您的会话、成员身份、密钥和通知，以及创建组织和删除账户。 |
| `orgs/{org}/*` | 组织及其头像、成员、邀请、角色和权限，以及项目。还有 Kaeso 调用的服务商、服务商密钥、安全设置、平台密钥、日志设置和存储、用量、审计日志、所有权转让、删除和恢复。 |
| `orgs/{org}/projects/{project}/*` | 项目及其头像、成员及其模型访问范围、服务账号、密钥及其模型和限额，以及密钥策略。还有目录和别名、限额、内容日志、请求日志和导出、用量、审计日志和[试验场](#试验场)。 |
| `instance/*` | 供实例管理员使用：用户、组织、实例设置和可通过其单点登录管理实例的组织（`instance/settings/admin-sso`）、models.dev 同步、系统健康状况和存储、更新，以及实例审计日志。 |

实例管理员能看到每个组织的元数据，但从不能看到其内容。他们在 `instance/orgs` 下创建、重命名、计划删除、恢复和删除组织。`POST instance/orgs/{org}/owner` 可将任何有效用户设为所有者，管理员本人也可以；实例管理员正是借此进入组织进行管理。组织的审计日志会记录此操作。

只有实例管理员能用 `POST instance/users/{user}/reset-2fa` 重置用户的双重验证。没有计费路由：`billing.view` 和 `billing.manage` 权限用于 Kaeso Cloud，在 `orgs/{org}/rights` 中带有 `cloud_only` 标记。

`GET orgs/{org}/providers` 需要 `org.view`，列出 Kaeso 调用的每个服务商，在每个组织中都相同。每个服务商带有：

- ID 和名称；
- 创建其密钥的页面（`key_page`）；
- 其密钥的开头（`key_prefix`，没有固定前缀时为 `null`）；
- Kaeso 调用它所用的格式，默认格式在前。

[支持的服务商](../concepts/supported-providers.md)以表格列出同一份列表。

## 角色和权限

每个组织成员有一个组织角色，每个项目成员和服务账号有一个项目角色。角色可以是内置角色，也可以是组织的自定义角色（`role_…`），您可以按成员单独为其添加或移除权限。组织角色还包含在每个项目中的权限。[权限](../concepts/rights.md)列出了每项权限和每个内置角色。

| 路由 | 权限 | 返回 |
| --- | --- | --- |
| `GET orgs/{org}/rights` | `org.view` | 按类别列出的每项权限，包括名称、说明、标记（`sensitive`、`recent_sign_in`、`cloud_only`、`owner_only`）及其所需的权限。 |
| `GET orgs/{org}/roles`、`GET …/roles/{role}` | `org.view` | 内置角色和自定义角色，包括其权限和持有者人数。可用 `?scope=org` 或 `?scope=project` 筛选。 |
| `POST orgs/{org}/roles`、`PATCH …/roles/{role}`、`DELETE …/roles/{role}` | `roles.manage` | 创建、更改或删除自定义角色。每个组织最多 50 个。 |
| `GET orgs/{org}/members/{user}/rights` | `members.view` | 成员在组织中、在每个项目中，以及在调用方可见的每个项目中的有效权限及其来源。 |
| `GET orgs/{org}/projects/{project}/members/{user}/rights` | `project.members.view` | 项目成员在此项目中的有效权限及其来源。 |
| `GET orgs/{org}/projects/{project}/my-rights` | `project.view` | 调用方自己在此项目中的权限及其来源。 |
| `GET orgs/{org}/rights/{right}/grantors` | `members.view` | 最多 10 位拥有该权限且可以更改权限的有效成员，所有者优先。 |
| `GET orgs/{org}/projects/{project}/rights/{right}/grantors` | `project.members.view` | 同上，针对此项目中的权限。 |

授权以 `{"role": "developer", "rights_added": [], "rights_removed": []}` 发送。`role` 是内置角色的名称或自定义角色的 ID；`preset` 是它已弃用的名称。响应包含 `{id, kind, name}` 形式的 `role`、有效的 `rights`，组织授权还包含 `project_rights`。邀请接受 `role` 和 `project_roles: [{project_id, role}]`，单点登录设置接受 `default_role`。

每次更改权限都遵循[授权规则](../concepts/rights.md#授权规则)：

- 授予自己没有的权限，或更改拥有您所缺权限的成员，会得到 403，例如“You can't grant `audit_log.view`: you don't hold it yourself.”
- 授予某项权限却缺少它所需的权限，或移除另一项权限所需的权限，会得到 400，消息会同时指出两者。例如“`logs.delete` needs `logs.storage.view`; remove `logs.delete` too.”

有效权限会列出其来源：

| 来源 | 权限来自 |
| --- | --- |
| `owner` | 组织所有者身份 |
| `role` | 组织角色，用于组织权限 |
| `project_role` | 项目角色 |
| `added` | 为成员单独添加的权限 |
| `org_role` | 组织角色在每个项目中的权限 |
| `org_right` | 延伸到项目中的组织权限，例如 `projects.view.all` |
| `key` | 在项目中行事的 API 密钥 |

## 试验场

控制台的[试验场](../console/playground.md)以已登录成员的身份向项目的模型发送请求。它的两个路由都需要会话和项目权限 `playground.use`。它们对密钥返回 403，因为应用在 `/v1` 上调用模型。

| 路由 | 作用 |
| --- | --- |
| `GET orgs/{org}/projects/{project}/playground/models` | 列出成员可以调用的模型：不含嵌入模型的目录，再按成员的模型访问范围收窄。每个模型都带有名称、别名、推理强度、限额和价格，以及是否接受 `temperature`、项目是否有对应的服务商密钥。 |
| `POST orgs/{org}/projects/{project}/playground/responses` | 接受带 `"stream": true` 的 OpenAI Responses 请求，并以 `/v1/responses` 发送的事件流响应，包括保活注释。 |

请求接受 `model`、`input`（字符串或输入项）、`instructions`、`temperature`、`top_p`、`max_output_tokens`、`reasoning.effort`、`reasoning.summary` 和必须为 `true` 的 `stream`。使用[会话](automation.md#改用会话)：

```bash
project=https://kaeso.example.com/platform/v1/orgs/org_…/projects/proj_…
curl -N "$project/playground/responses" \
  -b "$JAR" \
  -H "Origin: https://kaeso.example.com" \
  -H "content-type: application/json" \
  -d '{
    "model": "claude-haiku-4-5",
    "input": "Write a haiku about gateways.",
    "stream": true
  }'
```

试验场请求与其他模型请求一样，只是由成员在没有密钥的情况下发出：

- **相同的路径。** 模型在项目目录中解析，包括别名和 `provider/model`，且必须在成员的模型访问范围内。Kaeso 使用项目的服务商密钥，关闭流会取消发往服务商的请求。
- **限额。** 成员和项目的速率与预算适用。密钥限额不适用，因为没有密钥。
- **用量和日志。** Kaeso 将请求计入项目，并算作该成员的用量；按 `key` 查看用量时显示为 `playground`。其日志条目的来源为 `playground`，项目记录内容时还包含请求内容。`logs` 和 `logs/export` 可用 `source=api` 或 `source=playground` 筛选。

流开始之前的错误是 Platform API 错误：

| 状态码 | `type` | 原因 |
| --- | --- | --- |
| 400 | `invalid_request` | 模型不在目录中时，`param` 为 `model`。`"stream": false` 时为 `stream`。 |
| 404 | `not_found` | 服务商不向项目的服务商密钥提供该模型，消息与 `/v1` 应用收到的相同（[服务商不提供的模型](errors.md#服务商不提供的模型)）；或者服务商对请求中指定的其他内容返回了 404。 |
| 403 | `permission_denied` | 模型不在成员的模型访问范围内。 |
| 409 | `conflict` | 项目已归档，或没有该模型服务商的服务商密钥。 |
| 409 | `org_scheduled_for_deletion` | 组织已计划删除。 |
| 429 | `rate_limited` | 达到了速率或预算，消息会指明是哪一个。 |
| 503 | `unavailable` | Kaeso 无法读取目录、服务商密钥或限额，或正在重启。 |
| 429、502、503、504 | `provider_error` | 服务商对请求限速、失败或拒绝了服务商密钥、过载，或未及时响应。 |

流开始后，错误以 Responses 的 `error` 事件到达，与 `/v1` 上相同。
