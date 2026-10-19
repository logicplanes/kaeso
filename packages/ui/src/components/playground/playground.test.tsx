import { describe, expect, it, vi } from "vitest";
import { useState } from "react";
import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Composer } from "./composer.js";
import { CompareGrid } from "./compare-grid.js";
import { ResponseView, UserMessage, Thread } from "./response-view.js";
import { ResponseMetrics, formatLatency } from "./response-metrics.js";
import { ParameterPanel, type PlaygroundParameters } from "./parameter-panel.js";
import { ModelPicker } from "./model-picker.js";
import { KAESO_SELECT_VARIANTS } from "../select/select.js";
import { PromptSuggestions } from "./prompt-suggestions.js";
import * as playground from "./index.js";

describe("Composer", () => {
  it("sends with Enter, adds a line with Shift Enter, and clears", async () => {
    const onSubmit = vi.fn();
    render(<Composer onSubmit={onSubmit} />);
    const box = screen.getByRole("textbox", { name: "Prompt" });
    await userEvent.type(box, "Summarize{Shift>}{Enter}{/Shift}the incident");
    expect(onSubmit).not.toHaveBeenCalled();
    expect(box).toHaveValue("Summarize\nthe incident");
    await userEvent.keyboard("{Enter}");
    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(onSubmit).toHaveBeenCalledWith("Summarize\nthe incident");
    expect(box).toHaveValue("");
    expect(box).toHaveAttribute("aria-keyshortcuts", "Enter Meta+Enter Control+Enter");
    expect(box).toHaveAttribute("enterkeyhint", "send");
  });

  it("also sends with ⌘ Enter and Ctrl Enter", async () => {
    const onSubmit = vi.fn();
    render(<Composer onSubmit={onSubmit} />);
    const box = screen.getByRole("textbox");
    await userEvent.type(box, "One");
    await userEvent.keyboard("{Meta>}{Enter}{/Meta}");
    expect(onSubmit).toHaveBeenLastCalledWith("One");
    await userEvent.type(box, "Two");
    await userEvent.keyboard("{Control>}{Enter}{/Control}");
    expect(onSubmit).toHaveBeenLastCalledWith("Two");
    expect(onSubmit).toHaveBeenCalledTimes(2);
  });

  it("sends only with ⌘ Enter when submitOn is mod-enter; Enter adds a line", async () => {
    const onSubmit = vi.fn();
    render(<Composer onSubmit={onSubmit} submitOn="mod-enter" />);
    const box = screen.getByRole("textbox");
    await userEvent.type(box, "Summarize{Enter}the incident");
    expect(onSubmit).not.toHaveBeenCalled();
    expect(box).toHaveValue("Summarize\nthe incident");
    await userEvent.keyboard("{Meta>}{Enter}{/Meta}");
    expect(onSubmit).toHaveBeenCalledWith("Summarize\nthe incident");
    await userEvent.type(box, "Again");
    await userEvent.keyboard("{Control>}{Enter}{/Control}");
    expect(onSubmit).toHaveBeenLastCalledWith("Again");
    expect(box).toHaveAttribute("aria-keyshortcuts", "Meta+Enter Control+Enter");
    expect(box).not.toHaveAttribute("enterkeyhint");
  });

  it("adds a line with Enter on a touch screen, and sends with the button", async () => {
    const real = window.matchMedia;
    window.matchMedia = (q: string) =>
      ({
        matches: q === "(pointer: coarse)",
        media: q,
        onchange: null,
        addListener() {},
        removeListener() {},
        addEventListener() {},
        removeEventListener() {},
        dispatchEvent: () => false,
      }) as MediaQueryList;
    try {
      const onSubmit = vi.fn();
      render(<Composer onSubmit={onSubmit} />);
      const box = screen.getByRole("textbox");
      await userEvent.type(box, "Summarize{Enter}the incident");
      expect(onSubmit).not.toHaveBeenCalled();
      expect(box).toHaveValue("Summarize\nthe incident");
      expect(box).not.toHaveAttribute("enterkeyhint");
      expect(box).toHaveAttribute("aria-keyshortcuts", "Meta+Enter Control+Enter");
      await userEvent.click(screen.getByRole("button", { name: "Send" }));
      expect(onSubmit).toHaveBeenCalledWith("Summarize\nthe incident");
    } finally {
      window.matchMedia = real;
    }
  });

  it("sends with the button, never a prompt of only spaces", async () => {
    const onSubmit = vi.fn();
    render(<Composer onSubmit={onSubmit} />);
    const send = screen.getByRole("button", { name: "Send" });
    const box = screen.getByRole("textbox");
    expect(send).toBeDisabled();
    await userEvent.type(box, "   ");
    expect(send).toBeDisabled();
    await userEvent.keyboard("{Enter}");
    await userEvent.keyboard("{Meta>}{Enter}{/Meta}");
    expect(onSubmit).not.toHaveBeenCalled();
    expect(box).toHaveValue("   ");
    await userEvent.type(box, "Hi");
    await userEvent.click(send);
    expect(onSubmit).toHaveBeenCalledWith("   Hi");
  });

  it("shows Stop while streaming, in the same button; Enter neither sends nor stops", async () => {
    const onSubmit = vi.fn();
    const onStop = vi.fn();
    const { rerender } = render(<Composer defaultValue="Next question" onSubmit={onSubmit} onStop={onStop} streaming />);
    expect(screen.queryByRole("button", { name: "Send" })).toBeNull();
    const stop = screen.getByRole("button", { name: "Stop" });
    await userEvent.click(stop);
    expect(onStop).toHaveBeenCalledTimes(1);
    const box = screen.getByRole("textbox");
    box.focus();
    await userEvent.keyboard("{Enter}");
    await userEvent.keyboard("{Meta>}{Enter}{/Meta}");
    expect(onSubmit).not.toHaveBeenCalled();
    expect(onStop).toHaveBeenCalledTimes(1);
    expect(box).toHaveValue("Next question");
    expect(box).not.toBeDisabled();
    rerender(<Composer defaultValue="Next question" onSubmit={onSubmit} onStop={onStop} />);
    expect(screen.getByRole("button", { name: "Send" })).toBe(stop);
    await userEvent.keyboard("{Enter}");
    expect(onSubmit).toHaveBeenCalledWith("Next question");
  });

  it("doesn't send while sending", async () => {
    const onSubmit = vi.fn();
    render(<Composer loading defaultValue="Hi" onSubmit={onSubmit} />);
    screen.getByRole("textbox").focus();
    await userEvent.keyboard("{Enter}");
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("is controlled, and disabled with its text box", async () => {
    const onSubmit = vi.fn();
    function Controlled() {
      const [v, setV] = useState("Draft");
      return <Composer value={v} onValueChange={setV} onSubmit={onSubmit} aria-label="Message all models" />;
    }
    render(<Controlled />);
    const box = screen.getByRole("textbox", { name: "Message all models" });
    await userEvent.type(box, " two");
    expect(box).toHaveValue("Draft two");
    await userEvent.keyboard("{Enter}");
    expect(onSubmit).toHaveBeenCalledWith("Draft two");
    expect(box).toHaveValue("Draft two");
  });

  it("disables everything when disabled", async () => {
    const onSubmit = vi.fn();
    render(<Composer disabled defaultValue="Hi" onSubmit={onSubmit} />);
    const box = screen.getByRole("textbox");
    expect(box).toBeDisabled();
    expect(screen.getByRole("button", { name: "Send" })).toBeDisabled();
    fireEvent.keyDown(box, { key: "Enter" });
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("ignores Enter while composing with an input method", () => {
    const onSubmit = vi.fn();
    render(<Composer defaultValue="日本" onSubmit={onSubmit} />);
    const box = screen.getByRole("textbox");
    for (const init of [{}, { metaKey: true }]) {
      const e = new KeyboardEvent("keydown", { key: "Enter", isComposing: true, bubbles: true, cancelable: true, ...init });
      box.dispatchEvent(e);
      expect(e.defaultPrevented).toBe(false);
    }
    fireEvent.keyDown(box, { key: "Enter", keyCode: 229 });
    expect(onSubmit).not.toHaveBeenCalled();
    fireEvent.keyDown(box, { key: "Enter", keyCode: 13 });
    expect(onSubmit).toHaveBeenCalledWith("日本");
  });
});

function Grid({ initial = ["GPT-5", "Claude Sonnet 4.5"] }: { initial?: string[] }) {
  const [cols, setCols] = useState(initial);
  return (
    <CompareGrid aria-label="Models" onAdd={() => setCols((c) => [...c, `Model ${c.length + 1}`])}>
      {cols.map((c) => (
        <CompareGrid.Column
          key={c}
          label={c}
          header={<span>{c}</span>}
          footer={<span>{c} metrics</span>}
          onRemove={() => setCols((x) => x.filter((y) => y !== c))}
        >
          <p>{c} thread</p>
        </CompareGrid.Column>
      ))}
    </CompareGrid>
  );
}

describe("CompareGrid", () => {
  it("lays out columns with header, body and footer, each a named region", () => {
    render(<Grid />);
    expect(screen.getByRole("group", { name: "Models" })).toBeInTheDocument();
    const gpt = screen.getByRole("region", { name: "GPT-5" });
    expect(within(gpt).getByText("GPT-5 metrics")).toBeInTheDocument();
    expect(within(gpt).getByRole("region", { name: "GPT-5 thread" })).toHaveTextContent("GPT-5 thread");
  });

  it("adds columns up to four, then disables add with a reason", async () => {
    render(<Grid />);
    const adds = () => screen.getAllByRole("button", { name: "Add model" });
    expect(within(screen.getByRole("region", { name: "Claude Sonnet 4.5" })).getByRole("button", { name: "Add model" })).toBeInTheDocument();
    await userEvent.click(adds()[0]!);
    await userEvent.click(adds()[0]!);
    expect(screen.getAllByRole("region", { name: /^(GPT-5|Claude Sonnet 4.5|Model \d)$/ })).toHaveLength(4);
    for (const b of adds()) expect(b).toHaveAttribute("aria-disabled", "true");
    expect(within(screen.getByRole("region", { name: "Model 4" })).getByRole("button", { name: "Add model" })).toBeInTheDocument();
  });

  it("removes a column, and hides remove on the last one", async () => {
    render(<Grid />);
    await userEvent.click(screen.getByRole("button", { name: "Remove GPT-5" }));
    expect(screen.queryByRole("region", { name: "GPT-5" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Remove Claude Sonnet 4.5" })).toBeNull();
  });

  it("switches columns on a phone with previous and next", async () => {
    const onActiveChange = vi.fn();
    render(
      <CompareGrid aria-label="Models" onActiveChange={onActiveChange}>
        <CompareGrid.Column label="GPT-5" header="GPT-5" />
        <CompareGrid.Column label="Claude Sonnet 4.5" header="Claude" />
        <CompareGrid.Column label="GPT-5 mini" header="mini" />
      </CompareGrid>,
    );
    expect(screen.getByText("1 of 3")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Previous column" })).toBeDisabled();
    await userEvent.click(screen.getByRole("button", { name: "Next column" }));
    expect(onActiveChange).toHaveBeenLastCalledWith(1);
    expect(screen.getByText("2 of 3")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Next column" }));
    expect(screen.getByRole("button", { name: "Next column" })).toBeDisabled();
  });

  it("follows a column it adds when the active column is controlled", async () => {
    const seen: number[] = [];
    function Controlled() {
      const [cols, setCols] = useState(["GPT-5", "Claude Sonnet 4.5"]);
      const [active, setActive] = useState(0);
      return (
        <CompareGrid
          aria-label="Models"
          active={active}
          onActiveChange={(i) => {
            seen.push(i);
            setActive(i);
          }}
          onAdd={() => setCols((c) => [...c, "GPT-5 mini"])}
        >
          {cols.map((c) => (
            <CompareGrid.Column key={c} label={c} header={c} />
          ))}
        </CompareGrid>
      );
    }
    render(<Controlled />);
    expect(screen.getByText("1 of 2")).toBeInTheDocument();
    await userEvent.click(screen.getAllByRole("button", { name: "Add model" })[0]!);
    expect(seen).toEqual([2]);
    expect(screen.getByText("3 of 3")).toBeInTheDocument();
  });

  it("reports only where a scroll it started lands, then follows the reader's own scrolling", async () => {
    const width = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "offsetWidth");
    const left = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "offsetLeft");
    Object.defineProperty(HTMLElement.prototype, "offsetWidth", { configurable: true, get: () => 300 });
    Object.defineProperty(HTMLElement.prototype, "offsetLeft", {
      configurable: true,
      get(this: HTMLElement) {
        return [...(this.parentElement?.children ?? [])].indexOf(this) * 300;
      },
    });
    const scroll = vi.spyOn(Element.prototype, "scrollTo").mockImplementation(() => undefined);
    try {
      const onActiveChange = vi.fn();
      render(
        <CompareGrid aria-label="Models" onActiveChange={onActiveChange}>
          <CompareGrid.Column label="GPT-5" header="GPT-5" />
          <CompareGrid.Column label="Claude Sonnet 4.5" header="Claude" />
          <CompareGrid.Column label="GPT-5 mini" header="mini" />
        </CompareGrid>,
      );
      const box = screen.getByRole("region", { name: "GPT-5" }).parentElement!;
      const scrollTo = (x: number) => {
        box.scrollLeft = x;
        fireEvent.scroll(box);
      };
      await userEvent.click(screen.getByRole("button", { name: "Next column" }));
      scrollTo(100);
      expect(screen.getByText("2 of 3")).toBeInTheDocument();
      scrollTo(300);
      fireEvent.pointerDown(box);
      scrollTo(600);
      expect(onActiveChange.mock.calls.map(([i]) => i)).toEqual([1, 2]);
      expect(screen.getByText("3 of 3")).toBeInTheDocument();
    } finally {
      scroll.mockRestore();
      if (width) Object.defineProperty(HTMLElement.prototype, "offsetWidth", width);
      if (left) Object.defineProperty(HTMLElement.prototype, "offsetLeft", left);
    }
  });

  it("has no switcher for a single column without add", () => {
    render(
      <CompareGrid aria-label="Models">
        <CompareGrid.Column label="GPT-5" header="GPT-5" />
      </CompareGrid>,
    );
    expect(screen.queryByRole("button", { name: "Next column" })).toBeNull();
  });
});

describe("ResponseView", () => {
  it("streams: nothing drawn while pending, then text that grows, then done", () => {
    const { rerender, container } = render(<ResponseView status="pending" />);
    const article = screen.getByRole("article", { name: "Response" });
    expect(article).toHaveAttribute("aria-busy", "true");
    expect(article.textContent?.replace("Waiting for the response", "")).toBe("");
    expect(article).toHaveAccessibleDescription("Waiting for the response");
    rerender(<ResponseView status="streaming" text="The p95 latency" />);
    expect(article).toHaveTextContent("The p95 latency");
    expect(article).toHaveAccessibleDescription("Responding");
    expect(container.querySelector("p")?.textContent).toBe("The p95 latency");
    rerender(<ResponseView status="streaming" text={"The p95 latency rose **40%**.\n\n| Model | p95 |\n|---|--:|\n| gpt-5 | 2.1 s |"} />);
    expect(screen.getByRole("table")).toBeInTheDocument();
    expect(container.querySelector("strong")).toHaveTextContent("40%");
    rerender(<ResponseView status="done" text="The p95 latency rose 40%." />);
    expect(article).not.toHaveAttribute("aria-busy");
    expect(article).toHaveAccessibleDescription("");
  });

  it("shows reasoning in a closed Thinking disclosure, live while it streams", async () => {
    const { rerender } = render(<ResponseView status="streaming" reasoning={"**Reading the logs**\n\nThe 429s start at 14:05"} />);
    const toggle = screen.getByRole("button", { name: /Thinking/ });
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    expect(toggle).toHaveTextContent("The 429s start at 14:05");
    rerender(<ResponseView status="streaming" reasoning={"**Reading the logs**\n\nThe 429s start at 14:05 and stop at 14:20"} />);
    expect(toggle).toHaveTextContent("stop at 14:20");
    await userEvent.click(toggle);
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByText("Reading the logs")).toBeInTheDocument();
    rerender(<ResponseView status="done" reasoning="Done reading." reasoningDuration={4200} text="Rate limits." />);
    expect(screen.getByRole("button", { name: /Thought for 4.2 s/ })).toBeInTheDocument();
  });

  it("is calm on errors and stops, with Retry", async () => {
    const onRetry = vi.fn();
    const { rerender } = render(<ResponseView status="error" onRetry={onRetry} />);
    expect(screen.getByText("No response")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(onRetry).toHaveBeenCalled();
    rerender(<ResponseView status="stopped" text="Half an ans" />);
    expect(screen.getByText("Stopped")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Retry" })).toBeNull();
    rerender(<ResponseView status="done" text="" />);
    expect(screen.getByText("The model returned no text")).toBeInTheDocument();
  });

  it("never renders HTML from a response", () => {
    const { container } = render(<ResponseView text={'<img src=x onerror="alert(1)"> [x](javascript:alert(1))'} reasoning="<script>alert(1)</script>" />);
    expect(container.querySelector("img, script, a[href^=javascript]")).toBeNull();
  });

  it("draws a thread of turns", () => {
    render(
      <Thread>
        <UserMessage maxLines={0}>{"Why did errors spike?\nAt 14:05."}</UserMessage>
        <ResponseView text="Rate limits." footer={<ResponseMetrics duration={1200} />} />
      </Thread>,
    );
    expect(screen.getByRole("group", { name: "Prompt" }).textContent).toBe("Why did errors spike?\nAt 14:05.");
    expect(screen.getByLabelText("Metrics")).toHaveTextContent("Total1.20 s");
  });
});

describe("ResponseMetrics", () => {
  it("formats latency, tokens and cost", () => {
    render(<ResponseMetrics timeToFirstToken={420} duration={3140} inputTokens={1204} outputTokens={856} reasoningTokens={312} cost="0.004213" />);
    const m = screen.getByLabelText("Metrics");
    expect(m).toHaveTextContent("First token420 ms");
    expect(m).toHaveTextContent("Total3.14 s");
    expect(m).toHaveTextContent("Tokens1,204 → 856");
    expect(m).toHaveTextContent("Reasoning312");
    expect(m).toHaveTextContent("Cost$0.00421");
    expect(screen.getByLabelText("1,204 in, 856 out")).toBeInTheDocument();
  });

  it("shows a dash for the token count that is not known yet", () => {
    render(<ResponseMetrics inputTokens={1204} />);
    const m = screen.getByLabelText("Metrics");
    expect(m).toHaveTextContent("Tokens1,204 → —not yet known");
    expect(m).not.toHaveTextContent("[object Object]");
    expect(screen.getByLabelText("1,204 in, unknown out")).toBeInTheDocument();
  });

  it("is empty before a response, keeping its height", () => {
    const { container } = render(<ResponseMetrics reasoning />);
    const dl = screen.getByLabelText("Metrics");
    expect(dl.textContent).toBe("");
    expect(dl.className).toContain("min-h-16");
    expect(container.querySelectorAll("dd")).toHaveLength(0);
  });

  it("holds placeholders in place while live, then shows what is known", () => {
    const { rerender, container } = render(<ResponseMetrics live reasoning />);
    const items = () => [...container.querySelectorAll("dd")].map((d) => d.textContent?.replace("not yet known", ""));
    expect(items()).toEqual(["—", "—", "—", "—", "—"]);
    rerender(<ResponseMetrics live reasoning timeToFirstToken={380} />);
    expect(items()).toEqual(["380 ms", "—", "—", "—", "—"]);
    rerender(<ResponseMetrics timeToFirstToken={380} duration={65_000} inputTokens={12} outputTokens={0} reasoningTokens={0} cost="12.5" />);
    expect(items()).toEqual(["380 ms", "1 min 5 s", "12 → 0", "0", "$12.50"]);
    rerender(<ResponseMetrics timeToFirstToken={380} duration={900} inputTokens={12} outputTokens={4} />);
    expect(items()).toEqual(["380 ms", "900 ms", "12 → 4"]);
  });

  it("counts the total quietly while live", () => {
    vi.useFakeTimers();
    try {
      const { container, rerender } = render(<ResponseMetrics live startedAt={Date.now() - 1500} />);
      expect(container.querySelectorAll("dd")[1]?.textContent).toBe("1.50 s");
      act(() => void vi.advanceTimersByTime(500));
      expect(container.querySelectorAll("dd")[1]?.textContent).toBe("2.00 s");
      rerender(<ResponseMetrics startedAt={Date.now() - 2000} duration={2140} />);
      expect(container.querySelectorAll("dd")[0]?.textContent).toBe("2.14 s");
    } finally {
      vi.useRealTimers();
    }
  });

  it("formatLatency", () => {
    expect(formatLatency(0)).toBe("0 ms");
    expect(formatLatency(999.4)).toBe("999 ms");
    expect(formatLatency(1000)).toBe("1.00 s");
    expect(formatLatency(12_345)).toBe("12.3 s");
    expect(formatLatency(-1)).toBe("—");
    expect(formatLatency(Number.NaN)).toBe("—");
  });
});

describe("ParameterPanel", () => {
  function Panel(props: { reasoning?: boolean; onChange?: (p: PlaygroundParameters) => void; initial?: PlaygroundParameters }) {
    const [v, setV] = useState<PlaygroundParameters>(props.initial ?? {});
    return (
      <ParameterPanel
        value={v}
        reasoning={props.reasoning}
        onValueChange={(n) => {
          setV(n);
          props.onChange?.(n);
        }}
      />
    );
  }

  it("shows model defaults while unset, and changes a value", async () => {
    const onChange = vi.fn();
    render(<Panel onChange={onChange} />);
    expect(screen.getAllByText("1.00 default")).toHaveLength(2);
    const temperature = screen.getByRole("slider", { name: "Temperature" });
    temperature.focus();
    await userEvent.keyboard("{ArrowLeft}");
    expect(onChange).toHaveBeenLastCalledWith({ temperature: 0.95 });
    await userEvent.type(screen.getByRole("textbox", { name: "System instructions" }), "Be brief.");
    expect(onChange).toHaveBeenLastCalledWith({ temperature: 0.95, instructions: "Be brief." });
  });

  it("resets parameters to defaults and keeps the instructions", async () => {
    const onChange = vi.fn();
    render(<Panel onChange={onChange} initial={{ instructions: "Be brief.", temperature: 0.2, maxOutputTokens: 512 }} />);
    await userEvent.click(screen.getByRole("button", { name: "Reset to defaults" }));
    expect(onChange).toHaveBeenLastCalledWith({ instructions: "Be brief." });
    expect(screen.getByRole("button", { name: "Reset to defaults" })).toBeDisabled();
  });

  it("asks about reasoning only for a model that reasons", () => {
    const { unmount } = render(<Panel />);
    expect(screen.queryByText("Reasoning effort")).toBeNull();
    expect(screen.queryByRole("radiogroup", { name: "Reasoning summary" })).toBeNull();
    unmount();
    render(<Panel reasoning />);
    expect(screen.getByText("Reasoning effort")).toBeInTheDocument();
    expect(screen.getByText("Reasoning summary")).toBeInTheDocument();
  });

  it("hides sampling for models that take none", () => {
    render(<ParameterPanel value={{}} onValueChange={() => undefined} sampling={false} instructions={false} />);
    expect(screen.queryByRole("slider")).toBeNull();
    expect(screen.queryByRole("textbox", { name: "System instructions" })).toBeNull();
    expect(screen.getByRole("textbox", { name: "Max output tokens" })).toBeInTheDocument();
  });
});

describe("ModelPicker", () => {
  it("shows the chosen model with its provider mark and name", () => {
    render(
      <ModelPicker
        value="gpt-5"
        options={[
          { value: "gpt-5", provider: "openai", name: "GPT-5" },
          { value: "claude-sonnet-4-5", provider: "anthropic", name: "Claude Sonnet 4.5" },
        ]}
      />,
    );
    const trigger = screen.getByRole("button", { name: "Model" });
    expect(trigger).toHaveTextContent("GPT-5");
    expect(trigger).toHaveTextContent("gpt-5");
  });

  it("draws its ghost in full text colour through Select's own appearance, without a styling warning", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    try {
      render(<ModelPicker value="gpt-5" options={[{ value: "gpt-5", provider: "openai", name: "GPT-5" }]} />);
      expect(screen.getByRole("button", { name: "Model" })).toHaveClass(...KAESO_SELECT_VARIANTS.appearance.header.classes.split(" "));
      expect(warn).not.toHaveBeenCalled();
    } finally {
      warn.mockRestore();
    }
  });
});

describe("PromptSuggestions", () => {
  it("offers up to three prompts as buttons", async () => {
    const onSelect = vi.fn();
    render(<PromptSuggestions suggestions={["One", "Two", "Three", "Four"]} onSelect={onSelect} />);
    const list = screen.getByRole("list", { name: "Example prompts" });
    expect(within(list).getAllByRole("button")).toHaveLength(3);
    await userEvent.click(screen.getByRole("button", { name: "Two" }));
    expect(onSelect).toHaveBeenCalledWith("Two");
  });
});

describe("playground variants", () => {
  it("document every key", () => {
    const entries = Object.entries(playground).filter(([k]) => /^KAESO_.+_VARIANTS$/.test(k) && !k.endsWith("_DEFAULT_VARIANTS"));
    expect(entries.length).toBe(7);
    for (const [, variants] of entries)
      for (const keys of Object.values(variants as Record<string, Record<string, { classes: string; description: string }>>))
        for (const v of Object.values(keys)) {
          expect(typeof v.classes).toBe("string");
          expect(v.description.length).toBeGreaterThan(2);
        }
  });
});
