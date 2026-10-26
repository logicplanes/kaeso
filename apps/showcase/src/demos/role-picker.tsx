import { useState } from "react";
import { RolePicker } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";
import { ORG_ROLES } from "~/fixtures/rights";

export const meta = {
  title: "RolePicker",
  description: "Choosing a role: built-in, then custom, each with one sentence; ungivable roles say why.",
  group: "Kaeso",
  entry: "kaeso",
  import: "RolePicker",
  keyboard: [
    ["↑ ↓", "Moves between roles in the list"],
    ["Space", "Chooses the focused role"],
  ],
  a11y: [
    "A list is a radio group; each radio is named by the role and described by its sentence.",
    "A role the person can't give stays in the list, disabled, with the reason under it and in its tooltip.",
  ],
};
export const examples = [
  { id: "list", title: "List", fn: "List" },
  { id: "select", title: "Select", fn: "Compact" },
];
export function List() {
  const [role, setRole] = useState("developer");
  return <RolePicker className="w-480" aria-label="Role in the organization" roles={ORG_ROLES} value={role} onValueChange={setRole} />;
}
export function Compact() {
  const [role, setRole] = useState("developer");
  return <RolePicker appearance="select" aria-label="Role in the organization" roles={ORG_ROLES} value={role} onValueChange={setRole} />;
}
export function States() {
  return (
    <>
      <StateSection title="Chosen, hovered, focused and unavailable">
        <RolePicker
          className="w-480"
          aria-label="Role in the project"
          value="viewer"
          roles={[
            { value: "admin", label: "Admin", description: "Everything in the project", "data-force": "hover" },
            { value: "developer", label: "Developer", description: "Call models, own keys, see logs with content", "data-force": "focus" },
            { value: "viewer", label: "Viewer", description: "Read-only, without request content" },
            {
              value: "owner",
              label: "Owner",
              description: "Billing and deleting the project",
              disabled: true,
              disabledReason: "Only an owner can make someone an owner.",
            },
          ]}
        />
      </StateSection>
      <StateSection title="Disabled">
        <RolePicker className="w-480" aria-label="Role" disabled value="member" roles={ORG_ROLES.slice(3, 5)} />
      </StateSection>
      <StateSection title="Select, full width">
        <div className="w-360">
          <RolePicker appearance="select" fullWidth label="Role" value="support" roles={ORG_ROLES} />
        </div>
      </StateSection>
      <StateSection title="Phone width">
        <RolePicker className="w-320" aria-label="Role" value="finance" roles={ORG_ROLES} />
      </StateSection>
    </>
  );
}
