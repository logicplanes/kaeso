import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Meter: {
    description:
      'A limit or quota: label and value, a 6px track cut square like a plate, threshold ticks cut out of the bar, a caption. Past the first threshold the bar turns amber and the value and caption step up from the secondary to the text color, at the limit the bar turns red; the bar alone carries the color, and screen readers hear "near the limit" or "over the limit" after the value; in light the bar\'s end keeps 3:1 against the track. With `shape="gauge"` it is a half circle with the value in its middle, for the one number a page is about.',
    props: {
      "aria-label": "The accessible name, for when there is no visible label.",
      caption: '"Resets in 12 days"; near or over the limit, say what happens, such as "Over the limit: requests are refused".',
      label: "The label above the bar, or the caption under a gauge's arc.",
      leading: "Leading content in the label row (an icon).",
      shape: "A bar (`bar`), or a half circle with square ends (`gauge`) with the number in its middle and, under its caption, near or over the limit.",
      thresholds: "Alert thresholds as ratios, drawn as ticks: [0.5, 0.8]. A gauge marks 0.8 when none are given.",
      tone: "Colors the bar by how full it is (`auto`) or keeps it neutral.",
      trailing: "Content at the end of the label row, such as the value.",
      value: "0–1 (or more when over the limit).",
      valueLabel: '"$32.10 of $50.00"',
      valueText:
        'What screen readers hear as the value when `valueLabel` is not plain text, such as a shortened figure with the exact one in a tooltip: "10,624,184 of 100,000,000".',
    },
  },
  Progress: {
    description:
      "Progress for work over a second: a determinate 6px bar, or an indeterminate 2px line. With reduced motion the indeterminate line spans the width and pulses instead of sliding, so it never reads as a third done.",
    props: {
      "aria-label": "The progress bar's accessible name.",
      value: "0–1; omit for indeterminate.",
    },
  },
} satisfies ComponentDocs;
