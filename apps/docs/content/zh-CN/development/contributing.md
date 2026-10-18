---
title: 参与贡献
description: 报告问题、提出变更，并让拉取请求顺利合并。
order: 2
---

在 [Discussions](https://github.com/logicplanes/kaeso/discussions) 中提问和分享想法，把缺陷和具体的功能请求提交为 [issue](https://github.com/logicplanes/kaeso/issues)。比小修复更大的改动，先开一个 issue，在写代码之前与维护者就方案达成一致。所有参与者都遵守[行为准则](https://github.com/logicplanes/kaeso/blob/dev/CODE_OF_CONDUCT.md)。

## 发起讨论

在做出任何决定之前，先在 Discussions 中讨论想法。**Ideas** 用于希望拥有的功能，**Q&A** 用于安装、运行或使用 Kaeso 的问题，**Show and tell** 用于展示自己的作品。维护者认可的想法会成为功能请求。

## 提交 issue

选择合适的模板：

| 模板 | 用于 |
| --- | --- |
| Bug report | Kaeso 做了不该做的事，或没有做该做的事。模板会询问版本、所属领域、发生了什么、最小复现以及 Kaeso 的运行方式。对于界面上看到的问题，还会询问语言、主题、宽度和浏览器 |
| Feature request | 新功能、对现有功能的修改，或文档应当说明的内容。涉及线路字段时，附上服务商的文档或 OpenAPI 模式链接 |

粘贴任何内容之前，先删除其中的 API 密钥、服务商密钥、会话 Cookie 和个人数据。

> [!IMPORTANT]
> 通过仓库的[安全公告](https://github.com/logicplanes/kaeso/security/advisories/new)私下报告安全问题，绝不要提交公开 issue。[`SECURITY.md`](https://github.com/logicplanes/kaeso/blob/dev/SECURITY.md) 说明了报告应包含的内容以及后续流程。

## 进行变更

1. 从默认分支 `dev` 创建分支。
2. 先写一个失败的测试。修复缺陷时，先用测试重现这个缺陷。
3. 修改代码，直到测试通过。
4. 在同一个拉取请求中修改变更涉及的其他所有内容。Platform API 的变更要同时更新网关、控制台和 `openapi.json`。KPP 的变更从一个 [fixture](kpp/fixtures.md) 开始，并更新该方言的映射页面。
5. 当使用、运行或集成 Kaeso 的人会注意到这个变更时，用英语、德语和简体中文更新文档（[编写文档](documentation.md)）。
6. 运行[完成变更之前](testing.md#完成变更之前)列出的检查。在浅色和深色主题、桌面和手机宽度以及三种语言下查看每一个改动过的界面。
7. 向 `dev` 提交一个只包含这一项变更的拉取请求。模板会询问改了什么、为什么改、如何测试，并附有检查清单。

修改 Platform API 之后，重新生成 `openapi.json`，并与变更一起提交：

```bash
cargo run -q -p kaeso -- openapi > openapi.json
```

修改权限、角色或设置会改变某个[参考快照](testing.md#快照与参考文档)。审阅其差异，并更新它对应的页面。

## 撰写标题

标题会成为这个拉取请求在变更日志中的那一行，所以要写成一句话，说明对使用、运行 Kaeso 或基于它构建的人有什么变化。例如：“Keep dialogs centered and resize them smoothly when their content changes”。不要修改 `CHANGELOG.md`：Release 工作流会根据已合并的拉取请求生成它。

**Labels** 工作流会根据改动的文件设置领域标签（例如 `area: console`）和 `internal`。当标题的第一个词选不中正确的分节时，维护者会设置类型标签，例如 `fixed`（[标签](releasing.md#标签)）。

## 合并

必需的 **CI** 检查通过后，拉取请求即可合并（[持续集成](testing.md#持续集成)）。它会作为一个压缩后的提交合并，提交主题就是标题，所以每条提交信息都要说明改了什么、为什么改。

提交贡献即表示同意以 [MIT 许可证](https://github.com/logicplanes/kaeso/blob/dev/LICENSE)授权这些贡献。

## 堆叠相互依赖的拉取请求

当一项变更依赖另一项尚未合并的变更时，把它们作为一个堆栈提交，而不是合成一个大的拉取请求。先安装一次 `gh stack` 命令：

```bash
gh extension install github/gh-stack
```

在 `dev` 上开始一个堆栈，为每项变更在下方分支之上添加一个分支，然后提交拉取请求：

```bash
gh stack init
gh stack add store-tokens
gh stack submit
```

`gh stack submit` 会推送每个分支，并为每个分支创建一个以下方分支为基础的拉取请求。下方的分支变化后，`gh stack rebase` 会更新其余分支。拉取请求从底部开始依次审阅和合并，每个都要有自己的绿色 CI。

## 遵守仓库规则

每项变更都遵守以下规则，其中大多数由 CI 检查：

- **任何地方都不写代码注释。** 名称、类型、小函数和测试解释代码，其余内容由本文档解释。这也包括配置文件中的 `#`、Markdown 中的 `<!-- -->` 以及页面代码块中的注释。**No comments** 任务会强制执行这一点。
- **测试断言的是人或应用看到的内容。** 测试是确定性的：注入时钟，不使用 sleep。覆盖率阈值只升不降。
- **界面只使用 `@logicplanes/kaeso-ui`**，`className` 只用于布局，每个词都有三种语言（[翻译](translations.md)）。
- **JavaScript 只用 pnpm**，一次性工具用 `pnpm dlx`。Cargo 构建 Rust 代码，锁文件只通过各自的工具修改。
- **仓库中不放机密信息或个人数据。** 使用 `kaeso.example.com`、`sk-kso-…` 和 `owner@example.com` 这样的占位符。
- **仓库中不放计划。** 路线图、待办事项和未决问题都放到 issue 中。

网关有自己的附加规则，例如不允许 panic、金额不使用浮点数（[网关规则](architecture/gateway.md#规则)）。KPP 也是如此（[设计原则](kpp/index.md#设计原则)）。

仓库根目录以及 `crates`、`packages`、`apps/console`、`apps/docs` 和 `apps/showcase` 各有一个 `AGENTS.md`，概括该部分的规则和检查，供编码代理和人阅读。

## 添加依赖

用 `pnpm --filter <package> add --save-exact <name>` 以精确版本添加 JavaScript 依赖，用 `cargo add` 添加 Rust 依赖。在拉取请求的描述中用一行说明为什么需要它。

Kaeso 采用 MIT 许可证，它发布的所有内容都只依赖宽松许可证的代码。GPL、LGPL 和 AGPL 代码一律不允许。两项检查负责执行这一政策：

| 发布的内容 | 允许的许可证 | 检查方式 |
| --- | --- | --- |
| 网关和更新器 | `deny.toml` 中的列表：MIT、MIT-0、Apache-2.0、BSD-2-Clause、BSD-3-Clause、ISC、Unicode 和 Zlib | `cargo deny check` |
| `kaeso-kpp`、`kaeso-codecs` | 同一列表，没有例外 | `cargo deny --manifest-path crates/kaeso-codecs/Cargo.toml check` |
| UI 库、控制台和文档 | MIT、ISC、BSD-2-Clause、BSD-3-Clause、Apache-2.0 和 0BSD | `pnpm build` 之后运行 `pnpm licenses:check` |

`cargo deny check` 还会拒绝有安全公告的 crate、已撤回的 crate、通配符版本，以及来自未知注册表或 Git 仓库的 crate。少数只通过其他 crate 间接进入网关的 crate 对另一种宽松许可证有例外，`deny.toml` 逐一列出了它们。只在测试或构建中运行的工具（例如 axe-core）永远不会进入软件包、镜像或发布的网站。UI 库的字体 Inter 和 Geist Mono 采用 SIL Open Font License，缺少其许可证文件的软件包会被 `pnpm release:check` 拒绝。

KPP 的库代码只能依赖 `serde`、`serde_json`、`bytes`、`thiserror`、`indexmap`、`base64` 和 `mime`，其政策禁止 `tokio`、`reqwest`、`hyper` 和 `axum`。其他任何依赖都要先征得维护者同意。

## 下一步

- [本地环境搭建](local-setup.md)：让本地检出带着示例数据运行起来。
- [测试](testing.md)：找到覆盖本次变更的测试套件。
- [编写文档](documentation.md)：更新变更涉及的页面。
- [翻译](translations.md)：为新文本补充德语和中文。
