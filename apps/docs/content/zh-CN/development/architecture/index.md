---
title: 架构
description: 网关、KPP、控制台、UI 库和文档如何在同一个仓库中协同工作。
order: 2
---

```text
                     crates/kaeso-kpp, crates/kaeso-codecs
                                    │
apps ─▶ /v1 ─────────▶ gateway (crates/kaeso, crates/kaeso-*) ─▶ OpenAI, Anthropic
                         │    │
console ─▶ /platform/v1 ─┘    └─▶ openapi.json ─▶ the console's API client
   ▲
packages/ui, packages/tokens ─▶ apps/console, apps/docs, apps/showcase
```

- **网关的每个模型请求都使用 KPP。** KPP 将应用的格式解码为其规范格式，再编码为服务商的格式，回程亦然。网关从不自行解析或构建服务商的 JSON，并且它在 Cargo 工作区内通过路径依赖 KPP，因此同时涉及两者的修改只需一个拉取请求。
- **控制台只使用 Platform API**，不使用其他任何接口，因此控制台能做的一切，脚本也能做。它的 API 客户端由 `openapi.json` 生成，而该文件由网关根据自身代码生成，因此两者同步变化。
- **控制台、文档和展示站点都用 UI 库绘制。** 每个可见元素都来自基于 `@logicplanes/kaeso-tokens` 构建的 `@logicplanes/kaeso-ui`，两者都从工作区引用（`workspace:*`）。网关发送的邮件使用相同的设计令牌取值。
- **统一版本。** 各部分共享发布的版本号，发布中的 `compose.yaml` 固定使用该发布的网关和控制台镜像。

## 页面

| 页面 | 内容 |
| --- | --- |
| [网关](gateway.md) | 请求路径、各个 crate、配置，以及网关代码遵循的规则 |
| [控制台](console.md) | Web 控制台中的数据、样式、打包和错误 |
| [文档站点](docs-site.md) | Markdown 页面如何变成 docs.kaeso.app |
| [邮件](emails.md) | 网关如何构建它发送的邮件 |
| [KPP 内部实现](../kpp/index.md) | 规范格式和编解码器 |

## Rust 工作区

| Crate | 职责 |
| --- | --- |
| `kaeso` | 网关的二进制程序：命令行（`serve`、`migrate`、`master-key`、`user`、`setup-link`、`healthcheck`、`openapi`）、启动和关闭。唯一可以使用 `anyhow` 的 crate。 |
| `kaeso-server` | HTTP 接口：`/v1`、`/platform/v1`、健康检查和指标的路由；提取器、错误映射、OpenAPI 文档、CSRF 检查、请求 ID、请求体大小限制；两个 API 背后的服务，其中包括以 Valkey Lua 脚本实现的限额计数器；配置、后台任务和遥测。 |
| `kaeso-core` | 不含 I/O 的领域规则：ID、权限和角色、密钥格式、模型解析、目录、能力检查、限额、费用、流式回复组装器。测试无需数据库。其 `data/` 中存放随每个发布附带的 models.dev 快照。 |
| `kaeso-store` | 存储：通过 sqlx 访问 PostgreSQL（离线查询数据位于仓库根目录的 `.sqlx/`）、ClickHouse，以及 Valkey 连接及其缓存失效通知；PostgreSQL 和 ClickHouse 的迁移，在锁保护下运行。 |
| `kaeso-upstream` | 服务商传输：HTTP 客户端、认证头、基础 URL、超时、流式请求体和取消。 |
| `kaeso-crypto` | 信封加密（AES-256-GCM）、主密钥环、密码、TOTP 以及机密的包装类型。 |
| `kaeso-mail` | 邮件：SMTP 投递，以及采用同一布局、提供三种语言的所有邮件（[邮件](emails.md)）。 |
| `kaeso-testkit` | 仅用于测试：开发和测试数据库、由 KPP 的 fixture 构建的模拟服务商、模拟身份提供方。 |
| `kaeso-kpp`、`kaeso-codecs` | KPP 的类型和编解码器，发布到 crates.io（[KPP 内部实现](../kpp/index.md)）。 |
| `kpp-conformance` | 一个服务器，为[一致性测试套件](../testing.md#sdk-一致性)向官方 Python SDK 提供 KPP 的编码器。 |
| `xtask` | 仓库任务：`cargo xtask models-dev` 刷新 models.dev 快照，`cargo xtask record` [录制服务商的响应](../kpp/fixtures.md#录制真实响应)。 |

所有 crate 共享根目录 `Cargo.toml` 中工作区的版本、edition 和 lint 设置。只有 `kaeso-kpp` 和 `kaeso-codecs` 会发布；其他 crate 都设置了 `publish = false`。

## TypeScript 工作区

| 包 | 文件夹 | 职责 |
| --- | --- | --- |
| `kaeso-console` | `apps/console` | Web 控制台，一个与网关并列提供服务的静态 React 应用（[控制台](console.md)） |
| `kaeso-docs` | `apps/docs` | 本文档（[文档站点](docs-site.md)） |
| `kaeso-showcase` | `apps/showcase` | UI 库每个组件的每种状态及其指南；通过 `pnpm --filter kaeso-showcase dev` 在本地运行 |
| `@logicplanes/kaeso-ui` | `packages/ui` | 组件库：基于 Base UI 的 React 组件，用 Tailwind CSS 和设计令牌设置样式 |
| `@logicplanes/kaeso-tokens` | `packages/tokens` | 设计令牌，形式包括 CSS 变量、Tailwind 主题、TypeScript 常量和 JSON |

各应用读取的是这些库的构建产物，因此 `pnpm build:packages` 要在任何依赖它们的步骤之前运行。UI 库为使用它的人和工具附带了自己的规则和组件注册表（`packages/ui/AGENTS.md`、`packages/ui/foundations/`、`packages/ui/ai/`）；它的展示站点记录了每个组件。

## 生产环境

```text
browser, apps ─▶ https://kaeso.example.com ─▶ Caddy ─▶ /v1/*, /platform/* ─▶ gateway
                                                   └▶ everything else ─▶ console (static files)
gateway ─▶ PostgreSQL (state), ClickHouse (request logs), Valkey (counters, caches)
```

控制台是一组静态文件，由它自己的小型容器提供服务，与网关位于同一域名下，因此会话 Cookie 是第一方 Cookie，也不涉及 CORS。安装方法见[自托管](../../self-hosting/index.md)。
