---
title: 文档站点
description: apps/docs/content 中的 Markdown 页面如何变成 docs.kaeso.app 上的静态站点。
order: 3
---

文档就是 `apps/docs` 中的 `kaeso-docs` 包：三种语言的 Markdown 页面，构建为一个静态站点，外观和用法与控制台一致，并且只用 `@logicplanes/kaeso-ui` 绘制。如何编写页面见[编写文档](../documentation.md)；本页介绍站点如何运作。

```text
content/<language>/**/*.md ─▶ scripts/content (at build time, in Vite) ─▶ virtual modules ─▶ src/ (React, @logicplanes/kaeso-ui) ─▶ dist/
                              frontmatter · marked's lexer · links,          site/<language>: sidebar, titles, previous/next, a loader per page
                              anchors and images checked · search words      page/<language>/<path>: one page's blocks, its own chunk
                              the same pages in every language               search/<language>: the index, loaded with the search
```

## 内容模型

| 组成部分 | 位置 | 规则 |
| --- | --- | --- |
| 一种语言 | `content/en`、`content/de`、`content/zh-CN` | 英语位于 `/`，其他语言位于 `/de` 和 `/zh-CN` 之下；每种语言都有相同的文件，`order` 也相同 |
| 一个页面 | `content/<language>/<folders>/<name>.md` | 地址就是它的路径：`content/en/console/api-keys.md` 对应 `/console/api-keys`；`index.md` 是其所在文件夹的页面 |
| 首页 | `content/<language>/index.md` | 必需 |
| 一个侧边栏分组 | 某种语言中的一个文件夹 | 名称取自其 `index.md` 的 `title`，否则取文件夹名；位置由该 `index.md` 的 `order` 决定 |
| 嵌套页面 | 分组内带有 `index.md` 的文件夹 | 其中的页面缩进显示在它下面；最多三层文件夹 |
| 文件夹内的顺序 | frontmatter 的 `order` | 文件夹的 `index.md` 排在最前，然后按 `order`，再按标签排序 |
| 页面在侧边栏中的标签 | frontmatter 的 `nav` | 默认为标题；`index.md` 默认为该语言的“概览”（首页为“简介”） |
| 图片和文件 | `content/` 中的任意位置 | 只有被页面显示或链接时才会发布，其尺寸在构建时读取 |
| 草稿和私人笔记 | 以 `_` 或 `.` 开头的名称 | 不包含在站点中 |

frontmatter 恰好有四个字段：`title` 和 `description`（一个简短的句子，最多 140 个字符）是必需的，`order` 和 `nav` 是可选的。任何其他字段都是错误，因此拼写错误绝不会悄无声息地失效。

## 构建

`apps/docs/scripts/content` 将页面转换为站点的模块：

| 文件 | 职责 |
| --- | --- |
| `frontmatter.ts` | 四个字段，每种错误都有对应的消息；每条描述一个句子，支持拉丁文和中文标点 |
| `slug.ts` | GitHub 的标题锚点（`## Rate limits` 对应 `#rate-limits`，重复的标题加上 `-1`、`-2`），以及由文件名得出的地址 |
| `compile.ts` | 单个页面：关闭原始 HTML 的 marked 词法分析器，然后转换为 `src/markdown/ast.ts` 中的页面格式；检查链接；标注块；搜索词 |
| `site.ts` | 单种语言：地址、侧边栏、上一页和下一页、按 GitHub 的方式解析的相对链接、图片及其尺寸、搜索 |
| `docs.ts` | 所有语言：每种语言一个站点，带有各自的前缀和标签，并检查它们以相同顺序包含相同的页面 |
| `changelog.ts` | 更新日志页面，在每种语言中都由 `CHANGELOG.md` 生成（[更新日志](#更新日志)） |
| `plugin.ts` | Vite 插件：虚拟模块、开发时在内容变化后重新加载、每个页面和语言各一个 HTML 文件（带有面向搜索引擎和链接预览的 head）、`404.html` 和站点地图 |
| `check.ts` | `pnpm --filter kaeso-docs check:content`：不构建应用而执行相同的检查 |

每个问题都会指明其文件：指向不存在的页面或标题的链接、没有替代文本或来自 `content/` 之外的图片、使用 http、https 或 mailto 以外协议的链接、第二个 `#` 标题、两个文件对应同一地址、某种语言缺少某个页面或其 `order` 不同。开发时这些问题会被打印出来，页面仍会渲染；构建和 `check:content` 则会因此失败。

### 安全

页面在 GitHub 上通过浏览器编辑，因此构建把它们当作不可信的文本。marked 只做词法分析，并关闭其 HTML 分词器：`<script>`、`<img onerror>` 以及其他所有标签都保持为字面文本。链接引用被关闭，链接只保留 http、https 和 mailto，判断前会先去除空白和控制字符，因此 `java\nscript:` 也会被识别出来。图片只能来自 `content/`。浏览器收到的是普通对象，从不是 HTML，也没有任何地方设置 `innerHTML`。`scripts/content/compile.test.ts` 中包含这些用例。

### 在 GitHub 和站点上都有效的链接

链接写在文件之间，与 GitHub 的跟随方式一致：`[keys](../console/api-keys.md#create-a-key)`。构建会相对于文件所在文件夹解析它，检查页面和标题是否存在，并写出该页面语言下的站点地址（`/de/console/api-keys#schlüssel-erstellen`）。标题使用 GitHub 的锚点，因此同一个锚点在两处都有效；翻译后的页面链接到翻译后标题的锚点。

### 更新日志

`/changelog` 在每种语言中都是唯一一个不对应 `content/` 中文件的页面：构建根据仓库根目录下的 `CHANGELOG.md` 生成它，因此更新日志只有一个来源，也从不需要手工翻译。`scripts/changelog.mjs` 中的 `history` 与发布时移动更新日志的是同一段代码，它把文件拆分为各个版本、`### Added` 这样的小节和 `#### Model API` 这样的主题；`changelog.ts` 像编译页面一样编译每个部分，并解析文件中的链接引用。链接如果既不是完整地址，也不是页面上的某个版本，构建就会失败，与页面中的失效链接一样。

在侧边栏中，这个页面紧跟在简介之后。页面自身的文字（包括小节名称）属于站点，并且已翻译；条目保持英文，在德文和中文页面上标记为 `lang="en"`，这些页面会在描述下方说明这一点。每个版本的锚点是 `v` 加上版本号，与其 Git 标签一致，在每种语言中都相同：`/de/changelog#v{{version}}`。最新的稳定版本标记为“最新”，与 GitHub 一致；预发布版本会标记为预发布；每个版本都以页面语言显示日期，并链接到它的 GitHub 版本以及自上一个版本以来的变更。`[Unreleased]` 下没有内容时会被省略，网站从发布标签构建时就是如此；有内容时，它排在最前面，并说明这些变更尚未包含在任何版本中。页面为订阅阅读器提供 GitHub 上的版本订阅源，页面中的词语也会进入搜索，排在其他所有页面之后。单元测试和端到端测试读取 `e2e/fixtures/CHANGELOG.md`；`KAESO_DOCS_CHANGELOG` 可以指定另一个文件。

## 应用

| 文件夹 | 职责 |
| --- | --- |
| `src/router.tsx` | 每种语言一棵路由树：一个加载该语言站点模块和库文案的布局，以及每个地址一个路由 |
| `src/site/` | 加载某种语言的站点模块、其搜索索引和库的文案，各只加载一次 |
| `src/page/` | 页面（标题、描述、内容、上一页和下一页、“在 GitHub 上编辑”、“本页内容”）、未找到页面，以及无法加载的页面 |
| `src/markdown/` | 页面格式（`ast.ts`）及其用库组件进行的绘制（`render.tsx`） |
| `src/shell/` | 带有侧边栏的 `AppFrame`、搜索、语言和主题选择框、经由路由器的链接、提示消息 |
| `src/i18n/` | 各种语言及其地址、记住的偏好，以及 `messages/` 中站点自己的文案 |

框架与控制台相同：带有“跳到主要内容”链接和 `Sidebar` 的 `AppFrame`，文字标志单独位于左上角，距左边和上边各 28px，面板嵌在外框之内，宽度低于 1024px 时显示手机顶栏和导航侧滑面板；手机顶栏中的搜索按钮位于导航开关旁边。页面的布局与控制台的设置页面相同：`title-lg` 标题和 `body-lg` 描述位于 640px 宽的列中；正文为 `body-lg`，标题为 `title`（`##`）和 `title-sm`（`###`），代码放在 `CodeBlock` 中，标注块放在 `Banner` 中，表格放在水平 `ScrollArea` 内的 `Table` 中。宽度从 1280px 起，“本页内容”（`TableOfContents`）位于右侧；没有它的页面也保留这一列，因此正文在页面之间从不移动。

悬停或聚焦链接时会加载页面的 chunk（`defaultPreload: "intent"`），因此页面能立即打开。搜索（`CommandPalette`）及其索引在首次绘制后片刻加载；侧边栏或手机顶栏中的搜索按钮、⌘K 或 Ctrl+K 都能打开搜索，加载完成前输入的内容会成为它的查询。它按标题、描述和正文词语查找页面，按标题自身的词语查找标题，并提供主题和其他语言的切换。

首次访问英语地址时，会依据浏览器的语言偏好跳转到 `/de` 或 `/zh-CN`，而不触发页面导航；在菜单或搜索中选择一种语言会以该语言打开同一页面，并记为 `kaeso-docs:locale`。主题沿用库的主题：跟随系统、浅色或深色，由主题脚本在首次绘制之前应用。

每个错误都是一条提示消息：chunk 无法加载的页面会保留框架，显示带有“重试”的空状态，并在提示消息中说明发生了什么；无法加载的搜索会关闭并显示一条提示消息。

## 设计选择

- **用 frontmatter，而不是侧边栏文件。** 页面的位置由其文件夹和 `order` 决定；分区的名称和位置由其 `index.md` 决定。在 GitHub 上移动或添加页面时，每种语言只涉及一个文件，从不需要另一个必须保持一致的文件。
- **开头的 `# Title`。** GitHub 会把 frontmatter 显示为一个小表格，因此页面可以用 `# ` 加标题开头，以便在 GitHub 上阅读；站点会去掉这个标题。其他任何 `#` 标题都是错误。
- **标注块。** GitHub 的五种提示块会变成 `Banner`：Note 为中性，Tip 为成功，Important 为信息，Warning 和 Caution 为警告。Caution 不使用危险色调，因为危险横幅带有 `role="alert"`，屏幕阅读器会在页面打开时朗读它。
- **表格**至少 640px 宽，放在水平 `ScrollArea` 中：在手机上横向滚动，而不是把每一列挤成每行一个词。
- **不使用 TanStack Query。** 站点除了自己的 chunk 外不获取任何内容，这些 chunk 由路由器的加载器加载并保留（`defaultStaleTime: Infinity`）。
- **有意不支持：** 页面中的原始 HTML、MDX 或组件，脚注，来自其他站点的图片，链接引用定义。
- **代码**会按库支持的语言高亮（TypeScript、TSX、JavaScript、JSON、Bash、Python、YAML、diff 及其别名）；其他语言在同样的代码块中显示为纯文本。
- **统一版本。** 页面描述的是最新发布；没有按版本区分的副本。

## 构建产物

`pnpm --filter kaeso-docs build` 写出 `apps/docs/dist`：应用本身；每种语言中每个页面的 `<path>.html`（`/de/console/api-keys` 对应 `de/console/api-keys.html`）；每种语言一个标记为 `noindex` 的 `404.html`；列出每种语言的每个页面及其 `hreflang` 备用链接的 `sitemap.xml`；允许抓取全部内容并指向站点地图的 `robots.txt`；`site.webmanifest`、图标、`og.png` 和 `CNAME`。页面的 head 包含其 `lang`、`<title>` 和描述、规范地址、三种语言加 `x-default` 的 `hreflang` 备用链接、带 `og.png` 的 Open Graph 和 Twitter 标签、其 `og:locale` 以及作为备用的另外两种语言，还有结构化数据：每种语言的首页为 `WebSite`，其他页面为 `TechArticle`。站点地图不含 `lastmod`，因为构建并不知道页面上次修改的时间。任何静态托管都能提供服务：对 `/console/api-keys` 的请求得到 `console/api-keys.html`，未知地址得到 `404.html`，它在框架中显示“未找到”。由维护者将其发布到 docs.kaeso.app。

`vite.config.ts` 做的事与控制台的相同：把从 `@logicplanes/kaeso-ui` 的导入改写为库中各组件的入口（`uiEntryImports`），并将主题脚本作为 `/theme-init.js` 提供；它还会添加 `theme-color` meta 标签，并预加载界面字体。构建最后运行 `scripts/budget.ts`，按[打包体积预算](../performance.md#文档站点)检查框架和每个页面；再运行站点检查（`scripts/check-site.mjs`，见[网站](../releasing.md#网站)）；之后 `pnpm --filter kaeso-docs licenses:check`（`scripts/licenses.ts`）检查其中每个包的许可证。
