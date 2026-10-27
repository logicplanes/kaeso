import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  MetricRow: {
    description: "Three to five stats in a row with generous space between them.",
    props: {
      children: "The stats, side by side.",
    },
  },
  Stat: {
    description: "A big number followed by its word, no box, as on a usage page.",
    props: {
      delta: 'A delta badge: "+12%".',
      size: "The size of the number.",
      unit: 'The word after it, on the same baseline: "total spend", "sessions".',
      value: 'The number, already formatted: "$12.40", "1.2M".',
    },
  },
  StatGrid: {
    description:
      "`StatTile`s in one raised group, divided by hairlines: two to a row, then as many as there are tiles (up to four) from 1024px. The last tile of a short row widens to fill it (the third of three spans both columns below 1024px), so the group never shows an empty cell. Where a number doesn't fit its tile (a phone, a long amount), every tile's number steps down a size, and the tiles take a row each when even the smallest doesn't fit. At the same width, live numbers only step the size down, never back up, so a digit that comes and goes doesn't make the tiles jump; a value that still can't fit ends in an ellipsis and shows in full in a tooltip.",
    props: {
      "aria-label": "The grid's accessible name.",
      children: "The stats, in a grid.",
    },
  },
  StatTile: {
    description: "One metric of a dashboard row: its label, the number, the change against the previous period and the trend.",
    props: {
      change:
        'The change against the previous period, as a fraction (0.12 is +12%). `better` says which direction is good: errors are better down. The badge is green or red and says "better" or "worse" to screen readers.',
      detail: 'A quiet line under the number: "vs. the previous 30 days", "0.4% of requests".',
      label: 'What the number is: "Requests", "Cost".',
      loading: "While the numbers load: the value, the change and the trend show their shape at the tile's final height.",
      trend:
        "One value per day (or step), drawn as a sparkline across the tile. A tile without it has no room for one; pass `[]` while a trend loads to keep it.",
      trendColor: "The trend's color; the first series color by default.",
      value: "The number, already formatted.",
    },
  },
} satisfies ComponentDocs;
