import { SecretReveal, EnvironmentSnippet, Dialog, Button } from "@logicplanes/kaeso-ui";
import { SECRET } from "~/fixtures/kaeso";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "SecretReveal",
  description: "The one-time secret: calm, centered, certain. Copy it, store it, see how to use it.",
  group: "Kaeso",
  entry: "kaeso",
  import: "SecretReveal, EnvironmentSnippet",
  a11y: [
    "The secret is a read-only text box; a click selects all of it.",
    "“Copy key” becomes “Copied” without changing size.",
    "The warning is text, not only color.",
  ],
};
export const examples = [
  { id: "reveal", title: "After creating a key", fn: "Reveal", minHeight: 560 },
  { id: "environment", title: "EnvironmentSnippet", fn: "Environment", minHeight: 360 },
  { id: "platform", title: "A Platform API key", fn: "Platform", minHeight: 440 },
];
export function Reveal() {
  return (
    <div className="w-560">
      <SecretReveal secret={SECRET} name="checkout-web" baseUrl="https://gateway.northwind.example" model="gpt-5-mini" />
    </div>
  );
}
export function Platform() {
  return (
    <div className="w-560">
      <SecretReveal
        secret={SECRET}
        name="terraform"
        baseUrl="https://gateway.northwind.example"
        snippets={[
          {
            label: "curl",
            lang: "bash",
            code: `curl https://gateway.northwind.example/platform/v1/orgs/org_01/projects \\\n  -H "Authorization: Bearer ${SECRET}"`,
          },
        ]}
      />
    </div>
  );
}
export function Environment() {
  return (
    <div className="w-640">
      <EnvironmentSnippet baseUrl="https://gateway.northwind.example" />
    </div>
  );
}
export function InDialog() {
  return (
    <Dialog trigger={<Button variant="primary">Create key</Button>} title="Key created" description="checkout-web can call the gateway now." size="lg">
      <SecretReveal secret={SECRET} baseUrl="https://gateway.northwind.example" />
    </Dialog>
  );
}
export function States() {
  return (
    <StateSection title="Reveal and environment">
      <div className="flex flex-col gap-40">
        <Reveal />
        <Environment />
      </div>
    </StateSection>
  );
}
