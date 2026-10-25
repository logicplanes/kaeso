---
title: 翻译
description: 控制台、UI 库、邮件和文档如何使用英语、德语和简体中文。
order: 6
---

Kaeso 向用户展示的每个词都有英语、德语和简体中文版本（`en`、`de`、`zh-CN`）。英语是源语言：修改英语文本时，要在同一个拉取请求中附上对应的德语和中文，缺少任何一种时检查都会失败。

| 部分 | 文案所在位置 | 检查方式 |
| --- | --- | --- |
| 控制台 | `apps/console/src/i18n/locales/<locale>/<area>.json` | `pnpm i18n`、`pnpm lint` |
| UI 库 | `packages/ui/src/i18n/locales/` | 库的测试 |
| 展示站点 | `apps/showcase/src/i18n/` 以及 `apps/showcase/src/lib/locale.tsx` 中的 `DOCS_TEXT` | `pnpm --filter kaeso-showcase gen` |
| 邮件 | `crates/kaeso-mail/src/templates.rs` 中每封邮件各一个函数 | 邮件 crate 的测试（[邮件](architecture/emails.md#语言)） |
| 本文档 | `apps/docs/content/<locale>/` 和 `apps/docs/src/i18n/messages` | `check:content` 和类型检查（[编写文档](documentation.md#翻译)） |

网关用英语撰写自己的应答，例如错误消息和权限名称；控制台会翻译它认识的部分（[网关写入的内容](#网关写入的内容)）。

## 写作风格

每种语言都使用同样的语气：简短、平实、没有赘语。翻译意思而不是逐词翻译，并保持与英语一样简短。

| | 英语 | 德语 | 简体中文 |
| --- | --- | --- | --- |
| 称呼 | 不称呼，或用“you” | 用“Sie”及“Ihr”“Ihre”“Ihnen”，从不用“du”；标题、按钮、菜单项和简短提示保持不定式（“Schlüssel erstellen”“gleich erneut versuchen”） | 直接面向某个人时用“您”，从不用“你”；其他情况使用中性表达 |
| 按钮 | 动词在前：“Create key” | 不定式，动词在后：„Schlüssel erstellen“ | 动词在前：“创建密钥” |
| 标题 | 句首大写 | 遵循德语的大小写规则，其余按句子格式 | 末尾不加标点 |
| 省略号 | “Loading…”，一个字符 | „Wird geladen…“ | “加载中…” |
| 数字和日期 | 来自 `@logicplanes/kaeso-ui/format`，从不手动格式化 | 同左 | 同左 |
| 引号 | "…" | „…“ | “…” |
| 标点 | | | 全角（，。：；？（）），前后不加空格；中文与拉丁字母、数字或代码之间加一个空格（`3 个密钥`、`API 密钥`） |

德语通常比英语长约三分之一。布局必须容纳得下：标签不被截断，描述仍放在一行内，按钮保持宽度，表格不溢出。中文更短，但需要完整的行高；绝不能把它缩小。德语保留 Token、Playground 和 Budget 等约定俗成的术语，而不是为它们另造德语词。

以下内容从不翻译：

- 产品和服务商名称（Kaeso、OpenAI、Anthropic、models.dev）
- 模型 ID、各类 ID 和密钥前缀
- 代码、设置（`KAESO_*`）、路径和字段名
- 权限（`keys.all.view`）
- HTTP 方法和状态码
- 由库格式化的单位
- 人名和组织名称

## 术语表

| 英语 | 德语 | 简体中文 |
| --- | --- | --- |
| organization | Organisation | 组织 |
| project | Projekt | 项目 |
| member | Mitglied | 成员 |
| owner | Inhaber | 所有者 |
| admin | Admin | 管理员 |
| developer | Entwickler | 开发者 |
| viewer | Betrachter | 查看者 |
| auditor | Prüfer | 审计员 |
| role | Rolle | 角色 |
| right (permission) | Recht | 权限 |
| key | Schlüssel | 密钥 |
| API key | API-Schlüssel | API 密钥 |
| platform key | Plattform-Schlüssel | 平台密钥 |
| provider key | Anbieter-Schlüssel | 服务商密钥 |
| master key | Hauptschlüssel | 主密钥 |
| provider | Anbieter | 服务商 |
| model | Modell | 模型 |
| alias | Alias | 别名 |
| limit | Limit | 限额 |
| budget | Budget | 预算 |
| rate limit | Ratenlimit | 速率限制 |
| usage | Nutzung | 用量 |
| cost | Kosten | 费用 |
| request | Anfrage | 请求 |
| request logs | Anfrageprotokolle | 请求日志 |
| logs | Protokolle | 日志 |
| token | Token | Token |
| reasoning | Reasoning | 推理 |
| playground | Playground | 试验场 |
| service account | Dienstkonto | 服务账号 |
| invite, invitation | einladen, Einladung | 邀请 |
| sign in, sign out | anmelden, abmelden | 登录、退出登录 |
| account | Konto | 账户 |
| user | Benutzer | 用户 |
| session | Sitzung | 会话 |
| settings | Einstellungen | 设置 |
| instance | Instanz | 实例 |
| install (a Kaeso installation) | Installation | 实例 |
| active (key, account) | aktiv | 有效 |
| release | Release | 版本 |
| pre-release | Vorabversion | 预发布版本 |
| update | Update | 更新 |
| updater | Updater | 更新器 |
| catalogue | Katalog | 目录 |
| availability | Verfügbarkeit | 可用性 |
| identity provider | Identitätsanbieter | 身份提供商 |
| issuer URL | Issuer-URL | 颁发者 URL |
| holder of a role | Träger einer Rolle | 持有者 |
| inference | Inferenz | 模型调用 |
| two-factor authentication | Zwei-Faktor-Authentifizierung | 双重验证 |
| single sign-on (SSO) | Single Sign-on (SSO) | 单点登录（SSO） |
| audit log | Audit-Log | 审计日志 |
| agent (connected through the MCP server) | Agent | 智能体 |
| harness (the tool an agent runs in) | Harness | 运行环境 |
| preset (of an agent's rights) | Vorlage | 预设 |
| scheduled for deletion | zur Löschung vorgemerkt | 已计划删除 |
| restore | wiederherstellen | 恢复 |
| archive | archivieren | 归档 |
| revoke | widerrufen | 撤销 |
| disable | deaktivieren | 停用 |
| default | Standard | 默认 |
| streamed | gestreamt | 流式 |
| tooltip | Tooltip | 提示框 |
| toast | Toast | 提示消息 |
| label | Beschriftung | 标签 |
| button | Button | 按钮 |
| accessible name | zugänglicher Name | 无障碍名称 |
| panel | Panel | 面板 |
| sheet | Sheet | 侧滑面板 |
| pill | Pille | 胶囊 |
| toolbar | Werkzeugleiste | 工具栏 |
| when you control it | bei gesteuertem Wert/Zustand | 受控时 |
| on first render | beim ersten Rendern | 首次渲染时 |

## 控制台

### 使用哪种语言

- 用户在**账户** › **外观** › **语言**中选择：自动、English、Deutsch 或简体中文。所做的选择保存在其账户中（通过带 `locale` 的 `PATCH me` 写入，从 `auth/session` 读取），因此在每个浏览器中都会生效。
- **自动**是默认选项，它取浏览器语言中控制台支持的第一个（`navigator.languages`：`de-AT` 是德语，任何中文都是简体中文），否则回退到英语，并在浏览器语言改变时随之改变。
- 浏览器会保存一份账户选择的副本（`kaeso:locale`），因此首次绘制（包括登录页）就已经使用该语言。英语不需要额外加载任何内容；德语和中文会在首次绘制之前加载各自的文案目录（控制台的和库的，各一个 chunk）。
- 如果会话报告了在另一个浏览器中做出的选择，`src/i18n/sync.tsx` 会切换到该语言。切换时控制台会以新语言重新绘制；页面、其数据和所在位置都保持不变。

### 编写文本

```tsx
import { t } from "~/i18n";
import { tRich } from "~/i18n/rich";

<Button>{t("keys.create", "Create key")}</Button>;
toast.success(t("keys.revoked", "{name} revoked", { name: key.name }));
t("keys.count", "{count, plural, one {# key} other {# keys}}", { count });
tRich("signIn.newHere", "New here? {link}", {
  link: (
    <Link href="/sign-up">
      {t("signIn.createAccount", "Create an account")}
    </Link>
  ),
});
```

- **`t(key, english, values?)`** 返回一个字符串。在渲染时或需要文本时（提示消息、标题）调用它，从不在模块顶层调用。导入时计算出的常量会停留在页面启动时的语言。标签表格要写成函数（`() => t(…)`），或者在组件内部构建。
- **键和英语原文都是字符串字面量**，并排书写。键的形式是 camelCase 的 `<area>.<words>`，例如 `org.members.invite` 或 `logs.inspector.overview`。其第一部分指明文案目录文件（`src/i18n/locales/<locale>/<area>.json`）：`common`、`shell`、`auth`、`errors`、`rights`、`format`、`kaeso`、`signIn`、`public`、`account`、`org`、`orgHome`、`instance`、`project`、`keys`、`limits`、`logs`、`models`、`playground`。键表示文本的含义，而不是它所在的位置；一个键始终只对应一种英语原文，含义改变时就用新键。
- **共用词语**位于 `common`（`src/i18n/common.ts` 列出了它们）：Cancel、Save、Delete、Continue、Name、Email、Never 等。调用对应的函数，例如 `common.cancel()`，或使用 `t("common.cancel", "Cancel")`，英语原文必须完全一致。在其所在位置含义不同的词使用自己的键。
- **只用完整的句子。** 从不拼接片段（`"Joined " + date`、`` `${n} keys` ``），因为各语言的语序不同。值放入占位符（`{name}`）；决定词形的数字通过 `plural`（`one`、`other`、`=0`；德语有相同的形式，中文只有 `other`）；变体通过 `select`。数字、金额和日期在传入之前先用 `@logicplanes/kaeso-ui/format` 格式化；这些格式化函数会跟随语言。
- **属于元素的部分**，例如链接或加粗的名称，作为 `tRich` 的占位符，`tRich` 返回节点。
- `aria-label`、`title`、占位文本和页面标题（`usePageTitle`）与其他文本一样处理。

### 网关写入的内容

网关的错误消息、权限目录中的名称、描述和类别，以及内置角色的名称和描述，都以英语送达。API 客户端会在应答到达时翻译它认识的内容（`src/api/`，键为 `errors.*` 和 `rights.*`）。它按 ID 翻译权限和角色，按英语原文及其值翻译消息。它不认识的内容按网关的原文显示。

切换语言会重新加载数据。当网关写出的消息在 `src/api/gateway-errors.ts` 中没有对应规则时，`crates/kaeso-server/tests/error_copy.rs` 会失败，因此新消息要在同一个拉取请求中附上德语和中文。

## UI 库

- `LocaleProvider`（`@logicplanes/kaeso-ui/i18n`）为其下的所有内容设置语言：库自身的文本，以及 `@logicplanes/kaeso-ui/format` 书写数字和日期的方式。
- 组件通过 `useUiText` 以 `t("dialog.close", "Close")` 的形式写出文本。英语原文紧挨着键，因此英语页面只携带它所用组件的文本。
- 德语和中文位于 `packages/ui/src/i18n/locales/de.ts` 和 `zh-CN.ts`，`loadUiMessages(locale)` 按需加载其中之一；英语不加载任何内容。`en.ts` 为翻译人员列出每个键及其英语原文，一个测试让它与组件保持同步，并检查两种译文都包含每个键且占位符相同。
- 键的形式是 camelCase 的 `component.meaning`：`dialog.close`、`datePicker.last7Days`、`list.clearSelection`。占位符遵循 ICU MessageFormat 的通用子集：`{name}`、`{count, plural, one {# key} other {# keys}}`、`{kind, select, … other {…}}`。

### 在组件中

```tsx
import { useUiText } from "../../i18n/context";

export function CopyButton({ value, label: labelProp }: CopyButtonProps) {
  const t = useUiText();
  const label = labelProp ?? t("copyButton.copy", "Copy");
  return (
    <Button
      shape="circle"
      icon={icons.copy}
      aria-label={label}
      onClick={() => copy(value)}
    />
  );
}
```

- 从 `../../i18n/context` 导入 `useUiText` 和 `useLocale`，从不从索引文件导入：context 模块不包含格式化函数，因此组件的打包体积保持很小。
- 英语原文是同一调用中的普通字符串字面量；文案目录测试会读取源码中的每个 `t("key", "English")`。
- 用户看到或听到的一切都经过 `t`：可见文本、`aria-label`、标题、占位文本、只提供给屏幕阅读器的文本、提示消息和空状态。应用传入的数据（名称、角色、权限）和产品名称保持原样。
- 有默认值的文本属性，例如 `emptyText`，在其签名中不设默认值；组件使用 `emptyText ?? t("combobox.noResults", "No results")`，因此默认值会跟随语言，而应用自己的文本优先。
- 计数是一条带复数形式的消息（`t("list.selected", "{count} selected", { count })`），从不由两次调用拼接而成。作为值传入的数字会按语言书写（1,234 或 1.234）。
- 星期和月份名称以及日期来自按 `useLocale()` 的语言调用的 `Intl`，或来自 `@logicplanes/kaeso-ui/format`，从不来自源码中的英语名称。
- 在文本变化时保持宽度的位置（`Button` 的 `reserveLabels`、`SwapText`）会收到每个备选文本的译文，从而按当前语言中最宽的那个预留宽度。
- 不在模块顶层做任何工作：标签列表，例如日期选择器的预设，是 `t` 的函数，在渲染时调用。React 之外写出文本的辅助函数（`keyLabel`、`liveLine`）把组件的 `t` 作为最后一个参数。
- 名称列表用该语言的分隔符连接（`t("rightsMatrix.listSeparator", ", ")`，在中文中为“、”）。

### 在应用中

```tsx
import {
  LocaleProvider,
  loadUiMessages,
  resolveUiLocale,
  type UiMessages,
} from "@logicplanes/kaeso-ui/i18n";

const locale = resolveUiLocale(navigator.language);
const messages: UiMessages = await loadUiMessages(locale);

root.render(
  <LocaleProvider locale={locale} messages={messages}>
    <App />
  </LocaleProvider>,
);
```

- 没有 `LocaleProvider` 时，一切都是英语。
- `resolveUiLocale` 把 BCP 47 标签映射到已提供的语言：`de-AT` 是 `de`，`zh-Hans` 是 `zh-CN`，其他都是 `en`。`loadUiMessages` 对英语返回 `{}`，不加载任何内容。
- 要切换语言，先加载新的文案目录，再同时用新语言和新目录渲染 provider；在新目录到达之前，保留旧的一对。
- provider 还会设置格式化函数（`formatNumber`、`formatDate`、`formatRelative`、`formatUSD` 等）的语言，这些都是普通函数；`setFormatLocale(locale, messages)` 在 React 之外做同样的事。应用自己的文本使用它自己的文案目录，`formatMessage(template, values, locale)` 以同样的方式格式化 ICU 消息。

## 展示站点

- 展示站点的每个页面都以每种语言构建：英文位于 `/components/button`，德文位于 `/de/components/button`，中文位于 `/zh-CN/components/button`。外框、首页网格、指南以及每个组件和属性的描述从首次绘制起就使用页面的语言。
- 侧边栏中的语言控件会以另一种语言打开同一页面，所做的选择会被记住。英文地址先跟随记住的选择，再跟随浏览器的语言偏好；德文或中文地址始终显示该语言。`?lang=de`、`?lang=zh-CN` 和 `?lang=en` 会被记住，并重定向到该语言的地址。
- 外框的文案位于 `DOCS_TEXT`（`apps/showcase/src/lib/locale.tsx`）。其他所有文本位于 `apps/showcase/src/i18n/de.json` 和 `zh-CN.json`，以合并空白后的英语原文为键。
- `pnpm --filter kaeso-showcase gen` 会失败并列出每个缺少德语或中文条目的文本，因此新的描述无法只以英语发布。
- 组件预览中的演示是代码，保持英语；名称（`Button`、`formatNumber`）和代码从不翻译。

## 检查

- `pnpm i18n --write` 根据源码写出控制台的 `src/i18n/locales/en/<area>.json`，这就是翻译人员的清单；`pnpm i18n` 检查每个键都有德语和中文且占位符相同，并且没有多余的条目。它会拒绝仍是英语的译文，但名称、代码、缩写以及“Top k”等少数术语可以保留英语。它还会拒绝德语中的“du”、中文中的“你”、直双引号，以及紧挨中文字符的半角标点。`pnpm test` 也会运行这项检查。
- UI 库的目录测试、展示站点的 `gen` 和文档站点的文案测试也会拒绝同样的问题：译文中残留的英语和非正式称呼。德语或中文邮件保留英语句子，或使用“du”或“Tokens”时，邮件 crate 的测试会失败。
- `pnpm lint` 拒绝直接写在控制台中的文本：JSX 文本、文本属性（`label`、`title`、`description`、`placeholder`、`aria-label` 等）、对象的文本字段，以及以字面量写出的提示消息（规则 `kaeso/no-literal-text`）。
- `apps/console/src/copy.test.ts` 和 `apps/console/src/toast-copy.test.ts` 按与英语相同的长度限制检查德语和中文的描述和提示消息，并留有 15% 的余量。
- 用德语和中文、在手机和桌面宽度、两种主题下查看修改过的每个页面：在**外观**下选择语言，或者在浏览器存储中设置 `kaeso:locale`。在展示站点中，查看修改过的组件的 `/de/components/<slug>/states` 和 `/zh-CN/components/<slug>/states`。
