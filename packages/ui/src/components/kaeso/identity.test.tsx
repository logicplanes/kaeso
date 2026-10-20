import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { KeyDisplay, ModelLabel, RequestTimeline, StatusCode } from "./identity.js";
import { LocaleProvider, type UiMessages } from "../../i18n/index.js";
import { labelWidth } from "../charts/core.js";
import de from "../../i18n/locales/de.js";
import zhCN from "../../i18n/locales/zh-CN.js";

describe("KeyDisplay", () => {
  it("names each access", () => {
    const { rerender } = render(<KeyDisplay redacted="sk-kso-AbC1…wxyz" access="inference" copy={false} />);
    expect(screen.getByText("Inference")).toBeInTheDocument();
    rerender(<KeyDisplay redacted="sk-kso-AbC1…wxyz" access="platform" copy={false} />);
    expect(screen.getByText("Platform")).toBeInTheDocument();
    rerender(<KeyDisplay redacted="sk-kso-AbC1…wxyz" access="both" copy={false} />);
    expect(screen.getByText("Inference + platform")).toBeInTheDocument();
  });
});

describe("StatusCode", () => {
  it("colors only what needs a look: a success is the plain code", () => {
    const { rerender } = render(<StatusCode code={200} />);
    expect(screen.getByText("200")).toHaveClass("text-fg");
    rerender(<StatusCode code={429} />);
    expect(screen.getByText("429")).toHaveClass("text-warning-text");
    rerender(<StatusCode code={529} />);
    expect(screen.getByText("529")).toHaveClass("text-danger-text");
    expect(screen.getByText("Overloaded")).toBeInTheDocument();
  });
  it("shows a short model id whole or not at all, and keeps a long one's start before its ellipsis", () => {
    const { rerender } = render(<ModelLabel provider="openai" model="gpt-5.4-mini" name="GPT-5.4 mini" />);
    expect(screen.getByText("gpt-5.4-mini")).toHaveClass("min-w-max");
    rerender(<ModelLabel provider="anthropic" model="claude-sonnet-5-5-20260901" name="Claude Sonnet 5.5" />);
    expect(screen.getByText("claude-sonnet-5-5-20260901")).toHaveClass("min-w-96");
  });
  it("leaves out the provider's mark in a list that already groups under it", () => {
    const { rerender } = render(<ModelLabel provider="openai" model="gpt-5.4-mini" />);
    expect(screen.getByRole("img", { name: "OpenAI" })).toBeInTheDocument();
    rerender(<ModelLabel provider="openai" model="gpt-5.4-mini" mark={false} />);
    expect(screen.queryByRole("img", { name: "OpenAI" })).toBeNull();
    expect(screen.getByText("gpt-5.4-mini")).toBeInTheDocument();
  });
});

describe("RequestTimeline", () => {
  it("is 240px by default and spans its container when full", () => {
    const { rerender } = render(<RequestTimeline ttfbMs={1054} totalMs={15532} />);
    const bar = screen.getByRole("img", { name: "Time to first byte 1.1 s of 15.5 s total" });
    expect(bar).toHaveClass("w-240");
    rerender(<RequestTimeline ttfbMs={1054} totalMs={15532} width="full" />);
    expect(screen.getByRole("img")).toHaveClass("w-full");
  });
  it.each<[string, UiMessages]>([
    ["en", {}],
    ["de", de],
    ["zh-CN", zhCN],
  ])("%s: fits 160px, each time over its label, the same lines whatever the times", (locale, messages) => {
    const lines = (ttfb: number, total: number) => {
      const { container, unmount } = render(
        <LocaleProvider locale={locale} messages={messages}>
          <RequestTimeline ttfbMs={ttfb} totalMs={total} width={160} />
        </LocaleProvider>,
      );
      const root = container.firstElementChild!;
      expect(root).toHaveClass("w-160");
      const columns = [...root.children[1]!.children];
      const out = columns.map((c) => [...c.children].map((l) => ({ text: l.textContent ?? "", className: l.className })));
      unmount();
      return out;
    };
    const short = lines(412, 1840);
    const long = lines(10_054, 125_532);
    for (const column of [...short, ...long]) {
      expect(column).toHaveLength(2);
      expect(column[0]!.className).toContain("whitespace-nowrap");
      expect(column[1]!.className).toContain("wrap-break-word");
      for (const line of column) expect(line.className).not.toMatch(/truncate|line-clamp|overflow-hidden/);
    }
    expect(short.map((c) => c[1]!.text)).toEqual(long.map((c) => c[1]!.text));
    expect(long.map((c) => c[0]!.text)).toEqual(["10.1 s", "125.5 s"].map((s) => (locale === "de" ? s.replace(".", ",") : s)));
    const [first, total] = short.map((c) => c[1]!.text);
    expect(labelWidth(first!) + 8 + labelWidth(total!)).toBeLessThanOrEqual(160);
    expect(labelWidth("9999.9 s") * 2 + 8).toBeLessThanOrEqual(160);
  });
});
