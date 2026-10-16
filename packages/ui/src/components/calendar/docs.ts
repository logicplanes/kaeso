import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Calendar: {
    description:
      "The calendar: 36px day circles, today ringed, ranges as a continuous strip with ink ends. Always six weeks, so paging never changes its height; with two months side by side, each shows only its own days.",
    props: {
      defaultMonth: "The month shown first when `month` isn't controlled; the arrows move from there.",
      disabled: "Returns `true` for days that can't be chosen.",
      mode: "Picks one day (`single`) or a range (`range`).",
      month: "The month shown, when you control it.",
      numberOfMonths: "How many months are shown side by side.",
      onMonthChange: "Called when the month shown changes.",
      onSelect: "Called with the new day or range.",
      selected: "The selected day or range.",
      weekStartsOn: "0 = Sunday, 1 = Monday (default).",
    },
  },
} satisfies ComponentDocs;
