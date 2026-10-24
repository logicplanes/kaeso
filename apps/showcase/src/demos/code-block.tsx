import { CodeBlock, sdkSnippets } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "CodeBlock",
  description: "Highlighted code on a well in Kaeso's monochrome theme: copy on hover, tabs, line numbers, collapse.",
  group: "Components",
  entry: "code-block",
  import: "CodeBlock",
  a11y: ["The copy circle is named “Copy code” and confirms with “Copied”."],
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
      <CodeBlock tabs={sdkSnippets({ baseUrl: "https://gateway.northwind.example" })} />
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
export function States() {
  return (
    <>
      <StateSection title="Variants">
        <div className="flex w-720 flex-col gap-24">
          <Tabs />
          <File />
          <CodeBlock lang="bash" code="curl -s https://gateway.northwind.example/healthz" />
        </div>
      </StateSection>
      <StateSection title="Wraps on phones">
        <div className="flex w-full max-w-720 flex-col gap-24">
          <CodeBlock
            lang="bash"
            wrap="phone"
            code={
              'curl https://gateway.northwind.example/v1/responses \\\n  -H "Authorization: Bearer $KAESO_API_KEY" \\\n  -d \'{"model": "claude-haiku-4-5", "input": "Write a haiku about gateways."}\''
            }
          />
        </div>
      </StateSection>
    </>
  );
}
