import { describe, expect, it, vi } from "vitest";
import { useState } from "react";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { RightsMatrix, type RightsGroup } from "./rights";
import { groupCascade, rightsCascade, type RightChange } from "./rights-logic";

const groups = [
  {
    label: "Keys",
    rights: [
      { code: "keys.own.create", description: "Create keys for yourself", granted: true, origin: "preset" as const },
      { code: "keys.all.manage", description: "Manage every key", granted: false, origin: "none" as const, sensitive: true },
      {
        code: "keys.platform_access.grant",
        description: "Give keys platform access",
        granted: false,
        origin: "none" as const,
        disabled: true,
        disabledReason: "Only rights you hold can be granted",
      },
    ],
  },
];

const named: RightsGroup[] = [
  {
    label: "Keys",
    description: "API keys of members and service accounts.",
    rights: [
      { code: "keys.view", label: "View keys", description: "See every key and its usage", granted: false },
      { code: "keys.update", label: "Edit keys", description: "Rename keys and change their limits", granted: false, requires: ["keys.view"] },
      { code: "keys.revoke", label: "Revoke keys", description: "Disable or archive any key", granted: false, requires: ["keys.update"] },
    ],
  },
  {
    label: "Logs",
    rights: [
      {
        code: "logs.view",
        label: "View logs",
        description: "See request metadata",
        granted: true,
        sources: [{ label: "Developer", detail: "Organization role" }],
      },
    ],
  },
];

function Editable({ onChanges }: { onChanges?: (c: RightChange[]) => void }) {
  const [gs, setGs] = useState(named);
  return (
    <RightsMatrix
      searchable
      groups={gs}
      onRightsChange={(changes) => {
        onChanges?.(changes);
        setGs((prev) =>
          prev.map((g) => ({ ...g, rights: g.rights.map((r) => ({ ...r, granted: changes.find((c) => c.code === r.code)?.granted ?? r.granted })) })),
        );
      }}
    />
  );
}

describe("RightsMatrix", () => {
  it("toggles a right by its code and counts granted rights", async () => {
    const onChange = vi.fn();
    render(<RightsMatrix groups={groups} onChange={onChange} />);
    expect(screen.getByText("1 of 3")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("checkbox", { name: /^keys\.all\.manage/ }));
    expect(onChange).toHaveBeenCalledWith("keys.all.manage", true);
  });
  it("marks sensitive rights and disables rights that can't be granted", () => {
    render(<RightsMatrix groups={groups} />);
    expect(screen.getByRole("img", { name: "Sensitive" })).toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: /^keys\.platform_access\.grant/ })).toHaveAttribute("aria-disabled", "true");
  });
  it("grants what a right requires and says so", async () => {
    const onChanges = vi.fn();
    render(<Editable onChanges={onChanges} />);
    expect(screen.getByText("Needs Edit keys")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("checkbox", { name: "Revoke keys" }));
    expect(onChanges).toHaveBeenLastCalledWith([
      { code: "keys.revoke", granted: true },
      { code: "keys.update", granted: true },
      { code: "keys.view", granted: true },
    ]);
    expect(screen.getByRole("checkbox", { name: "View keys" })).toBeChecked();
    expect(screen.getByText("Also granted Edit keys, View keys")).toBeInTheDocument();
  });
  it("withdraws what requires a right", async () => {
    const onChanges = vi.fn();
    render(<Editable onChanges={onChanges} />);
    await userEvent.click(screen.getByRole("checkbox", { name: "Revoke keys" }));
    await userEvent.click(screen.getByRole("checkbox", { name: "View keys" }));
    expect(onChanges).toHaveBeenLastCalledWith([
      { code: "keys.view", granted: false },
      { code: "keys.update", granted: false },
      { code: "keys.revoke", granted: false },
    ]);
    expect(screen.getByRole("checkbox", { name: "Revoke keys" })).not.toBeChecked();
  });
  it("toggles a whole group and shows a mixed group", async () => {
    render(<Editable />);
    const all = screen.getByRole("checkbox", { name: "Every right in Keys" });
    await userEvent.click(screen.getByRole("checkbox", { name: "View keys" }));
    expect(all).toHaveAttribute("aria-checked", "mixed");
    await userEvent.click(all);
    expect(screen.getByRole("checkbox", { name: "Revoke keys" })).toBeChecked();
    expect(all).toBeChecked();
    await userEvent.click(all);
    expect(screen.getByRole("checkbox", { name: "View keys" })).not.toBeChecked();
  });
  it("filters rights by name, code or description", async () => {
    render(<Editable />);
    await userEvent.type(screen.getByRole("searchbox", { name: "Search rights" }), "archive");
    expect(screen.getByRole("checkbox", { name: "Revoke keys" })).toBeInTheDocument();
    expect(screen.queryByRole("checkbox", { name: "View keys" })).not.toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Logs" })).not.toBeInTheDocument();
    await userEvent.clear(screen.getByRole("searchbox", { name: "Search rights" }));
    await userEvent.type(screen.getByRole("searchbox", { name: "Search rights" }), "nothing like it");
    expect(screen.getByText("No rights match")).toBeInTheDocument();
  });
  it("moves between checkboxes with the arrow keys", async () => {
    render(<Editable />);
    screen.getByRole("checkbox", { name: "Every right in Keys" }).focus();
    await userEvent.keyboard("{ArrowDown}");
    expect(screen.getByRole("checkbox", { name: "View keys" })).toHaveFocus();
    await userEvent.keyboard("{End}");
    expect(screen.getByRole("checkbox", { name: "View logs" })).toHaveFocus();
    await userEvent.keyboard("{Home}");
    expect(screen.getByRole("checkbox", { name: "Every right in Keys" })).toHaveFocus();
  });
  it("shows marks and sources in read-only mode", () => {
    render(<RightsMatrix readOnly groups={named} />);
    expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
    const logs = screen.getByRole("region", { name: "Logs" });
    expect(within(logs).getByRole("img", { name: "Granted" })).toBeInTheDocument();
    expect(within(logs).getByText("Developer")).toBeInTheDocument();
    expect(screen.getAllByRole("img", { name: "Not granted" })).toHaveLength(3);
    expect(screen.queryByText(/^Needs /)).not.toBeInTheDocument();
  });
});

describe("rightsCascade", () => {
  const rights = [
    { code: "a", granted: false },
    { code: "b", granted: false, requires: ["a"] },
    { code: "c", granted: false, requires: ["b"] },
    { code: "d", granted: false, requires: ["a"], disabled: true },
  ];
  it("follows requirements both ways and skips disabled rights", () => {
    expect(rightsCascade(rights, "c", true)).toEqual([
      { code: "c", granted: true },
      { code: "b", granted: true },
      { code: "a", granted: true },
    ]);
    const on = rights.map((r) => ({ ...r, granted: !r.disabled }));
    expect(rightsCascade(on, "a", false)).toEqual([
      { code: "a", granted: false },
      { code: "b", granted: false },
      { code: "c", granted: false },
    ]);
  });
  it("grants a group with what it needs", () => {
    expect(groupCascade(rights, [rights[2]!], true).map((c) => c.code)).toEqual(["c", "b", "a"]);
  });
});

describe("RightsMatrix requirements", () => {
  it("doesn't say a requirement every right has", () => {
    render(
      <RightsMatrix
        groups={[
          {
            label: "Models",
            rights: [
              { code: "project.view", label: "View project", description: "See it", granted: true },
              { code: "models.view", label: "View models", description: "See models", granted: false, requires: ["project.view"] },
              { code: "models.manage", label: "Manage models", description: "Change models", granted: false, requires: ["project.view", "models.view"] },
            ],
          },
        ]}
      />,
    );
    expect(screen.getByText("Needs View models")).toBeInTheDocument();
    expect(screen.queryByText(/Needs View project/)).not.toBeInTheDocument();
  });
});
