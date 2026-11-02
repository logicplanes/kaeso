import { screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { setFormatTimeZone } from "@logicplanes/kaeso-ui/format";
import { renderUi } from "~/test/render";
import { PeriodPicker } from "./period-picker";

const days = () => ({ from: "2026-10-09", to: "2026-10-15" });

async function open(period?: "7d") {
  const onChange = vi.fn();
  const view = await renderUi(<PeriodPicker from="2026-10-01" to="2026-10-07" period={period} days={days} onChange={onChange} />);
  await view.user.click(screen.getByRole("button", { description: "Period" }));
  const dialog = await screen.findByRole("dialog", { name: "Choose a date range" });
  return { ...view, dialog, onChange };
}

describe("PeriodPicker", () => {
  it("shows the chosen days and picks a named period", async () => {
    const { user, dialog, onChange } = await open();
    expect(screen.getByRole("button", { name: "1 Oct – 7 Oct", description: "Period" })).toBeInTheDocument();
    expect(within(dialog).getByRole("option", { name: "Custom" })).toHaveAttribute("aria-selected", "true");
    await user.click(within(dialog).getByRole("option", { name: "Last 7 days" }));
    await waitFor(() => expect(onChange).toHaveBeenCalledWith({ period: "7d", from: undefined, to: undefined }));
  });

  it("marks the named period that is in use", async () => {
    const { dialog } = await open("7d");
    expect(within(dialog).getByRole("option", { name: "Last 7 days" })).toHaveAttribute("aria-selected", "true");
    expect(within(dialog).getByRole("option", { name: "Custom" })).toHaveAttribute("aria-selected", "false");
  });

  it("picks custom days", async () => {
    const { user, dialog, onChange } = await open();
    const start = within(dialog).getByRole("textbox", { name: "Start date" });
    const end = within(dialog).getByRole("textbox", { name: "End date" });
    await user.clear(start);
    await user.type(start, "2026-10-02");
    await user.clear(end);
    await user.type(end, "2026-10-05");
    await user.click(within(dialog).getByRole("button", { name: "Apply" }));
    await waitFor(() => expect(onChange).toHaveBeenCalledWith({ period: undefined, from: "2026-10-02", to: "2026-10-05" }));
  });

  describe("ends on the UTC day, which usage counts by, whatever the install's time zone", () => {
    afterEach(() => vi.useRealTimers());

    async function pickUntil(zone: string, now: string, last: string) {
      setFormatTimeZone(zone);
      vi.setSystemTime(new Date(now));
      const view = await open();
      await view.user.clear(within(view.dialog).getByRole("textbox", { name: "End date" }));
      await view.user.type(within(view.dialog).getByRole("textbox", { name: "End date" }), last);
      await view.user.click(within(view.dialog).getByRole("button", { name: "Apply" }));
      return view;
    }

    it("refuses the install's today when that UTC day hasn't started", async () => {
      const { dialog, onChange } = await pickUntil("Asia/Tokyo", "2026-10-14T20:00:00Z", "2026-10-15");
      expect(within(dialog).getByRole("alert")).toHaveTextContent("This date can't be chosen. Choose another date.");
      expect(onChange).not.toHaveBeenCalledWith(expect.objectContaining({ to: "2026-10-15" }));
    });

    it("offers today's UTC day while the install's clock is still on the day before", async () => {
      const { onChange } = await pickUntil("America/Los_Angeles", "2026-10-15T03:00:00Z", "2026-10-15");
      await waitFor(() => expect(onChange).toHaveBeenCalledWith({ period: undefined, from: "2026-10-01", to: "2026-10-15" }));
    });
  });
});
