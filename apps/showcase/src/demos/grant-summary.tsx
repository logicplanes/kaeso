import { Button, GrantSummary } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "GrantSummary",
  description: "What an agent may do in a few short lines, each after a check mark, as on its consent page.",
  group: "Kaeso",
  entry: "kaeso",
  import: "GrantSummary",
  a11y: ["A list named by `aria-label`; the check marks are decorative."],
};
export const examples = [
  { id: "agent", title: "An agent's rights", fn: "Agent" },
  { id: "tools", title: "A client's tools", fn: "Tools" },
];

export function Agent() {
  return (
    <GrantSummary
      className="w-360"
      aria-label="What it can do"
      items={["Manages projects, keys, and models in Northwind", "Reads the rest of Northwind", "Also in organizations you join later"]}
      footer="For 90 days; revoke it any time in your account settings."
    />
  );
}

export function Tools() {
  return (
    <GrantSummary
      className="w-360"
      aria-label="Tools it can call"
      sections={[
        { id: "linear", title: "Linear", hint: "4 tools", items: ["linear__get_issue", "linear__list_issues", "linear__save_issue", "linear__list_projects"] },
        { id: "github", title: "GitHub", hint: "2 tools", items: ["github__get_pull_request", "github__list_commits"], defaultOpen: true },
      ]}
      footer="6 tools in Northwind › Checkout, until you disconnect it"
    />
  );
}

export function States() {
  return (
    <>
      <StateSection title="With a note">
        <Agent />
      </StateSection>
      <StateSection title="One line, no note">
        <GrantSummary className="w-360" aria-label="What it can do" items={["Reads your profile, nothing else"]} />
      </StateSection>
      <StateSection title="Sections per server">
        <Tools />
      </StateSection>
      <StateSection title="An action without a note">
        <GrantSummary
          className="w-360"
          aria-label="What it can do"
          items={["Reads your profile, nothing else"]}
          action={<Button size="sm">Customize</Button>}
        />
      </StateSection>
    </>
  );
}
