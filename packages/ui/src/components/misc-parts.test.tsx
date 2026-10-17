import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { DeleteResource } from "../blocks/delete-resource/delete-resource.js";
import { EmptyPage, ListPage, OverviewPage, TablePage } from "../blocks/pages/pages.js";
import { AppFrame } from "./app-frame/app-frame.js";
import { Label } from "./label/label.js";
import { Heading } from "./heading/heading.js";
import { Timeline } from "./timeline/timeline.js";
import { ScrollArea } from "./scroll-area/scroll-area.js";
import { Skeleton, SkeletonLine } from "./skeleton/skeleton.js";
import { ConversationView } from "./kaeso/conversation.js";
import { devWarn, warnControlled, warnMissingName, warnStylingClassName } from "../utils/dev.js";
import { resolveVariant } from "../utils/variants.js";

afterEach(() => vi.restoreAllMocks());

describe("DeleteResource", () => {
  it("asks for the name before deleting", async () => {
    const onConfirm = vi.fn();
    render(<DeleteResource inline kind="key" name="production" consequence="Apps that use it stop working." onConfirm={onConfirm} />);
    expect(screen.getByRole("heading", { name: "Delete this key?" })).toBeInTheDocument();
    const confirm = screen.getByRole("button", { name: "Delete key" });
    expect(confirm).toBeDisabled();
    await userEvent.type(screen.getByRole("textbox"), "production");
    await userEvent.click(confirm);
    expect(onConfirm).toHaveBeenCalled();
  });

  it("uses its own verb and a simple confirm, controlled", async () => {
    const onOpenChange = vi.fn();
    render(
      <DeleteResource
        open
        onOpenChange={onOpenChange}
        kind="project"
        name="Checkout"
        verb="Archive"
        confirm="simple"
        consequence="It can be restored."
        onConfirm={vi.fn()}
        trigger={<button>Archive</button>}
      />,
    );
    expect(await screen.findByRole("alertdialog", { name: "Archive this project?" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Archive project" })).toBeEnabled();
    expect(screen.queryByRole("textbox")).toBeNull();
  });
});

describe("page blocks", () => {
  it("compose a list, an overview with an aside, and an empty page", () => {
    render(
      <AppFrame sidebar={<nav aria-label="Main" />}>
        <ListPage header={{ title: "API keys" }} toolbar={<div>Toolbar</div>} sheet={<div>Sheet</div>}>
          <p>Rows</p>
        </ListPage>
        <TablePage header={{ title: "Logs" }}>
          <p>Table</p>
        </TablePage>
        <OverviewPage header={{ title: "Usage" }} metrics={<p>Metrics</p>} controls={<button>Period</button>} chart={<p>Chart</p>} aside={<p>Aside</p>}>
          <p>More</p>
        </OverviewPage>
        <OverviewPage header={{ title: "Plain" }} metrics={<p>Only metrics</p>} />
        <EmptyPage header={{ title: "Alerts" }} title="No alerts yet" />
        <EmptyPage title="Nothing" />
      </AppFrame>,
    );
    for (const text of ["API keys", "Toolbar", "Sheet", "Table", "Metrics", "Period", "Aside", "Only metrics", "No alerts yet", "Nothing"]) {
      expect(screen.getAllByText(text).length).toBeGreaterThan(0);
    }
  });
});

describe("small parts", () => {
  it("label, heading, timeline, scroll area and skeletons", () => {
    render(
      <>
        <Label htmlFor="x" optional size="sm">
          Name
        </Label>
        <Heading size="display" level={1} balance>
          Kaeso
        </Heading>
        <Heading>Default level</Heading>
        <Timeline
          events={[
            { id: "a", title: "Key created", time: "09:00", datetime: "2026-10-15T09:00:00Z", tone: "success", description: "By Ada" },
            { id: "b", title: "Key rotated", time: "10:00", marker: <span>R</span> },
          ]}
        />
        <ScrollArea orientation="both" aria-label="Logs" viewportClassName="h-200">
          <p>Lines</p>
        </ScrollArea>
        <ScrollArea orientation="horizontal">
          <p>Wide</p>
        </ScrollArea>
        <Skeleton shape="circle" />
        <SkeletonLine lines={3} data-testid="lines" />
        <SkeletonLine data-testid="one" />
      </>,
    );
    expect(screen.getByText("Name")).toHaveTextContent("Optional");
    expect(screen.getByRole("heading", { level: 1, name: "Kaeso" })).toHaveClass("text-balance");
    expect(screen.getByRole("heading", { name: "Default level" }).tagName).toMatch(/^H\d$/);
    expect(screen.getByText("09:00")).toHaveAttribute("dateTime", "2026-10-15T09:00:00Z");
    expect(screen.getByText("R")).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Logs" })).toHaveTextContent("Lines");
    expect(screen.getByText("Wide").closest("[role=region]")).toBeNull();
    expect(screen.getByTestId("lines").children).toHaveLength(3);
    expect(screen.getByTestId("lines").lastElementChild).toHaveClass("w-3/5");
    expect(screen.getByTestId("one").firstElementChild).toHaveClass("w-full");
  });
});

describe("ConversationView blocks", () => {
  it("shows every kind of block", async () => {
    render(
      <ConversationView
        markdown
        messages={[
          { role: "system", content: "You are the checkout assistant." },
          {
            role: "assistant",
            content: [
              { type: "reasoning", text: "Thinking about **refunds**." },
              { type: "reasoning", text: "", redacted: true },
              { type: "tool_call", id: "call_1", name: "lookup_order", arguments: { id: "o_42" } },
              { type: "tool_result", tool_call_id: "call_1", content: '{"status":"shipped"}' },
              { type: "tool_result", content: "Plain **markdown** result", is_error: true },
              { type: "tool_result", content: [{ type: "text", text: "Nested" }] },
              { type: "image", src: "https://kaeso.app/receipt.png", alt: "Receipt" },
              { type: "image", src: "data:image/png;base64,AAAA" },
              { type: "image", media_type: "image/png" },
              { type: "image", src: "javascript:alert(1)" },
              { type: "document", name: "invoice.pdf" },
              { type: "document" },
              { type: "refusal", text: "I can't share card numbers." },
            ],
          },
        ]}
      />,
    );
    expect(screen.getByText("lookup_order")).toBeInTheDocument();
    expect(screen.getByText('{"status":"shipped"}')).toBeInTheDocument();
    expect(screen.getByText("Error")).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Receipt" })).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Image input" })).toBeInTheDocument();
    expect(screen.getByText("invoice.pdf")).toBeInTheDocument();
    expect(screen.getByText("Document")).toBeInTheDocument();
    expect(screen.getByText("The model refused.")).toBeInTheDocument();
    expect(screen.getByText("Redacted by the provider")).toBeInTheDocument();
    await waitFor(() => expect(screen.getByText("Nested")).toBeInTheDocument());
  });

  it("shows tool results as text without markdown", () => {
    render(<ConversationView messages={[{ role: "tool", content: [{ type: "tool_result", content: "not json [" }] }]} />);
    expect(screen.getByText("not json [").tagName).toBe("PRE");
  });
});

describe("developer warnings", () => {
  it("warns once per message about styling, names and controlled values", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    warnStylingClassName("Button", "mt-8 bg-red-500 rounded-lg");
    warnStylingClassName("Button", "mt-8 w-full");
    warnStylingClassName("Button", undefined);
    warnMissingName("IconButton", false);
    warnMissingName("IconButton", true);
    warnControlled("Switch", "checked", true, undefined);
    warnControlled("Switch", "checked", true, () => undefined);
    devWarn("same message");
    devWarn("same message");
    expect(warn.mock.calls.map((c) => c[0])).toEqual([
      expect.stringContaining("Remove bg-red-500, rounded-lg"),
      expect.stringContaining("an icon-only control needs an aria-label"),
      expect.stringContaining("Use `defaultChecked`"),
      "[@logicplanes/kaeso-ui] same message",
    ]);
  });

  it("resolves a variant with a fallback", () => {
    const axis = { sm: { classes: "h-32", description: "" }, md: { classes: "h-36", description: "" } };
    expect(resolveVariant(axis, "sm", "md")).toBe("h-32");
    expect(resolveVariant(axis, undefined, "md")).toBe("h-36");
    expect(resolveVariant(axis, "xl" as "sm", "md")).toBe("h-36");
    expect(resolveVariant({} as typeof axis, "sm", "md")).toBe("");
  });
});
