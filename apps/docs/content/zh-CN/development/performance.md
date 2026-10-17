---
title: 性能
description: 控制台、文档和 UI 库的打包预算、编解码器的基准测试，以及模糊测试的结果。
order: 8
---

性能靠测量，而不是猜测：当某个页面超出其预算时，控制台和文档的构建就会失败；某个组件超出预算时，UI 库的构建会失败；KPP 的编解码器则有基准测试和模糊测试目标。需要更大预算的修改要在同一个拉取请求中提高上限、更新本页，并说明原因。

## 控制台

`pnpm --filter kaeso-console build` 最后会运行 `apps/console/scripts/budget.mjs`，它读取构建的清单（`dist/.vite/manifest.json`），并累加一个路由在绘制之前所需的 JavaScript 的 gzip 后大小：

- 入口 chunk 及其静态导入的所有内容；
- 该路由及其上方每个布局路由（`__root`、`_app`、`orgs/$org`、`projects/$project` 等）的延迟加载 chunk，以及它们静态导入的所有内容；
- 对于项目概览，还包括控制台路由的判定逻辑（`src/auth/route-gates.ts`），框架会在页面绘制之前将其与会话一同加载。

在使用时才加载的动态导入不计入：提示框的定位、日期选择器的日历、命令面板、近期登录对话框、快捷键列表、“创建组织”、提示消息容器，以及页面中与其数据一同加载的部分。当 `src/features` 下的某个模块位于入口或其静态导入中时，该脚本同样会使构建失败，因为包括登录页在内的每个页面都会先加载入口。

路由超过其上限时，构建就会失败。目标是一个路由应当达到的大小；上限是实测大小向上取整到下一个千字节，只有在需要时才在相应的拉取请求中说明理由并提高。

| 路由 | 文件 | 目标 | 上限 | gzip 后实测 |
| --- | --- | --: | --: | --: |
| 登录 | `src/routes/sign-in.tsx` | 120 kB | 126 kB | 126.0 kB |
| 项目概览 | `src/routes/_app/orgs/$org/projects/$project/index.tsx` | 180 kB | 206 kB | 205.9 kB |

1 kB 为 1,024 字节。两个路由目前都未达到目标。在登录页的 126.0 kB 中，`framework` chunk（带调度器的 React 和 React DOM、TanStack Router 和 Query、openapi-fetch，以及控制台的 API 客户端、会话和格式化）占 98.1 kB，`ui` chunk（库的 `cn` 及其专门配置的 `tailwind-merge`、其按钮、文本、字段、输入框和标志、页面标题以及公开页面的布局）占 14.3 kB，带路由树及其预加载列表的入口占 9.5 kB，其余为登录页自己的 chunk。项目概览在此基础上增加了控制台的框架（侧边栏及其 Base UI 和 Floating UI 部件；其菜单在页面空闲时或首次悬停、聚焦或按下时加载，库的日期选择器也是如此）、统计卡片和图表及其 d3 模块，以及页面本身。

### 影响这些数字的因素

- **路由。** 路由树位于入口中，因此每个路由都会把它的路径、查询参数验证和 `beforeLoad` 加到每个页面上。路由文件只从其页面导入类型：导入值会把页面的模块及其导入带进入口。它的查询参数检查是路由文件中的一个函数，`validateSearch` 和组件的 `Route.useSearch({ select })` 共用这个函数，因此该检查只在入口中出现一次，拆分出去的组件从那里导入它。加载器、组件，以及 pending、错误和未找到组件都拆分到该路由自己的 chunk 中。
- **预加载列表。** 对路由树可能加载的每个延迟 chunk，入口都携带其所需 chunk 的列表，以便浏览器立即获取它们。列表中的每个 chunk 都要占用其文件名的大小；一个被许多页面用到的新共享 chunk 会让每个页面都变大。
- **共享 chunk。** 打包器会把多个延迟加载页面共用的模块放进同一个 chunk。当概览不需要的某个页面与它需要的某个页面共用一个模块时，概览也会加载那个 chunk，连同打包器放进去的其他所有内容。
- **库。** 首次绘制时绘制的 `@logicplanes/kaeso-ui` 组件，或被这类组件导入的组件，都计入预算；交互时才加载的组件则不计入。
- **首次绘制与之后。** 页面只在数据到达后、交互后或首次绘制后才显示的内容，都可以等到那时再加载：概览的明细和侧栏（`src/features/project/overview-parts.tsx`）、其条形图（`src/kaeso/usage-bars.tsx`）和时间段选择器（`src/kaeso/period-picker.tsx`）、登录流程中输入地址之后的步骤（`src/features/sign-in/sign-in-steps.tsx`）、设置侧边栏、侧边栏的菜单、连接状态提示消息和单键快捷键（`src/shell/listeners.tsx`，加载完成前按下的键保存在 `src/shell/early-keys.ts` 中）。

### 构建如何让它们保持精简

`apps/console/vite.config.ts`：

- **`uiEntryImports`** 把 `import { … } from "@logicplanes/kaeso-ui"` 改写为库中各组件的入口，并把每个 `icons.name` 改写为从 `@hugeicons/core-free-icons` 导入该图标本身，因此路由只携带它所绘制的组件和图标。
- 路由器插件中的 **`codeSplittingOptions`** 把每个路由的加载器、组件、错误组件、未找到组件和 pending 组件拆分到各自的 chunk 中（`autoCodeSplitting`）。
- **`codeSplitting.groups`** 把框架和控制台首次绘制所需的模块（`src/api`、`src/auth`、`src/format`）放进一个 `framework` chunk，把每个页面绘制时都要用到的库部件（`cn`、按钮、标志、文本、区块、字段、输入组、页面标题、公开页面的布局，以及每个页面都会绘制的三个图标）放进一个 `ui` chunk，并把路由判定逻辑连同框架本来就会导入的内容放进一个 `gates` chunk。如果按共用它们的页面来拆分，这些模块的压缩效果会更差，还会在每个预加载列表中增加文件名。
- **`routePreloads` 和 `resolvePreloads`**（`build.modulePreload.resolveDependencies`）从预加载列表中排除入口已经导入的 chunk、`_app` 下控制台框架带来的 chunk，以及只有实例设置页面和试验场才需要的 chunk；这些 chunk 与其页面自己的 chunk 一同加载。
- **`chunkFileNames`** 只用七个字符的哈希为共享 chunk 命名；页面以及 `framework` 和 `ui` 分组保留各自的名称。清单和 source map 说明了每个 chunk 包含什么。
- **`build.modulePreload.polyfill`** 已关闭：构建所面向的每个浏览器都会自行预加载模块。
- **`resolve.alias`** 把 `use-sync-external-store` 的 shim 指向 React 自己的 `useSyncExternalStore`，因此路由器的 store 和 Base UI 不会携带第二份实现。
- **`themeInit`** 以 `/theme-init.js` 的形式提供库的绘制前主题脚本，**`fontPreload`** 则预加载样式表使用的界面字体，因此首次绘制无需内联脚本就已具备主题和字体。

## 文档站点

`pnpm --filter kaeso-docs build` 最后会运行 `apps/docs/scripts/budget.ts`，它测量页面在能够绘制之前所需内容的 gzip 后大小，超过上限时使构建失败。

| 预算 | 计入的内容 | 实测 | 上限 |
| --- | --- | --: | --: |
| 每种语言的框架 | 入口及其静态导入和样式表、该语言的站点模块，以及库的文案 | 204.6 kB（英语），210.1 kB（德语），210.6 kB（中文） | 216 kB |
| 单个页面的内容 | 每个页面自己的 chunk：其内容块和目录 | 最多 17.3 kB（`zh-CN/development/kpp/anthropic-messages`） | 每页 24 kB |

框架随代码变化，并随每个页面略有增长，因为站点模块携带侧边栏以及每个页面的标题和描述，从而让侧边栏与框架一同绘制、从不发生位移。德语和中文还会加载库中对应语言的文案，约 5 kB。上限是最大框架的实测大小再留出少许余量。页面内容有自己的预算，因此无论谁编辑，过长的页面都会连同其地址一起报错失败；解决办法是拆成两个页面。

带索引的搜索、提示消息容器和代码高亮器的语法在首次绘制之后加载，不计入预算。构建会打印搜索的大小，不含索引为 3.5 kB。

| 框架的组成部分 | 存在的原因 |
| --- | --- |
| React 和 TanStack Router（`framework`） | 每个页面 |
| `AppFrame`、`Sidebar`、导航侧滑面板 | 框架；`Sidebar` 带入了 Base UI 的菜单 |
| `Select` | 侧边栏底部的语言和主题选择框 |
| 提示框、`IconButton` | 搜索和折叠控件 |
| 页面及其渲染器 | `Text`、`Link`、`Code`、`CodeBlock`（其高亮器只在有代码块时加载）、`Banner`、`Table`、`ScrollArea`、`Checkbox`、`Separator`、`TableOfContents`，以及更新日志所用的 `Badge` 和日期格式化 |
| 侧边栏以及每个页面的标题和描述 | 该语言的 `site` 模块，从而让侧边栏与框架一同绘制、从不发生位移 |

- 来自 `@logicplanes/kaeso-ui` 的导入在构建时被改写为库中各组件的入口（`apps/docs/vite.config.ts` 中的 `uiEntryImports`），因此框架只包含站点使用的组件。
- 页面在构建时编译：marked 和内容构建从不进入浏览器。
- 每个页面的 HTML 都会预加载其语言和页面本身所需的 chunk，因此首次绘制不必等待第二次往返。
- 界面字体会被预加载（`fontPreload`），因此首次绘制就使用 Inter，字体到达时不会重新排版。
- 图片带有从文件读取的宽度和高度，因此页面不会在图片加载时发生位移。

## UI 库

`pnpm build:packages` 会构建 `@logicplanes/kaeso-ui`，最后运行 `packages/ui/scripts/budgets.mjs`。它测量 `packages/ui/dist` 中构建好的包的 gzip 后大小，超出预算时使构建失败。应用只为它导入的组件付出体积，因此这些预算衡量的是单个组件本身的开销。

| 预算 | 计入的内容 | 实测 | 预算 |
| --- | --- | --: | --: |
| 基础部分 | 至少 40% 的组件入口都会导入的共享 chunk | 5.3 kB | 8 kB |
| `Button` | `components/button.js` 及其导入的内容，不含基础部分和其他组件的入口 | 3.0 kB | 3 kB |
| `Menu` | `components/menu.js`，计算方式相同 | 4.8 kB | 9 kB |
| `DataTable` | `components/data-table.js`，计算方式相同 | 10.3 kB | 22 kB |
| `styles.css` | 整个样式表 | 18.0 kB | 28 kB |

出现以下情况时，同一个脚本也会使构建失败：

- `dist` 中的某个模块有打包工具必须保留的模块级语句，导致未使用的组件无法被移除；
- 某个文件用 `typeof process` 保护 `process.env.NODE_ENV`，这会让开发期检查留在浏览器包中；
- `formatMessage`、`useUiText` 或 `useLocale` 会连带引入格式化函数（`format.js`）。仅 `formatMessage` 为 0.7 kB。

## KPP

```bash
cargo bench -p kaeso-codecs --bench codecs
```

每个基准测试运行一个完整的同方言循环：将线路字节解码为 KPP，再将 KPP 编码回线路字节。所有方言的输入都由同一份 KPP 内容构建，因此数字可以相互比较。使用 criterion（`--warm-up-time 2 --measurement-time 5`）在 Apple M5 Pro、Rust 1.99.0、release 配置下测得：

| 基准测试 | Anthropic Messages | OpenAI Chat | OpenAI Responses |
| --- | --: | --: | --: |
| 100 条消息的对话（一个系统提示词、一个工具、25 轮由用户文本、带工具调用的助手文本、工具结果和助手文本组成的对话），解码并编码 | 352 µs（78 MiB/s） | 242 µs（105 MiB/s） | 279 µs（90 MiB/s） |
| 以 512 字节分块输入的 2,000 个事件的文本流，解码并编码 | 3.9 ms（每个事件约 2.0 µs，61 MiB/s） | 10.6 ms（每个事件约 5.3 µs，36 MiB/s） | 8.6 ms（每个事件约 4.3 µs，52 MiB/s） |

对网关而言，一段长对话在每个方向上的开销都远低于一毫秒，一个流式 token 只需几微秒，远低于任何模型两个 token 之间数十毫秒的间隔。Chat 和 Responses 的流编解码器在每个事件上做的工作比 Anthropic 的多，因为每个 Chat 分块都会重复响应信封，而每个 Responses 事件都带有序列号，其中一些还会重复已累积的条目；优化应当从这里着手。

每项修改都会检查基准测试能否构建（`cargo bench -p kaeso-codecs --bench codecs --no-run`）；对性能敏感的编解码器代码的修改，要把完整运行的结果与上表进行比较。

## 模糊测试

[模糊测试目标](testing.md#模糊测试)最近一次在上述机器上运行，每个目标 10 分钟，五个目标并行，参数为 `-rss_limit_mb=2048 -max_len=65536`：

| 目标 | 执行次数 | 覆盖率（边） | 峰值内存 | 结果 |
| --- | --: | --: | --: | --- |
| `sse` | 7,628,740 | 372 | 707 MB | 无崩溃 |
| `decode_anthropic_messages` | 3,599,755 | 11,047 | 588 MB | 无崩溃 |
| `decode_openai_chat` | 2,044,761 | 11,347 | 570 MB | 无崩溃 |
| `decode_openai_responses` | 1,166,478 | 10,263 | 571 MB | 无崩溃 |
| `decode_openai_embeddings` | 11,245,268 | 4,359 | 668 MB | 无崩溃 |

峰值内存是 libFuzzer 进程的内存，包括其内存中的语料库；它在最初几分钟后保持平稳，没有任何输入达到 2 GB 的上限。库对每个输入使用的内存受输入大小和 SSE 帧大小上限（`sse::DEFAULT_MAX_FRAME_BYTES`，16 MiB）的约束。
