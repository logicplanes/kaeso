import { useState } from "react";
import { screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { catalogue, roles } from "~/test/fixtures";
import { hoverReason } from "~/test/fixtures/org";
import { renderUi } from "~/test/render";
import { indexCatalogue } from "./catalogue";
import type { Grant } from "./grant";
import { GrantEditor } from "./rights-editor";
import type { Giver } from "./roles";

const cat = indexCatalogue(catalogue);
const projectRoles = roles.filter((r) => r.scope === "project");
const orgRoles = roles.filter((r) => r.scope === "org");
const owner: Giver = { held: new Set(), isOwner: true };

function Editor({ start, onChange, ...rest }: { start: Grant; onChange?: (g: Grant) => void } & Partial<Parameters<typeof GrantEditor>[0]>) {
  const [value, setValue] = useState(start);
  return (
    <GrantEditor
      cat={cat}
      scope="project"
      roles={projectRoles}
      giver={owner}
      {...rest}
      value={value}
      onChange={(g) => {
        setValue(g);
        onChange?.(g);
      }}
    />
  );
}

const checkbox = (name: RegExp) => screen.getAllByRole("checkbox").find((c) => name.test(c.closest("label")?.textContent ?? "")) as HTMLElement;

describe("GrantEditor", () => {
  it("adds and removes single rights on top of the role", async () => {
    const onChange = vi.fn();
    const { user } = await renderUi(<Editor start={{ role: "viewer", rights_added: [], rights_removed: [] }} onChange={onChange} />);

    expect(screen.getByRole("combobox", { name: "Role" })).toHaveTextContent("Viewer");
    expect(screen.getByText("Reads the project's settings, models, limits, keys, usage and log metadata; changes nothing.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /Customize/ }));
    expect(screen.getByRole("group", { name: "Rights in the project" })).toBeInTheDocument();
    await user.click(checkbox(/^Export logs/));
    expect(onChange).toHaveBeenLastCalledWith({ role: "viewer", rights_added: ["logs.export"], rights_removed: [] });
    await user.click(checkbox(/^View costs/));
    expect(onChange).toHaveBeenLastCalledWith({ role: "viewer", rights_added: ["logs.export"], rights_removed: ["costs.view"] });
    expect(screen.getByRole("button", { name: /Customize/ })).toHaveTextContent("9 rights · 2 changes");
    await user.click(checkbox(/^Export logs/));
    await user.click(checkbox(/^View costs/));
    expect(onChange).toHaveBeenLastCalledWith({ role: "viewer", rights_added: [], rights_removed: [] });
  });

  it("starts customized grants open and drops the changes with another role", async () => {
    const onChange = vi.fn();
    const { user } = await renderUi(<Editor start={{ role: "viewer", rights_added: ["logs.export"], rights_removed: [] }} onChange={onChange} labelled />);

    expect(screen.getByRole("group", { name: "Rights in the project" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Customize/ })).toHaveTextContent("10 rights · 1 change");
    await user.click(screen.getByRole("combobox", { name: "Role" }));
    await user.click(await screen.findByRole("option", { name: /Developer/ }));
    expect(onChange).toHaveBeenLastCalledWith({ role: "developer", rights_added: [], rights_removed: [] });
  });

  it("keeps people from giving roles and rights they don't hold", async () => {
    const { user } = await renderUi(
      <Editor
        start={{ role: "viewer", rights_added: [], rights_removed: [] }}
        giver={{ held: new Set(projectRoles.find((r) => r.id === "viewer")?.rights), isOwner: false }}
      />,
    );

    await user.click(screen.getByRole("button", { name: /Customize/ }));
    const edit = checkbox(/^Edit project/);
    expect(edit).toHaveAttribute("aria-disabled", "true");
    await hoverReason(user, edit, "You don't hold this right, so you can't grant it");
    await user.click(screen.getByRole("combobox", { name: "Role" }));
    expect(await screen.findByRole("option", { name: /Admin/ })).toHaveAttribute("aria-disabled", "true");
    expect(screen.getByRole("option", { name: /Admin/ })).toHaveTextContent(/You can't give it: you don't hold/);
  });

  it("locks every right of an organization owner", async () => {
    const onChange = vi.fn();
    const { user } = await renderUi(
      <Editor start={{ role: "owner", rights_added: [], rights_removed: [] }} scope="org" roles={orgRoles} onChange={onChange} defaultOpen />,
    );

    expect(screen.getByRole("group", { name: "Rights in the organization" })).toBeInTheDocument();
    const view = checkbox(/^View organization/);
    expect(view).toHaveAttribute("aria-disabled", "true");
    await hoverReason(user, view, "Owners hold every right; give another role to hold fewer");
    expect(onChange).not.toHaveBeenCalled();
  });

  it("can only be read when it's read-only", async () => {
    await renderUi(
      <Editor start={{ role: "developer", rights_added: [], rights_removed: [] }} readOnly="Archived service accounts can't change" defaultOpen />,
    );
    expect(screen.getByRole("combobox", { name: "Role" })).toBeDisabled();
    expect(checkbox(/^View project/)).toHaveAttribute("aria-disabled", "true");
  });

  it("waits for the catalogue and the roles", async () => {
    const { container } = await renderUi(
      <GrantEditor
        cat={undefined}
        scope="project"
        roles={undefined}
        value={{ role: "developer", rights_added: [], rights_removed: [] }}
        onChange={vi.fn()}
        giver={null}
      />,
    );
    expect(screen.queryByRole("combobox", { name: "Role" })).not.toBeInTheDocument();
    expect(within(container).queryByRole("group")).not.toBeInTheDocument();
  });
});
