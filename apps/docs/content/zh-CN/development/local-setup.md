---
title: 本地环境搭建
description: 从全新克隆到带示例数据的控制台，并让测试全部通过。
order: 1
---

克隆 Kaeso，启动开发栈，填入示例数据，然后运行测试。每条命令都在仓库根目录运行。

## 开始之前

- [Git](https://git-scm.com)。
- [Node.js](https://nodejs.org) 24，即 `.node-version` 中的版本。
- [rustup](https://rustup.rs)。第一次在仓库中运行 `cargo` 时，它会安装 `rust-toolchain.toml` 中固定的 Rust 工具链。
- 带 Compose 2.24 或更新版本的 Docker 引擎：Linux 上用 Docker Engine，macOS 上用 [Colima](stack.md#在-macos-上用-colima-运行-docker)。
- [Python](https://www.python.org) 3.13，仅[端到端测试](testing.md#端到端测试)需要。

## 启用 Corepack

Corepack 随 Node.js 一起安装，提供仓库固定的确切 pnpm 版本：

```bash
corepack enable
```

此后 `pnpm --version` 会输出 `package.json` 中写明的版本。

## 获取代码

```bash
git clone https://github.com/logicplanes/kaeso
cd kaeso
```

## 安装依赖

安装控制台、文档、展示站点以及 `packages/` 中的库：

```bash
pnpm install
```

Cargo 会在第一次构建时获取 Rust crate。

## 启动开发栈

从当前检出构建网关，并与它的数据库和模拟服务一起启动：

```bash
pnpm stack:up
```

第一次构建要编译网关，需要几分钟；之后的构建会复用 Docker 的缓存。所有服务都健康后，命令会输出一个设置链接 `http://localhost:3000/setup#token=…`。

## 添加示例数据

通过 Platform API 完成实例设置，并填入示例数据：

```bash
pnpm seed
```

得到的是组织 Northwind Labs，包含五个项目、六名成员、密钥、模型、一个别名和限额。命令还会向模拟服务商发送几百个请求，并添加过去 70 天的请求历史。如果想手动完成设置，跳过这一步，打开设置链接即可。

## 打开控制台

构建各个库并启动控制台：

```bash
pnpm dev
```

控制台运行在 http://localhost:3000 上，保存后会自动重新加载。以组织所有者兼实例管理员 `owner@example.com` 登录，或以开发者 `lin@example.com` 登录。两者的密码都是 `correct horse battery staple`。邮件会出现在 http://127.0.0.1:58025 上的 Mailpit 中。

## 运行测试

运行 TypeScript 和 Rust 测试：

```bash
pnpm test
cargo test --workspace --all-features
```

两者都应全部通过。Rust 测试会在开发栈上创建各自的数据库，不会影响示例数据。

## 下一步

- [参与贡献](contributing.md)：进行变更，并让拉取请求顺利合并。
- [开发栈](stack.md)：更改端口、运行第二个栈，或从源码运行网关。
- [测试](testing.md)：运行变更必须通过的每一项检查。
- [架构](architecture/index.md)：熟悉网关、控制台和文档的结构。
