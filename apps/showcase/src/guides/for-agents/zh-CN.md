---
title: 面向智能体
description: 智能体如何使用本组件库构建 Kaeso 界面。编写界面代码之前请先阅读。
---

# 面向智能体的 Kaeso UI

智能体如何使用 `@logicplanes/kaeso-ui` 构建 Kaeso 界面。编写界面代码之前请先阅读。本文件随软件包发布（`node_modules/@logicplanes/kaeso-ui/AGENTS.md`），也是页面 [ui.kaeso.app/for-agents](https://ui.kaeso.app/for-agents)。

## 在项目中加载

让智能体每次处理界面时都阅读这些规则：

- **Claude Code：** 在项目的 `CLAUDE.md` 中加入这一行：

  ```
  @node_modules/@logicplanes/kaeso-ui/AGENTS.md
  ```

- **Codex、Cursor 以及其他读取 `AGENTS.md` 的智能体：** 在项目的 `AGENTS.md` 中加入这一行：

  ```
  Before writing interface code, read node_modules/@logicplanes/kaeso-ui/AGENTS.md and follow it.
  ```

## 设计基础

组件背后的设计系统，位于 `node_modules/@logicplanes/kaeso-ui/foundations/`。按手头的工作阅读对应文件：

| 文件                           | 何时阅读                       |
| ------------------------------ | ------------------------------ |
| `foundations/tokens.md`        | 编写任何类名时：只存在令牌值   |
| `foundations/color.md`         | 选择表面、文字角色或状态色时   |
| `foundations/typography.md`    | 排版文字、数字、ID 或代码时    |
| `foundations/shape.md`         | 选择圆角、嵌套表面时           |
| `foundations/elevation.md`     | 绘制线条、描边环或凸起表面时   |
| `foundations/motion.md`        | 添加动画或显示加载时           |
| `foundations/icons.md`         | 放置图标时                     |
| `foundations/writing.md`       | 编写标签、消息、空状态或错误时 |
| `foundations/accessibility.md` | 构建任何可交互内容时           |
| `foundations/psychology.md`    | 审查界面时：27 条感知规则      |

## 机器可读

- `node_modules/@logicplanes/kaeso-ui/ai/component-registry.md`：每个组件的导入方式、属性、类型、默认值和变体，由类型生成。JSON 版本位于 `ai/component-registry.json`。
- `node_modules/@logicplanes/kaeso-ui/ai/component-guides.md`：每个组件的用途、键盘操作、无障碍、推荐与禁忌，以及每个示例的代码。首次使用某个组件前请阅读其章节。
- `node_modules/@logicplanes/kaeso-ui/ai/tokens.md`：每个令牌值，包括浅色和深色。`ai/icons.md`：图标词汇表。
- [ui.kaeso.app/llms.txt](https://ui.kaeso.app/llms.txt)：这些规则和每个组件的纯文本版本。
- [ui.kaeso.app/components.json](https://ui.kaeso.app/components.json)：在线版注册表。
- ui.kaeso.app 上的每个组件页面都有**复制页面**，可将其复制为 Markdown。

## 设置

使用 Tailwind CSS v4 时，在应用的样式表中：

```css
@import "tailwindcss";
@import "@logicplanes/kaeso-ui/tailwind.css";
```

不使用 Tailwind 时，导入一次 `@logicplanes/kaeso-ui/styles.css`。然后包裹一次应用：

```tsx
import { ThemeProvider, TooltipProvider, Toaster } from "@logicplanes/kaeso-ui";

export function Providers({ children }: { children: React.ReactNode }) {
  return (
    <ThemeProvider>
      <TooltipProvider>
        {children}
        <Toaster />
      </TooltipProvider>
    </ThemeProvider>
  );
}
```

## 规则

1. **组合，绝不另行设计。** 控制台只由 `@logicplanes/kaeso-ui` 构建。缺少什么，就加入组件库，而不是在应用中绘制。
2. **`className` 只用于布局**：外边距、宽度、网格位置、弹性布局。绝不在组件上设置颜色、圆角、阴影、字体或内边距；请使用其属性和变体。
3. **只存在令牌值。** `bg-panel`、`text-fg-secondary`、`h-36`、`rounded-pill`。没有 `bg-blue-500`、没有 `p-7`、没有任意值、没有 `dark:`。
4. **将输入框包在 `Field` 中**（或为其设置 `label`），以提供标签、提示和错误。没有标签或 `aria-label` 的输入框就是缺陷。
5. **每个视图只有一个主要按钮。** 其余均为 secondary 或 ghost。
6. **不使用装饰性卡片和装饰性图标。** 内容直接放在面板上；空状态和对话框用文字表达。
7. **颜色只用于数据和状态**，状态始终配有文字。
8. **使用格式化函数**：金额、Token、时长和日期使用 `@logicplanes/kaeso-ui/format`。金额始终是精确的十进制字符串。
9. **使用图标词汇表**：`icons.key`、`icons.model`，绝不按外观挑选图标。
10. **文案遵循统一语气**：句首大写，按钮以动词开头，不用感叹号或表情符号，错误提示要说明下一步该做什么。
11. **留出空间。** 让面板保持疏朗：组件自带间距，不要用额外的包裹层、边框或标题塞满界面。元素更少，每个都清晰。

## 哪个组件用于哪项工作

| 工作                  | 使用                                                         |
| --------------------- | ------------------------------------------------------------ |
| 应用布局              | `AppFrame` 搭配 `Sidebar`、`PageHeader`、`AppFrame.Body`     |
| 设置页面              | `AppFrame.Settings`、`SettingsGroup`、`SettingsRow`          |
| 需要浏览并操作的集合  | 带分组标题的 `List`                                          |
| 需要按列比较的集合    | `DataTable`（虚拟化、可排序）                                |
| 集合上方的筛选        | `Toolbar` 搭配 `FilterChip`、`SearchInput`、`DisplayOptions` |
| 在 2–4 个视图之间选择 | `Segmented`；页面分区：`PillTabs`；详情分区：`Tabs`          |
| 从众多选项中选一个    | `Select`；可搜索：`Combobox`                                 |
| 确认不可撤销的操作    | `AlertDialog`；删除：`DeleteResource` 区块                   |
| 分多步完成的任务      | `StepDialog`                                                 |
| 操作后的反馈          | `toast()`，可撤销的操作提供撤销                              |
| 只显示一次的机密      | `SecretReveal`                                               |
| 一目了然的数字        | `MetricRow` 搭配 `Stat`；随时间变化：`TimeseriesChart`       |
| 尚无内容可显示        | 带一个操作的 `Empty`                                         |

## 一个界面

```tsx
import { AppFrame, Sidebar, PageHeader, Toolbar, FilterChip, SearchInput, DataTable, Button, icons } from "@logicplanes/kaeso-ui";

export function RequestLogs({ rows }: { rows: LogRow[] }) {
  return (
    <AppFrame brandHref="/" sidebar={<ConsoleSidebar current="logs" />}>
      <PageHeader start={<AppFrame.MenuButton />} title="Request logs" actions={<Button variant="secondary">Export CSV</Button>} />
      <Toolbar>
        <FilterChip property="Status" operator="is" value="2xx" />
        <SearchInput aria-label="Search logs" placeholder="Search logs" />
      </Toolbar>
      <AppFrame.Body>
        <DataTable data={rows} columns={columns} />
      </AppFrame.Body>
    </AppFrame>
  );
}
```
