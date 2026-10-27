import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  ButtonGroup: {
    description:
      "Related actions side by side: connected pills, or spaced with `attached={false}`. An item with `pressed` is a toggle; pass `false` while it is off, and the selected item turns ink.",
    props: {
      "aria-label": "Names the group for assistive technology.",
      attached: "Connected pills (default) or separate pills with a gap.",
      children: "The buttons, joined into one pill.",
      size: "The size of every button in the group.",
    },
  },
} satisfies ComponentDocs;
