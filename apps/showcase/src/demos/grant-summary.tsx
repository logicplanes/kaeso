import { GrantSummary } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "GrantSummary",
  description: "What an agent may do in a few short lines, each after a check mark, as on its consent page.",
  group: "Kaeso",
  entry: "kaeso",
  import: "GrantSummary",
  a11y: ["A list named by `aria-label`; the check marks are decorative."],
};
export const examples = [{ id: "agent", title: "An agent's rights", fn: "Agent" }];

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

export function States() {
  return (
    <>
      <StateSection title="With a note">
        <Agent />
      </StateSection>
      <StateSection title="One line, no note">
        <GrantSummary className="w-360" aria-label="What it can do" items={["Reads your profile, nothing else"]} />
      </StateSection>
    </>
  );
}
