import { afterEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import axe from "axe-core";
import { useState } from "react";
import { FilterChip } from "./filter-chip.js";
import { Menu } from "../menu/menu.js";

afterEach(() => vi.useRealTimers());

const MODELS = [
  "gpt-5.4-mini",
  "gpt-5.4-nano",
  "gpt-5.6",
  "gpt-6-luna",
  "gpt-6-sol",
  "claude-haiku-5-5",
  "claude-opus-5-5",
  "claude-sonnet-5-5",
  "text-embedding-3-small",
];

describe("FilterChip", () => {
  it("opens its values when a filter is just added, and searches a long list", async () => {
    render(<FilterChip property="Model" operator="is" value="any" values={MODELS.map((m) => ({ value: m, label: m }))} defaultValuesOpen />);
    const search = await screen.findByRole("searchbox", { name: "Search model values" });
    fireEvent.change(search, { target: { value: "claude" } });
    expect(screen.getAllByRole("menuitemradio").map((i) => i.textContent)).toEqual(["claude-haiku-5-5", "claude-opus-5-5", "claude-sonnet-5-5"]);
    fireEvent.change(search, { target: { value: "nothing" } });
    expect(screen.queryAllByRole("menuitemradio")).toHaveLength(0);
    expect(screen.getByText("No values match")).toBeInTheDocument();
  });
  it("matches a rich label by its text value", async () => {
    const values = MODELS.map((m) => ({ value: m, label: <b>{m.toUpperCase()}</b>, textValue: `Display ${m}` }));
    render(<FilterChip property="Model" operator="is" value="any" values={values} defaultValuesOpen />);
    fireEvent.change(await screen.findByRole("searchbox"), { target: { value: "display gpt-6" } });
    expect(screen.getAllByRole("menuitemradio")).toHaveLength(2);
  });
  it("keeps a short list without a search", async () => {
    render(
      <FilterChip
        property="Status"
        operator="is"
        value="any"
        values={[
          { value: "200", label: "200" },
          { value: "429", label: "429" },
        ]}
        defaultValuesOpen
      />,
    );
    await screen.findAllByRole("menuitemradio");
    expect(screen.queryByRole("searchbox")).toBeNull();
  });
  it("keeps its search field out of the menu's own children (axe aria-required-children)", async () => {
    render(<FilterChip property="Model" operator="is" value="any" values={MODELS.map((m) => ({ value: m, label: m }))} defaultValuesOpen />);
    await screen.findByRole("searchbox", { name: "Search model values" });
    const menu = screen.getByRole("menu");
    const result = await axe.run(menu, { runOnly: { type: "rule", values: ["aria-required-children"] } });
    expect(result.violations).toEqual([]);
    expect(menu.parentElement).toHaveAttribute("tabindex", "0");
  });
  it("moves from its search field to the values with the arrow keys, the menu's items in a list named by the chip", async () => {
    render(<FilterChip property="Model" operator="is" value="any" values={MODELS.map((m) => ({ value: m, label: m }))} defaultValuesOpen />);
    const search = await screen.findByRole("searchbox", { name: "Search model values" });
    const menu = screen.getByRole("menu");
    expect(menu).not.toContainElement(search);
    expect(menu.getAttribute("aria-labelledby")).toBeTruthy();
    search.focus();
    fireEvent.keyDown(search, { key: "ArrowDown" });
    await waitFor(() => expect(document.activeElement?.getAttribute("role")).toBe("menuitemradio"));
  });
  it("puts focus on its value when the values it opened with close, not on the page", async () => {
    function Adding() {
      const [added, setAdded] = useState(false);
      return added ? (
        <FilterChip property="Model" operator="is" value="any" values={MODELS.map((m) => ({ value: m, label: m }))} defaultValuesOpen />
      ) : (
        <button type="button" onClick={() => setAdded(true)}>
          Model
        </button>
      );
    }
    render(<Adding />);
    const add = screen.getByRole("button", { name: "Model" });
    add.focus();
    fireEvent.click(add);
    const search = await screen.findByRole("searchbox", { name: "Search model values" });
    fireEvent.keyDown(search, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    await waitFor(() => expect(document.activeElement).toBe(screen.getByRole("button", { name: "any" })));
  });
  it("keeps the focus in its search when it's added from a menu that then closes", async () => {
    function Adding() {
      const [added, setAdded] = useState(false);
      return (
        <>
          <Menu>
            <Menu.Trigger>
              <button type="button">Add filter</button>
            </Menu.Trigger>
            <Menu.Content>
              <Menu.Item onClick={() => setAdded(true)}>Model</Menu.Item>
            </Menu.Content>
          </Menu>
          {added && <FilterChip property="Model" operator="is" value="any" values={MODELS.map((m) => ({ value: m, label: m }))} defaultValuesOpen />}
        </>
      );
    }
    vi.useFakeTimers();
    render(<Adding />);
    fireEvent.click(screen.getByRole("button", { name: "Add filter" }));
    await act(() => vi.runAllTimersAsync());
    fireEvent.click(screen.getByRole("menuitem", { name: "Model" }));
    await act(() => vi.runAllTimersAsync());
    expect(document.activeElement).toBe(screen.getByRole("searchbox", { name: "Search model values" }));
    expect(screen.getAllByRole("menu")).toHaveLength(1);
  });
  it("toggles several values with rich labels in a long list", async () => {
    const onSelectedChange = vi.fn();
    const values = Array.from({ length: 60 }, (_, i) => ({ value: `m${i}`, label: <b>{`model-${i}`}</b>, textValue: `model-${i}` }));
    render(
      <FilterChip
        property="Model"
        operator="is"
        value="any"
        values={values}
        selected={["m1"]}
        multiple
        onSelectedChange={onSelectedChange}
        defaultValuesOpen
      />,
    );
    await screen.findByRole("listbox", { name: "Model" });
    fireEvent.click(screen.getByRole("option", { name: "model-2" }));
    expect(onSelectedChange).toHaveBeenLastCalledWith(["m1", "m2"]);
    fireEvent.click(screen.getByRole("option", { name: "model-1" }));
    expect(onSelectedChange).toHaveBeenLastCalledWith([]);
    expect(screen.getByRole("listbox", { name: "Model" })).toBeInTheDocument();
  });

  it("captions groups in a short list too", async () => {
    const values = [
      { value: "anthropic", label: "Anthropic", group: "Connected" },
      { value: "deepseek", label: "DeepSeek", group: "Other providers" },
      { value: "groq", label: "Groq", group: "Other providers" },
    ];
    render(<FilterChip property="Provider" operator="is" value="any" values={values} defaultValuesOpen />);
    const other = await screen.findByRole("group", { name: "Other providers" });
    expect(other).toContainElement(screen.getByRole("menuitemradio", { name: "Groq" }));
    expect(screen.getByRole("group", { name: "Connected" })).toContainElement(screen.getByRole("menuitemradio", { name: "Anthropic" }));
  });

  it("opens a long list as a grouped, searchable list that renders only what is in view", async () => {
    const onSelectedChange = vi.fn();
    const values = Array.from({ length: 600 }, (_, i) => ({ value: `m${i}`, label: `model-${i}`, group: i % 2 ? "Anthropic" : "OpenAI" }));
    render(<FilterChip property="Model" operator="is" value="any" values={values} selected={["m3"]} onSelectedChange={onSelectedChange} defaultValuesOpen />);
    const list = await screen.findByRole("listbox", { name: "Model" });
    expect(screen.getAllByRole("option").length).toBeLessThan(80);
    expect(screen.getByRole("group", { name: "OpenAI" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "model-3" })).toHaveAttribute("aria-selected", "true");
    fireEvent.change(screen.getByRole("combobox", { name: "Search model values" }), { target: { value: "model-599" } });
    expect(screen.getAllByRole("option").map((o) => o.textContent)).toEqual(["model-599"]);
    fireEvent.click(screen.getByRole("option", { name: "model-599" }));
    expect(onSelectedChange).toHaveBeenCalledWith(["m599"]);
    await waitFor(() => expect(list).not.toBeInTheDocument());
  });
  it("doesn't open values it doesn't have, even when just added", () => {
    render(<FilterChip property="Key" operator="is" value="any" values={[]} defaultValuesOpen />);
    expect(screen.queryByRole("menu")).toBeNull();
    expect(screen.getByRole("button", { name: "any" })).toBeDisabled();
  });
});
