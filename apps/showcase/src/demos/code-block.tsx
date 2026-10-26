import { CodeBlock, sdkSnippets } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "CodeBlock",
  description: "Highlighted code on a well in Kaeso's monochrome theme: copy in the header or on hover, tabs, line numbers, collapse.",
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
    </>
  );
}
