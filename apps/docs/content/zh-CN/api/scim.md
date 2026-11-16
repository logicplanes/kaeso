---
title: SCIM 预配
description: 让身份提供方通过 SCIM 2.0 添加和移除成员，成员离开时其密钥会被归档。
order: 13
---

启用单点登录的组织可以让其身份提供方通过 SCIM 2.0 添加和移除成员（见 [RFC 7644](https://datatracker.ietf.org/doc/html/rfc7644)，架构见 [RFC 7643](https://datatracker.ietf.org/doc/html/rfc7643)）。当有人离开公司、身份提供方停用其账户时，Kaeso 会把此人从组织及其项目中移除，并归档其在该组织中的 API 密钥，无需任何人打开控制台。

## 设置

1. 连接单点登录并验证你的邮箱域名（见[单点登录](../console/organizations-and-projects.md#单点登录sso)）。在**未受邀的人员能否加入？**中选择加入时的角色：预配的成员会获得该角色。
2. 在**安全** › **预配**中选择**创建令牌**并复制。令牌只显示一次，Kaeso 只保存其哈希。此操作需要**管理 SSO** 权限和近期登录；智能体和平台密钥无法执行。
3. 在身份提供方处，为应用填写同一区域中的 **SCIM 基础 URL**（`https://kaeso.example.com/platform/v1/orgs/{org}/scim/v2`），并把令牌作为 HTTP Bearer 令牌。

**Okta**：在应用的 **Provisioning** 选项卡中选择 **SCIM 2.0**，填写基础 URL，将 **Unique identifier field for users** 设为 `userName`、认证方式设为 **HTTP Header**，并开启 **Push New Users**、**Push Profile Updates** 和 **Push Groups**（见 [Okta 的 SCIM 指南](https://developer.okta.com/docs/api/openapi/okta-scim/guides/scim-20)）。

**Microsoft Entra ID**：在企业应用的 **Provisioning** 中将模式设为 **Automatic**，把基础 URL 填为 **Tenant URL**、令牌填为 **Secret Token**，然后选择 **Test Connection**（见 [Entra ID 的 SCIM 指南](https://learn.microsoft.com/en-us/entra/identity/app-provisioning/use-scim-to-provision-users-and-groups)）。

**替换令牌**会生成新令牌；被替换的令牌还可使用 24 小时，便于你更新身份提供方。**删除**会立即停止预配；现有成员保留。

## 作用

SCIM 只管理组织已验证域名下的账户。地址位于其他域名的访客不会被列出、添加或移除。服务账户不受影响。

| 身份提供方 | Kaeso |
| --- | --- |
| 创建用户 | 如无账户则创建，并以单点登录的默认角色添加成员身份。此人首次通过身份提供方登录时会关联其身份。已验证域名下的现有账户也会加入 |
| 更改用户姓名 | 更改账户名称。地址不能通过 SCIM 更改 |
| 将 `active` 设为 false | 像**从组织中移除**一样，把成员从组织及其项目中移除，归档其在该组织中的密钥，并结束该组织身份提供方登录的会话。若此人不再属于任何组织，其所有会话都会结束 |
| 再次将 `active` 设为 true | 以默认角色重新添加成员身份。已归档的密钥保持归档 |
| 删除用户 | 如上移除成员，并不再记录身份提供方管理此人 |
| 推送组 | 保存组及其成员。目前它们不授予任何访问权限 |

组织的最后一位所有者永远不会被移除：身份提供方会收到 `409`，并在 `detail` 中说明原因。审计日志会记录 `member.provision` 和 `member.deprovision` 以及用户的 `externalId`，并像其他成员一样发送**成员变更**邮件。

**安全** › **预配**会显示身份提供方最近一次请求的时间、它保持活跃的成员数，以及 Kaeso 最近拒绝的请求及原因。

## 端点

每个请求都以 `Authorization: Bearer …` 携带组织的令牌。错误的令牌或其他组织的令牌会得到 `401`，同一地址的重复失败会像未知 API 密钥一样被限流。回复为 `application/scim+json`；错误遵循 RFC 7644，例如 `{"schemas": ["urn:ietf:params:scim:api:messages:2.0:Error"], "status": "409", "scimType": "uniqueness", "detail": "…"}`。

| 方法和路径 | 作用 |
| --- | --- |
| `GET /ServiceProviderConfig`、`GET /ResourceTypes`、`GET /Schemas` | Kaeso 支持的功能：`PATCH` 和过滤，不支持批量、排序和 ETag |
| `GET /Users` | 列出已验证域名下的成员以及身份提供方停用的用户。过滤 `userName eq "…"` 和 `externalId eq "…"`；`startIndex` 和 `count`，最多 200 |
| `POST /Users` | 创建用户：`userName`（邮箱地址）、`externalId`、`name`、`displayName`、`emails`、`active` |
| `GET`、`PUT`、`PATCH`、`DELETE /Users/{id}` | 读取、替换、修改或删除用户。ID 是 Kaeso 用户 ID，例如 `usr_…` |
| `GET /Groups`、`POST /Groups` | 列出或创建组；过滤 `displayName eq` 和 `externalId eq` |
| `GET`、`PUT`、`PATCH`、`DELETE /Groups/{id}` | 读取、替换、修改或删除组 |

`PATCH` 按身份提供方实际发送的方式接收操作：`op` 不区分大小写，可带或不带 `path`，`active` 可以是布尔值或字符串 `"True"`、`"False"`。Kaeso 不保存的属性（如职位或企业扩展）会被接受并忽略。

平台 API 管理令牌：`GET /orgs/{org}/scim`、`POST /orgs/{org}/scim/token` 和 `DELETE /orgs/{org}/scim/token`（见[平台 API](platform-api.md)）。
