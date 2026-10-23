import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { SettingsGroup, SettingsRow, SettingsSection } from "./settings.js";
import { Switch } from "../switch/switch.js";
import { Button } from "../button/button.js";
import { icons } from "../../icons/vocabulary.js";

describe("Settings", () => {
  it("lays out a section with rows that link, act, show a value or hold a control", async () => {
    const onClick = vi.fn();
    render(
      <SettingsSection title="Security" description="How people sign in." actions={<button>Audit log</button>}>
        <SettingsGroup aria-label="Sign-in">
          <SettingsRow title="Single sign-on" description="Okta" href="/settings/sso" icon={icons.key} data-force="hover" />
          <SettingsRow title="Sessions" value="3 active" onClick={onClick} data-force="focus" />
          <SettingsRow title="Require two-factor" control={<Switch aria-label="Require two-factor" />} id="2fa" />
          <SettingsRow title="Delete organization" danger data-force="hover" />
        </SettingsGroup>
      </SettingsSection>,
    );
    expect(screen.getByRole("heading", { name: "Security" })).toBeInTheDocument();
    expect(screen.getByText("How people sign in.")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: /Single sign-on/ })).toHaveAttribute("href", "/settings/sso");
    await userEvent.click(screen.getByRole("button", { name: /Sessions/ }));
    expect(onClick).toHaveBeenCalled();
    expect(screen.getByText("Require two-factor", { selector: "span" })).toHaveAttribute("id", "2fa-title");
    expect(screen.getByText("Delete organization")).toHaveClass("text-danger-text");
    expect(screen.getByRole("group", { name: "Sign-in" })).toBeInTheDocument();
  });

  it("moves every row's control under its text on a narrow row, and marks the control so a ghost button keeps to the row's edge", () => {
    render(
      <SettingsGroup aria-label="Password">
        <SettingsRow title="Password" description="Changing it signs out your other sessions." control={<Button size="sm">Change password</Button>} />
        <SettingsRow
          title="Chrome on macOS"
          icon={icons.key}
          control={
            <Button size="sm" variant="ghost">
              End session
            </Button>
          }
        />
        <SettingsRow title="Sessions" value="3 active" href="/settings/account/sessions" />
      </SettingsGroup>,
    );
    const rows = [...screen.getByRole("group", { name: "Password" }).children];
    for (const row of rows) expect(row).toHaveClass("@container");
    const change = screen.getByRole("button", { name: "Change password" }).parentElement!;
    expect(change).toHaveAttribute("data-row-control");
    expect(change).toHaveClass("@max-settings-row:basis-full");
    expect(change).not.toHaveClass("@max-settings-row:pl-36");
    const end = screen.getByRole("button", { name: "End session" });
    expect(end.parentElement).toHaveClass("@max-settings-row:basis-full", "@max-settings-row:pl-36");
    expect(end).toHaveClass("ghost-end:-mr-12");
    expect(screen.getByText("3 active").parentElement).not.toHaveAttribute("data-row-control");
  });

  it("leaves out the heading when a section has none", () => {
    render(
      <SettingsSection>
        <p>Rows</p>
      </SettingsSection>,
    );
    expect(screen.queryByRole("heading")).toBeNull();
  });
});
