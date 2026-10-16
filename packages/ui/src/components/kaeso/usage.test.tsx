import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { LimitMeter, UsageBreakdown } from "./usage.js";
import { LocaleProvider, type UiMessages } from "../../i18n/index.js";
import de from "../../i18n/locales/de.js";
import zhCN from "../../i18n/locales/zh-CN.js";

describe("UsageBreakdown", () => {
  it("names the kinds that were used and lists every kind", () => {
    render(<UsageBreakdown tokens={{ input: 1_200_000, output: 300_000, reasoning: 0 }} />);
    expect(screen.getByRole("img", { name: "Tokens: Input 1,200,000, Output 300,000" })).toBeInTheDocument();
    expect(screen.getByText("1.2M")).toHaveAttribute("title", "1,200,000 tokens");
    expect(screen.getAllByText("0")).toHaveLength(3);
  });

  it.each<[string, UiMessages, string, string]>([
    ["en", {}, "103M", "103,000,000 tokens"],
    ["de", de, "103\u00a0Mio.", "103.000.000 Token"],
    ["zh-CN", zhCN, "1.03亿", "103,000,000 个 Token"],
  ])("%s: writes 103 million input tokens as %s, the exact count on hover", (locale, messages, shown, exact) => {
    render(
      <LocaleProvider locale={locale} messages={messages}>
        <UsageBreakdown tokens={{ input: 103_000_000, output: 51_000 }} />
      </LocaleProvider>,
    );
    expect(screen.getByText(shown.replace(/\u00a0/g, " "))).toHaveAttribute("title", exact);
  });

  it("draws an empty bar when nothing was used", () => {
    const { container } = render(<UsageBreakdown tokens={{}} />);
    expect(container.querySelector("[role=img]")?.children).toHaveLength(0);
  });
});

describe("LimitMeter", () => {
  it("shows a hard limit with its window and reset", () => {
    const { container } = render(
      <LimitMeter kind="spend" scope="Production" window="per month" used={400} limit={500} valueLabel="$400.00 of $500.00" resets="Resets 1 Nov" />,
    );
    expect(screen.getByRole("meter", { name: "spend limit" })).toHaveAttribute("aria-valuenow", "80");
    expect(container).toHaveTextContent("Hard");
    expect(container).toHaveTextContent("$400.00 of $500.00 · per monthResets 1 Nov");
  });

  it("shows a soft limit, and a zero limit as empty", () => {
    const { container } = render(<LimitMeter kind="tokens" scope="Docs search" window="per day" used={10} limit={0} valueLabel="10 tokens" hard={false} />);
    expect(container).toHaveTextContent("Soft");
    expect(screen.getByRole("meter", { name: "tokens limit" })).toHaveAttribute("aria-valuenow", "0");
    render(<LimitMeter kind="requests" scope="All" window="per minute" used={1} limit={10} valueLabel="1" />);
    expect(screen.getByRole("meter", { name: "requests limit" })).toBeInTheDocument();
  });
});
