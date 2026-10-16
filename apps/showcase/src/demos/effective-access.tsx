import { Button, EffectiveAccess } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";
import { PROJECT_GROUPS, withGrants } from "~/fixtures/rights";

export const meta = {
  title: "EffectiveAccess",
  description: "What someone can do at a glance: a row per group, in words, under the roles it comes from.",
  group: "Kaeso",
  entry: "kaeso",
  import: "EffectiveAccess",
  a11y: ["A definition list: each group is a term, what is granted its description.", "A source chip with a detail says it in a tooltip."],
};
export const examples = [{ id: "member", title: "A member in a project", fn: "Member" }];

const DEVELOPER = [
  "project.view",
  "keys.own.create",
  "keys.all.view",
  "models.view",
  "playground.use",
  "logs.metadata.view",
  "logs.content.view",
  "project.members.view",
];

export function Member() {
  return (
    <EffectiveAccess
      className="w-560"
      aria-label="Maya Chen's access in Support bot"
      sources={[
        { label: "Developer", detail: "Project role" },
        { label: "Billing", detail: "Organization role" },
      ]}
      groups={withGrants(PROJECT_GROUPS, { Developer: DEVELOPER })}
      action={<Button size="sm">See every right</Button>}
    />
  );
}
export function States() {
  return (
    <>
      <StateSection title="Every right and none">
        <EffectiveAccess
          className="w-560"
          groups={withGrants(PROJECT_GROUPS.slice(0, 3), {
            Admin: PROJECT_GROUPS.flatMap((g) => g.rights.map((r) => r.code)).filter((c) => !["models.view", "models.manage", "playground.use"].includes(c)),
          })}
        />
      </StateSection>
      <StateSection title="Phone width">
        <EffectiveAccess
          className="w-320"
          sources={[{ label: "Viewer" }]}
          groups={withGrants(PROJECT_GROUPS, { Viewer: ["project.view", "keys.all.view", "models.view", "logs.metadata.view"] })}
        />
      </StateSection>
    </>
  );
}
