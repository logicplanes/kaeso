import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { SettingsGroup, SettingsRow, SettingsSection } from "./settings.js";
import { Switch } from "../switch/switch.js";
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

  it("leaves out the heading when a section has none", () => {
    render(
      <SettingsSection>
        <p>Rows</p>
      </SettingsSection>,
    );
    expect(screen.queryByRole("heading")).toBeNull();
  });
});
