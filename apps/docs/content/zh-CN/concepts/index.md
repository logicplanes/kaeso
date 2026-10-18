---
title: 概念
description: 本文档使用的术语，以及 Kaeso 如何处理请求、格式和权限。
order: 2
---

[请求流程](request-flow.md)跟随一个模型请求走完网关，[格式转换](format-translation.md)说明 KPP 在 API 格式之间传递什么，[权限](rights.md)列出每个角色可以做什么。所有页面都按同一含义使用下面这些术语。

## 术语

| 术语 | 含义 |
| --- | --- |
| 安装 | 一套 Kaeso 部署：网关、控制台及其数据库，使用同一个域名（[自托管](../self-hosting/index.md)） |
| 实例管理员 | 负责运行这套安装的用户，管理其用户、组织和设置；在组织内没有任何权限（[实例管理](../console/instance-administration.md)） |
| 组织 | 包含成员、服务商密钥、平台密钥和项目（[组织和项目](../console/organizations-and-projects.md)） |
| 项目 | 包含模型目录、API 密钥、服务账号、限额和请求日志。每个组织都有一个默认项目 |
| 成员 | 组织中的一个人，拥有一个组织角色，并在其所在的每个项目中拥有一个项目角色 |
| 服务账号 | 供软件使用的项目成员，例如部署或任务；它的密钥不受创建者离开的影响 |
| 角色、权限 | 一项权限允许一类操作，例如 `keys.all.view`；角色是一组有名称的权限（[权限](rights.md)） |
| API 密钥 | `sk-kso-…`：属于某个项目中一名成员或一个服务账号的密钥。有推理访问权限时，它通过模型 API 调用模型；有平台访问权限时，它通过 Platform API 管理该项目（[API 密钥](../console/api-keys.md)） |
| 平台密钥 | `sk-kso-org-…`：通过 Platform API 管理整个组织的密钥，从不调用模型 |
| 服务商密钥 | 组织的 OpenAI 或 Anthropic 密钥，Kaeso 用它调用服务商；应用永远看不到它（[服务商密钥](../console/provider-keys.md)） |
| 主密钥 | 安装自己的密钥，用于加密它存储的机密信息，例如服务商密钥（[备份和主密钥](../self-hosting/backups.md)） |
| 目录 | 项目可以调用的模型及其价格、限制和能力：默认来自 models.dev，也可以在项目中为自定义模型或覆盖值设定（[模型和别名](../console/models-and-aliases.md)） |
| 别名 | 项目为其目录中的模型起的名称，例如 `fast` |
| 限额 | 针对项目、成员或密钥的请求速率或 Token 速率，或 Token 预算或费用预算；硬预算会拒绝请求，软预算只发出提醒（[限额和预算](../console/limits-and-budgets.md)） |
| 请求日志 | 一个模型请求的记录：状态、Token、费用、耗时，以及在保留时的内容（[请求日志](../console/request-logs.md)） |
| API 格式 | 请求及其回复的形式：OpenAI Chat Completions、Responses 或 Embeddings，或 Anthropic Messages |
| KPP | Kaeso Provider Protocol：网关把每种 API 格式转换成的、再从中转换出来的统一格式（[格式转换](format-translation.md)） |
| 模型 API、Platform API | 安装的两个 API：应用调用模型的 `/v1`，以及控制台和脚本用来管理安装的 `/platform/v1`（[API](../api/index.md)） |
