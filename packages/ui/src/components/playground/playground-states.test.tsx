import { describe, expect, it, vi } from "vitest";
import { useState } from "react";
import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Composer } from "./composer.js";
import { CompareGrid } from "./compare-grid.js";
import { ResponseView, UserMessage } from "./response-view.js";
import { ResponseMetrics } from "./response-metrics.js";
import { Reasoning } from "./reasoning.js";
import { ParameterPanel, ParameterPopover } from "./parameter-panel.js";
import { PromptSuggestions } from "./prompt-suggestions.js";
import { ConversationView } from "../kaeso/conversation.js";

function Chat({ onStop }: { onStop?: () => void }) {
  const [streaming, setStreaming] = useState(false);
  return (
    <Composer
      streaming={streaming}
      onSubmit={() => setStreaming(true)}
      onStop={() => {
        onStop?.();
        setStreaming(false);
      }}
    />
  );
}

describe("Composer focus and hit areas", () => {
  it("keeps the focus on Send and Stop through a whole response", async () => {
    render(<Chat />);
    await userEvent.type(screen.getByRole("textbox", { name: "Prompt" }), "Summarize the incident");
    const send = screen.getByRole("button", { name: "Send" });
    send.focus();
    await userEvent.keyboard("{Enter}");
    const stop = screen.getByRole("button", { name: "Stop" });
    expect(stop).toHaveFocus();
    await userEvent.keyboard("{Enter}");
    const idle = screen.getByRole("button", { name: "Send" });
    expect(idle).toHaveFocus();
    expect(idle).toHaveAttribute("aria-disabled", "true");
  });

  it("says why Enter waits while a response arrives with a prompt written", async () => {
    const { rerender } = render(<Composer streaming defaultValue="" />);
    const box = screen.getByRole("textbox", { name: "Prompt" });
    expect(box).not.toHaveAttribute("aria-describedby");
    await userEvent.type(box, "Shorter, please");
    expect(box).toHaveAccessibleDescription("A response is arriving; stop it to send this prompt.");
    rerender(<Composer defaultValue="" />);
    expect(box).not.toHaveAttribute("aria-describedby");
  });

  it("puts the cursor in the text box on a click anywhere in the box but its controls", () => {
    render(<Composer actions={<button type="button">Parameters</button>} />);
    const box = screen.getByRole("textbox", { name: "Prompt" });
    const row = screen.getByRole("button", { name: "Parameters" }).parentElement!;
    fireEvent.mouseDown(row);
    expect(box).toHaveFocus();
    box.blur();
    fireEvent.mouseDown(screen.getByRole("button", { name: "Parameters" }));
    expect(box).not.toHaveFocus();
  });

  it("puts nested secondary and ghost buttons on the composer's own tones", () => {
    render(<Composer streaming />);
    expect(screen.getByRole("button", { name: "Stop" }).parentElement).toHaveClass("controls-on-fill");
  });
});

function Grid({ onRemove }: { onRemove?: () => void }) {
  const [models, setModels] = useState(["GPT-5", "Claude Sonnet 4.5"]);
  return (
    <CompareGrid aria-label="Models" onAdd={() => setModels((m) => [...m, `Model ${m.length + 1}`])}>
      {models.map((m) => (
        <CompareGrid.Column
          key={m}
          label={m}
          header={<button type="button">{m}</button>}
          onRemove={() => {
            onRemove?.();
            setModels((all) => all.filter((x) => x !== m));
          }}
        />
      ))}
    </CompareGrid>
  );
}

describe("CompareGrid focus and headers", () => {
  it("moves the focus to a column it adds, and to the column that takes a removed one's place", async () => {
    render(<Grid />);
    const add = screen.getAllByRole("button", { name: "Add model" });
    add[add.length - 1]!.focus();
    await userEvent.keyboard("{Enter}");
    expect(screen.getByRole("button", { name: "Model 3" })).toHaveFocus();
    screen.getByRole("button", { name: "Remove GPT-5" }).focus();
    await userEvent.keyboard("{Enter}");
    expect(screen.getByRole("button", { name: "Remove Claude Sonnet 4.5" })).toHaveFocus();
    screen.getByRole("button", { name: "Remove Model 3" }).focus();
    await userEvent.keyboard("{Enter}");
    expect(screen.getByRole("button", { name: "Claude Sonnet 4.5" })).toHaveFocus();
  });

  it("keeps every header the same, with Add in a rail of its own after the columns", () => {
    render(<Grid />);
    const headers = screen.getAllByRole("region", { name: /^(GPT-5|Claude Sonnet 4\.5)$/ }).map((r) => r.querySelector("header")!);
    expect(headers.map((h) => h.children.length)).toEqual([2, 2]);
    for (const h of headers) expect(within(h).queryByRole("button", { name: "Add model" })).toBeNull();
    const rail = screen.getAllByRole("button", { name: "Add model" }).at(-1)!.closest(".w-56");
    expect(rail).toHaveClass("hairline-l", "md:flex");
  });

  it("keeps the focus on Next column when it reaches the last column", async () => {
    const scroll = vi.spyOn(HTMLElement.prototype, "scrollTo").mockImplementation(() => undefined);
    try {
      render(<Grid />);
      const next = screen.getByRole("button", { name: "Next column" });
      next.focus();
      await userEvent.keyboard("{Enter}");
      expect(next).toHaveFocus();
      expect(next).toHaveAttribute("aria-disabled", "true");
      expect(screen.getByText("Claude Sonnet 4.5, 2 of 2")).toBeInTheDocument();
    } finally {
      scroll.mockRestore();
    }
  });

  it("announces the column's name with its place in one sentence", () => {
    render(<Grid />);
    expect(screen.getByText("GPT-5, 1 of 2")).toHaveClass("sr-only");
  });

  it("draws the thread's focus ring outside its faded scroller", () => {
    render(<Grid />);
    const thread = screen.getByRole("region", { name: "GPT-5 thread" });
    expect(thread.parentElement).toHaveClass("compare-thread");
    expect(thread).toHaveClass("scroll-fade-y");
    expect(thread).not.toHaveClass("focus-ring-inset");
  });
});

describe("ResponseView marks and announcements", () => {
  it("draws a quiet caret while waiting and at the end while writing, and none when done", () => {
    const { container, rerender } = render(<ResponseView status="pending" />);
    expect(container.querySelector("[data-pending] > span")).toHaveClass("animate-pulse-quiet", "reduced:animate-none");
    rerender(<ResponseView status="streaming" text="The p95 latency" />);
    expect(container.querySelector("[data-pending]")).toBeNull();
    expect(container.querySelector("p [data-caret]")).toBeInTheDocument();
    rerender(<ResponseView status="done" text="The p95 latency rose." />);
    expect(container.querySelector("[data-caret], [data-pending]")).toBeNull();
  });

  it("says once, by name, that a response finished, stopped or was cut off", () => {
    const { rerender } = render(<ResponseView aria-label="GPT-5 response" status="done" text="Earlier answer" />);
    const status = screen.getByRole("status");
    expect(status).toHaveTextContent("");
    rerender(<ResponseView aria-label="GPT-5 response" status="streaming" text="Rate" />);
    expect(status).toHaveTextContent("");
    rerender(<ResponseView aria-label="GPT-5 response" status="done" text="Rate limits." />);
    expect(status).toHaveTextContent("GPT-5 response finished");
    rerender(<ResponseView aria-label="GPT-5 response" status="pending" />);
    rerender(<ResponseView aria-label="GPT-5 response" status="stopped" text="Rate" />);
    expect(status).toHaveTextContent("GPT-5 response stopped");
    rerender(<ResponseView aria-label="GPT-5 response" status="streaming" text="Rate" />);
    rerender(<ResponseView aria-label="GPT-5 response" status="incomplete" incompleteReason="max_output_tokens" text="Rate" />);
    expect(status).toHaveTextContent("GPT-5 response: Stopped at the output limit");
    rerender(<ResponseView aria-label="GPT-5 response" status="streaming" text="Rate" />);
    rerender(<ResponseView aria-label="GPT-5 response" status="error" error="The provider timed out." />);
    expect(status).toHaveTextContent("");
    expect(screen.getByRole("alert")).toHaveTextContent("No response");
  });

  it("offers one Retry, a 32px secondary button, after a stop, a cut-off reply and an error", () => {
    const { rerender } = render(<ResponseView status="stopped" text="Half" onRetry={() => undefined} />);
    for (const props of [{ status: "incomplete" as const }, { status: "error" as const, error: "Timed out." }, { status: "stopped" as const }]) {
      rerender(<ResponseView text="Half" onRetry={() => undefined} {...props} />);
      const retry = screen.getByRole("button", { name: "Retry" });
      expect(retry).toHaveClass("h-32", "material-secondary");
      expect(screen.queryByRole("alert")?.contains(retry) ?? false).toBe(false);
    }
  });

  it("sets text 12px in and leaves code and tables on the edge; the footer on the text line", () => {
    const { container } = render(<ResponseView text={"Two did:\n\n```json\n{}\n```"} footer={<ResponseMetrics duration={1200} />} />);
    expect(container.querySelector(".markdown-text-inset")).toBeInTheDocument();
    expect(screen.getByLabelText("Metrics").parentElement).toHaveClass("px-12");
  });

  it("opens a long prompt with a disclosure button that says whether it is open", async () => {
    const height = vi.spyOn(HTMLElement.prototype, "scrollHeight", "get").mockReturnValue(200);
    try {
      render(<UserMessage maxLines={2}>{"Why did errors spike?"}</UserMessage>);
      const more = screen.getByRole("button", { name: "Show all" });
      expect(more).toHaveAttribute("aria-expanded", "false");
      expect(more).toHaveClass("h-32");
      await userEvent.click(more);
      expect(screen.getByRole("button", { name: "Show less" })).toHaveAttribute("aria-expanded", "true");
    } finally {
      height.mockRestore();
    }
  });
});

describe("ResponseMetrics room and money", () => {
  it("keeps the decimals the API sent, two at least, six at most", () => {
    const { rerender } = render(<ResponseMetrics cost="1.2311" />);
    const cost = () => screen.getByText("Cost").nextElementSibling?.firstElementChild?.textContent;
    expect(cost()).toBe("$1.2311");
    rerender(<ResponseMetrics cost="0.5" />);
    expect(cost()).toBe("$0.50");
    rerender(<ResponseMetrics cost="0.0000004" />);
    expect(cost()).toBe("<\u00a0$0.000001");
  });

  it("reserves a typical value's room under every value, live or done", () => {
    const { container, rerender } = render(<ResponseMetrics live reasoning />);
    const reserved = () => [...container.querySelectorAll("dd > [aria-hidden]")].map((s) => s.textContent);
    const live = reserved();
    expect(live).toEqual(["8.88 s", "8.88 s", "8,888 in · 888 out", "888", "$0.008888"]);
    rerender(<ResponseMetrics reasoning timeToFirstToken={420} duration={3140} inputTokens={1204} outputTokens={856} reasoningTokens={312} cost="0.004213" />);
    expect(reserved()).toEqual(live);
  });

  it("keeps the reasoning item after a response whose reasoning wasn't reported", () => {
    render(<ResponseMetrics reasoning duration={1200} inputTokens={12} outputTokens={4} />);
    expect(screen.getByText("Reasoning").nextElementSibling).toHaveTextContent("—not reported");
  });
});

describe("Reasoning and the request log share one disclosure", () => {
  it("puts the chevron first, and grows to 44px for a coarse pointer", () => {
    render(<Reasoning text="Checking the facts" duration={4200} />);
    const trigger = screen.getByRole("button", { name: /Thought for 4\.2 s/ });
    expect(trigger.firstElementChild?.tagName.toLowerCase()).toBe("svg");
    expect(trigger).toHaveClass("h-32", "coarse:h-44", "rounded-pill");
  });

  it("draws the request log's reasoning with the same trigger", async () => {
    render(<ConversationView messages={[{ role: "assistant", content: [{ type: "reasoning", text: "", redacted: true }] }]} />);
    const trigger = screen.getByRole("button", { name: /Reasoning/ });
    expect(trigger).toHaveClass("h-32", "coarse:h-44", "rounded-pill");
    expect(trigger).toHaveTextContent("ReasoningRedacted by the provider");
    await userEvent.click(trigger);
    expect(trigger).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByText("The provider returned this reasoning in encrypted form.")).toBeVisible();
  });

  it("says when a request has no messages", () => {
    render(<ConversationView messages={[]} />);
    expect(screen.getByText("This request has no messages.")).toBeInTheDocument();
  });
});

describe("ParameterPanel words and the popover's way out", () => {
  it("names the model's maximum under Max output tokens", () => {
    render(<ParameterPanel value={{}} onValueChange={() => undefined} maxOutputTokensLimit={128000} />);
    expect(screen.getByLabelText("Max output tokens")).toHaveAccessibleDescription("Up to 128,000 for this model");
  });

  it("opens on its first field, keeps the focus inside, and closes with its Close button", async () => {
    render(<ParameterPopover value={{ instructions: "Be brief." }} onValueChange={() => undefined} />);
    await userEvent.click(screen.getByRole("button", { name: "Parameters" }));
    const dialog = await screen.findByRole("dialog", { name: "Parameters" });
    await vi.waitFor(() => expect(within(dialog).getByRole("textbox", { name: "System instructions" })).toHaveFocus());
    await userEvent.click(within(dialog).getByRole("button", { name: "Close parameters" }));
    await vi.waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });
});

describe("PromptSuggestions rows", () => {
  it("draws pill rows that grow for a coarse pointer, and keeps two identical prompts", async () => {
    const onSelect = vi.fn();
    render(<PromptSuggestions suggestions={["Explain our refund policy", "Explain our refund policy"]} onSelect={onSelect} />);
    const rows = screen.getAllByRole("button", { name: "Explain our refund policy" });
    expect(rows).toHaveLength(2);
    expect(rows[0]).toHaveClass("rounded-pill", "coarse:h-44", "focus-visible:text-fg", "active:bg-fill-2");
    await userEvent.click(rows[1]!);
    expect(onSelect).toHaveBeenCalledWith("Explain our refund policy");
  });
});
