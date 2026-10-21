import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ModelPicker, type ModelPickerOption } from "./model-picker.js";

const options: ModelPickerOption[] = [
  { value: "gpt-5", provider: "openai", name: "GPT-5", capabilities: ["tools", "reasoning"], context: 400_000, price: "$1.25 / $10" },
  { value: "gpt-5-mini", provider: "openai", name: "GPT-5 mini" },
  { value: "gpt-4", provider: "openai", name: "GPT-4", deprecated: true },
  { value: "claude-sonnet-4-5", provider: "anthropic", name: "Claude Sonnet 4.5", note: "No key", disabled: true },
  { value: "deepseek-chat", provider: "deepseek", model: "deepseek-chat" },
];

const providers = [
  { id: "openai", name: "OpenAI" },
  { id: "anthropic", name: "Anthropic" },
  { id: "deepseek", name: "DeepSeek" },
];

const open = async (user: ReturnType<typeof userEvent.setup>) => {
  await user.click(screen.getByRole("button", { name: "Model" }));
  return screen.findByRole("listbox", { name: "Model" });
};

describe("ModelPicker", () => {
  it("groups models under their providers, connected ones first, with recent models on top", async () => {
    const user = userEvent.setup();
    render(<ModelPicker options={options} providers={[...providers, { id: "deepseek", name: "DeepSeek", connected: true }]} recent={["gpt-5-mini"]} />);
    const list = await open(user);
    expect(
      within(list)
        .getAllByRole("group")
        .map((g) => g.getAttribute("aria-labelledby") && document.getElementById(g.getAttribute("aria-labelledby")!)?.textContent),
    ).toEqual(["Recent", "DeepSeek", "Anthropic", "OpenAI"]);
    expect(
      within(screen.getByRole("group", { name: "OpenAI" }))
        .getAllByRole("option")
        .map((o) => o.textContent),
    ).toEqual(["GPT-5gpt-5400K · $1.25 / $10", "GPT-5 minigpt-5-mini"]);
    expect(screen.getByRole("img", { name: "Capabilities: Tool calling, Reasoning" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: /Claude Sonnet 4.5/ })).toHaveAttribute("aria-disabled", "true");
    expect(screen.getByRole("option", { name: /Claude Sonnet 4.5/ })).toHaveTextContent("No key");
  });

  it("names each provider as the caller does, and one it doesn't name by its id", async () => {
    const user = userEvent.setup();
    render(
      <ModelPicker
        options={[...options, { value: "acme/m1", provider: "acme", model: "m1" }]}
        providers={[...providers, { id: "deepseek", name: "DeepSeek" }]}
        value="claude-sonnet-4-5"
      />,
    );
    expect(screen.getByRole("button", { name: "Model" })).toContainElement(screen.getByRole("img", { name: "Anthropic" }));
    await user.click(screen.getByRole("button", { name: "Model" }));
    const groups = within(await screen.findByRole("listbox", { name: "Model" })).getAllByRole("group");
    expect(groups.map((g) => document.getElementById(g.getAttribute("aria-labelledby")!)?.textContent)).toEqual(["acme", "Anthropic", "DeepSeek", "OpenAI"]);
    await user.click(screen.getByRole("combobox", { name: "Provider" }));
    expect(screen.getByRole("option", { name: /^acme/ })).toBeInTheDocument();
  });

  it("searches by name, id and provider, and closes with the chosen model shown", async () => {
    const user = userEvent.setup();
    const onValueChange = vi.fn();
    function Controlled() {
      const [value, setValue] = useState<string | null>(null);
      return (
        <ModelPicker
          options={options}
          value={value}
          onValueChange={(v) => {
            onValueChange(v);
            setValue(v);
          }}
        />
      );
    }
    render(<Controlled />);
    await open(user);
    expect(screen.getByRole("combobox", { name: "Search models" })).toHaveFocus();
    await user.keyboard("deepseek");
    expect(screen.getAllByRole("option").map((o) => o.textContent)).toEqual(["deepseek-chat"]);
    await user.clear(screen.getByRole("combobox", { name: "Search models" }));
    await user.keyboard("gpt5mini{Enter}");
    expect(onValueChange).toHaveBeenCalledWith("gpt-5-mini");
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Model" })).toHaveTextContent("GPT-5 mini");
  });

  it("hides deprecated models behind a toggle and filters by provider", async () => {
    const user = userEvent.setup();
    render(<ModelPicker options={options} providers={providers} />);
    await open(user);
    expect(screen.queryByRole("option", { name: /GPT-4\b/ })).not.toBeInTheDocument();
    await user.click(screen.getByRole("switch", { name: "Deprecated" }));
    expect(screen.getByRole("option", { name: /GPT-4/ })).toBeInTheDocument();
    await user.click(screen.getByRole("combobox", { name: "Provider" }));
    expect(screen.getAllByRole("option", { name: /^(All providers|Anthropic|DeepSeek|OpenAI)/ }).map((o) => o.textContent)).toEqual([
      "All providers",
      "Anthropic1",
      "DeepSeek1",
      "OpenAI3",
    ]);
    await user.click(screen.getByRole("option", { name: /^Anthropic/ }));
    expect(screen.getAllByRole("option").map((o) => o.textContent)).toEqual(["Claude Sonnet 4.5claude-sonnet-4-5No key"]);
    await user.click(screen.getByRole("combobox", { name: "Provider" }));
    await user.click(screen.getByRole("option", { name: "All providers" }));
    expect(screen.getAllByRole("option")).toHaveLength(5);
  });

  it("offers only the providers whose models match the search, and opens fresh", async () => {
    const user = userEvent.setup();
    const onValueChange = vi.fn();
    render(<ModelPicker options={options} providers={providers} onValueChange={onValueChange} />);
    await open(user);
    await user.keyboard("gpt");
    await user.click(screen.getByRole("combobox", { name: "Provider" }));
    expect(screen.getAllByRole("option", { name: /^(All providers|Anthropic|DeepSeek|OpenAI)/ }).map((o) => o.textContent)).toEqual([
      "All providers",
      "OpenAI2",
    ]);
    await user.click(screen.getByRole("option", { name: /^OpenAI/ }));
    await user.click(screen.getAllByRole("option").find((o) => o.textContent?.startsWith("GPT-5 mini")) as HTMLElement);
    expect(onValueChange).toHaveBeenCalledWith("gpt-5-mini");
    expect(screen.queryByRole("listbox", { name: "Model" })).not.toBeInTheDocument();
    await open(user);
    expect(screen.getByRole("combobox", { name: "Search models" })).toHaveValue("");
    expect(screen.getByRole("combobox", { name: "Provider" })).toHaveTextContent("All providers");
    expect(screen.getAllByRole("option")).toHaveLength(4);
  });

  it("chooses several models and offers an option for every model", async () => {
    const user = userEvent.setup();
    const onValueChange = vi.fn();
    const { unmount } = render(<ModelPicker multiple options={options} defaultValue={["gpt-5"]} onValueChange={onValueChange} />);
    await open(user);
    await user.click(screen.getByRole("option", { name: /GPT-5 mini/ }));
    expect(onValueChange).toHaveBeenLastCalledWith(["gpt-5", "gpt-5-mini"]);
    const gpt5 = () => screen.getAllByRole("option").find((o) => o.textContent?.startsWith("GPT-5gpt-5")) as HTMLElement;
    await user.click(gpt5());
    expect(onValueChange).toHaveBeenLastCalledWith(["gpt-5-mini"]);
    await user.click(gpt5());
    expect(screen.getByRole("listbox")).toBeInTheDocument();
    await user.keyboard("{Escape}{Escape}");
    expect(screen.getByRole("button", { name: "Model" })).toHaveTextContent("GPT-5 mini +1");
    unmount();

    render(<ModelPicker options={options} allLabel="All models" appearance="default" size="md" onValueChange={onValueChange} />);
    expect(screen.getByRole("button", { name: "Model" })).toHaveTextContent("All models");
    await open(user);
    await user.keyboard("{ArrowDown}{Enter}");
    expect(onValueChange).toHaveBeenLastCalledWith("deepseek-chat");
    await open(user);
    await user.keyboard("{Home}{Enter}");
    expect(onValueChange).toHaveBeenLastCalledWith("");
  });

  it("stays fast with thousands of models", async () => {
    const user = userEvent.setup();
    const big = Array.from({ length: 10_000 }, (_, i) => ({ value: `m${i}`, provider: `p${i % 300}`, model: `model-${i}` }));
    render(<ModelPicker options={big} />);
    await open(user);
    expect(screen.getAllByRole("option").length).toBeLessThan(80);
    await user.paste("model-9999");
    expect(screen.getAllByRole("option")[0]).toHaveTextContent("model-9999");
  });
});
