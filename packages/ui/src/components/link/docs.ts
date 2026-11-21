import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Link: {
    description: "A link. Router links come from `LinkProvider`.",
    props: {
      children: "The link's text.",
      external: "Opens in a new tab, adds rel and a 14px ↗. Detected from the href when not set.",
      href: "The address the link goes to.",
      variant:
        "Underlined in text (`inline`) or on a line of its own under a form, with a touch target a finger can hit (`action`), secondary until hovered in footers and lists (`quiet`), on its own line in full text that steps back on hover (`standalone`), inside a heading that links to itself (`heading`), or as a page to read next on a cut plate, its name strong and a sentence under it (`card`).",
    },
  },
} satisfies ComponentDocs;
