---
title: 邮件
description: 网关如何构建它发送的邮件，面向 kaeso-mail crate 的贡献者。
order: 4
---

Kaeso 发送的每封邮件都来自网关的 `crates/kaeso-mail`：一个布局、一份由 Kaeso 设计令牌构建的样式表，以及每封邮件各一个函数，用英语、德语和简体中文写明这封邮件的内容。运维人员需要设置和看到的内容见[电子邮件](../../self-hosting/email.md)；本页介绍的是设计及其背后的理由。

## 邮件的构成

```text
KAESO                                   the wordmark alone, top left, 28px in and 28px down

Sign in to Kaeso                        heading: title-lg, 28/34, semibold
The link works once and expires in      one sentence: body-lg, 15/22, secondary
15 minutes.

( Sign in )                             one button: the xl pill, ink on white

If you did not ask to sign in, you      one quiet line: small, 13/18, tertiary
can ignore this email.

Sent by Kaeso at kaeso.example.com.     footer: caption, 12/16; the install, and why
```

- **没有卡片。** 画布上只有一列左对齐的文字，宽度最多 560px，与控制台一致：浅色模式下为白色，深色模式下为黑色。没有面板、没有边框，周围也没有灰色页面。
- **文案。** 简短的主题（最多 60 个字符，不带句号）、一个标题（最多 48 个字符）、最多一句话、最多一个按钮，以及最多一行次要说明，例如到期时间，或者如果不是本人操作该怎么办。句子遵循网关的文案规则：一句话，第二层意思用分号连接，以句号结尾，最多 100 个字符。由一个测试强制执行。
- **没有原始链接。** HTML 中从不显示 URL：URL 由按钮承载。纯文本部分在单独一行中给出完整链接，纯文本客户端、测试和控制台的端到端测试套件读取的就是这一部分。
- **页脚。** “Sent by Kaeso at”加上安装的主机名，取自 `KAESO_PUBLIC_URL` 并链接到该地址；对于总会发送的通知，还会说明发送原因：安全通知和组织通知会说明这一点，提醒邮件会说明如何关闭。
- **纯文本**以相同的顺序给出相同的文字，并以一行 `-- ` 签名结尾。

每封邮件渲染出的文本和 HTML 都作为快照保存在 `crates/kaeso-mail/src/snapshots/` 中。

## 构建方式

| 文件 | 职责 |
| --- | --- |
| `templates.rs` | 每封邮件一个函数，也就是该邮件的文案目录：每种语言的主题、标题、句子、按钮、次要说明和发送原因。`Email::render` 为某个安装渲染它。 |
| `locale.rs` | `Locale`（`en`、`de`、`zh-CN`）、如何选择语言，以及按各语言习惯书写的日期和数字。 |
| `layout.rs` | 共享布局的 HTML 版本（maud：编译时检查，每个值都会转义）和纯文本版本。 |
| `style.rs` | 来自 `@logicplanes/kaeso-tokens` 的设计令牌、在编译时由它们组合出的各角色内联样式，以及样式表。布局从不自行书写颜色或尺寸。 |
| `assets.rs`、`assets/` | 文字标志图片及其 SVG 源文件。 |
| `mailer.rs` | 队列、重试、SMTP 和 MIME 结构。 |

**适用于邮件的 HTML。** 用表格（`role="presentation"`）构建结构，能内联的样式全部内联，只有无法内联的部分才放在 `<style>` 块中：配色方案、样式重置和深色模式。在条件注释中为忽略 `max-width` 的 Outlook for Windows 提供固定 562px 的列，并使用 Segoe UI 字体和 96 DPI 图片。没有网页字体、外部 CSS 或图片、脚本或跟踪像素。字体栈与控制台相同，只是去掉了网页字体：已安装 Inter 时使用 Inter，否则使用各平台的界面字体。

**按钮**采用防弹设计：在所有地方都是一个带内边距、圆角半径为 9999px 的链接（一个 `xl` 胶囊：高 48px，左右内边距 22px，15px 中等字重），而对于无法为链接添加内边距或圆角的 Outlook for Windows，则使用弧度为 50% 的 VML `roundrect`。VML 需要固定宽度，宽度按标签每个字符 9px（每个中文字符 16px）计算，因此永不换行。

**隐藏的预览文本。** 即那句话，后面跟着不可见字符，这样收件箱列表会在主题之后显示这句话，而不是布局开头的文字。

**转义。** 用户自己选择的名称，例如组织、邀请人、密钥和项目名称，在文本和属性中都由 maud 转义。唯一不转义的标记是固定的：doctype、Outlook 的条件注释和 VML 按钮，VML 按钮会对其嵌入的 URL 和标签进行转义。测试会让标记和引号穿过每个名称。

**MIME。** `multipart/alternative`，纯文本在前，随后是一个 `multipart/related` HTML 部分，它以内联部分的形式通过 `Content-ID` 携带两张文字标志 PNG，且不带文件名，因此邮件应用不会将它们列为附件。

## 语言

每封邮件都有英语、德语和简体中文版本；各版本的设计相同，只有文字不同，HTML 的 `lang` 会注明是哪种语言。英语是源语言。

**使用哪种语言。** 由 `kaeso-server` 的 `services::mail_lang` 决定，依据是该用户在账户设置中的选择（`users.locale`，其中 `null` 表示自动）以及触发该邮件的请求的 `Accept-Language`：按偏好顺序取其中第一个 Kaeso 支持的语言，因此 `de-AT` 是德语，任何中文都是简体中文，任何英语都是英语。查询用户的选择从不会阻止邮件发送：如果数据库无法应答，就由后备规则决定。每类收件人的规则见[电子邮件](../../self-hosting/email.md#语言)。

**写作。** 每封邮件的函数对语言只做一次 `match`，其中是完整的句子，值按名称填入，从不拼接片段。术语表和行文风格见 [`@logicplanes/kaeso-ui` 的写作规则](https://github.com/logicplanes/kaeso/blob/dev/packages/ui/foundations/writing.md)。邮件以正式的口吻称呼收件人：德语在简短的句子中使用“Sie”；中文简洁，使用“您”和全角标点，并在中文与拉丁字母或数字之间加空格，这由 `locale::zh` 在衔接处添加，从不加在名称内部。德语和中文遵循文案规则，并采用控制台 15% 的宽松度：主题 69、标题 55、句子 115 个字符。由一个测试检查这些限制、布局的标签是否与英语一致，以及每种语言中每封邮件的主题和标题。

**日期和数字**遵循语言习惯：英语为 `2026-10-14`，德语为 `14. Okt. 2026`，中文为 `2026年10月14日`；金额为 `1250`、`1.250`、`1,250`；百分比为 `80%`、`80 %`、`80%`。

## 文字标志

界面在左上角单独显示文字标志，尺寸为 60×12（`Logo.Wordmark size="nav"`），邮件也是如此：图片为 64×16，带 2px 留白，放置在 26px 处，使字形从 28px 处开始，与下方文字对齐在同一条线上。

**内联传送，而非链接。** 自托管安装的公开 URL 可能是内部地址或未知地址，因此链接的图片可能无法加载，而且许多邮件应用默认屏蔽远程图片；内联部分总能显示，也不需要从安装获取任何内容。两张 PNG 合计约 6 KB。

**在任何屏幕上都清晰。** 每张 PNG 都是其显示尺寸的三倍（192×48），由 `assets/wordmark-light.svg` 和 `assets/wordmark-dark.svg` 渲染而来，其路径与 `@logicplanes/kaeso-ui` 中的相同。一个测试会用 resvg 重新渲染这些 SVG，当已提交的 PNG 与之不同时就会失败；`KAESO_UPDATE_MAIL_ASSETS=1 cargo test -p kaeso-mail assets` 会重新生成它们。两张图片都带有 `alt="Kaeso"` 以及 `width` 和 `height` 属性。

## 深色模式

Kaeso 自己控制深色模式，而不是交给各邮件应用的自动反色。三类邮件应用，对应三层处理：

1. **遵循 `prefers-color-scheme` 的应用**，例如 macOS 和 iOS 上的 Apple Mail、Outlook for Mac、iOS 和 Android、Thunderbird、Samsung Email 和 Hey。文档声明支持两种配色方案（`<meta name="color-scheme" content="light dark">`、`supported-color-schemes` 以及 `:root { color-scheme: light dark }`），告诉这些应用不要反转它，样式表中的 `@media (prefers-color-scheme: dark)` 规则则用 `!important` 将每个角色切换为其深色令牌，因为否则内联样式会优先：黑色画布、`#fafafa` 文字、`#a3a3a3` 和 `#858585` 的次要和第三级文字、`#fafafa` 底色配黑色文字的按钮，以及将浅色文字标志换成深色文字标志。
2. **Outlook.com 和新版 Outlook**，它们会自行将邮件变暗，并用 `data-ogsc`（颜色）和 `data-ogsb`（背景）标记被重新着色的元素。同样的规则写在 `[data-ogsc]` 和 `[data-ogsb]` 之下，用 Kaeso 的深色值替换它们的猜测，因此结果与第一层相同。
3. **不经询问就反色的应用**，例如 Gmail 应用、深色模式下的 Outlook for Windows，以及使用其他服务商账户的 Gmail（它会丢弃 `<style>`）。在这些应用中无法切换任何东西，因此浅色设计本身就经得起反色：
   - 文字是接近黑色的 `#0a0a0a`，从不使用纯黑，按钮的底色是 `#0a0a0a` 配白色文字，因此反色后会变成一个浅色胶囊配深色文字，正是深色主题自己的按钮；
   - 浅色文字标志带有 1.25px 的白色光晕，在白色背景上不可见；当背景被调暗而图片没有被调暗时，光晕会勾勒出字形的轮廓，使文字标志保持清晰可读；
   - 深色文字标志在条件注释中用 `display:none`、`max-height:0` 和 `mso-hide:all` 隐藏，因此永远不会单独出现。

样式表与内联样式由同一个令牌宏生成，测试会检查每条深色规则都是 `!important`，并且每个角色在每一层都有规则。

## 预览

[开发环境栈](../local-setup.md#3-启动开发环境栈)中包含 Mailpit，其 Web 界面位于 [http://127.0.0.1:58025](http://127.0.0.1:58025)，提供 HTML 兼容性检查和深色预览。`cargo test -p kaeso-mail --test smtp` 会以每种语言将每封邮件发送到 Mailpit，检查每封邮件到达时都带有两个部分和两张内联图片，并检查 Mailpit 的 HTML 检查（基于 caniemail 数据，按各邮件应用的份额加权）报告的支持率至少为 88%；之后它会删除这些邮件。`KAESO_TEST_SMTP_URL` 和 `KAESO_TEST_MAIL_API` 可以将它指向其他地方。

每封邮件的得分约为 91% 支持、8% 部分支持、1% 不支持。部分支持和不支持的条目都在预期之内：用于深色模式的 `@media`、`<style>` 和 `!important`，不支持它们的邮件应用由第三层处理代替；`border-radius`，Outlook for Windows 中由 VML 代替；以及 `<body>` 样式，由外层包装元素重复设置。
