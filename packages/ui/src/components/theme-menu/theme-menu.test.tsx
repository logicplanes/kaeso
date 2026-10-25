import { afterEach, describe, expect, it } from "vitest";
import { createRef } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ThemeMenu } from "./theme-menu.js";
import { ThemeProvider, THEME_STORAGE_KEY } from "../../theme/theme-provider.js";

afterEach(() => {
  document.documentElement.removeAttribute("data-theme");
  localStorage.clear();
});

describe("ThemeMenu", () => {
  it("forwards its ref to the trigger and switches the theme, which it remembers", async () => {
    const user = userEvent.setup();
    const ref = createRef<HTMLButtonElement>();
    render(
      <ThemeProvider>
        <ThemeMenu ref={ref} variant="secondary" size="md" data-force="hover" />
      </ThemeProvider>,
    );
    const trigger = screen.getByRole("button", { name: "Theme" });
    expect(ref.current).toBe(trigger);
    expect(trigger).toHaveAttribute("data-force", "hover");
    await user.click(trigger);
    expect((await screen.findAllByRole("menuitemradio")).map((o) => o.textContent)).toEqual(["System", "Light", "Dark"]);
    await user.click(screen.getByRole("menuitemradio", { name: "Dark" }));
    expect(document.documentElement).toHaveAttribute("data-theme", "dark");
    expect(localStorage.getItem(THEME_STORAGE_KEY)).toBe("dark");
  });
});
