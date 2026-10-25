---
title: Übersetzungen
description: Wie Konsole, UI-Bibliothek, E-Mails und Dokumentation Englisch, Deutsch und vereinfachtes Chinesisch sprechen.
order: 6
---

Jedes Wort, das Kaeso einer Person zeigt, gibt es auf Englisch, Deutsch und vereinfachtem Chinesisch (`en`, `de`, `zh-CN`). Englisch ist die Quelle: Eine Änderung an einem englischen Text kommt mit ihrer deutschen und chinesischen Fassung im selben Pull Request, und Prüfungen schlagen fehl, wenn eine fehlt.

| Teil | Wo seine Texte liegen | Geprüft von |
| --- | --- | --- |
| Die Konsole | `apps/console/src/i18n/locales/<locale>/<area>.json` | `pnpm i18n`, `pnpm lint` |
| Die UI-Bibliothek | `packages/ui/src/i18n/locales/` | Den Tests der Bibliothek |
| Der Showcase | `apps/showcase/src/i18n/` und `DOCS_TEXT` in `apps/showcase/src/lib/locale.tsx` | `pnpm --filter kaeso-showcase gen` |
| E-Mails | Eine Funktion pro E-Mail in `crates/kaeso-mail/src/templates.rs` | Den Tests des Mail-Crates ([E-Mails](architecture/emails.md#sprachen)) |
| Diese Dokumentation | `apps/docs/content/<locale>/` und `apps/docs/src/i18n/messages` | `check:content` und dem Typecheck ([Dokumentation schreiben](documentation.md#übersetzen)) |

Das Gateway schreibt seine eigenen Antworten, etwa Fehlermeldungen und die Namen von Rechten, auf Englisch; die Konsole übersetzt, was sie kennt ([was das Gateway schreibt](#was-das-gateway-schreibt)).

## Hausstil

Jede Sprache hat dieselbe Stimme: kurz, schlicht, keine Füllwörter. Übersetzen Sie die Bedeutung, nicht die Wörter, und bleiben Sie so kurz wie das Englische.

| | Englisch | Deutsch | Vereinfachtes Chinesisch |
| --- | --- | --- | --- |
| Anrede | Keine oder „you“ | „Sie“ mit „Ihr“, „Ihre“, „Ihnen“, nie „du“; Überschriften, Buttons, Menüeinträge und kurze Hinweise bleiben im Infinitiv („Schlüssel erstellen“, „gleich erneut versuchen“) | „您“, wo eine Person angesprochen wird, nie „你“; sonst neutrale Formulierungen |
| Buttons | Zuerst ein Verb: „Create key“ | Infinitiv, Verb am Ende: „Schlüssel erstellen“ | Zuerst ein Verb: „创建密钥“ |
| Titel | Satzschreibung | Deutsche Groß- und Kleinschreibung, sonst wie ein Satz | Keine Satzzeichen am Ende |
| Auslassungspunkte | „Loading…“, ein Zeichen | „Wird geladen…“ | „加载中…“ |
| Zahlen und Datumsangaben | Aus `@logicplanes/kaeso-ui/format`, nie von Hand formatiert | Ebenso | Ebenso |
| Anführungszeichen | "…" | „…“ | “…” |
| Interpunktion | | | Volle Breite (，。：；？（）), ohne Leerzeichen davor oder danach; ein Leerzeichen zwischen chinesischen und lateinischen Wörtern, Ziffern oder Code (`3 个密钥`, `API 密钥`) |

Deutsch ist etwa ein Drittel länger als Englisch. Layouts müssen das aushalten: Beschriftungen werden nicht abgeschnitten, Beschreibungen passen weiterhin in eine Zeile, Buttons behalten ihre Breite und Tabellen laufen nicht über. Chinesisch ist kürzer, braucht aber die volle Zeilenhöhe; nie kleiner setzen. Deutsch behält etablierte Begriffe wie Token, Playground und Budget, statt deutsche Wörter dafür zu erfinden.

Nie übersetzt werden:

- Produkt- und Anbieternamen (Kaeso, OpenAI, Anthropic, models.dev)
- Modell-IDs, IDs und Schlüsselpräfixe
- Code, Einstellungen (`KAESO_*`), Pfade und Feldnamen
- Rechte (`keys.all.view`)
- HTTP-Methoden und Statuscodes
- Einheiten, die die Bibliothek formatiert
- Die Namen von Personen und Organisationen

## Glossar

| Englisch | Deutsch | Vereinfachtes Chinesisch |
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

## Die Konsole

### Welche Sprache

- Eine Person wählt unter **Einstellungen** › **Konto** › **Darstellung** › **Sprache**: Automatisch, English, Deutsch oder 简体中文. Die Wahl wird an ihrem Konto gespeichert (`PATCH me` mit `locale`, gelesen aus `auth/session`), sodass sie ihr in jeden Browser folgt.
- **Automatisch**, der Standard, nimmt die erste Sprache des Browsers, die die Konsole hat (`navigator.languages`: `de-AT` ist Deutsch, jedes Chinesisch ist vereinfachtes Chinesisch), fällt auf Englisch zurück und folgt dem Browser, wenn sich dessen Sprache ändert.
- Der Browser hält eine Kopie der Wahl des Kontos (`kaeso:locale`), sodass schon das erste Zeichnen, die Anmeldung eingeschlossen, in dieser Sprache erscheint. Englisch lädt nichts weiter; Deutsch und Chinesisch laden ihre Kataloge (den der Konsole und den der Bibliothek, je ein Chunk) vor dem ersten Zeichnen.
- Meldet die Sitzung eine Wahl, die in einem anderen Browser getroffen wurde, wechselt `src/i18n/sync.tsx` zu ihr. Der Wechsel zeichnet die Konsole in der neuen Sprache neu; die Seite, ihre Daten und ihre Position bleiben.

### Texte schreiben

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

- **`t(key, english, values?)`** gibt einen String zurück. Rufen Sie sie beim Rendern auf oder dann, wenn der Text gebraucht wird (ein Toast, ein Titel), nie auf Modulebene. Eine beim Import berechnete Konstante bleibt in der Sprache, in der die Seite gestartet ist. Tabellen von Beschriftungen werden zu Funktionen (`() => t(…)`) oder innerhalb der Komponente gebaut.
- **Schlüssel und englischer Text sind String-Literale**, nebeneinander. Der Schlüssel ist `<area>.<words>` in camelCase, etwa `org.members.invite` oder `logs.inspector.overview`. Sein erster Teil benennt die Katalogdatei (`src/i18n/locales/<locale>/<area>.json`): `common`, `shell`, `auth`, `errors`, `rights`, `format`, `kaeso`, `signIn`, `public`, `account`, `org`, `orgHome`, `instance`, `project`, `keys`, `limits`, `logs`, `models`, `playground`. Ein Schlüssel benennt, was der Text bedeutet, nicht wo er steht; ein Schlüssel hat immer genau einen englischen Text, und eine geänderte Bedeutung bekommt einen neuen Schlüssel.
- **Gemeinsame Wörter** stehen in `common` (`src/i18n/common.ts` listet sie auf): Cancel, Save, Delete, Continue, Name, Email, Never und andere. Rufen Sie seine Funktion auf, etwa `common.cancel()`, oder verwenden Sie `t("common.cancel", "Cancel")` mit genau diesem englischen Text. Ein Wort, das an seiner Stelle etwas anderes bedeutet, bekommt einen eigenen Schlüssel.
- **Nur ganze Sätze.** Nie Fragmente zusammensetzen (`"Joined " + date`, `` `${n} keys` ``), weil sich die Wortstellung zwischen Sprachen unterscheidet. Werte gehören in Platzhalter (`{name}`); Zahlen, die eine Form bestimmen, laufen über `plural` (`one`, `other`, `=0`; Deutsch hat dieselben Formen, Chinesisch nur `other`); Varianten laufen über `select`. Formatieren Sie Zahlen, Geldbeträge und Datumsangaben vor der Übergabe mit `@logicplanes/kaeso-ui/format`; die Formatierer folgen der Sprache.
- **Teile, die Elemente sind**, etwa ein Link oder ein fett gesetzter Name, sind Platzhalter von `tRich`, das Knoten zurückgibt.
- `aria-label`s, `title`s, Platzhaltertexte und der Seitentitel (`usePageTitle`) sind Text wie jeder andere.

### Was das Gateway schreibt

Die Fehlermeldungen des Gateways, die Namen, Beschreibungen und Kategorien im Rechtekatalog sowie Namen und Beschreibungen der eingebauten Rollen kommen auf Englisch an. Der API-Client (`src/api/`) übersetzt, was er kennt, sobald Antworten eintreffen, mit den Schlüsseln `errors.*` und `rights.*`. Rechte und Rollen übersetzt er über ihre ID, Meldungen über ihren englischen Text und ihre Werte. Was er nicht kennt, erscheint so, wie das Gateway es geschrieben hat.

Ein Sprachwechsel lädt die Daten neu. `crates/kaeso-server/tests/error_copy.rs` schlägt fehl, wenn das Gateway eine Meldung schreibt, für die `src/api/gateway-errors.ts` keine Regel hat; eine neue Meldung kommt deshalb mit ihrer deutschen und chinesischen Fassung im selben Pull Request.

## Die UI-Bibliothek

- `LocaleProvider` (`@logicplanes/kaeso-ui/i18n`) setzt die Sprache für alles darunter: die eigenen Texte der Bibliothek und wie `@logicplanes/kaeso-ui/format` Zahlen und Datumsangaben schreibt.
- Eine Komponente schreibt ihren Text als `t("dialog.close", "Close")` mit `useUiText`. Der englische Text steht neben dem Schlüssel, sodass eine Seite auf Englisch nur die Texte der Komponenten enthält, die sie nutzt.
- Deutsch und Chinesisch liegen in `packages/ui/src/i18n/locales/de.ts` und `zh-CN.ts`, und `loadUiMessages(locale)` lädt eine davon bei Bedarf; Englisch lädt nichts. `en.ts` listet jeden Schlüssel mit seinem englischen Text für Übersetzende auf, und ein Test hält die Datei im Gleichklang mit den Komponenten und prüft, dass beide Übersetzungen jeden Schlüssel mit denselben Platzhaltern haben.
- Schlüssel sind `component.meaning` in camelCase: `dialog.close`, `datePicker.last7Days`, `list.clearSelection`. Platzhalter folgen der gemeinsamen Teilmenge von ICU MessageFormat: `{name}`, `{count, plural, one {# key} other {# keys}}`, `{kind, select, … other {…}}`.

### In einer Komponente

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

- Importieren Sie `useUiText` und `useLocale` aus `../../i18n/context`, nie aus dem Index: Das Kontextmodul enthält keine Formatierer, sodass das Bundle einer Komponente klein bleibt.
- Der englische Text ist ein einfaches String-Literal im selben Aufruf; der Katalogtest liest jedes `t("key", "English")` im Quellcode.
- Alles, was eine Person sieht oder hört, läuft über `t`: sichtbarer Text, `aria-label`, Titel, Platzhaltertexte, Text nur für Screenreader, Toasts und leere Zustände. Daten, die die App übergibt (Namen, Rollen, Rechte), und Produktnamen bleiben, wie sie sind.
- Eine Text-Prop mit Standardwert, etwa `emptyText`, hat in ihrer Signatur keinen; die Komponente nutzt `emptyText ?? t("combobox.noResults", "No results")`, sodass der Standard der Sprache folgt und der eigene Text einer App Vorrang hat.
- Anzahlen sind eine Nachricht mit Plural (`t("list.selected", "{count} selected", { count })`), nie zwei zusammengesetzte Aufrufe. Eine als Wert übergebene Zahl wird für die Sprache geschrieben (1,234 oder 1.234).
- Namen von Wochentagen und Monaten sowie Datumsangaben kommen aus `Intl` für `useLocale()` oder aus `@logicplanes/kaeso-ui/format`, nie aus englischen Namen im Quellcode.
- Stellen, die ihre Breite behalten, während sich ihr Text ändert (`reserveLabels` von `Button`, `SwapText`), bekommen jede Alternative übersetzt, sodass die breiteste in der aktuellen Sprache reserviert wird.
- Keine Arbeit auf Modulebene: Eine Liste von Beschriftungen, etwa die Voreinstellungen der Datumsauswahl, ist eine Funktion von `t`, die beim Rendern aufgerufen wird. Ein Helfer außerhalb von React, der Text schreibt (`keyLabel`, `liveLine`), nimmt das `t` der Komponente als letztes Argument.
- Eine Liste von Namen wird mit dem Trennzeichen der Sprache verbunden (`t("rightsMatrix.listSeparator", ", ")`, auf Chinesisch „、“).

### In einer App

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

- Ohne `LocaleProvider` ist alles Englisch.
- `resolveUiLocale` bildet ein BCP-47-Tag auf eine mitgelieferte Sprache ab: `de-AT` ist `de`, `zh-Hans` ist `zh-CN`, alles andere `en`. `loadUiMessages` gibt für Englisch `{}` zurück und lädt nichts.
- Laden Sie für einen Sprachwechsel den neuen Katalog und rendern Sie den Provider dann mit neuer Sprache und neuem Katalog zugleich; behalten Sie das alte Paar, bis der Katalog da ist.
- Der Provider setzt auch die Sprache der Formatierer (`formatNumber`, `formatDate`, `formatRelative`, `formatUSD` und andere), die einfache Funktionen sind; `setFormatLocale(locale, messages)` tut dasselbe ohne React. Die eigenen Texte einer App nutzen ihre eigenen Kataloge, und `formatMessage(template, values, locale)` formatiert ICU-Nachrichten auf dieselbe Weise.

## Der Showcase

- Jede Seite des Showcases wird in jeder Sprache gebaut: Englisch unter `/components/button`, Deutsch unter `/de/components/button`, Chinesisch unter `/zh-CN/components/button`. Das Chrome, das Raster der Startseite, die Anleitungen und jede Beschreibung von Komponenten und Props stehen ab dem ersten Zeichnen in der Sprache der Seite.
- Die Sprachauswahl in der Seitenleiste öffnet dieselbe Seite in einer anderen Sprache und wird gespeichert. Eine englische Adresse folgt der gespeicherten Wahl, dann den Sprachen des Browsers; eine deutsche oder chinesische Adresse zeigt immer diese Sprache. `?lang=de`, `?lang=zh-CN` und `?lang=en` werden gespeichert und leiten auf die Adresse dieser Sprache weiter.
- Die Texte des Chromes liegen in `DOCS_TEXT` (`apps/showcase/src/lib/locale.tsx`). Alle anderen Texte liegen in `apps/showcase/src/i18n/de.json` und `zh-CN.json`, mit dem englischen Text samt zusammengefasstem Leerraum als Schlüssel.
- `pnpm --filter kaeso-showcase gen` schlägt fehl und listet jeden Text ohne deutschen oder chinesischen Eintrag auf, sodass eine neue Beschreibung nicht nur auf Englisch ausgeliefert werden kann.
- Die Demos in den Komponentenvorschauen sind Code und bleiben Englisch, ebenso Namen wie `Button` und `formatNumber`.

## Prüfen

- `pnpm i18n --write` schreibt die `src/i18n/locales/en/<area>.json` der Konsole aus dem Quellcode; das ist die Liste für Übersetzende. `pnpm i18n` prüft, dass jeder Schlüssel Deutsch und Chinesisch mit denselben Platzhaltern hat und dass nichts übrig ist. Es lehnt eine Übersetzung ab, die noch englisch ist, wobei Namen, Code, Abkürzungen und einige Begriffe wie „Top k“ englisch bleiben dürfen. Außerdem lehnt es „du“ im Deutschen, „你“ im Chinesischen, gerade Anführungszeichen und halbbreite Satzzeichen neben chinesischen Zeichen ab. `pnpm test` führt dieselbe Prüfung aus.
- Der Katalogtest der UI-Bibliothek, `gen` des Showcases und der Nachrichtentest der Dokumentationswebsite lehnen dasselbe ab: Englisch in einer Übersetzung und eine informelle Anrede. Die Tests des Mail-Crates schlagen fehl, wenn eine deutsche oder chinesische E-Mail einen englischen Satz behält oder „du“ oder „Tokens“ verwendet.
- `pnpm lint` lehnt Text ab, der direkt in die Konsole geschrieben ist: JSX-Text, Text-Props (`label`, `title`, `description`, `placeholder`, `aria-label` und andere), Textfelder von Objekten und als Literale geschriebene Toasts (die Regel `kaeso/no-literal-text`).
- `apps/console/src/copy.test.ts` und `apps/console/src/toast-copy.test.ts` prüfen deutsche und chinesische Beschreibungen und Toasts gegen dieselben Längengrenzen wie die englischen, mit 15 % Spielraum.
- Sehen Sie sich jede geänderte Seite auf Deutsch und Chinesisch an, in Telefon- und Desktopbreite und in beiden Themes. Wählen Sie die Sprache unter **Darstellung** oder setzen Sie `kaeso:locale` im Speicher des Browsers. Öffnen Sie im Showcase `/de/components/<slug>/states` und `/zh-CN/components/<slug>/states` einer geänderten Komponente.
