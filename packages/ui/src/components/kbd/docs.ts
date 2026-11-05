import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Kbd: {
    description: "A key cap, 20px tall in 11/500 Inter with a fill and ring, or with `keys` a whole shortcut: `⌘ K`, or a sequence like `G then L`.",
    props: {
      children: "A key name (`mod`, `shift`, `k`, `enter`) or any text, for one key.",
      keys: 'A whole shortcut instead of one key. Chords are arrays pressed together, and the outer list is pressed in order: `[["g"], ["l"]]` is G then L.',
      tone: "On the panel (`default`) or on an ink surface such as a tooltip (`onInk`).",
    },
  },
} satisfies ComponentDocs;
