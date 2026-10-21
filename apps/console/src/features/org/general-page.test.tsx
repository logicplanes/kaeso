import { describe, expect, it, vi } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ALL_ORG_RIGHTS, ORG, membership, org, signedIn, whoAmI, withRights } from "~/test/fixtures";
import { OTHER_USER, THIRD_USER, hoverReason, member, notifications, roleGrant } from "~/test/fixtures/org";
import { apiError, on, page, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import { atFixtureTime } from "~/test/clock";

atFixtureTime();

const PATH = `/settings/orgs/${ORG}/general`;

describe("organization general settings", () => {
  it("shows the organization's picture, name, id and creation date", async () => {
    signedIn();
    await renderApp(PATH);
    expect(await screen.findByRole("heading", { name: "General", level: 1 })).toBeInTheDocument();
    expect(await screen.findByRole("textbox", { name: "Organization name" })).toHaveValue("Northwind Labs");
    expect(screen.getByText("Organization id")).toBeInTheDocument();
    expect(screen.getByText(ORG)).toBeInTheDocument();
    expect(screen.getByText("15 Oct 2026")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Upload" })).toBeInTheDocument();
  });

  it("renames the organization with the trimmed name", async () => {
    signedIn();
    on("PATCH", "orgs/:org", ({ body }: { body: { name: string } }) => org({ name: body.name }));
    const { user } = await renderApp(PATH);
    const input = await screen.findByRole("textbox", { name: "Organization name" });
    const save = screen.getByRole("button", { name: "Save" });
    await hoverReason(user, save, "No change to save");
    await user.clear(input);
    await hoverReason(user, save, "Enter a name");
    await user.type(input, "  Contoso  ");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(await notifications().findByText("Name saved")).toBeInTheDocument();
    expect(requests("PATCH", "orgs/:org").map((r) => r.body)).toEqual([{ name: "Contoso" }]);
  });

  it("saves the name when Enter is pressed", async () => {
    signedIn();
    on("PATCH", "orgs/:org", org({ name: "Contoso" }));
    const { user } = await renderApp(PATH);
    const input = await screen.findByRole("textbox", { name: "Organization name" });
    await user.type(input, "{Enter}");
    expect(requests("PATCH", "orgs/:org")).toHaveLength(0);
    await user.clear(input);
    await user.type(input, "Contoso{Enter}");
    await waitFor(() => expect(requests("PATCH", "orgs/:org").map((r) => r.body)).toEqual([{ name: "Contoso" }]));
  });

  it("sends the name once while the rename is on its way", async () => {
    signedIn();
    let answer: (view: unknown) => void = () => {};
    on("PATCH", "orgs/:org", () => new Promise((resolve) => (answer = resolve)));
    const { user } = await renderApp(PATH);
    const input = await screen.findByRole("textbox", { name: "Organization name" });
    await user.clear(input);
    await user.type(input, "Contoso{Enter}{Enter}");
    answer(org({ name: "Contoso" }));
    expect(await notifications().findByText("Name saved")).toBeInTheDocument();
    expect(requests("PATCH", "orgs/:org")).toHaveLength(1);
  });

  it("shows the API's message when the rename is refused", async () => {
    signedIn();
    on("PATCH", "orgs/:org", apiError(422, "invalid_request", "That name is taken.", { param: "name" }));
    const { user } = await renderApp(PATH);
    const input = await screen.findByRole("textbox", { name: "Organization name" });
    await user.clear(input);
    await user.type(input, "Contoso");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(await notifications().findByText("That name is taken.")).toBeInTheDocument();
  });

  it("locks the name and picture for people without Edit organization", async () => {
    withRights(["org.view"], { role: "admin" });
    const { user } = await renderApp(PATH);
    const input = await screen.findByRole("textbox", { name: "Organization name" });
    expect(input).toBeDisabled();
    expect(screen.getByRole("button", { name: "Upload" })).toHaveAttribute("aria-disabled", "true");
    await hoverReason(user, screen.getByRole("button", { name: "Upload" }), "You need Edit organization; ask an owner or admin for it.");
    expect(requests("GET", "orgs/:org/rights/:right/grantors")).toHaveLength(0);
    expect(screen.queryByRole("heading", { name: "Ownership" })).not.toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "Delete organization" })).not.toBeInTheDocument();
  });

  it("hides the page from people who can't view the organization", async () => {
    withRights([], { role: "admin" });
    await renderApp(PATH);
    expect(await screen.findByText("You can't see the general settings here")).toBeInTheDocument();
    expect(screen.getByText("You need View organization; ask an owner or admin for it.")).toBeInTheDocument();
    expect(screen.queryByRole("textbox", { name: "Organization name" })).not.toBeInTheDocument();
  });

  it("goes home from the settings of an organization the person doesn't belong to", async () => {
    signedIn();
    await renderApp("/settings/orgs/org_contoso/general");
    expect(await notifications().findByText("That organization doesn't exist, or you don't have access to it")).toBeInTheDocument();
    await waitFor(() => expect(window.location.pathname).not.toContain("org_contoso"));
    expect(screen.queryByRole("heading", { name: "General", level: 1 })).not.toBeInTheDocument();
  });

  it("says when the organization couldn't load", async () => {
    signedIn();
    on("GET", "orgs/:org", apiError(403, "forbidden", "The gateway failed."));
    await renderApp(PATH);
    expect(await screen.findByText("The organization couldn't load")).toBeInTheDocument();
    expect(screen.getByText("The notification says why; try again from there.")).toBeInTheDocument();
  });

  it("rejects pictures that aren't images and removes the current one", async () => {
    signedIn();
    on("GET", "orgs/:org", org({ avatar_url: "/avatars/org.webp" }));
    on("DELETE", "orgs/:org/avatar", org());
    const { user, container } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Remove" }));
    expect(await notifications().findByText("Picture removed")).toBeInTheDocument();
    expect(requests("DELETE", "orgs/:org/avatar")).toHaveLength(1);
    const picker = container.ownerDocument.querySelector<HTMLInputElement>('input[type="file"]');
    expect(picker).not.toBeNull();
    await userEvent.setup({ applyAccept: false }).upload(picker as HTMLInputElement, new File(["%PDF"], "logo.pdf", { type: "application/pdf" }));
    expect(await notifications().findByText("Choose a PNG, JPEG or WebP image.")).toBeInTheDocument();
  });

  it("uploads a new picture as a square image", async () => {
    signedIn();
    on("PUT", "orgs/:org/avatar", org({ avatar_url: "/avatars/org.webp" }));
    const close = vi.fn();
    vi.stubGlobal(
      "createImageBitmap",
      vi.fn(async () => ({ width: 400, height: 300, close })),
    );
    const draw = vi.fn();
    const context = vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue({ drawImage: draw, imageSmoothingQuality: "low" } as never);
    const blob = vi
      .spyOn(HTMLCanvasElement.prototype, "toBlob")
      .mockImplementation((done: BlobCallback) => done(new Blob([new Uint8Array([1, 2, 3])], { type: "image/webp" })));
    try {
      const { user, container } = await renderApp(PATH);
      await screen.findByRole("button", { name: "Upload" });
      const picker = container.ownerDocument.querySelector<HTMLInputElement>('input[type="file"]') as HTMLInputElement;
      await user.upload(picker, new File(["png"], "logo.png", { type: "image/png" }));
      expect(await notifications().findByText("Picture saved")).toBeInTheDocument();
      expect(requests("PUT", "orgs/:org/avatar").map((r) => r.body)).toEqual([{ content_type: "image/webp", data: btoa("\u0001\u0002\u0003") }]);
      expect(draw).toHaveBeenCalledWith(expect.anything(), 50, 0, 300, 300, 0, 0, 256, 256);
    } finally {
      context.mockRestore();
      blob.mockRestore();
      vi.unstubAllGlobals();
    }
  });
});

describe("transferring ownership", () => {
  const members = [
    member(),
    member({ user: { id: OTHER_USER, name: "Grace Hopper", email: "grace@example.com" }, grant: roleGrant("admin", "Admin") }),
    member({ user: { id: THIRD_USER, name: "Linus Park", email: "linus@example.com", state: "suspended" }, grant: roleGrant("member", "Member") }),
  ];

  it("makes an active member an owner in your place", async () => {
    signedIn();
    on("GET", "orgs/:org/members", page(members));
    on("POST", "orgs/:org/ownership-transfer", members[1]);
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Transfer ownership" }));
    const dialog = await screen.findByRole("dialog", { name: "Transfer ownership" });
    expect(within(dialog).getByText("Choose the member who becomes an owner; you become an admin.")).toBeInTheDocument();
    await hoverReason(user, within(dialog).getByRole("button", { name: "Transfer ownership" }), "Choose a member");
    await user.click(within(dialog).getByRole("combobox", { name: "New owner" }));
    expect(screen.queryByRole("option", { name: /Linus Park/ })).not.toBeInTheDocument();
    expect(screen.queryByRole("option", { name: /Ada Okafor/ })).not.toBeInTheDocument();
    await user.click(await screen.findByRole("option", { name: /Grace Hopper/ }));
    expect(await within(dialog).findByText("Grace Hopper becomes an owner and you become an admin.")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Transfer ownership" }));
    expect(await notifications().findByText("Grace Hopper is an owner now; you're an admin")).toBeInTheDocument();
    expect(requests("POST", "orgs/:org/ownership-transfer").map((r) => r.body)).toEqual([{ user_id: OTHER_USER }]);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Transfer ownership" })).not.toBeInTheDocument());
  });

  it("stays open while the transfer is on its way, so it can't be sent twice", async () => {
    signedIn();
    on("GET", "orgs/:org/members", page(members));
    let respond: (body: unknown) => void = () => undefined;
    on("POST", "orgs/:org/ownership-transfer", () => new Promise((resolve) => (respond = resolve)));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Transfer ownership" }));
    const dialog = await screen.findByRole("dialog", { name: "Transfer ownership" });
    await user.click(within(dialog).getByRole("combobox", { name: "New owner" }));
    await user.click(await screen.findByRole("option", { name: /Grace Hopper/ }));
    await user.click(within(dialog).getByRole("button", { name: "Transfer ownership" }));
    await waitFor(() => expect(requests("POST", "orgs/:org/ownership-transfer")).toHaveLength(1));
    await user.keyboard("{Escape}");
    expect(screen.getByRole("dialog", { name: "Transfer ownership" })).toBe(dialog);
    expect(within(dialog).getByRole("button", { name: "Close" })).toBeDisabled();
    respond(members[1]);
    expect(await notifications().findByText("Grace Hopper is an owner now; you're an admin")).toBeInTheDocument();
    expect(requests("POST", "orgs/:org/ownership-transfer")).toHaveLength(1);
  });

  it("transfers with the keyboard only once a member is chosen", async () => {
    const hedy = member({
      user: { id: "usr_hedy", name: "Hedy Lamarr", email: null, avatar_url: "/avatars/hedy.webp" },
      grant: roleGrant("member", "Member"),
    });
    signedIn();
    on("GET", "orgs/:org/members", page([...members, hedy]));
    on("POST", "orgs/:org/ownership-transfer", hedy);
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Transfer ownership" }));
    const dialog = await screen.findByRole("dialog", { name: "Transfer ownership" });
    await user.click(within(dialog).getByRole("combobox", { name: "New owner" }));
    await user.keyboard("{Control>}{Enter}{/Control}");
    expect(requests("POST", "orgs/:org/ownership-transfer")).toHaveLength(0);
    const option = await screen.findByRole("option", { name: /Hedy Lamarr/ });
    expect(option.querySelector('img[src="/avatars/hedy.webp"]')).not.toBeNull();
    await user.click(option);
    expect(await within(dialog).findByText("Hedy Lamarr becomes an owner and you become an admin.")).toBeInTheDocument();
    await user.keyboard("{Control>}{Enter}{/Control}");
    expect(await notifications().findByText("Hedy Lamarr is an owner now; you're an admin")).toBeInTheDocument();
    expect(requests("POST", "orgs/:org/ownership-transfer").map((r) => r.body)).toEqual([{ user_id: "usr_hedy" }]);
  });

  it("says the members are loading until they arrive", async () => {
    signedIn();
    let release: () => void = () => undefined;
    on("GET", "orgs/:org/members", () => new Promise((resolve) => (release = () => resolve(page(members)))));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Transfer ownership" }));
    const dialog = await screen.findByRole("dialog", { name: "Transfer ownership" });
    expect(within(dialog).getByRole("combobox", { name: "New owner" })).toHaveAttribute("placeholder", "Loading members…");
    release();
    await waitFor(() => expect(within(dialog).getByRole("combobox", { name: "New owner" })).toHaveAttribute("placeholder", "Choose a member"));
  });

  it("forgets the chosen member when cancelled, so it opens with no one chosen", async () => {
    signedIn();
    on("GET", "orgs/:org/members", page(members));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Transfer ownership" }));
    const dialog = await screen.findByRole("dialog", { name: "Transfer ownership" });
    await user.click(within(dialog).getByRole("combobox", { name: "New owner" }));
    await user.click(await screen.findByRole("option", { name: /Grace Hopper/ }));
    expect(await within(dialog).findByText("Grace Hopper becomes an owner and you become an admin.")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Transfer ownership" })).not.toBeInTheDocument());

    await user.click(screen.getByRole("button", { name: "Transfer ownership" }));
    const again = await screen.findByRole("dialog", { name: "Transfer ownership" });
    expect(within(again).getByText("Choose the member who becomes an owner; you become an admin.")).toBeInTheDocument();
    expect(within(again).getByRole("combobox", { name: "New owner" })).toHaveValue("");
    expect(within(again).getByRole("button", { name: "Transfer ownership" })).toHaveAttribute("aria-disabled", "true");
  });

  it("closes without transferring when cancelled", async () => {
    signedIn();
    on("GET", "orgs/:org/members", page(members));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Transfer ownership" }));
    const dialog = await screen.findByRole("dialog", { name: "Transfer ownership" });
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Transfer ownership" })).not.toBeInTheDocument());
    expect(requests("POST", "orgs/:org/ownership-transfer")).toHaveLength(0);
  });
});

describe("deleting the organization", () => {
  it("schedules the deletion once the name is typed", async () => {
    signedIn();
    on("DELETE", "orgs/:org", org({ state: "scheduled_for_deletion" }));
    const { user } = await renderApp(PATH);
    expect(await screen.findByText("Keys stop at once; it stays restorable for 7 days.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Delete organization" }));
    const dialog = await screen.findByRole("dialog", { name: "Delete Northwind Labs" });
    expect(within(dialog).getByText("Every API key of the organization stops working.")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    const typed = within(dialog).getByRole("textbox", { name: "Organization name" });
    await user.type(typed, "Northwind");
    const blocked = within(dialog).getByRole("button", { name: "Delete organization" });
    await hoverReason(user, blocked, "Type Northwind Labs to confirm");
    await user.click(blocked);
    expect(requests("DELETE", "orgs/:org")).toHaveLength(0);
    await user.type(typed, " Labs");
    await user.click(within(dialog).getByRole("button", { name: "Delete organization" }));
    expect(await notifications().findByText("Northwind Labs is scheduled for deletion")).toBeInTheDocument();
    expect(requests("DELETE", "orgs/:org")).toHaveLength(1);
  });

  it("closes the dialog and says why when the deletion is refused", async () => {
    signedIn();
    on("DELETE", "orgs/:org", apiError(409, "conflict", "Billing is still open."));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Delete organization" }));
    const dialog = await screen.findByRole("dialog", { name: "Delete Northwind Labs" });
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.type(within(dialog).getByRole("textbox", { name: "Organization name" }), "Northwind Labs{Enter}");
    expect(await notifications().findByText("Billing is still open.")).toBeInTheDocument();
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Delete Northwind Labs" })).not.toBeInTheDocument());
    await user.click(screen.getByRole("button", { name: "Delete organization" }));
    const again = await screen.findByRole("dialog", { name: "Delete Northwind Labs" });
    await user.click(within(again).getByRole("button", { name: "Continue" }));
    expect(within(again).getByRole("textbox", { name: "Organization name" })).toHaveValue("");
  });

  it("forgets what was typed when the dialog is closed", async () => {
    signedIn();
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Delete organization" }));
    const dialog = await screen.findByRole("dialog", { name: "Delete Northwind Labs" });
    await user.click(within(dialog).getByRole("button", { name: "Close" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Delete Northwind Labs" })).not.toBeInTheDocument());
    expect(requests("DELETE", "orgs/:org")).toHaveLength(0);
  });

  it("isn't offered when the install doesn't allow deleting organizations", async () => {
    withRights(ALL_ORG_RIGHTS, { role: "admin", session: { org_deletion: { allowed: false, grace_days: 7 } } });
    await renderApp(PATH);
    expect(await screen.findByRole("textbox", { name: "Organization name" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Delete organization" })).not.toBeInTheDocument();
  });

  it("shows the deletion banner instead when the organization is already scheduled", async () => {
    signedIn({ session: whoAmI({ memberships: [membership({ org_state: "scheduled_for_deletion", org_deletion_due_at: "2026-10-22T12:00:00Z" })] }) });
    on("GET", "orgs/:org", org({ state: "scheduled_for_deletion" }));
    await renderApp(PATH);
    expect(await screen.findByText("This organization is scheduled for deletion")).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Organization name" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Delete organization" })).not.toBeInTheDocument();
  });
});

describe("the organization settings entry", () => {
  it("opens the general page", async () => {
    signedIn();
    await renderApp(`/settings/orgs/${ORG}`);
    expect(await screen.findByRole("heading", { name: "General", level: 1 })).toBeInTheDocument();
    expect(window.location.pathname).toBe(PATH);
  });
});

describe("the reason a locked change names", () => {
  it("names who can give the right when the person can see the members", async () => {
    withRights(["org.view", "members.view"], { role: "admin" });
    on("GET", "orgs/:org/members", page([]));
    on("GET", "orgs/:org/rights/:right/grantors", { data: [{ id: "usr_owner", name: "Grace Hopper", email: null, state: "active" }] });
    const { user } = await renderApp(PATH);
    const upload = await screen.findByRole("button", { name: "Upload" });
    await waitFor(() => expect(requests("GET", "orgs/:org/rights/:right/grantors")).toHaveLength(1));
    expect(upload).toHaveAttribute("aria-disabled", "true");
    await hoverReason(user, upload, "You need Edit organization; ask Grace Hopper for it.");
  });
});
