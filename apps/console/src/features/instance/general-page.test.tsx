import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import type { UserEvent } from "@testing-library/user-event";
import type { SettingsView } from "~/api/generated/types";
import { signedIn } from "~/test/fixtures";
import { instanceSettings, system } from "~/test/fixtures/instance-account";
import { hoverReason, notifications } from "~/test/fixtures/org";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const PATH = "/settings/instance/general";
const LOGGING = "/settings/instance/logging";
const SYSTEM = "/settings/instance/system";

function settingsApi(initial: Partial<SettingsView> = {}) {
  let current = instanceSettings(initial);
  on("GET", "instance/system", system());
  on("GET", "instance/settings", () => current);
  on("PATCH", "instance/settings", ({ body }: { body: Partial<SettingsView> }) => {
    current = { ...current, ...body };
    return current;
  });
}

const patches = () => requests("PATCH", "instance/settings").map((r) => r.body);

async function choose(user: UserEvent, select: string, option: string) {
  await user.click(screen.getByRole("combobox", { name: select }));
  await user.click(await screen.findByRole("option", { name: new RegExp(`^${option}`) }));
}

async function setNumber(user: UserEvent, name: string, value: string) {
  const input = screen.getByRole("textbox", { name });
  await user.clear(input);
  if (value) await user.type(input, value);
}

describe("instance general page", () => {
  it("shows who can join, how people sign in and deleting organizations", async () => {
    signedIn();
    settingsApi();
    await renderApp(PATH);
    expect(await screen.findByRole("heading", { name: "General", level: 1 })).toBeInTheDocument();
    expect(await screen.findByRole("combobox", { name: "Account creation" })).toHaveTextContent("Invitation only");
    expect(screen.getByRole("combobox", { name: "Organization creation" })).toHaveTextContent("Instance admins only");
    expect(screen.queryByRole("button", { name: "Edit domains" })).not.toBeInTheDocument();
    expect(screen.getByRole("switch", { name: "Organizations can be deleted" })).toBeChecked();
    expect(screen.getByRole("textbox", { name: "Grace period" })).toHaveValue("7");
    expect(screen.getAllByText("On")).toHaveLength(2);
    expect(screen.getByText("Off")).toBeInTheDocument();
    expect(screen.queryByRole("textbox", { name: "Default metadata retention" })).not.toBeInTheDocument();
    expect(screen.queryByText("smtp")).not.toBeInTheDocument();
  });

  it("shows the API's message when the settings couldn't load", async () => {
    signedIn();
    on("GET", "instance/settings", apiError(403, "permission_denied", "Only instance admins can read the settings."));
    await renderApp(PATH);
    expect((await notifications().findAllByText("Only instance admins can read the settings.")).length).toBeGreaterThan(0);
    expect(await screen.findByText("The instance settings couldn't load")).toBeInTheDocument();
    expect(screen.queryByRole("combobox", { name: "Account creation" })).not.toBeInTheDocument();
  });
});

describe("deleting organizations", () => {
  it("turns deleting organizations off and on", async () => {
    signedIn();
    settingsApi();
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("switch", { name: "Organizations can be deleted" }));
    expect(await notifications().findByText("Organizations can't be deleted")).toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole("switch", { name: "Organizations can be deleted" })).not.toBeChecked());
    await user.click(screen.getByRole("switch", { name: "Organizations can be deleted" }));
    expect(await notifications().findByText("Organizations can be deleted")).toBeInTheDocument();
    expect(patches()).toEqual([{ org_deletion: false }, { org_deletion: true }]);
  });

  it("marks only the setting that is saving as busy", async () => {
    signedIn();
    on("GET", "instance/settings", instanceSettings());
    const waiting: (() => void)[] = [];
    on(
      "PATCH",
      "instance/settings",
      ({ body }: { body: Partial<SettingsView> }) => new Promise((resolve) => waiting.push(() => resolve(instanceSettings(body)))),
    );
    const { user } = await renderApp(PATH);
    const toggle = () => screen.getByRole("switch", { name: "Organizations can be deleted" });
    const save = () => screen.getByRole("button", { name: "Save" });
    await user.click(await screen.findByRole("switch", { name: "Organizations can be deleted" }));
    await waitFor(() => expect(toggle()).toHaveAttribute("aria-busy", "true"));
    expect(save()).not.toHaveAttribute("aria-busy");
    waiting.shift()?.();
    expect(await notifications().findByText("Organizations can't be deleted")).toBeInTheDocument();
    await waitFor(() => expect(toggle()).not.toHaveAttribute("aria-busy"));
    await setNumber(user, "Grace period", "14");
    await user.click(save());
    await waitFor(() => expect(save()).toHaveAttribute("aria-busy", "true"));
    expect(toggle()).not.toHaveAttribute("aria-busy");
    waiting.shift()?.();
    expect(await notifications().findByText("Deletions wait 14 days")).toBeInTheDocument();
  });

  it("saves a grace period between 1 and 90 days", async () => {
    signedIn();
    settingsApi();
    const { user } = await renderApp(PATH);
    await screen.findByRole("textbox", { name: "Grace period" });
    const save = () => screen.getByRole("button", { name: "Save" });
    await hoverReason(user, save(), "No change to save");
    await setNumber(user, "Grace period", "91");
    await hoverReason(user, save(), "Enter 1 to 90 days");
    await setNumber(user, "Grace period", "14");
    await user.click(save());
    expect(await notifications().findByText("Deletions wait 14 days")).toBeInTheDocument();
    expect(patches()).toEqual([{ org_deletion_grace_days: 14 }]);
  });
});

describe("request log retention", () => {
  it("says when the settings couldn't load", async () => {
    signedIn();
    on("GET", "instance/settings", apiError(429, "rate_limited", "Too many requests; try again in a minute."));
    await renderApp(LOGGING);
    expect(await screen.findByText("The instance settings couldn't load")).toBeInTheDocument();
  });

  it("has a page of its own", async () => {
    signedIn();
    settingsApi();
    await renderApp(LOGGING);
    expect(await screen.findByRole("heading", { name: "Logging", level: 1 })).toBeInTheDocument();
    expect(await screen.findByRole("textbox", { name: "Default metadata retention" })).toHaveValue("90");
    expect(screen.getByRole("textbox", { name: "Maximum content retention" })).toHaveValue("90");
  });

  it("explains what keeps the retention from being saved", async () => {
    signedIn();
    settingsApi();
    const { user } = await renderApp(LOGGING);
    await screen.findByRole("textbox", { name: "Default metadata retention" });
    const save = () => screen.getByRole("button", { name: "Save retention" });
    await hoverReason(user, save(), "No change to save");
    await setNumber(user, "Default content retention", "");
    await hoverReason(user, save(), "Enter whole days, at least 1");
    await setNumber(user, "Default content retention", "30");
    await setNumber(user, "Default metadata retention", "400");
    await hoverReason(user, save(), "The default metadata retention is above its maximum");
    await setNumber(user, "Default metadata retention", "90");
    await setNumber(user, "Default content retention", "120");
    await hoverReason(user, save(), "The default content retention is above its maximum");
    await setNumber(user, "Default content retention", "60");
    await setNumber(user, "Default metadata retention", "45");
    await hoverReason(user, save(), "Content can't be kept longer than metadata");
    expect(patches()).toEqual([]);
  });

  it("saves the defaults and maximums together", async () => {
    signedIn();
    settingsApi();
    const { user } = await renderApp(LOGGING);
    await screen.findByRole("textbox", { name: "Default metadata retention" });
    await setNumber(user, "Default metadata retention", "60");
    await setNumber(user, "Maximum metadata retention", "180");
    await user.click(screen.getByRole("button", { name: "Save retention" }));
    expect(await notifications().findByText("Retention saved")).toBeInTheDocument();
    expect(patches()).toEqual([{ metadata_retention_days: 60, content_retention_days: 30, max_metadata_retention_days: 180, max_content_retention_days: 90 }]);
  });
});

describe("the test email", () => {
  it("shows how the install sends mail on the system page", async () => {
    signedIn();
    settingsApi();
    await renderApp(SYSTEM);
    expect(await screen.findByText("smtp")).toBeInTheDocument();
    expect(screen.getByText("mail.example.com:587")).toBeInTheDocument();
    expect(screen.getByText("kaeso@example.com")).toBeInTheDocument();
  });

  it("leaves out the host for mail that doesn't use one", async () => {
    signedIn();
    settingsApi({ mail: { transport: "log", host: null, port: null, from: "kaeso@example.com" } as never });
    await renderApp(SYSTEM);
    expect(await screen.findByText("log")).toBeInTheDocument();
    expect(screen.queryByText("Host")).not.toBeInTheDocument();
  });

  it("sends one to yourself when no recipient is given", async () => {
    signedIn();
    settingsApi();
    on("POST", "instance/settings/test-email", { to: "owner@example.com" });
    const { user } = await renderApp(SYSTEM);
    await user.click(await screen.findByRole("button", { name: "Send test email" }));
    const dialog = await screen.findByRole("dialog", { name: "Send a test email" });
    await user.click(within(dialog).getByRole("button", { name: "Send test email" }));
    expect(await notifications().findByText("Test email sent to owner@example.com")).toBeInTheDocument();
    expect(requests("POST", "instance/settings/test-email").map((r) => r.body)).toEqual([{}]);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Send a test email" })).not.toBeInTheDocument());
  });

  it("sends one to a valid recipient from the keyboard", async () => {
    signedIn();
    settingsApi();
    on("POST", "instance/settings/test-email", ({ body }: { body: { to: string } }) => ({ to: body.to }));
    const { user } = await renderApp(SYSTEM);
    await user.click(await screen.findByRole("button", { name: "Send test email" }));
    const dialog = await screen.findByRole("dialog", { name: "Send a test email" });
    const input = within(dialog).getByRole("textbox", { name: /Recipient/ });
    await user.type(input, "ops");
    await hoverReason(user, within(dialog).getByRole("button", { name: "Send test email" }), "Enter an email address, or leave it empty");
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    expect(requests("POST", "instance/settings/test-email")).toHaveLength(0);
    await user.type(input, "@example.com ");
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    expect(await notifications().findByText("Test email sent to ops@example.com")).toBeInTheDocument();
    expect(requests("POST", "instance/settings/test-email").map((r) => r.body)).toEqual([{ to: "ops@example.com" }]);
  });

  it("opens with an empty recipient after one was typed and cancelled", async () => {
    signedIn();
    settingsApi();
    const { user } = await renderApp(SYSTEM);
    await user.click(await screen.findByRole("button", { name: "Send test email" }));
    const dialog = await screen.findByRole("dialog", { name: "Send a test email" });
    await user.type(within(dialog).getByRole("textbox", { name: /Recipient/ }), "ops");
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Send a test email" })).not.toBeInTheDocument());
    await user.click(screen.getByRole("button", { name: "Send test email" }));
    const again = await screen.findByRole("dialog", { name: "Send a test email" });
    expect(within(again).getByRole("textbox", { name: /Recipient/ })).toHaveValue("");
    expect(within(again).getByRole("button", { name: "Send test email" })).not.toHaveAttribute("aria-disabled", "true");
  });

  it("keeps the dialog open and shows why sending failed", async () => {
    signedIn();
    settingsApi();
    on("POST", "instance/settings/test-email", apiError(502, "mail_failed", "The mail server refused the connection."));
    const { user } = await renderApp(SYSTEM);
    await user.click(await screen.findByRole("button", { name: "Send test email" }));
    const dialog = await screen.findByRole("dialog", { name: "Send a test email" });
    await user.click(within(dialog).getByRole("button", { name: "Send test email" }));
    expect((await notifications().findAllByText("The mail server refused the connection.")).length).toBeGreaterThan(0);
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Send a test email" })).not.toBeInTheDocument());
  });
});

describe("accounts", () => {
  it("lets anyone create an account", async () => {
    signedIn();
    settingsApi();
    const { user } = await renderApp(PATH);
    await screen.findByRole("combobox", { name: "Account creation" });
    await choose(user, "Account creation", "Anyone");
    expect(await notifications().findByText("Anyone can create an account")).toBeInTheDocument();
    expect(patches()).toEqual([{ account_creation: "everyone" }]);
  });

  it("asks for domains before allowing sign-up by domain", async () => {
    signedIn();
    settingsApi();
    const { user } = await renderApp(PATH);
    await screen.findByRole("combobox", { name: "Account creation" });
    await choose(user, "Account creation", "Allowed email domains");
    const dialog = await screen.findByRole("dialog", { name: "Allow email domains" });
    await hoverReason(user, within(dialog).getByRole("button", { name: "Allow domains" }), "Add at least one domain");
    await user.type(within(dialog).getByRole("textbox", { name: "Allowed domains" }), "example.org,");
    await user.click(within(dialog).getByRole("button", { name: "Allow domains" }));
    expect(await notifications().findByText("Allowed email domains can sign up")).toBeInTheDocument();
    expect(patches()).toEqual([{ account_creation: "allowed_domains", allowed_domains: ["example.org"] }]);
    expect(await screen.findByText("example.org")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Edit domains" })).toBeInTheDocument();
  });

  it("changes nothing when the domains dialog is cancelled", async () => {
    signedIn();
    settingsApi();
    const { user } = await renderApp(PATH);
    await screen.findByRole("combobox", { name: "Organization creation" });
    await choose(user, "Organization creation", "Users on allowed domains");
    const dialog = await screen.findByRole("dialog", { name: "Allow email domains" });
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Allow email domains" })).not.toBeInTheDocument());
    await choose(user, "Account creation", "Allowed email domains");
    await screen.findByRole("dialog", { name: "Allow email domains" });
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Allow email domains" })).not.toBeInTheDocument());
    expect(patches()).toEqual([]);
  });

  it("refuses entries that aren't domains", async () => {
    signedIn();
    settingsApi();
    const { user } = await renderApp(PATH);
    await screen.findByRole("combobox", { name: "Account creation" });
    await choose(user, "Account creation", "Allowed email domains");
    const dialog = await screen.findByRole("dialog", { name: "Allow email domains" });
    await user.type(within(dialog).getByRole("textbox", { name: "Allowed domains" }), "localhost,");
    expect(await within(dialog).findByText("Not a domain, like example.com or *.example.com")).toBeInTheDocument();
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    expect(patches()).toEqual([]);
  });

  it("edits the allowed domains", async () => {
    signedIn();
    settingsApi({ org_creation: "allowed_domains", allowed_domains: ["example.com", "*.example.net"] });
    const { user } = await renderApp(PATH);
    expect(await screen.findByText("example.com, *.example.net")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Edit domains" }));
    const dialog = await screen.findByRole("dialog", { name: "Allowed domains" });
    await hoverReason(user, within(dialog).getByRole("button", { name: "Save domains" }), "No change to save");
    await user.type(within(dialog).getByRole("textbox", { name: "Allowed domains" }), "@example.org ");
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    expect(await notifications().findByText("Allowed domains saved")).toBeInTheDocument();
    expect(patches()).toEqual([{ allowed_domains: ["example.com", "*.example.net", "@example.org"] }]);
  });

  it("sums up long domain lists", async () => {
    signedIn();
    settingsApi({ account_creation: "allowed_domains", allowed_domains: ["alpha.example.com", "beta.example.com", "gamma.example.com", "delta.example.com"] });
    await renderApp(PATH);
    expect(await screen.findByText("alpha.example.com, beta.example.com and 2 more")).toBeInTheDocument();
  });

  it("counts domains whose names are too long to list", async () => {
    signedIn();
    settingsApi({ account_creation: "allowed_domains", allowed_domains: ["a-very-long-subdomain-name.department.example.com", "example.org"] });
    await renderApp(PATH);
    expect(await screen.findByText("2 domains")).toBeInTheDocument();
  });

  it("changes who can create organizations", async () => {
    signedIn();
    settingsApi({ allowed_domains: ["example.com"] });
    const { user } = await renderApp(PATH);
    await screen.findByRole("combobox", { name: "Organization creation" });
    await choose(user, "Organization creation", "Users on allowed domains");
    expect(await notifications().findByText("Allowed email domains can create organizations")).toBeInTheDocument();
    await choose(user, "Organization creation", "Any signed-in user");
    expect(await notifications().findByText("Any signed-in user can create organizations")).toBeInTheDocument();
    await choose(user, "Organization creation", "Instance admins only");
    expect(await notifications().findByText("Only instance admins can create organizations")).toBeInTheDocument();
    await choose(user, "Account creation", "Invitation only");
    await choose(user, "Account creation", "Allowed email domains");
    expect(await notifications().findByText("Allowed email domains can sign up")).toBeInTheDocument();
    await choose(user, "Account creation", "Invitation only");
    expect(await notifications().findByText("Accounts are by invitation only")).toBeInTheDocument();
    expect(patches()).toEqual([
      { org_creation: "allowed_domains" },
      { org_creation: "everyone" },
      { org_creation: "instance_admins" },
      { account_creation: "allowed_domains" },
      { account_creation: "invitation_only" },
    ]);
  });

  it("shows the API's message when a change is refused", async () => {
    signedIn();
    settingsApi();
    on("PATCH", "instance/settings", apiError(422, "invalid_request", "Sign-up needs a mail transport.", { param: "account_creation" }));
    const { user } = await renderApp(PATH);
    await screen.findByRole("combobox", { name: "Account creation" });
    await choose(user, "Account creation", "Anyone");
    expect((await notifications().findAllByText("Sign-up needs a mail transport.")).length).toBeGreaterThan(0);
  });
});
