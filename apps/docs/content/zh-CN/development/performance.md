---
title: 性能
description: 控制台、文档和 UI 库的打包预算、编解码器的基准测试，以及模糊测试的结果。
order: 8
---

性能靠测量，而不是靠猜测。页面超出预算时，控制台和文档的构建会失败；组件超出预算时，UI 库的构建会失败；KPP 的编解码器有基准测试和模糊测试目标。需要更大预算的变更要在同一个拉取请求中提高预算、说明原因，并更新本页。

所有大小均为 gzip 压缩后的大小；1 kB 为 1,024 字节。

## 控制台

`pnpm --filter kaeso-console build` 最后运行 `apps/console/scripts/budget.mjs`。它读取构建清单（`dist/.vite/manifest.json`），累加一个路由在首次绘制前需要的 JavaScript：

- 入口块及其静态导入的所有内容；
- 该路由以及其上每个布局路由（`__root`、`_app`、`orgs/$org`、`projects/$project` 等）的懒加载块，连同它们的静态导入；
- 对项目概览而言，还包括路由守卫（`src/auth/route-gates.ts`），框架会在页面绘制前与会话一起加载它。

用到时才加载的代码不计入：提示框的定位、日期选择器的日历、命令面板、重新登录对话框、快捷键列表、**创建组织**、提示消息容器，以及页面中与数据一起加载的部分。如果 `src/features` 下的模块出现在入口或其静态导入中，脚本也会让构建失败，因为包括登录页在内的每个页面都会先加载入口。

| 路由 | 文件 | 目标 | 上限 | 实测 |
| --- | --- | --: | --: | --: |
| 登录 | `src/routes/sign-in.tsx` | 120 kB | 126 kB | 123.1 kB |
| 项目概览 | `src/routes/_app/orgs/$org/projects/$project/index.tsx` | 180 kB | 190 kB | 181.6 kB |

路由超过上限时构建失败。目标是路由应当达到的大小，目前两个路由都还没有达到。上限是实测大小再加几 kB 的余量，路由变小后约为 5%，这样小改动能放得下，而性能倒退会失败。只有在需要的拉取请求中、给出理由时才能提高上限；路由变小时则降低上限。

登录页的 123.1 kB 中：

| 块 | 内容 | 大小 |
| --- | --- | --: |
| `framework` | React 和 React DOM 及调度器、TanStack Router 和 Query、openapi-fetch，以及控制台的 API 客户端、会话和格式化 | 95.3 kB |
| `ui` | 库的 `cn` 及其自己的 `tailwind-merge` 配置，库的按钮、文本、字段、输入框和文字标志，页面标题和公开页面的布局 | 13.3 kB |
| 入口 | 路由树及其预加载列表 | 9.7 kB |
| 登录页 | 它自己的块 | 其余部分 |

项目概览在此基础上增加了控制台框架、统计卡片和图表，以及页面本身。框架是侧边栏及其 Base UI 和 Floating UI 部分；侧边栏的菜单在页面空闲时或首次悬停、聚焦、按下时加载，带 Base UI 对话框的导航抽屉则在页面空闲时或首次点按时加载。图表使用 d3 和库自己的线性比例尺，不包含 d3-scale 的颜色、插值和数字格式模块。

`src/main.tsx` 在加载语言和渲染之前，就请求与地址匹配的路由的块（`src/router.tsx` 中的 `loadPageChunks`），因此页面代码在入口仍在执行时就开始下载。德语和中文会同时获取控制台和库的文案。

### 影响这些数字的因素

- **路由。** 路由树位于入口中，因此每个路由的路径、搜索参数校验和 `beforeLoad` 都是每个页面的一部分。路由文件只从其页面导入类型；导入值会把页面模块及其导入拉进入口。搜索参数校验是路由文件中的一个函数，由 `validateSearch` 和组件的 `Route.useSearch({ select })` 共用，因此它在入口中只出现一次。loader、组件以及 pending、错误和未找到组件都被拆分到路由自己的块中。
- **入口导入的模块**会完整地进入首次绘制，连同它们导入的一切，包括只有后续页面才用到的导出。因此会话的查询（`src/auth/session-queries.ts`）和时间段的搜索参数校验（`src/format/period-search.ts`）是独立模块：TanStack Query 的 observer 和日期标签只留在用到它们的页面中。
- **预加载列表。** 对于路由树可能加载的每个懒加载块，入口都带有它所需块的列表，以便浏览器立即获取。列表中的每个块都要占用它的文件名，因此一个被许多页面用到的新共享块会让每个页面都变大。
- **共享块。** 打包工具会把多个懒加载页面共享的模块放进同一个块。如果概览不需要的页面与概览需要的页面共享某个模块，概览也会加载这个块，以及块中的其他所有内容。
- **库。** 首次绘制时显示的 `@logicplanes/kaeso-ui` 组件，或通过所显示内容导入的组件，都计入预算；交互时才加载的组件不计入。
- **首次绘制与之后。** 只在数据到达后、交互后或首次绘制后才显示的内容，都可以那时再加载。例如：概览的明细和侧栏（`src/features/project/overview-parts.tsx`）、它的条形图（`src/kaeso/usage-bars.tsx`）和时间段选择器（`src/kaeso/period-picker.tsx`）、登录页输入地址之后的步骤（`src/features/sign-in/sign-in-steps.tsx`）、设置侧边栏、侧边栏菜单、连接提示消息，以及单键快捷键（`src/shell/listeners.tsx`；加载前按下的键由 `src/shell/early-keys.ts` 保存）。

### 构建如何让它们保持精简

在 `apps/console/vite.config.ts` 中：

| 设置 | 作用 |
| --- | --- |
| `uiEntryImports` | 把 `import { … } from "@logicplanes/kaeso-ui"` 改写为库中各组件的入口，把每个 `icons.name` 改写为从 `@hugeicons/core-free-icons` 只导入该图标，因此路由只包含它所绘制的组件和图标 |
| `codeSplittingOptions` | 让路由插件把每个路由的 loader、组件以及错误、未找到和 pending 组件拆分到各自的块中（`autoCodeSplitting`） |
| `codeSplitting.groups` | 把框架和控制台首次绘制所需的模块（`src/api`、`src/auth`、`src/format`）放进一个 `framework` 块；把每个页面绘制都要用到的库部分（`cn`、按钮、标志、文本、区块、字段、输入组、页面标题、公开页面的布局，以及每个页面都会绘制的三个图标）放进一个 `ui` 块；把路由守卫和框架本来就会导入的内容放进一个 `gates` 块。如果按共享它们的页面拆分，这些模块的压缩效果会更差，还会让每个预加载列表变长 |
| `routePreloads`、`resolvePreloads`（`build.modulePreload.resolveDependencies`） | 从预加载列表中去掉入口已经导入的块、`_app` 下框架带来的块，以及只有实例设置页面和试验场需要的块；这些块随其页面自己的块一起加载 |
| `chunkFileNames` | 共享块只用七个字符的哈希命名；页面以及 `framework` 和 `ui` 分组保留名称。每个块的内容可从清单和 source map 中查到 |
| `build.modulePreload.polyfill` | 关闭：构建所面向的每个浏览器都能自行预加载模块 |
| `resolve.alias` | 把 `use-sync-external-store` 的 shim 指向 React 自带的 `useSyncExternalStore`，这样路由的 store 和 Base UI 就不会再带一份实现 |
| `themeInit`、`fontPreload` | 把库在绘制前运行的主题脚本作为 `/theme-init.js` 提供，并预加载界面字体，使首次绘制无需内联脚本就具备主题和字体 |

## 文档站点

`pnpm --filter kaeso-docs build` 最后运行 `apps/docs/scripts/budget.ts`，它测量页面在首次绘制前需要的内容，超过上限时让构建失败。

| 预算 | 计入内容 | 实测 | 上限 |
| --- | --- | --: | --: |
| 框架，按语言 | 入口及其静态导入和样式表、该语言的站点模块，以及库的文案 | 201.9 kB（英语）、207.4 kB（德语）、207.9 kB（中文） | 216 kB |
| 单个页面的内容 | 每个页面自己的块：它的内容块和目录 | 最多 17.8 kB（`zh-CN/development/kpp/anthropic-messages`） | 每页 24 kB |

每增加一个页面，框架都会略微变大，因为该语言的站点模块包含侧边栏以及每个页面的标题和描述，这样侧边栏能与框架一起绘制，永远不会跳动。德语和中文还会加载库在该语言下的文案，约 5 kB。框架的上限是最大框架再留一点余量。页面内容有自己的上限，因此页面过长时，无论谁编辑它，构建都会带着它的地址失败；解决办法是把它拆成两个页面。

带索引的搜索、提示消息容器、导航抽屉和代码高亮的语法定义都在首次绘制之后加载，不计入预算。构建会打印搜索的大小：不含索引 6.7 kB，包括 Base UI 的对话框。

| 框架的组成部分 | 存在的原因 |
| --- | --- |
| React 和 TanStack Router（`framework`） | 每个页面 |
| `AppFrame`、`Sidebar` | 框架；`Sidebar` 带来 Base UI 的菜单，导航抽屉在页面空闲时或首次点按时加载 |
| `Select` | 侧边栏底部的语言和主题选择 |
| 提示框、`IconButton` | 搜索和折叠控件 |
| 页面及其渲染器 | `Text`、`Link`、`Code`、`CodeBlock`（只有出现代码块时才加载高亮器）、`Banner`、`Table`、`ScrollArea`、`CheckboxBox`、`Separator`、`TableOfContents`，以及用于变更日志的 `Badge` 和日期格式化 |
| 该语言的 `site` 模块 | 侧边栏以及每个页面的标题和描述 |

站点还让首次绘制保持精简和稳定：

- 构建时，从 `@logicplanes/kaeso-ui` 的导入会被改写为库中各组件的入口（`apps/docs/vite.config.ts` 中的 `uiEntryImports`）。
- 页面在构建时编译，因此 marked 和内容构建永远不会进入浏览器。
- 每个页面的 HTML 都会预加载其语言和页面本身所需的块，因此首次绘制无需等待第二次往返。
- 界面字体会被预加载（`fontPreload`），因此首次绘制就使用 Inter，字体到达时不会重排。
- 图片带有从文件中读取的宽度和高度，因此加载时页面不会跳动。

## UI 库

`pnpm build:packages` 构建 `@logicplanes/kaeso-ui`，最后运行 `packages/ui/scripts/budgets.mjs`，它测量 `packages/ui/dist` 中已构建的包，超出预算时让构建失败。应用只为它导入的组件付出代价，因此这些预算衡量的是单个组件本身的开销。

| 预算 | 计入内容 | 实测 | 预算 |
| --- | --- | --: | --: |
| 基础部分 | 至少被 40% 的组件入口导入的共享块 | 5.3 kB | 8 kB |
| `Button` | `components/button.js` 及其导入，不含基础部分和其他组件的入口 | 3.0 kB | 3 kB |
| `Menu` | `components/menu.js`，计算方式相同 | 4.8 kB | 9 kB |
| `DataTable` | `components/data-table.js`，计算方式相同 | 10.3 kB | 22 kB |
| `styles.css` | 整个样式表 | 18.0 kB | 28 kB |

在以下情况下，同一个脚本也会让构建失败：

- `dist` 中的某个模块在模块级别有打包工具必须保留的语句，导致未使用的组件无法被移除；
- 某个文件用 `typeof process` 来判断 `process.env.NODE_ENV`，这会把开发期检查留在浏览器包中；
- `formatMessage`、`useUiText` 或 `useLocale` 会连带引入格式化函数（`format.js`）。单独的 `formatMessage` 为 0.7 kB。

## KPP

```bash
cargo bench -p kaeso-codecs --bench codecs
```

每个基准测试运行一个完整的同方言循环：将 wire 字节解码为 KPP，再将 KPP 编码回 wire 字节。所有方言的输入都由同一份 KPP 内容构建，因此数字可以相互比较。使用 criterion（`--warm-up-time 2 --measurement-time 5`）在 Apple M5 Pro、Rust 1.99.0、release 配置下测得：

| 基准测试 | Anthropic Messages | OpenAI Chat | OpenAI Responses |
| --- | --: | --: | --: |
| 100 条消息的对话（一个系统提示词、一个工具、25 轮由用户文本、带工具调用的助手文本、工具结果和助手文本组成的对话），解码并编码 | 352 µs（78 MiB/s） | 242 µs（105 MiB/s） | 279 µs（90 MiB/s） |
| 以 512 字节分块输入的 2,000 个事件的文本流，解码并编码 | 3.9 ms（每个事件约 2.0 µs，61 MiB/s） | 10.6 ms（每个事件约 5.3 µs，36 MiB/s） | 8.6 ms（每个事件约 4.3 µs，52 MiB/s） |

对网关而言，一段长对话在每个方向上的开销都远低于一毫秒，一个流式 Token 只需几微秒，远低于任何模型两个 Token 之间数十毫秒的间隔。Chat 和 Responses 的流编解码器在每个事件上做的工作比 Anthropic 的多：每个 Chat 分块都会重复响应信封，每个 Responses 事件都带有序列号，其中一些还会重复已累积的条目。优化应当从这里着手。

CI 会检查基准测试能否构建（`cargo bench -p kaeso-codecs --bench codecs --no-run`）。修改对性能敏感的编解码器代码时，要把完整运行的结果与上表进行比较。

## 模糊测试

[模糊测试目标](testing.md#模糊测试)最近一次在上述机器上运行，每个目标 10 分钟，五个目标并行，参数为 `-rss_limit_mb=2048 -max_len=65536`：

| 目标 | 执行次数 | 覆盖率（边） | 峰值内存 | 结果 |
| --- | --: | --: | --: | --- |
| `sse` | 7,628,740 | 372 | 707 MB | 无崩溃 |
| `decode_anthropic_messages` | 3,599,755 | 11,047 | 588 MB | 无崩溃 |
| `decode_openai_chat` | 2,044,761 | 11,347 | 570 MB | 无崩溃 |
| `decode_openai_responses` | 1,166,478 | 10,263 | 571 MB | 无崩溃 |
| `decode_openai_embeddings` | 11,245,268 | 4,359 | 668 MB | 无崩溃 |

峰值内存是整个 libFuzzer 进程的内存，包括其内存中的语料库；它在最初几分钟后保持平稳，没有任何输入达到 2 GB 的上限。库对每个输入使用的内存受输入大小和 SSE 帧大小上限（`sse::DEFAULT_MAX_FRAME_BYTES`，16 MiB）的约束。
