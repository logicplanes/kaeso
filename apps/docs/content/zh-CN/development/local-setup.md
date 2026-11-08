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

## 一键设置

一条命令即可检查仓库所需的一切，安装依赖，构建 UI 库，启动开发栈并填入示例数据：

```bash
pnpm bootstrap
```

如果缺少某个前置条件或版本过旧、Docker 没有响应，或者其他程序占用了开发栈需要的端口，它会在做任何更改之前停下，并说明如何解决，例如运行 `corepack enable`，或设置用于更换端口的变量。您可以随时再次运行它：已设置好的实例会保留其数据。

首次运行会从您的检出代码构建网关，需要几分钟；之后的运行会复用 Docker 的缓存。示例数据是组织 Northwind Labs，包含五个项目、六名成员、十个密钥、模型、别名和限额。数千封客服邮件和 Slack 问题会经过网关，由模拟服务商写出回复。随后是 70 天的请求历史：每个应用和每个人都有自己的模型、客户端和时段，项目保存内容的地方带有内容，还有响应缓存命中、服务商过载时的回退，以及促销期间触及的限额。

| 选项 | 作用 |
| --- | --- |
| `--check` | 只检查前置条件、可用磁盘空间和端口 |
| `--all` | 还会安装[完整测试套件](testing.md)所需的浏览器、Cargo 工具和 Python 包 |
| `--no-seed` | 启动开发栈但不填入示例数据；打开它输出的设置链接，手动设置 Kaeso |

设置 `KAESO_SEED_WHOLE_DAYS=1` 后，历史记录截止到昨天，而不是十分钟前，适用于会把日期向前移动的演示。`pnpm stack:up` 和 `pnpm seed` 可分别单独启动开发栈和填入示例数据（[开发栈](stack.md)）。

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
