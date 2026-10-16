import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ParameterPanel, ParameterPopover } from "./parameter-panel";

describe("ParameterPanel reasoning and limits", () => {
  it("offers only the efforts a model supports, and sets the summary", async () => {
    const onChange = vi.fn();
    render(
      <ParameterPanel layout="panel" value={{ reasoningEffort: "low" }} onValueChange={onChange} reasoning={["low", "high"]} maxOutputTokensLimit={8192} />,
    );
    await userEvent.click(screen.getByRole("combobox", { name: "Reasoning effort" }));
    expect(screen.queryByRole("option", { name: /Medium/ })).toBeNull();
    await userEvent.click(screen.getByRole("option", { name: /High/ }));
    expect(onChange).toHaveBeenLastCalledWith({ reasoningEffort: "high" });
    await userEvent.click(screen.getByRole("radio", { name: "Detailed" }));
    expect(onChange).toHaveBeenLastCalledWith({ reasoningEffort: "low", reasoningSummary: "detailed" });
  });

  it("returns effort and summary to the model default", async () => {
    const onChange = vi.fn();
    render(<ParameterPanel value={{ reasoningEffort: "high", reasoningSummary: "off" }} onValueChange={onChange} reasoning />);
    await userEvent.click(screen.getByRole("radio", { name: "Auto" }));
    expect(onChange).toHaveBeenLastCalledWith({ reasoningEffort: "high" });
    await userEvent.click(screen.getByRole("combobox", { name: "Reasoning effort" }));
    await userEvent.click(screen.getByRole("option", { name: "Model default" }));
    expect(onChange).toHaveBeenLastCalledWith({ reasoningSummary: "off" });
  });

  it("takes a positive whole number of output tokens, or none", () => {
    const onChange = vi.fn();
    render(<ParameterPanel value={{}} onValueChange={onChange} sampling={false} instructions={false} />);
    const input = screen.getByRole("textbox", { name: "Max output tokens" });
    fireEvent.change(input, { target: { value: "1024.6" } });
    expect(onChange).toHaveBeenLastCalledWith({ maxOutputTokens: 1025 });
    fireEvent.change(input, { target: { value: "0" } });
    expect(onChange).toHaveBeenLastCalledWith({ maxOutputTokens: undefined });
    fireEvent.change(input, { target: { value: "" } });
    expect(onChange).toHaveBeenLastCalledWith({ maxOutputTokens: undefined });
  });

  it("moves top P with the keyboard", () => {
    const onChange = vi.fn();
    render(<ParameterPanel value={{ topP: 0.5 }} onValueChange={onChange} instructions={false} />);
    const slider = screen.getAllByRole("slider")[1]!;
    fireEvent.keyDown(slider, { key: "ArrowRight" });
    expect(onChange).toHaveBeenLastCalledWith({ topP: 0.51 });
  });
});

describe("ParameterPopover", () => {
  it("opens the panel from its circle and from a custom trigger", async () => {
    const onOpenChange = vi.fn();
    const { rerender } = render(<ParameterPopover value={{}} onValueChange={vi.fn()} onOpenChange={onOpenChange} />);
    await userEvent.click(screen.getByRole("button", { name: "Parameters" }));
    expect(onOpenChange).toHaveBeenCalledWith(true);
    expect(await screen.findByRole("dialog", { name: "Parameters" })).toBeInTheDocument();
    rerender(<ParameterPopover value={{}} onValueChange={vi.fn()} open title="Settings" trigger={<button>Tune</button>} side="top" align="start" />);
    expect(await screen.findByRole("dialog", { name: "Settings" })).toBeInTheDocument();
    render(<ParameterPopover value={{}} onValueChange={vi.fn()} defaultOpen title="Defaults" />);
    expect(await screen.findByRole("dialog", { name: "Defaults" })).toBeInTheDocument();
  });
});
