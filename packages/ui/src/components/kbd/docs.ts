import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Kbd: {
    description: "One key cap: 20px tall, 11/500 Inter, radius 6, fill and ring.",
    props: {
      children: "A key name (`mod`, `shift`, `k`, `enter`) or any text.",
      tone: "On the panel (`default`) or on an ink surface such as a tooltip (`onInk`).",
    },
  },
  KbdSequence: {
    description: "A shortcut: `⌘ K`, or a sequence like `G then L`.",
    props: {
      keys: 'Chords are arrays pressed together; the outer list is pressed in order: `[["g"], ["l"]]` → G then L.',
      tone: "On the panel (`default`) or on an ink surface such as a tooltip (`onInk`).",
    },
  },
} satisfies ComponentDocs;
