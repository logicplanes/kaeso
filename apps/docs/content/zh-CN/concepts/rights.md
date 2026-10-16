---
title: 权限
description: 组织和项目中的每项权限、哪些角色拥有它们，以及授予权限的规则。
order: 3
---

一项权限允许一类操作，每项权限都写作 `resource.action`，例如 `keys.all.view`。`orgs/{org}` 下的每个 Platform API 操作都只检查一项权限，检查对象为其路径中的组织或项目。角色是具名的权限集合；[成员和角色](../console/members-and-roles.md)介绍了如何在控制台中授予角色。

- 实例管理不是一项权限，而是用户上的一个标记。它不授予组织或项目中的任何权限。
- 个人账户本身不需要任何权限。
- 每项组织权限都还需要 `org.view`，每项项目权限都还需要 `project.view`。**需要**另一项权限的权限会连同所需权限一起授予：授予某项权限却不包含其所需权限的更改会被拒绝。已存储的权限从不会因此规则而被更改。
- **敏感**权限会暴露敏感数据或难以撤销；请谨慎授予。
- **需近期登录**的权限在通过控制台会话使用时，需要在过去 15 分钟内登录过。
- **仅限 Kaeso Cloud** 的权限在自托管部署上不起作用。

## 角色

每位组织成员有一个组织角色，每位项目成员和每个服务账号有一个项目角色。在角色之上，每个人还可以单独添加或移除权限。角色是关联的：更改一个角色会更改所有拥有该角色的人。

**内置角色**在每个组织中都相同，且无法更改。在 Platform API 中，其 ID 为其英文名称的小写形式。

| 组织角色 | 描述 |
| --- | --- |
| 所有者 | 拥有所有权限，包括未来版本新增的权限；只有所有者可以管理所有者。 |
| 管理员 | 管理组织和所有项目，但不能删除组织、转让所有权或更改账单。 |
| 财务 | 管理账单，并查看所有项目的用量和费用。 |
| 开发者 | 创建项目，查看用量、费用和服务商密钥；在被添加的项目中工作。 |
| 审计员 | 查看整个组织以供审查，包括审计日志和所有项目，但不做任何更改；看不到提示词或输出。 |
| 成员 | 查看组织及其成员；在被添加的项目中工作。 |

| 项目角色 | 描述 |
| --- | --- |
| 管理员 | 管理项目：成员、密钥、服务账号、模型、限额、日志和设置。 |
| 开发者 | 使用自己的密钥和试验场调用模型，并可查看项目，包括提示词和输出。 |
| 查看者 | 查看项目的设置、模型、限额、密钥、用量和日志元数据；不做任何更改。 |

### 每个内置角色拥有的权限

组织角色还拥有**在所有项目中的权限**，叠加在成员在各项目中自身的项目角色之上。

| 组织角色 | 组织权限 | 在所有项目中的权限 | 在默认项目中的初始角色 |
| --- | --- | --- | --- |
| 所有者 | 全部，包括未来版本新增的权限 | 全部 | 管理员 |
| 管理员 | 除 `org.delete`、`org.ownership.transfer` 和 `billing.manage` 之外的全部 | 全部 | 管理员 |
| 财务 | `org.view`、`members.view`、`projects.view.all`、`logs.storage.view`、`usage.org.view`、`costs.org.view`、`billing.view`、`billing.manage` | 无 | 查看者 |
| 开发者 | `org.view`、`members.view`、`projects.create`、`provider_credentials.view`、`logs.storage.view`、`usage.org.view`、`costs.org.view` | 无 | 开发者 |
| 审计员 | `org.view`、`members.view`、`projects.view.all`、`provider_credentials.view`、`platform_keys.view`、`logs.storage.view`、`usage.org.view`、`costs.org.view`、`billing.view`、`audit_log.view`、`audit_log.export` | 查看者的权限，以及 `project.audit_log.view` | 查看者 |
| 成员 | `org.view`、`members.view` | 无 | 查看者 |

| 项目角色 | 权限 |
| --- | --- |
| 管理员 | 所有项目权限 |
| 开发者 | 查看者的权限，以及 `api.request`、`playground.use`、`keys.own.create`、`keys.own.manage`、`logs.content.view` |
| 查看者 | `project.view`、`project.members.view`、`service_accounts.view`、`keys.all.view`、`models.view`、`limits.view`、`logs.metadata.view`、`usage.view`、`costs.view` |

有两项组织权限还会延伸到每个项目中：`projects.view.all` 授予 `project.view`，`projects.archive.any` 授予 `project.view` 和 `project.archive`。因此，成员在某个项目中的权限是其项目角色的权限，加上其组织角色在所有项目中的权限，再加上这两项权限所延伸的权限。

默认项目：所有者和管理员以项目管理员身份加入，开发者以开发者身份加入，其他所有组织角色（包括自定义角色）以查看者身份加入。

### 自定义角色

- 拥有 `roles.manage` 时，每个组织最多可以定义 50 个自定义角色，范围为组织或项目。组织角色包含组织权限和在所有项目中的权限；项目角色包含项目权限。拥有 `org.view` 的任何人都可以看到它们。
- 名称为 1 到 50 个字符，在同一组织和范围内不区分大小写地唯一，且不能与该范围内置角色的名称相同，也不能是 `custom`；描述最多 200 个字符。
- 自定义角色无法设置所有者：所有者只能是内置角色。
- 有人持有的角色（成员、有效的服务账号或待处理的邀请）无法删除，通过单点登录加入的人员所用的角色也无法删除。

## 授权规则

1. **只授予自己拥有的权限。** 更改成员的角色或权限、邀请、添加项目成员、创建或更改服务账号、创建、更改或删除自定义角色，以及为通过单点登录（SSO）加入的人员选择角色时，只能授予调用者拥有的权限：组织中的组织权限、该项目中的项目权限，只有在调用者在所有项目中都拥有某些权限时，才能授予在所有项目中的这些权限。
2. **只对同级者操作。** 只有拥有成员在该范围内所有权限的人，才能更改该成员的角色和权限，或将其移除。
3. **所有者**只能由所有者设置、更改、移除和邀请。组织永远不会失去最后一位所有者。
4. 每次更改所授予的每项权限都会检查其**所需权限**。
5. **密钥没有自己的权限。** API 密钥在其项目中以其成员或服务账号的身份行事，组织平台密钥则在其组织中以管理员角色的权限行事。

Platform API 中有关权限、角色和授权的路由，请参阅 [Platform API](../api/platform-api.md)。

## 组织权限

### 组织

| 权限 | 名称 | 允许的操作 | 备注 |
| --- | --- | --- | --- |
| `org.view` | 查看组织 | 查看组织及其设置和角色。 |  |
| `org.update` | 编辑组织 | 重命名组织，更改其图片和常规设置，并选择默认项目。 |  |
| `org.delete` | 删除组织 | 计划删除组织，或在等待期间将其恢复。 | 敏感；需近期登录 |
| `org.ownership.transfer` | 转让所有权 | 让另一位成员代替您成为所有者。 | 敏感；需近期登录；需要 `members.view` |

### 成员与访问

| 权限 | 名称 | 允许的操作 | 备注 |
| --- | --- | --- | --- |
| `members.view` | 查看成员 | 查看成员及其角色和权限。 |  |
| `members.invite` | 邀请成员 | 通过邮件邀请他人，并授予您自己可授予的角色。 | 需要 `members.view` |
| `members.invites.revoke` | 取消邀请 | 取消待处理的邀请。 | 需要 `members.view` |
| `members.remove` | 移除成员 | 将成员移出组织，并归档其在组织中的密钥。 | 敏感；需要 `members.view` |
| `members.rights.update` | 更改成员角色 | 更改成员的组织角色和单项组织权限。 | 需要 `members.view` |
| `roles.manage` | 管理角色 | 创建、编辑和删除组织的自定义角色。 |  |

### 项目

| 权限 | 名称 | 允许的操作 | 备注 |
| --- | --- | --- | --- |
| `projects.view.all` | 查看所有项目 | 查看所有项目，包括该成员未加入的项目。 |  |
| `projects.create` | 创建项目 | 创建项目，创建者成为项目管理员。 |  |
| `projects.archive.any` | 归档任意项目 | 归档任意项目及其所有密钥。 | 敏感；需要 `projects.view.all` |

### 服务商

| 权限 | 名称 | 允许的操作 | 备注 |
| --- | --- | --- | --- |
| `provider_credentials.view` | 查看服务商密钥 | 查看组织的服务商密钥（仅元数据，绝不含密钥内容）。 |  |
| `provider_credentials.create` | 添加服务商密钥 | 为组织添加服务商密钥。 | 需近期登录；需要 `provider_credentials.view` |
| `provider_credentials.replace` | 更改服务商密钥 | 替换和重命名服务商密钥，选择各服务商的默认密钥，并将密钥分配给项目。 | 需近期登录；需要 `provider_credentials.view` |
| `provider_credentials.delete` | 删除服务商密钥 | 从组织中移除服务商密钥。 | 敏感；需要 `provider_credentials.view` |

### 安全

| 权限 | 名称 | 允许的操作 | 备注 |
| --- | --- | --- | --- |
| `security.sso.manage` | 管理 SSO | 配置组织的身份提供商，以及经由它加入的人员的角色。 | 需近期登录 |
| `security.sso.require` | 要求 SSO | 要求成员通过 SSO 登录此组织。 | 敏感；需近期登录 |
| `security.2fa.require` | 要求双重验证 | 要求成员使用双重验证。 | 需近期登录 |
| `security.key_policy.manage` | 设置密钥有效期 | 设置组织的密钥最长有效期。 |  |

### 密钥

| 权限 | 名称 | 允许的操作 | 备注 |
| --- | --- | --- | --- |
| `platform_keys.view` | 查看平台密钥 | 查看组织的平台密钥（元数据）。 |  |
| `platform_keys.create` | 创建平台密钥 | 创建可管理组织级一切内容的组织平台密钥。 | 敏感；需近期登录；需要 `platform_keys.view` |
| `platform_keys.revoke` | 停用平台密钥 | 停用、启用或归档组织平台密钥。 | 需要 `platform_keys.view` |

### 日志与数据

| 权限 | 名称 | 允许的操作 | 备注 |
| --- | --- | --- | --- |
| `logs.settings.manage` | 管理日志设置 | 设置日志保留期，并为组织开启或关闭内容日志。 |  |
| `logs.storage.view` | 查看日志存储 | 查看日志条目的数量及其占用的空间。 |  |
| `logs.delete` | 删除日志 | 删除组织在某一时间范围内的日志。 | 敏感；需近期登录；需要 `logs.storage.view` |

### 用量与账单

| 权限 | 名称 | 允许的操作 | 备注 |
| --- | --- | --- | --- |
| `usage.org.view` | 查看组织用量 | 查看所有项目的用量。 |  |
| `costs.org.view` | 查看组织费用 | 查看所有项目的费用。 | 需要 `usage.org.view` |
| `billing.view` | 查看账单 | 查看套餐、发票和付款状态（Kaeso Cloud）。 | 仅限 Kaeso Cloud |
| `billing.manage` | 管理账单 | 更改套餐和付款方式（Kaeso Cloud）。 | 仅限 Kaeso Cloud；需要 `billing.view` |

### 审计

| 权限 | 名称 | 允许的操作 | 备注 |
| --- | --- | --- | --- |
| `audit_log.view` | 查看审计日志 | 查看组织的审计日志。 |  |
| `audit_log.export` | 导出审计日志 | 导出组织的审计日志。 | 需要 `audit_log.view` |

## 项目权限

### 项目

| 权限 | 名称 | 允许的操作 | 备注 |
| --- | --- | --- | --- |
| `project.view` | 查看项目 | 查看项目及其设置。 |  |
| `project.update` | 编辑项目 | 重命名项目，并更改其设置和图片。 |  |
| `project.archive` | 归档项目 | 归档此项目及其所有密钥。 | 敏感 |

### 成员与访问

| 权限 | 名称 | 允许的操作 | 备注 |
| --- | --- | --- | --- |
| `project.members.view` | 查看成员 | 查看项目成员及其角色和权限。 |  |
| `project.members.add` | 添加成员 | 将组织成员添加到项目，并授予您自己可授予的角色。 | 需要 `project.members.view` |
| `project.members.remove` | 移除成员 | 将成员移出项目，并归档其在项目中的密钥。 | 需要 `project.members.view` |
| `project.members.rights.update` | 更改成员角色 | 更改成员的项目角色和单项项目权限。 | 需要 `project.members.view` |
| `service_accounts.view` | 查看服务账号 | 查看项目的服务账号及其权限。 |  |
| `service_accounts.create` | 创建服务账号 | 创建服务账号，并授予您自己持有的权限。 | 需要 `service_accounts.view` |
| `service_accounts.rights.update` | 更改服务账号 | 重命名服务账号，并更改其角色和权限。 | 需要 `service_accounts.view` |
| `service_accounts.archive` | 归档服务账号 | 归档服务账号及其密钥。 | 需要 `service_accounts.view` |

### 模型调用

| 权限 | 名称 | 允许的操作 | 备注 |
| --- | --- | --- | --- |
| `api.request` | 使用密钥调用模型 | 使用自己的 API 密钥调用项目的模型。 |  |
| `playground.use` | 使用试验场 | 在控制台试验场中调用项目的模型。 |  |

### 密钥

| 权限 | 名称 | 允许的操作 | 备注 |
| --- | --- | --- | --- |
| `keys.own.create` | 创建自己的密钥 | 为自己创建 API 密钥。 |  |
| `keys.own.manage` | 管理自己的密钥 | 停用、启用或归档自己的 API 密钥。 |  |
| `keys.all.view` | 查看所有密钥 | 查看项目中的所有密钥（元数据）。 |  |
| `keys.all.manage` | 管理所有密钥 | 停用、启用或归档项目中任何人的密钥。 | 需要 `keys.all.view` |
| `service_accounts.keys.create` | 创建服务账号密钥 | 为项目的服务账号创建密钥。 | 需要 `service_accounts.view` |
| `keys.platform_access.grant` | 授予密钥平台访问权限 | 授予新密钥访问 Platform API 的权限。 | 敏感；需近期登录 |
| `security.key_policy.project.manage` | 设置密钥有效期 | 设置项目的密钥最长有效期。 |  |

### 模型

| 权限 | 名称 | 允许的操作 | 备注 |
| --- | --- | --- | --- |
| `models.view` | 查看模型 | 查看项目的模型、别名和路由。 |  |
| `models.manage` | 管理模型 | 添加、更改和移除模型及别名。 | 需要 `models.view` |

### 限额与预算

| 权限 | 名称 | 允许的操作 | 备注 |
| --- | --- | --- | --- |
| `limits.view` | 查看限额 | 查看项目、成员和密钥的限额、预算及模型访问权限。 |  |
| `limits.manage` | 管理限额 | 更改项目、成员和密钥的限额、预算及模型访问权限。 | 需要 `limits.view` |

### 日志与数据

| 权限 | 名称 | 允许的操作 | 备注 |
| --- | --- | --- | --- |
| `logs.metadata.view` | 查看请求日志 | 查看请求日志：模型、Token、费用、延迟、状态和密钥。 |  |
| `logs.content.view` | 查看提示词和输出 | 查看日志中的请求和响应正文。 | 敏感；需要 `logs.metadata.view` |
| `logs.export` | 导出日志 | 导出项目的请求日志。 | 敏感；需要 `logs.metadata.view` |
| `logs.content.disable` | 关闭内容日志 | 为此项目关闭内容日志。 |  |

### 用量与费用

| 权限 | 名称 | 允许的操作 | 备注 |
| --- | --- | --- | --- |
| `usage.view` | 查看用量 | 查看项目的用量。 |  |
| `costs.view` | 查看费用 | 查看项目的费用。 | 需要 `usage.view` |

### 审计

| 权限 | 名称 | 允许的操作 | 备注 |
| --- | --- | --- | --- |
| `project.audit_log.view` | 查看项目审计日志 | 查看此项目中变更的审计日志。 |  |
