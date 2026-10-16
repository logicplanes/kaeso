---
title: 编写文档
description: 本站的页面如何编写、放置、链接、检查和翻译。
order: 6
---

本站的每个页面都是 `apps/docs/content` 中的一个 Markdown 文件，每种语言一个文件夹：`content/en`、`content/de` 和 `content/zh-CN`，各文件夹中的文件名相同。这里是保存文档的唯一位置；代码中只保留指向这里的简短 `README.md` 文件。如果某个变更会被使用、运行或集成 Kaeso 的人注意到，就要在同一个拉取请求中更新这些页面。

## 编辑页面

**在 GitHub 上：** 在任意页面底部选择“在 GitHub 上编辑”，或者打开 `apps/docs/content` 中的文件进行编辑，然后针对 `dev` 提交变更。GitHub 显示的页面与本站的效果相差不大，页面之间的每个链接在 GitHub 上也都能使用。

**在本地：**

```bash
pnpm build:packages
pnpm --filter kaeso-docs dev
```

站点运行在 [http://localhost:4480](http://localhost:4480)；保存页面时会自动重新加载，终端会指出需要修复的问题。`KAESO_DOCS_PORT` 可以指定其他端口。

| 命令 | 作用 |
| --- | --- |
| `pnpm --filter kaeso-docs check:content` | 无需构建，检查每个页面的前置元数据、链接、锚点、图片和地址，以及每种语言的页面及其 `order` 是否一致 |
| `pnpm --filter kaeso-docs build` | 将站点构建到 `apps/docs/dist`，然后检查[打包体积预算](performance.md#文档站点) |
| `pnpm --filter kaeso-docs preview` | 像静态托管服务那样提供构建结果 |

站点自身的代码和其他包一样经过测试（[测试](testing.md#文档站点)）。

## 页面格式

```markdown
---
title: API keys
description: Create keys for apps and scripts, and keep them under control.
order: 5
---

Every request to the model API carries a **key**.

## Create a key
```

| 字段 | 必填 | 说明 |
| --- | --- | --- |
| `title` | 是 | 页面的标题，也是它在侧边栏、搜索和浏览器标签页中的名称 |
| `description` | 是 | 标题下方的一句简短描述，最多 140 个字符；也用作页面面向搜索引擎和链接预览的描述 |
| `order` | 否 | 一个整数：在同一文件夹的页面中，数值越小越靠前；没有该字段的页面排在后面；数值相同或都没有该字段时，按文件名排序 |
| `nav` | 否 | 侧边栏中使用的较短标签 |

前置元数据中不能有其他字段；未知字段会导致错误。页面中的标题从 `##` 开始：页面标题是页面中唯一的 `#`。为了在 GitHub 上便于阅读，页面可以以 `# ` 加上完全相同的标题开头；本站只会显示一次。**本页内容**列出 `##` 和 `###` 标题。

## 页面的位置

| 文件 | 地址 | 在侧边栏中 |
| --- | --- | --- |
| `content/en/index.md` | `/` | “简介”，排在最前 |
| `content/en/console/index.md` | `/console` | 以其 `title` 为分组命名，并按其 `order` 排列分组；显示为“概览” |
| `content/en/console/api-keys.md` | `/console/api-keys` | 位于该分组中 |
| `content/de/console/api-keys.md` | `/de/console/api-keys` | 同一页面的德语版本 |
| `content/zh-CN/console/api-keys.md` | `/zh-CN/console/api-keys` | 同一页面的简体中文版本 |

文件名就是地址：小写，单词之间用连字符连接（`provider-keys.md`）。移动或重命名文件会改变它在每种语言中的地址，所有指向它的链接都必须随之修改；构建会指出没有修改的链接。分组内的文件夹需要一个 `index.md`，并将其页面嵌套在它之下，最多嵌套三层文件夹。名称以 `_` 开头的文件和文件夹不会发布。

## 链接

通过文件链接到其他页面，路径相对于当前页面，与 GitHub 解析的方式完全一致：

```markdown
See [rate limits](limits-and-budgets.md#rates) and [the API](../api/index.md).
```

- `#rates` 是 GitHub 为标题生成的锚点：小写，去掉标点，空格换成连字符。同一页面上的标题写作 `[budgets](#budgets)`。
- 指向不存在的页面或标题的链接会导致构建失败，并给出文件和链接。
- 链接直接写在行内；不支持引用式链接（`[文本][ref]` 加上下方的 `[ref]: …`），它们会显示为文本。
- 其他网站：只能使用 `http`、`https` 和 `mailto` 地址。仓库中不属于页面的文件链接到 GitHub，写作 `https://github.com/logicplanes/kaeso/blob/dev/<path>`，或者以代码形式写出：`crates/kaeso-mail`。

## 图片

将图片放在页面旁边，或放在页面旁边的 `images/` 文件夹中，用相对路径显示，并配上说明图片内容的替代文本：

```markdown
![The request flow: app, key, model, limits, provider](images/request-flow.svg)
```

支持 PNG、JPEG、GIF、WebP、AVIF 和 SVG。来自其他网站的图片不会显示，没有替代文本的图片会导致构建失败。选择在浅色和深色背景下都清晰可读的图片；SVG 可以通过 `@media (prefers-color-scheme: dark)` 自行切换颜色。链接到 `content/` 中的任何其他文件都会发布该文件。

## 提示块

使用 GitHub 的提示块，第一行只写类型：

```markdown
> [!NOTE]
> A key's secret is shown once.
```

`NOTE`、`TIP`、`IMPORTANT`、`WARNING` 和 `CAUTION` 各有自己的样式和标题。请谨慎使用。

## 其他语法

GitHub 风格的 Markdown：**粗体**、_斜体_、~~删除线~~、`代码`、列表和编号列表、任务列表、表格（`--:` 列右对齐，用于数字）、引用块、水平分隔线以及围栏代码块。围栏代码块要标明语言，还可以标明文件名：

````markdown
```ts title="support-bot/src/client.ts"
export const client = new OpenAI({ baseURL: "https://kaeso.example.com/v1" });
```
````

TypeScript、TSX、JavaScript、JSON、Bash（`sh`、`shell`、`console`）、Python、YAML 和 diff 会高亮显示；其他语言显示为纯文本。原始 HTML 不会被渲染，而是显示为文本。代码块中从不包含注释：在代码块前面的句子中解释命令。

## 写作

- 每条陈述都按照 Kaeso 的代码来描述：动笔之前，对照网关的配置核对设置，对照控制台的英文目录核对标签，对照网关、KPP 和控制台核对行为。绝不描述不存在的功能。
- 页面描述当前状态。计划、待解决的问题和待办事项放到 issue 中。
- 标题和小标题使用句子大小写，用词平实，句子简短，不使用感叹号。
- 控制台标签用粗体，并与控制台显示的完全一致：**API 密钥**、**创建密钥**、**设置** › **实例**。
- 示例使用 Kaeso 自己的名称和数据（`kaeso.example.com`、`sk-kso-…`、`owner@example.com`），绝不使用真实的密钥或个人数据。

## 翻译

每个页面都有英语、德语和简体中文版本。对英文页面的修改要在同一个拉取请求中同步到 `content/de` 和 `content/zh-CN`。

- **结构相同。** 文件名相同、`order` 相同、标题相同且顺序相同，代码块逐字节相同。标题、描述、正文、表格和提示块都要翻译。
- **链接。** 保持链接的文件不变；锚点使用译文页面自己的标题锚点。
- **保持原样的内容。** 代码、命令、设置（`KAESO_*`）、路径、字段名、权限（`keys.all.view`）、产品名称和模型 ID。
- **控制台标签。** 使用控制台对每个标签的译法，见其目录 `apps/console/src/i18n/locales/de` 和 `apps/console/src/i18n/locales/zh-CN`。
- **读起来自然**，而不是逐字直译，并保留每个事实和数字。

写作风格和术语表见[翻译](translations.md#写作风格)。有两条规则专门适用于页面：

| | 德语 | 简体中文 |
| --- | --- | --- |
| 步骤 | 不定式，动词在后：„Schlüssel erstellen“ | 动词在前：“创建密钥” |
| 描述 | 一句话 | 一句话，以“。”结尾 |

本站自身的文字，例如“在 GitHub 上编辑”或搜索，位于 `apps/docs/src/i18n/messages/en.ts`、`de.ts` 和 `zh-CN.ts` 中，并带有类型约束，缺少翻译时类型检查会失败。
