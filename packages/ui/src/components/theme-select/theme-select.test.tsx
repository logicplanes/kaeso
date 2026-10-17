import { afterEach, describe, expect, it } from "vitest";
import { createRef } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ThemeSelect } from "./theme-select.js";
import { ThemeProvider, THEME_STORAGE_KEY } from "../../theme/theme-provider.js";
import { LocaleProvider } from "../../i18n/index.js";
import de from "../../i18n/locales/de.js";
import zhCN from "../../i18n/locales/zh-CN.js";

afterEach(() => {
  document.documentElement.removeAttribute("data-theme");
  localStorage.clear();
});

describe("ThemeSelect", () => {
  it("shows the current choice and switches the theme, which it remembers", async () => {
    const user = userEvent.setup();
    render(
      <ThemeProvider>
        <ThemeSelect />
      </ThemeProvider>,
    );
    const trigger = screen.getByRole("combobox", { name: "Theme" });
    expect(trigger).toHaveTextContent("System");
    await user.click(trigger);
    expect((await screen.findAllByRole("option")).map((o) => o.textContent)).toEqual(["System", "Light", "Dark"]);
    await user.click(screen.getByRole("option", { name: "Dark" }));
    expect(document.documentElement).toHaveAttribute("data-theme", "dark");
    expect(localStorage.getItem(THEME_STORAGE_KEY)).toBe("dark");
    expect(trigger).toHaveTextContent("Dark");
    await user.click(trigger);
    await user.click(await screen.findByRole("option", { name: "System" }));
    expect(document.documentElement).not.toHaveAttribute("data-theme");
  });

  it("names itself and its choices in the provider's language", async () => {
    const user = userEvent.setup();
    const { rerender } = render(
      <LocaleProvider locale="de" messages={de}>
        <ThemeProvider>
          <ThemeSelect />
        </ThemeProvider>
      </LocaleProvider>,
    );
    const trigger = screen.getByRole("combobox", { name: "Design" });
    await user.click(trigger);
    expect((await screen.findAllByRole("option")).map((o) => o.textContent)).toEqual(["System", "Hell", "Dunkel"]);
    await user.keyboard("{Escape}");
    rerender(
      <LocaleProvider locale="zh-CN" messages={zhCN}>
        <ThemeProvider>
          <ThemeSelect />
        </ThemeProvider>
      </LocaleProvider>,
    );
    expect(screen.getByRole("combobox", { name: "主题" })).toHaveTextContent("跟随系统");
  });

  it("is a quiet small pill by default and takes the size, appearance and width asked for", () => {
    const ref = createRef<HTMLButtonElement>();
    const { rerender } = render(<ThemeSelect ref={ref} />);
    const trigger = screen.getByRole("combobox", { name: "Theme" });
    expect(ref.current).toBe(trigger);
    expect(trigger).toHaveClass("h-32", "material-ghost");
    expect(trigger).not.toHaveClass("w-full");
    rerender(<ThemeSelect ref={ref} size="md" appearance="default" fullWidth data-force="hover" />);
    expect(trigger).toHaveClass("h-36", "material-secondary", "w-full");
    expect(trigger).toHaveAttribute("data-force", "hover");
  });
});
