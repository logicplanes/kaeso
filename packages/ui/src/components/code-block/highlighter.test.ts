import { describe, expect, it } from "vitest";
import { CODE_LANGS, HIGHLIGHT_MAX_CHARS, highlight, resolveCodeLang, type CodeLang } from "./highlighter.js";

const keyword = "text-code-keyword";
const string = "text-code-string";
const constant = "text-code-constant";
const comment = "text-code-comment italic";
const fn = "text-code-function";
const type = "text-code-type";
const property = "text-code-property";
const tag = "text-code-tag";
const attribute = "text-code-attribute";
const variable = "text-code-variable";
const punctuation = "text-code-punctuation";

async function tones(code: string, lang: CodeLang): Promise<Record<string, string>> {
  const out: Record<string, string> = {};
  for (const token of (await highlight(code, lang))?.flat() ?? []) {
    const text = token.content.trim();
    if (text && !(text in out)) out[text] = token.className;
  }
  return out;
}

describe("resolveCodeLang", () => {
  it.each([
    ["TypeScript", "ts"],
    ["mts", "ts"],
    ["javascript", "js"],
    ["cjs", "js"],
    ["jsx", "jsx"],
    ["json5", "jsonc"],
    ["ndjson", "json"],
    ["sh", "bash"],
    ["zsh", "bash"],
    ["shell", "bash"],
    ["console", "shellsession"],
    ["ps1", "powershell"],
    ["py", "python"],
    ["yml", "yaml"],
    ["env", "dotenv"],
    [".env", "dotenv"],
    ["properties", "ini"],
    ["patch", "diff"],
    ["rs", "rust"],
    ["golang", "go"],
    ["postgresql", "sql"],
    ["svg", "xml"],
    ["md", "markdown"],
    ["Dockerfile", "dockerfile"],
    ["docker", "dockerfile"],
    ["gql", "graphql"],
    ["kt", "kotlin"],
    ["c++", "cpp"],
    ["cs", "csharp"],
    ["C#", "csharp"],
    ["rb", "ruby"],
    ["tf", "hcl"],
    ["terraform", "hcl"],
    ["Makefile", "make"],
    ["protobuf", "proto"],
    ["language-python", "python"],
    ["ts title=app.ts", "ts"],
    ["ts{1,3}", "ts"],
    ["caddyfile", "text"],
    ["brainfuck", "text"],
    ["constructor", "text"],
    ["__proto__", "text"],
    ["", "text"],
    [undefined, "text"],
  ] as const)("reads %s as %s", (info, lang) => {
    expect(resolveCodeLang(info)).toBe(lang);
  });

  it("knows every language it offers by its own name", () => {
    for (const lang of CODE_LANGS) expect(resolveCodeLang(lang)).toBe(lang);
  });
});

describe("highlight", () => {
  it("leaves plain text alone and refuses an unknown language", async () => {
    expect(await highlight("hello", "text")).toBeNull();
    expect(await highlight("puts 1", "brainfuck" as CodeLang)).toBeNull();
  });

  it("loads every language it offers on demand and keeps the code as written", async () => {
    for (const lang of CODE_LANGS) {
      const code = "x = 1\n\n\ty";
      const lines = await highlight(code, lang);
      expect(lines?.map((line) => line.map((t) => t.content).join("")).join("\n"), lang).toBe(code);
    }
  }, 60_000);

  it("splits JSON into keys, values and punctuation", async () => {
    expect(await highlight('{"a":1}', "json")).toEqual([
      [
        { content: "{", className: punctuation },
        { content: '"', className: punctuation },
        { content: "a", className: property },
        { content: '"', className: punctuation },
        { content: ":", className: punctuation },
        { content: "1", className: constant },
        { content: "}", className: punctuation },
      ],
    ]);
  });

  it("returns nothing for empty code, one line per CRLF line, and keeps tabs", async () => {
    expect(await highlight("", "ts")).toEqual([[]]);
    const lines = await highlight("const a = 1;\r\n\tconst b = 2;\r\n", "ts");
    expect(lines?.map((line) => line.map((t) => t.content).join(""))).toEqual(["const a = 1;", "\tconst b = 2;", ""]);
  });

  it("leaves code beyond the size cap plain, and keeps long lines whole", async () => {
    expect(await highlight("1".repeat(HIGHLIGHT_MAX_CHARS + 1), "json")).toBeNull();
    const line = `[${"1,".repeat(3_000)}1]`;
    const tokens = await highlight(line, "json");
    expect(tokens?.[0]?.map((t) => t.content).join("")).toBe(line);
  });

  it("colours a shell session's commands and prompts and leaves its output plain", async () => {
    const lines = await highlight("$ pnpm install --frozen-lockfile \\\n  --prod\nPackages: +412\n\n(venv) dev@kaeso:~/app$ echo $HOME", "shellsession");
    const parts = (i: number) => lines?.[i]?.filter((t) => t.content.trim()).map((t) => [t.content.trim(), t.className]);
    expect(parts(0)).toEqual([
      ["$", punctuation],
      ["pnpm", fn],
      ["install", ""],
      ["--frozen-lockfile", constant],
      ["\\", constant],
    ]);
    expect(parts(1)).toEqual([["--prod", constant]]);
    expect(lines?.[2]).toEqual([{ content: "Packages: +412", className: "" }]);
    expect(lines?.[3]).toEqual([]);
    expect(lines?.[4]?.slice(0, 3)).toEqual([
      { content: "(venv) dev@kaeso:~/app$ ", className: punctuation },
      { content: "echo", className: fn },
      { content: " ", className: "" },
    ]);
    expect(lines?.[4]?.at(-1)).toEqual({ content: "$HOME", className: variable });
  });

  it("highlights a shell session without prompts as commands", async () => {
    expect(await tones("docker compose up -d\ncurl -s $KAESO_URL/healthz", "shellsession")).toMatchObject({
      docker: fn,
      "-d": constant,
      curl: fn,
      $KAESO_URL: variable,
    });
  });
});

describe("highlight colours each language family", () => {
  it("TypeScript and JavaScript", async () => {
    expect(
      await tones(
        'import { Kaeso } from "@kaeso/sdk";\n// route it\nconst client = new Kaeso({ baseURL: `https://${host}/v1`, retries: 3, stream: true });\ninterface Model { id: string }\nexport function route<T>(key: string): T {}',
        "ts",
      ),
    ).toMatchObject({
      import: keyword,
      '"@kaeso/sdk"': string,
      "// route it": comment,
      const: keyword,
      new: keyword,
      baseURL: property,
      "`https://": string,
      "${": keyword,
      host: "",
      "3": constant,
      true: constant,
      interface: keyword,
      Model: type,
      string: type,
      route: fn,
      T: type,
    });
    expect(await tones("module.exports = async (key) => await fetch(`/v1/models/${key}`, { method: 'GET' });", "js")).toMatchObject({
      async: keyword,
      fetch: fn,
      method: property,
      "'GET'": string,
      "=>": punctuation,
    });
  });

  it("JSX and TSX", async () => {
    const expected = { Row: tag, label: attribute, '"keys"': string, open: fn };
    expect(await tones('<Row label="keys" onClick={() => open(name)} />', "tsx")).toMatchObject(expected);
    expect(await tones('<Row label="keys" onClick={() => open(name)} />', "jsx")).toMatchObject(expected);
  });

  it("Python, f-strings included", async () => {
    expect(
      await tones(
        '@retry(times=3)\ndef ask(prompt: str) -> str:\n    """Send one prompt."""\n    # stream it\n    return f"Answer: {prompt}" if prompt else None\nclass Router(Base):\n    pass',
        "python",
      ),
    ).toMatchObject({
      retry: fn,
      def: keyword,
      ask: fn,
      str: type,
      '"""Send one prompt."""': string,
      "# stream it": comment,
      'f"Answer:': string,
      "{": keyword,
      None: constant,
      Router: type,
      Base: type,
    });
  });

  it("shell: commands, flags, variables, strings and heredocs", async () => {
    expect(
      await tones(
        'export KAESO_URL="https://kaeso.example.com"\n# create a key\ncurl -sS "$KAESO_URL/v1/models" -H "Authorization: Bearer ${KAESO_API_KEY}"\ncat <<EOF > .env\nKAESO_MASTER_KEY=$KEY\nEOF\nif [[ -z "$1" ]]; then exit 1; fi',
        "bash",
      ),
    ).toMatchObject({
      export: keyword,
      KAESO_URL: variable,
      '"https://kaeso.example.com"': string,
      "# create a key": comment,
      curl: fn,
      "-sS": constant,
      $KAESO_URL: variable,
      "${KAESO_API_KEY}": variable,
      "KAESO_MASTER_KEY=": string,
      $KEY: variable,
      if: keyword,
      $1: variable,
    });
    expect(await tones('$env:KAESO_API_KEY = "sk-kso-abc"\n# call it\nInvoke-RestMethod -Uri $url', "powershell")).toMatchObject({
      "$env:KAESO_API_KEY": variable,
      '"sk-kso-abc"': string,
      "# call it": comment,
      "Invoke-RestMethod": fn,
    });
  });

  it("configuration: YAML, TOML, INI, .env and JSON with comments", async () => {
    expect(await tones("services:\n  gateway:\n    restart: unless-stopped\n    port: 8080\n    debug: false # off", "yaml")).toMatchObject({
      services: property,
      restart: property,
      "unless-stopped": string,
      "8080": constant,
      false: constant,
      "# off": comment,
    });
    expect(await tones('[package]\nname = "kaeso-kpp"\nedition = 2024 # year', "toml")).toMatchObject({
      package: type,
      name: property,
      '"kaeso-kpp"': string,
      "2024": constant,
      "# year": comment,
    });
    expect(await tones("[server]\nhost = kaeso.example.com\n; local", "ini")).toMatchObject({ server: type, host: property, "; local": comment });
    expect(await tones('# Kaeso\nKAESO_MASTER_KEY=sk-kso-abc\nKAESO_DATABASE_URL="postgres://kaeso@db/kaeso"', "dotenv")).toMatchObject({
      "# Kaeso": comment,
      KAESO_MASTER_KEY: property,
      "sk-kso-abc": string,
      '"postgres://kaeso@db/kaeso"': string,
    });
    expect(await tones('{\n  // the route\n  "route": "default"\n}', "jsonc")).toMatchObject({ "// the route": comment, route: property, '"default"': string });
  });

  it("diffs: file headers, ranges, added and removed lines", async () => {
    expect(await tones("--- a/compose.yaml\n+++ b/compose.yaml\n@@ -1,2 +1,2 @@\n-  image: kaeso:0.5\n+  image: kaeso:0.6", "diff")).toMatchObject({
      "--- a/compose.yaml": "text-code-deleted",
      "+++ b/compose.yaml": "text-code-inserted",
      "@@": punctuation,
      "-1,2 +1,2": keyword,
      "-  image: kaeso:0.5": "text-code-deleted",
      "+  image: kaeso:0.6": "text-code-inserted",
    });
  });

  it("systems languages: Rust, Go, C and C++", async () => {
    expect(await tones('#[derive(Debug)]\npub fn new(name: &str) -> Result<Self, Error> {\n    println!("route {}", 42); // done\n}', "rust")).toMatchObject({
      derive: attribute,
      pub: keyword,
      new: fn,
      str: type,
      Result: type,
      "println!": fn,
      '"route': string,
      "42": constant,
      "// done": comment,
    });
    expect(await tones('func (r *Route) Pick(keys []string) (string, error) {\n\treturn "", nil\n}', "go")).toMatchObject({
      func: keyword,
      Route: type,
      Pick: fn,
      string: type,
      error: type,
      '""': string,
      nil: constant,
    });
    expect(await tones('#include <stdio.h>\nint main(void) {\n    printf("%d", 0);\n}', "c")).toMatchObject({
      "#include": attribute,
      int: type,
      main: fn,
      printf: fn,
      "0": constant,
    });
    expect(await tones("namespace kaeso {\nclass Router : public Base {};\n}", "cpp")).toMatchObject({
      namespace: keyword,
      kaeso: type,
      Router: type,
      public: keyword,
    });
  });

  it("JVM and .NET languages: Java, Kotlin, Swift and C#", async () => {
    expect(await tones('@Service\npublic class Router {\n    public List<String> pick(String key) { return List.of("a"); }\n}', "java")).toMatchObject({
      Service: attribute,
      public: keyword,
      Router: type,
      List: type,
      String: type,
      pick: fn,
      '"a"': string,
    });
    expect(await tones('data class Route(val name: String)\nfun main() { println("route ${name}") }', "kotlin")).toMatchObject({
      data: keyword,
      Route: type,
      String: type,
      main: fn,
      '"route': string,
      "${": keyword,
    });
    expect(await tones('struct Route: Codable {\n    func pick() -> String? { return nil }\n}\nprint("route \\(name)")', "swift")).toMatchObject({
      struct: keyword,
      Route: type,
      pick: fn,
      nil: constant,
      "\\(": keyword,
    });
    expect(await tones('public sealed class Router : IRouter {\n    public Task<string> Pick() => $"route {key}";\n}', "csharp")).toMatchObject({
      public: keyword,
      Router: type,
      IRouter: type,
      Pick: fn,
      '$"route': string,
    });
    expect(await tones('var name = $"route {key}";', "csharp")).toMatchObject({ var: keyword, '$"route': string, "{": keyword, key: "" });
  });

  it("scripting languages: PHP, Ruby and Lua", async () => {
    expect(await tones('<?php\nfunction pick(string $key): ?string { return "route {$key}"; }', "php")).toMatchObject({
      function: keyword,
      pick: fn,
      string: type,
      $key: variable,
    });
    expect(await tones('class Router < Base\n  def initialize(name)\n    @name = name\n    puts "route #{name}"\n  end\nend', "ruby")).toMatchObject({
      class: keyword,
      Router: type,
      def: keyword,
      initialize: fn,
      "@name": variable,
      "#{": keyword,
    });
    expect(await tones("local M = {}\n-- a router\nreturn nil", "lua")).toMatchObject({ local: keyword, "-- a router": comment, nil: constant });
  });

  it("the web: HTML, CSS, SCSS, XML, Vue and Svelte", async () => {
    expect(await tones('<!-- console -->\n<a href="https://kaeso.example.com">Kaeso</a>\n<script>const n = 1;</script>', "html")).toMatchObject({
      "<!-- console -->": comment,
      a: tag,
      href: attribute,
      '"https://kaeso.example.com"': string,
      Kaeso: "",
      const: keyword,
    });
    expect(await tones(".code-well > pre:hover {\n  color: var(--k-text);\n  margin: 16px;\n}", "css")).toMatchObject({
      "code-well": attribute,
      pre: tag,
      color: property,
      var: fn,
      "--k-text": variable,
      "16px": constant,
    });
    expect(await tones("$gutter: 24px;\n.code { padding: $gutter; }", "scss")).toMatchObject({ $gutter: variable, "24px": constant, padding: property });
    expect(await tones('<project xmlns="http://maven.apache.org/POM/4.0.0"><!-- pom --></project>', "xml")).toMatchObject({
      project: tag,
      xmlns: attribute,
      "<!-- pom -->": comment,
    });
    expect(await tones('<template>\n  <button @click="pick">Pick</button>\n</template>', "vue")).toMatchObject({
      template: tag,
      button: tag,
      "@click": attribute,
    });
    expect(await tones('<script lang="ts">\n  export let name: string;\n</script>', "svelte")).toMatchObject({
      script: tag,
      lang: attribute,
      export: keyword,
      string: type,
    });
  });

  it("APIs and data: SQL, GraphQL, HTTP and Protocol Buffers", async () => {
    expect(await tones("-- per model\nSELECT model, COUNT(*) FROM request_logs WHERE day > '2026-01-01' LIMIT 10;", "sql")).toMatchObject({
      "-- per model": comment,
      SELECT: keyword,
      COUNT: fn,
      "'2026-01-01'": string,
      "10": constant,
    });
    expect(await tones("query Models($first: Int = 10) {\n  models(first: $first) { id }\n}\nfragment F on Model { name }", "graphql")).toMatchObject({
      query: keyword,
      Models: fn,
      $first: variable,
      Int: type,
      "10": constant,
    });
    expect(await tones("POST /v1/responses HTTP/1.1\nAuthorization: Bearer sk-kso-abc", "http")).toMatchObject({
      POST: keyword,
      Authorization: property,
      "Bearer sk-kso-abc": string,
    });
    expect(await tones('syntax = "proto3";\nmessage Model {\n  string id = 1;\n}', "proto")).toMatchObject({
      syntax: keyword,
      '"proto3"': string,
      message: keyword,
      Model: type,
      "1": constant,
    });
  });

  it("infrastructure: Dockerfile, HCL, nginx and Makefile", async () => {
    expect(await tones('FROM node:24-alpine AS build\n# serve it\nCMD ["node", "server.js"]', "dockerfile")).toMatchObject({
      FROM: keyword,
      AS: keyword,
      "# serve it": comment,
      CMD: keyword,
      '"node"': string,
    });
    expect(await tones('resource "aws_instance" "kaeso" {\n  count = 2\n  enabled = true # on\n}', "hcl")).toMatchObject({
      resource: keyword,
      count: property,
      "2": constant,
      true: constant,
      "# on": comment,
    });
    expect(await tones("server {\n    listen 443 ssl;\n    proxy_set_header Host $host;\n}", "nginx")).toMatchObject({
      listen: keyword,
      "443": constant,
      $host: variable,
    });
    expect(await tones("BIN := kaeso\nbuild: deps\n\tcargo build -p $(BIN)", "make")).toMatchObject({ BIN: variable, build: fn, "$(BIN)": variable });
  });

  it("Markdown: headings, emphasis, code and links", async () => {
    expect(await tones("# Kaeso gateway\nUse **bold**, `pnpm install` and [the docs](https://docs.kaeso.app).", "markdown")).toMatchObject({
      "# Kaeso gateway": `${fn} font-semibold`,
      bold: "font-semibold",
      "pnpm install": string,
      "the docs": fn,
      "https://docs.kaeso.app": string,
    });
  });
});
