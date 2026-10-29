import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { EffectiveAccess } from "./effective-access.js";
import { RolePicker, type RoleOption } from "./role-picker.js";

const roles: RoleOption[] = [
  {
    value: "owner",
    label: "Owner",
    description: "Everything, including deleting the organization",
    disabled: true,
    disabledReason: "Only owners can make someone an owner",
  },
  { value: "admin", label: "Admin", description: "Everything except ownership and deletion" },
  { value: "developer", label: "Developer", description: "Create projects, see provider keys and usage" },
  { value: "support", label: "Support", description: "Read logs to help customers", custom: true, hint: "6 rights" },
];

describe("RolePicker", () => {
  it("lists built-in roles, then custom ones, and picks one", async () => {
    const onValueChange = vi.fn();
    render(<RolePicker aria-label="Role" roles={roles} value="developer" onValueChange={onValueChange} />);
    expect(screen.getByText("Built-in roles")).toBeInTheDocument();
    expect(screen.getByText("Custom roles")).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "Developer" })).toBeChecked();
    await userEvent.click(screen.getByText("Support"));
    expect(onValueChange).toHaveBeenCalledWith("support");
  });
  it("keeps a role the person can't give, disabled, with the reason", () => {
    render(<RolePicker aria-label="Role" roles={roles} value="developer" />);
    expect(screen.getByRole("radio", { name: "Owner" })).toHaveAttribute("aria-disabled", "true");
    expect(screen.getByText("Only owners can make someone an owner", { ignore: "script, style, [hidden]" })).toBeVisible();
  });
  it("has no captions without custom roles", () => {
    render(<RolePicker aria-label="Role" roles={roles.filter((r) => !r.custom)} />);
    expect(screen.queryByText("Built-in roles")).not.toBeInTheDocument();
  });
});

describe("EffectiveAccess", () => {
  it("says what is granted per group in words", () => {
    render(
      <EffectiveAccess
        sources={[{ label: "Developer" }]}
        groups={[
          {
            label: "Keys",
            rights: [
              { code: "keys.view", label: "View keys", description: "", granted: true },
              { code: "keys.manage", label: "Manage keys", description: "", granted: false },
            ],
          },
          { label: "Logs", rights: [{ code: "logs.view", description: "", granted: true }] },
          { label: "Members", rights: [{ code: "members.view", description: "", granted: false }] },
        ]}
      />,
    );
    expect(screen.getByText("View keys")).toBeInTheDocument();
    expect(screen.getByText("1 of 2")).toBeInTheDocument();
    expect(screen.getByText("Every right")).toBeInTheDocument();
    expect(screen.getByText("No rights")).toBeInTheDocument();
    expect(screen.getByText("Members")).toBeInTheDocument();
    expect(screen.getByText("Developer")).toBeInTheDocument();
  });
  it("centres a group on one line in its 40px row, stacked or in columns", () => {
    render(<EffectiveAccess groups={[{ label: "Keys", rights: [{ code: "keys.view", label: "View keys", description: "", granted: true }] }]} />);
    expect(screen.getByText("Keys").parentElement).toHaveClass("min-h-40", "justify-center", "@auth:content-center");
  });
});
