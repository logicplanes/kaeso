---
title: 控制台
description: Web 控制台如何读取数据、绘制页面、加载代码和显示错误。
order: 2
---

控制台是 `apps/console` 中的一个静态 React 应用，与网关位于同一域名下提供服务。它通过网关让用户登录，除了网关设置的 HttpOnly 会话 Cookie 之外不保存任何东西。列表、筛选条件、时间段和打开的详情都保存在地址中，因此链接可以分享，后退按钮也能正常工作。

| 部分 | 位置 |
| --- | --- |
| 路由、加载器和页面 | `src/routes`、`src/features` |
| 由 `openapi.json` 生成的 Platform API 客户端 | `src/api` |
| 会话、权限和路由守卫 | `src/auth` |
| 应用框架、侧边栏、命令面板和快捷键 | `src/shell` |
| 由库组件组合而成、仅供控制台使用的部件 | `src/kaeso` |
| 各种语言及其文案目录 | `src/i18n` |
| 开发环境栈的模拟服务商、身份提供方和主密钥 | `stack` |

## 规则

- **只使用 Platform API**，且每次调用都经过 `src/api` 中的客户端。API 做不到的事情不会显示，也不会伪造任何内容。
- **权限来自 API，从不猜测。** 组织权限来自 `auth/session`，项目权限来自 `GET …/projects/{project}/my-rights`，每项权限的名称、描述、类别和依赖来自 `GET orgs/{org}/rights`。`403` 是最终结论：用户无权查看的区域会被隐藏，单个操作则被禁用，并显示所需权限的名称以及谁能授予它（`useNeedsRight`）。
- **浏览器中没有机密。** 会话是一个控制台从不读取的 Cookie。密钥的机密部分只显示一次，从不保存。`localStorage` 只保存上次使用的组织和项目、主题、语言、侧边栏宽度、表格设置、已关闭的横幅以及试验场中未发送的提示词。
- **链接中的令牌放在片段中传递。** 控制台读取一次 `location.hash`，调用 API，然后替换掉带令牌的地址。
- **每个可见元素都来自 `@logicplanes/kaeso-ui`**，`className` 只用于布局：外边距、宽度、网格、flex、间距和定位。`pnpm lint` 通过 `apps/console/eslint/kaeso.js` 中的规则强制执行这一点。
- **每个词都有三种语言**，通过 `t()` 实现（[翻译](../translations.md#控制台)）。
- **描述是能放在一行内的一个句子**，各位置有各自的长度限制：设置行 60 个字符，区块或页面 80 个，对话框 100 个，空状态 90 个，提示消息 60 个。`src/copy.test.ts` 在每种语言中检查这些限制。

## 数据

每次读取都是一个以请求为键的 TanStack Query 查询；每次修改都是一个 mutation，它会使所涉及的数据和审计日志失效。`src/api/client.ts` 中的客户端由仓库的 `openapi.json` 生成，发送会话 Cookie（同源），并将每次失败转换为带有 API 的 `type`、`message`、`param` 和 `kaeso-request-id` 的 `ApiError`（`src/api/errors.ts`）。

列表使用游标分页（`src/api/paging.ts`）；导出是带 Cookie 的流式下载（`src/api/download.ts`）。

筛选条件、时间段和打开的侧滑面板保存在地址中。每个路由在一个函数中检查其查询参数，`validateSearch` 和页面的 `Route.useSearch({ select })` 共用这个函数，因为路由器也会把父级路由的原始值交给页面；页面不认识的值会回退到默认值。

查询默认设置（`src/query.ts`）：缓存的数据立即显示，并在后台重新获取；列表在 10 秒内视为新鲜，目录和设置为 60 秒，会话则总是重新验证；客户端错误从不重试。

## 样式

控制台没有自己的样式。`@logicplanes/kaeso-ui` 提供每个可见元素和样式表（`src/styles.css` 导入库的 Tailwind 入口）。控制台代码只把 Tailwind 用于布局，使用同一套令牌尺度。库的绘制前主题脚本由一个 Vite 插件以 `/theme-init.js` 的形式提供，因此内容安全策略不允许任何内联脚本。

## 打包

路由由路由器拆分（`autoCodeSplitting`），因此页面自己的代码随页面一起加载。控制台代码将 `@logicplanes/kaeso-ui` 作为一个包导入，构建时会把这些导入改写为库中各组件的入口，并把每个 `icons.name` 改写为对该图标本身的导入（`vite.config.ts` 中的 `uiEntryImports`），因此页面只携带它所绘制的组件和图标。库把模块级的组件工厂标记为纯函数，因此用到某个组件时不会连带保留其文件中的其余部分。

首次绘制不需要的部分在使用时才加载：提示消息容器、命令面板、近期登录对话框、快捷键列表、“创建组织”、提示框和日期选择器的日历。概览页的明细和侧栏，以及登录流程中输入地址之后的步骤，与它们等待的数据一同加载。`pnpm build` 会根据[预算](../performance.md#控制台)检查结果。

## 错误

每个错误都是一条提示消息；页面中从不显示错误。修改操作经过 `useApiMutation`（`src/auth/mutation.ts`），公开表单经过 `useSubmit`，加载失败则由查询缓存的处理程序（`src/query.ts`）处理，并提供“重试”。网关的失败会附带请求 ID。缺失的对象（404）由页面或侧滑面板自己的状态表示，会话结束则跳转到登录页。
