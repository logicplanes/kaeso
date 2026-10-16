import type { RightsGroup, RoleOption } from "@logicplanes/kaeso-ui";

export const PROJECT_GROUPS: RightsGroup[] = [
  {
    label: "Project",
    description: "The project's name, settings and picture.",
    rights: [
      { code: "project.view", label: "View project", description: "See the project and its settings", granted: true },
      {
        code: "project.update",
        label: "Edit project",
        description: "Rename it and change its settings and picture",
        granted: false,
        requires: ["project.view"],
      },
      {
        code: "project.archive",
        label: "Archive project",
        description: "Archive the project and every key in it",
        granted: false,
        sensitive: true,
        requires: ["project.update"],
      },
    ],
  },
  {
    label: "Keys",
    description: "API keys of members and service accounts.",
    rights: [
      { code: "keys.own.create", label: "Create own keys", description: "Create user keys for themselves", granted: true },
      { code: "keys.all.view", label: "View every key", description: "See every key in the project, never its secret", granted: true },
      { code: "keys.all.manage", label: "Manage every key", description: "Disable or archive anyone's keys", granted: false, requires: ["keys.all.view"] },
      {
        code: "keys.platform_access.grant",
        label: "Give platform access",
        description: "Let a new key call the Platform API",
        granted: false,
        sensitive: true,
        requires: ["keys.all.manage"],
      },
    ],
  },
  {
    label: "Models",
    description: "Models, aliases and routing.",
    rights: [
      { code: "models.view", label: "View models", description: "See configured models, aliases and routing", granted: true },
      { code: "models.manage", label: "Manage models", description: "Change models, aliases and routing", granted: false, requires: ["models.view"] },
      {
        code: "playground.use",
        label: "Use the Playground",
        description: "Call the project's models from the console",
        granted: true,
        requires: ["models.view"],
      },
    ],
  },
  {
    label: "Logs",
    description: "Request logs and their content.",
    rights: [
      { code: "logs.metadata.view", label: "View logs", description: "Model, tokens, cost, latency and status of each request", granted: true },
      {
        code: "logs.content.view",
        label: "View request content",
        description: "Prompts and outputs of each request",
        granted: false,
        sensitive: true,
        requires: ["logs.metadata.view"],
      },
      { code: "logs.export", label: "Export logs", description: "Download logs as a file", granted: false, sensitive: true, requires: ["logs.metadata.view"] },
    ],
  },
  {
    label: "Members",
    description: "Who is in the project and what they can do.",
    rights: [
      { code: "project.members.view", label: "View members", description: "See project members and their roles", granted: true },
      {
        code: "project.members.add",
        label: "Add members",
        description: "Add organization members to the project",
        granted: false,
        requires: ["project.members.view"],
      },
      {
        code: "project.members.roles.update",
        label: "Change roles",
        description: "Give members another project role",
        granted: false,
        requires: ["project.members.view"],
      },
      {
        code: "project.members.remove",
        label: "Remove members",
        description: "Take members out of the project",
        granted: false,
        requires: ["project.members.view"],
      },
    ],
  },
];

export const ORG_ROLES: RoleOption[] = [
  {
    value: "owner",
    label: "Owner",
    description: "Everything, including deleting the organization",
    disabled: true,
    disabledReason: "Only an owner can make someone an owner",
  },
  { value: "admin", label: "Admin", description: "Everything except ownership and deletion", hint: "29 rights" },
  { value: "billing", label: "Billing", description: "Usage, costs and the plan", hint: "8 rights" },
  { value: "developer", label: "Developer", description: "Create projects, see provider keys and usage", hint: "7 rights" },
  { value: "member", label: "Member", description: "See the organization and its members", hint: "2 rights" },
  { value: "support", label: "Support lead", description: "Read logs and usage across projects to help customers", custom: true, hint: "9 rights" },
  { value: "finance", label: "Finance", description: "Costs, budgets and invoices", custom: true, hint: "6 rights" },
];

export function withGrants(groups: RightsGroup[], granted: Record<string, string[]>): RightsGroup[] {
  return groups.map((g) => ({
    ...g,
    rights: g.rights.map((r) => {
      const sources = Object.entries(granted)
        .filter(([, codes]) => codes.includes(r.code))
        .map(([label]) => ({ label, detail: label.includes("·") ? "Project role" : "Organization role" }));
      return { ...r, granted: sources.length > 0, sources };
    }),
  }));
}
