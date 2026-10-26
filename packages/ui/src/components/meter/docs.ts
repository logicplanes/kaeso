import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Meter: {
    description:
      'A limit or quota: label and value, a 6px pill track, threshold ticks cut out of the bar, a caption. Past the first threshold the bar turns amber and the value and caption turn the warning color, at the limit red, and screen readers hear "near the limit" or "over the limit" after the value; in light the bar\'s end keeps 3:1 against the track.',
    props: {
      "aria-label": "The accessible name, for when there is no visible label.",
      caption: '"Resets in 12 days"; near or over the limit, say what happens, such as "Over the limit: requests are refused".',
      label: "The label above the bar.",
      leading: "Leading content in the label row (an icon).",
      thresholds: "Alert thresholds as ratios, drawn as ticks: [0.5, 0.8].",
      tone: "Colors the bar by how full it is (`auto`) or keeps it neutral.",
      trailing: "Content at the end of the label row, such as the value.",
      value: "0–1 (or more when over the limit).",
      valueLabel: '"$32.10 of $50.00"',
    },
  },
  Progress: {
    description:
      "Progress for work over a second: a determinate pill, or an indeterminate 2px line. With reduced motion the indeterminate line spans the width and pulses instead of sliding, so it never reads as a third done.",
    props: {
      "aria-label": "The progress bar's accessible name.",
      value: "0–1; omit for indeterminate.",
    },
  },
} satisfies ComponentDocs;
