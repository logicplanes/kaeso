import { useState } from "react";
import { RightsMatrix, type RightChange, type RightsGroup } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";
import { PROJECT_GROUPS, withGrants } from "~/fixtures/rights";

export const meta = {
  title: "RightsMatrix",
  description: "Rights by group: checkboxes that keep requirements, or marks with the roles that grant them.",
  group: "Kaeso",
  entry: "kaeso",
  import: "RightsMatrix",
  keyboard: [
    ["Space", "Grants or withdraws the focused right, or every right of its group"],
    ["↑ ↓", "Moves between checkboxes"],
    ["Home End", "First or last checkbox"],
  ],
  a11y: [
    "Each checkbox is named by the right's name and described by its description.",
    "A group's checkbox is mixed when some of its rights are granted.",
    "Granting a right grants what it needs; the change is announced politely.",
    "⚠ means the right exposes sensitive data or is hard to undo; its tooltip says so.",
    "A right the person can't grant is disabled, and its row's tooltip says why.",
  ],
};
export const examples = [
  { id: "role", title: "A custom role", fn: "Role" },
  { id: "member", title: "Against a role", fn: "Member" },
  { id: "effective", title: "Read-only, with sources", fn: "Effective" },
];

const apply = (gs: RightsGroup[], changes: RightChange[]) =>
  gs.map((g) => ({ ...g, rights: g.rights.map((r) => ({ ...r, granted: changes.find((c) => c.code === r.code)?.granted ?? r.granted })) }));

export function Role() {
  const [groups, setGroups] = useState(PROJECT_GROUPS);
  return (
    <RightsMatrix className="w-640" searchable aria-label="Rights of Support lead" groups={groups} onRightsChange={(c) => setGroups((gs) => apply(gs, c))} />
  );
}

const DEVELOPER = new Set([
  "project.view",
  "keys.own.create",
  "keys.all.view",
  "models.view",
  "playground.use",
  "logs.metadata.view",
  "logs.content.view",
  "project.members.view",
]);
const HELD = new Set([...DEVELOPER, "models.manage", "keys.all.manage", "project.update"]);

export function Member() {
  const [granted, setGranted] = useState(() => new Set([...DEVELOPER, "models.manage"].filter((c) => c !== "logs.content.view")));
  const groups = PROJECT_GROUPS.map((g) => ({
    ...g,
    rights: g.rights.map((r) => {
      const inRole = DEVELOPER.has(r.code);
      const on = granted.has(r.code);
      return {
        ...r,
        granted: on,
        origin: inRole ? (on ? ("preset" as const) : ("removed" as const)) : on ? ("added" as const) : ("none" as const),
        ...(HELD.has(r.code) ? {} : { disabled: true, disabledReason: "You can grant only rights you hold" }),
      };
    }),
  }));
  return (
    <RightsMatrix
      className="w-640"
      aria-label="Rights of Maya Chen"
      originLabels={{ preset: "Developer" }}
      groups={groups}
      onRightsChange={(changes) =>
        setGranted((s) => {
          const next = new Set(s);
          for (const c of changes) {
            if (c.granted) next.add(c.code);
            else next.delete(c.code);
          }
          return next;
        })
      }
    />
  );
}

const EFFECTIVE = withGrants(PROJECT_GROUPS, {
  Developer: [
    "project.view",
    "keys.own.create",
    "keys.all.view",
    "models.view",
    "playground.use",
    "logs.metadata.view",
    "logs.content.view",
    "project.members.view",
  ],
  "Support bot · Viewer": ["project.view", "keys.all.view", "models.view", "logs.metadata.view", "project.members.view"],
});

export function Effective() {
  return <RightsMatrix className="w-640" readOnly searchable aria-label="Maya Chen's rights in Support bot" groups={EFFECTIVE} />;
}

export function States() {
  const [groups, setGroups] = useState(PROJECT_GROUPS.slice(0, 2));
  return (
    <>
      <StateSection title="Edit: requirements, mixed group, sensitive">
        <RightsMatrix className="w-640" groups={groups} onRightsChange={(c) => setGroups((gs) => apply(gs, c))} />
      </StateSection>
      <StateSection title="Against a role, with rights that can't be granted">
        <Member />
      </StateSection>
      <StateSection title="Read-only with sources">
        <RightsMatrix className="w-640" readOnly groups={EFFECTIVE.slice(1, 3)} />
      </StateSection>
      <StateSection title="Code names only">
        <RightsMatrix
          className="w-640"
          groups={[
            {
              label: "keys.*",
              rights: [
                { code: "keys.own.create", description: "Create one's own user keys", granted: true, origin: "preset" },
                { code: "keys.all.manage", description: "Disable or archive anyone's keys in the project", granted: false, origin: "none" },
              ],
            },
          ]}
        />
      </StateSection>
      <StateSection title="Phone width">
        <div className="w-360">
          <RightsMatrix searchable groups={EFFECTIVE.slice(1, 2)} />
        </div>
      </StateSection>
    </>
  );
}
