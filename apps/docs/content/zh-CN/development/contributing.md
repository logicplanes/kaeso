---
title: 参与贡献
description: 报告问题、提出变更，以及每个 Kaeso 变更都要遵守的规则。
order: 10
---

想法和问题请发到 [Discussions](https://github.com/logicplanes/kaeso/discussions)，缺陷和具体的功能请求请提交 [issue](https://github.com/logicplanes/kaeso/issues)。外部贡献者的拉取请求由维护者酌情审阅；对于比小修复更大的改动，请先开一个 issue，以便就方案达成一致。所有参与者都遵守[行为准则](https://github.com/logicplanes/kaeso/blob/dev/CODE_OF_CONDUCT.md)。

## Discussions 与 issue

Discussions 用于在做出决定之前先讨论想法：**Ideas** 用于提出希望拥有的功能，**Q&A** 用于询问安装、运行或使用 Kaeso 的问题，**Show and tell** 用于展示用 Kaeso 构建的作品。达成一致的想法会成为功能请求。

Issue 有两个模板：

| 模板 | 用于 |
| --- | --- |
| Bug report | Kaeso 做了不该做的事，或没有做该做的事。模板会询问版本、所属领域、发生了什么、最小复现以及 Kaeso 的运行方式；对于界面上看到的问题，还会询问语言、主题、宽度和浏览器 |
| Feature request | 新功能、对现有功能的修改，或文档应当说明的内容。涉及 wire 字段时，请附上服务商的文档或 OpenAPI 模式链接 |

粘贴任何内容之前，请先删除其中的 API 密钥、服务商密钥、会话 Cookie 和个人数据。

> [!IMPORTANT]
> 安全问题请通过仓库的[安全公告](https://github.com/logicplanes/kaeso/security/advisories/new)私下报告，切勿在公开 issue 中报告。[`SECURITY.md`](https://github.com/logicplanes/kaeso/blob/dev/SECURITY.md) 说明了报告应包含的内容以及后续流程。

## 提交变更

1. **从 `dev` 创建分支**，`dev` 是默认分支。`prod` 上是最新的版本，只有维护者会推送到它（[分支](releasing.md#分支)）。
2. **先写一个失败的测试，再写代码。** 每个缺陷修复都从一个能复现该缺陷的测试开始。
3. **在同一个拉取请求中修改变更涉及的所有内容。** Platform API 的变更要同时更新网关、控制台和 `openapi.json`。KPP 的变更从一个 [fixture](kpp/fixtures.md) 开始，并更新该方言的映射页面。新增的依赖要在[依赖与许可证](dependencies.md)中添加一行。对权限、角色或设置的修改会改变[参考快照](testing.md#快照与参考文档)；审阅其差异，并更新对应的页面。
4. **更新文档**：如果使用、运行或集成 Kaeso 的人会注意到这项变更，就用英语、德语和简体中文更新 `apps/docs/content` 中的页面（[编写文档](documentation.md)）。
5. **在 `CHANGELOG.md` 中添加一行**：每个会被人注意到的变更都要在 `[Unreleased]` 下添加（[变更日志](releasing.md#变更日志)）。
6. **运行检查**：[完成变更之前](testing.md#完成变更之前)中的所有检查，并在两种主题下、在桌面和手机宽度下、用全部三种语言查看每个修改过的界面。
7. **向 `dev` 发起拉取请求。** 每个拉取请求只包含一个变更。模板会询问变更内容和原因、测试方式，并附有上述步骤的检查清单。

### 变更如何进入 dev

每个变更都通过拉取请求进入 `dev`。必需检查 **CI** 通过后（[持续集成](testing.md#持续集成)）才能合并，合并时压缩为一个提交；合并提交和变基合并均已关闭。拉取请求的标题会成为该提交的主题行，因此标题和每条提交信息都要说明变更了什么以及原因。维护者可以绕过这些规则。

### 堆叠的拉取请求

如果一个变更依赖另一个尚未合并的变更，请用 GitHub 的堆叠拉取请求把它们作为一组提交，而不是合成一个大的拉取请求。`gh stack` 命令来自 `github/gh-stack` 扩展（`gh extension install github/gh-stack`）：

```bash
gh stack init
gh stack add store-tokens
gh stack submit
```

`gh stack init` 在 `dev` 上开始一个堆叠，`gh stack add` 在当前分支之上添加一个分支，`gh stack submit` 推送每个分支，并为每个分支创建一个以下方分支为基础的拉取请求。下方的分支变化后，`gh stack rebase` 会让整个堆叠保持最新。这些拉取请求从下往上依次审阅和合并，每个都需要自己的 CI 通过。

## 每个变更都要遵守的规则

- **任何地方都不写代码注释。** 不使用 `//`、`///`、`//!`、`/* */`、JSDoc 或 JSX 注释；SQL 中不使用 `--`；TOML、YAML、Python、shell、Dockerfile、`.dockerignore`、Caddyfile、`.env.example` 以及 ignore 和 attributes 文件中不使用 `#` 注释；HTML、Astro 和 Markdown 中不使用 `<!-- -->`；页面的代码块中也不包含注释。代码由命名、类型、小函数和测试来说明，其余内容由本文档说明。CI 作业 **No comments** 会强制执行这一点。
- **每个变更都有测试**，断言用户或应用看到的内容，并且结果确定：使用可注入的时钟，不使用 sleep。覆盖率阈值从不降低；覆盖率提高时，相应提高阈值。
- **界面只使用 `@logicplanes/kaeso-ui`**，`className` 只用于布局，并且每段文字都有三种语言的版本（[翻译](translations.md)）。
- **只使用与 MIT 兼容的依赖**，由 `cargo deny` 和 `pnpm licenses:check` 检查（[许可证政策](dependencies.md#许可证政策)）。
- **仓库中不包含密钥或个人数据**，只使用 `kaeso.example.com`、`sk-kso-…` 和 `owner@example.com` 这样的占位符。
- **仓库中不放计划。** 路线图、待办事项和待解决的问题放到 issue 中。
- **JavaScript 只用 pnpm**；一次性工具用 `pnpm dlx` 运行。用 `pnpm --filter <package> add --save-exact <name>` 以精确版本添加依赖。Rust 代码用 Cargo 构建。锁文件只通过其对应的工具修改。

网关有自己的规则，例如库代码中不得 panic、金额不使用浮点数、每个端点恰好对应一项权限（[网关](architecture/gateway.md#规则)）；KPP 也有（[设计原则](kpp/index.md#设计原则)）。

## AGENTS.md 文件

仓库根目录、`crates`、`packages`、`apps/console`、`apps/docs` 和 `apps/showcase` 各有一个 `AGENTS.md`，写明该部分的规则和检查，供编码智能体和开发者参考。它们是这些页面的摘要。`packages/ui/AGENTS.md` 写明用 UI 库构建应用时的规则，并随其 npm 包一起发布。

## 许可证

提交贡献即表示同意该贡献以 [MIT 许可证](https://github.com/logicplanes/kaeso/blob/dev/LICENSE)授权。
