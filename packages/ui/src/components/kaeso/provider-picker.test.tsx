import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ProviderPicker, type ProviderPickerOption } from "./provider-picker.js";
import { LocaleProvider } from "../../i18n/index.js";

const providers: ProviderPickerOption[] = [
  { id: "openai", name: "OpenAI", connected: true, hint: "2 keys" },
  { id: "togetherai", name: "Together AI" },
  { id: "anthropic", name: "Anthropic", connected: true, hint: "1 key" },
  { id: "groq", name: "Groq" },
  { id: "deepseek", name: "DeepSeek", disabled: true },
];

const shown = (o: HTMLElement) =>
  [...o.children]
    .filter((c) => !c.hasAttribute("aria-hidden"))
    .map((c) => c.textContent)
    .join("");
const names = (el: HTMLElement) => within(el).getAllByRole("option").map(shown);

describe("ProviderPicker", () => {
  it("lists connected providers first with their hints, then the rest from A to Z", () => {
    render(<ProviderPicker providers={providers} aria-label="Provider" />);
    expect(names(screen.getByRole("group", { name: "Connected" }))).toEqual(["Anthropic1 key", "OpenAI2 keys"]);
    expect(names(screen.getByRole("group", { name: "Other providers" }))).toEqual(["DeepSeek", "Groq", "Together AI"]);
    expect(screen.getByRole("option", { name: "DeepSeek" })).toHaveAttribute("aria-disabled", "true");
  });

  it("draws the first letter of a provider's name where it has no mark, and names one without a name by its id", () => {
    render(<ProviderPicker providers={[{ id: "acme", name: "Zenith Labs" }, { id: "gemini" }]} aria-label="Provider" />);
    const zenith = screen.getByRole("option", { name: "Zenith Labs" });
    expect(zenith.querySelector("[aria-hidden] [role='img']")).toHaveTextContent(/^Z$/);
    expect(screen.getByRole("option", { name: "gemini" })).toBeInTheDocument();
  });

  it("skips the captions when nothing is connected and says when no provider matches", async () => {
    const user = userEvent.setup();
    render(<ProviderPicker providers={providers.map((p) => ({ ...p, connected: false }))} aria-label="Provider" autoFocus />);
    expect(screen.queryByRole("group")).not.toBeInTheDocument();
    expect(names(screen.getByRole("listbox", { name: "Provider" }))[0]).toBe("Anthropic1 key");
    await user.keyboard("tgt");
    expect(names(screen.getByRole("listbox"))).toEqual(["Together AI"]);
    await user.keyboard("zz");
    expect(screen.getByRole("status")).toHaveTextContent("No providers match");
  });

  it("chooses with the keyboard in a dialog step", async () => {
    const user = userEvent.setup();
    const onValueChange = vi.fn();
    function Step() {
      const [value, setValue] = useState<string | null>("openai");
      return (
        <ProviderPicker
          providers={providers}
          label="Provider"
          value={value}
          onValueChange={(v) => {
            onValueChange(v);
            setValue(v);
          }}
          autoFocus
        />
      );
    }
    render(<Step />);
    expect(screen.getByRole("option", { name: "OpenAI 2 keys" })).toHaveAttribute("aria-selected", "true");
    await user.keyboard("{ArrowDown}{Enter}");
    expect(onValueChange).toHaveBeenCalledWith("groq");
    expect(screen.getByRole("option", { name: "Groq" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("listbox", { name: "Provider" })).toBeInTheDocument();
  });

  it("opens from a pill with an option for every provider", async () => {
    const user = userEvent.setup();
    const onValueChange = vi.fn();
    render(
      <ProviderPicker
        appearance="select"
        size="sm"
        providers={providers}
        allLabel="All providers"
        aria-label="Provider"
        value="groq"
        onValueChange={onValueChange}
      />,
    );
    const trigger = screen.getByRole("button", { name: "Groq", description: "Provider" });
    expect(trigger).toHaveTextContent("Groq");
    render(<ProviderPicker appearance="select" providers={providers} allLabel="All providers" aria-label="Every provider" value={null} />);
    const all = screen.getByRole("button", { name: "All providers", description: "Every provider" });
    expect(all.firstElementChild).not.toHaveClass("text-fg-tertiary");
    expect(all).toHaveTextContent("All providers");
    await user.click(trigger);
    expect(screen.getByRole("combobox", { name: "Search providers" })).toHaveFocus();
    await user.keyboard("{Home}{Enter}");
    expect(onValueChange).toHaveBeenCalledWith(null);
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  });

  it("speaks German", () => {
    render(
      <LocaleProvider locale="de" messages={{ "providerPicker.connected": "Verbunden", "providerPicker.other": "Weitere Anbieter" }}>
        <ProviderPicker providers={providers} aria-label="Anbieter" />
      </LocaleProvider>,
    );
    expect(screen.getByRole("group", { name: "Weitere Anbieter" })).toBeInTheDocument();
  });

  it("shows tiles with each provider's mark, connected first, and continues only on a press", async () => {
    const user = userEvent.setup();
    const onValueChange = vi.fn();
    const onChoose = vi.fn();
    render(<ProviderPicker appearance="grid" providers={providers} aria-label="Provider" onValueChange={onValueChange} onChoose={onChoose} />);
    const group = screen.getByRole("radiogroup", { name: "Provider" });
    expect(
      within(screen.getByRole("group", { name: "Connected" }))
        .getAllByRole("radio")
        .map((r) => r.textContent),
    ).toEqual(["Anthropic1 key", "OpenAI2 keys"]);
    expect(
      within(screen.getByRole("group", { name: "Other providers" }))
        .getAllByRole("radio")
        .map((r) => r.textContent),
    ).toEqual(["DeepSeek", "Groq", "Together AI"]);
    expect(within(group).getByRole("radio", { name: "OpenAI 2 keys" })).toBeInTheDocument();
    expect(screen.queryByRole("searchbox")).not.toBeInTheDocument();
    await user.click(screen.getByRole("radio", { name: "Groq" }));
    expect(onValueChange).toHaveBeenLastCalledWith("groq");
    expect(onChoose).toHaveBeenCalledWith("groq");
    onChoose.mockClear();
    await user.keyboard("{ArrowRight}");
    expect(onValueChange).toHaveBeenLastCalledWith("togetherai");
    expect(onChoose).not.toHaveBeenCalled();
  });

  it("searches the tiles once there are many providers", async () => {
    const user = userEvent.setup();
    const many: ProviderPickerOption[] = Array.from({ length: 12 }, (_, i) => ({ id: `p${i}`, name: `Provider ${String.fromCharCode(65 + i)}` }));
    render(<ProviderPicker appearance="grid" providers={many} aria-label="Provider" />);
    expect(screen.getAllByRole("radio")).toHaveLength(12);
    await user.type(screen.getByRole("searchbox", { name: "Search providers" }), "provider c");
    expect(screen.getAllByRole("radio")).toHaveLength(1);
    expect(screen.getByRole("radio", { name: "Provider C" })).toBeInTheDocument();
    await user.clear(screen.getByRole("searchbox", { name: "Search providers" }));
    await user.type(screen.getByRole("searchbox", { name: "Search providers" }), "nothing");
    expect(screen.getByText("No providers match")).toBeInTheDocument();
  });
});
