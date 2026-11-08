import type { CodeLang, CodeToken } from "./highlighter.js";
import { kaesoShikiTheme } from "./kaeso-theme.js";
import { codeToneClass, codeTokenClass } from "./tones.js";

type Grammar = { name: string };
type Highlighter = {
  codeToTokens: (
    code: string,
    o: { lang: string; theme: string; tokenizeMaxLineLength?: number | undefined },
  ) => { tokens: { content: string; color?: string | undefined; fontStyle?: number | undefined }[][] };
  loadLanguage: (...l: unknown[]) => Promise<void>;
};
let instance: Promise<Highlighter> | null = null;

const HIGHLIGHT_MAX_LINE = 2_000;

export function getHighlighter(): Promise<Highlighter> {
  instance ??= (async () => {
    const [{ createHighlighterCore }, { createJavaScriptRegexEngine }] = await Promise.all([import("shiki/core"), import("shiki/engine/javascript")]);
    return (await createHighlighterCore({ themes: [kaesoShikiTheme as never], langs: [], engine: createJavaScriptRegexEngine() })) as unknown as Highlighter;
  })().catch((error: unknown) => {
    instance = null;
    throw error;
  });
  return instance;
}

type Loader = () => Promise<{ default: Grammar[] }>;
const LOADERS: Record<Exclude<CodeLang, "text">, Loader> = {
  ts: () => import("shiki/dist/langs/typescript.mjs"),
  tsx: () => import("shiki/dist/langs/tsx.mjs"),
  js: () => import("shiki/dist/langs/javascript.mjs"),
  jsx: () => import("shiki/dist/langs/jsx.mjs"),
  json: () => import("shiki/dist/langs/json.mjs"),
  jsonc: () => import("shiki/dist/langs/jsonc.mjs"),
  bash: () => import("shiki/dist/langs/bash.mjs"),
  shellsession: () => import("shiki/dist/langs/bash.mjs"),
  powershell: () => import("shiki/dist/langs/powershell.mjs"),
  python: () => import("shiki/dist/langs/python.mjs"),
  yaml: () => import("shiki/dist/langs/yaml.mjs"),
  toml: () => import("shiki/dist/langs/toml.mjs"),
  ini: () => import("shiki/dist/langs/ini.mjs"),
  dotenv: () => import("shiki/dist/langs/dotenv.mjs"),
  diff: () => import("shiki/dist/langs/diff.mjs"),
  rust: () => import("shiki/dist/langs/rust.mjs"),
  go: () => import("shiki/dist/langs/go.mjs"),
  sql: () => import("shiki/dist/langs/sql.mjs"),
  html: () => import("shiki/dist/langs/html.mjs"),
  css: () => import("shiki/dist/langs/css.mjs"),
  scss: () => import("shiki/dist/langs/scss.mjs"),
  xml: () => import("shiki/dist/langs/xml.mjs"),
  markdown: () => import("shiki/dist/langs/markdown.mjs"),
  dockerfile: () => import("shiki/dist/langs/dockerfile.mjs"),
  http: () => import("shiki/dist/langs/http.mjs"),
  graphql: () => import("shiki/dist/langs/graphql.mjs"),
  java: () => import("shiki/dist/langs/java.mjs"),
  kotlin: () => import("shiki/dist/langs/kotlin.mjs"),
  swift: () => import("shiki/dist/langs/swift.mjs"),
  c: () => import("shiki/dist/langs/c.mjs"),
  cpp: () => import("shiki/dist/langs/cpp.mjs"),
  csharp: () => import("shiki/dist/langs/csharp.mjs"),
  php: () => import("shiki/dist/langs/php.mjs"),
  ruby: () => import("shiki/dist/langs/ruby.mjs"),
  lua: () => import("shiki/dist/langs/lua.mjs"),
  hcl: () => import("shiki/dist/langs/hcl.mjs"),
  nginx: () => import("shiki/dist/langs/nginx.mjs"),
  make: () => import("shiki/dist/langs/make.mjs"),
  proto: () => import("shiki/dist/langs/proto.mjs"),
  vue: () => import("shiki/dist/langs/vue.mjs"),
  svelte: () => import("shiki/dist/langs/svelte.mjs"),
};

const grammars: Partial<Record<CodeLang, Promise<string>>> = {};

function grammar(h: Highlighter, lang: Exclude<CodeLang, "text">): Promise<string> {
  let loaded = grammars[lang];
  if (!loaded) {
    // A language module lists the grammars it embeds first and its own grammar last.
    loaded = LOADERS[lang]().then(async ({ default: list }) => {
      await h.loadLanguage(list);
      return list[list.length - 1]?.name ?? lang;
    });
    loaded.catch(() => delete grammars[lang]);
    grammars[lang] = loaded;
  }
  return loaded;
}

const PROMPT = /^(?:\(\S+\)\s*)?(?:[\w.-]+@[\w.-]+(?::\S*)?|~\S*)?[$#%❯➜] /;

function tokenize(h: Highlighter, code: string, id: string): CodeToken[][] {
  return h
    .codeToTokens(code, { lang: id, theme: "kaeso", tokenizeMaxLineLength: HIGHLIGHT_MAX_LINE })
    .tokens.map((line) => line.map((token) => ({ content: token.content, className: codeTokenClass(token.color, token.fontStyle) })));
}

// Shiki's shellsession grammar colours only the prompt, so a session highlights each command with the shell grammar and leaves its output plain.
// A session without a single prompt is a list of commands, as in a `console` fence of a model's reply.
function session(h: Highlighter, code: string, id: string): CodeToken[][] {
  let continued = false;
  const lines = code.split("\n").map((line) => {
    const prompt = continued ? "" : PROMPT.exec(line)?.[0];
    continued = prompt !== undefined && line.endsWith("\\");
    return { line, prompt };
  });
  if (lines.every(({ prompt }) => prompt === undefined)) return tokenize(h, code, id);
  const commands = tokenize(h, lines.map(({ line, prompt }) => (prompt === undefined ? "" : line.slice(prompt.length))).join("\n"), id);
  return lines.map(({ line, prompt }, i) => {
    if (prompt === undefined) return line ? [{ content: line, className: "" }] : [];
    return [...(prompt ? [{ content: prompt, className: codeToneClass("punctuation") }] : []), ...(commands[i] ?? [])];
  });
}

export async function tokens(code: string, lang: Exclude<CodeLang, "text">): Promise<CodeToken[][]> {
  const h = await getHighlighter();
  const id = await grammar(h, lang);
  return lang === "shellsession" ? session(h, code, id) : tokenize(h, code, id);
}
