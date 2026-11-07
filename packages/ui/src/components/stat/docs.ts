import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Stat: {
    description:
      'A number. Plain, it is a big number followed by its word, no box, as on a usage page. As a `tile` (`variant="tile"`), it is one metric of a dashboard row in a `StatGroup`: its label, the number, the change against the previous period and the trend.',
    props: {
      change:
        'The change against the previous period, as a fraction (0.12 is +12%). `better` says which direction is good: errors are better down. It is written as text by `Delta`: green or red, and "better" or "worse" for screen readers.',
      delta: 'A delta badge: "+12%".',
      detail: 'A quiet line under the number: "vs. the previous 30 days", "0.4% of requests".',
      label: 'What the number is: "Requests", "Cost".',
      loading: "While the numbers load: the value, the change and the trend show their shape at the tile's final height.",
      size: "The size of the number.",
      trend:
        "One value per day (or step), drawn as a sparkline across the tile. A tile without it has no room for one; pass `[]` while a trend loads to keep it.",
      trendColor: "The trend's color; the first series color by default.",
      unit: 'The word after it, on the same baseline: "total spend", "sessions".',
      value: 'The number, already formatted: "$12.40", "1.2M".',
      variant: "A plain number with its unit and delta, or a tile in a `StatGroup` with its label, change, detail and trend.",
    },
  },
  StatGroup: {
    description:
      "Stats side by side. As a `grid` (the default), tiles sit in one raised group divided by hairlines: two to a row, then as many as there are tiles (up to four) from 1024px. The last tile of a short row widens to fill it, so the group never shows an empty cell. Where a number doesn't fit its tile, every tile's number steps down a size, and the tiles take a row each when even the smallest doesn't fit; live numbers only step down at the same width, so a digit that comes and goes doesn't make the tiles jump. As a `row`, three to five plain numbers sit side by side with generous space between them.",
    props: {
      "aria-label": "The grid's accessible name.",
      children: "The stats.",
      variant: "Tiles in a fitted grid (`grid`), or plain numbers in a row that wraps (`row`).",
    },
  },
  Delta: {
    description:
      'A change written as text, "↑16%", in the success or danger text color only when one direction is better, and said as "better" or "worse" to screen readers. No pill: it sits beside a quiet note such as "vs. prior 30 days".',
    props: {
      better: "Which direction is good: `down` for errors and latency. Without a better side the change is written in the secondary text color.",
      value: "The change as a fraction: 0.16 for 16% more, -0.04 for 4% less.",
    },
  },
  RollingNumber: {
    description:
      "A formatted number whose digits roll to their new value when it changes, each digit a column sliding to its place, and characters that join it (the fifth digit of 10,000) drop in. It sits still on first paint, and with reduced motion the digits change in place. The page's text stays the value itself, for screen readers, tests and copying.",
    props: {
      value: 'The number, already formatted: "59,415", "$62.19", "137.1M".',
    },
  },
} satisfies ComponentDocs;
