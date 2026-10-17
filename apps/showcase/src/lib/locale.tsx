import { useSyncExternalStore, type ReactNode } from "react";
import { navigate } from "astro:transitions/client";
import { Select } from "@logicplanes/kaeso-ui";
import { LocaleProvider, loadUiMessages, type UiMessages } from "@logicplanes/kaeso-ui/i18n";
import { DOCS_LANGUAGES, DOCS_LANG_STORAGE_KEY, docsLangOf, localizedPath, pageOfPath, parseDocsLang, type DocsLang } from "./languages";

export type { DocsLang } from "./languages";

function remember(lang: DocsLang) {
  try {
    localStorage.setItem(DOCS_LANG_STORAGE_KEY, lang);
  } catch {}
}

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
    Documentation: "Dokumentation",
    States: "Zustände",
    Light: "Hell",
    Dark: "Dunkel",
    "All examples": "Alle Beispiele",
    "All states · light and dark": "Alle Zustände · hell und dunkel",
    "Search pages, components and props": "Seiten, Komponenten, Props suchen",
    "Nothing matches. Try a component or a prop name.": "Keine Treffer. Suchen Sie nach einer Komponente oder einem Prop.",
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
    Documentation: "文档",
    States: "状态",
    Light: "浅色",
    Dark: "深色",
    "All examples": "全部示例",
    "All states · light and dark": "全部状态 · 浅色和深色",
    "Search pages, components and props": "搜索页面、组件和属性",
    "Nothing matches. Try a component or a prop name.": "没有匹配项。试试组件名或属性名。",
  },
};

export function docsText(lang: DocsLang, english: string): string {
  if (lang === "en") return english;
  const key = english.replace(/\s+/g, " ").trim();
  return DOCS_TEXT[lang][key] ?? loaded.get(lang)?.text[key] ?? english;
}

export function inlineCode(text: string): string {
  const escaped = text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  return escaped.replace(/`([^`]+)`/g, '<code class="rounded-code bg-fill-1 px-4 text-small mono">$1</code>');
}

interface Snapshot {
  lang: DocsLang;
  locale: string;
  messages: UiMessages;
}

type Catalog = { messages: UiMessages; text: Record<string, string> };
const ENGLISH: Snapshot = { lang: "en", locale: "en", messages: {} };
const loaded = new Map<DocsLang, Catalog>([["en", { messages: {}, text: {} }]]);
const loading = new Map<DocsLang, Promise<void>>();
const listeners = new Set<() => void>();
let current: Snapshot = ENGLISH;

export function loadDocsLang(lang: DocsLang): Promise<void> {
  if (loaded.has(lang)) return Promise.resolve();
  let p = loading.get(lang);
  if (!p) {
    p = Promise.all([loadUiMessages(lang), import(`../i18n/${lang}.json`).then((m: { default: Record<string, string> }) => m.default)]).then(
      ([messages, text]) => void loaded.set(lang, { messages, text }),
      (error: unknown) => {
        loading.delete(lang);
        throw error;
      },
    );
    loading.set(lang, p);
  }
  return p;
}

function show(lang: DocsLang) {
  if (current.lang === lang) return;
  current = { lang, locale: lang, messages: loaded.get(lang)?.messages ?? {} };
  for (const l of listeners) l();
}

export async function renderDocsLang(lang: DocsLang): Promise<void> {
  await loadDocsLang(lang);
  show(lang);
}

export async function prepareDocsPage(param: string | undefined): Promise<DocsLang> {
  const lang = docsLangOf(param);
  await renderDocsLang(lang);
  return lang;
}

const documentLang = () => parseDocsLang(document.documentElement.lang) ?? "en";

if (typeof document !== "undefined") {
  await renderDocsLang(documentLang());
  document.addEventListener("astro:before-preparation", (event) => {
    const load = event.loader;
    event.loader = async () => {
      await Promise.all([load(), loadDocsLang(pageOfPath(event.to.pathname).lang).catch(() => undefined)]);
    };
  });
  document.addEventListener("astro:after-swap", () => show(documentLang()));
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}
const getSnapshot = () => current;

export function setDocsLang(lang: DocsLang) {
  remember(lang);
  void navigate(`${localizedPath(lang, pageOfPath(location.pathname).path)}${location.hash}`);
}

export function useDocsLang(): Snapshot {
  return useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
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
      onValueChange={(v) => setDocsLang(parseDocsLang(v) ?? "en")}
      options={DOCS_LANGUAGES.map((l) => ({ value: l.value, label: <span lang={l.value}>{l.label}</span>, textValue: l.label }))}
    />
  );
}
