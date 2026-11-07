import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { CellInset, Dashboard, DashboardCell, Readout } from "./dashboard.js";
import { Legend } from "./legend.js";
import { RankList } from "./rank-list.js";
import { MetricTabs } from "./metric-tabs.js";
import { Delta } from "../stat/delta.js";

const tabs = [
  { id: "requests", label: "Requests", value: "59,415", change: { value: 0.17 }, detail: "vs. prior 30 days" },
  { id: "tokens", label: "Tokens", value: "137.1M" },
  { id: "errors", label: "Error rate", value: "0.7%", change: { value: 0.23, better: "down" as const }, detail: "412 failed" },
];

describe("MetricTabs", () => {
  it("says each total and its change, and marks the chosen one", () => {
    render(<MetricTabs aria-label="Metric to chart" items={tabs} value="requests" onValueChange={() => {}} />);
    const requests = screen.getByRole("tab", { name: /Requests/ });
    expect(requests).toHaveAttribute("aria-selected", "true");
    expect(requests).toHaveTextContent("59,415↑17% bettervs. prior 30 days");
    expect(screen.getByRole("tab", { name: /Error rate/ })).toHaveTextContent("↑23% worse");
  });
  it("moves between totals with the arrow keys, Home and End", () => {
    const pick = vi.fn();
    render(<MetricTabs aria-label="Metric to chart" items={tabs} value="requests" onValueChange={pick} />);
    const first = screen.getByRole("tab", { name: /Requests/ });
    fireEvent.keyDown(first, { key: "ArrowRight" });
    expect(pick).toHaveBeenLastCalledWith("tokens");
    fireEvent.keyDown(first, { key: "ArrowLeft" });
    expect(pick).toHaveBeenLastCalledWith("errors");
    fireEvent.keyDown(first, { key: "End" });
    expect(pick).toHaveBeenLastCalledWith("errors");
    fireEvent.keyDown(first, { key: "Home" });
    expect(pick).toHaveBeenLastCalledWith("requests");
    fireEvent.keyDown(first, { key: "a" });
    expect(pick).toHaveBeenCalledTimes(4);
    fireEvent.click(screen.getByRole("tab", { name: /Tokens/ }));
    expect(pick).toHaveBeenLastCalledWith("tokens");
  });
  it("widens the last tab of an odd row on narrow screens, and keeps every tab's height while loading", () => {
    render(<MetricTabs aria-label="Metric to chart" items={tabs} value="tokens" onValueChange={() => {}} loading />);
    const all = screen.getAllByRole("tab");
    expect(all.at(-1)).toHaveClass("col-span-2", "lg:col-span-1");
    expect(all[0]).toHaveAttribute("aria-busy", "true");
    expect(all[0]).not.toHaveTextContent("59,415");
  });
});

describe("MetricTabs, stacked", () => {
  const trend = [
    {
      id: "requests",
      label: "Requests",
      value: "59,415",
      change: { value: 0.17 },
      detail: "vs. prior 30 days",
      trend: [1, 3, 2],
      trendLabel: "Requests per day",
      trendColor: "var(--k-viz-1)",
    },
    { id: "tokens", label: "Tokens", value: "137.1M", trend: [4], trendLabel: "Tokens per day" },
    { id: "errors", label: "Error rate", value: <b>0.7%</b>, change: { value: -0.2, better: "down" as const } },
  ];
  it("stacks each total with its trend and moves with the up and down keys", () => {
    const pick = vi.fn();
    render(<MetricTabs aria-label="Metric to chart" orientation="vertical" items={trend} value="tokens" onValueChange={pick} controls="chart" />);
    expect(screen.getByRole("tablist")).toHaveAttribute("aria-orientation", "vertical");
    const requests = screen.getByRole("tab", { name: /Requests/ });
    expect(requests).toHaveTextContent("Requests↑17% better59,415vs. prior 30 days");
    expect(screen.getByRole("img", { name: "Requests per day" })).toBeInTheDocument();
    expect(screen.queryByRole("img", { name: "Tokens per day" })).not.toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /Error rate/ })).toHaveTextContent("↓20% better");
    fireEvent.keyDown(requests, { key: "ArrowDown" });
    expect(pick).toHaveBeenLastCalledWith("tokens");
    fireEvent.keyDown(requests, { key: "ArrowUp" });
    expect(pick).toHaveBeenLastCalledWith("errors");
    fireEvent.keyDown(requests, { key: "ArrowRight" });
    expect(pick).toHaveBeenCalledTimes(2);
  });
  it("keeps each stacked total's place while it loads", () => {
    render(<MetricTabs aria-label="Metric to chart" orientation="vertical" items={trend} value="requests" onValueChange={() => {}} loading />);
    const requests = screen.getByRole("tab", { name: /Requests/ });
    expect(requests).toHaveAttribute("aria-busy", "true");
    expect(requests).not.toHaveTextContent("59,415");
    expect(screen.queryByRole("img", { name: "Requests per day" })).not.toBeInTheDocument();
  });
  it("draws a trend under a total laid out in a row", () => {
    render(<MetricTabs aria-label="Metric to chart" items={trend} value="requests" onValueChange={() => {}} />);
    expect(screen.getByRole("img", { name: "Requests per day" })).toBeInTheDocument();
  });
});

describe("Dashboard", () => {
  it("lays cells on one surface with their label and actions", () => {
    render(
      <Dashboard aria-label="Usage in the period">
        <DashboardCell title="Cache hit" actions={<button type="button">Request logs</button>}>
          <Readout value="48%" change={{ value: 0.005 }} detail="62.6M input tokens from cache" />
          <CellInset grow>
            <p>Chart</p>
          </CellInset>
        </DashboardCell>
        <DashboardCell>
          <CellInset>
            <p>Plain</p>
          </CellInset>
        </DashboardCell>
      </Dashboard>,
    );
    expect(screen.getByLabelText("Usage in the period")).toHaveClass("bg-line", "rounded-group");
    expect(screen.getByRole("heading", { name: "Cache hit" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Request logs" })).toBeInTheDocument();
    expect(screen.getByText("48%").parentElement).toHaveClass("text-metric");
    expect(screen.getByText("↑0.5%").parentElement?.parentElement).toHaveTextContent("↑0.5% better");
  });
  it("keeps a readout's height while it loads, and sets a small one under its label", () => {
    const { rerender } = render(<Readout value="$60.45" loading />);
    expect(screen.queryByText("$60.45")).not.toBeInTheDocument();
    rerender(<Readout size="sm" label="November so far" value="$13.39" unit="USD" />);
    expect(screen.getByText("$13.39").parentElement).toHaveClass("text-title");
    expect(screen.getByText("November so far")).toBeInTheDocument();
    expect(screen.getByText("USD")).toBeInTheDocument();
  });
});

describe("Dashboard headings", () => {
  it("titles its cells one level below the heading above it", () => {
    const { rerender } = render(
      <Dashboard aria-label="Usage">
        <DashboardCell title="Cache hit" />
      </Dashboard>,
    );
    expect(screen.getByRole("heading", { name: "Cache hit", level: 3 })).toBeInTheDocument();
    rerender(
      <Dashboard aria-label="Usage" headingLevel={2}>
        <DashboardCell title="Cache hit" href="/analytics" />
      </Dashboard>,
    );
    expect(screen.getByRole("heading", { name: "Cache hit", level: 2 })).toBeInTheDocument();
  });
});

describe("Readout", () => {
  it("rolls to a new total in place", () => {
    const { rerender } = render(<Readout label="Requests" value="1,204" />);
    rerender(<Readout label="Requests" value="1,205" />);
    expect(screen.getByText("Requests").parentElement).toHaveTextContent("1,205");
  });
});

describe("RankList", () => {
  const items = [
    { id: "a", label: "Claude Haiku 5.5", value: "31,629", secondary: "53%", share: 0.53 },
    { id: "b", label: "GPT-5.4 mini", value: "4,655", secondary: "8%", share: 0.08 },
  ];
  it("draws each row's share of the largest behind it and opens a row", () => {
    const open = vi.fn();
    render(<RankList aria-label="Models by requests" items={items} onOpen={open} />);
    const rows = screen.getAllByRole("button");
    expect(rows[0]).toHaveTextContent("Claude Haiku 5.553%31,629");
    const bars = screen.getByRole("list", { name: "Models by requests" }).querySelectorAll(".width-motion");
    expect((bars[0] as HTMLElement).style.width).toBe("100%");
    expect(Number.parseFloat((bars[1] as HTMLElement).style.width)).toBeCloseTo(15.09, 1);
    fireEvent.click(rows[1]!);
    expect(open).toHaveBeenCalledWith("b");
  });
  it("shows plain rows without bars, its loading shape and its empty state", () => {
    const { rerender } = render(<RankList aria-label="Recent errors" items={[{ id: "x", label: "Provider overloaded", secondary: "1 h ago" }]} />);
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
    expect(screen.getByRole("list").querySelector("[aria-hidden]")).toBeNull();
    rerender(<RankList aria-label="Recent errors" items={[]} loading rows={3} />);
    expect(screen.getByRole("list")).toHaveAttribute("aria-busy", "true");
    expect(screen.getAllByRole("listitem")).toHaveLength(3);
    rerender(<RankList aria-label="Recent errors" items={[]} empty="Nothing in this period" />);
    expect(screen.getByText("Nothing in this period")).toBeInTheDocument();
  });
});

describe("Legend and Delta", () => {
  it("says each series' total and draws the comparison dashed", () => {
    const { container } = render(
      <Legend
        items={[
          { id: "now", label: "This period", value: "59,415", color: "var(--k-viz-1)" },
          { id: "before", label: "vs. prior 30 days", value: "50,819", color: "var(--k-text-tertiary)", dashed: true },
        ]}
      />,
    );
    expect(screen.getAllByRole("listitem")[1]).toHaveTextContent("vs. prior 30 days50,819");
    expect(container.querySelectorAll("line")[1]).toHaveAttribute("stroke-dasharray", "3 3");
  });
  it("writes no change in the secondary color and says nothing about better or worse", () => {
    render(<Delta value={0} />);
    const delta = screen.getByText("0%").parentElement?.parentElement;
    expect(delta).toHaveClass("text-fg-secondary");
    expect(delta).toHaveTextContent(/^0%$/);
  });
});
