import { CodeBlock, sdkSnippets } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "CodeBlock",
  description: "Highlighted code on a well in Kaeso's syntax palette, in every common language: copy in the header or on hover, tabs, line numbers, collapse.",
  group: "Components",
  entry: "code-block",
  import: "CodeBlock",
  a11y: [
    "The copy circle is named “Copy code” and confirms with “Copied”; with a title or tabs it stays in the header.",
    "The code is one tab stop, named after its title, and scrolls sideways with the arrow keys.",
  ],
};
const long = Array.from(
  { length: 30 },
  (_, i) => `  { "id": "model_${String(i + 1).padStart(2, "0")}", "provider": "${i % 2 ? "anthropic" : "openai"}" },`,
).join("\n");
export const examples = [
  { id: "tabs", title: "SDK tabs", fn: "Tabs" },
  { id: "file", title: "File with highlighted lines", fn: "File" },
  { id: "collapse", title: "Collapsed beyond 24 lines", fn: "Collapse" },
  { id: "languages", title: "Languages", fn: "Languages" },
];
export function Tabs() {
  return (
    <div className="w-full">
      <CodeBlock tabs={sdkSnippets({ baseUrl: "https://kaeso.example.com" })} />
    </div>
  );
}
export function File() {
  return (
    <div className="w-full">
      <CodeBlock
        title="compose.yaml"
        lang="yaml"
        lineNumbers
        highlightLines={[5, 6]}
        code={`services:\n  gateway:\n    image: ghcr.io/logicplanes/kaeso-gateway:0.4.0\n    environment:\n      KAESO_MASTER_KEY: \${KAESO_MASTER_KEY}\n      KAESO_DATABASE_URL: postgres://kaeso@db/kaeso\n    ports:\n      - "8080:8080"`}
      />
    </div>
  );
}
export function Collapse() {
  return (
    <div className="w-full">
      <CodeBlock lang="json" code={`[\n${long}\n]`} />
    </div>
  );
}
const languages = [
  {
    label: "Rust",
    lang: "rust" as const,
    code: `use std::time::Duration;\n\n/// Retries a request against the next route.\n#[derive(Debug, Clone)]\npub struct Retry {\n    attempts: u32,\n    backoff: Duration,\n}\n\nimpl Retry {\n    pub fn next(&self, attempt: u32) -> Option<Duration> {\n        (attempt < self.attempts).then(|| self.backoff * 2u32.pow(attempt))\n    }\n}`,
  },
  {
    label: "Go",
    lang: "go" as const,
    code: `package main\n\nimport (\n\t"fmt"\n\t"os"\n)\n\n// main lists the models a key can call.\nfunc main() {\n\tkey := os.Getenv("KAESO_API_KEY")\n\tif key == "" {\n\t\tfmt.Fprintln(os.Stderr, "set KAESO_API_KEY")\n\t\tos.Exit(1)\n\t}\n\tfmt.Printf("listing models with %d characters of key\\n", len(key))\n}`,
  },
  {
    label: "SQL",
    lang: "sql" as const,
    code: `-- Requests and cost per model in the last 7 days\nSELECT model, count(*) AS requests, sum(cost_usd) AS cost\nFROM request_logs\nWHERE project = 'checkout' AND created_at > now() - INTERVAL '7 days'\nGROUP BY model\nORDER BY cost DESC\nLIMIT 10;`,
  },
  {
    label: "Dockerfile",
    lang: "dockerfile" as const,
    code: `FROM node:24-alpine AS build\nWORKDIR /app\nCOPY package.json pnpm-lock.yaml ./\nRUN corepack enable && pnpm install --frozen-lockfile\n# The gateway listens on 8080 inside the container\nENV PORT=8080\nCMD ["node", "dist/server.js"]`,
  },
  {
    label: ".env",
    lang: "dotenv" as const,
    code: `# Kaeso\nKAESO_URL=https://kaeso.example.com\nKAESO_API_KEY="sk-kso-AbC123"\nKAESO_TIMEOUT_SECONDS=30`,
  },
];
export function Languages() {
  return (
    <div className="w-full">
      <CodeBlock tabs={languages} />
    </div>
  );
}
const families = [
  {
    lang: "tsx" as const,
    code: `export function KeyRow({ name, created }: { name: string; created: Date }) {\n  return <Row label={name} onClick={() => open(name)}>{format(created)}</Row>;\n}`,
  },
  {
    lang: "python" as const,
    code: `@retry(times=3)\ndef ask(prompt: str) -> str:\n    """Send one prompt through Kaeso."""\n    return client.responses.create(model="claude-haiku-4-5", input=f"Answer: {prompt}").output_text`,
  },
  {
    lang: "bash" as const,
    code: `export KAESO_URL="https://kaeso.example.com"\n# List the models this key can call\ncurl -sS "$KAESO_URL/v1/models" -H "Authorization: Bearer \${KAESO_API_KEY}" | jq -r '.data[].id'`,
  },
  {
    lang: "shellsession" as const,
    code: `$ docker compose up -d --wait\n ✔ Container kaeso-gateway  Healthy\n$ curl -s https://kaeso.example.com/healthz\nok`,
  },
  { lang: "toml" as const, code: `[gateway]\nlisten = "0.0.0.0:8080"\nmax_body_bytes = 10_485_760 # 10 MiB\nstream = true` },
  { lang: "html" as const, code: `<!-- Sign-in link in an email -->\n<a href="https://kaeso.example.com/sign-in" class="button">Sign in to Kaeso</a>` },
  { lang: "css" as const, code: `.code-well > pre:hover {\n  color: var(--k-text);\n  padding: 16px;\n}` },
  {
    lang: "diff" as const,
    code: `--- a/compose.yaml\n+++ b/compose.yaml\n@@ -2,3 +2,3 @@\n   gateway:\n-    image: ghcr.io/logicplanes/kaeso-gateway:0.4.0\n+    image: ghcr.io/logicplanes/kaeso-gateway:0.4.1`,
  },
  {
    lang: "markdown" as const,
    code: `## Rotate a key\nOpen **Keys**, choose the key and select \`Rotate\`. See [API keys](https://docs.kaeso.example.com/keys).`,
  },
];
const thousand = Array.from({ length: 1003 }, (_, i) => `log.info("request ${i + 1} routed to gpt-5")`).join("\n");

export function States() {
  return (
    <>
      <StateSection title="Variants">
        <div className="flex w-640 flex-col gap-24">
          <Tabs />
          <File />
          <CodeBlock lang="bash" code="curl -s https://kaeso.example.com/healthz" />
        </div>
      </StateSection>
      <StateSection title="Collapsed, a long title, more than 999 lines, plain surface">
        <div className="flex w-640 flex-col gap-24">
          <Collapse />
          <CodeBlock title="deploy/kaeso/production/eu-west-1/gateway/compose.override.yaml" lang="yaml" code={"services:\n  gateway:\n    restart: always"} />
          <CodeBlock lang="ts" lineNumbers highlightLines={[2, 3]} collapseAfter={6} code={thousand} />
          <div className="rounded-well bg-fill-1 p-8">
            <CodeBlock surface="plain" lang="json" code={'{ "model": "gpt-5", "stream": true, "temperature": null }'} />
          </div>
        </div>
      </StateSection>
      <StateSection title="Languages">
        <div className="flex w-full max-w-640 flex-col gap-24">
          <Languages />
          {families.map((f) => (
            <CodeBlock key={f.lang} title={f.lang} lang={f.lang} code={f.code} copy={false} />
          ))}
        </div>
      </StateSection>
      <StateSection title="Wraps on phones">
        <div className="flex w-full max-w-640 flex-col gap-24">
          <CodeBlock
            lang="bash"
            wrap="phone"
            code={
              'curl https://kaeso.example.com/v1/responses \\\n  -H "Authorization: Bearer $KAESO_API_KEY" \\\n  -d \'{"model": "claude-haiku-4-5", "input": "Write a haiku about gateways."}\''
            }
          />
          <CodeBlock
            lang="bash"
            wrap
            code={'curl https://kaeso.example.com/v1/models -H "Authorization: Bearer $KAESO_API_KEY" -H "Accept: application/json"'}
          />
        </div>
      </StateSection>
      <StateSection title="Wraps at 320px between words, never inside a flag; a word wider than the line wraps inside itself">
        <div className="flex w-320 max-w-full flex-col gap-24">
          <CodeBlock lang="bash" wrap code="docker compose -f deploy/compose.yaml up -d --wait --force-recreate kaeso-gateway kaeso-console" />
          <CodeBlock lang="text" wrap code="docker compose -f deploy/compose.yaml up -d --wait --force-recreate kaeso-gateway kaeso-console" />
          <CodeBlock lang="bash" wrap code="curl https://kaeso.example.com/v1/projects/proj_01JC8Z3V5N2M4K7Q9R6T/models/claude-haiku-4-5/routes" />
        </div>
      </StateSection>
    </>
  );
}
