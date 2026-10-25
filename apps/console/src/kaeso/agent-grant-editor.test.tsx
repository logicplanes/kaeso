import { useEffect, useState } from "react";
import { screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { Grantable } from "~/api/generated/types";
import { ORG, OTHER_PROJECT, PROJECT } from "~/test/fixtures";
import { agents, grantable } from "~/test/fixtures/agents";
import { renderUi } from "~/test/render";
import { presetDraft, type AgentDraft } from "./agent-grant";
import { AgentGrantEditor, PresetLine, PresetPicker } from "./agent-grant-editor";

const seen = vi.fn<(d: AgentDraft) => void>();

function Editor({ g = grantable(), start }: { g?: Grantable; start?: AgentDraft }) {
  const [value, setValue] = useState(start ?? presetDraft(agents, g, "builder"));
  useEffect(() => seen(value), [value]);
  return (
    <>
      <PresetPicker cat={agents} grantable={g} value={value} onChange={setValue} />
      <PresetLine cat={agents} grantable={g} value={value} />
      <AgentGrantEditor cat={agents} grantable={g} value={value} onChange={setValue} defaultOpen />
    </>
  );
}

const latest = (): AgentDraft | undefined => seen.mock.calls.at(-1)?.[0];

const checkbox = (scope: HTMLElement, name: RegExp) =>
  within(scope)
    .getAllByRole("checkbox")
    .find((c) => name.test(c.closest("label")?.textContent ?? "")) as HTMLElement;

const ADMIN = [
  "instance.system.view",
  "instance.settings.manage",
  "instance.model_sources.manage",
  "instance.updates.manage",
  "instance.users.view",
  "instance.orgs.view",
  "instance.audit_log.view",
];

describe("AgentGrantEditor", () => {
  it("switches presets and marks a changed toggle as custom", async () => {
    const { user } = await renderUi(<Editor />);
    expect(screen.getByText("Works in projects like a developer and manages their models; sees personal keys.")).toBeInTheDocument();
    await user.click(screen.getByRole("radio", { name: "Observer" }));
    expect(latest()?.preset).toBe("observer");
    await user.click(screen.getByRole("button", { name: /^Account/ }));
    await user.click(checkbox(screen.getByRole("group", { name: "Rights on your account" }), /^Edit the profile/));
    expect(latest()?.customized).toBe(true);
    expect(latest()?.account).toEqual(["account.view", "account.update"]);
    expect(screen.getByText("Custom (based on Observer)")).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "Observer" })).not.toBeChecked();
  });

  it("gives instance admins an instance section with presets and single rights", async () => {
    const { user } = await renderUi(<Editor g={grantable({ instance: ADMIN })} />);
    await user.click(screen.getByRole("button", { name: /^Instance/ }));
    await user.click(screen.getByRole("radio", { name: "Read" }));
    expect(latest()?.instance).toEqual(["instance.system.view", "instance.users.view", "instance.orgs.view", "instance.audit_log.view"]);
    expect(screen.getByText(/Also reads the instance\.$/)).toBeInTheDocument();
    await user.click(checkbox(screen.getByRole("group", { name: "Rights on the instance" }), /^Change instance settings/));
    expect(latest()?.instance).toContain("instance.settings.manage");
    expect(screen.getByRole("button", { name: /^Instance/ })).toHaveTextContent("Custom");
    expect(
      within(screen.getByRole("group", { name: "Rights on the instance" }))
        .getByText("Apply updates")
        .closest("label"),
    ).toBeTruthy();
  });

  it("opens and closes organizations, and picks projects one by one", async () => {
    const start = presetDraft(agents, grantable(), "builder");
    const { user } = await renderUi(
      <Editor start={{ ...start, orgs: { ...start.orgs, [ORG]: { included: false, rights: [], all: true, projectRights: [], selected: {} } } }} />,
    );
    expect(screen.getByText("This organization doesn't allow agents.")).toBeInTheDocument();
    await user.click(checkbox(document.body, /^Northwind/));
    expect(latest()?.orgs[ORG]?.included).toBe(true);
    expect(latest()?.orgs[ORG]?.rights).toContain("projects.create");
    await user.click(screen.getByRole("button", { name: /^Rights in Northwind/ }));
    await user.click(checkbox(screen.getByRole("group", { name: "Rights in Northwind" }), /^Create projects/));
    expect(latest()?.orgs[ORG]?.rights).not.toContain("projects.create");
    await user.click(checkbox(screen.getByRole("group", { name: "Rights in every project of Northwind" }), /^Manage limits/));
    expect(latest()?.orgs[ORG]?.projectRights).toContain("limits.manage");
    await user.click(screen.getByRole("radio", { name: "Selected projects" }));
    await user.click(checkbox(document.body, /^Support triage/));
    expect(latest()?.orgs[ORG]?.selected[OTHER_PROJECT]).toEqual(expect.arrayContaining(["project.view", "models.view"]));
    expect(latest()?.orgs[ORG]?.selected[OTHER_PROJECT]).not.toContain("api.request");
    await user.click(checkbox(screen.getByRole("group", { name: "Rights in Support triage" }), /^View limits/));
    expect(latest()?.orgs[ORG]?.selected[OTHER_PROJECT]).not.toContain("limits.view");
    await user.click(checkbox(document.body, /^Checkout/));
    expect(Object.keys(latest()?.orgs[ORG]?.selected ?? {})).toEqual([OTHER_PROJECT, PROJECT]);
    await user.click(checkbox(document.body, /^Support triage/));
    expect(Object.keys(latest()?.orgs[ORG]?.selected ?? {})).toEqual([PROJECT]);
    await user.click(checkbox(document.body, /^Northwind/));
    expect(latest()?.orgs[ORG]?.included).toBe(false);
    expect(screen.getByText("Acts only in organizations you join later.")).toBeInTheDocument();
  });

  it("extends the preset to organizations joined later unless the person turns it off", async () => {
    const start = presetDraft(agents, grantable(), "builder");
    const { user } = await renderUi(<Editor start={{ ...start, orgs: {} }} />);
    const later = screen.getByRole("switch", { name: /Also orgs I join later/ });
    expect(later).toBeChecked();
    expect(latest()?.future.rights).toContain("projects.create");
    await user.click(later);
    expect(latest()?.future.on).toBe(false);
    await user.click(screen.getByRole("radio", { name: "Operator" }));
    expect(latest()?.preset).toBe("operator");
    expect(latest()?.future.on).toBe(false);
    await user.click(screen.getByRole("switch", { name: /Also orgs I join later/ }));
    expect(latest()?.future).toMatchObject({ on: true, rights: expect.arrayContaining(["projects.create"]) });
  });

  it("leaves rights agents can't have and the person lacks switched off", async () => {
    await renderUi(<Editor />);
    const org = screen.getByRole("button", { name: /^Rights in Northwind/ });
    org.click();
    const rights = await screen.findByRole("group", { name: "Rights in Northwind" });
    expect(checkbox(rights, /^Manage SSO/)).toHaveAttribute("aria-disabled", "true");
    expect(checkbox(rights, /^Edit organization/)).not.toHaveAttribute("aria-disabled", "true");
  });
});
