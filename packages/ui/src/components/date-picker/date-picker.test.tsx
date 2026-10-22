import { describe, expect, it, vi } from "vitest";
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { DatePicker, DateRangePicker, type DateRange } from "./date-picker.js";
import { describeInTimeZone, TIME_ZONES } from "../../../test/time-zone.js";
import { lazyModulesSettled } from "../../utils/lazy-module.js";

await Promise.all([import("./date-popup.js"), import("./range-popup.js")]);

const popoverLoaded = () => act(() => lazyModulesSettled());

describe("DateRangePicker", () => {
  it("opens on a click and takes a preset", async () => {
    const onChange = vi.fn();
    render(<DateRangePicker aria-label="Period" onValueChange={onChange} />);
    await userEvent.click(screen.getByRole("button", { description: "Period" }));
    await popoverLoaded();
    await userEvent.click(screen.getByRole("option", { name: "Last 7 days" }));
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("button", { description: "Period" })).toHaveAccessibleName("Last 7 days");
  });

  it("shows a value changed from outside by its dates, not the last preset's name", async () => {
    const presets = [
      {
        label: "Fixed week",
        range: () => ({
          from: new Date(2026, 9, 6),
          to: new Date(2026, 9, 12),
        }),
      },
    ];
    function Controlled({ value }: { value: DateRange }) {
      return <DateRangePicker aria-label="Period" value={value} presets={presets} />;
    }
    const { rerender } = render(<Controlled value={{ from: new Date(2026, 8, 13), to: new Date(2026, 9, 12) }} />);
    await userEvent.click(screen.getByRole("button", { description: "Period" }));
    await popoverLoaded();
    await userEvent.click(screen.getByRole("option", { name: "Fixed week" }));
    rerender(<Controlled value={{ from: new Date(2026, 9, 6), to: new Date(2026, 9, 12) }} />);
    expect(screen.getByRole("button", { description: "Period" })).toHaveAccessibleName("Fixed week");
    rerender(<Controlled value={{ from: new Date(2026, 8, 13), to: new Date(2026, 9, 12) }} />);
    expect(screen.getByRole("button", { description: "Period" })).toHaveAccessibleName("13 Sep – 12 Oct");
  });

  it("opens complete once loaded: presets, both months and the typed fields together", async () => {
    render(<DateRangePicker aria-label="Period" />);
    const trigger = screen.getByRole("button", { description: "Period" });
    await userEvent.hover(trigger);
    await userEvent.click(trigger);
    await popoverLoaded();
    screen.getByRole("listbox", { name: "Presets" });
    expect(trigger).toHaveAttribute("aria-expanded", "true");
    expect(screen.getAllByRole("grid")).toHaveLength(2);
    expect(screen.getByRole("textbox", { name: "Start date" })).toBeInTheDocument();
    expect(screen.queryByText("", { selector: "[aria-busy]" })).not.toBeInTheDocument();
  });
});

describe("DateRangePicker's popover", () => {
  const months = () => screen.getAllByRole("grid").map((g) => g.getAttribute("aria-label"));
  const open = async () => {
    await userEvent.click(screen.getByRole("button", { description: "Period" }));
    await popoverLoaded();
    screen.getByRole("listbox", { name: "Presets" });
  };

  it("makes the presets one tab stop that the arrow keys, Home and End move through", async () => {
    render(<DateRangePicker aria-label="Period" />);
    await open();
    const options = within(screen.getByRole("listbox", { name: "Presets" })).getAllByRole("option");
    expect(options.filter((o) => o.tabIndex === 0)).toHaveLength(1);
    const start = options.find((o) => o.tabIndex === 0)!;
    start.focus();
    await userEvent.keyboard("{Home}");
    expect(options[0]).toHaveFocus();
    await userEvent.keyboard("{End}");
    expect(options.at(-1)).toHaveFocus();
    await userEvent.keyboard("{Home}{ArrowDown}");
    expect(options.indexOf(document.activeElement as HTMLElement)).toBeGreaterThan(0);
    await userEvent.keyboard("{ArrowUp}");
    expect(options[0]).toHaveFocus();
  });
  it("opens on the range's months, its last on the right", async () => {
    render(
      <DateRangePicker
        aria-label="Period"
        defaultValue={{
          from: new Date(2026, 6, 16),
          to: new Date(2026, 9, 13),
        }}
      />,
    );
    await open();
    expect(months()).toEqual(["September 2026", "October 2026"]);
  });

  it("opens on the range's months again after paging away", async () => {
    render(
      <DateRangePicker
        aria-label="Period"
        defaultValue={{
          from: new Date(2026, 6, 16),
          to: new Date(2026, 9, 13),
        }}
      />,
    );
    await open();
    for (let i = 0; i < 4; i++) await userEvent.click(screen.getByRole("button", { name: /previous month/i }));
    expect(months()).toEqual(["May 2026", "June 2026"]);
    await userEvent.keyboard("{Escape}");
    await open();
    expect(months()).toEqual(["September 2026", "October 2026"]);
  });

  it("names a controlled preset, and marks it in the list", async () => {
    const presets = [
      {
        label: "All time",
        range: () => ({
          from: new Date(2024, 0, 1),
          to: new Date(2026, 9, 13),
        }),
      },
    ];
    render(<DateRangePicker aria-label="Period" value={presets[0]!.range()} presets={presets} preset="All time" />);
    expect(screen.getByRole("button", { description: "Period" })).toHaveAccessibleName("All time");
    await open();
    expect(screen.getByRole("option", { name: "All time" })).toHaveAttribute("aria-selected", "true");
    expect(months()).toEqual(["September 2026", "October 2026"]);
  });

  it("applies typed days with Apply, not before, and says they are no preset", async () => {
    const onChange = vi.fn();
    render(<DateRangePicker aria-label="Period" defaultValue={{ from: new Date(2026, 9, 1), to: new Date(2026, 9, 12) }} onValueChange={onChange} />);
    await open();
    const start = screen.getByRole("textbox", { name: "Start date" });
    await userEvent.clear(start);
    await userEvent.type(start, "2026-03-02{Enter}");
    const end = screen.getByRole("textbox", { name: "End date" });
    await userEvent.clear(end);
    await userEvent.type(end, "2026-04-05{Enter}");
    expect(onChange).not.toHaveBeenCalled();
    expect(months()).toEqual(["March 2026", "April 2026"]);
    expect(screen.getByRole("option", { name: "Custom" })).toHaveAttribute("aria-selected", "true");
    await userEvent.click(screen.getByRole("button", { name: "Apply" }));
    expect(onChange).toHaveBeenCalledTimes(1);
    const [range, details] = onChange.mock.calls[0]!;
    expect([range.from.toDateString(), range.to.toDateString()]).toEqual([new Date(2026, 2, 2).toDateString(), new Date(2026, 3, 5).toDateString()]);
    expect(details).toEqual({ preset: null });
    expect(screen.getByRole("button", { description: "Period" })).toHaveAccessibleName("2 Mar – 5 Apr");
  });

  it("refuses a typed day that is disabled", async () => {
    render(
      <DateRangePicker
        aria-label="Period"
        defaultValue={{ from: new Date(2026, 9, 1), to: new Date(2026, 9, 12) }}
        disabledDays={(d) => d > new Date(2026, 9, 13)}
      />,
    );
    await open();
    const end = screen.getByRole("textbox", { name: "End date" });
    await userEvent.clear(end);
    await userEvent.type(end, "2027-01-01{Enter}");
    expect(end).toHaveValue("2026-10-12");
  });

  it("leaves the range as it was when closed without Apply", async () => {
    const onChange = vi.fn();
    render(<DateRangePicker aria-label="Period" defaultValue={{ from: new Date(2026, 9, 1), to: new Date(2026, 9, 12) }} onValueChange={onChange} />);
    await open();
    const start = screen.getByRole("textbox", { name: "Start date" });
    await userEvent.clear(start);
    await userEvent.type(start, "2026-10-05{Enter}");
    await userEvent.keyboard("{Escape}");
    expect(onChange).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { description: "Period" })).toHaveAccessibleName("1 Oct – 12 Oct");
  });

  it("hands over the preset with its range", async () => {
    const onChange = vi.fn();
    const presets = [
      {
        label: "Last 6 months",
        range: () => ({
          from: new Date(2026, 3, 14),
          to: new Date(2026, 9, 13),
        }),
      },
    ];
    render(<DateRangePicker aria-label="Period" presets={presets} onValueChange={onChange} />);
    await open();
    await userEvent.click(screen.getByRole("option", { name: "Last 6 months" }));
    expect(onChange.mock.calls[0]![1]).toEqual({ preset: "Last 6 months" });
  });
});

describe("DatePicker", () => {
  it("opens on a click", async () => {
    render(<DatePicker aria-label="Expires" />);
    const trigger = screen.getByRole("button", { description: "Expires" });
    await userEvent.click(trigger);
    expect(trigger).toHaveAttribute("aria-expanded", "true");
    await popoverLoaded();
    expect(screen.getByRole("dialog", { name: "Choose a date" })).toBeInTheDocument();
    expect(trigger).toHaveAttribute("aria-expanded", "true");
  });

  it("refuses a typed day that is disabled", async () => {
    const onChange = vi.fn();
    render(<DatePicker aria-label="Expires" defaultValue={new Date(2026, 9, 1)} disabledDays={(d) => d > new Date(2026, 9, 13)} onValueChange={onChange} />);
    await userEvent.click(screen.getByRole("button", { description: "Expires" }));
    await popoverLoaded();
    const field = screen.getByRole("textbox", { name: "Date" });
    await userEvent.clear(field);
    await userEvent.type(field, "2027-01-01{Enter}");
    expect(onChange).not.toHaveBeenCalled();
    expect(field).toHaveValue("2026-10-01");
    expect(screen.getByRole("button", { description: "Expires" })).toHaveAccessibleName("1 Oct 2026");
  });
});

describe("the trigger's name", () => {
  it.each([
    ["DatePicker", <DatePicker key="d" aria-label="Expires" />, "Expires", "Pick a date"],
    ["DatePicker with a date", <DatePicker key="d" aria-label="Expires" defaultValue={new Date(2027, 0, 1)} />, "Expires", "1 Jan 2027"],
    ["DateRangePicker", <DateRangePicker key="r" aria-label="Period" />, "Period", "Pick a range"],
    [
      "DateRangePicker with a range",
      <DateRangePicker key="r" aria-label="Period" defaultValue={{ from: new Date(2026, 9, 1), to: new Date(2026, 9, 12) }} />,
      "Period",
      "1 Oct – 12 Oct",
    ],
  ])("%s is what it shows, described by its aria-label", (_, element, purpose, shown) => {
    render(element);
    const trigger = screen.getByRole("button", { name: shown });
    expect(trigger).toHaveAccessibleDescription(purpose);
    expect(trigger).toHaveTextContent(shown);
    expect(trigger).not.toHaveAttribute("aria-label");
  });
  it("comes from aria-labelledby, followed by what it shows", () => {
    render(
      <>
        <span id="period-heading">Usage period</span>
        <DateRangePicker aria-labelledby="period-heading" />
      </>,
    );
    expect(screen.getByRole("button", { name: "Usage period Pick a range" })).not.toHaveAttribute("aria-describedby");
  });
  it("comes from a field's label, with the help after the value", () => {
    render(
      <>
        <DatePicker label="Expiry date" description="The key stops working at the start of this day." defaultValue={new Date(2027, 0, 1)} />
        <DateRangePicker label="Period" />
      </>,
    );
    expect(screen.getByRole("button", { name: "Expiry date" })).toHaveAccessibleDescription("1 Jan 2027 The key stops working at the start of this day.");
    expect(screen.getByRole("button", { name: "Period" })).toHaveAccessibleDescription("Pick a range");
  });
  it("is what it shows when nothing else names it", () => {
    render(<DateRangePicker />);
    const trigger = screen.getByRole("button", { name: "Pick a range" });
    expect(trigger).not.toHaveAttribute("aria-describedby");
  });
});

describe("the typed ISO field and the calendar agree", () => {
  const ymd = (d: Date | null | undefined) => (d ? [d.getFullYear(), d.getMonth() + 1, d.getDate(), d.getHours()] : null);
  for (const tz of TIME_ZONES) {
    describeInTimeZone(tz, () => {
      it("shows a picked day as that day", async () => {
        const onChange = vi.fn();
        render(<DatePicker aria-label="Expires" defaultValue={new Date(2026, 11, 20)} onValueChange={onChange} />);
        expect(screen.getByRole("button", { description: "Expires" })).toHaveAccessibleName("20 Dec 2026");
        await userEvent.click(screen.getByRole("button", { description: "Expires" }));
        await popoverLoaded();
        const field = screen.getByRole("textbox", { name: "Date" });
        expect(field).toHaveValue("2026-12-20");
        await userEvent.click(screen.getByRole("button", { name: /next month/i }));
        await userEvent.click(screen.getByRole("button", { name: /January 1, 2027/ }));
        expect(ymd(onChange.mock.lastCall?.[0])).toEqual([2027, 1, 1, 0]);
        const trigger = screen.getByRole("button", { description: "Expires" });
        expect(trigger).toHaveAccessibleName("1 Jan 2027");
        await userEvent.click(trigger);
        await popoverLoaded();
        expect(screen.getByRole("textbox", { name: "Date" })).toHaveValue("2027-01-01");
      });

      it("takes a typed day as that day", async () => {
        const onChange = vi.fn();
        render(<DatePicker aria-label="Expires" defaultValue={new Date(2027, 0, 1)} onValueChange={onChange} />);
        await userEvent.click(screen.getByRole("button", { description: "Expires" }));
        await popoverLoaded();
        const field = screen.getByRole("textbox", { name: "Date" });
        expect(field).toHaveValue("2027-01-01");
        await userEvent.clear(field);
        await userEvent.type(field, "2027-01-15{Enter}");
        expect(ymd(onChange.mock.lastCall?.[0])).toEqual([2027, 1, 15, 0]);
        expect(field).toHaveValue("2027-01-15");
        expect(screen.getByRole("button", { description: "Expires" })).toHaveAccessibleName("15 Jan 2027");
        expect(screen.getByRole("gridcell", { selected: true })).toHaveTextContent(/^15$/);
        await userEvent.clear(field);
        await userEvent.type(field, "2027-03-28{Enter}");
        expect(ymd(onChange.mock.lastCall?.[0])).toEqual([2027, 3, 28, 0]);
        expect(field).toHaveValue("2027-03-28");
        expect(screen.getByRole("button", { description: "Expires" })).toHaveAccessibleName("28 Mar 2027");
      });

      it("puts back the day when the typed text isn't a date", async () => {
        const onChange = vi.fn();
        render(<DatePicker aria-label="Expires" defaultValue={new Date(2027, 0, 1)} onValueChange={onChange} />);
        await userEvent.click(screen.getByRole("button", { description: "Expires" }));
        await popoverLoaded();
        const field = screen.getByRole("textbox", { name: "Date" });
        await userEvent.clear(field);
        await userEvent.type(field, "2027-02-30{Enter}");
        expect(onChange).not.toHaveBeenCalled();
        expect(field).toHaveValue("2027-01-01");
      });

      it("shows and takes a range's days as those days", async () => {
        const onChange = vi.fn<(r: DateRange | null) => void>();
        render(
          <DateRangePicker
            aria-label="Period"
            defaultValue={{
              from: new Date(2026, 11, 31),
              to: new Date(2027, 0, 1),
            }}
            onValueChange={onChange}
          />,
        );
        expect(screen.getByRole("button", { description: "Period" })).toHaveAccessibleName("31 Dec 2026 – 1 Jan 2027");
        await userEvent.click(screen.getByRole("button", { description: "Period" }));
        await popoverLoaded();
        const start = screen.getByRole("textbox", { name: "Start date" });
        const end = screen.getByRole("textbox", { name: "End date" });
        expect(start).toHaveValue("2026-12-31");
        expect(end).toHaveValue("2027-01-01");
        await userEvent.clear(end);
        await userEvent.type(end, "2027-01-07{Enter}");
        expect(end).toHaveValue("2027-01-07");
        await userEvent.click(screen.getByRole("button", { name: "Apply" }));
        const range = onChange.mock.lastCall?.[0];
        expect(ymd(range?.from)).toEqual([2026, 12, 31, 0]);
        expect(ymd(range?.to)).toEqual([2027, 1, 7, 0]);
        expect(screen.getByRole("button", { description: "Period" })).toHaveAccessibleName("31 Dec 2026 – 7 Jan 2027");
      });
    });
  }
});
