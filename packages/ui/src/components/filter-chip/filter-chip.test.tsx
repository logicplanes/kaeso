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
  it("doesn't open values it doesn't have, even when just added", () => {
    render(<FilterChip property="Key" operator="is" value="any" values={[]} defaultValuesOpen />);
    expect(screen.queryByRole("menu")).toBeNull();
    expect(screen.getByRole("button", { name: "any" })).toBeDisabled();
  });
});
