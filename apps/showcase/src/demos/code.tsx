import { Code } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Code",
  description: "Inline code: Geist Mono 13 on a soft fill, radius 6 in running text.",
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
    <>
      <StateSection title="Shapes">
        <div className="flex items-center gap-12">
          <Code>gpt-5</Code>
          <Code>curl -s localhost:8080/healthz</Code>
          <Code shape="pill">pill</Code>
          <Code shape="rounded">rounded</Code>
        </div>
      </StateSection>
      <StateSection title="In running text at 280px: box, text and cell">
        <div className="flex w-280 flex-col gap-16 text-body text-fg-secondary">
          <p>
            Set <Code>KAESO_MASTER_KEY</Code> to <Code>sk-kso-AbC123dEf456gHi789jKl012mNo345</Code>, then call the gateway.
          </p>
          <p>
            Print the document with <Code flow="text">docker compose exec gateway kaeso openapi</Code>; every operation names its right.
          </p>
          <table className="w-full text-small">
            <tbody>
              <tr>
                <td className="w-1/2 align-top">Model</td>
                <td>
                  <Code flow="cell">claude-sonnet-4-5-20250929</Code>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </StateSection>
    </>
  );
}
