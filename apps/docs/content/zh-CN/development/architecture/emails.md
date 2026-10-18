---
title: 邮件
description: 网关如何构建它发送的邮件，供修改 kaeso-mail crate 时参考。
order: 4
---

Kaeso 发送的每封邮件都来自 `crates/kaeso-mail`：一套布局、一份由 Kaeso 令牌构建的样式表，以及每封邮件一个函数，提供英语、德语和简体中文。[电子邮件](../../self-hosting/email.md)介绍运维人员需要设置和会看到的内容；本页介绍代码。

## 邮件的构成

| 部分 | 登录邮件中的内容 | 样式 |
| --- | --- | --- |
| 文字标志 | Kaeso | 单独位于左上角，距左 28px、距上 28px |
| 标题 | Sign in to Kaeso | `title-lg` |
| 一句话 | The link works once and expires in 15 minutes. | `body-lg`，次要色 |
| 一个按钮 | Sign in | `xl` 胶囊，白底墨色 |
| 一行弱提示 | If you did not ask to sign in, you can ignore this email. | `small`，第三级颜色 |
| 页脚 | Sent by Kaeso at kaeso.example.com. | `caption` |

- **没有卡片。** 单个左对齐的栏，最宽 562px，直接放在画布上：浅色模式为白色，深色模式为黑色。
- **长度限制。** 主题最多 60 个字符且不带句号，标题最多 48 个字符，句子最多 100 个字符。德语和中文多 15%：69、55 和 115。由测试强制执行。
- **不显示原始链接。** HTML 中从不显示 URL，由按钮承载它。纯文本部分在单独一行给出完整链接，用词和顺序与 HTML 相同。
- **页脚。** “Sent by Kaeso at” 加上实例的主机名，来自 `KAESO_PUBLIC_URL`。通知还会说明收到它的原因：安全和组织通知总会发送，提醒则说明如何关闭。

每封邮件的文本和 HTML 都是 `crates/kaeso-mail/src/snapshots/` 中的快照。

## 构建方式

| 文件 | 职责 |
| --- | --- |
| `templates.rs` | 每封邮件一个函数，构成 [Kaeso 发送的邮件](../../self-hosting/email.md#kaeso-发送的邮件)的目录：每种语言的主题、标题、句子、按钮、弱提示以及发送原因 |
| `locale.rs` | `Locale`（`en`、`de`、`zh-CN`），以及各语言书写日期和数字的方式 |
| `layout.rs` | 共用的布局，包括用 maud 生成的 HTML 和纯文本 |
| `style.rs` | `token!` 宏中 `@logicplanes/kaeso-tokens` 的值、每个角色的内联样式和样式表。布局从不自己写颜色或尺寸 |
| `assets.rs`、`assets/` | 文字标志图片及其 SVG 源文件 |
| `mailer.rs` | 最多 1,024 封邮件的队列、分别在 1、5 和 30 秒后的三次重试、SMTP、日志和内存三种传输方式，以及 MIME 结构 |

- **适合邮件的 HTML。** 用表格搭建结构，能写成内联样式的都写成内联样式。`<style>` 块只包含配色方案、重置样式和深色模式。条件注释为 Windows 版 Outlook 提供固定的 562px 栏宽。不使用网络字体、外部图片、脚本或跟踪像素。
- **按钮**是一个带内边距和全圆角的链接。Windows 版 Outlook 无法给链接加内边距或圆角，所以为它使用 VML `roundrect`，宽度根据标签计算，因此从不换行。
- **转义。** maud 会在文本和属性中转义用户选择的名称，例如组织、密钥和项目。唯一不转义的标记是固定的：doctype、Outlook 的条件注释，以及会转义所嵌入内容的 VML 按钮。测试会把标记和引号放进每个名称中。
- **MIME。** `multipart/alternative`，纯文本在前；HTML 部分通过 `Content-ID` 内嵌两张文字标志 PNG，所以邮件应用不会把它们列为附件。

## 语言

`kaeso-server` 中的 `services::mail_lang` 选择语言。用户在账户设置中的选择优先（`users.locale`；`null` 表示自动）。否则取触发这封邮件的请求的 `Accept-Language` 中第一种 Kaeso 支持的语言。数据库无法应答时由回退规则决定，所以这次查询永远不会阻止邮件发送。[电子邮件](../../self-hosting/email.md#语言)列出了每种收件人的规则。

每封邮件的函数对语言做一次 `match`，写完整的句子，并按名称填入值，从不拼接片段。德语用 “Sie” 称呼对方。中文使用“您”和全角标点，并在中文与拉丁字母或数字之间加空格，由 `locale::zh` 在衔接处添加。日期和数字遵循各自的语言：

| | 英语 | 德语 | 中文 |
| --- | --- | --- | --- |
| 日期 | `2026-10-14` | `14. Okt. 2026` | `2026年10月14日` |
| 金额 | `1250` | `1.250` | `1,250` |
| 百分比 | `80%` | `80 %` | `80%` |

## 文字标志

邮件以两张内嵌 PNG 携带文字标志，浅色和深色各一张，合计约 6 KB。自托管实例的地址可能是内部地址，而且许多邮件应用会屏蔽远程图片，所以链接的图片可能无法显示。每张 PNG 都是显示尺寸的三倍，由 `assets/wordmark-light.svg` 和 `assets/wordmark-dark.svg` 渲染而成。一个测试会用 resvg 重新渲染这些 SVG，与提交的 PNG 不一致时就会失败。重新生成它们：

```bash
KAESO_UPDATE_MAIL_ASSETS=1 cargo test -p kaeso-mail assets
```

## 深色模式

Kaeso 自己设定深色模式，而不是交给各个邮件应用自动反色。邮件应用分为三类，每类各有一层处理：

1. **遵循 `prefers-color-scheme` 的应用**，例如 Apple Mail 和 Thunderbird。文档声明支持两种配色方案，所以它们不会反色；`@media (prefers-color-scheme: dark)` 用 `!important` 把每个角色切换为深色令牌。
2. **Outlook.com 和新版 Outlook**，它们会自行调暗邮件，并用 `data-ogsc` 和 `data-ogsb` 标记重新着色的元素。这些属性下的同一套规则用 Kaeso 的深色值替换它们的猜测。
3. **不经询问就反色的应用**，例如 Gmail 应用。浅色设计经得起反色：接近黑色的文字、反色后变成深色主题浅色胶囊的按钮、浅色文字标志周围的白色描边，以及始终隐藏的深色文字标志。

样式表与内联样式来自同一个令牌宏。测试检查每条深色规则都带 `!important`，并且每个角色在每一层都有规则。

## 预览邮件

[开发栈](../stack.md)包含 Mailpit，它在 http://127.0.0.1:58025 上的网页界面提供 HTML 兼容性检查和深色预览。把每种语言的每封邮件发送到那里：

```bash
cargo test -p kaeso-mail --test smtp
```

该测试检查每封邮件到达时都带有两个部分和两张内嵌图片，并检查 Mailpit 的 HTML 检查报告的邮件应用支持度足够。之后它会删除自己的邮件。`KAESO_TEST_SMTP_URL` 和 `KAESO_TEST_MAIL_API` 可以让它指向别处。
