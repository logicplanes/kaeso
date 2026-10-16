import { useSyncExternalStore, type ReactNode } from "react";
import { Select, useTheme, type ThemePreference } from "@logicplanes/kaeso-ui";
import { useUiText } from "@logicplanes/kaeso-ui/i18n";
import { LocaleProvider, loadUiMessages, type UiMessages } from "@logicplanes/kaeso-ui/i18n";

export const DOCS_LANGUAGES = [
  { value: "en", label: "English" },
  { value: "de", label: "Deutsch" },
  { value: "zh-CN", label: "简体中文" },
] as const;
export type DocsLang = (typeof DOCS_LANGUAGES)[number]["value"];

export const DOCS_LANG_STORAGE_KEY = "kaeso:docs-lang";

function parse(value: string | null | undefined): DocsLang | undefined {
  const v = value?.trim().toLowerCase();
  if (v === "en") return "en";
  if (v === "de") return "de";
  if (v === "zh-cn" || v === "zh") return "zh-CN";
  return undefined;
}

function detected(): DocsLang {
  const prefs = typeof navigator === "undefined" ? [] : navigator.languages?.length ? navigator.languages : [navigator.language];
  for (const tag of prefs) {
    const l = tag?.toLowerCase() ?? "";
    if (l === "en" || l.startsWith("en-")) return "en";
    if (l === "de" || l.startsWith("de-")) return "de";
    if (l === "zh" || l.startsWith("zh-")) return "zh-CN";
  }
  return "en";
}

function remember(lang: DocsLang) {
  try {
    localStorage.setItem(DOCS_LANG_STORAGE_KEY, lang);
  } catch {}
}

function requested(): DocsLang {
  const q = parse(new URLSearchParams(location.search).get("lang"));
  if (q) {
    remember(q);
    return q;
  }
  try {
    return parse(localStorage.getItem(DOCS_LANG_STORAGE_KEY)) ?? detected();
  } catch {
    return detected();
  }
}

export const docsLangScript = `(function(){function p(v){v=(v||"").toLowerCase();if(v==="en"||v.indexOf("en-")===0)return"en";if(v==="de"||v.indexOf("de-")===0)return"de";if(v==="zh"||v.indexOf("zh-")===0)return"zh-CN";return""}function a(){var l="";try{l=p(new URLSearchParams(location.search).get("lang"))||p(localStorage.getItem("${DOCS_LANG_STORAGE_KEY}"))}catch(e){}if(!l){var n=navigator.languages&&navigator.languages.length?navigator.languages:[navigator.language];for(var i=0;i<n.length&&!l;i++)l=p(n[i])}document.documentElement.lang=l||"en"}a();if(!window.__kaesoDocsLang){window.__kaesoDocsLang=1;document.addEventListener("astro:after-swap",a)}})();`;

const DOCS_TEXT: Record<Exclude<DocsLang, "en">, Record<string, string>> = {
  de: {
    Overview: "Übersicht",
    Installation: "Installation",
    Theming: "Theming",
    "For agents": "Für Agents",
    Changelog: "Änderungen",
    Components: "Komponenten",
    "Get started": "Loslegen",
    Search: "Suchen",
    Language: "Sprache",
    Examples: "Beispiele",
    Props: "Props",
    "Not found": "Nicht gefunden",
    "This page doesn't exist.": "Diese Seite gibt es nicht.",
    "The link may be old, or the page moved.": "Der Link ist vielleicht veraltet, oder die Seite wurde verschoben.",
    "Go to the overview": "Zur Übersicht",
    "Copy page": "Seite kopieren",
    Copied: "Kopiert",
    "View source": "Quelltext",
    "Show code": "Code zeigen",
    "Hide code": "Code ausblenden",
    "On this page": "Auf dieser Seite",
    "Open navigation": "Navigation öffnen",
    "Expand sidebar": "Seitenleiste ausklappen",
    "Kaeso UI home": "Kaeso UI Startseite",
    "Copy code": "Code kopieren",
  },
  "zh-CN": {
    Overview: "概览",
    Installation: "安装",
    Theming: "主题",
    "For agents": "面向智能体",
    Changelog: "更新日志",
    Components: "组件",
    "Get started": "开始使用",
    Search: "搜索",
    Language: "语言",
    Examples: "示例",
    Props: "属性",
    "Not found": "未找到",
    "This page doesn't exist.": "此页面不存在。",
    "The link may be old, or the page moved.": "链接可能已过期，或页面已移动。",
    "Go to the overview": "前往概览",
    "Copy page": "复制页面",
    Copied: "已复制",
    "View source": "查看源码",
    "Show code": "显示代码",
    "Hide code": "隐藏代码",
    "On this page": "本页内容",
    "Open navigation": "打开导航",
    "Expand sidebar": "展开侧边栏",
    "Kaeso UI home": "Kaeso UI 首页",
    "Copy code": "复制代码",
  },
};

export function docsText(lang: DocsLang, english: string): string {
  if (lang === "en") return english;
  const key = english.replace(/\s+/g, " ").trim();
  return DOCS_TEXT[lang][key] ?? (current.lang === lang ? current.text[key] : undefined) ?? english;
}

export function inlineCode(text: string): string {
  const escaped = text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  return escaped.replace(/`([^`]+)`/g, '<code class="rounded-code bg-fill-1 px-4 text-small mono">$1</code>');
}

function translateStatic() {
  document.querySelectorAll<HTMLElement>("[data-docs-text]").forEach((el) => {
    const text = docsText(wanted, el.dataset.docsText ?? "");
    if (el.hasAttribute("data-docs-code")) el.innerHTML = inlineCode(text);
    else el.textContent = text;
  });
  document.querySelectorAll<HTMLElement>("[data-docs-label]").forEach((el) => {
    el.setAttribute("aria-label", docsText(wanted, el.dataset.docsLabel ?? ""));
  });
  const page = document.querySelector<HTMLElement>(`[data-docs-lang="${wanted}"][data-docs-page-title]`)?.dataset.docsPageTitle;
  const title = page ?? docsText(wanted, document.documentElement.dataset.docsPage ?? "");
  if (title) document.title = title === "Kaeso UI" ? title : `${title} · Kaeso UI`;
  document.querySelectorAll<HTMLElement>("[data-docs-title]").forEach((el) => {
    const shortcut = el.dataset.docsShortcut;
    const title = docsText(wanted, el.dataset.docsTitle ?? "");
    el.title = shortcut ? `${title}  ${shortcut}` : title;
  });
}

interface Snapshot {
  lang: DocsLang;
  locale: string;
  messages: UiMessages;
  text: Record<string, string>;
}

const ENGLISH: Snapshot = { lang: "en", locale: "en", messages: {}, text: {} };
let current: Snapshot = ENGLISH;
let wanted: DocsLang = "en";
type Catalog = { messages: UiMessages; text: Record<string, string> };
const catalogs = new Map<DocsLang, Promise<Catalog>>();
const listeners = new Set<() => void>();

function catalog(lang: Exclude<DocsLang, "en">): Promise<Catalog> {
  let p = catalogs.get(lang);
  if (!p) {
    p = Promise.all([loadUiMessages(lang), import(`../i18n/${lang}.json`).then((m: { default: Record<string, string> }) => m.default)]).then(
      ([messages, text]) => ({ messages, text }),
    );
    catalogs.set(lang, p);
  }
  return p;
}

function apply(lang: DocsLang) {
  wanted = lang;
  document.documentElement.lang = lang;
  translateStatic();
  if (lang === "en") {
    publish(ENGLISH);
    return;
  }
  if (current.lang === lang) return;
  void catalog(lang).then(
    ({ messages, text }) => {
      if (wanted === lang) publish({ lang, locale: lang, messages, text });
    },
    () => {},
  );
}

function publish(next: Snapshot) {
  if (next !== current) {
    current = next;
    for (const l of listeners) l();
  }
  translateStatic();
  mark();
}

function mark() {
  document.documentElement.dataset.uiLang = current.locale;
}

if (typeof window !== "undefined") {
  apply(requested());
  document.addEventListener("astro:after-swap", () => {
    mark();
    apply(requested());
  });
  document.addEventListener("astro:page-load", translateStatic);
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}
const getSnapshot = () => current;
const getServerSnapshot = () => ENGLISH;

export function setDocsLang(lang: DocsLang) {
  remember(lang);
  const url = new URL(location.href);
  if (lang === "en") url.searchParams.delete("lang");
  else url.searchParams.set("lang", lang);
  history.replaceState(history.state, "", url);
  apply(lang);
}

export function useDocsLang(): Snapshot {
  return useSyncExternalStore(subscribe, getSnapshot, getServerSnapshot);
}

export function DocsLocale({ children }: { children: ReactNode }) {
  const { locale, messages } = useDocsLang();
  return (
    <LocaleProvider locale={locale} messages={messages}>
      {children}
    </LocaleProvider>
  );
}

export function DocsLanguageSelect() {
  const { lang } = useDocsLang();
  return (
    <Select
      aria-label={docsText(lang, "Language")}
      size="sm"
      appearance="ghost"
      fullWidth
      value={lang}
      onValueChange={(v) => setDocsLang(parse(v) ?? "en")}
      options={DOCS_LANGUAGES.map((l) => ({ value: l.value, label: l.label }))}
    />
  );
}

export function DocsThemeSelect() {
  const { theme, setTheme } = useTheme();
  const t = useUiText();
  return (
    <Select
      aria-label={t("themeMenu.theme", "Theme")}
      size="sm"
      appearance="ghost"
      fullWidth
      value={theme}
      onValueChange={(v) => setTheme(v as ThemePreference)}
      options={[
        { value: "system", label: t("themeMenu.system", "System") },
        { value: "light", label: t("themeMenu.light", "Light") },
        { value: "dark", label: t("themeMenu.dark", "Dark") },
      ]}
    />
  );
}
