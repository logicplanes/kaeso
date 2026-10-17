import { Code } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Code",
  description: "Inline code: Geist Mono 13 on a soft fill. A pill for one word.",
  group: "Components",
  entry: "code",
  import: "Code",
  a11y: ["Code is a <code> element; screen readers read it as text."],
};
export const examples = [{ id: "basic", title: "Basic", fn: "Basic" }];
export function Basic() {
  return (
    <p className="max-w-prose text-body text-fg-secondary">
      Set <Code>KAESO_MASTER_KEY</Code>, then call <Code>POST /v1/responses</Code> with the model <Code>gpt-5-mini</Code>.
    </p>
  );
}
export function States() {
  return (
    <StateSection title="Shapes">
      <div className="flex items-center gap-12">
        <Code>gpt-5</Code>
        <Code>curl -s localhost:8080/healthz</Code>
        <Code shape="pill">auto</Code>
        <Code shape="rounded">rounded</Code>
      </div>
      <p className="mt-16 max-w-320 text-body text-fg-secondary">
        Print the document with <Code flow="text">docker compose exec gateway kaeso openapi</Code>; every operation names its right.
      </p>
    </StateSection>
  );
}
