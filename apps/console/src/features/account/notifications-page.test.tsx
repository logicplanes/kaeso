import { describe, expect, it } from "vitest";
import { screen, waitFor } from "@testing-library/react";
import { signedIn } from "~/test/fixtures";
import { notificationSettings, testEmailSent } from "~/test/fixtures/notifications";
import { apiError, on, requests } from "~/test/network";
import { notifications } from "~/test/fixtures/org";
import { renderApp } from "~/test/render";

const PATH = "/settings/account/notifications";

describe("notification settings", () => {
  it("lists the emails by area with when each goes out", async () => {
    signedIn();
    on("GET", "me/notifications", notificationSettings({ deliveries: { key_expiry: "off" } }));
    await renderApp(PATH);
    expect(await screen.findByRole("heading", { name: "Security" })).toBeInTheDocument();
    expect(screen.getAllByRole("heading", { level: 2 }).map((h) => h.textContent)).toEqual(["Security", "Project", "Organization"]);
    expect(screen.getAllByText("Always sent")).toHaveLength(2);
    expect(screen.getByText("When your password, 2FA, address or sessions change.")).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "Budget alerts" })).toHaveTextContent("Now");
    expect(screen.getByRole("combobox", { name: "Your key expires" })).toHaveTextContent("Off");
    expect(screen.getByRole("combobox", { name: "Member changes" })).toHaveTextContent("Daily");
    expect(screen.queryByText("This install doesn't send email")).not.toBeInTheDocument();
  });

  it("saves one email's delivery at a time", async () => {
    signedIn();
    on("GET", "me/notifications", notificationSettings());
    on("PUT", "me/notifications", ({ body }: { body: { kinds: Record<string, "now" | "daily" | "off"> } }) => notificationSettings({ deliveries: body.kinds }));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("combobox", { name: "Member changes" }));
    await user.click(await screen.findByRole("option", { name: "Off" }));
    expect(await notifications().findByText("Notifications saved")).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "Member changes" })).toHaveTextContent("Off");
    expect(requests("PUT", "me/notifications").map((r) => r.body)).toEqual([{ kinds: { org_members: "off" } }]);
  });

  it("shows the choice while it saves and holds the others until it's done", async () => {
    signedIn();
    on("GET", "me/notifications", notificationSettings());
    let release: () => void = () => undefined;
    const held = new Promise<void>((resolve) => (release = resolve));
    on("PUT", "me/notifications", async ({ body }: { body: { kinds: Record<string, "now" | "daily" | "off"> } }) => {
      await held;
      return notificationSettings({ deliveries: body.kinds });
    });
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("combobox", { name: "Budget alerts" }));
    await user.click(await screen.findByRole("option", { name: "Daily" }));
    await waitFor(() => expect(screen.getByRole("combobox", { name: "Member changes" })).toBeDisabled());
    expect(screen.getByRole("combobox", { name: "Budget alerts" })).toHaveTextContent("Daily");
    release();
    expect(await notifications().findByText("Notifications saved")).toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole("combobox", { name: "Member changes" })).toBeEnabled());
  });

  it("sends a test email", async () => {
    signedIn();
    on("GET", "me/notifications", notificationSettings());
    on("POST", "me/notifications/test", testEmailSent);
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Send a test email" }));
    expect(await notifications().findByText("Test email sent")).toBeInTheDocument();
    expect(requests("POST", "me/notifications/test")).toHaveLength(1);
  });

  it("says when the install doesn't send email and offers no choices", async () => {
    signedIn();
    on("GET", "me/notifications", notificationSettings({ sends_email: false }));
    const { user } = await renderApp(PATH);
    expect(await screen.findByText("It writes emails to its log instead; ask an instance admin to set up SMTP.")).toBeInTheDocument();
    for (const name of ["Budget alerts", "Your key expires", "Member changes"]) expect(screen.getByRole("combobox", { name })).toBeDisabled();
    const test = screen.getByRole("button", { name: "Send a test email" });
    expect(test).toHaveAttribute("aria-disabled", "true");
    await user.click(test);
    expect(requests("POST", "me/notifications/test")).toHaveLength(0);
  });

  it("names an email or area a newer gateway adds in its own words", async () => {
    signedIn();
    const settings = notificationSettings();
    settings.areas.push({
      area: "billing",
      name: "Billing",
      kinds: [
        {
          kind: "invoice_ready",
          name: "Invoice ready",
          description: "Your invoice is ready.",
          always: false,
          default: "now",
          delivery: "now",
          reason: "billing.view",
        },
      ],
    });
    on("GET", "me/notifications", settings);
    await renderApp(PATH);
    expect(await screen.findByRole("heading", { name: "Billing" })).toBeInTheDocument();
    expect(screen.getByText("Your invoice is ready.")).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "Invoice ready" })).toHaveTextContent("Now");
  });

  it("says when the notification settings couldn't load", async () => {
    signedIn();
    on("GET", "me/notifications", apiError(429, "rate_limited", "Too many requests; try again in a minute."));
    await renderApp(PATH);
    expect(await screen.findByText("Your notification settings couldn't load")).toBeInTheDocument();
  });

  it("shows why saving was refused", async () => {
    signedIn();
    on("GET", "me/notifications", notificationSettings());
    on("PUT", "me/notifications", apiError(400, "invalid_request", "Security emails are always sent; they have no setting."));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("combobox", { name: "Member changes" }));
    await user.click(await screen.findByRole("option", { name: "Now" }));
    expect((await notifications().findAllByText("Security emails are always sent; they have no setting.")).length).toBeGreaterThan(0);
  });
});
