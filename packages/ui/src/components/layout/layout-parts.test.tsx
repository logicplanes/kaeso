import { afterEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Grid, Inline, Stack } from "./layout";
import { AuthLayout, Section, SplitView } from "../section/section";
import { Meter, Progress, meterColor } from "../meter/meter";
import { ButtonGroup } from "../button-group/button-group";
import { Pagination } from "../pagination/pagination";
import { Toolbar } from "../toolbar/toolbar";
import { Portal } from "../portal/portal";
import { StatusDot } from "../status-dot/status-dot";
import { Logo } from "../logo/logo";
import { TableOfContents } from "../toc/toc";
import { ThemeMenu } from "../theme-menu/theme-menu";
import { ThemeProvider } from "../../theme/theme-provider";
import { icons } from "../../icons/vocabulary";

afterEach(() => vi.restoreAllMocks());

describe("Stack, Inline and Grid", () => {
  it("render as the element asked for, with gaps, alignment and columns", () => {
    render(
      <>
        <Stack as="section" gap={24} align="center" justify="between" data-testid="stack" />
        <Inline as="ul" wrap gap={8} align="baseline" data-testid="inline" />
        <Grid as="ol" columns={4} gap={32} align="start" data-testid="grid" />
        <Stack data-testid="plain" />
      </>,
    );
    expect(screen.getByTestId("stack").tagName).toBe("SECTION");
    expect(screen.getByTestId("stack")).toHaveClass("gap-24", "items-center", "justify-between");
    expect(screen.getByTestId("inline").tagName).toBe("UL");
    expect(screen.getByTestId("inline")).toHaveClass("flex-wrap");
    expect(screen.getByTestId("grid")).toHaveClass("lg:grid-cols-4", "gap-32");
    expect(screen.getByTestId("plain").tagName).toBe("DIV");
  });
});

describe("Section, SplitView and AuthLayout", () => {
  it("head a section, put an aside beside content and frame sign-in", () => {
    render(
      <>
        <Section title="Members" description="People in Northwind." actions={<button>Invite</button>} spacing="none">
          <p>Rows</p>
        </Section>
        <Section>
          <p>Untitled</p>
        </Section>
        <SplitView aside={<p>Aside</p>}>
          <p>Main</p>
        </SplitView>
        <AuthLayout title="Sign in to Kaeso" description="Use your work email." footer={<span>Terms</span>}>
          <input aria-label="Email" />
        </AuthLayout>
      </>,
    );
    expect(screen.getByRole("heading", { name: "Members" })).toBeInTheDocument();
    expect(screen.getByRole("complementary")).toHaveTextContent("Aside");
    expect(screen.getByRole("heading", { level: 1, name: "Sign in to Kaeso" })).toBeInTheDocument();
    expect(screen.getByText("Terms")).toBeInTheDocument();
  });
});

describe("Meter and Progress", () => {
  it("colors by thresholds and marks them", () => {
    expect(meterColor(1.2)).toBe("bg-danger");
    expect(meterColor(0.85)).toBe("bg-warning");
    expect(meterColor(0.5, [0.4, 0.9])).toBe("bg-warning");
    expect(meterColor(0.3, [0.4])).toBe("bg-viz-1");
    const { container } = render(
      <Meter value={0.6} label="Budget" valueLabel="$300.00" thresholds={[0.5, 0.9]} caption="of $500.00" leading={<span>L</span>} trailing={<span>T</span>} />,
    );
    const meter = screen.getByRole("meter", { name: "Budget" });
    expect(meter).toHaveAttribute("aria-valuetext", "$300.00");
    expect(container.querySelectorAll("span[aria-hidden].bg-panel")).toHaveLength(2);
    render(<Meter value={2} tone="neutral" aria-label="Share" />);
    expect(screen.getByRole("meter", { name: "Share" })).toHaveAttribute("aria-valuenow", "200");
    expect(screen.getByRole("meter", { name: "Share" })).toHaveAttribute("aria-valuemax", "200");
  });

  it("shows a known amount or an indeterminate line", () => {
    render(<Progress aria-label="Upload" value={0.42} />);
    expect(screen.getByRole("progressbar", { name: "Upload" })).toHaveAttribute("aria-valuenow", "42");
    render(<Progress aria-label="Loading" />);
    expect(screen.getByRole("progressbar", { name: "Loading" })).not.toHaveAttribute("aria-valuenow");
  });
});

describe("ButtonGroup", () => {
  it("joins buttons, names icon-only ones by tooltip, and spaces them apart", async () => {
    const onClick = vi.fn();
    render(
      <>
        <ButtonGroup aria-label="Align" size="sm">
          <ButtonGroup.Item icon={icons.list} aria-label="List" pressed onClick={onClick} />
          <ButtonGroup.Item icon={icons.board}>Board</ButtonGroup.Item>
        </ButtonGroup>
        <ButtonGroup aria-label="Spaced" attached={false} size="lg">
          <ButtonGroup.Item>One</ButtonGroup.Item>
        </ButtonGroup>
      </>,
    );
    await userEvent.click(screen.getByRole("button", { name: "List" }));
    expect(onClick).toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "List" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("group", { name: "Spaced" })).toHaveClass("gap-8");
  });
});

describe("Pagination", () => {
  it("loads more while there is more, and then disappears", async () => {
    const onLoadMore = vi.fn();
    const { container, rerender } = render(<Pagination hasMore onLoadMore={onLoadMore} />);
    await userEvent.click(screen.getByRole("button", { name: "Load more" }));
    expect(onLoadMore).toHaveBeenCalled();
    rerender(<Pagination hasMore={false} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("goes back and forth by cursor", async () => {
    const onNext = vi.fn();
    const onPrevious = vi.fn();
    render(<Pagination variant="cursor" hasMore hasPrevious onNext={onNext} onPrevious={onPrevious} />);
    await userEvent.click(screen.getByRole("button", { name: "Next page" }));
    await userEvent.click(screen.getByRole("button", { name: "Previous page" }));
    expect(onNext).toHaveBeenCalled();
    expect(onPrevious).toHaveBeenCalled();
  });

  it("shows the first, last and nearby page numbers with gaps between", async () => {
    const onPageChange = vi.fn();
    render(<Pagination variant="pages" page={5} pageCount={10} onPageChange={onPageChange} />);
    const nav = screen.getByRole("navigation", { name: "Pagination" });
    expect(nav).toHaveTextContent("1…456…10");
    expect(screen.getByRole("button", { name: "5" })).toHaveAttribute("aria-current", "page");
    await userEvent.click(screen.getByRole("button", { name: "6" }));
    await userEvent.click(screen.getByRole("button", { name: "Next page" }));
    await userEvent.click(screen.getByRole("button", { name: "Previous page" }));
    expect(onPageChange.mock.calls).toEqual([[6], [6], [4]]);
  });
});

describe("Toolbar", () => {
  it("moves focus between controls with the arrows and passes keys on", async () => {
    const onKeyDown = vi.fn();
    render(
      <Toolbar aria-label="Filters" size="inset" onKeyDown={onKeyDown}>
        <button>Status</button>
        <Toolbar.Spacer />
        <button>Model</button>
      </Toolbar>,
    );
    screen.getByRole("button", { name: "Status" }).focus();
    fireEvent.keyDown(screen.getByRole("button", { name: "Status" }), { key: "ArrowRight" });
    expect(screen.getByRole("button", { name: "Model" })).toHaveFocus();
    expect(onKeyDown).toHaveBeenCalled();
  });
});

describe("Portal", () => {
  it("renders into the body, or into a given container", () => {
    const target = document.createElement("div");
    document.body.append(target);
    render(
      <>
        <Portal>
          <p>In body</p>
        </Portal>
        <Portal container={target}>
          <p>In target</p>
        </Portal>
      </>,
    );
    expect(screen.getByText("In body").parentElement).toBe(document.body);
    expect(target).toHaveTextContent("In target");
    target.remove();
  });
});

describe("StatusDot", () => {
  it("warns when it has no word", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    render(<StatusDot tone="success" />);
    expect(warn).toHaveBeenCalledWith(expect.stringContaining("StatusDot"));
    render(<StatusDot tone="danger">Failed</StatusDot>);
    expect(screen.getByText("Failed")).toBeInTheDocument();
  });
});

describe("Logo", () => {
  it("draws every form, named or decorative", () => {
    render(
      <>
        <Logo />
        <Logo.Mark size="xl" title="Kaeso mark" />
        <Logo.Icon size="sm" decorative />
        <Logo.Lockup size="lg" title="Kaeso lockup" />
        <Logo.Wordmark size="md" className="text-fg" style={{ opacity: 1 }} title="Wordmark" />
      </>,
    );
    expect(screen.getByRole("img", { name: "Kaeso mark" })).toHaveClass("h-48");
    expect(screen.getByRole("img", { name: "Kaeso lockup" })).toHaveClass("h-40");
    expect(screen.getByRole("img", { name: "Wordmark" })).toBeInTheDocument();
    expect(screen.getAllByRole("img").length).toBe(4);
  });
});

describe("TableOfContents", () => {
  it("marks the heading in view as it scrolls, or the one given", () => {
    let callback: (entries: { isIntersecting: boolean; target: Element }[]) => void = () => undefined;
    const disconnect = vi.fn();
    vi.stubGlobal(
      "IntersectionObserver",
      class {
        constructor(cb: typeof callback) {
          callback = cb;
        }
        observe() {}
        disconnect = disconnect;
      },
    );
    document.body.insertAdjacentHTML("beforeend", '<h2 id="setup">Setup</h2><h3 id="keys">Keys</h3>');
    const items = [
      { id: "setup", label: "Setup" },
      { id: "keys", label: "Keys", depth: 1 as const },
      { id: "missing", label: "Missing" },
    ];
    const { unmount, rerender } = render(<TableOfContents items={items} />);
    act(() =>
      callback([
        { isIntersecting: false, target: document.getElementById("setup")! },
        { isIntersecting: true, target: document.getElementById("keys")! },
      ]),
    );
    expect(screen.getByRole("link", { name: "Keys" })).toHaveAttribute("aria-current", "location");
    rerender(<TableOfContents items={items} active="setup" title="Contents" />);
    expect(screen.getByRole("link", { name: "Setup" })).toHaveAttribute("aria-current", "location");
    unmount();
    expect(disconnect).toHaveBeenCalled();
    document.getElementById("setup")?.remove();
    document.getElementById("keys")?.remove();
    vi.unstubAllGlobals();
  });
});

describe("ThemeMenu", () => {
  it("chooses a theme", async () => {
    render(
      <ThemeProvider>
        <ThemeMenu />
      </ThemeProvider>,
    );
    await userEvent.click(screen.getByRole("button"));
    await userEvent.click(await screen.findByRole("menuitemradio", { name: "Dark" }));
    expect(document.documentElement).toHaveAttribute("data-theme", "dark");
    document.documentElement.removeAttribute("data-theme");
    localStorage.clear();
  });
});
