---
title: 本地环境搭建
description: 从全新克隆的代码开始，一步步得到带有示例数据、测试全部通过的运行中的安装。
order: 1
---

本页在本机上搭建所需的一切：从源代码构建的网关及其数据库、控制台、示例数据以及测试。所有命令都在仓库根目录下运行。

## 所需工具

| 工具 | 版本 | 用途 |
| --- | --- | --- |
| [Git](https://git-scm.com) | 任意较新版本 | 获取代码 |
| [Node.js](https://nodejs.org) | 24，与 `.node-version` 一致 | 控制台、UI 库、文档和脚本 |
| pnpm | 11，固定在 `package.json` 中，由 Corepack 安装 | TypeScript 工作区 |
| 通过 rustup 安装的 [Rust](https://rustup.rs) | 1.99，固定在 `rust-toolchain.toml` 中，首次使用时自动安装 | 网关和 KPP |
| 带 Compose 的 [Docker](https://docs.docker.com/get-docker/) | Docker Compose 2.24 或更高版本 | 开发环境栈 |
| [Python](https://www.python.org) | 3.13 | 仅用于[端到端测试](testing.md#端到端测试) |

Corepack 随 Node.js 一起提供。只需启用一次，它就会提供仓库固定的 pnpm 版本：

```bash
corepack enable
```

第一次在仓库中运行 `cargo` 时，rustup 会读取 `rust-toolchain.toml`，安装 Rust 1.99 以及 rustfmt、clippy 和 llvm-tools。也可以提前运行 `rustup show` 完成安装。

## 1. 获取代码

```bash
git clone https://github.com/logicplanes/kaeso
cd kaeso
```

## 2. 安装依赖

```bash
pnpm install
```

这会安装 TypeScript 工作区中的所有包：控制台、文档、展示站点以及 `packages/` 中的库。Cargo 会在首次构建时自行获取 Rust 依赖。

## 3. 启动开发环境栈

```bash
pnpm stack:up
```

这会从检出的代码构建网关镜像（`kaeso-dev-gateway`，来自 `deploy/gateway.Dockerfile`），并在 Docker 中将其与 PostgreSQL、ClickHouse、Valkey、Mailpit、一个模拟模型服务商和一个模拟身份提供方一起启动，然后等待所有服务都进入健康状态。首次构建需要编译网关，耗时几分钟；之后的构建会复用 Docker 的缓存。

| 服务 | 说明 | 本机地址 |
| --- | --- | --- |
| `gateway` | 从检出的代码构建的网关 | `127.0.0.1:18080` |
| `postgres`、`clickhouse`、`valkey` | 网关的数据库，Rust 测试也会使用 | `127.0.0.1:55432`、`127.0.0.1:58123`、`127.0.0.1:56379` |
| `mail` | Mailpit，接收登录链接、邀请和密码重置邮件 | `http://127.0.0.1:58025`，SMTP 在 `127.0.0.1:51025` |
| `provider` | 模拟的 OpenAI 和 Anthropic API，响应每个请求而不调用真实服务商 | 仅在栈内部 |
| `idp` | 用于单点登录的模拟 OpenID Connect 提供方 | `http://idp.localhost:18091` |
| `master-key` | 在网关启动之前将网关的主密钥写入一个卷 | |

该栈是 Compose 项目 `kaeso-dev`，定义在 `deploy/compose.dev.yaml` 中。第二个克隆可以在旁边运行自己的开发环境栈，见下文“运行第二个开发环境栈”。所有模型请求都发往模拟服务商，因此不需要真实的服务商密钥。

## 4. 填充示例数据

```bash
pnpm seed
```

这会像真人操作一样通过 Platform API 设置安装并填充数据：组织 Northwind Labs，包含五个项目、六名成员、密钥、模型、别名和限额；几百个经由网关发往模拟服务商的请求；以及 70 天的历史请求记录。

如果想手动设置安装，请跳过这一步：只要安装中还没有用户，`pnpm stack:up` 就会输出一个设置链接（`http://localhost:3000/setup#token=…`）。

## 5. 打开控制台

```bash
pnpm dev
```

这会构建 `packages/` 中的库，启动开发环境栈（网关源代码有变化时会重新构建网关），并在 http://localhost:3000 提供控制台。保存文件时页面会自动重新加载。

以 `owner@example.com`（实例管理员兼组织所有者）或 `lin@example.com`（开发者）登录。两者的密码都是 `correct horse battery staple`。网关发送的邮件会出现在 Mailpit 中：[http://127.0.0.1:58025](http://127.0.0.1:58025)。

```text
browser ─▶ http://localhost:3000 (Vite) ─▶ /platform, /v1 sent on to the gateway on 127.0.0.1:18080
                                         └▶ everything else: the console
gateway ─▶ provider (mock OpenAI and Anthropic), mail (Mailpit), idp (mock single sign-on)
```

网关的公开 URL 就是开发服务器的地址，因此会话 Cookie 是第一方 Cookie，浏览器发送的 `Origin` 也正是网关所期望的。生产环境以同样的方式在同一个域名下、通过同一个反向代理提供控制台和网关。

## 6. 运行测试

TypeScript 测试：

```bash
pnpm test
```

Rust 测试，使用开发环境栈的数据库：

```bash
cargo test --workspace --all-features
```

每个 Rust 测试都会创建自己的数据库，因此测试可以并行运行，也不会改动示例数据。[测试](testing.md)列出了所有测试套件、端到端测试以及变更必须通过的检查。

## 停止开发环境栈

```bash
pnpm stack:down
```

这会移除开发环境栈及其数据，包括 Rust 测试的数据库；下一次 `pnpm stack:up` 将从空状态开始。

## 更改端口和镜像

开发环境栈的端口来自 `apps/console/scripts/stack.mjs`。以下变量可以更改它们：`KAESO_GATEWAY_PORT`、`KAESO_POSTGRES_PORT`、`KAESO_CLICKHOUSE_PORT`、`KAESO_VALKEY_PORT`、`KAESO_MAIL_PORT`、`KAESO_MAIL_SMTP_PORT`、`KAESO_IDP_PORT` 和 `KAESO_PUBLIC_URL`。如果数据库在别处，Rust 测试会通过 `KAESO_TEST_POSTGRES_URL`、`KAESO_TEST_CLICKHOUSE_URL` 和 `KAESO_TEST_VALKEY_URL` 找到它们。

要让开发环境栈使用在别处构建的网关镜像（例如某个正式版本），指定该镜像即可；开发环境栈随后会直接使用它，而不再构建：

```bash
KAESO_GATEWAY_IMAGE=ghcr.io/logicplanes/kaeso-gateway:{{version}} pnpm stack:up
```

## 运行第二个开发环境栈

第二个克隆只要使用另一个项目名称和另一组端口，就会拥有自己的开发环境栈。`KAESO_STACK_PROJECT` 指定 Compose 项目的名称（默认为 `kaeso-dev`），网关镜像也以它命名；`pnpm stack:up`、`pnpm seed` 和 `pnpm stack:down` 随后只作用于该项目。在第二个克隆中：

```bash
export KAESO_STACK_PROJECT=kaeso-two KAESO_PUBLIC_URL=http://localhost:3001
export KAESO_GATEWAY_PORT=18180 KAESO_IDP_PORT=18191 KAESO_MAIL_PORT=58125 KAESO_MAIL_SMTP_PORT=51125
export KAESO_POSTGRES_PORT=55532 KAESO_CLICKHOUSE_PORT=58223 KAESO_VALKEY_PORT=56479
pnpm dev --port 3001
```

在同一个 shell 中运行 `pnpm seed` 即可填充该开发环境栈。要让 Rust 测试使用它的数据库，请设置上文的 `KAESO_TEST_…` 变量。

## 从源代码运行网关

如果需要一个几秒钟就能重启、可以挂调试器或输出额外日志的网关，可以用 Cargo 针对开发环境栈的数据库运行它。先在第二个终端中启动模拟服务商，这样不会有请求到达真实的服务商：

```bash
cargo run -p kaeso-testkit -- mock-provider 127.0.0.1:18090
```

在第一个终端中创建新的数据库并导出它们的设置，将两个服务商都指向模拟服务商，再加上公开 URL、主密钥以及只写入日志的邮件发送方式，然后启动网关：

```bash
export $(cargo run -q -p kaeso-testkit -- fresh-services)
export KAESO_PROVIDERS_OPENAI_BASE_URL=http://127.0.0.1:18090
export KAESO_PROVIDERS_ANTHROPIC_BASE_URL=http://127.0.0.1:18090
export KAESO_PUBLIC_URL=http://localhost:3000
export KAESO_LISTEN=127.0.0.1:8080
export KAESO_MASTER_KEYS="$(cargo run -q -p kaeso -- master-key generate)"
export KAESO_MAIL_TRANSPORT=log
cargo run -p kaeso -- serve
```

在第三个终端中让控制台连接到这个网关：

```bash
KAESO_GATEWAY_URL=http://127.0.0.1:8080 pnpm --filter kaeso-console dev
```

然后打开网关日志中的设置链接。如果要改为调用真实的服务商，去掉两行 `KAESO_PROVIDERS_…` 即可。这些数据库会一直保留，直到在第一个终端中将其删除：

```bash
cargo run -q -p kaeso-testkit -- drop-services "$KAESO_CLICKHOUSE_DATABASE"
```

所有设置都列在[配置](../self-hosting/configuration.md)中。

## 生成的代码

有两样东西是生成的，而不是手写的：

- **`openapi.json`**，即仓库根目录下的 Platform API 文档，由网关的代码生成。修改 Platform API 之后，重新生成它并与变更一起提交：

```bash
cargo run -q -p kaeso -- openapi > openapi.json
```

- **控制台的 API 客户端和路由树。** `pnpm --filter kaeso-console generate` 根据 `openapi.json` 生成客户端的类型，根据 `apps/console/src/routes` 生成路由树。控制台的 `dev`、`build`、`typecheck`、`test` 和 `e2e` 脚本会先运行它，两者的输出都不提交。

因此控制台和网关会一起变更：对 Platform API 的修改会在同一个拉取请求中更新 `openapi.json` 和控制台。

## 文档站点

```bash
pnpm build:packages
pnpm --filter kaeso-docs dev
```

站点运行在 [http://localhost:4480](http://localhost:4480)，在 `apps/docs/content` 中保存页面时会自动重新加载。`pnpm --filter kaeso-docs check:content` 无需启动站点即可检查每个页面、链接和翻译。其余内容见[编写文档](documentation.md)。

## UI 库

```bash
pnpm build:packages
pnpm --filter kaeso-showcase dev
```

包含所有组件的展示站点运行在 [http://localhost:4321](http://localhost:4321)，修改库的源代码时会自动重新加载。控制台和文档读取的则是库的构建产物：修改 `packages/ui` 或 `packages/tokens` 之后，运行 `pnpm build:packages` 并重启它们的开发服务器。
