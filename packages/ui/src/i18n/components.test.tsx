import { afterEach, describe, expect, it } from "vitest";
import type { ReactNode } from "react";
import { render, screen } from "@testing-library/react";
import { LocaleProvider, type UiMessages } from "./index";
import { formatRelative, formatUSD, setFormatLocale } from "../format";
import de from "./locales/de";
import zhCN from "./locales/zh-CN";
import { Dialog } from "../components/dialog/dialog";
import { Pagination } from "../components/pagination/pagination";
import { Calendar } from "../components/calendar/calendar";
import { DateRangePicker } from "../components/date-picker/date-picker";
import { SearchInput } from "../components/input/input";
import { CopyButton } from "../components/copy-button/copy-button";
import { TableOfContents } from "../components/toc/toc";
import { RightsMatrix } from "../components/kaeso/rights";
import { Kbd } from "../components/kbd/kbd";

const languages: [string, UiMessages][] = [
  ["de", de],
  ["zh-CN", zhCN],
];

function inLanguage(locale: string, messages: UiMessages, ui: ReactNode) {
  return render(
    <LocaleProvider locale={locale} messages={messages}>
      {ui}
    </LocaleProvider>,
  );
}

afterEach(() => setFormatLocale("en"));

describe("components in German and Chinese", () => {
  it.each(languages)("%s: a dialog's close button, pagination and the copy button", (locale, messages) => {
    const { unmount } = inLanguage(
      locale,
      messages,
      <Dialog defaultOpen title="Rename">
        <p>Body</p>
      </Dialog>,
    );
    expect(screen.getByRole("button", { name: messages["dialog.close"] ?? "" })).toBeInTheDocument();
    unmount();
    inLanguage(
      locale,
      messages,
      <>
        <Pagination variant="cursor" hasMore hasPrevious />
        <CopyButton value="sk-1" />
      </>,
    );
    expect(screen.getByRole("navigation", { name: messages["pagination.label"] ?? "" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: messages["pagination.next"] ?? "" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: messages["copyButton.copy"] ?? "" })).toBeInTheDocument();
  });

  it("German: weekday names, the month and the range picker's placeholder and presets", () => {
    inLanguage(
      "de",
      de,
      <>
        <Calendar defaultMonth={new Date(2026, 9, 1)} />
        <DateRangePicker aria-label="Zeitraum" />
        <DateRangePicker aria-label="Letzte Woche" preset="Last 7 days" value={{ from: new Date(2026, 9, 8), to: new Date(2026, 9, 14) }} />
      </>,
    );
    expect(screen.getByText("Oktober 2026")).toBeInTheDocument();
    expect(screen.getByText("Mo")).toBeInTheDocument();
    expect(screen.getByText("So")).toBeInTheDocument();
    expect(screen.getByText("Zeitraum wählen")).toBeInTheDocument();
    expect(screen.getByText("Letzte 7 Tage")).toBeInTheDocument();
  });

  it("Chinese: weekday names, the month, search, contents and the rights editor", () => {
    inLanguage(
      "zh-CN",
      zhCN,
      <>
        <Calendar defaultMonth={new Date(2026, 9, 1)} />
        <SearchInput aria-label="搜索密钥" />
        <TableOfContents items={[{ id: "a", label: "Intro" }]} active="a" />
        <RightsMatrix
          searchable
          groups={[{ label: "Keys", rights: [{ code: "keys.view", label: "View keys", description: "See every key", granted: false }] }]}
        />
      </>,
    );
    expect(screen.getByText("2026年10月")).toBeInTheDocument();
    expect(screen.getByText("周一")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("搜索")).toBeInTheDocument();
    expect(screen.getByText("本页内容")).toBeInTheDocument();
    expect(screen.getByRole("searchbox", { name: "搜索权限" })).toBeInTheDocument();
  });

  it("speaks key names in the language", () => {
    inLanguage("de", de, <Kbd>shift</Kbd>);
    expect(screen.getByLabelText("Umschalttaste")).toBeInTheDocument();
  });

  it("stays English without a provider", () => {
    render(<Pagination variant="cursor" hasMore hasPrevious />);
    expect(screen.getByRole("button", { name: "Next page" })).toBeInTheDocument();
  });
});

describe("the formatters in German and Chinese", () => {
  it("write dollars and relative times for the language", () => {
    const now = new Date(2026, 9, 14, 12, 0);
    setFormatLocale("de", de);
    expect(formatUSD("1234.5")).toBe("1.234,50\u00a0$");
    expect(formatRelative(new Date(2026, 9, 14, 11, 56), now)).toBe("vor 4 Min.");
    setFormatLocale("zh-CN", zhCN);
    expect(formatUSD("1234.5")).toBe("$1,234.50");
    setFormatLocale("en");
    expect(formatUSD("1234.5")).toBe("$1,234.50");
    expect(formatRelative(new Date(2026, 9, 14, 11, 56), now)).toBe("4 min ago");
  });
});
