import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  CopyButton: {
    description:
      'Copies a value, with a check and "Copied" for 1.2s: a circle (the default), a read-only pill field with the value in mono and a circle inside (`field`), or inline mono text that copies itself on click (`inline`). Inline text shows a copy icon on hover and keyboard focus that takes no room where its row has some to spare, ends in an ellipsis when it is longer than its container, and shows the whole value in its tooltip until it is copied.',
    props: {
      children: "What a field or inline text shows, when it differs from the copied text, such as a redacted form; the value is still what is copied.",
      display: "A circle (`icon`), a read-only pill field (`field`), or mono text inside a sentence or a row (`inline`).",
      label: 'What is being copied, for the accessible name and the tooltip: "Copy key". Defaults to "Copy" in the provider\'s language.',
      onCopy: "Called with the copied text.",
      size: "The circle's height, or a field's: 32px (`sm`) by default.",
      value: "The text that is copied.",
      variant: "A filled circle (`secondary`), a ghost (`ghost`), or a ghost in secondary text until hovered (`quiet`).",
    },
  },
} satisfies ComponentDocs;
