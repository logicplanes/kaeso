import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, within } from "@testing-library/react";
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
  it("fades long lines out at the edge it can still scroll to, and fades none when everything fits", () => {
    render(
      <CodeBlock code={'{ "error_message": "The server had an error while processing your request. Sorry about that!" }'} title="Metadata" copy={false} />,
    );
    const region = screen.getByRole("region", { name: "Metadata code" });
    expect(region).toHaveClass("overflow-x-auto", "scroll-fade-x", "py-16");
    expect(region.querySelector("code > span")).toHaveClass("px-16");
    expect(region.style.getPropertyValue("--scroll-area-overflow-x-end")).toBe("");
    Object.defineProperty(region, "scrollWidth", { configurable: true, value: 765 });
    Object.defineProperty(region, "clientWidth", { configurable: true, value: 271 });
    region.scrollLeft = 0;
    fireEvent.scroll(region);
    expect(region.style.getPropertyValue("--scroll-area-overflow-x-start")).toBe("");
    expect(region.style.getPropertyValue("--scroll-area-overflow-x-end")).toBe("494px");
    region.scrollLeft = 494;
    fireEvent.scroll(region);
    expect(region.style.getPropertyValue("--scroll-area-overflow-x-start")).toBe("494px");
    expect(region.style.getPropertyValue("--scroll-area-overflow-x-end")).toBe("");
  });

  it("highlights code once the highlighter loads", async () => {
    const { container } = render(<CodeBlock code={'const key = "sk-kso";\n'} title="client.ts" lineNumbers highlightLines={[1]} />);
    expect(screen.getByText("client.ts")).toBeInTheDocument();
    expect(container.querySelector(".bg-fill-2")).not.toBeNull();
    expect(screen.getByText('const key = "sk-kso";')).toBeInTheDocument();
    await highlighted();
    expect(screen.getByText('"sk-kso"')).toHaveClass("text-info-text");
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

  it("hangs a wrapped line from its own indent, and copies the code as written", () => {
    const code = '{\n    "message": "You need the right `models.manage` in this project; ask one of its admins for it."\n}';
    const { container } = render(<CodeBlock code={code} lang="text" wrap copy={false} />);
    const row = container.querySelectorAll("code > span")[1] as HTMLElement;
    const [indent, body] = [...row.children] as HTMLElement[];
    expect(indent).toHaveClass("shrink-0", "whitespace-pre");
    expect(indent?.textContent).toBe("    ");
    expect(body).toHaveClass("pl-16", "-indent-16");
    expect(body?.textContent?.startsWith('"message"')).toBe(true);
    expect(row.textContent).toBe(code.split("\n")[1]);
  });

  it("wraps long lines on phones or at every width when asked, indenting the part that wraps", () => {
    const line = 'curl https://kaeso.example.com/v1/responses -H "Authorization: Bearer $KAESO_API_KEY"';
    const { container, rerender } = render(<CodeBlock code={line} lang="text" wrap="phone" copy={false} />);
    const row = () => container.querySelector("code > span") as HTMLElement;
    expect(container.querySelector("code")).toHaveClass("sm:min-w-max");
    expect(container.querySelector("code")).not.toHaveClass("min-w-max");
    expect(row()).toHaveClass("max-sm:whitespace-pre-wrap", "max-sm:wrap-anywhere");
    expect(row().lastElementChild).toHaveClass("max-sm:pl-16", "max-sm:-indent-16");
    rerender(<CodeBlock code={line} lang="text" wrap copy={false} />);
    expect(row()).toHaveClass("whitespace-pre-wrap");
    expect(row()).not.toHaveClass("max-sm:whitespace-pre-wrap");
    expect(row().lastElementChild).toHaveClass("pl-16", "-indent-16");
    rerender(<CodeBlock code={line} lang="text" copy={false} />);
    expect(container.querySelector("code")).toHaveClass("min-w-max");
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
