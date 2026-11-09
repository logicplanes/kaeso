import { describe, expect, it, vi } from "vitest";
import { useState } from "react";
import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Composer } from "./composer.js";
import { CompareGrid } from "./compare-grid.js";
import { ResponseView, UserMessage } from "./response-view.js";
import { ResponseMetrics } from "./response-metrics.js";
import { Reasoning } from "./reasoning.js";
import { ParameterPanel } from "./parameter-panel.js";
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

  it("keeps every header the same, with Add at the end of the last column's header and no rail after it", () => {
    render(<Grid />);
    const headers = screen.getAllByRole("region", { name: /^(GPT-5|Claude Sonnet 4\.5)$/ }).map((r) => r.querySelector("header")!);
    expect(headers.map((h) => h.children.length)).toEqual([2, 3]);
    expect(within(headers[0]!).queryByRole("button", { name: "Add model" })).toBeNull();
    const add = within(headers[1]!).getByRole("button", { name: "Add model" });
    expect(add).toHaveClass("size-32", "material-ghost");
    expect(add.parentElement).toHaveClass("max-md:hidden");
    expect(document.querySelector(".w-56")).toBeNull();
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
  it("draws no caret while waiting or writing, only the text that has arrived", () => {
    const { container, rerender } = render(<ResponseView status="pending" />);
    const article = screen.getByRole("article", { name: "Response" });
    expect(article).toHaveAttribute("aria-busy", "true");
    expect(article.querySelectorAll(":scope > :not(.sr-only)")).toHaveLength(0);
    rerender(<ResponseView status="streaming" text="The p95 latency" />);
    expect(container.querySelector("p")?.textContent).toBe("The p95 latency");
    expect(container.querySelector("p")?.children).toHaveLength(0);
  });

  it("says once, by name, that a response finished, stopped or was cut off", () => {
    const { rerender } = render(<ResponseView aria-label="GPT-5 response" status="done" text="Earlier answer" />);
    const status = screen.getByRole("status");
    expect(status).toBeEmptyDOMElement();
    rerender(<ResponseView aria-label="GPT-5 response" status="streaming" text="Rate" />);
    expect(status).toBeEmptyDOMElement();
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
    expect(status).toBeEmptyDOMElement();
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

  it("sets text 12px in and leaves code and tables on the edge; the footer on the text line, after Copy and Retry", () => {
    const { container } = render(<ResponseView text={"Two did:\n\n```json\n{}\n```"} footer={<ResponseMetrics duration={1200} />} onRetry={() => undefined} />);
    expect(container.querySelector(".markdown-text-inset")).toBeInTheDocument();
    const metrics = screen.getByLabelText("Metrics").parentElement!;
    expect(metrics).toHaveClass("px-8");
    expect(metrics.parentElement).toHaveClass("min-h-32");
    expect(metrics.parentElement).not.toHaveClass("px-4");
    const row = metrics.parentElement!;
    expect(row).toHaveClass("pl-4");
    expect(within(row).getByRole("button", { name: "Copy response" })).toHaveClass("size-32", "text-fg-secondary");
    expect(within(row).getByRole("button", { name: "Retry" })).toHaveClass("material-ghost", "text-fg-secondary");
  });

  it("keeps the metrics on the text line when no Copy or Retry comes before them", () => {
    render(<ResponseView status="done" text="" footer={<ResponseMetrics duration={1200} />} />);
    expect(screen.getByLabelText("Metrics").parentElement).toHaveClass("px-12");
  });

  it("offers Copy and the icon Retry only under a finished reply with text", () => {
    const { rerender } = render(<ResponseView status="streaming" text="Half" onRetry={() => undefined} />);
    expect(screen.queryByRole("button", { name: "Copy response" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Retry" })).toBeNull();
    rerender(<ResponseView status="done" text="" onRetry={() => undefined} />);
    expect(screen.queryByRole("button", { name: "Copy response" })).toBeNull();
    expect(screen.getByRole("button", { name: "Retry" })).toHaveClass("material-ghost", "text-fg-secondary");
    rerender(<ResponseView status="error" error="Timed out." onRetry={() => undefined} />);
    expect(screen.getAllByRole("button", { name: "Retry" })).toHaveLength(1);
  });

  it("sets the prompt in a bubble at the end of the line, apart from the response that starts at the other edge", () => {
    render(<UserMessage>{"Why did errors spike?"}</UserMessage>);
    const group = screen.getByRole("group", { name: "Prompt" });
    expect(group).toHaveClass("items-end");
    expect(group.querySelector("p")).toHaveClass("bg-fill-1", "rounded-frame", "px-16", "py-10", "text-body-lg", "text-fg", "max-w-prose");
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
    const reserved = () => [...container.querySelectorAll("dd > [aria-hidden]")].map((s) => s.getAttribute("data-reserve"));
    const live = reserved();
    expect(live).toEqual(["8.88 s", "8.88 s", "8,888 in · 888 out", "888", "$0.008888"]);
    rerender(<ResponseMetrics reasoning timeToFirstToken={420} duration={3140} inputTokens={1204} outputTokens={856} reasoningTokens={312} cost="0.004213" />);
    expect(reserved()).toEqual(live);
    expect(screen.getByText("Tokens").nextElementSibling).toHaveTextContent(/^1,204 in · 856 out$/);
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
    render(<ParameterPanel value={{ instructions: "Be brief." }} onValueChange={() => undefined} trigger={<button type="button">Parameters</button>} />);
    await userEvent.click(screen.getByRole("button", { name: "Parameters" }));
    const dialog = await screen.findByRole("dialog", { name: "Parameters" });
    await vi.waitFor(() => expect(within(dialog).getByRole("textbox", { name: "System instructions" })).toHaveFocus());
    await userEvent.click(within(dialog).getByRole("button", { name: "Close parameters" }));
    await vi.waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });
});
