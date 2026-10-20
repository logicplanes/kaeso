import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { setFormatLocale } from "@logicplanes/kaeso-ui/format";
import { apply, load } from "~/i18n";
import { usage } from "~/test/fixtures/shell";
import { renderUi } from "~/test/render";
import { CostShare } from "./cost-share";
import { CostChart, RequestsChart, TokensChart, UsageMetrics, periodUSD, sumUSD, tileUSD, totals } from "./usage-panels";

const twoDays = [usage({ key: "2026-10-13", requests: 40, errors: 0, cost: "0.5" }), usage()];
const tile = (label: string) => screen.getAllByRole("listitem").find((li) => li.textContent?.startsWith(label)) as HTMLElement;
const rowsOf = (name: string) =>
  within(within(screen.getByRole("figure", { name })).getByRole("table"))
    .getAllByRole("row")
    .slice(1)
    .map((r) =>
      within(r)
        .getAllByRole("cell")
        .map((c) => c.textContent?.trim()),
    );

describe("usage sums", () => {
  it("adds dollar amounts without floating point drift", () => {
    expect(sumUSD(["0.1", "0.2", null, undefined])).toBe("0.3");
    expect(sumUSD(["1", "2"])).toBe("3.00");
    expect(sumUSD([])).toBe("0.00");
  });

  it("shows a period's cost in cents, and tiny costs as under a cent", () => {
    expect(periodUSD(null)).toBe("$0.00");
    expect(periodUSD("0.000")).toBe("$0.00");
    expect(periodUSD("0.001")).toBe("<$0.01");
    expect(periodUSD("12.345")).toBe("$12.35");
  });

  it("says under a cent in a whole phrase in German and Chinese", async () => {
    apply(await load("de"));
    setFormatLocale("de");
    expect(periodUSD("0.001")).toBe("unter 0,01\u00a0$");
    apply(await load("zh-CN"));
    setFormatLocale("zh-CN");
    expect(periodUSD("0.001")).toBe("低于 $0.01");
    setFormatLocale("en");
  });

  it("keeps two significant digits of a tile's sub-cent cost", () => {
    expect(tileUSD("0.004213")).toBe("$0.0042");
    expect(tileUSD("0")).toBe("$0.00");
    expect(tileUSD("3.5")).toBe("$3.50");
    expect(tileUSD("0.00999")).toBe("$0.01");
    expect(tileUSD("0.000099")).toBe("$0.000099");
    expect(tileUSD("0.0000001")).toBe("$0.0000001");
    expect(tileUSD("0.000000123")).toBe("$0.00000012");
  });

  it("totals requests, errors, tokens and cost", () => {
    expect(totals(twoDays)).toEqual({ requests: 140, errors: 5, tokens: 3500, cost: "1.75" });
    expect(totals([usage({ cost: null })]).cost).toBeNull();
  });
});

describe("UsageMetrics", () => {
  it("compares the period with the previous one", async () => {
    await renderUi(
      <UsageMetrics
        rows={twoDays}
        previous={[usage({ requests: 70, errors: 7, cost: "0.5" })]}
        from="2026-10-13"
        to="2026-10-14"
        compare="vs last week"
        showCost
      />,
    );
    expect(screen.getByRole("list", { name: "Usage in the period" })).toBeInTheDocument();
    expect(tile("Requests")).toHaveTextContent("140");
    expect(tile("Requests")).toHaveTextContent("100%");
    expect(tile("Requests")).toHaveTextContent("vs last week");
    expect(tile("Tokens")).toHaveTextContent("3.5K");
    expect(tile("Cost")).toHaveTextContent("$1.75");
    expect(tile("Cost")).toHaveTextContent("250%");
    expect(tile("Errors")).toHaveTextContent("5");
    expect(tile("Errors")).toHaveTextContent(/of requests/);
  });

  it("says usage went up from none", async () => {
    await renderUi(<UsageMetrics rows={twoDays} previous={[]} from="2026-10-13" to="2026-10-14" compare="vs last week" showCost={false} />);
    expect(tile("Requests")).toHaveTextContent("Up from none");
    expect(screen.queryByText("Cost")).not.toBeInTheDocument();
    expect(tile("Errors")).not.toHaveTextContent("vs last week");
  });

  it("shows each tile's shape while loading, not a number that reads as no usage", async () => {
    await renderUi(<UsageMetrics rows={[]} from="2026-10-13" to="2026-10-14" showCost loading />);
    for (const name of ["Requests", "Cost", "Errors"]) {
      expect(tile(name)).toHaveAttribute("aria-busy", "true");
      expect(tile(name)).not.toHaveTextContent(/\d|—/);
    }
    expect(tile("Errors")).not.toHaveTextContent("of requests");
  });

  it("leaves out a cost the gateway didn't price", async () => {
    await renderUi(<UsageMetrics rows={[usage({ cost: null })]} from="2026-10-14" to="2026-10-14" showCost />);
    expect(tile("Requests")).toHaveTextContent("100");
    expect(screen.queryByText("Cost")).not.toBeInTheDocument();
  });
});

describe("usage charts", () => {
  it("draws requests and errors per day", async () => {
    await renderUi(<RequestsChart rows={twoDays} from="2026-10-13" to="2026-10-14" />);
    expect(screen.getByRole("heading", { name: "Requests" })).toBeInTheDocument();
    expect(screen.getByText("Requests and errors per day (UTC)")).toBeInTheDocument();
    expect(rowsOf("Requests and errors per day").map((r) => r.slice(1))).toEqual([
      ["40", "0"],
      ["100", "5"],
    ]);
  });

  it("draws a single day as bars", async () => {
    await renderUi(<RequestsChart rows={[usage()]} from="2026-10-14" to="2026-10-14" />);
    expect(await screen.findByRole("columnheader", { name: "Category" })).toBeInTheDocument();
    expect(rowsOf("Requests and errors per day")).toEqual([["Oct 14", "100", "5"]]);
  });

  it("names weeks and months by their start", async () => {
    await renderUi(
      <>
        <RequestsChart rows={[usage({ key: "2026-10-05" }), usage({ key: "2026-10-12" })]} from="2026-10-05" to="2026-10-12" granularity="week" />
        <CostChart rows={[usage({ key: "2026-09-01" })]} from="2026-09-01" to="2026-10-01" granularity="month" />
      </>,
    );
    expect(screen.getByText("Requests and errors per week (UTC)")).toBeInTheDocument();
    expect(rowsOf("Requests and errors per week").map((r) => r[0])).toEqual(["Week of 5 Oct", "Week of 12 Oct"]);
    expect(screen.getByText("USD per month (UTC), at the prices of request time")).toBeInTheDocument();
    expect(rowsOf("Cost per month")).toEqual([
      ["Sep 2026", "$1.25"],
      ["Oct 2026", "$0.00"],
    ]);
  });

  it("says when a chart has nothing or couldn't load", async () => {
    const retry = vi.fn();
    const { user } = await renderUi(
      <>
        <RequestsChart rows={[]} from="2026-10-13" to="2026-10-14" />
        <CostChart rows={[]} from="2026-10-13" to="2026-10-14" failed onRetry={retry} />
      </>,
    );
    expect(screen.getByText("No requests in this period.")).toBeInTheDocument();
    expect(screen.getByText("Couldn't load this chart")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(retry).toHaveBeenCalled();
  });

  it("splits tokens by kind, with reasoning out of output", async () => {
    await renderUi(<TokensChart rows={[usage()]} from="2026-10-13" to="2026-10-14" />);
    expect(screen.getByText("By kind, per day (UTC)")).toBeInTheDocument();
    await waitFor(() =>
      expect(rowsOf("Tokens by kind per day")).toEqual([
        ["Oct 13", "0", "0", "0", "0", "0"],
        ["Oct 14", "1K", "200", "50", "400", "100"],
      ]),
    );
  });

  it("says when there are no tokens in a month", async () => {
    await renderUi(<TokensChart rows={[]} from="2026-09-01" to="2026-10-01" granularity="month" />);
    expect(await screen.findByText("No tokens in this period.")).toBeInTheDocument();
    expect(screen.getByText("By kind, per month (UTC)")).toBeInTheDocument();
  });
});

describe("CostShare", () => {
  it("shares the cost between the priced rows", async () => {
    await renderUi(
      <CostShare
        title="Cost by project"
        description="Where the money went."
        rows={[...twoDays, usage({ key: "free", cost: "0" })]}
        label={(key) => `Day ${key}`}
      />,
    );
    const share = screen.getByRole("figure", { name: "Cost by project" });
    expect(share).toHaveTextContent("$1.75");
    expect(share).toHaveTextContent("Total cost");
    expect(rowsOf("Cost by project")).toEqual([
      ["Day 2026-10-13", "$0.50", "28.6%"],
      ["Day 2026-10-14", "$1.25", "71.4%"],
    ]);
  });

  it("says when nothing cost anything", async () => {
    await renderUi(<CostShare title="Cost by model" description="Per model." rows={[usage({ cost: null })]} label={(key) => key} />);
    expect(screen.getByText("No cost in this period.")).toBeInTheDocument();
  });
});
