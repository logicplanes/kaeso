export const HIGHLIGHT_MAX_CHARS = 200_000;

/** Every language `CodeBlock`, `DiffView` and `Markdown` highlight. Each loads on first use, so a page pays only for the languages it shows. */
export const CODE_LANGS = [
  "ts",
  "tsx",
  "js",
  "jsx",
  "json",
  "jsonc",
  "bash",
  "shellsession",
  "powershell",
  "python",
  "yaml",
  "toml",
  "ini",
  "dotenv",
  "diff",
  "rust",
  "go",
  "sql",
  "html",
  "css",
  "scss",
  "xml",
  "markdown",
  "dockerfile",
  "http",
  "graphql",
  "java",
  "kotlin",
  "swift",
  "c",
  "cpp",
  "csharp",
  "php",
  "ruby",
  "lua",
  "hcl",
  "nginx",
  "make",
  "proto",
  "vue",
  "svelte",
] as const;
/** A language `CodeBlock` highlights, or `text` for none. `resolveCodeLang` turns a fence name such as `py` or `sh` into one. */
export type CodeLang = (typeof CODE_LANGS)[number] | "text";

const ALIASES: Record<string, CodeLang> = {
  typescript: "ts",
  mts: "ts",
  cts: "ts",
  javascript: "js",
  mjs: "js",
  cjs: "js",
  node: "js",
  jsonl: "json",
  ndjson: "json",
  geojson: "json",
  json5: "jsonc",
  sh: "bash",
  shell: "bash",
  shellscript: "bash",
  zsh: "bash",
  ksh: "bash",
  console: "shellsession",
  terminal: "shellsession",
  "shell-session": "shellsession",
  "sh-session": "shellsession",
  ps: "powershell",
  ps1: "powershell",
  pwsh: "powershell",
  py: "python",
  python3: "python",
  py3: "python",
  yml: "yaml",
  cfg: "ini",
  properties: "ini",
  editorconfig: "ini",
  gitconfig: "ini",
  env: "dotenv",
  patch: "diff",
  rs: "rust",
  golang: "go",
  postgres: "sql",
  postgresql: "sql",
  pgsql: "sql",
  mysql: "sql",
  sqlite: "sql",
  htm: "html",
  xhtml: "html",
  svg: "xml",
  plist: "xml",
  xsd: "xml",
  md: "markdown",
  mdx: "markdown",
  gfm: "markdown",
  docker: "dockerfile",
  containerfile: "dockerfile",
  gql: "graphql",
  kt: "kotlin",
  kts: "kotlin",
  h: "c",
  "c++": "cpp",
  cc: "cpp",
  cxx: "cpp",
  hpp: "cpp",
  hxx: "cpp",
  cs: "csharp",
  "c#": "csharp",
  rb: "ruby",
  terraform: "hcl",
  tf: "hcl",
  tfvars: "hcl",
  makefile: "make",
  mk: "make",
  protobuf: "proto",
};

const known = (name: string): name is Exclude<CodeLang, "text"> => (CODE_LANGS as readonly string[]).includes(name);

/**
 * The language for a Markdown fence's info string or a file's extension: `py` is `python`, `sh` and `zsh` are `bash`, `yml` is `yaml`,
 * `ts title=app.ts` is `ts`. Anything it doesn't know, such as `caddyfile`, is `text`, which shows the code plain.
 */
export function resolveCodeLang(info: string | undefined): CodeLang {
  const name = (info ?? "")
    .trim()
    .split(/[\s{]/)[0]
    ?.toLowerCase()
    .replace(/^language-|^\./, "");
  if (!name) return "text";
  if (known(name)) return name;
  return Object.hasOwn(ALIASES, name) ? (ALIASES[name] ?? "text") : "text";
}

export interface CodeToken {
  content: string;
  className: string;
}

/** Tokens of `code` with their palette classes, one array per line; `null` for `text`, an unknown language or code over 200,000 characters. */
export async function highlight(code: string, lang: CodeLang): Promise<CodeToken[][] | null> {
  if (!known(lang) || code.length > HIGHLIGHT_MAX_CHARS) return null;
  const { tokens } = await import("./shiki.js");
  return tokens(code.replace(/\r\n?/g, "\n"), lang);
}
