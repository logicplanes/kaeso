import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
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

  it("says each tile couldn't load when the usage failed, instead of loading for ever", async () => {
    await renderUi(<UsageMetrics rows={[]} from="2026-10-13" to="2026-10-14" showCost failed />);
    for (const name of ["Requests", "Tokens", "Cost", "Errors"]) {
      expect(tile(name)).not.toHaveAttribute("aria-busy");
      expect(tile(name)).toHaveTextContent("–");
      expect(tile(name)).toHaveTextContent("Couldn't load");
      expect(tile(name)).not.toHaveTextContent(/\d/);
    }
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
    expect(rowsOf("Requests and errors per day")).toEqual([["14 Oct", "100", "5"]]);
  });

  it("names weeks and months by their start", async () => {
    await renderUi(
      <>
        <RequestsChart rows={[usage({ key: "2026-10-05" }), usage({ key: "2026-10-12" })]} from="2026-10-05" to="2026-10-12" granularity="week" />
        <CostChart rows={[usage({ key: "2026-09-01" })]} from="2026-09-01" to="2026-10-01" granularity="month" />
      </>,
    );
    expect(screen.getByText("Requests and errors per week (UTC)")).toBeInTheDocument();
    expect(rowsOf("Requests and errors per week").map((r) => r[0])).toEqual(["Week of 5 Oct", "Week of 12 Oct, partial"]);
    expect(screen.getByText("USD per month (UTC), at the prices of request time")).toBeInTheDocument();
    expect(rowsOf("Cost per month")).toEqual([
      ["Sep 2026", "$1.25"],
      ["Oct 2026, partial", "$0.00"],
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
        ["13 Oct", "0", "0", "0", "0", "0"],
        ["14 Oct", "1K", "200", "50", "400", "100"],
      ]),
    );
  });

  it("says when there are no tokens in a month", async () => {
    await renderUi(<TokensChart rows={[]} from="2026-09-01" to="2026-10-01" granularity="month" />);
    expect(await screen.findByText("No tokens in this period.")).toBeInTheDocument();
    expect(screen.getByText("By kind, per month (UTC)")).toBeInTheDocument();
  });
});

const zoneBefore = process.env.TZ;
const drawn = () => {
  vi.useFakeTimers({ now: new Date("2026-10-20T09:00:00Z"), toFake: ["Date"] });
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(800);
};
const undrawn = () => {
  vi.useRealTimers();
  vi.restoreAllMocks();
};
describe.each(["Europe/Berlin", "America/Los_Angeles"])("charts in a browser in %s", (zone) => {
  beforeAll(() => {
    process.env.TZ = zone;
  });
  afterAll(() => {
    process.env.TZ = zoneBefore;
  });
  beforeEach(drawn);
  afterEach(undrawn);

  const month = Array.from({ length: 30 }, (_, i) => usage({ key: new Date(Date.UTC(2026, 8, 21 + i)).toISOString().slice(0, 10), requests: i ? 100 : 2249 }));

  it("names every day by the API's UTC day, in requests, cost and tokens alike", async () => {
    await renderUi(
      <>
        <RequestsChart rows={month} from="2026-09-21" to="2026-10-20" />
        <CostChart rows={month} from="2026-09-21" to="2026-10-20" />
        <TokensChart rows={month} from="2026-09-21" to="2026-10-20" />
      </>,
    );
    expect(rowsOf("Requests and errors per day")[0]).toEqual(["21 Sep", "2,249", "5"]);
    expect(rowsOf("Cost per day")[0]?.[0]).toBe("21 Sep");
    await waitFor(() => expect(rowsOf("Tokens by kind per day")[0]?.[0]).toBe("21 Sep"));
    const axis = (name: string) => [...screen.getByRole("figure", { name }).querySelectorAll("svg text[text-anchor=middle]")].map((t) => t.textContent);
    expect(axis("Requests and errors per day").length).toBeGreaterThan(3);
    expect(axis("Requests and errors per day")).toEqual(axis("Tokens by kind per day"));
    expect(axis("Requests and errors per day")).not.toContain("20 Sep");
  });

  it("puts October's month on October", async () => {
    const months = ["2026-05-01", "2026-06-01", "2026-07-01", "2026-08-01", "2026-09-01", "2026-10-01"].map((key) => usage({ key }));
    await renderUi(<RequestsChart rows={months} from="2026-04-21" to="2026-10-20" granularity="month" />);
    expect(rowsOf("Requests and errors per month").map((r) => r[0])).toEqual([
      "Apr 2026, partial",
      "May 2026",
      "Jun 2026",
      "Jul 2026",
      "Aug 2026",
      "Sep 2026",
      "Oct 2026, so far",
    ]);
    const labels = [...screen.getByRole("figure", { name: "Requests and errors per month" }).querySelectorAll("svg text[text-anchor=middle]")];
    expect(labels.map((t) => t.textContent).at(-1)).toBe("Oct");
  });
});

describe("the period still running", () => {
  beforeEach(drawn);
  afterEach(undrawn);

  it("says a lone today counts so far", async () => {
    await renderUi(<RequestsChart rows={[usage({ key: "2026-10-20", requests: 30 })]} from="2026-10-20" to="2026-10-20" />);
    expect(await screen.findByRole("columnheader", { name: "Category" })).toBeInTheDocument();
    expect(rowsOf("Requests and errors per day")).toEqual([["20 Oct, so far", "30", "5"]]);
  });

  it("says today counts so far, and leaves the share of errors out", async () => {
    await renderUi(<RequestsChart rows={[usage({ key: "2026-10-19" }), usage({ key: "2026-10-20", requests: 30 })]} from="2026-10-19" to="2026-10-20" />);
    expect(rowsOf("Requests and errors per day").map((r) => r[0])).toEqual(["19 Oct", "20 Oct, so far"]);
    const figure = screen.getByRole("figure", { name: "Requests and errors per day" });
    expect(figure.querySelectorAll("path[data-partial]").length).toBeGreaterThan(0);
    fireEvent.keyDown(within(figure).getByRole("group"), { key: "End" });
    expect(figure.querySelector("div[role=presentation]")?.textContent).toBe("20 Oct, so farRequests30Errors5");
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
