import { afterEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { THEME_STORAGE_KEY, ThemeProvider, installFocusModality, themeScript, useTheme } from "./theme-provider.js";

function Probe() {
  const { theme, resolvedTheme, setTheme } = useTheme();
  return (
    <div>
      <output>{`${theme}/${resolvedTheme}`}</output>
      <button onClick={() => setTheme("dark")}>Dark</button>
      <button onClick={() => setTheme("system")}>System</button>
    </div>
  );
}

function mockSystem(dark: boolean) {
  const listeners = new Set<() => void>();
  const mq = {
    matches: dark,
    addEventListener: (_: string, l: () => void) => listeners.add(l),
    removeEventListener: (_: string, l: () => void) => listeners.delete(l),
  };
  vi.spyOn(window, "matchMedia").mockReturnValue(mq as unknown as MediaQueryList);
  return {
    change(next: boolean) {
      mq.matches = next;
      listeners.forEach((l) => l());
    },
  };
}

afterEach(() => {
  vi.restoreAllMocks();
  localStorage.clear();
  document.documentElement.removeAttribute("data-theme");
});

describe("ThemeProvider", () => {
  it("follows the system until a theme is chosen, then stores and applies it", async () => {
    const system = mockSystem(true);
    render(
      <ThemeProvider>
        <Probe />
      </ThemeProvider>,
    );
    expect(screen.getByRole("status")).toHaveTextContent("system/dark");
    act(() => system.change(false));
    expect(screen.getByRole("status")).toHaveTextContent("system/light");
    await userEvent.click(screen.getByRole("button", { name: "Dark" }));
    expect(localStorage.getItem(THEME_STORAGE_KEY)).toBe("dark");
    expect(document.documentElement).toHaveAttribute("data-theme", "dark");
    await userEvent.click(screen.getByRole("button", { name: "System" }));
    expect(localStorage.getItem(THEME_STORAGE_KEY)).toBeNull();
    expect(document.documentElement).not.toHaveAttribute("data-theme");
  });

  it("keeps the theme the head script applied while it mounts", () => {
    mockSystem(false);
    localStorage.setItem(THEME_STORAGE_KEY, "dark");
    document.documentElement.setAttribute("data-theme", "dark");
    const seen: (string | null)[] = [];
    const observer = new MutationObserver((records) => records.forEach((r) => seen.push(r.oldValue)));
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"], attributeOldValue: true });
    render(
      <ThemeProvider>
        <Probe />
      </ThemeProvider>,
    );
    observer.takeRecords().forEach((r) => seen.push(r.oldValue));
    observer.disconnect();
    expect(screen.getByRole("status")).toHaveTextContent("dark/dark");
    expect(document.documentElement).toHaveAttribute("data-theme", "dark");
    expect(seen.filter((v) => v !== "dark")).toEqual([]);
  });

  it("starts from a stored theme, or from defaultTheme when none is stored", () => {
    mockSystem(false);
    localStorage.setItem(THEME_STORAGE_KEY, "dark");
    const { unmount } = render(
      <ThemeProvider>
        <Probe />
      </ThemeProvider>,
    );
    expect(screen.getByRole("status")).toHaveTextContent("dark/dark");
    unmount();
    localStorage.clear();
    render(
      <ThemeProvider defaultTheme="light">
        <Probe />
      </ThemeProvider>,
    );
    expect(screen.getByRole("status")).toHaveTextContent("light/light");
  });

  it("applies the theme to a target element and survives storage that throws", async () => {
    mockSystem(false);
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("blocked");
    });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("blocked");
    });
    const region = document.createElement("section");
    render(
      <ThemeProvider defaultTheme="dark" target={() => region}>
        <Probe />
      </ThemeProvider>,
    );
    expect(region).toHaveAttribute("data-theme", "dark");
    await userEvent.click(screen.getByRole("button", { name: "Dark" }));
    expect(screen.getByRole("status")).toHaveTextContent("dark/dark");
  });

  it("works without a provider, in light", () => {
    render(<Probe />);
    expect(screen.getByRole("status")).toHaveTextContent("system/light");
    fireEvent.click(screen.getByRole("button", { name: "Dark" }));
    expect(screen.getByRole("status")).toHaveTextContent("system/light");
  });
});

describe("focus modality", () => {
  it("marks pointer use and clears it on a plain key press", () => {
    installFocusModality();
    installFocusModality();
    const root = document.documentElement;
    fireEvent.pointerDown(document.body);
    expect(root).toHaveAttribute("data-k-pointer");
    fireEvent.keyDown(document.body, { key: "c", metaKey: true });
    expect(root).toHaveAttribute("data-k-pointer");
    fireEvent.keyDown(document.body, { key: "Tab" });
    expect(root).not.toHaveAttribute("data-k-pointer");
  });

  it("ships an inline script that applies a stored theme before the first paint", () => {
    localStorage.setItem(THEME_STORAGE_KEY, "light");
    new Function(themeScript)();
    expect(document.documentElement).toHaveAttribute("data-theme", "light");
  });

  it("listens to nothing but the pointer and the keyboard, so any framework can ship it", () => {
    const listen = vi.spyOn(document, "addEventListener");
    delete (window as unknown as { __kModality?: number }).__kModality;
    new Function(themeScript)();
    expect(listen.mock.calls.map(([type]) => type).sort()).toEqual(["keydown", "pointerdown"]);
    listen.mockRestore();
  });
});
