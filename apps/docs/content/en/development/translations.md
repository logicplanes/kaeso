---
title: Translations
description: How the console, the UI library, the emails and the documentation speak English, German and Simplified Chinese.
order: 6
---

Every word Kaeso shows a person exists in English, German and Simplified Chinese (`en`, `de`, `zh-CN`). English is the source: a change to an English text comes with its German and Chinese in the same pull request, and the checks fail when one is missing.

| Part | Where its words live | Checked by |
| --- | --- | --- |
| The console | `apps/console/src/i18n/locales/<locale>/<area>.json` | `pnpm i18n`, `pnpm lint` |
| The UI library | `packages/ui/src/i18n/locales/` | The library's tests |
| The showcase | `apps/showcase/src/i18n/` and `DOCS_TEXT` in `apps/showcase/src/lib/locale.tsx` | `pnpm --filter kaeso-showcase gen` |
| Emails | One function per email in `crates/kaeso-mail/src/templates.rs` | The mail crate's tests ([emails](architecture/emails.md#languages)) |
| This documentation | `apps/docs/content/<locale>/` and `apps/docs/src/i18n/messages` | `check:content` and the typecheck ([writing documentation](documentation.md#translating)) |

The gateway writes its own answers, such as error messages and the names of rights, in English; the console translates the ones it knows ([what the gateway writes](#what-the-gateway-writes)).

## House style

Every language has the same voice: short, plain, no filler. Translate the meaning, not the words, and keep it as short as the English.

| | English | German | Simplified Chinese |
| --- | --- | --- | --- |
| Address | None, or "you" | "Sie" with "Ihr", "Ihre", "Ihnen", never "du"; headings, buttons, menu items and short hints stay in the infinitive ("Schlüssel erstellen", "gleich erneut versuchen") | "您" where a person is addressed, never "你"; otherwise neutral phrasing |
| Buttons | A verb first: "Create key" | Infinitive, verb last: „Schlüssel erstellen“ | A verb first: "创建密钥" |
| Titles | Sentence case | German capitalization, otherwise sentence style | No trailing punctuation |
| Ellipsis | "Loading…", one character | „Wird geladen…“ | "加载中…" |
| Numbers and dates | From `@logicplanes/kaeso-ui/format`, never formatted by hand | The same | The same |
| Quotes | "…" | „…“ | “…” |
| Punctuation | | | Full-width (，。：；？（）) with no space around it; a space between Chinese and Latin words, digits or code (`3 个密钥`, `API 密钥`) |

German runs about a third longer than English. Layouts must hold it: labels aren't cut short, descriptions still fit on one line, buttons keep their width and tables don't overflow. Chinese is shorter but needs the full line height; never make it smaller. German keeps established terms such as Token, Playground and Budget rather than inventing German words for them.

Never translated: product and provider names (Kaeso, OpenAI, Anthropic, models.dev), model ids, ids and key prefixes, code, settings (`KAESO_*`), paths, field names, rights (`keys.all.view`), HTTP methods and status codes, units the library formats, and the names of people and organizations.

## Glossary

| English | German | Simplified Chinese |
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
| sign in, sign out | anmelden, abmelden | 登录, 退出登录 |
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

## The console

### Which language

- A person chooses under **Settings** › **Account** › **Appearance** › **Language**: Automatic, English, Deutsch or 简体中文. The choice is stored on their account (`PATCH me` with `locale`, read from `auth/session`), so it follows them to every browser.
- **Automatic**, the default, takes the first of the browser's languages the console has (`navigator.languages`; `de-AT` is German, any Chinese is Simplified Chinese), falls back to English, and follows the browser when its language changes.
- The browser keeps a copy of the choice (`kaeso:locale`), so the first paint, sign-in included, is already in that language. English loads nothing more; German and Chinese load their catalogs, the console's and the library's, one chunk each, before the first paint.
- When the session reports a choice made in another browser, `src/i18n/sync.tsx` switches to it. Switching draws the console again in the new language; the page, its data and its place stay.

### Writing text

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

- **`t(key, english, values?)`** returns a string. Call it while rendering or when the text is needed (a toast, a title), never at module level: a constant computed on import stays in the language the page started in. Tables of labels become functions (`() => t(…)`) or are built inside the component.
- **The key and the English are string literals**, side by side. The key is `<area>.<words>` in camelCase, such as `org.members.invite` or `logs.inspector.overview`. Its first part names the catalog file, `src/i18n/locales/<locale>/<area>.json`: `common`, `shell`, `auth`, `errors`, `rights`, `format`, `kaeso`, `signIn`, `public`, `account`, `org`, `orgHome`, `instance`, `project`, `keys`, `limits`, `logs`, `models` or `playground`. A key names what the text means, not where it sits. One key always has one English; a changed meaning gets a new key.
- **Shared words** such as Cancel, Save, Delete, Continue, Name, Email and Never are in `common` (`src/i18n/common.ts`). Call its function, such as `common.cancel()`, or `t("common.cancel", "Cancel")` with exactly that English. A word that means something else in its place gets its own key.
- **Whole sentences only.** Never join fragments (`"Joined " + date`, `` `${n} keys` ``), because word order differs between languages. Values go into placeholders (`{name}`); a number that chooses a form goes through `plural` (`one`, `other`, `=0`; German has the same forms, Chinese only `other`); variants go through `select`. Format numbers, money and dates with `@logicplanes/kaeso-ui/format` before passing them in; the formatters follow the language.
- **Parts that are elements**, such as a link or a bold name, are placeholders of `tRich`, which returns nodes.
- `aria-label`s, `title`s, placeholders and the page title (`usePageTitle`) are text like any other.

### What the gateway writes

The gateway's error messages, the names, descriptions and categories in the rights catalogue, and the built-in roles' names and descriptions arrive in English. The API client (`src/api/`) translates the ones it knows as answers arrive, with the keys `errors.*` and `rights.*`: rights and roles by their id, messages by their English and their values. Anything it doesn't know is shown as the gateway wrote it. Switching languages loads the data again. `crates/kaeso-server/tests/error_copy.rs` fails when the gateway writes a message without a rule in `src/api/gateway-errors.ts`, so a new message comes with its German and Chinese in the same pull request.

## The UI library

- `LocaleProvider` (`@logicplanes/kaeso-ui/i18n`) sets the language for everything under it: the library's own text, and how `@logicplanes/kaeso-ui/format` writes numbers and dates.
- A component writes its text as `t("dialog.close", "Close")` with `useUiText`. The English sits beside the key, so a page in English carries only the text of the components it uses.
- German and Chinese live in `packages/ui/src/i18n/locales/de.ts` and `zh-CN.ts`, and `loadUiMessages(locale)` loads one on demand; English loads nothing. `en.ts` lists every key with its English for translators. A test keeps it in step with the components and checks that both translations have every key with the same placeholders.
- Keys are `component.meaning` in camelCase: `dialog.close`, `datePicker.last7Days`, `list.clearSelection`. Placeholders follow the common subset of ICU MessageFormat: `{name}`, `{count, plural, one {# key} other {# keys}}`, `{kind, select, … other {…}}`.

### In a component

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

- Import `useUiText` and `useLocale` from `../../i18n/context`, never from the index: the context module carries no formatters, so a component's bundle stays small.
- The English is a plain string literal in the same call; the catalog test reads every `t("key", "English")` in the source.
- Everything a person sees or hears goes through `t`: visible text, `aria-label`, titles, placeholders, text only screen readers get, toasts and empty states. Data the app passes in (names, roles, rights) and product names stay as given.
- A text prop with a default, such as `emptyText`, has no default in its signature. The component uses `emptyText ?? t("combobox.noResults", "No results")`, so the default follows the language and an app's own text wins.
- A count is one message with a plural (`t("list.selected", "{count} selected", { count })`), never two calls joined. A number passed as a value is written for the language (1,234 or 1.234).
- Weekday and month names and dates come from `Intl` for `useLocale()` or from `@logicplanes/kaeso-ui/format`, never from English names in the source.
- Places that keep their width while their text changes (`Button`'s `reserveLabels`, `SwapText`) get every alternative translated, so the widest one in the current language is reserved.
- No work at module level: a list of labels, such as the date picker's presets, is a function of `t`, called while rendering. A helper outside React that writes text (`keyLabel`, `liveLine`) takes the component's `t` as its last argument.
- A list of names is joined with the language's separator (`t("rightsMatrix.listSeparator", ", ")`, which is "、" in Chinese).

### In an app

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

- Without a `LocaleProvider`, everything is English.
- `resolveUiLocale` maps a BCP 47 tag to a shipped language: `de-AT` is `de`, `zh-Hans` is `zh-CN`, anything else `en`. `loadUiMessages` returns `{}` for English and loads nothing.
- To change the language, load the new catalog, then render the provider with the new language and catalog together; until the catalog arrives, keep the old pair.
- The provider also sets the language of the formatters (`formatNumber`, `formatDate`, `formatRelative`, `formatUSD` and others), which are plain functions; `setFormatLocale(locale, messages)` does the same without React. An app's own text uses its own catalogs, and `formatMessage(template, values, locale)` formats ICU messages the same way.

## The showcase

- Every showcase page is built in each language: English at `/components/button`, German at `/de/components/button`, Chinese at `/zh-CN/components/button`. The chrome, the home grid, the guides and every component and prop description are in the page's language from the first paint.
- The sidebar's language control opens the same page in another language and is remembered. An English address follows the remembered choice, then the browser's languages; a German or Chinese address always shows that language. `?lang=de`, `?lang=zh-CN` and `?lang=en` are remembered and redirect to that language's address.
- The chrome's words live in `DOCS_TEXT` (`apps/showcase/src/lib/locale.tsx`). All other text lives in `apps/showcase/src/i18n/de.json` and `zh-CN.json`, keyed by the English with whitespace collapsed.
- `pnpm --filter kaeso-showcase gen` fails and lists every text without a German or Chinese entry, so a new description can't ship in English alone.
- The demos inside component previews are code and stay English, as do names such as `Button` and `formatNumber`.

## Checking

- `pnpm i18n --write` writes the console's `src/i18n/locales/en/<area>.json` from the source; that is the translators' list. `pnpm i18n` checks that every key has German and Chinese with the same placeholders and that nothing is left over, and refuses a translation that is still the English (names, codes, acronyms and a short list of terms such as "Top k" stay), "du" in German, "你" in Chinese, straight double quotes, and half-width punctuation next to Chinese characters; `pnpm test` runs the same check.
- The UI library's catalog test, the showcase's `gen` and the documentation site's message test refuse the same: English left in a translation and an informal address. The mail crate's tests fail when a German or Chinese email keeps an English sentence or uses "du" or "Tokens".
- `pnpm lint` refuses text written straight into the console (the rule `kaeso/no-literal-text`): JSX text, text props (`label`, `title`, `description`, `placeholder`, `aria-label` and others), text fields of objects, and toasts written as literals.
- `apps/console/src/copy.test.ts` checks German and Chinese descriptions against the same length limits as English, with 15% slack.
- Look at every page you changed in German and Chinese, at phone and desktop widths, in both themes. Choose the language under **Appearance**, or set `kaeso:locale` in the browser's storage. In the showcase, open a changed component's `/de/components/<slug>/states` and `/zh-CN/components/<slug>/states`.
