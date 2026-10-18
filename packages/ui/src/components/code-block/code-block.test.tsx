import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { CodeBlock } from "./code-block.js";

const highlights = vi.hoisted(() => new Set<Promise<unknown>>());
vi.mock("./highlighter.js", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./highlighter.js")>();
  return {
    ...actual,
    highlight: (...args: Parameters<typeof actual.highlight>) => {
      const result = actual.highlight(...args);
      highlights.add(result);
      return result;
    },
  };
});
const highlighted = () =>
  act(async () => {
    await Promise.all(highlights);
  });

beforeEach(() => vi.useFakeTimers({ toFake: ["Date"] }));
afterEach(() => vi.useRealTimers());

const long = Array.from({ length: 30 }, (_, i) => `const line${i} = ${i};`).join("\n");

describe("CodeBlock", () => {
  it("highlights code once the highlighter loads", async () => {
    const { container } = render(<CodeBlock code={'const key = "sk-kaeso";\n'} title="client.ts" lineNumbers highlightLines={[1]} />);
    expect(screen.getByText("client.ts")).toBeInTheDocument();
    expect(container.querySelector(".bg-fill-2")).not.toBeNull();
    expect(screen.getByText('const key = "sk-kaeso";')).toBeInTheDocument();
    await highlighted();
    expect(screen.getByText('"sk-kaeso"')).toHaveClass("text-info-text");
    expect(screen.getByText("const")).toHaveClass("font-semibold");
    expect(container.querySelector("[style]")).toBeNull();
  });

  it("switches between tabs", async () => {
    const { container } = render(
      <CodeBlock
        tabs={[
          { label: "Python", code: "print('hi')", lang: "python" },
          { label: "curl", code: "curl https://kaeso.app", lang: "bash" },
        ]}
      />,
    );
    await userEvent.click(screen.getByRole("tab", { name: "curl" }));
    expect(container.querySelector("code")).toHaveTextContent("curl https://kaeso.app");
    await highlighted();
    expect(container.querySelector("code")).toHaveTextContent("curl https://kaeso.app");
    expect(within(screen.getByRole("tabpanel", { name: "curl" })).getByText("curl")).not.toHaveTextContent("https");
  });

  it("opens on the tab it is told, reports a choice, and keeps its own when it lacks the label", async () => {
    const tabs = [
      { label: "curl", code: "curl https://kaeso.example.com", lang: "bash" as const },
      { label: "Python", code: "print('hi')", lang: "python" as const },
    ];
    const chosen: string[] = [];
    const { rerender } = render(<CodeBlock tabs={tabs} tab="Python" onTabChange={(label) => chosen.push(label)} />);
    expect(screen.getByRole("tab", { name: "Python" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tabpanel", { name: "Python" })).toHaveTextContent("print('hi')");
    await userEvent.click(screen.getByRole("tab", { name: "curl" }));
    expect(chosen).toEqual(["curl"]);
    rerender(<CodeBlock tabs={tabs} tab="curl" onTabChange={(label) => chosen.push(label)} />);
    expect(screen.getByRole("tabpanel", { name: "curl" })).toHaveTextContent("curl https://kaeso.example.com");
    rerender(<CodeBlock tabs={tabs} tab="TypeScript" />);
    expect(screen.getByRole("tab", { name: "curl" })).toHaveAttribute("aria-selected", "true");
  });

  it("moves between tabs with the arrow keys and names the code by its tab", async () => {
    render(
      <CodeBlock
        tabs={[
          { label: "Python", code: "print('hi')", lang: "python" },
          { label: "curl", code: "curl https://kaeso.app", lang: "bash" },
        ]}
      />,
    );
    const python = screen.getByRole("tab", { name: "Python" });
    const curl = screen.getByRole("tab", { name: "curl" });
    expect(python).toHaveAttribute("tabindex", "0");
    expect(curl).toHaveAttribute("tabindex", "-1");
    expect(screen.getByRole("tabpanel", { name: "Python" })).toHaveTextContent("print('hi')");
    expect(python).toHaveAttribute("aria-controls", screen.getByRole("tabpanel").id);
    python.focus();
    await userEvent.keyboard("{ArrowRight}");
    expect(curl).toHaveFocus();
    await userEvent.keyboard("{Enter}");
    expect(curl).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tabpanel", { name: "curl" })).toHaveTextContent("curl https://kaeso.app");
  });

  it("collapses long code and expands on request", async () => {
    render(<CodeBlock code={long} lang="text" wrap surface="plain" copy={false} />);
    expect(screen.queryByText("const line29 = 29;")).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Show all 30 lines" }));
    expect(screen.getByText("const line29 = 29;")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Show less" }));
    expect(screen.queryByText("const line29 = 29;")).toBeNull();
  });

  it("keeps the code clear of the copy button on touch screens, where the button always shows", () => {
    const { rerender } = render(<CodeBlock code="echo KAESO_MAIL_TRANSPORT=log >> .env" lang="bash" />);
    expect(screen.getByRole("button", { name: "Copy code" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Code" })).toHaveClass("coarse:mr-40");
    rerender(<CodeBlock code="echo KAESO_MAIL_TRANSPORT=log >> .env" lang="bash" copy={false} />);
    expect(screen.queryByRole("button", { name: "Copy code" })).toBeNull();
    expect(screen.getByRole("region", { name: "Code" })).not.toHaveClass("coarse:mr-40");
  });

  it("keeps empty lines and shows everything when collapsing is off", () => {
    render(<CodeBlock code={"a\n\nb"} lang="text" collapseAfter={0} />);
    expect(screen.getByText("a")).toBeInTheDocument();
    expect(screen.getByText("b")).toBeInTheDocument();
  });
});
