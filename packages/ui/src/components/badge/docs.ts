import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Badge: {
    description:
      "A small read-only label on a cut plate: 22px, 12/500, radius 4, set apart from a Tag's pill (24 or 28px) so a mark never looks like a control. Words carry meaning; color supports them, with no dot beside the word. A label longer than its room ends in an ellipsis and shows in full in a tooltip. `ink` is for at most one badge on a view.",
    props: {
      children: "The badge's text, one or two words.",
      tone: "The status it marks; keep it next to words that say the same.",
    },
  },
} satisfies ComponentDocs;
