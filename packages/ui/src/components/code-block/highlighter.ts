import { kaesoShikiTheme } from "./kaeso-theme";

type Highlighter = {
  codeToHtml: (code: string, o: { lang: string; theme: string }) => string;
  getLoadedLanguages: () => string[];
  loadLanguage: (...l: unknown[]) => Promise<void>;
};
let instance: Promise<Highlighter> | null = null;

export const CODE_LANGS = ["ts", "tsx", "js", "json", "bash", "python", "yaml", "diff"] as const;
export type CodeLang = (typeof CODE_LANGS)[number] | "text";

export function getHighlighter(): Promise<Highlighter> {
  instance ??= (async () => {
    const [{ createHighlighterCore }, { createJavaScriptRegexEngine }] = await Promise.all([import("shiki/core"), import("shiki/engine/javascript")]);
    return (await createHighlighterCore({ themes: [kaesoShikiTheme as never], langs: [], engine: createJavaScriptRegexEngine() })) as unknown as Highlighter;
  })();
  return instance;
}

const LOADERS: Record<Exclude<CodeLang, "text">, () => Promise<{ default: unknown }>> = {
  ts: () => import("shiki/dist/langs/typescript.mjs"),
  tsx: () => import("shiki/dist/langs/tsx.mjs"),
  js: () => import("shiki/dist/langs/javascript.mjs"),
  json: () => import("shiki/dist/langs/json.mjs"),
  bash: () => import("shiki/dist/langs/bash.mjs"),
  python: () => import("shiki/dist/langs/python.mjs"),
  yaml: () => import("shiki/dist/langs/yaml.mjs"),
  diff: () => import("shiki/dist/langs/diff.mjs"),
};

export async function highlight(code: string, lang: CodeLang): Promise<string | null> {
  if (lang === "text") return null;
  const h = await getHighlighter();
  if (!h.getLoadedLanguages().includes(lang)) {
    const loader = LOADERS[lang];
    if (!loader) return null;
    await h.loadLanguage((await loader()).default);
  }
  return h.codeToHtml(code, { lang, theme: "kaeso" });
}
